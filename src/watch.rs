//! Reading the game in a thread of its own: every frame of its window is read for a battle
//! record (kept), the list of records (its rows kept, for the times they tell), an
//! opponent's dialog and the formation dialog (counselled from the records, and marked over
//! the game). What it finds is reported to the window (`app`).

use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

use crate::{
    history::Plate,
    opponent::Front,
    store::{Counsel, Kept},
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
        time::{Duration, Instant},
    };

    use crate::{
        aoi,
        capture::{self, Seen, Shot},
        guide::{self, Rank, Visit, Words},
        history::{self, Battle, Reader, Steady},
        list::{ROW_ACROSS, ROW_TALL, Row},
        listing::{self, Fight, Pointed, Shows, Told, Up},
        opponent,
        overlay::Overlay,
        store::Store,
    };

    /// A record is taken for opened from the list when it shows this shortly after the list.
    const OPENING: Duration = Duration::from_secs(3);

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

    /// Whether two frames read the list's rows the same: a row for a row, the names drawn
    /// the same (see `history::Plate::same`).
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
    // The list of records: how long it is up, what its rows read in the frame before, and
    // the rows as they were last told with what the pointer was over then (a record opens
    // from one of them).
    let mut up = Up::default();
    let mut rows_before: Vec<Option<Row>> = Vec::new();
    let mut told: Option<(Instant, Vec<Option<Told>>, Pointed)> = None;
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
            let opened = |seen: &Instant| {
                record_since.is_some_and(|since| since.duration_since(*seen) <= OPENING)
            };
            let mut linked = false;
            if let Some((_, rows, pointed)) = told.as_ref().filter(|(seen, ..)| opened(seen)) {
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
                if let Some(row) = listing::opened(rows, *pointed, fight, saved.id) {
                    store.link(row, saved.id).map_err(records)?;
                    linked = true;
                }
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
            let read: Vec<Option<Row>> = rows.iter().map(|shown| shown.row.clone()).collect();
            // The rows are told once two frames in a row read them the same: a list that
            // comes up or goes is drawn faint for a few frames.
            if same_rows(&read, &rows_before) {
                let rows_told = store.see(&read, since, clock()).map_err(records)?;
                let tops: Vec<f32> = rows.iter().map(|shown| shown.top).collect();
                let read_rows: Vec<(f32, bool)> = tops
                    .iter()
                    .zip(&rows_told)
                    .filter_map(|(top, told)| told.map(|told| (*top, told.battle.is_some())))
                    .collect();
                marks = guide::for_list(&read_rows, since.is_some());
                // The pointer's place is in the screen's pixels only on the overlay's thread.
                let at = overlay.as_ref().and_then(|_| pointer(&shot));
                let over = listing::pointed(at, &tops, ROW_TALL, ROW_ACROSS);
                told = Some((now, rows_told, over));
            }
            rows_before = read;
        } else {
            rows_before.clear();
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
