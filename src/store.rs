//! The battles read, kept in SQLite: one row a battle, the teams in the players' terms
//! (`a1` to `a4` the attacker's strikers from the left, `d1` to `d4` the defender's,
//! `asp1`, `asp2`, `dsp1`, `dsp2` the specials, the lower id first).
//!
//! The game tells no battle's time closer than "20 hours ago", and the same two players
//! may fight again within a minute. The damage is what tells a battle from another: twelve
//! numbers that no other battle repeats. It is kept as the battle's fingerprint and as
//! nothing else, for it says little of who won (it leaves out healing and blows taken, and
//! grows on summons and on enemies healed).

use std::path::Path;

use rusqlite::{Connection, params_from_iter};

use crate::history::{Battle, Plate};

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
    pub attacker: Player,
    pub defender: Player,
    pub attacker_won: bool,
    pub attack: [i64; 6],
    pub defense: [i64; 6],
}

/// A player, known by the name drawn (see `history::Plate`) and named by the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Player {
    pub id: i64,
    pub name: Option<String>,
}

pub struct Store(Connection);

impl Store {
    /// The database at `path`, made if it is not there.
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let connection = Connection::open(path)?;
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
                attacker INTEGER NOT NULL REFERENCES players (id),
                defender INTEGER NOT NULL REFERENCES players (id),
                attacker_won INTEGER NOT NULL,
                {slots}
                fingerprint TEXT NOT NULL UNIQUE
            );"
        ))?;
        Ok(Self(connection))
    }

    /// Keeps a battle; `false` for one kept before.
    pub fn save(&self, battle: &Battle) -> rusqlite::Result<bool> {
        let fingerprint = fingerprint(battle);
        // Asked before the players are looked up, so a battle kept before makes no player.
        let kept: bool = self.0.query_row(
            "SELECT EXISTS (SELECT 1 FROM battles WHERE fingerprint = ?1)",
            [&fingerprint],
            |row| row.get(0),
        )?;
        if kept {
            return Ok(false);
        }
        let mut values = vec![
            self.player(&battle.attacker.plate)?,
            self.player(&battle.defender.plate)?,
            i64::from(battle.attacker_won),
        ];
        let fighters = battle.attacker.team.iter().chain(&battle.defender.team);
        values.extend(fighters.map(|fighter| fighter.student));
        let marks = vec!["?"; values.len()].join(", ");
        let sql = format!(
            "INSERT INTO battles (attacker, defender, attacker_won, {}, fingerprint)
             VALUES ({marks}, ?)",
            SLOTS.join(", ")
        );
        let values = values.into_iter().map(rusqlite::types::Value::from);
        self.0
            .execute(&sql, params_from_iter(values.chain([fingerprint.into()])))?;
        Ok(true)
    }

    /// The player a plate names: one kept whose plate draws the same name, or a new one.
    fn player(&self, plate: &Plate) -> rusqlite::Result<i64> {
        let mut players = self
            .0
            .prepare("SELECT id, plate FROM players ORDER BY id")?;
        let mut rows = players.query([])?;
        while let Some(row) = rows.next()? {
            let kept: Vec<u8> = row.get(1)?;
            if Plate::from_png(&kept).is_some_and(|kept| kept.same(plate)) {
                return row.get(0);
            }
        }
        self.0
            .execute("INSERT INTO players (plate) VALUES (?1)", [plate.png()])?;
        Ok(self.0.last_insert_rowid())
    }

    /// The battles kept, the latest first.
    pub fn battles(&self) -> rusqlite::Result<Vec<Kept>> {
        let slots: String = SLOTS.iter().map(|slot| format!(", b.{slot}")).collect();
        let mut battles = self.0.prepare(&format!(
            "SELECT b.id, b.seen_at, b.attacker, a.name, b.defender, d.name, b.attacker_won{slots}
             FROM battles b
             JOIN players a ON a.id = b.attacker
             JOIN players d ON d.id = b.defender
             ORDER BY b.id DESC"
        ))?;
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
                    attacker: Player {
                        id: row.get(2)?,
                        name: row.get(3)?,
                    },
                    defender: Player {
                        id: row.get(4)?,
                        name: row.get(5)?,
                    },
                    attacker_won: row.get(6)?,
                    attack: team(row, 7)?,
                    defense: team(row, 13)?,
                })
            })?
            .collect()
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

    use super::Store;
    use crate::history::{Battle, Fighter, Party, Plate};

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
            plate: plate(strokes),
            team: [0, 1, 2, 3, 4, 5].map(fighter),
        }
    }

    fn battle(damage: u32) -> Battle {
        Battle {
            attacker: party(2, 10000, damage),
            defender: party(5, 20000, damage),
            attacker_won: true,
        }
    }

    fn store() -> Store {
        Store::open(Path::new(":memory:")).unwrap()
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
        assert_ne!(kept.attacker.id, kept.defender.id, "two players");
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
        };
        store.save(&back).unwrap();
        let kept = store.battles().unwrap();
        assert_eq!(kept[0].attacker.id, kept[1].defender.id);
        assert_eq!(kept[0].defender.id, kept[1].attacker.id);
    }

    #[test]
    fn a_player_is_named() {
        let store = store();
        store.save(&battle(100)).unwrap();
        let defender = store.battles().unwrap()[0].defender.id;
        assert!(store.name(defender, "ライバル").unwrap());
        assert!(
            !store.name(defender + 100, "だれか").unwrap(),
            "no such player"
        );
        let kept = &store.battles().unwrap()[0];
        assert_eq!(kept.defender.name.as_deref(), Some("ライバル"));
        assert_eq!(kept.attacker.name, None);
    }
}
