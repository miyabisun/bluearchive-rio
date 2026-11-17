//! Reading the game in a thread of its own: every frame of its window is read for a battle
//! record (kept), the list of records (its rows kept, for the times they tell), an
//! opponent's dialog and the formation dialog (counselled from the records, and marked over
//! the game). What it finds is reported to the window (`app`).

use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

use crate::{
    history::Plate,
    list::{ROW_ACROSS, ROW_TALL, Row, Shown},
    listing::{self, Fight, Pointed, Told},
    opponent::Front,
    store::{Counsel, Kept, Store},
};

/// A screen of the game that Rio reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    /// A battle record (戦闘履歴): whether it is kept, by now or from before. Not kept, it
    /// is being read, or has not stood still for long enough yet.
    Record { kept: bool },
    /// The list of battle records (戦闘履歴, the dialog that lists the battles).
    List,
    /// An opponent's dialog (対戦相手).
    Opponent,
    /// The formation dialog (編成).
    Formation,
}

/// Where the reading stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// No window of the game yet.
    NoGame,
    /// The game's frames are read; the screen on it, if it is one Rio reads.
    Watching(Option<Screen>),
    /// The user paused the reading.
    Paused,
    /// The game cannot be read, or the records not kept: why.
    Failed(String),
}

/// The opponent the game showed last, and what the records counsel against them.
#[derive(Debug, Clone, PartialEq)]
pub struct Opponent {
    pub front: Front,
    /// The opponent's name as the game draws it, where its dialog was read.
    pub name: Option<Plate>,
    pub counsel: Counsel,
}

/// What the reading found.
#[derive(Debug, Clone)]
pub enum Update {
    /// The battles kept, the latest first: as the app starts, and after one more is kept.
    Battles(Vec<Kept>),
    Opponent(Opponent),
}

/// A record is taken for opened from the list when it shows this shortly after the list.
const OPENING: Duration = Duration::from_secs(3);

/// The list of records from frame to frame: what its rows read in the frame before, the rows
/// as they were last told with their tops, and which of those the pointer was over last (a
/// record opens from the row whose button is pressed).
#[derive(Default)]
#[cfg_attr(not(windows), allow(dead_code))]
struct Listing {
    before: Vec<Option<Row>>,
    told: Option<(Instant, Vec<Option<Told>>, Vec<f32>)>,
    over: Pointed,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl Listing {
    /// A frame of the list read at `now`: its rows in full sight, the list up `since` (see
    /// `listing::Up`), `clock` the seconds since 1970. The rows are told (and kept, see
    /// `Store::see`) once two frames in a row read them the same: a list that comes up or
    /// goes is drawn faint for a few frames. For each row told: its top, and whether its
    /// record was read. `None` for a frame that tells nothing: a record coming up over
    /// the list dims it, and it shows no row in full; what was told stays as it is then.
    fn see(
        &mut self,
        store: &Store,
        rows: &[Shown],
        since: Option<i64>,
        clock: i64,
        now: Instant,
    ) -> rusqlite::Result<Option<Vec<(f32, bool)>>> {
        let read: Vec<Option<Row>> = rows.iter().map(|shown| shown.row.clone()).collect();
        let steady = !read.is_empty() && same_rows(&read, &self.before);
        self.before = read;
        if !steady {
            return Ok(None);
        }
        let told = store.see(&self.before, since, clock)?;
        let tops: Vec<f32> = rows.iter().map(|shown| shown.top).collect();
        let read = tops.iter().zip(&told);
        let read = read.filter_map(|(top, told)| told.map(|told| (*top, told.battle.is_some())));
        let read = read.collect();
        self.told = Some((now, told, tops));
        Ok(Some(read))
    }

    /// A frame that shows no list.
    fn miss(&mut self) {
        self.before.clear();
    }

    /// Where the pointer is on the measured screen (`None` off the game): which of the
    /// rows last told it is over.
    fn point(&mut self, at: Option<(f32, f32)>) {
        if let Some((_, _, tops)) = &self.told {
            self.over = listing::pointed(at, tops, ROW_TALL, ROW_ACROSS);
        }
    }

    /// The row a record was opened from (see `listing::opened`), if the record first
    /// showed at `shown`, shortly after the list: the record's fight, the battle's number.
    fn opened(&self, shown: Instant, fight: Fight, battle: i64) -> Option<i64> {
        let (seen, rows, _) = self.told.as_ref()?;
        (shown.duration_since(*seen) <= OPENING)
            .then(|| listing::opened(rows, self.over, fight, battle))?
    }
}

/// Whether two frames read the list's rows the same: a row for a row, the names drawn the
/// same (see `history::Plate::same`).
#[cfg_attr(not(windows), allow(dead_code))]
fn same_rows(now: &[Option<Row>], before: &[Option<Row>]) -> bool {
    let same = |now: &Option<Row>, before: &Option<Row>| match (now, before) {
        (Some(now), Some(before)) => {
            (now.attacked, now.won, now.ago) == (before.attacked, before.won, before.ago)
                && match (&now.opponent, &before.opponent) {
                    (Some(now), Some(before)) => now.same(before),
                    (None, None) => true,
                    _ => false,
                }
        }
        (None, None) => true,
        _ => false,
    };
    now.len() == before.len()
        && now
            .iter()
            .zip(before)
            .all(|(now, before)| same(now, before))
}

/// Starts reading the game, the records kept in the database at `database`. `paused` stops
/// the reading while it is set; what is found goes to `report` with the status then.
pub fn start(
    database: PathBuf,
    paused: Arc<AtomicBool>,
    report: impl Fn(Status, Option<Update>) + Send + 'static,
) {
    std::thread::spawn(move || {
        if let Err(error) = run(&database, &paused, &report) {
            report(Status::Failed(error), None);
        }
    });
}

#[cfg(not(windows))]
fn run(
    _database: &std::path::Path,
    _paused: &AtomicBool,
    _report: &dyn Fn(Status, Option<Update>),
) -> Result<(), String> {
    Err("ゲームのウィンドウを読めるのは Windows だけです。".to_owned())
}

#[cfg(windows)]
fn run(
    database: &std::path::Path,
    paused: &AtomicBool,
    report: &dyn Fn(Status, Option<Update>),
) -> Result<(), String> {
    use std::{
        collections::HashMap,
        sync::{
            atomic::Ordering,
            mpsc::{RecvTimeoutError, channel},
        },
    };

    use crate::{
        aoi,
        capture::{self, Seen, Shot},
        guide::{self, Rank, Visit, Words},
        history::{self, Battle, Reader, Steady},
        listing::{Shows, Up},
        opponent,
        overlay::Overlay,
    };

    /// The seconds since 1970.
    fn clock() -> i64 {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
        now.map_or(0, |now| now.as_secs() as i64)
    }

    /// Where the pointer is on the measured screen, if it is over the game's screen. The
    /// pointer is only asked for: nothing is sent to the game.
    fn pointer(shot: &Shot) -> Option<(f32, f32)> {
        use windows_sys::Win32::{Foundation::POINT, UI::WindowsAndMessaging::GetCursorPos};
        let (origin, size) = (shot.origin?, shot.image.dimensions());
        let scale = history::scale(size.0, size.1)?;
        let mut at = POINT { x: 0, y: 0 };
        // SAFETY: the point outlives the call.
        if unsafe { GetCursorPos(&raw mut at) } == 0 {
            return None;
        }
        let (x, y) = ((at.x - origin.0) as f32, (at.y - origin.1) as f32);
        let inside = x >= 0.0 && y >= 0.0 && x < size.0 as f32 && y < size.1 as f32;
        inside.then(|| history::measured(size, scale, (x, y)))
    }

    /// A student's place in the formation dialog's list, where Aoi read all it is ordered by.
    fn rank(student: &aoi::Held) -> Option<(i64, Rank)> {
        let rank = Rank {
            level: student.level?,
            rarity: student.rarity?,
            bond: student.bond?,
        };
        Some((student.id?, rank))
    }

    let records = |error: rusqlite::Error| format!("戦闘の記録を読み書きできません: {error}");
    if let Some(dir) = database.parent() {
        std::fs::create_dir_all(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    }
    let (reader, store) = (Reader::new(), Store::open(database).map_err(records)?);
    report(
        Status::NoGame,
        Some(Update::Battles(store.battles().map_err(records)?)),
    );
    // Without the window over the game, the counsel still shows in Rio's own.
    let mut overlay = Overlay::new();
    let (mut steady, mut visit) = (Steady::new(Battle::alike), Visit::default());
    // The opponent whose dialog was open last: the formation dialog shows the front again,
    // without the name.
    let mut met: Option<(Front, Option<Plate>)> = None;
    let mut counselled: Option<Opponent> = None;
    // The students held and their ranks, as Aoi wrote them when the opponent came up.
    let mut roster: Option<Vec<i64>> = None;
    let mut ranks: HashMap<i64, Rank> = HashMap::new();
    // The list of records: how long it is up, and what it told from frame to frame.
    let mut up = Up::default();
    let mut listing = Listing::default();
    // Since when a record shows, and whether the one told last was kept just then.
    let mut record_since: Option<Instant> = None;
    let mut fresh = false;

    let (seen, window) = channel();
    let frames = capture::frames(move |what| {
        let _ = seen.send(what);
    });
    let (mut status, mut trouble) = (Status::NoGame, None);
    let tell = |now: Status, status: &mut Status| {
        if now != *status {
            *status = now.clone();
            report(now, None);
        }
    };
    loop {
        if let Some(overlay) = &overlay {
            overlay.pump();
        }
        for what in window.try_iter() {
            trouble = match what {
                Seen::Failed(message) => {
                    Some(format!("ゲームのウィンドウを取り込めません: {message}"))
                }
                Seen::Found | Seen::Gone => None,
            };
        }
        let shot = match frames.recv_timeout(Duration::from_secs(1)) {
            Ok(shot) if !paused.load(Ordering::Relaxed) => shot,
            // Paused, or no frame for a while (no window, or one hidden): nothing to lie over.
            Ok(_) | Err(RecvTimeoutError::Timeout) => {
                if let Some(overlay) = &mut overlay {
                    overlay.hide();
                }
                let now = if paused.load(Ordering::Relaxed) {
                    Status::Paused
                } else {
                    trouble.clone().map_or(Status::NoGame, Status::Failed)
                };
                tell(now, &mut status);
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
        };
        let (frame, now) = (&shot.image, Instant::now());
        let record = reader.read(frame);
        let recorded = record.is_some() || reader.titled(frame);
        let listed = if recorded { None } else { reader.list(frame) };
        let shows = match (recorded, &listed) {
            (true, _) => Shows::Record,
            (false, Some(_)) => Shows::List,
            (false, None) => Shows::Other,
        };
        let since = up.see(shows, now, clock());
        record_since = recorded.then(|| record_since.unwrap_or(now));
        if let Some(battle) = steady.see(record, now) {
            let saved = store.keep(battle).map_err(records)?;
            fresh = saved.new;
            // The row the record was opened from, where the list showed just before it: the
            // row's time is the battle's.
            let opponent = if battle.user_attacked {
                &battle.defender
            } else {
                &battle.attacker
            };
            let fight = Fight {
                attacked: battle.user_attacked,
                won: battle.attacker_won == battle.user_attacked,
                opponent: match &opponent.plate {
                    Some(name) => store.known(name).map_err(records)?,
                    None => None,
                },
            };
            let row = record_since.and_then(|shown| listing.opened(shown, fight, saved.id));
            let linked = row.is_some();
            if let Some(row) = row {
                store.link(row, saved.id).map_err(records)?;
            }
            if saved.new || linked {
                report(
                    status.clone(),
                    Some(Update::Battles(store.battles().map_err(records)?)),
                );
            }
            // What is counselled may change with the battle just kept.
            if saved.new
                && let Some(mut opponent) = counselled.take()
            {
                opponent.counsel = store
                    .counsel(&opponent.front, opponent.name.as_ref(), roster.as_deref())
                    .map_err(records)?;
                report(status.clone(), Some(Update::Opponent(opponent.clone())));
                counselled = Some(opponent);
            }
        }

        // Told, the record is in the database: kept just now, or found there from before.
        let mut screen = recorded.then_some(Screen::Record {
            kept: steady.told(),
        });
        let mut marks = Vec::new();
        if recorded {
            marks = guide::for_record(match (steady.told(), fresh) {
                (false, _) => Words::Reading,
                (true, true) => Words::Kept,
                (true, false) => Words::Known,
            });
        }
        if let Some(rows) = &listed {
            screen = Some(Screen::List);
            let told = listing.see(&store, rows, since, clock(), now);
            if let Some(read) = told.map_err(records)? {
                marks = guide::for_list(&read, since.is_some());
            }
        } else {
            listing.miss();
        }
        // The row the pointer is over: asked while the list shows (dimmed under a record
        // coming up as well) and as a record first shows, when the pointer still lies where
        // the button was pressed. Its place is in the screen's pixels only on the overlay's
        // thread.
        if listed.is_some() || record_since == Some(now) {
            listing.point(overlay.as_ref().and_then(|_| pointer(&shot)));
        }
        let (dialog, forming) = (reader.front(frame), reader.formation(frame));
        if let Some(front) = dialog {
            met = Some((front, opponent::name(frame)));
            screen = Some(Screen::Opponent);
        } else if forming.is_some() {
            screen = Some(Screen::Formation);
        }
        if let Some(front) = dialog.or(forming.as_ref().map(|shown| shown.front)) {
            // The name is the opponent's while the front is the one its dialog showed.
            let name = met.as_ref().filter(|(met, _)| *met == front);
            let name = name.and_then(|(_, name)| name.clone());
            let same = |known: &Opponent| known.front == front && known.name == name;
            if !counselled.as_ref().is_some_and(same) {
                let held = aoi::held().ok();
                roster = held
                    .as_ref()
                    .map(|held| held.iter().filter_map(|student| student.id).collect());
                ranks = held.iter().flatten().filter_map(rank).collect();
                let counsel = store
                    .counsel(&front, name.as_ref(), roster.as_deref())
                    .map_err(records)?;
                let opponent = Opponent {
                    front,
                    name,
                    counsel,
                };
                report(status.clone(), Some(Update::Opponent(opponent.clone())));
                counselled = Some(opponent);
            }
            if let Some(Opponent { counsel, .. }) = &counselled {
                marks = match &forming {
                    Some(shown) => guide::for_formation(
                        shown,
                        counsel.behind(),
                        counsel.attack.as_ref().map(|tried| tried.attack),
                        visit.enter(&shown.placed, now),
                        &ranks,
                    ),
                    None => counsel.behind().map_or_else(Vec::new, guide::for_opponent),
                };
            }
        }
        if let Some(overlay) = &mut overlay {
            match shot.origin {
                Some(origin) if shot.in_front => overlay.show(&marks, origin, frame.dimensions()),
                _ => overlay.hide(),
            }
        }
        tell(Status::Watching(screen), &mut status);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::Path,
        time::{Duration, Instant},
    };

    use image::{GrayImage, Luma};

    use super::Listing;
    use crate::{
        history::{Battle, Fighter, Party, Plate},
        list::{Ago, Row, Shown, Unit},
        listing::Fight,
        store::Store,
    };

    /// A name of `strokes` upright strokes.
    fn plate(strokes: u32) -> Plate {
        let mut drawn = GrayImage::new(80, 32);
        for stroke in 0..strokes {
            for y in 4..28 {
                for x in 0..4 {
                    drawn.put_pixel(4 + stroke * 12 + x, y, Luma([255]));
                }
            }
        }
        Plate(drawn)
    }

    /// A row with its top at `top`: a defense won `minutes` ago against the player of a name.
    fn shown(top: f32, name: u32, minutes: u32) -> Shown {
        let ago = Ago {
            count: minutes,
            unit: Unit::Minute,
        };
        Shown {
            top,
            row: Some(Row {
                attacked: false,
                won: true,
                opponent: Some(plate(name)),
                ago,
            }),
        }
    }

    /// The record of a defense the user won against the player of a name.
    fn record(name: u32) -> Battle {
        let party = |strokes: u32, first: i64| Party {
            plate: Some(plate(strokes)),
            team: [0, 1, 2, 3, 4, 5].map(|n| Fighter {
                student: first + n,
                damage: 100 + n as u32,
            }),
        };
        Battle {
            attacker: party(name, 10000),
            defender: party(1, 20000),
            attacker_won: false,
            user_attacked: false,
        }
    }

    #[test]
    fn a_record_is_the_row_whose_button_was_pressed_though_the_list_dims_under_it() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        let start = Instant::now();
        let after = |millis: u64| start + Duration::from_millis(millis);
        let mut listing = Listing::default();
        let rows = [shown(398.0, 5, 1), shown(572.0, 7, 9)];
        // The list comes up: told once two frames read the same, no record read yet.
        assert_eq!(
            listing.see(&store, &rows, Some(1000), 1000, start).unwrap(),
            None,
            "one frame is not enough"
        );
        let told = listing.see(&store, &rows, Some(1000), 1000, after(100));
        assert_eq!(told.unwrap(), Some(vec![(398.0, false), (572.0, false)]));
        // The first row's button is pressed. The record comes up over the list, which dims:
        // for some frames it shows no row in full.
        listing.point(Some((1750.0, 480.0)));
        for frame in 2..5 {
            let dimmed = listing.see(&store, &[], Some(1000), 1000, after(frame * 100));
            assert_eq!(dimmed.unwrap(), None, "nothing to tell");
            listing.point(Some((1750.0, 480.0)));
        }
        // The record is read: the user defended and won against the first row's player.
        let battle = store.keep(&record(5)).unwrap();
        let fight = Fight {
            attacked: false,
            won: true,
            opponent: store.known(&plate(5)).unwrap(),
        };
        let first = store.listed().unwrap()[0].id;
        assert_eq!(listing.opened(after(600), fight, battle.id), Some(first));
        assert_eq!(
            listing.opened(after(60_000), fight, battle.id),
            None,
            "a record that shows long after the list is no row's"
        );
        // Back on the list, the row tells that its record is read.
        store.link(first, battle.id).unwrap();
        listing
            .see(&store, &rows, Some(1000), 1002, after(2000))
            .unwrap();
        let told = listing.see(&store, &rows, Some(1000), 1002, after(2100));
        assert_eq!(told.unwrap(), Some(vec![(398.0, true), (572.0, false)]));
    }

    #[test]
    fn the_pointer_tells_nothing_before_a_row_is_told() {
        let mut listing = Listing::default();
        listing.point(Some((1750.0, 480.0)));
        let fight = Fight {
            attacked: false,
            won: true,
            opponent: None,
        };
        assert_eq!(listing.opened(Instant::now(), fight, 1), None);
    }
}
