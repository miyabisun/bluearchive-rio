//! What Rio marks over the game, on the measured screen (see `history`): the strikers the
//! user takes to stand where the game shows "?", and in the formation dialog the cards to
//! take out and the card to pick next for the attack the user chose. A card picked takes the
//! leftmost empty place, so the picks are told one at a time, each for that place; students
//! who stood elsewhere before are moved after the dialog, by dragging. Over a battle record it
//! says whether the record is kept, and over the list of records which rows' records are:
//! the user's eyes stay on the game.

use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use crate::{
    Squad, Student,
    aoi::Held,
    formation::{
        CARD_AROUND, COLUMNS, FRONT_SIDE, Formation, HIDDEN, LIST, TABS, TEAM, TEAM_SIDE, Tab, seat,
    },
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
    /// How many rows the list is to be scrolled to a student sought (see `rows_to`):
    /// written at the foot of the student's icon about `middle`.
    Rows { rows: u8, middle: (f32, f32) },
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

/// The unit the rows to a student sought are counted in, as the user reads it after the
/// number (`Mark::Rows`).
pub const ROWS_UNIT: &str = "列";

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

/// The side of a sought student's icon, and its middle near the list's top (to scroll up)
/// and above the OK button (to scroll down).
pub(crate) const SEEK_SIDE: f32 = 100.0;
const SEEK_UP: (f32, f32) = (2232.0, 455.0);
const SEEK_DOWN: (f32, f32) = (2232.0, 918.0);

/// The way to a student the list does not show, from the cards it shows (in its order),
/// the students known to stand `first` in it (those placed when the dialog opened, see
/// `Lead`) and the students' ranks.
pub fn way(student: i64, cards: &[i64], first: &[i64], rank: &HashMap<i64, Rank>) -> Way {
    // One of those standing first out of sight is above, and with one of them in sight
    // the list is at its top.
    if first.contains(&student) {
        return Way::Up;
    }
    if cards.iter().any(|card| first.contains(card)) {
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

/// What the list's order is told from: the ranks of the students held (see `ranked`), the
/// students that fight as specials (the snapshot's, held or not: the list shows one squad
/// a tab), and whether Aoi read the level, the stars and the bond of every student held,
/// each under an id. A student it did not stands in the list where the ranks do not tell,
/// and how far it is to another cannot be counted.
#[derive(Debug, Clone, Default)]
pub struct Roll {
    pub ranks: HashMap<i64, Rank>,
    pub specials: HashSet<i64>,
    pub whole: bool,
}

/// The roll of the students held, as Aoi read them, with the snapshot's students.
pub fn roll(held: &[Held], students: &[Student]) -> Roll {
    let ranks = ranked(held, students);
    Roll {
        whole: !held.is_empty() && ranks.len() == held.len(),
        ranks,
        specials: students
            .iter()
            .filter(|student| student.squad == Squad::Support)
            .map(|student| student.id)
            .collect(),
    }
}

/// A card in sight: its student, and its place in the list's grid, counted along the rows
/// from the first column of the first row in sight (the row times `COLUMNS`, and the column).
pub type Seat = (i64, usize);

/// The seats of the cards the dialog shows; none where a card lies off the list's grid.
pub fn seated(shown: &Formation) -> Option<Vec<Seat>> {
    let first = shown.cards.first();
    shown
        .cards
        .iter()
        .map(|card| {
            let (row, column) = seat(card, first?)?;
            Some((card.student, row * COLUMNS + column))
        })
        .collect()
}

/// What is known of the students a tab's list shows first, whatever their ranks: those of
/// the team as it stood when the dialog opened. `known` are the ones known to stand there,
/// `whole` tells that they are all of them. They are all known where Rio saw the dialog
/// open, or read the list's top since (`opened_with`); else none is (see `Visit`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lead {
    pub known: Vec<i64>,
    pub whole: bool,
}

impl Lead {
    /// These, and no other.
    pub fn all(known: &[i64]) -> Self {
        Self {
            known: known.to_vec(),
            whole: true,
        }
    }
}

/// How many students a tab's list shows first at most: the team's places of its squad.
fn most(tab: Tab) -> usize {
    match tab {
        Tab::Striker => 4,
        Tab::Special => 2,
    }
}

fn before(rank: &Rank, of: &Rank) -> bool {
    rank.against(of) == Some(Ordering::Less)
}

fn after(rank: &Rank, of: &Rank) -> bool {
    rank.against(of) == Some(Ordering::Greater)
}

/// The cards in sight, read as a stretch of a tab's list: the students standing first
/// (those the dialog opened with, in an order not known), then the others by their ranks.
/// A student standing first is missing at its rank's place, so each of the others stands
/// as many places on from its rank's as students standing first rank after it. A card's
/// column tells that count (four at most, in rows of six): measured on 2026-11-24, where
/// three of a team of four ranked after the twelve cards in sight and every card stood
/// three places on.
struct Stretch<'a> {
    /// How many students are known to stand first, and how many more may.
    first: usize,
    spare: usize,
    /// How many of those known are in sight: none, or all of them from the first place on.
    leading: usize,
    /// The others' cards in sight: each one's seat and rank.
    others: Vec<(usize, &'a Rank)>,
    /// The ranks of the others out of sight, but the student sought.
    unseen: Vec<&'a Rank>,
    /// For each two neighbours of `others`, the students out of sight whose ranks are
    /// between theirs, beyond the places open between the two cards: they stand first.
    gone: Vec<usize>,
}

impl<'a> Stretch<'a> {
    /// The cards at `seats` as a stretch of the list of `tab`, looked at for the student
    /// `sought`. `None` where they are none:
    /// - not every student held has a rank, or a card's student has none;
    /// - the cards of those known to stand first are in sight, but not all of them from
    ///   the list's first place on, side by side up to the first of the others, with no
    ///   student out of sight ranking before that one;
    /// - two of the others stand against their ranks, or the ranks do not put as many
    ///   students between them as places are open there (none between cards side by side;
    ///   a card not read leaves its place open), or do not tell how many; students beyond
    ///   the open places stand first, as far as more may than are known.
    fn read(
        sought: Option<i64>,
        seats: &[Seat],
        lead: &Lead,
        tab: Tab,
        roll: &'a Roll,
    ) -> Option<Self> {
        let start = seats.first()?.1;
        if !roll.whole || seats.windows(2).any(|pair| pair[1].1 <= pair[0].1) {
            return None;
        }
        let of_tab = |id: &i64| (tab == Tab::Special) == roll.specials.contains(id);
        let known: Vec<i64> = lead.known.iter().copied().filter(of_tab).collect();
        let spare = match lead.whole {
            true => 0,
            false => most(tab).checked_sub(known.len())?,
        };
        let seen = |id: &i64| seats.iter().any(|seat| seat.0 == *id);
        let other = |id: &i64| of_tab(id) && !known.contains(id);
        let unseen: Vec<&Rank> = roll
            .ranks
            .iter()
            .filter(|(id, _)| other(id) && !seen(id) && Some(**id) != sought)
            .map(|(_, rank)| rank)
            .collect();

        let leading = seats
            .iter()
            .take_while(|seat| known.contains(&seat.0))
            .count();
        if leading > 0 && (!lead.whole || leading != known.len() || start != 0) {
            return None;
        }
        let side_by_side = |pair: &[Seat]| pair[1].1 == pair[0].1 + 1;
        if !seats[..(leading + 1).min(seats.len())]
            .windows(2)
            .all(side_by_side)
        {
            return None;
        }
        let mut others = Vec::with_capacity(seats.len() - leading);
        for seat in &seats[leading..] {
            let rank = roll.ranks.get(&seat.0).filter(|_| other(&seat.0))?;
            others.push((seat.1, rank));
        }
        let mut gone = Vec::with_capacity(others.len());
        for pair in others.windows(2) {
            let ((place, card), (next_place, next)) = (pair[0], pair[1]);
            let open = next_place - place - 1;
            let between = |rank: &&&Rank| after(rank, card) && before(rank, next);
            let maybe = |rank: &&&Rank| !before(rank, card) && !after(rank, next);
            let surely = unseen.iter().filter(between).count();
            let may = unseen.iter().filter(maybe).count();
            if after(card, next) || surely < open || (surely > 0 && may != surely) {
                return None;
            }
            gone.push(surely - open);
        }
        if gone.iter().sum::<usize>() > spare {
            return None;
        }
        if let (true, Some(&(_, card))) = (leading > 0, others.first())
            && unseen.iter().any(|rank| before(rank, card))
        {
            return None;
        }
        Some(Self {
            first: known.len(),
            spare,
            leading,
            others,
            unseen,
            gone,
        })
    }

    /// How many of the students out of sight stand where `sure` tells for sure, and how
    /// many may stand there.
    fn count(&self, sure: impl Fn(&Rank) -> bool, may: impl Fn(&Rank) -> bool) -> (usize, usize) {
        let surely = self.unseen.iter().filter(|rank| sure(rank)).count();
        (surely, self.unseen.iter().filter(|rank| may(rank)).count())
    }

    /// The students standing first, not known and out of sight, who rank after the other
    /// at `index`, as its card's column tells: the card's place in the whole list is the
    /// count of those known to stand first, of the others before it (those in sight, the
    /// `sought` one if it stands before, and those out of sight the ranks put there), and
    /// of these. `Some(None)` where the ranks leave open how many stand before it; `None`
    /// where the column would have more stand first than may.
    fn over(&self, index: usize, sought: usize) -> Option<Option<usize>> {
        let (place, card) = self.others[index];
        let (surely, may) = self.count(|rank| before(rank, card), |rank| !after(rank, card));
        if surely != may {
            return Some(None);
        }
        let ahead = self.first + index + sought + surely;
        let over = (place + COLUMNS - ahead % COLUMNS) % COLUMNS;
        (over <= self.spare).then_some(Some(over))
    }

    /// Those of `over` for the last of the others. Every card whose place the ranks tell
    /// must tell the same, and one at least must; where all standing first are known,
    /// the card at `index` alone is asked, and it may leave its place open.
    fn over_last(&self, index: usize, sought: usize) -> Option<usize> {
        if self.spare == 0 {
            return self.over(index, sought).map(|_| 0);
        }
        let mut told = None;
        for at in 0..self.others.len() {
            if let Some(over) = self.over(at, sought)? {
                let last = over.checked_sub(self.gone[at..].iter().sum())?;
                if told.is_some_and(|told| told != last) {
                    return None;
                }
                told = Some(last);
            }
        }
        told
    }

    /// Those of `over` for the first of the others: the last one's, and those who rank
    /// between two cards in sight.
    fn over_first(&self, sought: usize) -> Option<usize> {
        let over = self.over_last(0, sought)? + self.gone.iter().sum::<usize>();
        (over <= self.spare).then_some(over)
    }
}

/// How many rows the list is to be scrolled for `student`, whom it does not show: the rows
/// from the last row in sight down to the student's (`Way::Down`), or from the first up to
/// it (`Way::Up`); the row next to the one in sight is 1.
///
/// The cards in sight are taken for a stretch of the tab's list (see `Stretch`), and the
/// student's place is counted from the card at its end. Where not all standing first are
/// known (`lead`), the student sought is taken for one of the others unless known to stand
/// first, and the count allows for those who may: of the students the ranks put between
/// the end card and the student, as many may be missing as stand first by the card's
/// column (downwards: those ranking after the last card; upwards: those the team has
/// places left for). `None` wherever the count is not sure:
/// - the way is not told, the student has no rank, or the cards are no stretch of the list;
/// - the end card stands in another column than its place in the list has (told from the
///   ranks where all standing first are known, else it tells how many do);
/// - by one count the student stands in one row and by another in the next: students the
///   ranks do not order against it or the end card, or who may stand first.
pub fn rows_to(
    student: i64,
    way: Way,
    seats: &[Seat],
    lead: &Lead,
    tab: Tab,
    roll: &Roll,
) -> Option<u8> {
    let stretch = Stretch::read(Some(student), seats, lead, tab, roll)?;
    let (start, end) = (seats.first()?.1, seats.last()?.1);
    // The rows the two counts give, where they give the same and it is not the end row.
    let told = |rows: (usize, usize)| {
        (rows.0 == rows.1 && rows.0 > 0)
            .then(|| u8::try_from(rows.0).ok())
            .flatten()
    };
    // The first card in sight, the first of its row, with every row above it full.
    let top = || {
        let first = stretch.others.first();
        first.filter(|_| stretch.leading == 0 && start == 0)
    };
    let of_tab = (tab == Tab::Special) == roll.specials.contains(&student);

    if of_tab && lead.known.contains(&student) {
        // The first row, from the first card in sight.
        let &(_, card) = top()?;
        if way != Way::Up {
            return None;
        }
        let ahead = stretch.first + stretch.over_first(0)?;
        let (surely, may) = stretch.count(|rank| before(rank, card), |rank| !after(rank, card));
        return told(((ahead + surely) / COLUMNS, (ahead + may) / COLUMNS));
    }
    let own = roll.ranks.get(&student).filter(|_| of_tab)?;
    match way {
        Way::Down => {
            // From the last card in sight on, to the student's row.
            let (surely, may, over) = match stretch.others.last() {
                Some(&(_, card)) => {
                    if before(own, card) {
                        return None;
                    }
                    let over = stretch.over_last(stretch.others.len() - 1, 0)?;
                    let (surely, may) = stretch.count(
                        |rank| after(rank, card) && before(rank, own),
                        |rank| !before(rank, card) && !after(rank, own),
                    );
                    (surely, may, over)
                }
                // Only the cards of those standing first in sight: every other follows.
                None => {
                    let (surely, may) =
                        stretch.count(|rank| before(rank, own), |rank| !after(rank, own));
                    (surely, may, 0)
                }
            };
            // Of the students between, those who stand first are not there.
            let (column, least) = (end % COLUMNS, surely - over.min(surely));
            told(((column + least + 1) / COLUMNS, (column + may + 1) / COLUMNS))
        }
        Way::Up => {
            // Back from the first card in sight, over full rows.
            let &(_, card) = top()?;
            if after(own, card) {
                return None;
            }
            let over = stretch.over_first(1)?;
            let (surely, may) = stretch.count(
                |rank| after(rank, own) && before(rank, card),
                |rank| !before(rank, own) && !after(rank, card),
            );
            let least = surely - (stretch.spare - over).min(surely);
            told(((least + COLUMNS) / COLUMNS, (may + COLUMNS) / COLUMNS))
        }
        Way::Either => None,
    }
}

/// The students a tab's list shows first, read off its top: the fewest cards from the
/// first place on (up to the team's places) after which the cards in sight are the others
/// by their ranks, from the first of them all. `None` where the cards are not the list's
/// top (a student out of sight may stand before the first of the others), or do not tell
/// (a card among the first not read, cards against the ranks).
pub fn opened_with(seats: &[Seat], tab: Tab, roll: &Roll) -> Option<Vec<i64>> {
    (0..=most(tab)).find_map(|count| {
        let first: Vec<i64> = seats.get(..count)?.iter().map(|seat| seat.0).collect();
        let stretch = Stretch::read(None, seats, &Lead::all(&first), tab, roll)?;
        let &(place, card) = stretch.others.first()?;
        let follow = stretch.unseen.iter().all(|rank| after(rank, card));
        (stretch.leading == count && place == count && follow).then_some(first)
    })
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

/// The student to pick next into the open tab's `places` of the team, for which the attack
/// has the students `wanted`, place by place. The game puts a card picked into the leftmost
/// empty place (the user's account of 2026-11-24, not measured), so the student the attack
/// has for that place lands where the attack has it, whatever is taken out before or
/// after. With no place empty, the leftmost one to be emptied counts. Where that place's
/// student stands elsewhere in the team, it is the first of the attack's not placed.
/// `None` once all of them stand.
fn next(wanted: &[i64], places: &[Option<i64>]) -> Option<i64> {
    let stands = |student: &&i64| places.contains(&Some(**student));
    let leaves = |place: &Option<i64>| place.is_some_and(|student| !wanted.contains(&student));
    let place = places
        .iter()
        .position(Option::is_none)
        .or_else(|| places.iter().position(leaves));
    place
        .and_then(|place| wanted.get(place))
        .filter(|student| !stands(student))
        .or_else(|| wanted.iter().find(|student| !stands(student)))
        .copied()
}

/// The marks over the formation dialog: the strikers the user takes to stand behind the
/// front, and for the `attack` chosen (A1 to A4, then the specials) the cards to take out of the
/// team, the one student of the open tab to pick next (see `next`): the card, or out of
/// sight the way to scroll (with how many rows away, where `roll` and what is known of the
/// `lead` of the tab's list tell it for sure), and the other tab once this one is done.
pub fn for_formation(
    shown: &Formation,
    behind: Option<[i64; 3]>,
    attack: Option<[i64; 6]>,
    lead: &Lead,
    roll: &Roll,
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
    let (wanted, places, others, other_tab) = match shown.tab {
        Tab::Striker => (strikers, &shown.placed[..4], specials, TABS[1]),
        Tab::Special => (specials, &shown.placed[4..], strikers, TABS[0]),
    };
    let in_sight: Vec<i64> = shown.cards.iter().map(|card| card.student).collect();
    let seats = seated(shown);
    let student = next(wanted, places);
    let card = student.and_then(|student| shown.cards.iter().find(|card| card.student == student));
    if let Some(card) = card {
        let ((x, y), (side, up, down)) = (card.middle, CARD_AROUND);
        marks.push(Mark::Pick((
            x - side,
            (y - up).max(LIST.0),
            x + side,
            (y + down).min(LIST.1),
        )));
    } else if let Some(student) = student {
        let way = way(student, &in_sight, &lead.known, &roll.ranks);
        let middle = match way {
            Way::Up => SEEK_UP,
            Way::Down | Way::Either => SEEK_DOWN,
        };
        marks.push(Mark::Seek {
            student,
            way,
            middle,
        });
        let rows = seats
            .as_deref()
            .and_then(|seats| rows_to(student, way, seats, lead, shown.tab, roll));
        if let Some(rows) = rows {
            marks.push(Mark::Rows { rows, middle });
        }
    }
    if wanted.iter().all(placed) && !others.iter().all(placed) {
        marks.push(Mark::Pick(other_tab));
    }
    marks
}

/// The dialog is taken to have closed when it was not read for this long, and to have
/// opened under Rio's eyes when frames without it were read for this long before it.
const AWAY: Duration = Duration::from_secs(2);

/// What is known of the team as it stood when the formation dialog opened: each tab's
/// list shows its students of that team first, whatever is picked afterwards. A dialog
/// that Rio saw open has the team it showed then. One that was up before Rio looked
/// (Rio started, or read nothing for a while) has a team not known: it is read off a
/// tab's list once the list's top is in sight (`opened_with`), and until then the rows are
/// counted with whoever may stand first (`rows_to`).
#[derive(Default)]
pub struct Visit {
    /// The strikers' and the specials' of that team.
    leads: [Lead; 2],
    /// When the dialog was read last, and since when frames without it were read after.
    seen: Option<Instant>,
    apart: Option<Instant>,
}

impl Visit {
    /// A frame read `now` does not show the dialog.
    pub fn away(&mut self, now: Instant) {
        self.apart.get_or_insert(now);
    }

    /// What is known of the students the open tab's list shows first, given a reading of
    /// the dialog `now` and the roll its list is ordered by.
    pub fn enter(&mut self, shown: &Formation, roll: &Roll, now: Instant) -> &Lead {
        if self.seen.is_none_or(|seen| now.duration_since(seen) > AWAY) {
            let (strikers, specials) = shown.placed.split_at(4);
            let team = |placed: &[Option<i64>]| {
                Lead::all(&placed.iter().flatten().copied().collect::<Vec<_>>())
            };
            let opened = |since| now.duration_since(since) > AWAY;
            self.leads = match self.apart.is_some_and(opened) {
                true => [team(strikers), team(specials)],
                false => [Lead::default(), Lead::default()],
            };
        }
        (self.seen, self.apart) = (Some(now), None);
        let lead = &mut self.leads[usize::from(shown.tab == Tab::Special)];
        if !lead.whole
            && let Some(first) =
                seated(shown).and_then(|seats| opened_with(&seats, shown.tab, roll))
        {
            *lead = Lead::all(&first);
        }
        lead
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        time::{Duration, Instant},
    };

    use super::{
        Lead, Mark, Rank, Roll, Seat, Visit, Way, Words, for_formation, for_list, for_opponent,
        for_record, names, next, opened_with, ranked, roll, rows_to, school_place, way,
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

    /// A list of the strikers' tab for the tests of the rows: 1 to 4 entered with, then 10
    /// to 39 in the order of their bonds (each a bond lower than the one before); 201 and
    /// 202 are specials. Its rows: 1 2 3 4 10 11 / 12..17 / 18..23 / 24..29 / 30..35 / 36..39.
    fn listed() -> Roll {
        let mut ranks: HashMap<i64, Rank> = (1..=4).map(|id| (id, rank(90, 5, 99))).collect();
        ranks.extend((10..40).map(|id| (id, rank(90, 5, 90 - id as u8))));
        ranks.extend([(201, rank(90, 5, 50)), (202, rank(90, 5, 40))]);
        Roll {
            ranks,
            specials: [201, 202].into(),
            whole: true,
        }
    }

    /// The cards of whole rows in sight, each with its place in the grid: the students
    /// `from..=to`, the first in the first column.
    fn seats(from: i64, to: i64) -> Vec<Seat> {
        (from..=to).zip(0..).collect()
    }

    const ENTERED: [i64; 6] = [1, 2, 3, 4, 201, 202];

    /// The team the dialog opened with, all of it known.
    fn opened() -> Lead {
        Lead::all(&ENTERED)
    }

    /// The list of `listed`, its team of ranks apart: by rank 1 stands before every other
    /// student as well, 2 to 4 after them all. The list's rows are the same.
    fn apart() -> Roll {
        let mut roll = listed();
        for (id, bond) in [(2, 30), (3, 29), (4, 28)] {
            roll.ranks.insert(id, rank(90, 5, bond));
        }
        roll
    }

    /// Cards on the measured grid, row by row from the first column of a row `down` rows
    /// below the list's first one in sight.
    fn grid(students: impl IntoIterator<Item = i64>) -> Vec<Card> {
        students
            .into_iter()
            .zip(0..)
            .map(|(student, place): (i64, usize)| Card {
                student,
                middle: (
                    1144.0 + (place % 6) as f32 * 205.7,
                    584.0 + (place / 6) as f32 * 282.0,
                ),
            })
            .collect()
    }

    /// With nothing known of the team the dialog opened with, a card's column still tells
    /// how many students stand first whose rank is after the card's: here 2, 3 and 4. They
    /// may rank between the last card and the student sought, or after it: the rows are
    /// told where both give the same row.
    #[test]
    fn the_rows_below_are_told_without_the_team_the_dialog_opened_with() {
        let (roll, seen, nothing) = (apart(), seats(12, 23), Lead::default());
        let rows = |student| rows_to(student, Way::Down, &seen, &nothing, Tab::Striker, &roll);
        assert_eq!(rows(24), Some(1), "the next card");
        assert_eq!(
            rows(29),
            Some(1),
            "the last of the next row, whoever stands first"
        );
        assert_eq!(rows(30), None, "the next row, or this one's end");
        assert_eq!(rows(32), None);
        assert_eq!(rows(33), Some(2), "past the three that may stand first");
        assert_eq!(rows(35), Some(2));
        assert_eq!(rows(36), None);
        assert_eq!(rows(39), Some(3));
        // The team known, every one of them is told (as before).
        let known = |student| rows_to(student, Way::Down, &seen, &opened(), Tab::Striker, &roll);
        assert_eq!(
            (known(30), known(32), known(36)),
            (Some(2), Some(2), Some(3))
        );
        // One of the three known to stand first leaves two that may.
        let partly = Lead {
            known: vec![2],
            whole: false,
        };
        let rows = |student| rows_to(student, Way::Down, &seen, &partly, Tab::Striker, &roll);
        assert_eq!((rows(31), rows(32)), (None, Some(2)));
    }

    /// Upwards the first card's column tells the three whose rank is after it; of the four
    /// places one is left, for a student whose rank is before it.
    #[test]
    fn the_rows_above_are_told_without_the_team_the_dialog_opened_with() {
        let (roll, seen, nothing) = (apart(), seats(24, 35), Lead::default());
        let rows = |student| rows_to(student, Way::Up, &seen, &nothing, Tab::Striker, &roll);
        assert_eq!(rows(23), Some(1));
        assert_eq!(
            rows(18),
            Some(1),
            "the first of the row above, or its second"
        );
        assert_eq!(rows(17), None, "the row above, or the one before it");
        assert_eq!(rows(16), Some(2));
        assert_eq!(rows(12), Some(2));
        assert_eq!(rows(11), None);
        assert_eq!(rows(10), Some(3));
        // A student known to stand first is in the first row: three rows up.
        let partly = Lead {
            known: vec![2],
            whole: false,
        };
        assert_eq!(
            rows_to(2, Way::Up, &seen, &partly, Tab::Striker, &roll),
            Some(3)
        );
    }

    /// A column that would have more students stand first than the team has places tells
    /// that the cards are not the ranks' stretch.
    #[test]
    fn no_rows_are_told_where_more_would_stand_first_than_the_team_holds() {
        let (roll, nothing) = (apart(), Lead::default());
        // Seven students rank before 16 (1, and 10 to 15): in the first column it would
        // have five more before it.
        assert_eq!(
            rows_to(33, Way::Down, &seats(16, 27), &nothing, Tab::Striker, &roll),
            None
        );
        // One student between two cards side by side stands first; five do not.
        let one: Vec<Seat> = (12..=24).filter(|id| *id != 15).zip(0..).collect();
        assert_eq!(
            rows_to(39, Way::Down, &one, &nothing, Tab::Striker, &roll),
            Some(3)
        );
        let five: Vec<Seat> = (12..=28)
            .filter(|id| !(15..20).contains(id))
            .zip(0..)
            .collect();
        assert_eq!(
            rows_to(39, Way::Down, &five, &nothing, Tab::Striker, &roll),
            None
        );
    }

    /// The list's top shows who it opened with: the cards before the first one that every
    /// student out of sight follows by rank.
    #[test]
    fn the_team_the_dialog_opened_with_is_read_off_the_lists_top() {
        let roll = apart();
        let top: Vec<Seat> = [1, 2, 3, 4, 10, 11, 12, 13, 14, 15, 16, 17]
            .into_iter()
            .zip(0..)
            .collect();
        assert_eq!(
            opened_with(&top, Tab::Striker, &roll),
            Some(vec![1, 2, 3, 4])
        );
        // A team of none: the list is the ranks' order from its first card on.
        let mut plain = apart();
        for id in 2..=4 {
            plain.ranks.remove(&id);
        }
        let top: Vec<Seat> = [1].into_iter().chain(10..=20).zip(0..).collect();
        assert_eq!(opened_with(&top, Tab::Striker, &plain), Some(Vec::new()));
        // Not the top: students out of sight rank before the first card.
        assert_eq!(opened_with(&seats(12, 23), Tab::Striker, &roll), None);
        assert_eq!(opened_with(&seats(18, 29), Tab::Striker, &plain), None);
        // A card of the first places not read: who stands there is not known.
        let holed: Vec<Seat> = [1, 2, 4, 10, 11, 12]
            .into_iter()
            .zip([0, 1, 3, 4, 5, 6])
            .collect();
        assert_eq!(opened_with(&holed, Tab::Striker, &roll), None);
        // The specials' list opens with two at most.
        let mut specials = listed();
        specials
            .ranks
            .extend((210..224).map(|id| (id, rank(90, 5, (250 - id) as u8))));
        specials.specials.extend(210..224);
        specials.ranks.insert(201, rank(90, 5, 1));
        specials.ranks.insert(202, rank(90, 5, 60));
        let top: Vec<Seat> = [201, 202, 210, 211, 212, 213]
            .into_iter()
            .zip(0..)
            .collect();
        assert_eq!(
            opened_with(&top, Tab::Special, &specials),
            Some(vec![201]),
            "202 ranks first of the others as well"
        );
    }

    #[test]
    fn the_rows_to_a_student_below_are_counted_from_the_last_card() {
        let (roll, seen) = (listed(), seats(12, 23));
        let rows = |student| rows_to(student, Way::Down, &seen, &opened(), Tab::Striker, &roll);
        assert_eq!(rows(24), Some(1), "the first card of the next row");
        assert_eq!(rows(29), Some(1), "the last card of the next row");
        assert_eq!(rows(30), Some(2), "the row after");
        assert_eq!(rows(39), Some(3), "the list's last card");
        // The list at its top: the team's four, then the others.
        let top: Vec<Seat> = [1, 2, 3, 4, 10, 11, 12, 13, 14, 15, 16, 17]
            .into_iter()
            .zip(0..)
            .collect();
        let rows = |student| rows_to(student, Way::Down, &top, &opened(), Tab::Striker, &roll);
        assert_eq!(rows(18), Some(1));
        assert_eq!(rows(36), Some(4));
    }

    #[test]
    fn the_rows_to_a_student_above_are_counted_from_the_first_card() {
        let (roll, seen) = (listed(), seats(24, 35));
        let rows = |student| rows_to(student, Way::Up, &seen, &opened(), Tab::Striker, &roll);
        assert_eq!(rows(23), Some(1), "the last card of the row before");
        assert_eq!(rows(18), Some(1), "the first card of the row before");
        assert_eq!(rows(17), Some(2));
        assert_eq!(rows(11), Some(3), "the first row, after the team");
        // A student entered with stands in the first row, wherever in it.
        assert_eq!(rows(2), Some(3));
        assert_eq!(rows(4), Some(3));
    }

    #[test]
    fn the_specials_are_counted_among_themselves() {
        let mut roll = listed();
        // Fourteen specials more, in the order of their bonds: 201 202 210..213 / 214..219 / 220..223.
        roll.ranks
            .extend((210..224).map(|id| (id, rank(90, 5, (250 - id) as u8))));
        roll.specials.extend(210..224);
        let seen: Vec<Seat> = [201, 202, 210, 211, 212, 213]
            .into_iter()
            .zip(0..)
            .collect();
        let rows = |student| rows_to(student, Way::Down, &seen, &opened(), Tab::Special, &roll);
        assert_eq!(rows(214), Some(1));
        assert_eq!(rows(220), Some(2));
        assert_eq!(rows(24), None, "a striker is not of this tab");
    }

    /// Where the way is not told, a card in sight has no rank, or Aoi has not read every
    /// student held, no number is told: a number is right, or there is none.
    #[test]
    fn no_rows_are_told_from_what_is_not_known() {
        let (roll, seen) = (listed(), seats(12, 23));
        let striker = Tab::Striker;
        assert_eq!(
            rows_to(24, Way::Either, &seen, &opened(), striker, &roll),
            None
        );
        assert_eq!(
            rows_to(77, Way::Down, &seen, &opened(), striker, &roll),
            None,
            "no rank"
        );
        assert_eq!(
            rows_to(24, Way::Down, &[], &opened(), striker, &roll),
            None,
            "no card read"
        );
        // A card in sight that Aoi's roster lacks.
        let mut strange = seen.clone();
        strange[3].0 = 77;
        assert_eq!(
            rows_to(24, Way::Down, &strange, &opened(), striker, &roll),
            None
        );
        // A student held whose level, stars or bond Aoi has not read stands somewhere.
        let partly = Roll {
            whole: false,
            ..listed()
        };
        assert_eq!(
            rows_to(24, Way::Down, &seen, &opened(), striker, &partly),
            None
        );
    }

    /// The cards in sight must be the stretch of the list the ranks tell: a student the
    /// ranks put between two cards side by side, a first card that is
    /// not the first of its row, or a column the ranks do not give the card, tell that they
    /// are not.
    #[test]
    fn no_rows_are_told_where_the_cards_are_not_the_ranks_stretch() {
        let roll = listed();
        let striker = Tab::Striker;
        // 15 is not in the list (the cards close up): the ranks put it between 14 and 16.
        let closed: Vec<Seat> = (12..=24).filter(|id| *id != 15).zip(0..).collect();
        assert_eq!(
            rows_to(30, Way::Down, &closed, &opened(), striker, &roll),
            None
        );
        // The ranks put 13 in the second column of its row, the screen in the first.
        let shifted = seats(13, 24);
        assert_eq!(
            rows_to(30, Way::Down, &shifted, &opened(), striker, &roll),
            None
        );
        assert_eq!(
            rows_to(12, Way::Up, &shifted, &opened(), striker, &roll),
            None
        );
        // The last row in sight is not read to its end: the student may stand in it.
        assert_eq!(
            rows_to(17, Way::Down, &seats(12, 16), &opened(), striker, &roll),
            None
        );
        // The first card read is not the first of its row.
        let late: Vec<Seat> = (25..=35).zip(1..).collect();
        assert_eq!(rows_to(23, Way::Up, &late, &opened(), striker, &roll), None);
        // The team's cards in sight, but not all four of them.
        let cut: Vec<Seat> = [1, 2, 3, 10, 11, 12].into_iter().zip(0..).collect();
        assert_eq!(
            rows_to(18, Way::Down, &cut, &opened(), striker, &roll),
            None
        );
    }

    /// A card that was not read leaves its place open among the cards in sight (on real
    /// screens of 2026-11-24 two faces of 102 did not read). Where the ranks put exactly as
    /// many students between its neighbours as places are open, the cards are still the
    /// ranks' stretch and the rows are told; where they put fewer, more, or may put one,
    /// they are not.
    #[test]
    fn a_card_not_read_is_the_student_the_ranks_put_in_its_place() {
        let roll = listed();
        let striker = Tab::Striker;
        let without = |from: i64, to: i64, missing: &[i64]| -> Vec<Seat> {
            seats(from, to)
                .into_iter()
                .filter(|seat| !missing.contains(&seat.0))
                .collect()
        };
        // The card of 15 was not read: its place is open, and the ranks put 15 there.
        let open = without(12, 23, &[15]);
        assert_eq!(
            rows_to(24, Way::Down, &open, &opened(), striker, &roll),
            Some(1)
        );
        assert_eq!(
            rows_to(30, Way::Down, &open, &opened(), striker, &roll),
            Some(2)
        );
        let open = without(24, 35, &[27, 28]);
        assert_eq!(
            rows_to(11, Way::Up, &open, &opened(), striker, &roll),
            Some(3)
        );
        // The last card in sight was not read: the count runs from the last one read.
        let open = without(12, 23, &[23]);
        assert_eq!(
            rows_to(24, Way::Down, &open, &opened(), striker, &roll),
            Some(1)
        );
        assert_eq!(
            rows_to(23, Way::Down, &open, &opened(), striker, &roll),
            None,
            "in the end row"
        );
        // A place open where the ranks put no student, or two.
        let none: Vec<Seat> = (12..=22).zip([0, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11]).collect();
        assert_eq!(
            rows_to(30, Way::Down, &none, &opened(), striker, &roll),
            None
        );
        let two: Vec<Seat> = [12, 13, 14, 17, 18, 19, 20, 21, 22, 23, 24]
            .into_iter()
            .zip([0, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11])
            .collect();
        assert_eq!(
            rows_to(30, Way::Down, &two, &opened(), striker, &roll),
            None
        );
        // The ranks do not tell whether 16 stands in the open place or after it.
        let mut unsure = listed();
        let rank = unsure.ranks[&15].clone();
        unsure.ranks.insert(16, rank);
        let open = without(12, 23, &[15, 16]);
        assert_eq!(
            rows_to(24, Way::Down, &open, &opened(), striker, &unsure),
            Some(1)
        );
        let open = without(12, 23, &[15]);
        assert_eq!(
            rows_to(24, Way::Down, &open, &opened(), striker, &unsure),
            None
        );
        // A card of the team not read: who stands where among them is not known.
        let team: Vec<Seat> = [1, 2, 4, 10, 11].into_iter().zip([0, 1, 3, 4, 5]).collect();
        assert_eq!(
            rows_to(18, Way::Down, &team, &opened(), striker, &roll),
            None
        );
        // Places that do not rise are no stretch at all.
        let back: Vec<Seat> = vec![(12, 0), (13, 1), (14, 1)];
        assert_eq!(
            rows_to(30, Way::Down, &back, &opened(), striker, &roll),
            None
        );
    }

    /// Two students the order does not tell apart (alike in level, stars and bond, of no
    /// known school) leave the count open by one: told where both counts give the same
    /// row, not where the row's end lies between them.
    #[test]
    fn students_the_order_does_not_tell_apart_leave_a_row_open_only_at_its_end() {
        let striker = Tab::Striker;
        let alike = |a: i64, b: i64| {
            let mut roll = listed();
            let rank = roll.ranks[&a].clone();
            roll.ranks.insert(b, rank);
            roll
        };
        let seen = seats(12, 23);
        // 24 and 25 stand in either order, both in the next row.
        let roll = alike(24, 25);
        assert_eq!(
            rows_to(24, Way::Down, &seen, &opened(), striker, &roll),
            Some(1)
        );
        assert_eq!(
            rows_to(26, Way::Down, &seen, &opened(), striker, &roll),
            Some(1)
        );
        // 29 and 30 stand in either order: the last of one row and the first of the next.
        let roll = alike(29, 30);
        assert_eq!(
            rows_to(29, Way::Down, &seen, &opened(), striker, &roll),
            None
        );
        assert_eq!(
            rows_to(30, Way::Down, &seen, &opened(), striker, &roll),
            None
        );
        assert_eq!(
            rows_to(31, Way::Down, &seen, &opened(), striker, &roll),
            Some(2)
        );
        // The same upwards: 17 and 18 across the rows' bound, 19 and 20 within a row.
        let seen = seats(24, 35);
        let roll = alike(17, 18);
        assert_eq!(rows_to(18, Way::Up, &seen, &opened(), striker, &roll), None);
        assert_eq!(
            rows_to(16, Way::Up, &seen, &opened(), striker, &roll),
            Some(2)
        );
        let roll = alike(19, 20);
        assert_eq!(
            rows_to(19, Way::Up, &seen, &opened(), striker, &roll),
            Some(1)
        );
    }

    /// The roll is Aoi's reading joined with the snapshot: the ranks, the snapshot's
    /// specials, and whether every student held has an id and all three of its rank.
    #[test]
    fn the_roll_tells_whether_every_student_held_has_a_rank() {
        let held = |id: Option<i64>, level: Option<u8>| crate::aoi::Held {
            id,
            name: "名簿の名前".to_owned(),
            level,
            rarity: Some(5),
            bond: Some(30),
        };
        let students = crate::students();
        let special = students
            .iter()
            .find(|student| student.squad == crate::Squad::Support)
            .unwrap()
            .id;
        let all = roll(
            &[held(Some(10005), Some(90)), held(Some(special), Some(80))],
            &students,
        );
        assert!(all.whole);
        assert_eq!(all.ranks.len(), 2);
        assert!(all.specials.contains(&special) && !all.specials.contains(&10005));
        assert!(
            !roll(&[held(Some(10005), None)], &students).whole,
            "a level not read"
        );
        assert!(
            !roll(&[held(None, Some(90))], &students).whole,
            "a student of no id"
        );
        assert!(!roll(&[], &students).whole, "no roster at all");
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

    /// A roll of these ranks alone: no special among them, every student read.
    fn told(ranks: HashMap<i64, Rank>) -> Roll {
        Roll {
            ranks,
            specials: std::collections::HashSet::new(),
            whole: true,
        }
    }

    /// Beside the way to a student out of sight stands how many rows away the student is,
    /// where the cards in sight lie on the list's grid and the count is sure.
    #[test]
    fn the_rows_to_a_student_out_of_sight_are_marked_with_the_way() {
        // The list of `listed`, scrolled to its second and third rows (12 to 23), where the
        // measured list has its columns (1144, 205.7 apart) and two rows in sight.
        let cards = (12..24)
            .zip(0..)
            .map(|(student, place): (i64, usize)| Card {
                student,
                middle: (
                    1144.0 + (place % 6) as f32 * 205.7,
                    584.0 + (place / 6) as f32 * 282.0,
                ),
            })
            .collect();
        let scrolled = Formation {
            placed: [Some(1), Some(2), Some(3), Some(4), Some(201), Some(202)],
            cards,
            ..shown()
        };
        // The attack keeps 1 and 2 and wants 30 (two rows below), then 11 (the row above).
        let attack = Some([1, 2, 30, 11, 201, 202]);
        let marks = for_formation(&scrolled, None, attack, &opened(), &listed());
        assert_eq!(
            marks[2..],
            [
                Mark::Seek {
                    student: 30,
                    way: Way::Down,
                    middle: (2232.0, 918.0),
                },
                Mark::Rows {
                    rows: 2,
                    middle: (2232.0, 918.0),
                },
            ]
        );
        let picked = Formation {
            placed: [Some(1), Some(2), Some(30), Some(4), Some(201), Some(202)],
            ..scrolled.clone()
        };
        let marks = for_formation(&picked, None, attack, &opened(), &listed());
        assert_eq!(
            marks[1..],
            [
                Mark::Seek {
                    student: 11,
                    way: Way::Up,
                    middle: (2232.0, 455.0),
                },
                Mark::Rows {
                    rows: 1,
                    middle: (2232.0, 455.0),
                },
            ]
        );
        // Not every student read: the ways alone, as before.
        let partly = Roll {
            whole: false,
            ..listed()
        };
        let marks = for_formation(&scrolled, None, attack, &opened(), &partly);
        assert!(
            !marks.iter().any(|mark| matches!(mark, Mark::Rows { .. })),
            "{marks:?}"
        );
        assert_eq!(marks.len(), 3, "the two to take out, and the way to 30");
    }

    /// The game puts a card picked into the leftmost empty place of its squad, so the
    /// student to pick is the one the attack has for that place, one at a time.
    #[test]
    fn the_next_pick_is_the_attacks_student_for_the_leftmost_empty_place() {
        let wanted = [1, 9, 3, 8];
        let next = |places: [Option<i64>; 4]| next(&wanted, &places);
        assert_eq!(next([Some(1), None, Some(3), None]), Some(9), "the second");
        assert_eq!(next([Some(1), Some(9), Some(3), None]), Some(8), "then");
        assert_eq!(next([Some(1), Some(9), Some(3), Some(8)]), None, "done");
        assert_eq!(next([None; 4]), Some(1), "an empty team, from the left");
        // A student to take out beside it: a pick still lands in the empty place.
        assert_eq!(next([Some(1), Some(4), Some(3), None]), Some(8));
        assert_eq!(next([Some(1), None, Some(3), Some(4)]), Some(9));
        // No place empty: the leftmost one to be emptied.
        assert_eq!(next([Some(1), Some(2), Some(3), Some(4)]), Some(9));
        // The place's student stands elsewhere: the first of the attack not placed.
        assert_eq!(next([Some(9), None, Some(3), None]), Some(1));
    }

    #[test]
    fn one_student_is_told_at_a_time() {
        // The attack has 9 (out of sight) for the second place and 8 (in sight) for the fourth.
        let attack = Some([1, 9, 3, 8, 201, 203]);
        let marks = |second: Option<i64>| {
            let team = Formation {
                placed: [Some(1), second, Some(3), None, Some(201), Some(203)],
                ..shown()
            };
            for_formation(
                &team,
                None,
                attack,
                &Lead::all(&[1, 2, 3, 4]),
                &told(ranks()),
            )
        };
        assert_eq!(
            marks(None),
            [Mark::Seek {
                student: 9,
                way: Way::Down,
                middle: (2232.0, 918.0),
            }],
            "the second place is the leftmost empty one: 8 waits, in sight as it is"
        );
        assert_eq!(
            marks(Some(9)),
            [Mark::Pick((1102.0, 654.0, 1298.0, 922.0))],
            "9 stands: 8"
        );
    }

    #[test]
    fn the_cards_to_pick_and_to_take_out_are_marked() {
        // The attack keeps 1 and 2, and wants 8 (in sight) then 9 (not), with 201 and 204.
        let marks = for_formation(
            &shown(),
            Some([301, 302, 303]),
            Some([1, 2, 8, 9, 201, 204]),
            &Lead::all(&[1, 2, 3, 4]),
            &told(ranks()),
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
                // 9 is told once 8 stands.
                Mark::Pick((1102.0, 654.0, 1298.0, 922.0)),
            ]
        );
    }

    #[test]
    fn a_card_is_marked_within_the_list() {
        let mut low = shown();
        low.cards[7].middle.1 = 866.0;
        let marks = for_formation(
            &low,
            None,
            Some([1, 2, 3, 8, 201, 203]),
            &Lead::all(&[]),
            &told(ranks()),
        );
        assert_eq!(
            marks.last(),
            Some(&Mark::Pick((1102.0, 776.0, 1298.0, 962.0))),
            "cut at the list's foot"
        );
    }

    #[test]
    fn the_other_tab_is_marked_once_this_one_is_done() {
        let attack = Some([1, 2, 3, 4, 201, 204]);
        let marks = for_formation(
            &shown(),
            None,
            attack,
            &Lead::all(&[1, 2, 3, 4]),
            &told(ranks()),
        );
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
        let marks = for_formation(
            &specials,
            None,
            attack,
            &Lead::all(&[201, 203]),
            &told(ranks()),
        );
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
        assert_eq!(
            for_formation(&shown(), None, None, &Lead::all(&[]), &told(ranks())),
            []
        );
        let chosen = Some([1, 2, 3, 4, 201, 203]);
        assert_eq!(
            for_formation(&shown(), None, chosen, &Lead::all(&[]), &told(ranks())),
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

    /// The dialog on the strikers' tab with the list of `apart`: the strikers placed, and
    /// the cards in sight.
    fn dialog(strikers: [Option<i64>; 4], cards: Vec<Card>) -> Formation {
        let [first, second, third, fourth] = strikers;
        Formation {
            placed: [first, second, third, fourth, Some(201), Some(202)],
            cards,
            ..shown()
        }
    }

    /// With nothing known of the team the dialog opened with, the rows are marked where the
    /// cards in sight tell them whoever stands first.
    #[test]
    fn the_rows_are_marked_without_the_team_the_dialog_opened_with() {
        let scrolled = dialog([Some(1), None, Some(3), None], grid(12..24));
        // The attack has 33 (two rows below) for the second place.
        let attack = Some([1, 33, 3, 30, 201, 202]);
        let marks = for_formation(&scrolled, None, attack, &Lead::default(), &apart());
        let seek = |student| Mark::Seek {
            student,
            way: Way::Down,
            middle: (2232.0, 918.0),
        };
        assert_eq!(
            marks,
            [
                seek(33),
                Mark::Rows {
                    rows: 2,
                    middle: (2232.0, 918.0),
                },
            ]
        );
        // 30 stands in the next row or the one after, by who stands first: the way alone.
        let attack = Some([1, 30, 3, 33, 201, 202]);
        let marks = for_formation(&scrolled, None, attack, &Lead::default(), &apart());
        assert_eq!(marks, [seek(30)]);
    }

    #[test]
    fn a_dialog_seen_to_open_shows_its_team_first() {
        let (start, mut visit, roll) = (Instant::now(), Visit::default(), apart());
        let team = |third| dialog([Some(1), Some(2), third, None], grid(12..24));
        // Other screens for a while, then the dialog: it opened under Rio's eyes.
        visit.away(start);
        let opened = start + Duration::from_secs(3);
        let all = |known: &[i64]| Lead::all(known);
        assert_eq!(*visit.enter(&team(Some(3)), &roll, opened), all(&[1, 2, 3]));
        let soon = opened + Duration::from_secs(1);
        assert_eq!(
            *visit.enter(&team(None), &roll, soon),
            all(&[1, 2, 3]),
            "one taken out"
        );
        // The specials' tab shows the team's specials first.
        let specials = Formation {
            tab: Tab::Special,
            cards: Vec::new(),
            ..team(None)
        };
        assert_eq!(*visit.enter(&specials, &roll, soon), all(&[201, 202]));
        // Closed, and opened anew with another team.
        visit.away(soon + Duration::from_secs(1));
        let later = soon + Duration::from_secs(60);
        assert_eq!(
            *visit.enter(&team(Some(9)), &roll, later),
            all(&[1, 2, 9]),
            "opened anew"
        );
        // Nothing read for a while (the reading paused): what opened meanwhile was not seen.
        let resumed = later + Duration::from_secs(60);
        assert_eq!(
            *visit.enter(&team(None), &roll, resumed),
            Lead::default(),
            "not seen to open"
        );
    }

    /// A dialog already up when Rio first looks did not open under its eyes: whoever
    /// stands in the team now, who it opened with is told by the list's top alone.
    #[test]
    fn a_dialog_up_before_rio_looked_tells_its_team_at_the_lists_top() {
        let (start, mut visit, roll) = (Instant::now(), Visit::default(), apart());
        // Two of the four were taken out before Rio looked, and the list is scrolled.
        let team = [Some(1), None, Some(3), None];
        let scrolled = dialog(team, grid(12..24));
        assert_eq!(*visit.enter(&scrolled, &roll, start), Lead::default());
        // A frame not read between two of the dialog is no opening.
        visit.away(start + Duration::from_millis(300));
        let soon = start + Duration::from_millis(400);
        assert_eq!(*visit.enter(&scrolled, &roll, soon), Lead::default());
        // The list's top comes in sight: the team the dialog opened with stands there.
        let top = dialog(team, grid([1, 2, 3, 4, 10, 11, 12, 13, 14, 15, 16, 17]));
        let then = soon + Duration::from_secs(1);
        assert_eq!(*visit.enter(&top, &roll, then), Lead::all(&[1, 2, 3, 4]));
        // It stays known with the list scrolled away again.
        let after = then + Duration::from_secs(1);
        assert_eq!(
            *visit.enter(&scrolled, &roll, after),
            Lead::all(&[1, 2, 3, 4])
        );
    }

    /// A moment without the dialog as Rio starts is no proof that the dialog opened then.
    #[test]
    fn a_moment_without_the_dialog_is_no_opening() {
        let (start, mut visit, roll) = (Instant::now(), Visit::default(), apart());
        visit.away(start);
        let scrolled = dialog([Some(1), None, Some(3), None], grid(12..24));
        let soon = start + Duration::from_millis(200);
        assert_eq!(*visit.enter(&scrolled, &roll, soon), Lead::default());
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
