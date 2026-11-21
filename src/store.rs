//! The battles read, kept in SQLite: one row a battle, the teams in the players' terms
//! (`a1` to `a4` the attacker's strikers from the left, `d1` to `d4` the defender's,
//! `asp1`, `asp2`, `dsp1`, `dsp2` the specials, the lower id first).
//!
//! The game tells no battle's time closer than "20 hours ago", and the same two players
//! may fight again within a minute. The damage is what tells a battle from another: twelve
//! numbers that no other battle repeats. It is kept as the battle's fingerprint and as
//! nothing else, for it says little of who won (it leaves out healing and blows taken, and
//! grows on summons and on enemies healed).
//!
//! The rows of the game's list of records are kept too (`list_rows`, see `listing`): a row
//! tells roughly when its battle was, and a record opened from a row is that row's battle.
//! A row is kept with the row seen right above it; nothing else orders the rows.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params_from_iter, types::Value};

use crate::{
    history::{Battle, Party, Plate},
    list::Row,
    listing::{self, Fight, Listed, Match, Sight, Sighting, Sights, Told},
    opponent::Front,
    source::{Reported, Source},
    tier::Tier,
};

/// The students' columns, the attacker's team then the defender's.
const SLOTS: [&str; 12] = [
    "a1", "a2", "a3", "a4", "asp1", "asp2", "d1", "d2", "d3", "d4", "dsp1", "dsp2",
];

/// A battle kept: who attacked whom, who won, and the students of each team in their places.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    pub id: i64,
    /// The local time Rio first saw the record, "2026-11-17 03:10:00": not the battle's.
    pub seen_at: String,
    /// `None` for an anonymous player (see `history::Party`).
    pub attacker: Option<Player>,
    pub defender: Option<Player>,
    pub attacker_won: bool,
    pub attack: [i64; 6],
    pub defense: [i64; 6],
    /// When the battle was, where a row of the game's list told.
    pub fought: Option<Fought>,
}

/// When a battle was, as far as the game's list told it: the middle of the time its row
/// allowed, as local time ("2026-11-18 00:10:00"), and how many seconds that time spans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fought {
    pub about: String,
    pub within: i64,
}

/// A battle kept: its number, and whether it was kept just now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Saved {
    pub id: i64,
    pub new: bool,
}

/// A player, known by the name drawn (see `history::Plate`) and named by the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Player {
    pub id: i64,
    pub name: Option<String>,
    /// The name as the game draws it (a PNG, see `history::Plate::png`): what the window
    /// shows of a player the user gave no name.
    pub plate: Vec<u8>,
}

/// An attack against one defense: the team, the battles kept that it won and all its
/// battles kept, and whether the user registered it by hand as told to win (it may have no
/// battle then), with the record it was introduced with where the registration has one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tried {
    pub attack: [i64; 6],
    pub wins: u32,
    pub battles: u32,
    pub registered: bool,
    pub reported: Option<Reported>,
}

/// The attacks, the one likeliest to win first. The share an attack is taken to win is that
/// of its battles, the user's own counted whole and those it was introduced with counted
/// half (what is told is half believed until it is tried):
/// (wins + reported wins / 2) / (battles + reported battles / 2).
/// Of the same share, the one with more battles counted so first; then in the order given
/// (the caller gives the newest first). An attack with no battle of either kind (registered
/// with its students alone) has no share: it stands after those that have one.
pub fn ranked(mut attacks: Vec<Tried>) -> Vec<Tried> {
    /// The wins and the battles in halves: the user's own count two each, those told one.
    fn halves(tried: &Tried) -> (u64, u64) {
        let told = tried
            .reported
            .map_or((0, 0), |told| (told.wins, told.battles));
        (
            u64::from(tried.wins) * 2 + u64::from(told.0),
            u64::from(tried.battles) * 2 + u64::from(told.1),
        )
    }
    // Stable: those nothing tells apart stay as given.
    attacks.sort_by(|one, other| {
        let ((wins, battles), (other_wins, other_battles)) = (halves(one), halves(other));
        // The larger share first: wins / battles against the other's, without dividing.
        // One with no battle is told apart from no other so, and stands last as the one
        // with the fewest battles.
        (other_wins * battles)
            .cmp(&(wins * other_battles))
            .then(other_battles.cmp(&battles))
    });
    attacks
}

/// A formation the user registered by hand: an attack told to win against a defense, with
/// what it was told with (`Source`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registered {
    pub id: i64,
    /// The local time it was registered, "2026-11-21 13:10:00".
    pub added_at: String,
    pub attack: [i64; 6],
    pub defense: [i64; 6],
    pub source: Source,
}

/// What became of a change to a registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Amended {
    Done,
    /// No registration has the number.
    Gone,
    /// Another registration, of this number, has the same teams already.
    Taken(i64),
}

/// Where a defense stands among those that may be behind a front, the likeliest first:
/// PvP has its trends, so what is used now says more than what was used most.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Ground {
    /// The opponent defended so within `OWN`.
    Theirs,
    /// The user registered the defense by hand, with an attack told to win against it: a
    /// defense going round.
    Registered,
    /// Other players did within `TREND`: the trend.
    Trend,
    /// Only battles before those times tell it.
    Before,
    /// Only the user's own defense tells it.
    Mine,
}

/// A defense that may stand behind an opponent's front: the strikers D2 to D4, what the
/// records say for it, and the attacks that won against it. Players are counted, never their
/// battles: fighting one player again and again says no more of the others.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Likely {
    pub rest: [i64; 3],
    pub ground: Ground,
    /// The day the opponent last defended so ("2026-11-17"), if they ever did.
    pub their_last: Option<String>,
    /// The other players (neither the opponent nor the user) who defended so within
    /// `TREND`. Those the game hides the names of cannot be told apart: they count as one.
    pub others: u32,
    /// The other players who did so before that only.
    pub others_before: u32,
    /// Whether the user defended so.
    pub mine: bool,
    /// Whether the user registered the defense by hand.
    pub registered: bool,
    /// The attacks that won against it at least once, the best first: all of them, for
    /// the window to offer those the user can take (`view::offered`).
    pub attacks: Vec<Tried>,
}

/// How long ago, in seconds, a battle still tells what the opponent uses now (a month), and
/// what players use now (two weeks).
pub const OWN: i64 = 30 * DAY;
pub const TREND: i64 = 14 * DAY;
const DAY: i64 = 24 * 3600;

/// How many candidates are listed: of the defenses, and of the attacks against each.
pub const SHOWN: usize = 5;

/// What the records say against an opponent's front: the defenses that may stand behind
/// it, the likeliest first, `SHOWN` at most. Which of them does, and which attack to take,
/// is the user's to say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Counsel {
    pub likely: Vec<Likely>,
}

/// The columns of a registration beside its students: what it was told with (`Source`).
const TOLD: &str = "reported_battles, reported_wins, url";

/// `?1, ?2` and so on to `?count`.
fn marks(count: usize) -> String {
    let marks: Vec<String> = (1..=count).map(|at| format!("?{at}")).collect();
    marks.join(", ")
}

/// What a registration was told with, as its columns keep it (`TOLD`).
fn told_with(source: &Source) -> [Value; 3] {
    let reported = source.reported;
    [
        reported.map(|told| i64::from(told.battles)).into(),
        reported.map(|told| i64::from(told.wins)).into(),
        source.url.clone().into(),
    ]
}

/// A registration of a row: its number, when it was registered, the twelve students in
/// `SLOTS`' order, then `TOLD`.
fn registration(row: &rusqlite::Row<'_>) -> rusqlite::Result<Registered> {
    let mut teams = [[0; 6]; 2];
    for (slot, student) in teams.as_flattened_mut().iter_mut().enumerate() {
        *student = row.get(2 + slot)?;
    }
    let (battles, wins): (Option<u32>, Option<u32>) = (row.get(14)?, row.get(15)?);
    let told = battles.zip(wins);
    Ok(Registered {
        id: row.get(0)?,
        added_at: row.get(1)?,
        attack: teams[0],
        defense: teams[1],
        source: Source {
            reported: told.and_then(|(battles, wins)| Reported::new(battles, wins)),
            url: row.get(16)?,
        },
    })
}

/// An attack and a defense as the tables keep them: the twelve students in `SLOTS`' order,
/// each team's specials the smaller id first (which stands left tells nothing). Two
/// formations are the same where these are.
pub fn both(mut attack: [i64; 6], mut defense: [i64; 6]) -> [i64; 12] {
    attack[4..].sort_unstable();
    defense[4..].sort_unstable();
    let mut both = [0; 12];
    both[..6].copy_from_slice(&attack);
    both[6..].copy_from_slice(&defense);
    both
}

/// A battle behind a front, for telling the defenses likely there: who defended, with whom
/// behind the first striker, and when (seconds since 1970, and the day in local time).
struct Defended {
    defender: Option<i64>,
    rest: [i64; 3],
    at: i64,
    day: String,
}

/// The defenses of `defended`, the likeliest at the time `now` first (see `Ground`): those
/// the `opponent` used within `OWN`, the one used last first; then the trend, by how many
/// other players used a defense within `TREND`, then the one used last; then those only
/// older battles tell, and those only the user's (`me`) do, the one used last first.
fn likely(
    defended: &[Defended],
    registered: &[[i64; 3]],
    opponent: Option<i64>,
    me: Option<i64>,
    now: i64,
) -> Vec<Likely> {
    /// What is found of one defense: when the opponent, any other player and the user last
    /// used it, and the other players who did within `TREND` and ever (`None`: those the
    /// game hides the names of).
    struct Found {
        rest: [i64; 3],
        theirs: Option<(i64, String)>,
        other: Option<i64>,
        mine: Option<i64>,
        lately: Vec<Option<i64>>,
        ever: Vec<Option<i64>>,
        registered: bool,
    }
    /// Where a defense is found, made if it is new.
    fn of(found: &mut Vec<Found>, rest: [i64; 3]) -> &mut Found {
        let known = found.iter().position(|found| found.rest == rest);
        let known = known.unwrap_or_else(|| {
            found.push(Found {
                rest,
                theirs: None,
                other: None,
                mine: None,
                lately: Vec::new(),
                ever: Vec::new(),
                registered: false,
            });
            found.len() - 1
        });
        &mut found[known]
    }
    let mut found: Vec<Found> = Vec::new();
    for rest in registered {
        of(&mut found, *rest).registered = true;
    }
    for battle in defended {
        let found = of(&mut found, battle.rest);
        let later = |last: Option<i64>| last.is_none_or(|last| last < battle.at);
        match battle.defender {
            Some(defender) if Some(defender) == opponent => {
                if later(found.theirs.as_ref().map(|(at, _)| *at)) {
                    found.theirs = Some((battle.at, battle.day.clone()));
                }
            }
            Some(defender) if Some(defender) == me => {
                found.mine = found.mine.max(Some(battle.at));
            }
            defender => {
                found.other = found.other.max(Some(battle.at));
                if !found.ever.contains(&defender) {
                    found.ever.push(defender);
                }
                if now - battle.at <= TREND && !found.lately.contains(&defender) {
                    found.lately.push(defender);
                }
            }
        }
    }
    let mut likely: Vec<(Likely, (u32, Option<i64>))> = found
        .into_iter()
        .map(|found| {
            let theirs = found.theirs.as_ref().map(|(at, _)| *at);
            let others = found.lately.len() as u32;
            // Where it stands, and what orders it among those standing there.
            let (ground, order) = if theirs.is_some_and(|at| now - at <= OWN) {
                (Ground::Theirs, (0, theirs))
            } else if found.registered {
                (Ground::Registered, (others, found.other))
            } else if others > 0 {
                (Ground::Trend, (others, found.other))
            } else if theirs.is_some() || found.other.is_some() {
                (Ground::Before, (0, theirs.max(found.other)))
            } else {
                (Ground::Mine, (0, found.mine))
            };
            let likely = Likely {
                rest: found.rest,
                ground,
                their_last: found.theirs.map(|(_, day)| day),
                others,
                others_before: found.ever.len() as u32 - others,
                mine: found.mine.is_some(),
                registered: found.registered,
                attacks: Vec::new(),
            };
            (likely, order)
        })
        .collect();
    likely.sort_by_key(|(likely, order)| (likely.ground, std::cmp::Reverse(*order)));
    likely.into_iter().map(|(likely, _)| likely).collect()
}

pub struct Store(Connection);

impl Store {
    /// The database at `path`, made if it is not there.
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        Self::prepared(Connection::open(path)?)
    }

    /// The tables made where they are not there.
    fn prepared(connection: Connection) -> rusqlite::Result<Self> {
        // The reading keeps the battles while the window keeps whom the user barred: one
        // waits for the other rather than fail.
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        let slots: String = SLOTS
            .iter()
            .map(|slot| format!("{slot} INTEGER NOT NULL, "))
            .collect();
        connection.execute_batch(&format!(
            "CREATE TABLE IF NOT EXISTS players (
                id INTEGER PRIMARY KEY,
                plate BLOB NOT NULL,
                name TEXT
            );
            CREATE TABLE IF NOT EXISTS battles (
                id INTEGER PRIMARY KEY,
                seen_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
                attacker INTEGER REFERENCES players (id),
                defender INTEGER REFERENCES players (id),
                attacker_won INTEGER NOT NULL,
                {slots}
                fingerprint TEXT NOT NULL UNIQUE,
                user_attacked INTEGER
            );
            CREATE TABLE IF NOT EXISTS list_rows (
                id INTEGER PRIMARY KEY,
                above INTEGER UNIQUE REFERENCES list_rows (id),
                attacked INTEGER NOT NULL,
                won INTEGER NOT NULL,
                opponent INTEGER REFERENCES players (id),
                earliest INTEGER NOT NULL,
                latest INTEGER NOT NULL,
                battle INTEGER UNIQUE REFERENCES battles (id)
            );
            CREATE TABLE IF NOT EXISTS barred (student INTEGER PRIMARY KEY);
            CREATE TABLE IF NOT EXISTS tiers (student INTEGER PRIMARY KEY, tier INTEGER NOT NULL);
            CREATE TABLE IF NOT EXISTS registered (
                id INTEGER PRIMARY KEY,
                added_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
                {slots}
                reported_battles INTEGER,
                reported_wins INTEGER,
                url TEXT,
                UNIQUE ({every})
            );",
            every = SLOTS.join(", ")
        ))?;
        // A column a later version keeps: an older database gains it, empty for the rows it
        // holds. Which side of a battle the user's own is (kept since 0.1.11), and what a
        // registration was told with (0.1.16 and older kept its students alone).
        let gains = |table: &str, column: &str, kind: &str| -> rusqlite::Result<()> {
            let there: i64 = connection.query_row(
                "SELECT COUNT(*) FROM pragma_table_info(?1) WHERE name = ?2",
                [table, column],
                |row| row.get(0),
            )?;
            if there == 0 {
                let gain = format!("ALTER TABLE {table} ADD COLUMN {column} {kind}");
                connection.execute_batch(&gain)?;
            }
            Ok(())
        };
        gains("battles", "user_attacked", "INTEGER")?;
        gains("registered", "reported_battles", "INTEGER")?;
        gains("registered", "reported_wins", "INTEGER")?;
        gains("registered", "url", "TEXT")?;
        // A student stands in a tier or among those barred, never both: one that 0.1.16 or
        // an older version kept as both is barred alone.
        connection
            .execute_batch("DELETE FROM tiers WHERE student IN (SELECT student FROM barred)")?;
        // The versions 0.1.4 to 0.1.7 kept the rows at places in one line (`listed`), some
        // of them wrong. What a record told of a row is taken over, each row by itself
        // (the upper first); where the rows stand is seen anew. A record that has its row
        // here already keeps it: an old version run in between makes its table anew.
        let old: i64 = connection.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'listed'",
            [],
            |row| row.get(0),
        )?;
        if old > 0 {
            connection.execute_batch(
                "BEGIN;
                 INSERT OR IGNORE INTO list_rows (attacked, won, opponent, earliest, latest, battle)
                 SELECT attacked, won, opponent, earliest, latest, battle FROM listed
                 WHERE battle IS NOT NULL ORDER BY place DESC;
                 DROP TABLE listed;
                 COMMIT;",
            )?;
        }
        // The versions 0.1.8 and 0.1.9 could take the rows in sight for rows one off along
        // a run of rows that tell the same, and kept what followed from it: a row that is
        // not in the list, rows cut off from the rows under them. Which rows of a database
        // they wrote are such cannot be told, so what they kept is dropped once, but for
        // what a record told: the rows that have a record stay, each by itself.
        let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version < 1 {
            connection.execute_batch(
                "BEGIN;
                 UPDATE list_rows SET above = NULL;
                 DELETE FROM list_rows WHERE battle IS NULL;
                 PRAGMA user_version = 1;
                 COMMIT;",
            )?;
        }
        Ok(Self(connection))
    }

    /// Keeps a battle; `false` for one kept before.
    pub fn save(&self, battle: &Battle) -> rusqlite::Result<bool> {
        Ok(self.keep(battle)?.new)
    }

    /// Keeps a battle, or finds it kept: its number either way.
    pub fn keep(&self, battle: &Battle) -> rusqlite::Result<Saved> {
        let fingerprint = fingerprint(battle);
        // Asked before the players are looked up, so a battle kept before makes no player.
        let kept = self.0.query_row(
            "SELECT id FROM battles WHERE fingerprint = ?1",
            [&fingerprint],
            |row| row.get(0),
        );
        if let Some(id) = kept.optional()? {
            return Ok(Saved { id, new: false });
        }
        let player = |party: &Party| self.opponent(party.plate.as_ref());
        let mut values: Vec<Value> = vec![
            player(&battle.attacker)?.into(),
            player(&battle.defender)?.into(),
            battle.attacker_won.into(),
        ];
        let fighters = battle.attacker.team.iter().chain(&battle.defender.team);
        values.extend(fighters.map(|fighter| Value::from(fighter.student)));
        values.push(fingerprint.into());
        values.push(battle.user_attacked.into());
        let marks = vec!["?"; values.len()].join(", ");
        let sql = format!(
            "INSERT INTO battles (attacker, defender, attacker_won, {}, fingerprint,
                                  user_attacked)
             VALUES ({marks})",
            SLOTS.join(", ")
        );
        self.0.execute(&sql, params_from_iter(values))?;
        Ok(Saved {
            id: self.0.last_insert_rowid(),
            new: true,
        })
    }

    /// The player of a name the game draws: one kept whose plate draws the same name, or a
    /// new one; none for no name (an anonymous player).
    pub fn opponent(&self, name: Option<&Plate>) -> rusqlite::Result<Option<i64>> {
        let Some(plate) = name else {
            return Ok(None);
        };
        if let Some(known) = self.known(plate)? {
            return Ok(Some(known));
        }
        self.0
            .execute("INSERT INTO players (plate) VALUES (?1)", [plate.png()])?;
        Ok(Some(self.0.last_insert_rowid()))
    }

    /// The rows of the game's list kept whose time reaches `from` (seconds since 1970) or
    /// later, in the order they were kept: an older row is no row in sight.
    pub fn listed(&self, from: i64) -> rusqlite::Result<Vec<Listed>> {
        let mut rows = self.0.prepare(
            "SELECT id, above, attacked, won, opponent, earliest, latest, battle FROM list_rows
             WHERE latest >= ?1 ORDER BY id",
        )?;
        rows.query_map([from], |row| {
            Ok(Listed {
                id: row.get(0)?,
                above: row.get(1)?,
                fight: Fight {
                    attacked: row.get(2)?,
                    won: row.get(3)?,
                    opponent: row.get(4)?,
                },
                range: (row.get(5)?, row.get(6)?),
                battle: row.get(7)?,
            })
        })?
        .collect()
    }

    /// Keeps a row of the game's list, by itself (see `join`): its number.
    pub fn list(&self, row: &Sighting) -> rusqlite::Result<i64> {
        self.0.execute(
            "INSERT INTO list_rows (attacked, won, opponent, earliest, latest)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                row.fight.attacked,
                row.fight.won,
                row.fight.opponent,
                row.range.0,
                row.range.1
            ],
        )?;
        Ok(self.0.last_insert_rowid())
    }

    /// Narrows the time of a row kept.
    pub fn narrow(&self, row: i64, range: (i64, i64)) -> rusqlite::Result<()> {
        self.0.execute(
            "UPDATE list_rows SET earliest = ?2, latest = ?3 WHERE id = ?1",
            rusqlite::params![row, range.0, range.1],
        )?;
        Ok(())
    }

    /// Keeps that the row `lower` stands right under the row `upper`: the list showed them
    /// so. What was kept against it gives way: another row under `upper`, and `upper`
    /// under a row that hangs under `lower` (never a ring).
    pub fn join(&self, upper: i64, lower: i64) -> rusqlite::Result<()> {
        let above: Option<i64> = self.0.query_row(
            "SELECT above FROM list_rows WHERE id = ?1",
            [lower],
            |row| row.get(0),
        )?;
        if above == Some(upper) {
            return Ok(());
        }
        self.0.execute(
            "UPDATE list_rows SET above = NULL WHERE above = ?1",
            [upper],
        )?;
        self.0.execute(
            "WITH RECURSIVE under (id) AS (
                 SELECT ?2 UNION SELECT r.id FROM list_rows r JOIN under ON r.above = under.id
             )
             UPDATE list_rows SET above = NULL WHERE id = ?1 AND id IN (SELECT id FROM under)",
            [upper, lower],
        )?;
        self.0.execute(
            "UPDATE list_rows SET above = ?1 WHERE id = ?2",
            [upper, lower],
        )?;
        Ok(())
    }

    /// The row kept right above the row `of` (`above`), or right under it.
    fn beside(&self, of: i64, above: bool) -> rusqlite::Result<Option<Told>> {
        let which = if above {
            "id = (SELECT above FROM list_rows WHERE id = ?1)"
        } else {
            "above = ?1"
        };
        let sql =
            format!("SELECT id, attacked, won, opponent, battle FROM list_rows WHERE {which}");
        let row = self.0.query_row(&sql, [of], |row| {
            Ok(Told {
                id: row.get(0)?,
                sure: true,
                fight: Fight {
                    attacked: row.get(1)?,
                    won: row.get(2)?,
                    opponent: row.get(3)?,
                },
                battle: row.get(4)?,
            })
        });
        row.optional()
    }

    /// Takes a battle kept for the record of a row: the row's time is the battle's from
    /// here on, and no other row's. A row that had it and tells the same fight takes the
    /// record this row had instead: of rows that tell the same, Rio may have taken one for
    /// another, and neither is left without its record.
    pub fn link(&self, row: i64, battle: i64) -> rusqlite::Result<()> {
        let had: Option<i64> =
            self.0
                .query_row("SELECT battle FROM list_rows WHERE id = ?1", [row], |row| {
                    row.get(0)
                })?;
        let other = self.0.query_row(
            "SELECT o.id FROM list_rows o JOIN list_rows r ON r.id = ?1
             WHERE o.battle = ?2 AND o.id <> ?1 AND o.attacked = r.attacked AND o.won = r.won
                   AND o.opponent IS r.opponent",
            [row, battle],
            |row| row.get::<_, i64>(0),
        );
        let other = other.optional()?;
        self.0.execute(
            "UPDATE list_rows SET battle = NULL WHERE battle = ?2 AND id <> ?1",
            [row, battle],
        )?;
        self.0.execute(
            "UPDATE list_rows SET battle = ?2 WHERE id = ?1",
            [row, battle],
        )?;
        if let (Some(other), Some(had)) = (other, had.filter(|had| *had != battle)) {
            self.0.execute(
                "UPDATE list_rows SET battle = ?2 WHERE id = ?1",
                [other, had],
            )?;
        }
        Ok(())
    }

    /// The player kept whose plate draws the same name.
    pub fn known(&self, plate: &Plate) -> rusqlite::Result<Option<i64>> {
        let mut players = self
            .0
            .prepare("SELECT id, plate FROM players ORDER BY id")?;
        let mut rows = players.query([])?;
        while let Some(row) = rows.next()? {
            let kept: Vec<u8> = row.get(1)?;
            if Plate::from_png(&kept).is_some_and(|kept| kept.same(plate)) {
                return row.get(0).map(Some);
            }
        }
        Ok(None)
    }

    /// The user's own player: the one on the user's side of the battle kept last that
    /// tells its side. Of battles kept before the side was (0.1.10 and older), the player
    /// in the most of them: every record shows the user.
    pub fn me(&self) -> rusqlite::Result<Option<i64>> {
        let told = self.0.query_row(
            "SELECT CASE WHEN user_attacked THEN attacker ELSE defender END FROM battles
             WHERE user_attacked IS NOT NULL ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get::<_, Option<i64>>(0),
        );
        if let Some(me) = told.optional()?.flatten() {
            return Ok(Some(me));
        }
        let most = self.0.query_row(
            "SELECT player FROM (
                 SELECT attacker AS player FROM battles UNION ALL SELECT defender FROM battles
             ) WHERE player IS NOT NULL GROUP BY player ORDER BY COUNT(*) DESC, player LIMIT 1",
            [],
            |row| row.get(0),
        );
        most.optional()
    }

    /// The attacks against a defense (D1 to D4, then the specials): those of the battles
    /// kept and those the user registered, the one likeliest to win first (see `ranked`);
    /// of those `ranked` does not tell apart, the one fought or registered last first.
    pub fn attacks(&self, mut defense: [i64; 6]) -> rusqlite::Result<Vec<Tried>> {
        defense[4..].sort_unstable();
        let against = "d1 = ?1 AND d2 = ?2 AND d3 = ?3 AND d4 = ?4 AND dsp1 = ?5 AND dsp2 = ?6";
        let team = |row: &rusqlite::Row<'_>| -> rusqlite::Result<[i64; 6]> {
            let mut attack = [0; 6];
            for (slot, student) in attack.iter_mut().enumerate() {
                *student = row.get(slot)?;
            }
            Ok(attack)
        };
        // Each attack with when it was last heard of (and the number of that battle or
        // registration, for two of one second).
        let mut fought = self.0.prepare(&format!(
            "SELECT a1, a2, a3, a4, asp1, asp2, SUM(attacker_won), COUNT(*), MAX(seen_at), MAX(id)
             FROM battles WHERE {against} GROUP BY a1, a2, a3, a4, asp1, asp2"
        ))?;
        let mut heard: Vec<(Tried, (String, i64))> = fought
            .query_map(defense, |row| {
                let tried = Tried {
                    attack: team(row)?,
                    wins: row.get(6)?,
                    battles: row.get(7)?,
                    registered: false,
                    reported: None,
                };
                Ok((tried, (row.get(8)?, row.get(9)?)))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let mut told = self.0.prepare(&format!(
            "SELECT a1, a2, a3, a4, asp1, asp2, reported_battles, reported_wins, added_at, id
             FROM registered WHERE {against}"
        ))?;
        let told = told.query_map(defense, |row| {
            let (battles, wins): (Option<u32>, Option<u32>) = (row.get(6)?, row.get(7)?);
            let reported =
                (battles.zip(wins)).and_then(|(battles, wins)| Reported::new(battles, wins));
            Ok((team(row)?, reported, (row.get(8)?, row.get(9)?)))
        })?;
        for told in told {
            let (attack, reported, at): ([i64; 6], Option<Reported>, (String, i64)) = told?;
            let known = heard.iter_mut().find(|(tried, _)| tried.attack == attack);
            match known {
                Some((tried, last)) => {
                    (tried.registered, tried.reported) = (true, reported);
                    if at > *last {
                        *last = at;
                    }
                }
                None => {
                    let tried = Tried {
                        attack,
                        wins: 0,
                        battles: 0,
                        registered: true,
                        reported,
                    };
                    heard.push((tried, at));
                }
            }
        }
        heard.sort_by(|(_, one), (_, other)| other.cmp(one));
        Ok(ranked(heard.into_iter().map(|(tried, _)| tried).collect()))
    }

    /// What the records say against a front at the time `now` (seconds since 1970): the
    /// defenses that may stand behind it (see `likely`), each with the attacks that won
    /// against it. `opponent` is the opponent's name where the game shows one.
    pub fn counsel(
        &self,
        front: &Front,
        opponent: Option<&Plate>,
        now: i64,
    ) -> rusqlite::Result<Counsel> {
        let opponent = match opponent {
            Some(plate) => self.known(plate)?,
            None => None,
        };
        let mut specials = front.specials;
        specials.sort_unstable();
        // Only defenses are looked at: an attack's field differs. A battle was when its row
        // of the game's list tells (the middle of the time the row allows); one opened from
        // no row kept, when Rio saw it (a record is of the last days).
        let mut defended = self.0.prepare(
            "SELECT defender, d2, d3, d4, at, date(at, 'unixepoch', 'localtime') FROM (
                 SELECT b.id, b.defender, b.d2, b.d3, b.d4,
                        COALESCE((l.earliest + l.latest) / 2,
                                 CAST(strftime('%s', b.seen_at, 'utc') AS INTEGER)) AS at
                 FROM battles b LEFT JOIN list_rows l ON l.battle = b.id
                 WHERE b.d1 = ?1 AND b.dsp1 = ?2 AND b.dsp2 = ?3
             ) ORDER BY id",
        )?;
        let defended: Vec<Defended> = defended
            .query_map([front.first, specials[0], specials[1]], |row| {
                Ok(Defended {
                    defender: row.get(0)?,
                    rest: [row.get(1)?, row.get(2)?, row.get(3)?],
                    at: row.get(4)?,
                    day: row.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        // The defenses the user registered behind the same front.
        let mut registered = self.0.prepare(
            "SELECT DISTINCT d2, d3, d4 FROM registered
             WHERE d1 = ?1 AND dsp1 = ?2 AND dsp2 = ?3 ORDER BY id",
        )?;
        let registered: Vec<[i64; 3]> = registered
            .query_map([front.first, specials[0], specials[1]], |row| {
                Ok([row.get(0)?, row.get(1)?, row.get(2)?])
            })?
            .collect::<rusqlite::Result<_>>()?;
        let mut likely = likely(&defended, &registered, opponent, self.me()?, now);
        likely.truncate(SHOWN);
        for defense in &mut likely {
            let [second, third, fourth] = defense.rest;
            let [left, right] = specials;
            let mut tried = self.attacks([front.first, second, third, fourth, left, right])?;
            tried.retain(|tried| tried.wins > 0 || tried.registered);
            defense.attacks = tried;
        }
        Ok(Counsel { likely })
    }

    /// The students the user sorted into tiers, each with its tier, by id.
    pub fn tiers(&self) -> rusqlite::Result<Vec<(i64, Tier)>> {
        let mut tiers = self
            .0
            .prepare("SELECT student, tier FROM tiers ORDER BY student")?;
        let kept = tiers
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<Vec<(i64, i64)>>>()?;
        let known = |(student, rank)| Tier::of_rank(rank).map(|tier| (student, tier));
        Ok(kept.into_iter().filter_map(known).collect())
    }

    /// Puts `student` into a tier, out of the one the student was in and from among those
    /// barred (a student stands in a tier or among them, never both); `None` takes the
    /// student out of the tiers.
    pub fn rank(&self, student: i64, tier: Option<Tier>) -> rusqlite::Result<()> {
        let Some(tier) = tier else {
            let out = "DELETE FROM tiers WHERE student = ?1";
            return self.0.execute(out, [student]).map(drop);
        };
        let change = self.0.unchecked_transaction()?;
        change.execute("DELETE FROM barred WHERE student = ?1", [student])?;
        change.execute(
            "INSERT INTO tiers (student, tier) VALUES (?1, ?2)
             ON CONFLICT (student) DO UPDATE SET tier = excluded.tier",
            [student, tier.rank()],
        )?;
        change.commit()
    }

    /// Registers an attack told to win against a defense (each A1 to A4 or D1 to D4, then
    /// the specials, in either order) with what it was told with: the registration's
    /// number, and whether it is new. Teams registered before stay as they were registered.
    pub fn register(
        &self,
        attack: [i64; 6],
        defense: [i64; 6],
        source: &Source,
    ) -> rusqlite::Result<(i64, bool)> {
        let teams = both(attack, defense);
        let mut values: Vec<Value> = teams.map(Value::from).into();
        values.extend(told_with(source));
        let new = self.0.execute(
            &format!(
                "INSERT OR IGNORE INTO registered ({}, {TOLD}) VALUES ({})",
                SLOTS.join(", "),
                marks(values.len())
            ),
            params_from_iter(values),
        )?;
        if new > 0 {
            return Ok((self.0.last_insert_rowid(), true));
        }
        let known = self.registered_as(teams)?;
        Ok((known.ok_or(rusqlite::Error::QueryReturnedNoRows)?, false))
    }

    /// The number of the registration of these teams (as `both` gives them).
    fn registered_as(&self, teams: [i64; 12]) -> rusqlite::Result<Option<i64>> {
        let is: Vec<String> = (SLOTS.iter().enumerate())
            .map(|(at, slot)| format!("{slot} = ?{}", at + 1))
            .collect();
        let known = format!("SELECT id FROM registered WHERE {}", is.join(" AND "));
        self.0
            .query_row(&known, params_from_iter(teams), |row| row.get(0))
            .optional()
    }

    /// The formations registered, the one registered last first.
    pub fn registered(&self) -> rusqlite::Result<Vec<Registered>> {
        let mut registered = self.0.prepare(&format!(
            "SELECT id, added_at, {}, {TOLD} FROM registered ORDER BY id DESC",
            SLOTS.join(", ")
        ))?;
        registered.query_map([], registration)?.collect()
    }

    /// The registration of a number.
    pub fn registration(&self, id: i64) -> rusqlite::Result<Option<Registered>> {
        let one = format!(
            "SELECT id, added_at, {}, {TOLD} FROM registered WHERE id = ?1",
            SLOTS.join(", ")
        );
        self.0.query_row(&one, [id], registration).optional()
    }

    /// Changes a registration's teams and what it was told with; its number and when it was
    /// registered stay.
    pub fn amend(
        &self,
        id: i64,
        attack: [i64; 6],
        defense: [i64; 6],
        source: &Source,
    ) -> rusqlite::Result<Amended> {
        let teams = both(attack, defense);
        if self.registration(id)?.is_none() {
            return Ok(Amended::Gone);
        }
        if let Some(other) = self.registered_as(teams)?
            && other != id
        {
            return Ok(Amended::Taken(other));
        }
        let set: Vec<String> = (SLOTS.iter().enumerate())
            .map(|(at, slot)| format!("{slot} = ?{}", at + 1))
            .collect();
        let mut values: Vec<Value> = teams.map(Value::from).into();
        values.extend(told_with(source));
        values.push(id.into());
        self.0.execute(
            &format!(
                "UPDATE registered SET {}, reported_battles = ?13, reported_wins = ?14, url = ?15
                 WHERE id = ?16",
                set.join(", ")
            ),
            params_from_iter(values),
        )?;
        Ok(Amended::Done)
    }

    /// Takes the registration of a number away: what it was, to put back (`restore`).
    pub fn forget(&self, id: i64) -> rusqlite::Result<Option<Registered>> {
        let Some(was) = self.registration(id)? else {
            return Ok(None);
        };
        self.0
            .execute("DELETE FROM registered WHERE id = ?1", [id])?;
        Ok(Some(was))
    }

    /// Puts a registration taken away back as it was, under its number; `false` where its
    /// teams were registered anew since.
    pub fn restore(&self, registered: &Registered) -> rusqlite::Result<bool> {
        let mut values: Vec<Value> = vec![registered.id.into(), registered.added_at.clone().into()];
        values.extend(both(registered.attack, registered.defense).map(Value::from));
        values.extend(told_with(&registered.source));
        let back = self.0.execute(
            &format!(
                "INSERT OR IGNORE INTO registered (id, added_at, {}, {TOLD}) VALUES ({})",
                SLOTS.join(", "),
                marks(values.len())
            ),
            params_from_iter(values),
        )?;
        Ok(back > 0)
    }

    /// Takes a registered attack against a defense away again: what it was, if it was.
    pub fn unregister(
        &self,
        attack: [i64; 6],
        defense: [i64; 6],
    ) -> rusqlite::Result<Option<Registered>> {
        match self.registered_as(both(attack, defense))? {
            Some(id) => self.forget(id),
            None => Ok(None),
        }
    }

    /// The students the user keeps out of their attacks (編成NG), by id, ascending.
    pub fn barred(&self) -> rusqlite::Result<Vec<i64>> {
        let mut barred = self
            .0
            .prepare("SELECT student FROM barred ORDER BY student")?;
        barred.query_map([], |row| row.get(0))?.collect()
    }

    /// Bars `student` from the user's attacks, out of the tier the student was in (see
    /// `rank`), or lets the student back in.
    pub fn bar(&self, student: i64, barred: bool) -> rusqlite::Result<()> {
        if !barred {
            let back = "DELETE FROM barred WHERE student = ?1";
            return self.0.execute(back, [student]).map(drop);
        }
        let change = self.0.unchecked_transaction()?;
        change.execute("DELETE FROM tiers WHERE student = ?1", [student])?;
        change.execute(
            "INSERT OR IGNORE INTO barred (student) VALUES (?1)",
            [student],
        )?;
        change.commit()
    }

    /// The battles kept, the latest first.
    pub fn battles(&self) -> rusqlite::Result<Vec<Kept>> {
        let slots: String = SLOTS.iter().map(|slot| format!(", b.{slot}")).collect();
        let mut battles = self.0.prepare(&format!(
            "SELECT b.id, b.seen_at, b.attacker, a.name, a.plate, b.defender, d.name, d.plate,
                    b.attacker_won{slots},
                    datetime((l.earliest + l.latest) / 2, 'unixepoch', 'localtime'),
                    l.latest - l.earliest
             FROM battles b
             LEFT JOIN players a ON a.id = b.attacker
             LEFT JOIN players d ON d.id = b.defender
             LEFT JOIN list_rows l ON l.battle = b.id
             ORDER BY b.id DESC"
        ))?;
        let player = |row: &rusqlite::Row, at: usize| -> rusqlite::Result<Option<Player>> {
            let id: Option<i64> = row.get(at)?;
            id.map(|id| {
                Ok(Player {
                    id,
                    name: row.get(at + 1)?,
                    plate: row.get(at + 2)?,
                })
            })
            .transpose()
        };
        let team = |row: &rusqlite::Row, from: usize| -> rusqlite::Result<[i64; 6]> {
            let mut team = [0; 6];
            for (slot, student) in team.iter_mut().enumerate() {
                *student = row.get(from + slot)?;
            }
            Ok(team)
        };
        battles
            .query_map([], |row| {
                Ok(Kept {
                    id: row.get(0)?,
                    seen_at: row.get(1)?,
                    attacker: player(row, 2)?,
                    defender: player(row, 5)?,
                    attacker_won: row.get(8)?,
                    attack: team(row, 9)?,
                    defense: team(row, 15)?,
                    fought: match (row.get(21)?, row.get(22)?) {
                        (Some(about), Some(within)) => Some(Fought { about, within }),
                        _ => None,
                    },
                })
            })?
            .collect()
    }

    /// Tells the rows a frame read of the game's list, a row for a row (`carried`: the
    /// numbers of those told a frame ago, see `listing::align`; `twice`: whether the frame
    /// before read the row the same): `since` is when the list came up (seconds since 1970,
    /// `None` if Rio did not see it come), `clock` the time now. A row not kept yet is kept
    /// once it read twice, in a list whose rows' times count: in one that was up before Rio
    /// looked, only the latest time a row allows is known. A name not met makes a player
    /// only where its row is kept, and a row never kept stays untold.
    pub fn see(
        &self,
        rows: &[Option<Row>],
        carried: &[Option<i64>],
        twice: &[bool],
        since: Option<i64>,
        clock: i64,
    ) -> rusqlite::Result<Sights> {
        let new: Vec<bool> = twice
            .iter()
            .map(|twice| *twice && since.is_some())
            .collect();
        let mut seen = Vec::new();
        for (row, new) in rows.iter().zip(&new) {
            // The opponent's number: none for a row that tells no row kept.
            let opponent = match (row, new) {
                (None, _) => None,
                (Some(Row { opponent: None, .. }), _) => Some(None),
                (Some(Row { opponent: name, .. }), true) => Some(self.opponent(name.as_ref())?),
                (
                    Some(Row {
                        opponent: Some(name),
                        ..
                    }),
                    false,
                ) => self.known(name)?.map(Some),
            };
            seen.push(row.as_ref().zip(opponent).map(|(row, opponent)| Sighting {
                fight: Fight {
                    attacked: row.attacked,
                    won: row.won,
                    opponent,
                },
                range: row.ago.range(since.unwrap_or(0), clock),
            }));
        }
        self.tell(&seen, carried, &new)
    }

    /// Tells the rows in sight of the game's list, from the top (`None` for a row that
    /// did not read): for each the row kept that it is, its time narrowed by what it tells
    /// now. A row not kept yet is kept if `new` says so for it; where any row may be kept
    /// (the list came up under Rio's eyes, so its rows' times count), the rows surely told
    /// are kept side by side as they stand.
    pub fn tell(
        &self,
        seen: &[Option<Sighting>],
        carried: &[Option<i64>],
        new: &[bool],
    ) -> rusqlite::Result<Sights> {
        let timed = new.contains(&true);
        // A row kept that ends two days before the oldest time in sight is no row in sight,
        // nor beside one.
        let oldest = seen.iter().flatten().map(|row| row.range.0).min();
        let kept = self.listed(oldest.unwrap_or(0) - 2 * 86400)?;
        let matches = listing::align(&kept, seen, carried);
        let told = |known: &Listed, sure| Sight {
            row: Some(Told {
                id: known.id,
                sure,
                fight: known.fight,
                battle: known.battle,
            }),
            read: Some(known.battle.is_some()),
        };
        let mut rows = Vec::new();
        for ((row, found), new) in seen.iter().zip(matches).zip(new) {
            rows.push(match (row, found) {
                (Some(row), Match::Kept(known)) => {
                    let known = &kept[known];
                    let range = listing::narrowed(known.range, row.range);
                    if range != known.range {
                        self.narrow(known.id, range)?;
                    }
                    told(known, true)
                }
                (_, Match::Likely(known)) => told(&kept[known], false),
                (Some(row), Match::New) if *new => Sight {
                    row: Some(Told {
                        id: self.list(row)?,
                        sure: true,
                        fight: row.fight,
                        battle: None,
                    }),
                    read: Some(false),
                },
                (_, Match::Unsure(read)) => Sight { row: None, read },
                _ => Sight::default(),
            });
        }
        let sure = |sight: &Sight| sight.row.filter(|row| row.sure && timed);
        for pair in rows.windows(2) {
            if let (Some(upper), Some(lower)) = (sure(&pair[0]), sure(&pair[1])) {
                self.join(upper.id, lower.id)?;
            }
        }
        let beside = |sight: Option<&Sight>, above| match sight.and_then(sure) {
            Some(row) => self.beside(row.id, above),
            None => Ok(None),
        };
        Ok(Sights {
            above: beside(rows.first(), true)?,
            below: beside(rows.last(), false)?,
            rows,
        })
    }

    /// Names a player; `false` when there is no such player.
    pub fn name(&self, player: i64, name: &str) -> rusqlite::Result<bool> {
        let named = self.0.execute(
            "UPDATE players SET name = ?2 WHERE id = ?1",
            rusqlite::params![player, name],
        )?;
        Ok(named == 1)
    }
}

/// What tells a battle from another: the damage of the twelve students in the order of
/// their columns, "60812-156315-...".
fn fingerprint(battle: &Battle) -> String {
    let fighters = battle.attacker.team.iter().chain(&battle.defender.team);
    let damage: Vec<String> = fighters.map(|fighter| fighter.damage.to_string()).collect();
    damage.join("-")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use image::{GrayImage, Luma};

    use super::{Amended, Ground, Likely, Player, Registered, Store, Tried, ranked};
    use crate::{
        history::{Battle, Fighter, Party, Plate},
        list::{Ago, Row, Unit},
        listing::{Fight, Sight, Sighting, Sights},
        opponent::Front,
        source::{Reported, Source},
        tier::Tier,
    };

    /// A name of `strokes` upright strokes: two names differ in their strokes.
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

    fn party(strokes: u32, first: i64, damage: u32) -> Party {
        let fighter = |n: i64| Fighter {
            student: first + n,
            damage: damage + n as u32,
        };
        Party {
            plate: Some(plate(strokes)),
            team: [0, 1, 2, 3, 4, 5].map(fighter),
        }
    }

    fn named(player: &Option<Player>) -> &Player {
        player.as_ref().expect("a player with a name drawn")
    }

    fn battle(damage: u32) -> Battle {
        Battle {
            attacker: party(2, 10000, damage),
            defender: party(5, 20000, damage),
            attacker_won: true,
            user_attacked: true,
        }
    }

    fn store() -> Store {
        Store::open(Path::new(":memory:")).unwrap()
    }

    #[test]
    fn a_student_is_in_one_tier_at_a_time() {
        let store = store();
        assert!(store.tiers().unwrap().is_empty(), "nobody at first");
        store.rank(20008, Some(Tier::B)).unwrap();
        store.rank(10062, Some(Tier::S)).unwrap();
        assert_eq!(
            store.tiers().unwrap(),
            [(10062, Tier::S), (20008, Tier::B)],
            "by id"
        );
        // Moved to another tier, she is in that one alone.
        store.rank(20008, Some(Tier::D)).unwrap();
        assert_eq!(store.tiers().unwrap(), [(10062, Tier::S), (20008, Tier::D)]);
        // Taken out; one never in a tier changes nothing.
        store.rank(10062, None).unwrap();
        store.rank(13010, None).unwrap();
        assert_eq!(store.tiers().unwrap(), [(20008, Tier::D)]);
        // A number no tier has (a later version's) is passed over.
        let odd = "INSERT INTO tiers (student, tier) VALUES (1, 9)";
        store.0.execute(odd, []).unwrap();
        assert_eq!(store.tiers().unwrap(), [(20008, Tier::D)]);
    }

    #[test]
    fn a_student_is_barred_once_and_let_back_in() {
        let store = store();
        assert!(store.barred().unwrap().is_empty(), "nobody at first");
        for student in [20008, 10062, 20008] {
            store.bar(student, true).unwrap();
        }
        assert_eq!(
            store.barred().unwrap(),
            [10062, 20008],
            "each once, in order"
        );
        store.bar(20008, false).unwrap();
        store.bar(13010, false).unwrap();
        assert_eq!(
            store.barred().unwrap(),
            [10062],
            "one never barred changes nothing"
        );
    }

    #[test]
    fn a_student_stands_in_a_tier_or_among_those_barred_never_both() {
        let store = store();
        store.rank(10062, Some(Tier::S)).unwrap();
        // Barred, she leaves her tier.
        store.bar(10062, true).unwrap();
        assert_eq!(store.barred().unwrap(), [10062]);
        assert!(store.tiers().unwrap().is_empty(), "out of the tier");
        // Put into a tier, she is let back in.
        store.rank(10062, Some(Tier::B)).unwrap();
        assert_eq!(store.tiers().unwrap(), [(10062, Tier::B)]);
        assert!(store.barred().unwrap().is_empty(), "no longer barred");
        // Taken out of the tiers, one barred stays barred; let back in, one in a tier stays.
        store.bar(20008, true).unwrap();
        store.rank(20008, None).unwrap();
        assert_eq!(store.barred().unwrap(), [20008]);
        store.bar(10062, false).unwrap();
        assert_eq!(store.tiers().unwrap(), [(10062, Tier::B)]);
        // A database of a version that let a student be both: opened, she is barred alone.
        let both = "INSERT INTO tiers (student, tier) VALUES (20008, 1)";
        store.0.execute(both, []).unwrap();
        let store = Store::prepared(store.0).unwrap();
        assert_eq!(store.barred().unwrap(), [20008]);
        assert_eq!(store.tiers().unwrap(), [(10062, Tier::B)]);
    }

    #[test]
    fn a_battle_seen_again_is_kept_once() {
        let store = store();
        assert!(store.save(&battle(100)).unwrap(), "new");
        assert!(!store.save(&battle(100)).unwrap(), "seen before");
        assert!(
            store.save(&battle(200)).unwrap(),
            "another battle of the same teams"
        );
        assert_eq!(store.battles().unwrap().len(), 2);
    }

    #[test]
    fn a_battle_is_told_by_its_damage_alone() {
        let store = store();
        store.save(&battle(100)).unwrap();
        // The attacker took another name since: the record shows the same damage.
        let renamed = Battle {
            attacker: party(3, 10000, 100),
            ..battle(100)
        };
        assert!(!store.save(&renamed).unwrap(), "the same battle");
        assert_eq!(store.battles().unwrap().len(), 1);
        assert!(
            !store.name(3, "だれか").unwrap(),
            "no player is made for a battle kept before"
        );
    }

    #[test]
    fn an_anonymous_player_is_nobody() {
        let store = store();
        let hidden = |damage| Battle {
            defender: Party {
                plate: None,
                ..party(5, 20000, damage)
            },
            ..battle(damage)
        };
        store.save(&hidden(100)).unwrap();
        store.save(&hidden(200)).unwrap();
        let kept = store.battles().unwrap();
        assert_eq!(kept.len(), 2);
        assert!(kept.iter().all(|battle| battle.defender.is_none()));
        assert!(kept.iter().all(|battle| battle.attacker.is_some()));
        assert!(
            !store.name(2, "だれか").unwrap(),
            "no player is made for the anonymous"
        );
    }

    #[test]
    fn a_player_is_known_by_the_name_drawn() {
        let store = store();
        store.save(&battle(100)).unwrap();
        let defender = named(&store.battles().unwrap()[0].defender).id;
        assert_eq!(store.known(&plate(5)).unwrap(), Some(defender));
        assert_eq!(store.known(&plate(7)).unwrap(), None, "a name not met");
        assert!(!store.name(3, "だれか").unwrap(), "asking makes no player");
    }

    #[test]
    fn the_attacks_against_a_defense_are_ranked_by_their_wins() {
        let store = store();
        // One attack wins once and loses once; another wins its only battle.
        store.save(&battle(100)).unwrap();
        let lost = Battle {
            attacker_won: false,
            ..battle(200)
        };
        store.save(&lost).unwrap();
        let mut sure = battle(300);
        sure.attacker.team[0].student = 19000;
        store.save(&sure).unwrap();
        // Against another defense.
        let mut apart = battle(400);
        apart.defender.team[1].student = 29001;
        store.save(&apart).unwrap();

        let tried = store
            .attacks([20000, 20001, 20002, 20003, 20005, 20004])
            .unwrap();
        let ranked: Vec<_> = tried
            .iter()
            .map(|tried| (tried.attack[0], tried.wins, tried.battles))
            .collect();
        assert_eq!(ranked, [(19000, 1, 1), (10000, 1, 2)]);
        assert_eq!(tried[1].attack, [10000, 10001, 10002, 10003, 10004, 10005]);
    }

    /// The front of the defenses these tests keep.
    const FRONT: Front = Front {
        first: 20000,
        specials: [20004, 20005],
    };

    /// A battle the user (the name of two strokes) won attacking the player of a name of
    /// `defender` strokes, whose second striker is `second`.
    fn against(damage: u32, defender: u32, second: i64) -> Battle {
        let mut battle = battle(damage);
        battle.defender.plate = Some(plate(defender));
        battle.defender.team[1].student = second;
        battle
    }

    #[test]
    fn the_user_is_the_player_on_the_records_own_side() {
        let store = store();
        assert_eq!(store.me().unwrap(), None, "no record yet");
        store.save(&against(100, 5, 20001)).unwrap();
        let me = store.known(&plate(2)).unwrap();
        assert_eq!(
            store.me().unwrap(),
            me,
            "the attacker of a record the user attacked in"
        );
        // Attacked by the same player: the user is the defender there.
        let mut attacked = against(200, 2, 20001);
        attacked.attacker.plate = Some(plate(5));
        attacked.user_attacked = false;
        store.save(&attacked).unwrap();
        assert_eq!(store.me().unwrap(), me);
        // Records kept before the side was: the player in the most of them.
        store.save(&against(300, 3, 20001)).unwrap();
        store
            .0
            .execute_batch("UPDATE battles SET user_attacked = NULL")
            .unwrap();
        assert_eq!(store.me().unwrap(), me);
    }

    /// The time the battles of these tests are told from, and a day in seconds.
    const NOW: i64 = 1_800_000_000;
    const DAY: i64 = 24 * 3600;

    /// Keeps `battle` as one Rio saw `days` days before `NOW`.
    fn days_ago(store: &Store, days: i64, battle: &Battle) -> i64 {
        let id = store.keep(battle).unwrap().id;
        let seen =
            "UPDATE battles SET seen_at = datetime(?2, 'unixepoch', 'localtime') WHERE id = ?1";
        store.0.execute(seen, (id, NOW - days * DAY)).unwrap();
        id
    }

    /// What a counsel tells of each defense: its second striker, where it stands, and the
    /// other players of the trend and of before.
    fn told(store: &Store, opponent: Option<&Plate>) -> Vec<(i64, Ground, u32, u32)> {
        let counsel = store.counsel(&FRONT, opponent, NOW).unwrap();
        let told = |likely: &Likely| {
            let others = (likely.others, likely.others_before);
            (likely.rest[0], likely.ground, others.0, others.1)
        };
        counsel.likely.iter().map(told).collect()
    }

    #[test]
    fn the_defenses_likely_are_the_opponents_of_the_month_then_the_trend_of_two_weeks() {
        let store = store();
        // The user's own defense behind the same front, attacked five times yesterday: more
        // records than of any other defense.
        for damage in [10, 20, 30, 40, 50] {
            let mut own = against(damage, 2, 28001);
            own.attacker.plate = Some(plate(6));
            own.user_attacked = false;
            days_ago(&store, 1, &own);
        }
        // The opponent's own of three days ago and of forty; a defense two other players
        // used within two weeks; one three others used, twenty days ago.
        days_ago(&store, 3, &against(100, 5, 29001));
        days_ago(&store, 40, &against(150, 5, 29002));
        days_ago(&store, 2, &against(200, 3, 20001));
        days_ago(&store, 5, &against(300, 4, 20001));
        for (damage, player) in [(400, 3), (500, 4), (600, 7)] {
            days_ago(&store, 20, &against(damage, player, 29005));
        }
        assert_eq!(
            told(&store, Some(&plate(5))),
            [
                (29001, Ground::Theirs, 0, 0),
                (20001, Ground::Trend, 2, 0),
                (29005, Ground::Before, 0, 3),
                (29002, Ground::Before, 0, 0),
                (28001, Ground::Mine, 0, 0),
            ],
            "what is used now comes before what more players used once; the user's own last"
        );
        let counsel = store.counsel(&FRONT, Some(&plate(5)), NOW).unwrap();
        let day = |ago: i64| -> Option<String> {
            let day = "SELECT date(?1, 'unixepoch', 'localtime')";
            store
                .0
                .query_row(day, [NOW - ago * DAY], |row| row.get(0))
                .unwrap()
        };
        assert_eq!(counsel.likely[0].their_last, day(3));
        assert_eq!(counsel.likely[3].their_last, day(40));
        assert_eq!(counsel.likely[1].their_last, None);
        assert_eq!(counsel.likely[0].rest, [29001, 20002, 20003]);
        let mine: Vec<bool> = counsel.likely.iter().map(|likely| likely.mine).collect();
        assert_eq!(mine, [false, false, false, false, true]);
        // To a player not met, and to one without a name, the opponent is one of the other
        // players: their defense of three days ago is of the trend, by one player.
        for opponent in [Some(plate(1)), None] {
            let told = told(&store, opponent.as_ref());
            let first: Vec<i64> = told.iter().map(|told| told.0).collect();
            assert_eq!(first, [20001, 29001, 29005, 29002, 28001]);
            assert_eq!(told[1], (29001, Ground::Trend, 1, 0));
            assert_eq!(told[3], (29002, Ground::Before, 0, 1));
        }
        // Another front: nothing.
        let unseen = Front { first: 1, ..FRONT };
        assert!(store.counsel(&unseen, None, NOW).unwrap().likely.is_empty());
    }

    #[test]
    fn the_trend_counts_players_not_their_battles_and_the_anonymous_as_one() {
        let store = store();
        // One player's defense, met four times yesterday; one of two players, ten days ago;
        // one of players the game hides the names of, met three times two days ago.
        for damage in [1, 2, 3, 4] {
            days_ago(&store, 1, &against(damage * 100, 3, 31));
        }
        days_ago(&store, 10, &against(500, 4, 32));
        days_ago(&store, 10, &against(600, 5, 32));
        for damage in [7, 8, 9] {
            let mut hidden = against(damage * 100, 5, 33);
            hidden.defender.plate = None;
            days_ago(&store, 2, &hidden);
        }
        let told: Vec<(i64, u32)> = told(&store, None)
            .iter()
            .map(|told| (told.0, told.2))
            .collect();
        assert_eq!(
            told,
            [(32, 2), (31, 1), (33, 1)],
            "more players first, then the one used last: battles count for nothing"
        );
    }

    #[test]
    fn five_defenses_at_most_are_told() {
        let store = store();
        // Seven defenses, of one player each.
        for (days, second) in [
            (7, 31),
            (6, 32),
            (5, 33),
            (4, 34),
            (3, 35),
            (2, 36),
            (1, 37),
        ] {
            days_ago(&store, days, &against(second as u32 * 100, 3, second));
        }
        let first: Vec<i64> = told(&store, None).iter().map(|told| told.0).collect();
        assert_eq!(first, [37, 36, 35, 34, 33], "the ones used last");
    }

    #[test]
    fn a_battle_was_when_its_row_of_the_list_tells_not_when_rio_saw_it() {
        let store = store();
        let id = days_ago(&store, 0, &against(100, 3, 31));
        assert_eq!(told(&store, None), [(31, Ground::Trend, 1, 0)]);
        // Its row of the game's list tells twenty days ago: no trend of now.
        let then = NOW - 20 * DAY;
        let row = "INSERT INTO list_rows (attacked, won, earliest, latest, battle)
                   VALUES (1, 1, ?1, ?2, ?3)";
        store
            .0
            .execute(row, (then - 3600, then + 3600, id))
            .unwrap();
        assert_eq!(told(&store, None), [(31, Ground::Before, 0, 1)]);
    }

    /// What a formation is registered with when nothing was told with it.
    fn bare() -> Source {
        Source::default()
    }

    /// A record it was introduced with, and the page.
    fn told_with(battles: u32, wins: u32, url: &str) -> Source {
        Source {
            reported: Reported::new(battles, wins),
            url: Some(url.to_owned()),
        }
    }

    #[test]
    fn an_attack_told_to_win_is_registered_once_with_its_defense() {
        let store = store();
        let (attack, defense) = (
            [1, 2, 3, 4, 6, 5],
            [20000, 20001, 20002, 20003, 20005, 20004],
        );
        let (id, new) = store.register(attack, defense, &bare()).unwrap();
        assert!(new && id > 0, "new, under a number");
        // Which special stands left tells nothing: the same teams again.
        let again = (
            [1, 2, 3, 4, 5, 6],
            [20000, 20001, 20002, 20003, 20004, 20005],
        );
        assert_eq!(
            store.register(again.0, again.1, &bare()).unwrap(),
            (id, false),
            "the registration it is, not a new one"
        );
        // No battle tells of this defense: it is likely because it was registered, and the
        // attack against it has no battle either.
        let counsel = store.counsel(&FRONT, None, NOW).unwrap();
        assert_eq!(counsel.likely.len(), 1);
        let likely = &counsel.likely[0];
        assert_eq!(
            (likely.rest, likely.ground, likely.registered),
            ([20001, 20002, 20003], Ground::Registered, true)
        );
        let told = Tried {
            attack: again.0,
            wins: 0,
            battles: 0,
            registered: true,
            reported: None,
        };
        assert_eq!(likely.attacks, [told]);
        // Another front does not see it.
        let other = Front {
            first: 20001,
            ..FRONT
        };
        assert!(store.counsel(&other, None, NOW).unwrap().likely.is_empty());
        // Taken away, by either order of the specials: what it was comes back.
        let gone = store.unregister(attack, defense).unwrap();
        assert_eq!(gone.map(|gone| (gone.id, gone.attack)), Some((id, again.0)));
        assert!(store.counsel(&FRONT, None, NOW).unwrap().likely.is_empty());
        assert_eq!(store.unregister(attack, defense).unwrap(), None, "gone");
    }

    #[test]
    fn a_registration_keeps_the_record_and_the_page_it_was_told_with() {
        let store = store();
        let defense = [20000, 20001, 20002, 20003, 20005, 20004];
        let post = "https://x.com/someone/status/1";
        let (first, _) = store
            .register([1, 2, 3, 4, 6, 5], defense, &told_with(20, 18, post))
            .unwrap();
        let (second, _) = store
            .register([7, 8, 9, 10, 11, 12], defense, &bare())
            .unwrap();
        let kept = store.registered().unwrap();
        assert_eq!(
            kept.iter().map(|one| one.id).collect::<Vec<_>>(),
            [second, first],
            "the one registered last first"
        );
        assert_eq!(kept[1].attack, [1, 2, 3, 4, 5, 6], "the specials by id");
        assert_eq!(kept[1].defense, [20000, 20001, 20002, 20003, 20004, 20005]);
        assert_eq!(kept[1].source, told_with(20, 18, post));
        assert_eq!(kept[0].source, bare(), "nothing was told with it");
        assert_eq!(kept[1].added_at.len(), 19, "2026-11-21 13:10:00");
        assert_eq!(store.registration(first).unwrap().as_ref(), Some(&kept[1]));
        assert_eq!(store.registration(99).unwrap(), None);
        // Registered again with something else told: it stays as it was registered.
        let again = store.register([1, 2, 3, 4, 5, 6], defense, &bare());
        assert_eq!(again.unwrap(), (first, false));
        assert_eq!(
            store.registration(first).unwrap().unwrap().source,
            told_with(20, 18, post)
        );
        // The record goes with the attack to what is counselled: the one with a record
        // before those that have none, and of those the one registered last first.
        store
            .register([13, 8, 9, 10, 11, 12], defense, &bare())
            .unwrap();
        let counsel = store.counsel(&FRONT, None, NOW).unwrap();
        let told: Vec<_> = (counsel.likely[0].attacks.iter())
            .map(|tried| (tried.attack[0], tried.registered, tried.reported))
            .collect();
        assert_eq!(
            told,
            [
                (1, true, Reported::new(20, 18)),
                (13, true, None),
                (7, true, None)
            ]
        );
    }

    #[test]
    fn a_registration_is_changed_by_its_number() {
        let store = store();
        let defense = [20000, 20001, 20002, 20003, 20004, 20005];
        let (one, _) = store
            .register([1, 2, 3, 4, 5, 6], defense, &bare())
            .unwrap();
        let (other, _) = store
            .register([7, 8, 9, 10, 11, 12], defense, &bare())
            .unwrap();
        // Another attack, a record and a page: under the same number, registered when it was.
        let was = store.registration(one).unwrap().unwrap();
        let source = told_with(10, 7, "https://example.com/a");
        let changed = store.amend(one, [1, 2, 3, 30, 6, 5], defense, &source);
        assert_eq!(changed.unwrap(), Amended::Done);
        let now = store.registration(one).unwrap().unwrap();
        assert_eq!(now.attack, [1, 2, 3, 30, 5, 6], "the specials by id");
        assert_eq!((now.source, now.added_at), (source, was.added_at));
        assert_eq!(store.registered().unwrap().len(), 2);
        // What it was told with alone, the teams as they are.
        let changed = store.amend(one, now.attack, defense, &bare());
        assert_eq!(changed.unwrap(), Amended::Done);
        assert_eq!(store.registration(one).unwrap().unwrap().source, bare());
        // Into the teams of another registration: nothing changes.
        let taken = store.amend(one, [7, 8, 9, 10, 12, 11], defense, &bare());
        assert_eq!(
            taken.unwrap(),
            Amended::Taken(other),
            "the specials in either order"
        );
        assert_eq!(store.registration(one).unwrap().unwrap().attack, now.attack);
        // A number no registration has.
        let gone = store.amend(99, [1, 2, 3, 4, 5, 6], defense, &bare());
        assert_eq!(gone.unwrap(), Amended::Gone);
        assert_eq!(store.registered().unwrap().len(), 2);
    }

    #[test]
    fn a_registration_taken_away_is_put_back_as_it_was() {
        let store = store();
        let defense = [20000, 20001, 20002, 20003, 20004, 20005];
        let source = told_with(20, 18, "https://example.com/a");
        let (id, _) = store
            .register([1, 2, 3, 4, 5, 6], defense, &source)
            .unwrap();
        let (kept, _) = store
            .register([7, 8, 9, 10, 11, 12], defense, &bare())
            .unwrap();
        let was = store.registration(id).unwrap().unwrap();
        assert_eq!(
            store.forget(id).unwrap().as_ref(),
            Some(&was),
            "what it was"
        );
        let left: Vec<i64> = (store.registered().unwrap().iter())
            .map(|one| one.id)
            .collect();
        assert_eq!(left, [kept], "the other stays");
        assert_eq!(store.forget(id).unwrap(), None, "gone already");
        // Put back: the same number, time and what it was told with.
        assert!(store.restore(&was).unwrap());
        assert_eq!(store.registration(id).unwrap(), Some(was.clone()));
        // Its teams registered anew meanwhile: the new registration stays, nothing is doubled.
        store.forget(id).unwrap();
        let (anew, _) = store.register(was.attack, was.defense, &bare()).unwrap();
        assert!(!store.restore(&was).unwrap());
        let now: Vec<Registered> = store.registered().unwrap();
        assert_eq!(now.len(), 2);
        assert_eq!(store.registration(anew).unwrap().unwrap().source, bare());
    }

    #[test]
    fn a_database_kept_before_the_record_and_the_page_gains_them() {
        // The registrations of a database as the version 0.1.16 made it.
        let connection = rusqlite::Connection::open_in_memory().unwrap();
        let slots: String = super::SLOTS
            .iter()
            .map(|slot| format!("{slot} INTEGER NOT NULL, "))
            .collect();
        connection
            .execute_batch(&format!(
                "CREATE TABLE registered (
                     id INTEGER PRIMARY KEY,
                     added_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
                     {slots}
                     UNIQUE ({every})
                 );
                 INSERT INTO registered ({every})
                 VALUES (1, 2, 3, 4, 5, 6, 20000, 20001, 20002, 20003, 20004, 20005);",
                every = super::SLOTS.join(", ")
            ))
            .unwrap();
        let store = Store::prepared(connection).unwrap();
        let kept = store.registered().unwrap();
        assert_eq!(kept.len(), 1, "what was registered stays");
        assert_eq!(kept[0].attack, [1, 2, 3, 4, 5, 6]);
        assert_eq!(kept[0].source, bare(), "with nothing told");
        // The record and the page are kept from now on.
        let source = told_with(5, 4, "https://example.com/a");
        let changed = store.amend(kept[0].id, kept[0].attack, kept[0].defense, &source);
        assert_eq!(changed.unwrap(), Amended::Done);
        // Opened again, the columns are there.
        let store = Store::prepared(store.0).unwrap();
        assert_eq!(store.registered().unwrap()[0].source, source);
    }

    /// An attack by its first student, with the user's own wins and battles and those told
    /// of it.
    fn tried(first: i64, own: (u32, u32), told: Option<(u32, u32)>, registered: bool) -> Tried {
        Tried {
            attack: [first, 2, 3, 4, 5, 6],
            wins: own.0,
            battles: own.1,
            registered,
            reported: told.and_then(|(wins, battles)| Reported::new(battles, wins)),
        }
    }

    fn firsts(attacks: Vec<Tried>) -> Vec<i64> {
        attacks.iter().map(|tried| tried.attack[0]).collect()
    }

    #[test]
    fn the_attacks_are_ranked_by_the_share_they_are_taken_to_win() {
        // The user's own battles: two of three. Told alone: eighteen of twenty. Both: the
        // user's one win of one and none of ten told, (1 + 0) / (1 + 5); three of four and
        // nine of ten told, (3 + 4.5) / (4 + 5). And one registered with its students alone.
        let attacks = vec![
            tried(1, (2, 3), None, false),
            tried(2, (0, 0), None, true),
            tried(3, (0, 0), Some((18, 20)), true),
            tried(4, (1, 1), Some((0, 10)), true),
            tried(5, (3, 4), Some((9, 10)), true),
        ];
        assert_eq!(
            firsts(ranked(attacks)),
            [3, 5, 1, 4, 2],
            "0.9, 0.83, 0.67, 0.17, then the one whose share nobody knows"
        );
    }

    #[test]
    fn what_is_told_of_an_attack_counts_half_of_what_the_user_fought() {
        // Told to win eighteen of twenty, the user lost with it: three times, it still
        // stands over the user's two of three, (0 + 9) / (3 + 10) > 2 / 3; four times, under
        // it, 9 / 14 < 2 / 3. Counted whole, what was told would keep it first.
        let own = tried(1, (2, 3), None, false);
        let thrice = tried(2, (0, 3), Some((18, 20)), true);
        assert_eq!(firsts(ranked(vec![own.clone(), thrice])), [2, 1]);
        let four_times = tried(2, (0, 4), Some((18, 20)), true);
        assert_eq!(firsts(ranked(vec![four_times, own])), [1, 2]);
    }

    #[test]
    fn of_the_same_share_the_attack_with_more_battles_then_the_one_given_first() {
        // Every one won all: the user's one battle, four battles told (two, counted half),
        // the user's two. More battles first; the same, as given (the newest first).
        let all_won = vec![
            tried(1, (1, 1), None, false),
            tried(2, (0, 0), Some((4, 4)), true),
            tried(3, (2, 2), None, false),
        ];
        assert_eq!(firsts(ranked(all_won)), [2, 3, 1]);
        // Half won: the user's own two battles weigh more than two told.
        let half = vec![
            tried(1, (0, 0), Some((1, 2)), true),
            tried(2, (1, 2), None, false),
        ];
        assert_eq!(firsts(ranked(half)), [2, 1]);
        // Those nobody knows the share of stay as given, after one that lost its battle.
        let unknown = vec![
            tried(1, (0, 0), None, true),
            tried(2, (0, 0), None, true),
            tried(3, (0, 1), None, true),
        ];
        assert_eq!(firsts(ranked(unknown)), [3, 1, 2]);
    }

    #[test]
    fn what_was_registered_is_ranked_with_what_the_battles_tell() {
        let store = store();
        // The trend: two other players' defense (the second striker 31), won against.
        days_ago(&store, 1, &against(100, 3, 31));
        days_ago(&store, 1, &against(200, 4, 31));
        // The opponent's own (32), and another trend (34) of one other player.
        days_ago(&store, 2, &against(300, 5, 32));
        days_ago(&store, 1, &against(350, 3, 34));
        // Against the first trend: an attack that won once, and one that lost its battle.
        let with = |damage: u32, first: i64, won: bool| {
            let mut battle = against(damage, 3, 31);
            battle.attacker.team[0].student = first;
            battle.attacker_won = won;
            days_ago(&store, 1, &battle);
        };
        with(400, 15000, true);
        with(500, 16000, false);
        // Registered: a defense no battle tells (33); against the first trend's defense an
        // attack with no battle and nothing told, the attack that won twice, the one that
        // lost (told to win nine of ten), and one told to win eighteen of twenty.
        let defense = |second: i64| [20000, second, 20002, 20003, 20004, 20005];
        let attack = |first: i64| [first, 10001, 10002, 10003, 10004, 10005];
        let told = |battles, wins| Source {
            reported: Reported::new(battles, wins),
            url: None,
        };
        let register = |attack, defense, source: Source| {
            store.register(attack, defense, &source).unwrap();
        };
        register([1, 2, 3, 4, 5, 6], defense(33), bare());
        register([7, 8, 9, 10, 11, 12], defense(31), bare());
        register(attack(10000), defense(31), bare());
        register(attack(16000), defense(31), told(10, 9));
        register(attack(8), defense(31), told(20, 18));

        let counsel = store.counsel(&FRONT, Some(&plate(5)), NOW).unwrap();
        let grounds: Vec<_> = (counsel.likely.iter())
            .map(|likely| (likely.rest[0], likely.ground, likely.registered))
            .collect();
        assert_eq!(
            grounds,
            [
                (32, Ground::Theirs, false),
                (31, Ground::Registered, true),
                (33, Ground::Registered, true),
                (34, Ground::Trend, false),
            ],
            "the opponent's own first; of the registered, the one more players use"
        );
        // By the share each is taken to win: the two that won every battle (the one with
        // more battles first), the one told to win 0.9, the one that lost its battle but is
        // told to win, (0 + 4.5) / (1 + 5); last the one nothing tells of. None is dropped.
        let attacks: Vec<_> = (counsel.likely[1].attacks.iter())
            .map(|tried| (tried.attack[0], tried.wins, tried.battles, tried.registered))
            .collect();
        assert_eq!(
            attacks,
            [
                (10000, 2, 2, true),
                (15000, 1, 1, false),
                (8, 0, 0, true),
                (16000, 0, 1, true),
                (7, 0, 0, true),
            ]
        );
        let reported: Vec<_> = (counsel.likely[1].attacks.iter())
            .map(|tried| tried.reported)
            .collect();
        assert_eq!(
            reported,
            [
                None,
                None,
                Reported::new(20, 18),
                Reported::new(10, 9),
                None
            ]
        );
    }

    #[test]
    fn a_likely_defense_comes_with_the_attacks_that_won_against_it() {
        let store = store();
        // One attack wins once and loses once; one with a student not held wins its only
        // battle; one never won.
        store.save(&battle(100)).unwrap();
        let lost = Battle {
            attacker_won: false,
            ..battle(200)
        };
        store.save(&lost).unwrap();
        let mut rich = battle(300);
        rich.attacker.team[0].student = 19000;
        store.save(&rich).unwrap();
        let mut never = battle(400);
        never.attacker.team[1].student = 18000;
        never.attacker_won = false;
        store.save(&never).unwrap();

        let attacks = || -> Vec<(i64, u32, u32)> {
            let counsel = store.counsel(&FRONT, None, NOW).unwrap();
            let tried = counsel.likely[0].attacks.iter();
            tried
                .map(|tried| (tried.attack[0], tried.wins, tried.battles))
                .collect()
        };
        // Whether the user holds the students is not the records' to say: the window sets
        // aside what the user cannot take (`view::offered`).
        assert_eq!(attacks(), [(19000, 1, 1), (10000, 1, 2)]);
        // Six more attacks that won: all are told, for the window to pick from.
        for student in 17000..17006 {
            let mut more = battle(student as u32);
            more.attacker.team[2].student = student;
            store.save(&more).unwrap();
        }
        assert_eq!(attacks().len(), 8);
    }

    #[test]
    fn a_database_kept_before_the_users_side_was_told_gains_it() {
        // The battles of a database as the version 0.1.10 made it.
        let connection = rusqlite::Connection::open_in_memory().unwrap();
        let slots: String = super::SLOTS
            .iter()
            .map(|slot| format!("{slot} INTEGER NOT NULL, "))
            .collect();
        connection
            .execute_batch(&format!(
                "CREATE TABLE battles (
                     id INTEGER PRIMARY KEY,
                     seen_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
                     attacker INTEGER, defender INTEGER, attacker_won INTEGER NOT NULL,
                     {slots} fingerprint TEXT NOT NULL UNIQUE
                 );"
            ))
            .unwrap();
        let store = Store::prepared(connection).unwrap();
        assert!(store.keep(&battle(100)).unwrap().new);
        assert_eq!(store.me().unwrap(), store.known(&plate(2)).unwrap());
        // Opened again, the column is there.
        assert_eq!(
            Store::prepared(store.0).unwrap().battles().unwrap().len(),
            1
        );
    }

    #[test]
    fn a_battle_comes_back_as_it_was_read() {
        let store = store();
        let read = battle(100);
        store.save(&read).unwrap();
        let kept = &store.battles().unwrap()[0];
        assert!(kept.attacker_won);
        assert_eq!(
            kept.attack,
            read.attacker.team.map(|fighter| fighter.student)
        );
        assert_eq!(
            kept.defense,
            read.defender.team.map(|fighter| fighter.student)
        );
        assert_ne!(
            named(&kept.attacker).id,
            named(&kept.defender).id,
            "two players"
        );
        assert!(!kept.seen_at.is_empty());
    }

    #[test]
    fn the_latest_battle_comes_first() {
        let store = store();
        store.save(&battle(100)).unwrap();
        let lost = Battle {
            attacker_won: false,
            ..battle(200)
        };
        store.save(&lost).unwrap();
        let kept = store.battles().unwrap();
        assert!(!kept[0].attacker_won, "the battle kept last");
        assert!(kept[1].attacker_won);
    }

    #[test]
    fn a_player_met_again_is_the_same_player() {
        let store = store();
        store.save(&battle(100)).unwrap();
        // The same defender attacks back.
        let back = Battle {
            attacker: party(5, 20000, 300),
            defender: party(2, 10000, 300),
            attacker_won: false,
            user_attacked: false,
        };
        store.save(&back).unwrap();
        let kept = store.battles().unwrap();
        assert_eq!(named(&kept[0].attacker).id, named(&kept[1].defender).id);
        assert_eq!(named(&kept[0].defender).id, named(&kept[1].attacker).id);
    }

    #[test]
    fn a_player_is_named() {
        let store = store();
        store.save(&battle(100)).unwrap();
        let defender = named(&store.battles().unwrap()[0].defender).id;
        assert!(store.name(defender, "ライバル").unwrap());
        assert!(
            !store.name(defender + 100, "だれか").unwrap(),
            "no such player"
        );
        let kept = &store.battles().unwrap()[0];
        assert_eq!(named(&kept.defender).name.as_deref(), Some("ライバル"));
        assert_eq!(named(&kept.attacker).name, None);
    }

    /// A row of the game's list: a defense won against the player `opponent`, within the
    /// hour that ends at `at`.
    fn row(opponent: Option<i64>, at: i64) -> Sighting {
        Sighting {
            fight: Fight {
                attacked: false,
                won: true,
                opponent,
            },
            range: (at - 3600, at),
        }
    }

    #[test]
    fn a_battle_kept_has_one_number() {
        let store = store();
        let first = store.keep(&battle(100)).unwrap();
        assert!(first.new, "kept just now");
        let again = store.keep(&battle(100)).unwrap();
        assert_eq!((again.id, again.new), (first.id, false), "kept before");
        let other = store.keep(&battle(200)).unwrap();
        assert!(other.new && other.id != first.id, "another battle");
        assert_eq!(store.battles().unwrap().len(), 2);
    }

    #[test]
    fn a_name_of_the_list_is_the_player_of_the_records() {
        let store = store();
        store.save(&battle(100)).unwrap();
        let defender = named(&store.battles().unwrap()[0].defender).id;
        assert_eq!(store.opponent(Some(&plate(5))).unwrap(), Some(defender));
        let met = store.opponent(Some(&plate(7))).unwrap().expect("a player");
        assert_ne!(met, defender, "a name not met is a new player");
        assert_eq!(store.opponent(Some(&plate(7))).unwrap(), Some(met));
        assert_eq!(store.opponent(None).unwrap(), None, "nobody");
    }

    #[test]
    fn the_rows_listed_come_back_as_they_were_kept() {
        let store = store();
        let player = store.opponent(Some(&plate(3))).unwrap();
        let older = store.list(&row(player, 5000)).unwrap();
        let newer = store.list(&row(None, 9000)).unwrap();
        let listed = store.listed(0).unwrap();
        let ids: Vec<i64> = listed.iter().map(|row| row.id).collect();
        assert_eq!(ids, [older, newer]);
        assert_eq!(listed[1].fight, row(None, 9000).fight);
        assert_eq!(listed[1].range, (5400, 9000));
        assert_eq!(listed[0].fight.opponent, player);
        assert!(
            listed
                .iter()
                .all(|row| row.battle.is_none() && row.above.is_none())
        );
        // The time told again, narrower.
        store.narrow(older, (4000, 4500)).unwrap();
        assert_eq!(store.listed(0).unwrap()[0].range, (4000, 4500));
        let late: Vec<i64> = store
            .listed(6000)
            .unwrap()
            .iter()
            .map(|row| row.id)
            .collect();
        assert_eq!(late, [newer], "a row that ended before is left out");
    }

    /// The row kept right above each of `rows`.
    fn above(store: &Store, rows: &[i64]) -> Vec<Option<i64>> {
        let listed = store.listed(0).unwrap();
        let above = |id: &i64| listed.iter().find(|row| row.id == *id).unwrap().above;
        rows.iter().map(above).collect()
    }

    #[test]
    fn rows_the_list_shows_side_by_side_are_kept_so() {
        let store = store();
        let list = |at| store.list(&row(None, at)).unwrap();
        let (a, b, c) = (list(9000), list(8000), list(7000));
        store.join(a, b).unwrap();
        store.join(b, c).unwrap();
        assert_eq!(above(&store, &[a, b, c]), [None, Some(a), Some(b)]);
        // The list shows another row under the first: what was kept gives way.
        let d = list(8500);
        store.join(a, d).unwrap();
        assert_eq!(above(&store, &[a, d, b, c]), [None, Some(a), None, Some(b)]);
        // The list shows the row kept under another above it: never a ring.
        store.join(c, b).unwrap();
        assert_eq!(above(&store, &[b, c]), [Some(c), None]);
    }

    #[test]
    fn a_record_read_from_a_row_has_the_rows_time() {
        let store = store();
        let read = store.keep(&battle(100)).unwrap();
        store.save(&battle(200)).unwrap();
        let first = store.list(&row(None, 9000)).unwrap();
        let second = store.list(&row(None, 9000)).unwrap();
        store.link(first, read.id).unwrap();
        let kept = store.battles().unwrap();
        let fought = kept[1].fought.as_ref().expect("the battle read from a row");
        assert_eq!(fought.within, 3600, "the hour the row allowed");
        assert_eq!(fought.about.len(), "2026-11-18 00:10:00".len());
        assert_eq!(kept[0].fought, None, "a battle no row told of");
        assert_eq!(store.listed(0).unwrap()[0].battle, Some(read.id));
        // The record turns out to be the other row's: one row, one record.
        store.link(second, read.id).unwrap();
        let listed = store.listed(0).unwrap();
        assert_eq!((listed[0].battle, listed[1].battle), (None, Some(read.id)));
        assert!(store.battles().unwrap()[1].fought.is_some());
    }

    /// The numbers of the rows told.
    fn ids(sights: &Sights) -> Vec<Option<i64>> {
        let id = |sight: &Sight| sight.row.map(|row| row.id);
        sights.rows.iter().map(id).collect()
    }

    #[test]
    fn a_record_another_row_has_changes_hands_with_the_rows_own() {
        let store = store();
        let (one, other) = (
            store.keep(&battle(100)).unwrap(),
            store.keep(&battle(200)).unwrap(),
        );
        // Two rows that tell the same, each with the other's record: Rio could not tell
        // which of them was pressed.
        let first = store.list(&row(None, 9000)).unwrap();
        let second = store.list(&row(None, 9000)).unwrap();
        store.link(first, other.id).unwrap();
        store.link(second, one.id).unwrap();
        // The first is pressed, and shows the record the second has: they change hands.
        store.link(first, one.id).unwrap();
        let listed = store.listed(0).unwrap();
        assert_eq!(
            (listed[0].battle, listed[1].battle),
            (Some(one.id), Some(other.id)),
            "no row loses its record"
        );
        // A row that tells another fight had the record: it loses it, and takes none.
        let third = store
            .list(&row(store.opponent(Some(&plate(3))).unwrap(), 5000))
            .unwrap();
        store.link(third, other.id).unwrap();
        store.link(first, other.id).unwrap();
        let listed = store.listed(0).unwrap();
        let battles: Vec<Option<i64>> = listed.iter().map(|row| row.battle).collect();
        assert_eq!(battles, [Some(other.id), None, None]);
    }

    #[test]
    fn rows_kept_before_the_runs_could_be_trusted_are_kept_by_their_records_only() {
        // A database as the version 0.1.9 left it: rows side by side, one of them with a
        // record, one that tells the same kept under it that is not in the list.
        let old = Store::open(Path::new(":memory:")).unwrap();
        let read = old.keep(&battle(100)).unwrap();
        let rows: Vec<i64> = (0..3)
            .map(|_| old.list(&row(None, 9000)).unwrap())
            .collect();
        old.join(rows[0], rows[1]).unwrap();
        old.join(rows[1], rows[2]).unwrap();
        old.link(rows[1], read.id).unwrap();
        old.0.execute_batch("PRAGMA user_version = 0").unwrap();
        let store = Store::prepared(old.0).unwrap();
        let listed = store.listed(0).unwrap();
        let kept: Vec<_> = listed
            .iter()
            .map(|row| (row.id, row.above, row.battle))
            .collect();
        assert_eq!(kept, [(rows[1], None, Some(read.id))]);
        // Done once: rows kept from here on stay as they are kept.
        let under = store.list(&row(None, 5000)).unwrap();
        store.join(rows[1], under).unwrap();
        let store = Store::prepared(store.0).unwrap();
        assert_eq!(above(&store, &[under]), [Some(rows[1])]);
    }

    #[test]
    fn the_rows_in_sight_are_kept_and_told_again() {
        let store = store();
        let told = store
            .tell(&[Some(row(None, 9000)), None], &[None, None], &[true; 2])
            .unwrap();
        let first = ids(&told)[0].expect("the row read is kept");
        assert_eq!(ids(&told)[1], None, "a row that did not read is not");
        assert_eq!(told.rows[0].read, Some(false));
        assert_eq!(told.rows[1].read, None);
        // Seen again an hour on, its time narrower, and the row under it read now.
        let later = [
            Some(Sighting {
                range: (8000, 8500),
                ..row(None, 9000)
            }),
            Some(row(None, 2000)),
        ];
        let again = store.tell(&later, &[None, None], &[true; 2]).unwrap();
        let second = ids(&again)[1].expect("kept now");
        assert_eq!(ids(&again)[0], Some(first), "the same row");
        let listed = store.listed(0).unwrap();
        assert_eq!(listed.len(), 2, "no row twice");
        assert_eq!(listed[0].range, (8000, 8500));
        assert_eq!(above(&store, &[first, second]), [None, Some(first)]);
        // A record read from the first row shows with it.
        let read = store.keep(&battle(100)).unwrap();
        store.link(first, read.id).unwrap();
        let told = store.tell(&later, &[None, None], &[true; 2]).unwrap();
        let battles: Vec<Option<i64>> = told.rows.iter().map(|row| row.row?.battle).collect();
        assert_eq!(battles, [Some(read.id), None]);
        let read: Vec<Option<bool>> = told.rows.iter().map(|row| row.read).collect();
        assert_eq!(read, [Some(true), Some(false)]);
    }

    #[test]
    fn the_rows_cut_by_the_lists_edges_are_those_kept_beside_the_ones_in_sight() {
        let store = store();
        let rows: Vec<Option<Sighting>> = [90_000, 80_000, 70_000, 60_000]
            .into_iter()
            .map(|at| Some(row(None, at)))
            .collect();
        let top = store.tell(&rows[..3], &[None; 3], &[true; 3]).unwrap();
        assert_eq!((top.above, top.below), (None, None), "none seen there yet");
        // The list moves a row down: the first row is cut now.
        let carried = [ids(&top)[1], ids(&top)[2], None];
        let down = store.tell(&rows[1..], &carried, &[true; 3]).unwrap();
        assert_eq!(down.above.map(|row| row.id), ids(&top)[0]);
        assert_eq!(down.below, None);
        // And back up: the last is.
        let up = store.tell(&rows[..3], &[None; 3], &[true; 3]).unwrap();
        assert_eq!(ids(&up), ids(&top));
        assert_eq!(up.below.map(|row| row.id), ids(&down)[2]);
    }

    #[test]
    fn a_row_only_likely_is_kept_beside_no_row() {
        let store = store();
        // Two rows kept side by side, then two battles more: the list shows those and the
        // row that was on top.
        let old = [Some(row(None, 5000)), Some(row(None, 1000))];
        let before = store.tell(&old, &[None, None], &[true; 2]).unwrap();
        let rows = [
            Some(row(None, 20_000)),
            Some(row(None, 15_000)),
            Some(row(None, 5000)),
        ];
        let told = store.tell(&rows, &[None; 3], &[true; 3]).unwrap();
        let likely = told.rows[2].row.expect("told");
        assert_eq!((Some(likely.id), likely.sure), (ids(&before)[0], false));
        assert_eq!(above(&store, &[likely.id]), [None], "nothing is kept of it");
        assert_eq!(told.below, None, "nor told by it");
    }

    #[test]
    fn a_row_read_in_one_frame_only_is_told_if_kept_and_not_kept_if_new() {
        let store = store();
        let rows: Vec<Option<Sighting>> = [90_000, 80_000, 70_000]
            .into_iter()
            .map(|at| Some(row(None, at)))
            .collect();
        let kept = store.tell(&rows[..2], &[None; 2], &[true; 2]).unwrap();
        // The list moves a row down: the third row came into sight in this frame.
        let moved = store.tell(&rows[1..], &[None; 2], &[true, false]).unwrap();
        assert_eq!(
            ids(&moved),
            [ids(&kept)[1], None],
            "not kept on one reading"
        );
        assert_eq!(store.listed(0).unwrap().len(), 2);
        // And back up: the first row, read in this frame only, is the row kept.
        let back = store.tell(&rows[..2], &[None; 2], &[false, true]).unwrap();
        assert_eq!(ids(&back), ids(&kept));
        // A name not met makes no player on one reading.
        let met = |name| Row {
            attacked: false,
            won: true,
            opponent: Some(plate(name)),
            ago: Ago {
                count: 2,
                unit: Unit::Hour,
            },
        };
        let once = store.see(&[Some(met(4))], &[None], &[false], Some(100_000), 100_000);
        assert_eq!(ids(&once.unwrap()), [None]);
        assert_eq!(store.known(&plate(4)).unwrap(), None);
        let twice = store.see(&[Some(met(4))], &[None], &[true], Some(100_000), 100_000);
        assert!(ids(&twice.unwrap())[0].is_some());
        assert!(store.known(&plate(4)).unwrap().is_some());
    }

    #[test]
    fn a_list_that_was_up_before_rio_looked_keeps_no_new_row() {
        let store = store();
        let kept = store
            .tell(&[Some(row(None, 9000))], &[None], &[true])
            .unwrap();
        // Its rows' times do not count: only the rows kept are told.
        let seen = [Some(row(None, 9500)), Some(row(None, 9000))];
        let told = store.tell(&seen, &[None, None], &[false; 2]).unwrap();
        assert_eq!(
            ids(&told),
            [None, ids(&kept)[0]],
            "a row never kept stays untold"
        );
        assert_eq!(store.listed(0).unwrap().len(), 1);
    }

    #[test]
    fn the_rows_a_frame_reads_are_told_a_row_for_a_row() {
        let store = store();
        // A defense won so many hours ago, against a name of so many strokes or nobody.
        let row = |name: Option<u32>, count| {
            Some(Row {
                attacked: false,
                won: true,
                opponent: name.map(plate),
                ago: Ago {
                    count,
                    unit: Unit::Hour,
                },
            })
        };
        // A list that came up just now: a player with a name, a row that did not read, and
        // an anonymous player.
        let told = store
            .see(
                &[row(Some(5), 2), None, row(None, 5)],
                &[None; 3],
                &[true; 3],
                Some(100_000),
                100_000,
            )
            .unwrap();
        let first = told.rows[0].row.expect("the first row is kept");
        assert!(
            told.rows[1].row.is_none() && told.rows[2].row.is_some(),
            "a row for a row"
        );
        assert_eq!(
            first.fight.opponent,
            store.known(&plate(5)).unwrap(),
            "the name is a player's now"
        );
        assert!(first.fight.opponent.is_some());
        assert_eq!(
            told.rows[2].row.expect("kept").fight.opponent,
            None,
            "nobody"
        );
        assert_eq!(
            store.listed(0).unwrap()[0].range,
            (100_000 - 3 * 3600, 100_000 - 2 * 3600),
            "two hours ago and not yet three"
        );
        // An hour on, a list that was up before Rio looked: the row kept is told, a name
        // not met is nobody's and its row no row.
        let told = store
            .see(
                &[row(Some(7), 1), row(Some(5), 3)],
                &[None, None],
                &[true; 2],
                None,
                103_600,
            )
            .unwrap();
        assert_eq!(store.known(&plate(7)).unwrap(), None, "no player is made");
        assert_eq!(ids(&told), [None, Some(first.id)]);
        assert_eq!(store.listed(0).unwrap().len(), 2);
    }

    #[test]
    fn what_a_record_told_of_a_row_is_taken_over_from_the_rows_kept_at_places() {
        // A database as the version 0.1.7 left it.
        let old = Store::open(Path::new(":memory:")).unwrap();
        let read = old.keep(&battle(100)).unwrap();
        let other = old.keep(&battle(200)).unwrap();
        old.0
            .execute_batch(&format!(
                "CREATE TABLE listed (
                     id INTEGER PRIMARY KEY, place REAL NOT NULL, attacked INTEGER NOT NULL,
                     won INTEGER NOT NULL, opponent INTEGER REFERENCES players (id),
                     earliest INTEGER NOT NULL, latest INTEGER NOT NULL,
                     battle INTEGER UNIQUE REFERENCES battles (id)
                 );
                 INSERT INTO listed (place, attacked, won, earliest, latest, battle) VALUES
                     (1.0, 1, 1, 4000, 5000, {}), (3.0, 0, 1, 8000, 9000, {}),
                     (2.0, 0, 0, 6000, 7000, NULL);",
                read.id, other.id
            ))
            .unwrap();
        let store = Store::prepared(old.0).unwrap();
        let listed = store.listed(0).unwrap();
        let rows: Vec<_> = listed.iter().map(|row| (row.range, row.battle)).collect();
        assert_eq!(
            rows,
            [
                ((8000, 9000), Some(other.id)),
                ((4000, 5000), Some(read.id))
            ],
            "the rows a record was read of, the upper first"
        );
        assert!(listed.iter().all(|row| row.above.is_none()));
        assert!(store.battles().unwrap()[1].fought.is_some());
        // The old version run once more made its table anew and read the same record of a
        // row: taken over again, the record stays with the row that has it.
        store
            .0
            .execute_batch(&format!(
                "CREATE TABLE listed (
                     id INTEGER PRIMARY KEY, place REAL NOT NULL, attacked INTEGER NOT NULL,
                     won INTEGER NOT NULL, opponent INTEGER REFERENCES players (id),
                     earliest INTEGER NOT NULL, latest INTEGER NOT NULL,
                     battle INTEGER UNIQUE REFERENCES battles (id)
                 );
                 INSERT INTO listed (place, attacked, won, earliest, latest, battle) VALUES
                     (1.0, 1, 1, 4100, 4900, {});",
                read.id
            ))
            .unwrap();
        let store = Store::prepared(store.0).unwrap();
        assert_eq!(store.listed(0).unwrap(), listed);
        // Opened again, there is nothing left to take over.
        assert_eq!(
            Store::prepared(store.0).unwrap().listed(0).unwrap().len(),
            2
        );
    }
}
