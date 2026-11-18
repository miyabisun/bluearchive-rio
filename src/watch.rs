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
    list::{self, ROW_ACROSS, ROW_TALL, Row, Shown},
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

/// A record is taken for opened from the list when it shows this shortly after the list:
/// the game may take its time to fetch a record.
const OPENING: Duration = Duration::from_secs(10);

/// The list of records from frame to frame: what its rows read in the frame before and what
/// each was told to be, the rows as they were last told with their tops, and which of those
/// the pointer was over last (a record opens from the row whose button is pressed).
#[derive(Default)]
pub struct Listing {
    before: Vec<Option<Row>>,
    tops: Vec<f32>,
    known: Vec<Option<Told>>,
    told: Option<(Instant, Vec<Option<Told>>, Vec<f32>)>,
    over: Pointed,
}

impl Listing {
    /// A frame of the list read at `now`: its rows in full sight, the list up `since` (see
    /// `listing::Up`), `clock` the seconds since 1970. The rows are told by the rows the
    /// frame before showed too, wherever the list moved between the two
    /// (`listing::shift`): a row kept is told as it comes into sight, a row not kept yet
    /// is kept once two frames in a row read it the same (a list that comes up or goes is
    /// drawn faint for a few frames; see `Store::see`). A row that reads otherwise than a
    /// frame ago is the row it was then: the ring a press draws lies over it. For each row
    /// whose record is known to be read or not: its top, and which. `None` for a frame
    /// that tells nothing, and leaves what was told as it is: one that shows no row in
    /// full (a record coming up over the list dims it), or none of the rows the frame
    /// before showed (the list jumped, or is drawn faint).
    pub fn see(
        &mut self,
        store: &Store,
        rows: &[Shown],
        since: Option<i64>,
        clock: i64,
        now: Instant,
    ) -> rusqlite::Result<Option<Vec<(f32, bool)>>> {
        if rows.is_empty() {
            return Ok(None);
        }
        let read: Vec<Option<Row>> = rows.iter().map(|shown| shown.row.clone()).collect();
        let tops: Vec<f32> = rows.iter().map(|shown| shown.top).collect();
        let (before, was) = (
            std::mem::take(&mut self.before),
            std::mem::take(&mut self.known),
        );
        let stood = || listing::still((&self.tops, &tops), &before, &read, Row::same);
        let moved = listing::shift(&before, &read, Row::same).or_else(|| stood().then_some(0));
        // The row of the frame before that a row of this one is, and the row as both
        // frames read it.
        let earlier = |row: usize| {
            let at = usize::try_from(row as isize + moved?).ok()?;
            (at < before.len()).then_some(at)
        };
        let twice = |row: usize| {
            let (before, now) = (before[earlier(row)?].as_ref()?, read[row].as_ref()?);
            before.same(now).then(|| now.clone())
        };
        // A frame that shares no row with the one before tells nothing, and leaves what was
        // told last: the list jumped (its rows are told a frame on), or it is drawn faint
        // as a record comes up over it, and the record is of a row told before.
        if !(0..rows.len()).any(|row| earlier(row).is_some()) {
            (self.before, self.tops, self.known) = (read, tops, vec![None; rows.len()]);
            return Ok(None);
        }
        // A row the frame before showed counts as both frames read it (not at all where
        // they differ: the ring a press draws lies over it); a row that came into sight,
        // as this frame reads it, to be told if it is a row kept.
        let came = |row: usize| earlier(row).is_none();
        let seen: Vec<Option<Row>> = (0..rows.len())
            .map(|row| {
                if came(row) {
                    read[row].clone()
                } else {
                    twice(row)
                }
            })
            .collect();
        let carried: Vec<Option<i64>> = (0..rows.len())
            .map(|row| Some(was[earlier(row)?].filter(|told| told.sure)?.id))
            .collect();
        let stayed: Vec<bool> = (0..rows.len()).map(|row| !came(row)).collect();
        let sights = store.see(&seen, &carried, &stayed, since, clock)?;
        let (mut known, mut marks) = (vec![None; rows.len()], Vec::new());
        for (row, sight) in sights.rows.iter().enumerate() {
            // A row not told now is the row it was a frame ago.
            known[row] = sight.row.or_else(|| was[earlier(row)?]);
            let told = known[row].map(|told| told.battle.is_some());
            if let Some(read) = sight.read.or(told) {
                marks.push((rows[row].top, read));
            }
        }
        let (above, below) = (sights.above, sights.below);
        // With the rows the list's edges cut, where their buttons show: a record may be
        // opened from those too.
        let cut = list::cut(&tops);
        let ends = [(cut.0, above), (cut.1, below)].map(|(top, row)| top.map(|top| (top, row)));
        let full = tops.iter().copied().zip(known.iter().copied());
        let told = ends[0].into_iter().chain(full).chain(ends[1]);
        let (shown, told): (Vec<f32>, Vec<Option<Told>>) = told.unzip();
        self.told = Some((now, told, shown));
        (self.before, self.tops, self.known) = (read, tops, known);
        Ok(Some(marks))
    }

    /// A frame that shows no list: a record (the list is still up under it, and comes back
    /// as it was), or another screen.
    pub fn miss(&mut self, record: bool) {
        if !record {
            self.before.clear();
            self.known.clear();
        }
    }

    /// Where the pointer is on the measured screen (`None` off the game): which of the
    /// rows last told it is over.
    pub fn point(&mut self, at: Option<(f32, f32)>) {
        if let Some((_, _, tops)) = &self.told {
            self.over = listing::pointed(at, tops, ROW_TALL, ROW_ACROSS);
        }
    }

    /// The row a record was opened from (see `listing::opened`), if the record first
    /// showed at `shown`, not long after the list: the record's fight, the battle's number.
    pub fn opened(&self, shown: Instant, fight: Fight, battle: i64) -> Option<i64> {
        let (seen, rows, _) = self.told.as_ref()?;
        (shown.duration_since(*seen) <= OPENING)
            .then(|| listing::opened(rows, self.over, fight, battle))?
    }

    /// What the list told last, in a line for the trail: for each row from the top its
    /// number ("?" for a row not told, "~" before one only likely) and whether its record
    /// is read ("r") or not ("u"); then what the pointer is over, if `pointer`.
    pub fn trail(&self, pointer: bool) -> String {
        let row = |row: &Option<Told>| match row {
            None => "?".to_owned(),
            Some(told) => format!(
                "{}{}{}",
                if told.sure { "" } else { "~" },
                told.id,
                if told.battle.is_some() { "r" } else { "u" }
            ),
        };
        let rows = self.told.iter().flat_map(|(_, rows, _)| rows).map(row);
        let rows: Vec<String> = rows.collect();
        let over = format!(" pointer {:?}", self.over);
        format!(
            "rows [{}]{}",
            rows.join(" "),
            if pointer { &over } else { "" }
        )
    }
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
        trail::Trail,
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
    let mut trail = Trail::open(database.parent().unwrap_or(std::path::Path::new(".")));
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
            let (battle, new, list) = (saved.id, saved.new, listing.trail(true));
            trail.note(
                clock(),
                format!("record: battle {battle} (new {new}) of {fight:?}; {list}; row {row:?}"),
            );
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
                let list = listing.trail(false);
                trail.note(clock(), format!("list: up since {since:?}; {list}"));
            }
        } else {
            listing.miss(recorded);
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
        // a frame reads its rows as they are not, then for some frames it shows no row in
        // full.
        listing.point(Some((1750.0, 480.0)));
        let faint = [shown(398.0, 2, 3), shown(572.0, 3, 4)];
        let told = listing.see(&store, &faint, Some(1000), 1000, after(150));
        assert_eq!(told.unwrap(), None, "nothing is told of it");
        listing.point(Some((1750.0, 480.0)));
        for frame in 2..5 {
            let dimmed = listing.see(&store, &[], Some(1000), 1000, after(frame * 100));
            assert_eq!(dimmed.unwrap(), None, "nothing to tell");
            listing.point(Some((1750.0, 480.0)));
        }
        // The record is read: the user defended and won against the first row's player.
        listing.miss(true);
        let battle = store.keep(&record(5)).unwrap();
        let fight = Fight {
            attacked: false,
            won: true,
            opponent: store.known(&plate(5)).unwrap(),
        };
        let first = store.listed(0).unwrap()[0].id;
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
    fn the_rows_are_told_as_the_list_moves_under_them() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        let start = Instant::now();
        let mut frame = 0;
        let mut listing = Listing::default();
        let mut see = |listing: &mut Listing, rows: &[Shown]| {
            frame += 1;
            let now = start + Duration::from_millis(frame * 100);
            listing.see(&store, rows, Some(1000), 1000, now).unwrap()
        };
        // Rows from the tops down: the names of so many strokes, as many minutes ago.
        let tops = [516.0, 691.0, 866.0];
        let rows = |names: &[u32]| -> Vec<Shown> {
            let row = |(top, name): (&f32, &u32)| shown(*top, *name, *name);
            tops.iter().zip(names).map(row).collect()
        };
        see(&mut listing, &rows(&[2, 3, 4]));
        see(&mut listing, &rows(&[2, 3, 4]));
        // The list moves up by a row: the two rows seen before are told at once, where they
        // are now; the row that came into sight, a frame later.
        let moved = rows(&[3, 4, 5]);
        assert_eq!(
            see(&mut listing, &moved),
            Some(vec![(516.0, false), (691.0, false)])
        );
        assert_eq!(see(&mut listing, &moved).map(|told| told.len()), Some(3));
        let ids: Vec<i64> = store.listed(0).unwrap().iter().map(|row| row.id).collect();
        let above: Vec<Option<i64>> = store
            .listed(0)
            .unwrap()
            .iter()
            .map(|row| row.above)
            .collect();
        assert_eq!(ids.len(), 4, "four rows, none twice");
        assert_eq!(above, [None, Some(ids[0]), Some(ids[1]), Some(ids[2])]);
        // The row the list's top cuts now is the first one kept: its button shows still,
        // and a record opened from there is its record.
        let battle = store.keep(&record(2)).unwrap();
        let fight = Fight {
            attacked: false,
            won: true,
            opponent: store.known(&plate(2)).unwrap(),
        };
        listing.point(Some((1750.0, 450.0)));
        let now = start + Duration::from_millis(1000);
        assert_eq!(listing.opened(now, fight, battle.id), Some(ids[0]));
        // The list jumps: nothing is told of rows no frame before showed.
        assert_eq!(see(&mut listing, &rows(&[6, 7])), None);
    }

    #[test]
    fn a_row_keeps_what_it_was_told_while_a_press_hides_it() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        let start = Instant::now();
        let after = |millis: u64| start + Duration::from_millis(millis);
        let mut listing = Listing::default();
        let rows = [shown(398.0, 5, 1), shown(572.0, 7, 9)];
        listing.see(&store, &rows, Some(1000), 1000, start).unwrap();
        let told = listing.see(&store, &rows, Some(1000), 1000, after(100));
        assert_eq!(told.unwrap().map(|told| told.len()), Some(2));
        // The ring a press draws lies over the first row: it does not read, then reads as
        // another name.
        let pressed = [
            Shown {
                top: 398.0,
                row: None,
            },
            shown(572.0, 7, 9),
        ];
        let told = listing.see(&store, &pressed, Some(1000), 1000, after(200));
        assert_eq!(told.unwrap(), Some(vec![(398.0, false), (572.0, false)]));
        let pressed = [shown(398.0, 6, 1), shown(572.0, 7, 9)];
        for frame in 3..5 {
            let told = listing.see(&store, &pressed, Some(1000), 1000, after(frame * 100));
            assert_eq!(told.unwrap(), Some(vec![(398.0, false), (572.0, false)]));
        }
        assert_eq!(store.listed(0).unwrap().len(), 2, "no row is made of it");
        let battle = store.keep(&record(5)).unwrap();
        let fight = Fight {
            attacked: false,
            won: true,
            opponent: store.known(&plate(5)).unwrap(),
        };
        let first = store.listed(0).unwrap()[0].id;
        assert_eq!(listing.opened(after(700), fight, battle.id), Some(first));
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

    /// A name whose strokes stand where the bits of `name` are set (1 to 63).
    fn named(name: u32) -> Plate {
        let mut drawn = GrayImage::new(80, 32);
        for stroke in (0..6).filter(|bit| name >> bit & 1 == 1) {
            for y in 4..28 {
                for x in 0..4 {
                    drawn.put_pixel(4 + stroke * 12 + x, y, Luma([255]));
                }
            }
        }
        Plate(drawn)
    }

    /// A battle of the game's list: the opponent's name (0 for an anonymous one), whether
    /// the user attacked and won, and how many seconds before the first look it was.
    type Fought = (u32, bool, bool, i64);
    const HOUR: i64 = 3600;
    const DAY: i64 = 86400;
    /// A list from the newest battle down: the same opponent beaten twice within the hour
    /// (rows that tell the same, side by side), and the players 5 and 6 fought day after
    /// day in the same order (rows that tell the same but for the day).
    const LIST: [Fought; 18] = [
        (1, true, true, 120),
        (1, true, false, 400),
        (2, false, true, 3000),
        (0, false, true, 4 * HOUR),
        (3, true, true, 9 * HOUR),
        (3, true, true, 9 * HOUR + 60),
        (4, false, false, 20 * HOUR),
        (5, true, true, DAY + 3000),
        (6, true, true, DAY + 3600),
        (7, false, true, DAY + 9000),
        (5, true, true, 2 * DAY + 3000),
        (6, true, true, 2 * DAY + 3600),
        (8, false, false, 2 * DAY + 9000),
        (5, true, true, 3 * DAY + 3000),
        (6, true, true, 3 * DAY + 3600),
        (9, true, false, 3 * DAY + 5000),
        (9, true, false, 3 * DAY + 5100),
        (10, true, true, 5 * DAY),
    ];
    /// When the first look is (seconds since 1970).
    const FIRST: i64 = 1_000_000;

    /// What the game writes of a battle so many seconds ago.
    fn ago(seconds: i64) -> Ago {
        let (count, unit) = match seconds {
            ..HOUR => (seconds / 60, Unit::Minute),
            HOUR..DAY => (seconds / HOUR, Unit::Hour),
            _ => (seconds / DAY, Unit::Day),
        };
        Ago {
            count: count as u32,
            unit,
        }
    }

    /// The rows in full sight of a list that came up `later` seconds after the first look
    /// and is scrolled `down` pixels.
    fn sight(list: &[Fought], later: i64, down: f32) -> Vec<Shown> {
        let row = |(at, &(name, attacked, won, age)): (usize, &Fought)| {
            let top = 396.0 + at as f32 * 174.7 - down;
            (387.0..=884.0).contains(&top).then(|| Shown {
                top,
                row: Some(Row {
                    attacked,
                    won,
                    opponent: (name > 0).then(|| named(name)),
                    ago: ago(age + later),
                }),
            })
        };
        list.iter().enumerate().filter_map(row).collect()
    }

    /// Shows a list that comes up `later` seconds after the first look, scrolled frame by
    /// frame as `scroll` says: whether the record of each row in the last frame is read.
    fn look(
        listing: &mut Listing,
        store: &Store,
        list: &[Fought],
        later: i64,
        scroll: impl Iterator<Item = f32>,
    ) -> Vec<bool> {
        let start = Instant::now();
        let mut told = None;
        listing.miss(false);
        for (frame, down) in scroll.enumerate() {
            let now = start + Duration::from_millis(frame as u64 * 100);
            let (since, clock) = (FIRST + later, FIRST + later + frame as i64 / 10);
            let rows = sight(list, later, down);
            told = listing.see(store, &rows, Some(since), clock, now).unwrap();
        }
        told.iter().flatten().map(|(_, read)| *read).collect()
    }

    /// The list scrolled from `from` to `to` pixels, `step` a frame, and left there for
    /// two frames more.
    fn scrolled(from: f32, to: f32, step: f32) -> impl Iterator<Item = f32> {
        let frames = ((to - from) / step).abs() as usize;
        let at = move |frame: usize| from + (to - from).signum() * step * frame as f32;
        (0..=frames).map(at).chain([to, to])
    }

    /// The fights the rows of a list tell, from the top.
    fn fights(store: &Store, list: &[Fought]) -> Vec<Fight> {
        let fight = |&(name, attacked, won, _): &Fought| Fight {
            attacked,
            won,
            opponent: (name > 0).then(|| store.known(&named(name)).unwrap().expect("met")),
        };
        list.iter().map(fight).collect()
    }

    /// The rows kept, in runs of rows that stand side by side, each from its top: what
    /// they tell.
    fn runs(store: &Store) -> Vec<Vec<Fight>> {
        let rows = store.listed(0).unwrap();
        let under = |id: i64| rows.iter().find(|row| row.above == Some(id));
        let run = |top: &crate::listing::Listed| {
            let rest = std::iter::successors(under(top.id), |row| under(row.id));
            std::iter::once(top)
                .chain(rest)
                .map(|row| row.fight)
                .collect()
        };
        rows.iter()
            .filter(|row| row.above.is_none())
            .map(run)
            .collect()
    }

    #[test]
    fn a_list_scrolled_through_is_kept_as_it_stands() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        let mut listing = Listing::default();
        let scroll = [0.0; 3].into_iter().chain(scrolled(0.0, 2500.0, 25.0));
        look(&mut listing, &store, &LIST, 0, scroll);
        assert_eq!(
            runs(&store),
            [fights(&store, &LIST)],
            "one run, each row once"
        );
        // The record of the second row is read.
        let read = store.keep(&record(1)).unwrap();
        store.link(store.listed(0).unwrap()[1].id, read.id).unwrap();
        // An hour on, two battles more: the list is opened anew and scrolled through, a
        // row a frame this time.
        let more = [(11, false, true, 300 - HOUR), (1, true, true, 900 - HOUR)];
        let list = [&more[..], &LIST[..]].concat();
        // At its top: the two new rows, and the row that was on top. Then a row down: the
        // row whose record is read comes into sight, and is told at once.
        let top = look(&mut listing, &store, &list, HOUR, [0.0; 3].into_iter());
        assert_eq!(top, [false, false, false]);
        let down = look(
            &mut listing,
            &store,
            &list,
            HOUR,
            [0.0, 0.0, 174.7].into_iter(),
        );
        assert_eq!(down, [false, false, true]);
        let scroll = [0.0; 3].into_iter().chain(scrolled(0.0, 2900.0, 174.7));
        look(&mut listing, &store, &list, HOUR, scroll);
        assert_eq!(runs(&store), [fights(&store, &list)], "the new rows on top");
    }

    #[test]
    fn a_list_flung_and_scrolled_back_is_kept_as_it_stands() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        let mut listing = Listing::default();
        // The top, a fling down to the rows of two days ago, and slowly back up.
        let scroll = [0.0, 0.0, 0.0, 700.0, 1500.0, 1500.0, 1500.0];
        let scroll = scroll.into_iter().chain(scrolled(1500.0, 0.0, 40.0));
        look(&mut listing, &store, &LIST, 0, scroll);
        assert_eq!(runs(&store), [fights(&store, &LIST[..12])]);
        // The day after, the list opens on rows never seen and is flung to the rows that
        // tell what the rows of the day before told then: none is taken for another.
        let more: Vec<Fought> = (0..6)
            .map(|n| (20 + n, false, true, 600 * (n as i64 + 1) - DAY))
            .collect();
        let list = [&more[..], &LIST[..]].concat();
        let scroll = [0.0, 0.0, 2800.0, 2800.0, 2800.0];
        let scroll = scroll.into_iter().chain(scrolled(2800.0, 0.0, 40.0));
        look(&mut listing, &store, &list, DAY, scroll);
        assert_eq!(runs(&store), [fights(&store, &list[..19])]);
    }

    #[test]
    fn of_two_rows_that_tell_the_same_the_one_pressed_has_the_record() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        let mut listing = Listing::default();
        // The rows 4 and 5 tell the same: the player 3 beaten twice within the hour.
        look(&mut listing, &store, &LIST, 0, scrolled(0.0, 500.0, 25.0));
        let shown = sight(&LIST, 0, 500.0);
        let tops: Vec<f32> = shown.iter().map(|row| row.top).collect();
        assert_eq!(tops.len(), 3, "the rows 3, 4 and 5");
        let fight = fights(&store, &LIST[..6])[5];
        let twins: Vec<i64> = store
            .listed(0)
            .unwrap()
            .iter()
            .filter(|row| row.fight == fight)
            .map(|row| row.id)
            .collect();
        let now = Instant::now();
        assert_eq!(
            listing.opened(now, fight, 1),
            Some(twins[0]),
            "the upper, unless told"
        );
        // The lower one's button is pressed.
        listing.point(Some((1750.0, tops[2] + 80.0)));
        assert_eq!(listing.opened(now, fight, 1), Some(twins[1]));
    }
}
