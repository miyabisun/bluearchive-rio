//! Reading the game in a thread of its own: every frame of its window is read for a battle
//! record (kept), an opponent's dialog and the formation dialog (counselled from the
//! records, and marked over the game). What it finds is reported to the window (`app`).

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
        capture::{self, Seen},
        guide::{self, Rank, Visit},
        history::{Battle, Reader, Steady},
        opponent,
        overlay::Overlay,
        store::Store,
    };

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
        if let Some(battle) = steady.see(record, now)
            && store.save(battle).map_err(records)?
        {
            report(
                status.clone(),
                Some(Update::Battles(store.battles().map_err(records)?)),
            );
            // What is counselled may change with the battle just kept.
            if let Some(mut opponent) = counselled.take() {
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
        let (dialog, forming) = (reader.front(frame), reader.formation(frame));
        if let Some(front) = dialog {
            met = Some((front, opponent::name(frame)));
            screen = Some(Screen::Opponent);
        } else if forming.is_some() {
            screen = Some(Screen::Formation);
        }
        let mut marks = Vec::new();
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
