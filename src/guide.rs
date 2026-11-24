//! What Rio marks over the game, on the measured screen (see `history`): the strikers the
//! user takes to stand where the game shows "?", and in the formation dialog the cards to
//! pick and to take out for the attack the user chose. Positions in the team are set after the
//! dialog, by dragging: in it, the six students are only chosen. Over a battle record it
//! says whether the record is kept, and over the list of records which rows' records are:
//! the user's eyes stay on the game.

use std::{
    cmp::Ordering,
    collections::HashMap,
    time::{Duration, Instant},
};

use crate::{
    Student,
    aoi::Held,
    formation::{CARD_AROUND, FRONT_SIDE, Formation, HIDDEN, LIST, TABS, TEAM, TEAM_SIDE, Tab},
    history::Area,
    list::BUTTON,
    opponent::{CARD, CARD_SIDE, CARD_STEP},
};

/// The way to scroll the list to a student it does not show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    Up,
    Down,
    /// The list's order does not tell.
    Either,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mark {
    /// A student expected where the game shows "?": the icon about a middle, in a square.
    Guess {
        student: i64,
        middle: (f32, f32),
        side: f32,
    },
    /// Something to press: a card of the list to pick, or the other tab.
    Pick(Area),
    /// A card of the team to take out.
    Drop(Area),
    /// A student to pick that the list does not show: the icon, and the way to scroll.
    Seek {
        student: i64,
        way: Way,
        middle: (f32, f32),
    },
    /// Words on a ground of their own, `tall` high: their left end and their middle at a
    /// point.
    Say {
        words: Words,
        at: (f32, f32),
        tall: f32,
    },
}

/// What Rio says over the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Words {
    /// A record is being read, and not kept yet.
    Reading,
    /// The record was kept just now.
    Kept,
    /// The record was kept before.
    Known,
    /// A row's record was read.
    Read,
    /// A row's record was not read yet.
    Unread,
    /// The list was up before Rio looked: its rows are read once it is opened anew.
    Reopen,
}

impl Words {
    pub const ALL: [Self; 6] = [
        Self::Reading,
        Self::Kept,
        Self::Known,
        Self::Read,
        Self::Unread,
        Self::Reopen,
    ];

    /// The words as the user reads them.
    pub fn text(self) -> &'static str {
        match self {
            Self::Reading => "読み取り中",
            Self::Kept => "登録しました",
            Self::Known => "既に登録済み",
            Self::Read => "読み取り済み",
            Self::Unread => "未読",
            Self::Reopen => "開き直すと読み取ります",
        }
    }

    /// The name of the words' picture in `data/words`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Reading => "reading",
            Self::Kept => "kept",
            Self::Known => "known",
            Self::Read => "read",
            Self::Unread => "unread",
            Self::Reopen => "reopen",
        }
    }
}

/// Where a record's words begin, right of its title, and how tall they are; how tall a
/// row's are, and where they begin from the corner of the row's button.
const RECORD_SAY: (f32, f32) = (1322.0, 226.0);
const RECORD_TALL: f32 = 56.0;
const ROW_SAY: (f32, f32) = (10.0, 2.0);
const ROW_SAY_TALL: f32 = 40.0;
/// Where the list's own words begin, right of its title, and how tall they are.
const LIST_SAY: (f32, f32) = (1322.0, 323.0);
const LIST_SAY_TALL: f32 = 48.0;

/// The words over a battle record.
pub fn for_record(words: Words) -> Vec<Mark> {
    vec![Mark::Say {
        words,
        at: RECORD_SAY,
        tall: RECORD_TALL,
    }]
}

/// The words over the rows of the list of records: for each row its top, and whether its
/// record was read. A row to open has its button framed. A list that is not `timed` (it
/// was up before Rio looked, so its rows' times do not count) says beside its title that
/// opening it anew has it read.
pub fn for_list(rows: &[(f32, bool)], timed: bool) -> Vec<Mark> {
    let (left, up, right, down) = BUTTON;
    let mut marks = Vec::new();
    if !timed {
        marks.push(Mark::Say {
            words: Words::Reopen,
            at: LIST_SAY,
            tall: LIST_SAY_TALL,
        });
    }
    for &(top, read) in rows {
        if !read {
            marks.push(Mark::Pick((left, top + up, right, top + down)));
        }
        marks.push(Mark::Say {
            words: if read { Words::Read } else { Words::Unread },
            at: (left + ROW_SAY.0, top + ROW_SAY.1 + ROW_SAY_TALL / 2.0),
            tall: ROW_SAY_TALL,
        });
    }
    marks
}

/// What the list orders a student by, after the students of the team as it stood when the
/// dialog opened: the level, the stars, then the bond, the higher first; among students
/// alike in all three, the school in the game's order of schools (`SCHOOLS`), then the name
/// as the game sorts names (`names`). Five stars are the most (a weapon's stars do not
/// count). Measured on 2026-11-24 over the game's student list in its Lv order: 264 of the
/// 265 neighbours of 266 students stand so, the other with a bond Aoi had read before it
/// rose; the order the students were accepted in tells nothing (76 pairs one way, 77 the
/// other), nor do the weapon, the skills, the equipment, the HP or the attack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rank {
    pub level: u8,
    pub rarity: u8,
    pub bond: u8,
    /// The school's place in `SCHOOLS`; `None` for a school not among them.
    pub school: Option<u8>,
    pub name: String,
}

impl Rank {
    /// Where a student of this rank stands in the list against one of `other`: before it
    /// (`Less`) or after it. `None` where the two are alike in level, stars and bond and
    /// their schools or names do not tell.
    pub fn against(&self, other: &Self) -> Option<Ordering> {
        let key = |rank: &Self| (rank.level, rank.rarity, rank.bond);
        match key(other).cmp(&key(self)) {
            Ordering::Equal => match self.school?.cmp(&other.school?) {
                Ordering::Equal => names(&self.name, &other.name),
                decided => Some(decided),
            },
            decided => Some(decided),
        }
    }
}

/// The schools in the order the game's student list shows them sorted by school (学校),
/// by SchaleDB's names for them: read from the game on 2026-11-24. A school the list did
/// not show then (a collaboration's) is not placed.
pub const SCHOOLS: [&str; 13] = [
    "Hyakkiyako",
    "RedWinter",
    "Trinity",
    "Gehenna",
    "Abydos",
    "Millennium",
    "Arius",
    "Shanhaijing",
    "Valkyrie",
    "WildHunt",
    "SRT",
    "Highlander",
    "Odyssey",
];

/// A school's place in `SCHOOLS`.
pub fn school_place(school: &str) -> Option<u8> {
    let place = SCHOOLS.iter().position(|known| *known == school)?;
    u8::try_from(place).ok()
}

/// Where the name `a` stands against `b` as the game sorts names, rising: before it
/// (`Less`) or after it. Read from the game's student list sorted by name (名前) on
/// 2026-11-24: a voiced or small kana counts as its plain one (アズサ before アスナ), a name
/// that starts the other stands first (アズサ before アズサ（水着）), and kana stand before
/// kanji (（バニーガール） before （制服）); over 102 names, the pairs told so were all the
/// game's. `None` where the first difference is anything else (two kanji, which the game
/// orders as JIS does, a sign, the long vowel mark), or there is none.
pub fn names(a: &str, b: &str) -> Option<Ordering> {
    let (mut a, mut b) = (a.chars().filter_map(plain), b.chars().filter_map(plain));
    loop {
        match (a.next(), b.next()) {
            (None, None) => return None,
            (None, Some(_)) => return Some(Ordering::Less),
            (Some(_), None) => return Some(Ordering::Greater),
            (Some(x), Some(y)) if x == y => {}
            (Some(x), Some(y)) => {
                return match (Letter::of(x), Letter::of(y)) {
                    (Letter::Kana, Letter::Kana) => Some(x.cmp(&y)),
                    (Letter::Kana, Letter::Kanji) => Some(Ordering::Less),
                    (Letter::Kanji, Letter::Kana) => Some(Ordering::Greater),
                    _ => None,
                };
            }
        }
    }
}

/// The voiced kana and the small ones, each over the plain kana it counts as.
const MARKED: &str = "ガギグゲゴザジズゼゾダヂヅデドバビブベボパピプペポヴァィゥェォッャュョヮヵヶ";
const PLAIN: &str = "カキクケコサシスセソタチツテトハヒフヘホハヒフヘホウアイウエオツヤユヨワカケ";

/// A letter as the names' order sees it at first: hiragana as katakana, a voiced or small
/// kana as its plain one; a voicing mark written apart is no letter.
fn plain(letter: char) -> Option<char> {
    let letter = match u32::from(letter) {
        0x3099 | 0x309a => return None,
        code @ 0x3041..=0x3096 => char::from_u32(code + 0x60)?,
        _ => letter,
    };
    let marked = MARKED.chars().position(|marked| marked == letter);
    Some(
        marked
            .and_then(|at| PLAIN.chars().nth(at))
            .unwrap_or(letter),
    )
}

/// What a letter is to the names' order.
enum Letter {
    Kana,
    Kanji,
    Other,
}

impl Letter {
    fn of(letter: char) -> Self {
        match u32::from(letter) {
            0x30a1..=0x30fa => Self::Kana,
            0x4e00..=0x9fff => Self::Kanji,
            _ => Self::Other,
        }
    }
}

/// The ranks of the students held, by id: the level, the stars and the bond as Aoi read
/// them, the school and the name of the snapshot's student of that id (a student the
/// snapshot lacks is of no known school). A student Aoi could not read all three of, or
/// gave no id, has no rank.
pub fn ranked(held: &[Held], students: &[Student]) -> HashMap<i64, Rank> {
    let rank = |held: &Held| {
        let id = held.id?;
        let student = students.iter().find(|student| student.id == id);
        let rank = Rank {
            level: held.level?,
            rarity: held.rarity?,
            bond: held.bond?,
            school: student.and_then(|student| school_place(&student.school)),
            name: student.map_or_else(|| held.name.clone(), |student| student.name.clone()),
        };
        Some((id, rank))
    };
    held.iter().filter_map(rank).collect()
}

/// The side of a sought student's icon, the middle of the first one near the list's top
/// (to scroll up) and above the OK button (to scroll down), and the step to the next, to
/// the left.
pub(crate) const SEEK_SIDE: f32 = 100.0;
const SEEK_UP: (f32, f32) = (2232.0, 455.0);
const SEEK_DOWN: (f32, f32) = (2232.0, 918.0);
const SEEK_STEP: f32 = 122.0;

/// The way to a student the list does not show, from the cards it shows (in its order),
/// the students placed when the dialog opened and the students' ranks.
pub fn way(student: i64, cards: &[i64], entered: &[i64], rank: &HashMap<i64, Rank>) -> Way {
    // The students entered with stand first: one of them out of sight is above, and with
    // one of them in sight the list is at its top.
    if entered.contains(&student) {
        return Way::Up;
    }
    if cards.iter().any(|card| entered.contains(card)) {
        return Way::Down;
    }
    let of = |student: Option<&i64>| student.and_then(|student| rank.get(student));
    let (Some(own), Some(first), Some(last)) =
        (of(Some(&student)), of(cards.first()), of(cards.last()))
    else {
        return Way::Either;
    };
    // The cards in sight are a stretch of the order, and the student is not among them:
    // before its last card, the student stands before all of it; after its first, after
    // all of it. Told both (the ranks are not the list's), or neither, the way is not known.
    match (own.against(first), own.against(last)) {
        (Some(Ordering::Less) | None, Some(Ordering::Less)) | (Some(Ordering::Less), None) => {
            Way::Up
        }
        (Some(Ordering::Greater), Some(Ordering::Greater) | None)
        | (None, Some(Ordering::Greater)) => Way::Down,
        _ => Way::Either,
    }
}

fn guesses(behind: [i64; 3], middle: impl Fn(usize) -> (f32, f32), side: f32) -> Vec<Mark> {
    (0..3)
        .map(|at| Mark::Guess {
            student: behind[at],
            middle: middle(at),
            side,
        })
        .collect()
}

/// The marks over the opponent's dialog: the strikers the user takes to stand behind the
/// front.
pub fn for_opponent(behind: [i64; 3]) -> Vec<Mark> {
    let card = |at: usize| (CARD.0 + (at + 1) as f32 * CARD_STEP, CARD.1);
    guesses(behind, card, CARD_SIDE)
}

/// The marks over the formation dialog: the strikers the user takes to stand behind the
/// front, and for the `attack` chosen (A1 to A4, then the specials) the cards to take out of the
/// team, the cards of the open tab to pick, the students to scroll to, and the other tab
/// once this one is done.
pub fn for_formation(
    shown: &Formation,
    behind: Option<[i64; 3]>,
    attack: Option<[i64; 6]>,
    entered: &[i64],
    rank: &HashMap<i64, Rank>,
) -> Vec<Mark> {
    let mut marks = behind.map_or_else(Vec::new, |behind| {
        guesses(behind, |at| HIDDEN[at], FRONT_SIDE)
    });
    let Some(attack) = attack else {
        return marks;
    };
    let placed = |student: &i64| shown.placed.contains(&Some(*student));
    for (slot, student) in shown.placed.iter().enumerate() {
        if student.is_some_and(|student| !attack.contains(&student)) {
            let ((x, y), half) = (TEAM[slot], TEAM_SIDE / 2.0);
            marks.push(Mark::Drop((x - half, y - half, x + half, y + half)));
        }
    }
    let (strikers, specials) = attack.split_at(4);
    let (wanted, others, other_tab) = match shown.tab {
        Tab::Striker => (strikers, specials, TABS[1]),
        Tab::Special => (specials, strikers, TABS[0]),
    };
    let in_sight: Vec<i64> = shown.cards.iter().map(|card| card.student).collect();
    let (mut above, mut below) = (0.0, 0.0);
    for student in wanted.iter().filter(|student| !placed(student)) {
        if let Some(card) = shown.cards.iter().find(|card| card.student == *student) {
            let ((x, y), (side, up, down)) = (card.middle, CARD_AROUND);
            marks.push(Mark::Pick((
                x - side,
                (y - up).max(LIST.0),
                x + side,
                (y + down).min(LIST.1),
            )));
            continue;
        }
        let way = way(*student, &in_sight, entered, rank);
        let (first, taken) = match way {
            Way::Up => (SEEK_UP, &mut above),
            Way::Down | Way::Either => (SEEK_DOWN, &mut below),
        };
        marks.push(Mark::Seek {
            student: *student,
            way,
            middle: (first.0 - *taken * SEEK_STEP, first.1),
        });
        *taken += 1.0;
    }
    if wanted.iter().all(placed) && !others.iter().all(placed) {
        marks.push(Mark::Pick(other_tab));
    }
    marks
}

/// The dialog is taken to have closed when it was not read for this long.
const AWAY: Duration = Duration::from_secs(2);

/// The team as it stood when the formation dialog opened: the list shows those students
/// first, whatever is picked afterwards.
#[derive(Default)]
pub struct Visit {
    entered: Vec<i64>,
    seen: Option<Instant>,
}

impl Visit {
    /// The students placed when the dialog opened, given the team a reading shows `now`.
    pub fn enter(&mut self, placed: &[Option<i64>; 6], now: Instant) -> &[i64] {
        if self.seen.is_none_or(|seen| now.duration_since(seen) > AWAY) {
            self.entered = placed.iter().flatten().copied().collect();
        }
        self.seen = Some(now);
        &self.entered
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        time::{Duration, Instant},
    };

    use super::{
        Mark, Rank, Visit, Way, Words, for_formation, for_list, for_opponent, for_record, names,
        ranked, school_place, way,
    };
    use crate::{
        formation::{Card, Formation, Tab},
        opponent::Front,
    };

    fn rank(level: u8, rarity: u8, bond: u8) -> Rank {
        Rank {
            level,
            rarity,
            bond,
            school: None,
            name: String::new(),
        }
    }

    /// A student alike every other of these tests' in level, stars and bond.
    fn alike(school: &str, name: &str) -> Rank {
        Rank {
            school: school_place(school),
            name: name.to_owned(),
            ..rank(90, 5, 30)
        }
    }

    /// Students 1 to 4 entered; 5 to 9 fall in rank, 7 and 8 alike.
    fn ranks() -> HashMap<i64, Rank> {
        [
            (5, rank(90, 5, 50)),
            (6, rank(90, 5, 30)),
            (7, rank(90, 4, 30)),
            (8, rank(90, 4, 30)),
            (9, rank(80, 5, 20)),
        ]
        .into()
    }

    #[test]
    fn the_way_to_a_student_follows_the_lists_order() {
        let (entered, ranks) = ([1, 2, 3, 4], ranks());
        let way = |student, cards: &[i64]| way(student, cards, &entered, &ranks);
        assert_eq!(way(9, &[1, 2, 3, 4, 5, 6]), Way::Down, "the list's top");
        assert_eq!(way(2, &[6, 7, 8]), Way::Up, "placed when the dialog opened");
        assert_eq!(way(5, &[6, 7, 8]), Way::Up, "a higher bond than the first");
        assert_eq!(way(9, &[6, 7, 8]), Way::Down, "a lower level than the last");
        assert_eq!(
            way(7, &[5, 6, 8]),
            Way::Down,
            "alike the last, not the first"
        );
        assert_eq!(way(8, &[7, 9]), Way::Up, "alike the first, not the last");
        assert_eq!(way(7, &[8]), Way::Either, "alike every card in sight");
        assert_eq!(
            way(77, &[6, 7, 8]),
            Way::Either,
            "a student of no known rank"
        );
        assert_eq!(way(9, &[]), Way::Either, "no card read");
    }

    /// The game's own name order, read from its student list sorted by name on 2026-11-24:
    /// a voiced kana counts as its plain one, a name that starts another stands first, kana
    /// stand before kanji; two kanji, signs and the long vowel mark are not decided.
    #[test]
    fn names_stand_as_the_game_sorts_them() {
        use std::cmp::Ordering::{Greater, Less};
        // Neighbours of the game's list, each standing before the next.
        for pair in [
            ["アイリ", "アイリ（バンド）"],
            ["アイリ（バンド）", "アオバ"],
            ["アズサ（水着）", "アスナ"],
            ["カズサ（バンド）", "カスミ"],
            ["ジュリ", "シュン"],
            ["アズサ（レーシング）", "アズサ（水着）"],
            ["アスナ（バニーガール）", "アスナ（制服）"],
        ] {
            assert_eq!(names(pair[0], pair[1]), Some(Less), "{pair:?}");
            assert_eq!(names(pair[1], pair[0]), Some(Greater), "{pair:?}");
        }
        for pair in [
            ["イズミ（水着）", "イズミ（正月）"],
            ["シロコ（水着）", "シロコ＊テラー"],
            ["マリー", "マリナ"],
            ["ハス", "バス"],
            ["ミカ", "ミカ"],
        ] {
            assert_eq!(names(pair[0], pair[1]), None, "{pair:?}");
            assert_eq!(names(pair[1], pair[0]), None, "{pair:?}");
        }
    }

    /// Students alike in level, stars and bond stand by their school in the game's order of
    /// schools, and within a school by name (measured on 2026-11-24: 264 of 265 neighbours
    /// of a list of 266, the other a bond Aoi had read before it rose).
    #[test]
    fn students_alike_stand_by_school_then_name() {
        let ranks: HashMap<i64, Rank> = [
            (11, alike("Hyakkiyako", "ワカモ")),
            (12, alike("Trinity", "ミカ（水着）")),
            (13, alike("Abydos", "シロコ（水着）")),
            (14, alike("Abydos", "ノノミ")),
            (15, alike("Abydos", "ホシノ（水着）")),
            (16, rank(90, 5, 30)),
            (17, alike("Tokiwadai", "ミサカ")),
            (18, alike("Abydos", "シロコ＊テラー")),
            (20, rank(90, 5, 50)),
            (21, rank(80, 5, 20)),
        ]
        .into();
        let way = |student, cards: &[i64]| way(student, cards, &[], &ranks);
        assert_eq!(
            way(11, &[12, 13, 14]),
            Way::Up,
            "a school before the first's"
        );
        assert_eq!(way(15, &[12, 13, 14]), Way::Down, "a name after the last's");
        assert_eq!(
            way(13, &[14]),
            Way::Up,
            "one card in sight, a name before it"
        );
        assert_eq!(
            way(15, &[14]),
            Way::Down,
            "one card in sight, a name after it"
        );
        // The level, the stars and the bond still come first.
        assert_eq!(way(20, &[12, 13]), Way::Up);
        assert_eq!(way(21, &[12, 13]), Way::Down);
        assert_eq!(way(15, &[20, 13]), Way::Down, "after the last, alike it");
        assert_eq!(way(11, &[13, 21]), Way::Up, "before the first, alike it");
        // A school or a name that does not tell: as before.
        assert_eq!(
            way(16, &[12, 13]),
            Way::Either,
            "a student of no known school"
        );
        assert_eq!(way(11, &[16]), Way::Either, "a card of no known school");
        assert_eq!(
            way(17, &[12, 13]),
            Way::Either,
            "a school the order was not read for"
        );
        assert_eq!(way(18, &[13]), Way::Either, "names that do not tell");
        assert_eq!(
            way(16, &[20, 12]),
            Way::Down,
            "alike the last, not the first"
        );
        assert_eq!(way(16, &[12, 21]), Way::Up, "alike the first, not the last");
        // Between the first and the last and yet not in sight: the ranks are not the list's.
        assert_eq!(way(13, &[12, 15]), Way::Either);
        assert_eq!(way(13, &[15, 12]), Way::Either, "a stretch out of order");
    }

    /// A student's rank is the level, the stars and the bond Aoi read, with the school and
    /// the name of the snapshot's student of that id; one Aoi could not read in full, or
    /// without an id, has none.
    #[test]
    fn ranks_join_aois_reading_with_the_snapshot() {
        let held = |id: Option<i64>, level: Option<u8>| crate::aoi::Held {
            id,
            name: "名簿の名前".to_owned(),
            level,
            rarity: Some(5),
            bond: Some(30),
        };
        let students = crate::students();
        let wakamo = students.iter().find(|s| s.name == "ワカモ").unwrap().id;
        let ranks = ranked(
            &[
                held(Some(wakamo), Some(90)),
                held(Some(10005), None),
                held(None, Some(90)),
                held(Some(99_999), Some(90)),
            ],
            &students,
        );
        assert_eq!(ranks.len(), 2, "{ranks:?}");
        let known = &ranks[&wakamo];
        assert_eq!((known.level, known.rarity, known.bond), (90, 5, 30));
        assert_eq!(known.school, school_place("Hyakkiyako"));
        assert_eq!(known.name, "ワカモ");
        // Newer than the snapshot: ordered by the level, the stars and the bond alone.
        assert_eq!(ranks[&99_999].school, None);
    }

    /// Every school of the snapshot but a collaboration's has its place in the game's order.
    #[test]
    fn the_schools_of_the_snapshot_have_their_places() {
        let unplaced: std::collections::BTreeSet<String> = crate::students()
            .into_iter()
            .filter(|student| school_place(&student.school).is_none())
            .map(|student| student.school)
            .collect();
        let unplaced: Vec<&str> = unplaced.iter().map(String::as_str).collect();
        assert_eq!(unplaced, ["ETC", "Sakugawa", "Tokiwadai"]);
    }

    /// The strikers' tab at the list's top: the team first, then 5 to 8.
    fn shown() -> Formation {
        let card = |student: i64, column: f32, row: f32| Card {
            student,
            middle: (1000.0 + column * 200.0, row),
        };
        Formation {
            front: Front {
                first: 100,
                specials: [201, 202],
            },
            tab: Tab::Striker,
            placed: [Some(1), Some(2), Some(3), Some(4), Some(201), Some(203)],
            cards: vec![
                card(1, 0.0, 462.0),
                card(2, 1.0, 462.0),
                card(3, 2.0, 462.0),
                card(4, 3.0, 462.0),
                card(5, 4.0, 462.0),
                card(6, 5.0, 462.0),
                card(7, 0.0, 744.0),
                card(8, 1.0, 744.0),
            ],
        }
    }

    #[test]
    fn the_cards_to_pick_and_to_take_out_are_marked() {
        // The attack keeps 1 and 2, and wants 8 (in sight) and 9 (not), with 201 and 204.
        let marks = for_formation(
            &shown(),
            Some([301, 302, 303]),
            Some([1, 2, 8, 9, 201, 204]),
            &[1, 2, 3, 4],
            &ranks(),
        );
        let guess = |student, x| Mark::Guess {
            student,
            middle: (x, 875.0),
            side: 116.0,
        };
        assert_eq!(
            marks,
            [
                guess(301, 365.0),
                guess(302, 498.0),
                guess(303, 631.0),
                // 3 and 4 leave the strikers, 203 the specials.
                Mark::Drop((455.5, 1026.5, 624.5, 1195.5)),
                Mark::Drop((645.5, 1026.5, 814.5, 1195.5)),
                Mark::Drop((1046.5, 1026.5, 1215.5, 1195.5)),
                // 8 is the second card of the second row; the list cuts the card's foot.
                Mark::Pick((1102.0, 654.0, 1298.0, 922.0)),
                Mark::Seek {
                    student: 9,
                    way: Way::Down,
                    middle: (2232.0, 918.0),
                },
            ]
        );
    }

    #[test]
    fn a_card_is_marked_within_the_list() {
        let mut low = shown();
        low.cards[7].middle.1 = 866.0;
        let marks = for_formation(&low, None, Some([1, 2, 3, 8, 201, 203]), &[], &ranks());
        assert_eq!(
            marks.last(),
            Some(&Mark::Pick((1102.0, 776.0, 1298.0, 962.0))),
            "cut at the list's foot"
        );
    }

    #[test]
    fn the_other_tab_is_marked_once_this_one_is_done() {
        let attack = Some([1, 2, 3, 4, 201, 204]);
        let marks = for_formation(&shown(), None, attack, &[1, 2, 3, 4], &ranks());
        assert_eq!(
            marks,
            [
                Mark::Drop((1046.5, 1026.5, 1215.5, 1195.5)),
                Mark::Pick((1256.0, 259.0, 1472.0, 330.0)),
            ],
            "the strikers stand: the specials' tab"
        );
        // On the specials' tab, the special to pick is a card or a student to seek.
        let specials = Formation {
            tab: Tab::Special,
            cards: Vec::new(),
            ..shown()
        };
        let marks = for_formation(&specials, None, attack, &[201, 203], &ranks());
        assert_eq!(
            marks[1],
            Mark::Seek {
                student: 204,
                way: Way::Either,
                middle: (2232.0, 918.0),
            }
        );
    }

    #[test]
    fn nothing_is_marked_without_counsel() {
        assert_eq!(for_formation(&shown(), None, None, &[], &ranks()), []);
        let chosen = Some([1, 2, 3, 4, 201, 203]);
        assert_eq!(
            for_formation(&shown(), None, chosen, &[], &ranks()),
            [],
            "the team stands as counselled"
        );
    }

    #[test]
    fn the_strikers_expected_cover_the_opponents_hidden_cards() {
        assert_eq!(
            for_opponent([301, 302, 303])[2],
            Mark::Guess {
                student: 303,
                middle: (1406.5, 381.0),
                side: 101.0,
            }
        );
    }

    #[test]
    fn the_team_entered_with_stays_while_the_dialog_is_open() {
        let (start, mut visit) = (Instant::now(), Visit::default());
        let team = |third| [Some(1), Some(2), third, None, None, None];
        assert_eq!(visit.enter(&team(Some(3)), start), [1, 2, 3]);
        let soon = start + Duration::from_secs(1);
        assert_eq!(visit.enter(&team(None), soon), [1, 2, 3], "one taken out");
        let later = soon + Duration::from_secs(60);
        assert_eq!(visit.enter(&team(Some(9)), later), [1, 2, 9], "opened anew");
    }

    #[test]
    fn a_record_says_where_it_stands_beside_its_title() {
        let [Mark::Say { words, at, tall }] = for_record(Words::Kept)[..] else {
            panic!("one mark of words");
        };
        assert_eq!(words, Words::Kept);
        assert!(at.0 > 1292.0, "right of the title, which ends at 1292");
        assert!((200.0..=250.0).contains(&at.1), "on the title's line");
        assert!(tall <= 60.0, "within the record's head");
    }

    #[test]
    fn the_rows_say_whether_their_records_are_read() {
        // Two rows in sight: the upper read, the lower not.
        let marks = for_list(&[(398.0, true), (573.0, false)], true);
        let said: Vec<(Words, f32)> = marks
            .iter()
            .filter_map(|mark| match mark {
                Mark::Say { words, at, .. } => Some((*words, at.1)),
                _ => None,
            })
            .collect();
        assert_eq!(said.len(), 2);
        assert_eq!((said[0].0, said[1].0), (Words::Read, Words::Unread));
        assert!((398.0..440.0).contains(&said[0].1), "at the row's top");
        assert!((573.0..615.0).contains(&said[1].1));
        // Only the row to open has its button framed.
        let framed: Vec<_> = marks
            .iter()
            .filter_map(|mark| match mark {
                Mark::Pick(area) => Some(*area),
                _ => None,
            })
            .collect();
        assert_eq!(framed.len(), 1);
        assert!(
            framed[0].1 >= 573.0 && framed[0].3 <= 573.0 + 165.0,
            "in the lower row"
        );
        assert!(for_list(&[], true).is_empty());
    }

    #[test]
    fn a_list_up_before_rio_looked_says_to_open_it_anew() {
        // A row kept before is still told; beside the title the list says what to do.
        let marks = for_list(&[(398.0, true)], false);
        let said: Vec<(Words, (f32, f32))> = marks
            .iter()
            .filter_map(|mark| match mark {
                Mark::Say { words, at, .. } => Some((*words, *at)),
                _ => None,
            })
            .collect();
        assert_eq!(said.len(), 2);
        assert_eq!(said[0].0, Words::Reopen);
        assert!(
            said[0].1.0 > 1291.0,
            "right of the title, which ends at 1291"
        );
        assert!(
            (298.0..=348.0).contains(&said[0].1.1),
            "on the title's line"
        );
        assert_eq!(said[1].0, Words::Read);
        assert_eq!(for_list(&[], false).len(), 1, "with no row told as well");
    }
}
