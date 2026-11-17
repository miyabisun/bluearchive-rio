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
//! The rows of the game's list of records are kept too (`listed`, see `listing`): a row
//! tells roughly when its battle was, and a record opened from a row is that row's battle.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params_from_iter, types::Value};

use crate::{
    history::{Battle, Party, Plate},
    list::Row,
    listing::{self, Fight, Listed, Match, Sighting, Told},
    opponent::Front,
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

/// A defense seen before behind a first striker and two specials: the strikers D2 to D4,
/// the battles that showed them so, and when the last of those was seen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    pub rest: [i64; 3],
    pub battles: u32,
    pub last_seen: String,
}

/// An attack kept against one defense: the team, the battles it won and all its battles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tried {
    pub attack: [i64; 6],
    pub wins: u32,
    pub battles: u32,
}

/// What the records say against an opponent's front.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Counsel {
    /// The defenses seen behind the front, any player's, the most seen first.
    pub seen: Vec<Seen>,
    /// What stood behind it when the opponent last defended so.
    pub last: Option<[i64; 3]>,
    /// The attack that won most against the defense expected, of those that won at all.
    pub attack: Option<Tried>,
}

impl Counsel {
    /// The strikers expected behind the front: the opponent's own last defense, or else
    /// the defense seen most.
    pub fn behind(&self) -> Option<[i64; 3]> {
        self.last.or(self.seen.first().map(|seen| seen.rest))
    }
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
                attacker INTEGER REFERENCES players (id),
                defender INTEGER REFERENCES players (id),
                attacker_won INTEGER NOT NULL,
                {slots}
                fingerprint TEXT NOT NULL UNIQUE
            );
            CREATE TABLE IF NOT EXISTS listed (
                id INTEGER PRIMARY KEY,
                place REAL NOT NULL,
                attacked INTEGER NOT NULL,
                won INTEGER NOT NULL,
                opponent INTEGER REFERENCES players (id),
                earliest INTEGER NOT NULL,
                latest INTEGER NOT NULL,
                battle INTEGER UNIQUE REFERENCES battles (id)
            );"
        ))?;
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
        let marks = vec!["?"; values.len()].join(", ");
        let sql = format!(
            "INSERT INTO battles (attacker, defender, attacker_won, {}, fingerprint)
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

    /// The rows of the game's list kept, the newest first: the game lists far fewer than
    /// are asked for here.
    pub fn listed(&self) -> rusqlite::Result<Vec<Listed>> {
        let mut rows = self.0.prepare(
            "SELECT id, place, attacked, won, opponent, earliest, latest, battle FROM listed
             ORDER BY place DESC LIMIT 500",
        )?;
        rows.query_map([], |row| {
            Ok(Listed {
                id: row.get(0)?,
                place: row.get(1)?,
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

    /// Keeps a row of the game's list at a place of the list (see `listing`): its number.
    pub fn list(&self, row: &Sighting, place: f64) -> rusqlite::Result<i64> {
        self.0.execute(
            "INSERT INTO listed (place, attacked, won, opponent, earliest, latest)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                place,
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
            "UPDATE listed SET earliest = ?2, latest = ?3 WHERE id = ?1",
            rusqlite::params![row, range.0, range.1],
        )?;
        Ok(())
    }

    /// Takes a battle kept for the record of a row: the row's time is the battle's from
    /// here on, and no other row's.
    pub fn link(&self, row: i64, battle: i64) -> rusqlite::Result<()> {
        self.0.execute(
            "UPDATE listed SET battle = NULL WHERE battle = ?2 AND id <> ?1",
            [row, battle],
        )?;
        self.0
            .execute("UPDATE listed SET battle = ?2 WHERE id = ?1", [row, battle])?;
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

    /// What stood behind a first striker and specials in the last battle kept of those a
    /// player defended so. "Last" is the last Rio saw: the game tells no battle's time.
    pub fn last_defense(
        &self,
        player: i64,
        first: i64,
        mut specials: [i64; 2],
    ) -> rusqlite::Result<Option<[i64; 3]>> {
        specials.sort_unstable();
        self.0
            .query_row(
                "SELECT d2, d3, d4 FROM battles
                 WHERE defender = ?1 AND d1 = ?2 AND dsp1 = ?3 AND dsp2 = ?4
                 ORDER BY id DESC LIMIT 1",
                [player, first, specials[0], specials[1]],
                |row| Ok([row.get(0)?, row.get(1)?, row.get(2)?]),
            )
            .optional()
    }

    /// The attacks kept against a defense (D1 to D4, then the specials), the one that won
    /// the largest share of its battles first.
    pub fn attacks(&self, mut defense: [i64; 6]) -> rusqlite::Result<Vec<Tried>> {
        defense[4..].sort_unstable();
        let mut attacks = self.0.prepare(
            "SELECT a1, a2, a3, a4, asp1, asp2, SUM(attacker_won), COUNT(*) FROM battles
             WHERE d1 = ?1 AND d2 = ?2 AND d3 = ?3 AND d4 = ?4 AND dsp1 = ?5 AND dsp2 = ?6
             GROUP BY a1, a2, a3, a4, asp1, asp2
             ORDER BY 1.0 * SUM(attacker_won) / COUNT(*) DESC, COUNT(*) DESC, MAX(id) DESC",
        )?;
        attacks
            .query_map(defense, |row| {
                let mut attack = [0; 6];
                for (slot, student) in attack.iter_mut().enumerate() {
                    *student = row.get(slot)?;
                }
                Ok(Tried {
                    attack,
                    wins: row.get(6)?,
                    battles: row.get(7)?,
                })
            })?
            .collect()
    }

    /// What the records say against a front: the defense to expect behind it, and the
    /// attack to try. `opponent` is the opponent's name where the game shows one; of the
    /// attacks, only teams of the students `held` are taken, where the roster is known.
    pub fn counsel(
        &self,
        front: &Front,
        opponent: Option<&Plate>,
        held: Option<&[i64]>,
    ) -> rusqlite::Result<Counsel> {
        let player = match opponent {
            Some(plate) => self.known(plate)?,
            None => None,
        };
        let mut counsel = Counsel {
            seen: self.defenses(front.first, front.specials)?,
            last: match player {
                Some(player) => self.last_defense(player, front.first, front.specials)?,
                None => None,
            },
            attack: None,
        };
        if let Some([second, third, fourth]) = counsel.behind() {
            let [left, right] = front.specials;
            let tried = self.attacks([front.first, second, third, fourth, left, right])?;
            counsel.attack = tried.into_iter().find(|tried| {
                tried.wins > 0
                    && held.is_none_or(|held| tried.attack.iter().all(|one| held.contains(one)))
            });
        }
        Ok(counsel)
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
             LEFT JOIN listed l ON l.battle = b.id
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

    /// The defenses kept whose first striker and specials are these, the most seen first:
    /// what stood behind them. Only defenses are looked at: an attack's field differs.
    pub fn defenses(&self, first: i64, mut specials: [i64; 2]) -> rusqlite::Result<Vec<Seen>> {
        specials.sort_unstable();
        let mut defenses = self.0.prepare(
            "SELECT d2, d3, d4, COUNT(*), MAX(seen_at) FROM battles
             WHERE d1 = ?1 AND dsp1 = ?2 AND dsp2 = ?3
             GROUP BY d2, d3, d4
             ORDER BY COUNT(*) DESC, MAX(seen_at) DESC, d2, d3, d4",
        )?;
        defenses
            .query_map([first, specials[0], specials[1]], |row| {
                Ok(Seen {
                    rest: [row.get(0)?, row.get(1)?, row.get(2)?],
                    battles: row.get(3)?,
                    last_seen: row.get(4)?,
                })
            })?
            .collect()
    }

    /// Tells the rows a frame read of the game's list, a row for a row: `since` is when the
    /// list came up (seconds since 1970, `None` if Rio did not see it come), `clock` the
    /// time now. A name not met makes a player only where its row is kept: in a list that
    /// was up before Rio looked, only the latest time a row allows is known, and a row
    /// never kept stays untold.
    pub fn see(
        &self,
        rows: &[Option<Row>],
        since: Option<i64>,
        clock: i64,
    ) -> rusqlite::Result<Vec<Option<Told>>> {
        let mut seen = Vec::new();
        for row in rows {
            // The opponent's number: none for a row that tells no row kept.
            let opponent = match (row, since) {
                (None, _) => None,
                (Some(Row { opponent: None, .. }), _) => Some(None),
                (Some(Row { opponent: name, .. }), Some(_)) => Some(self.opponent(name.as_ref())?),
                (
                    Some(Row {
                        opponent: Some(name),
                        ..
                    }),
                    None,
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
        self.tell(&seen, since.is_some())
    }

    /// Tells the rows in sight of the game's list, from the top (`None` for a row that
    /// did not read): for each the row kept that it is, its time narrowed by what it tells
    /// now. A row not kept yet is kept if `new` (the list came up under Rio's eyes, so its
    /// rows' times count); a row that may be either of two kept is not told.
    pub fn tell(
        &self,
        seen: &[Option<Sighting>],
        new: bool,
    ) -> rusqlite::Result<Vec<Option<Told>>> {
        let kept = self.listed()?;
        let matches = listing::align(&kept, seen);
        let places = listing::places(&kept, seen, &matches);
        let mut told = Vec::new();
        for (at, row) in seen.iter().enumerate() {
            told.push(match (row, matches[at], places[at]) {
                (Some(row), Match::Kept(known), _) => {
                    let known = &kept[known];
                    let range = listing::narrowed(known.range, row.range);
                    if range != known.range {
                        self.narrow(known.id, range)?;
                    }
                    Some(Told {
                        id: known.id,
                        fight: known.fight,
                        battle: known.battle,
                    })
                }
                (Some(row), Match::New, Some(place)) if new => Some(Told {
                    id: self.list(row, place)?,
                    fight: row.fight,
                    battle: None,
                }),
                _ => None,
            });
        }
        Ok(told)
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

    use super::{Player, Store};
    use crate::{
        history::{Battle, Fighter, Party, Plate},
        list::{Ago, Row, Unit},
        listing::{Fight, Sighting},
        opponent::Front,
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
    fn the_defenses_behind_a_first_striker_are_counted() {
        let store = store();
        // The same defense twice, one with another second striker, one of another first.
        store.save(&battle(100)).unwrap();
        store.save(&battle(200)).unwrap();
        let mut other = battle(300);
        other.defender.team[1].student = 29001;
        store.save(&other).unwrap();
        let mut apart = battle(400);
        apart.defender.team[0].student = 29999;
        store.save(&apart).unwrap();

        // The specials are asked for in either order.
        let seen = store.defenses(20000, [20005, 20004]).unwrap();
        let counted: Vec<_> = seen.iter().map(|seen| (seen.rest, seen.battles)).collect();
        assert_eq!(
            counted,
            [([20001, 20002, 20003], 2), ([29001, 20002, 20003], 1)]
        );
        assert!(seen.iter().all(|seen| !seen.last_seen.is_empty()));
        assert!(
            store.defenses(10000, [10004, 10005]).unwrap().is_empty(),
            "an attack is not counted as a defense"
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
    fn the_last_defense_of_a_player_is_told() {
        let store = store();
        store.save(&battle(100)).unwrap();
        let mut later = battle(200);
        later.defender.team[2].student = 29002;
        store.save(&later).unwrap();
        // Another player's defense behind the same front, kept last.
        let mut other = battle(300);
        other.defender.plate = Some(plate(3));
        other.defender.team[3].student = 29003;
        store.save(&other).unwrap();

        let defender = store.known(&plate(5)).unwrap().expect("the defender");
        assert_eq!(
            store.last_defense(defender, 20000, [20005, 20004]).unwrap(),
            Some([20001, 29002, 20003]),
            "the player's own defense, the one kept last"
        );
        assert_eq!(
            store.last_defense(defender, 29999, [20004, 20005]).unwrap(),
            None,
            "another first striker"
        );
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

    #[test]
    fn the_counsel_prefers_the_players_own_defense_and_a_team_held() {
        let store = store();
        // The defense seen most behind the front is another player's.
        for damage in [100, 200, 250] {
            let mut common = battle(damage);
            common.defender.plate = Some(plate(3));
            store.save(&common).unwrap();
        }
        let mut own = battle(300);
        own.defender.team[1].student = 29001;
        store.save(&own).unwrap();
        // Against the player's own defense, a team with a student not held won too.
        let mut rich = battle(400);
        rich.defender.team[1].student = 29001;
        rich.attacker.team[0].student = 19000;
        store.save(&rich).unwrap();

        let front = Front {
            first: 20000,
            specials: [20004, 20005],
        };
        let held: Vec<i64> = (10000..10006).collect();
        let counsel = store.counsel(&front, Some(&plate(5)), Some(&held)).unwrap();
        assert_eq!(
            counsel.behind(),
            Some([29001, 20002, 20003]),
            "the player's"
        );
        assert_eq!(
            counsel.attack.as_ref().map(|tried| tried.attack[0]),
            Some(10000),
            "the team held"
        );
        // A player not met, and no roster: the defense seen most, and any winning team.
        let counsel = store.counsel(&front, Some(&plate(7)), None).unwrap();
        assert_eq!(counsel.last, None);
        assert_eq!(
            counsel.behind(),
            Some([20001, 20002, 20003]),
            "the most seen"
        );
        assert!(counsel.attack.is_some(), "the attack that won against it");
        // A front never seen.
        let unseen = Front { first: 1, ..front };
        let counsel = store.counsel(&unseen, None, None).unwrap();
        assert_eq!((counsel.behind(), counsel.attack), (None, None));
    }

    #[test]
    fn an_attack_that_never_won_is_not_counselled() {
        let store = store();
        let lost = Battle {
            attacker_won: false,
            ..battle(100)
        };
        store.save(&lost).unwrap();
        let front = Front {
            first: 20000,
            specials: [20004, 20005],
        };
        let counsel = store.counsel(&front, None, None).unwrap();
        assert_eq!(counsel.behind(), Some([20001, 20002, 20003]));
        assert_eq!(counsel.attack, None);
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
    fn the_rows_listed_come_back_from_the_newest() {
        let store = store();
        let player = store.opponent(Some(&plate(3))).unwrap();
        let older = store.list(&row(player, 5000), 1.0).unwrap();
        let newer = store.list(&row(None, 9000), 2.0).unwrap();
        let between = store.list(&row(player, 7000), 1.5).unwrap();
        let listed = store.listed().unwrap();
        let ids: Vec<i64> = listed.iter().map(|row| row.id).collect();
        assert_eq!(ids, [newer, between, older]);
        assert_eq!(listed[0].fight, row(None, 9000).fight);
        assert_eq!(listed[0].range, (5400, 9000));
        assert_eq!(listed[2].fight.opponent, player);
        assert!(listed.iter().all(|row| row.battle.is_none()));
        // The time told again, narrower.
        store.narrow(older, (4000, 4500)).unwrap();
        assert_eq!(store.listed().unwrap()[2].range, (4000, 4500));
    }

    #[test]
    fn a_record_read_from_a_row_has_the_rows_time() {
        let store = store();
        let read = store.keep(&battle(100)).unwrap();
        store.save(&battle(200)).unwrap();
        let first = store.list(&row(None, 9000), 2.0).unwrap();
        let second = store.list(&row(None, 9000), 1.0).unwrap();
        store.link(first, read.id).unwrap();
        let kept = store.battles().unwrap();
        let fought = kept[1].fought.as_ref().expect("the battle read from a row");
        assert_eq!(fought.within, 3600, "the hour the row allowed");
        assert_eq!(fought.about.len(), "2026-11-18 00:10:00".len());
        assert_eq!(kept[0].fought, None, "a battle no row told of");
        assert_eq!(store.listed().unwrap()[0].battle, Some(read.id));
        // The record turns out to be the other row's: one row, one record.
        store.link(second, read.id).unwrap();
        let listed = store.listed().unwrap();
        assert_eq!((listed[0].battle, listed[1].battle), (None, Some(read.id)));
        assert!(store.battles().unwrap()[1].fought.is_some());
    }

    #[test]
    fn the_rows_in_sight_are_kept_and_told_again() {
        let store = store();
        let seen = [Some(row(None, 9000)), None, Some(row(None, 2000))];
        let told = store.tell(&seen, true).unwrap();
        let ids: Vec<Option<i64>> = told.iter().map(|row| row.map(|row| row.id)).collect();
        assert!(
            ids[0].is_some() && ids[2].is_some(),
            "the rows read are kept"
        );
        assert_eq!(ids[1], None, "a row that did not read is not");
        assert!(told.iter().flatten().all(|row| row.battle.is_none()));
        // Seen again an hour on: the same rows, their times narrower.
        let later = [
            Some(Sighting {
                range: (8000, 8500),
                ..row(None, 9000)
            }),
            Some(row(None, 2000)),
        ];
        let again = store.tell(&later, true).unwrap();
        let again: Vec<Option<i64>> = again.iter().map(|row| row.map(|row| row.id)).collect();
        assert_eq!(again, [ids[0], ids[2]]);
        let listed = store.listed().unwrap();
        assert_eq!(listed.len(), 2, "no row twice");
        assert_eq!(listed[0].range, (8000, 8500));
        assert!(
            listed[0].place > listed[1].place,
            "the upper row is the newer"
        );
        // A record read from the first row shows with it.
        let read = store.keep(&battle(100)).unwrap();
        store.link(ids[0].unwrap(), read.id).unwrap();
        let told = store.tell(&later, true).unwrap();
        assert_eq!(told[0].and_then(|row| row.battle), Some(read.id));
        assert_eq!(told[1].and_then(|row| row.battle), None);
    }

    #[test]
    fn a_list_that_was_up_before_rio_looked_keeps_no_new_row() {
        let store = store();
        let kept = store.tell(&[Some(row(None, 9000))], true).unwrap();
        // Its rows' times do not count: only the rows kept are told.
        let seen = [Some(row(None, 9500)), Some(row(None, 9000))];
        let told = store.tell(&seen, false).unwrap();
        assert_eq!(told[1].map(|row| row.id), kept[0].map(|row| row.id));
        assert_eq!(told[0], None, "a row never kept stays untold");
        assert_eq!(store.listed().unwrap().len(), 1);
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
                Some(100_000),
                100_000,
            )
            .unwrap();
        let first = told[0].expect("the first row is kept");
        assert!(told[1].is_none() && told[2].is_some(), "a row for a row");
        assert_eq!(
            first.fight.opponent,
            store.known(&plate(5)).unwrap(),
            "the name is a player's now"
        );
        assert!(first.fight.opponent.is_some());
        assert_eq!(told[2].expect("kept").fight.opponent, None, "nobody");
        assert_eq!(
            store.listed().unwrap()[0].range,
            (100_000 - 3 * 3600, 100_000 - 2 * 3600),
            "two hours ago and not yet three"
        );
        // An hour on, a list that was up before Rio looked: the row kept is told, a name
        // not met is nobody's and its row no row.
        let told = store
            .see(&[row(Some(7), 1), row(Some(5), 3)], None, 103_600)
            .unwrap();
        assert_eq!(told[0], None);
        assert_eq!(store.known(&plate(7)).unwrap(), None, "no player is made");
        assert_eq!(told[1].map(|row| row.id), Some(first.id));
        assert_eq!(store.listed().unwrap().len(), 2);
    }
}
