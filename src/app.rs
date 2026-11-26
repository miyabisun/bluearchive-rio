//! What the window shows and does: the snapshot the reading (`watch`) keeps current, and the
//! window's command (pausing). Made as Aoi's (bluearchive-aoi) `app`: the reading's thread
//! changes the snapshot and signals it, and the window takes a copy on every signal.

use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
};

use futures::channel::mpsc::UnboundedSender;

use crate::{
    source::Source,
    store::{Amended, Kept, Registered, Store, Who},
    tier::Tier,
    update,
    watch::{self, Choice, Opponent, Status, Update},
};

/// Everything the window shows.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub status: Status,
    /// The opponent the game showed last, with what the records counsel.
    pub opponent: Option<Opponent>,
    /// The battles kept, the latest first.
    pub battles: Vec<Kept>,
    /// The students the user keeps out of their attacks (編成NG), by id, ascending.
    pub barred: Vec<i64>,
    /// The students the user sorted into tiers (Tier表), each with its tier, by id.
    pub tiers: Vec<(i64, Tier)>,
}

impl Snapshot {
    /// The window as it starts, before anything is read.
    pub fn starting() -> Self {
        Self {
            status: Status::NoGame,
            opponent: None,
            battles: Vec::new(),
            barred: Vec::new(),
            tiers: Vec::new(),
        }
    }

    /// Takes in what the reading reports.
    pub fn apply(&mut self, status: Status, update: Option<Update>) {
        self.status = status;
        match update {
            Some(Update::Battles(battles)) => self.battles = battles,
            Some(Update::Opponent(opponent)) => self.opponent = Some(opponent),
            Some(Update::Barred(barred)) => self.barred = barred,
            Some(Update::Tiers(tiers)) => self.tiers = tiers,
            None => {}
        }
    }
}

/// Where the app keeps the battles and the window's choices.
#[derive(Debug, Clone)]
pub struct Places {
    pub database: PathBuf,
    pub settings: PathBuf,
}

/// The app's identifier, the name of its data folder.
pub const IDENTIFIER: &str = "io.github.miyabisun.bluearchive-rio";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The app's data folder: `RIO_DATA_DIR`, or the user's application data
/// (`%APPDATA%\io.github.miyabisun.bluearchive-rio`, beside Aoi's). `RIO_DATA_DIR` starts the
/// app on other records without touching the user's own.
pub fn places() -> Result<Places, String> {
    let dir = match std::env::var_os("RIO_DATA_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => dirs::data_dir()
            .ok_or("保存先のフォルダが分かりません")?
            .join(IDENTIFIER),
    };
    Ok(Places {
        database: dir.join("rio.db"),
        settings: dir.join("settings.json"),
    })
}

/// The app's version and the newer one the latest release brings, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateCheck {
    pub current: String,
    pub latest: Option<String>,
}

/// Asks GitHub for the latest release (see `update`); blocks on the network.
pub fn check_update() -> Result<UpdateCheck, String> {
    let tag = update::latest()?;
    Ok(UpdateCheck {
        current: format!("v{VERSION}"),
        latest: update::update_of(&tag, VERSION).map(|u| u.version),
    })
}

/// Puts the latest release in place of the running exe; blocks on the network. The caller
/// starts the new exe and ends this one.
pub fn apply_update() -> Result<(), String> {
    let tag = update::latest()?;
    let update = update::update_of(&tag, VERSION).ok_or("最新のバージョンです")?;
    update::install(&update)
}

/// Starts the exe now in place (the new version) and ends this one at once, as Aoi does:
/// asked to quit through its event loop instead, Aoi's old version once kept its window open
/// beside the new one (2026-11-07). The battles are in SQLite and the settings are written
/// whole or not at all, so nothing is left half kept.
pub fn restart() -> ! {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).spawn();
    }
    std::process::exit(0);
}

/// Why a formation the user registered could not be kept or taken away.
fn not_kept(error: rusqlite::Error) -> String {
    format!("登録した編成を保存できません: {error}")
}

/// What the window works with: the snapshot the reading keeps current, and what the user
/// chose in the window for the reading to mark over the game. Every change is signalled,
/// so the window takes the new snapshot.
pub struct Core {
    latest: Mutex<Snapshot>,
    pub places: Places,
    paused: Arc<AtomicBool>,
    chosen: Arc<Mutex<Option<Choice>>>,
    changed: UnboundedSender<()>,
}

impl Core {
    pub fn new(places: Places, changed: UnboundedSender<()>) -> Self {
        Self {
            latest: Mutex::new(Snapshot::starting()),
            places,
            paused: Arc::new(AtomicBool::new(false)),
            chosen: Arc::default(),
            changed,
        }
    }

    /// Takes what the user chose against an opponent (`None`: nothing), for the window to
    /// show and the reading to mark.
    pub fn choose(&self, choice: Option<Choice>) {
        *self.chosen.lock().unwrap_or_else(PoisonError::into_inner) = choice;
        let _ = self.changed.unbounded_send(());
    }

    /// Bars a student from the user's attacks (編成NG) or lets the student back in, kept
    /// with the records (see `sorted`). The student's tier stays as it is.
    pub fn bar(&self, student: i64, barred: bool) -> Result<(), String> {
        self.sorted(student, barred, |store| store.bar(student, barred))
    }

    /// Puts a student into a tier, out of the one the student was in; `None` takes the
    /// student out of the tiers. Whether the student is barred stays as it is.
    pub fn rank(&self, student: i64, tier: Option<Tier>) -> Result<(), String> {
        self.sorted(student, false, |store| store.rank(student, tier))
    }

    /// Keeps a change to where a student stands, and shows the tiers and those barred as
    /// the records have them now. An attack chosen with a student now `barred` is given
    /// up, its defense kept: nobody the user cannot send is marked over the game.
    fn sorted(
        &self,
        student: i64,
        barred: bool,
        change: impl FnOnce(&Store) -> rusqlite::Result<()>,
    ) -> Result<(), String> {
        let records = |error: rusqlite::Error| format!("Tier と編成NG を保存できません: {error}");
        let store = Store::open(&self.places.database).map_err(records)?;
        change(&store).map_err(records)?;
        let (out, tiers) = (
            store.barred().map_err(records)?,
            store.tiers().map_err(records)?,
        );
        if barred
            && let Some(choice) = self.chosen()
            && choice.attack.is_some_and(|team| team.contains(&student))
        {
            let attack = None;
            self.choose(Some(Choice { attack, ..choice }));
        }
        self.update(|snapshot| (snapshot.barred, snapshot.tiers) = (out, tiers));
        Ok(())
    }

    /// The formations registered, the one registered last first, as the records have them
    /// now: an agent may have changed them (`mcp`).
    pub fn registered(&self) -> Result<Vec<Registered>, String> {
        let store = Store::open(&self.places.database).map_err(not_kept)?;
        store.registered().map_err(not_kept)
    }

    /// Changes the registrations in the records, and asks them anew what they counsel
    /// against the opponent shown.
    fn registering<T>(
        &self,
        change: impl FnOnce(&Store) -> rusqlite::Result<T>,
    ) -> Result<T, String> {
        let store = Store::open(&self.places.database).map_err(not_kept)?;
        let done = change(&store).map_err(not_kept)?;
        self.counsel_anew(&store)?;
        Ok(done)
    }

    /// Registers an attack as told to win against a defense, with what it was told with;
    /// whether it is new.
    pub fn register(
        &self,
        attack: [i64; 6],
        defense: [i64; 6],
        source: &Source,
    ) -> Result<bool, String> {
        self.registering(|store| Ok(store.register(attack, defense, source)?.1))
    }

    /// Changes a registration's teams and what it was told with.
    pub fn amend(
        &self,
        id: i64,
        attack: [i64; 6],
        defense: [i64; 6],
        source: &Source,
    ) -> Result<Amended, String> {
        self.registering(|store| store.amend(id, attack, defense, source))
    }

    /// Takes a registration away: what it was, to put back (`restore`).
    pub fn forget(&self, id: i64) -> Result<Option<Registered>, String> {
        self.registering(|store| store.forget(id))
    }

    /// Puts a registration taken away back as it was; `false` where its teams were
    /// registered anew since.
    pub fn restore(&self, registered: &Registered) -> Result<bool, String> {
        self.registering(|store| store.restore(registered))
    }

    /// Takes a registered attack against a defense away again: what it was, if it was.
    pub fn unregister(
        &self,
        attack: [i64; 6],
        defense: [i64; 6],
    ) -> Result<Option<Registered>, String> {
        self.registering(|store| store.unregister(attack, defense))
    }

    /// Asks the records anew what they counsel against the opponent shown.
    fn counsel_anew(&self, store: &Store) -> Result<(), String> {
        let Some(mut opponent) = self.snapshot().opponent else {
            return Ok(());
        };
        let (front, who) = (opponent.front, Who::of(opponent.name.as_ref()));
        opponent.counsel = (store.counsel(&front, who, watch::clock())).map_err(not_kept)?;
        self.update(|snapshot| snapshot.opponent = Some(opponent));
        Ok(())
    }

    /// What the user chose last.
    pub fn chosen(&self) -> Option<Choice> {
        self.chosen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Starts reading the game.
    pub fn watch(self: &Arc<Self>) {
        let core = Arc::downgrade(self);
        watch::start(
            self.places.database.clone(),
            self.paused.clone(),
            self.chosen.clone(),
            move |status, update| {
                if let Some(core) = core.upgrade() {
                    core.update(|snapshot| snapshot.apply(status, update));
                }
            },
        );
    }

    /// Changes the snapshot and signals it.
    pub fn update(&self, change: impl FnOnce(&mut Snapshot)) {
        change(&mut self.latest.lock().unwrap_or_else(PoisonError::into_inner));
        let _ = self.changed.unbounded_send(());
    }

    /// The snapshot now.
    pub fn snapshot(&self) -> Snapshot {
        self.latest
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Pauses reading the game, or resumes it. The status says so at once; resumed, the
    /// reading tells where it stands with its next frame.
    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
        self.update(|snapshot| {
            snapshot.status = if paused {
                Status::Paused
            } else {
                Status::NoGame
            };
        });
    }
}

/// Runs the app: reads the game, and shows what it finds in the window.
pub fn run() {
    update::clean_up();
    let places = places().expect("the app's folder is known");
    crate::ui::run(places);
}

#[cfg(test)]
mod tests {
    use super::Snapshot;
    use crate::{
        opponent::Front,
        store::Counsel,
        watch::{Opponent, Screen, Status, Update},
    };

    #[test]
    fn what_the_reading_reports_is_taken_in() {
        let mut snapshot = Snapshot::starting();
        let opponent = Opponent {
            front: Front {
                first: 1,
                specials: [2, 3],
            },
            name: None,
            counsel: Counsel { likely: Vec::new() },
            held: None,
        };
        let watching = Status::Watching(Some(Screen::Opponent));
        snapshot.apply(watching.clone(), Some(Update::Opponent(opponent.clone())));
        assert_eq!(snapshot.opponent, Some(opponent.clone()));
        // The dialog closes: the opponent stays to look at.
        snapshot.apply(Status::Watching(None), None);
        assert_eq!(snapshot.status, Status::Watching(None));
        assert_eq!(snapshot.opponent, Some(opponent));
        snapshot.apply(Status::Watching(None), Some(Update::Battles(Vec::new())));
        assert!(snapshot.battles.is_empty());
    }
}
