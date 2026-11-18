//! What Rio marks over the game, on the measured screen (see `history`): the strikers the
//! user takes to stand where the game shows "?", and in the formation dialog the cards to
//! pick and to take out for the attack the user chose. Positions in the team are set after the
//! dialog, by dragging: in it, the six students are only chosen. Over a battle record it
//! says whether the record is kept, and over the list of records which rows' records are:
//! the user's eyes stay on the game.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use crate::{
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
/// dialog opened: the level, the stars, then the bond, the higher first. Five stars are
/// the most (a weapon's stars do not count). Students alike in all three stand in an order
/// Rio does not know (it may be the order they were met in).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Rank {
    pub level: u8,
    pub rarity: u8,
    pub bond: u8,
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
    // The cards in sight are a stretch of the order. A student alike its first card, and
    // not its last, cannot stand after the stretch; and the other way around.
    if own > first || (own == first && own != last) {
        Way::Up
    } else if own < last || (own == last && own != first) {
        Way::Down
    } else {
        Way::Either
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
        Mark, Rank, Visit, Way, Words, for_formation, for_list, for_opponent, for_record, way,
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
