//! The tiers the user sorts the students they use often into (Tier表), S the highest: the
//! students of a higher tier are offered first where a formation is put together by hand.
//! The table's last row holds the students the user keeps out of their attacks (編成NG):
//! whether a student is barred says nothing of her tier, and she may stand in both rows.

use crate::{Squad, Student};

/// A tier, the highest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tier {
    S,
    A,
    B,
    C,
    D,
}

impl Tier {
    pub const ALL: [Self; 5] = [Self::S, Self::A, Self::B, Self::C, Self::D];

    /// The tier's letter.
    pub fn name(self) -> &'static str {
        match self {
            Self::S => "S",
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
        }
    }

    /// The tier as the records keep it: 0 for S to 4 for D.
    pub fn rank(self) -> i64 {
        self as i64
    }

    /// The tier the records keep as `rank`; none for a number no tier has.
    pub fn of_rank(rank: i64) -> Option<Self> {
        Self::ALL.into_iter().find(|tier| tier.rank() == rank)
    }
}

/// A row of the table the students are sorted in (Tier表): a tier, or the students the user
/// keeps out of their attacks (編成NG).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Row {
    Tier(Tier),
    Barred,
}

impl Row {
    /// The rows from the table's top, and the one place that orders them: the tiers, the
    /// highest first, then those barred (the user, 2026-11-21: NG under D).
    pub const ALL: [Self; 6] = [
        Self::Tier(Tier::S),
        Self::Tier(Tier::A),
        Self::Tier(Tier::B),
        Self::Tier(Tier::C),
        Self::Tier(Tier::D),
        Self::Barred,
    ];

    /// What the row's plate says.
    pub fn name(self) -> &'static str {
        match self {
            Self::Tier(tier) => tier.name(),
            Self::Barred => "NG",
        }
    }
}

/// Where the user sorted a student: the tier she is of, and whether she is barred. Neither
/// says anything of the other (the user, 2026-11-21: one barred is only one the user cannot
/// send; she is put into a formation told of, or an opponent's, like anybody).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Sorted {
    pub tier: Option<Tier>,
    pub barred: bool,
}

impl Sorted {
    /// Where `student` stands among `tiers` (each student once, by id) and those `barred`.
    pub fn of(tiers: &[(i64, Tier)], barred: &[i64], student: i64) -> Self {
        Self {
            tier: of(tiers, student),
            barred: barred.contains(&student),
        }
    }

    /// Whether the student stands in `row`: one of a tier and barred stands in two rows.
    pub fn stands_in(self, row: Row) -> bool {
        match row {
            Row::Tier(tier) => self.tier == Some(tier),
            Row::Barred => self.barred,
        }
    }
}

/// What a student dropped in the table changes of where she stands: her tier (none: out of
/// the tiers), or whether she is barred.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Tier(Option<Tier>),
    Barred(bool),
}

/// What dropping a student's tile changes: she stands as `sorted`, the tile was dragged from
/// the row `from` and is dropped onto the row `onto` (none: the list of those in no row). A
/// row changes the one thing it tells; the list takes her out of the row her tile came from.
/// None where nothing would change (a row she stands in already): no drop is taken there.
pub fn dropped(sorted: Sorted, from: Option<Row>, onto: Option<Row>) -> Option<Change> {
    match (onto, from) {
        (Some(row), _) if sorted.stands_in(row) => None,
        (Some(Row::Tier(tier)), _) => Some(Change::Tier(Some(tier))),
        (Some(Row::Barred), _) => Some(Change::Barred(true)),
        (None, Some(Row::Tier(_))) => Some(Change::Tier(None)),
        (None, Some(Row::Barred)) => Some(Change::Barred(false)),
        (None, None) => None,
    }
}

/// The tier of a student among `tiers` (each student once, by id).
pub fn of(tiers: &[(i64, Tier)], student: i64) -> Option<Tier> {
    let found = tiers.iter().find(|(id, _)| *id == student);
    found.map(|(_, tier)| *tier)
}

/// The students, those of a higher tier first and those of no tier last; within a tier in
/// the order given.
pub fn by_tier<'a>(mut students: Vec<&'a Student>, tiers: &[(i64, Tier)]) -> Vec<&'a Student> {
    // Stable: `None`, no tier, is the last of the keys.
    students
        .sort_by_key(|student| of(tiers, student.id).map_or(Tier::ALL.len() as i64, Tier::rank));
    students
}

/// The students as they stand in a row of the table: the strikers before the specials (the
/// user, 2026-11-21: by squad, then by name); within a squad in the order given.
pub fn by_squad(mut students: Vec<&Student>) -> Vec<&Student> {
    // Stable: the strikers' key is the lower.
    students.sort_by_key(|student| student.squad == Squad::Support);
    students
}

#[cfg(test)]
mod tests {
    use super::{Change, Row, Sorted, Tier, by_squad, by_tier, dropped, of};
    use crate::{Armor, Attack, Role, Squad, Student};

    #[test]
    fn the_strikers_come_before_the_specials_and_the_order_given_stays_within() {
        let student = |(id, squad): (i64, Squad)| Student {
            id,
            name: format!("生徒{id}"),
            squad,
            role: Role::Tanker,
            attack: Attack::Pierce,
            armor: Armor::HeavyArmor,
            school: String::new(),
            reading: None,
        };
        let ids = |found: Vec<&Student>| -> Vec<i64> { found.iter().map(|one| one.id).collect() };
        let (striker, special) = (Squad::Main, Squad::Support);
        let mixed = [(1, special), (2, striker), (3, special), (4, striker)];
        let students: Vec<Student> = mixed.map(student).into();
        assert_eq!(
            ids(by_squad(students.iter().collect())),
            [2, 4, 1, 3],
            "the strikers as given, then the specials as given"
        );
        // One squad alone: as given.
        let students: Vec<Student> = [(3, special), (1, special)].map(student).into();
        assert_eq!(ids(by_squad(students.iter().collect())), [3, 1]);
        assert!(by_squad(Vec::new()).is_empty());
    }

    #[test]
    fn the_tiers_are_five_from_s_to_d() {
        let names: Vec<_> = Tier::ALL.into_iter().map(Tier::name).collect();
        assert_eq!(names, ["S", "A", "B", "C", "D"]);
        assert!(Tier::S < Tier::A && Tier::C < Tier::D, "the highest first");
        // As the records keep them, and back.
        for (rank, tier) in Tier::ALL.into_iter().enumerate() {
            assert_eq!(tier.rank(), rank as i64);
            assert_eq!(Tier::of_rank(rank as i64), Some(tier));
        }
        assert_eq!(Tier::of_rank(5), None);
        assert_eq!(Tier::of_rank(-1), None);
    }

    #[test]
    fn a_students_tier_is_looked_up_by_id() {
        let tiers = [(3, Tier::A), (7, Tier::S), (9, Tier::D)];
        assert_eq!(of(&tiers, 7), Some(Tier::S));
        assert_eq!(of(&tiers, 9), Some(Tier::D));
        assert_eq!(of(&tiers, 4), None, "in no tier");
        assert_eq!(of(&[], 7), None);
    }

    #[test]
    fn the_higher_tier_comes_first_and_the_order_given_stays_within() {
        let student = |id: i64| Student {
            id,
            name: format!("生徒{id}"),
            squad: Squad::Main,
            role: Role::Tanker,
            attack: Attack::Pierce,
            armor: Armor::HeavyArmor,
            school: String::new(),
            reading: None,
        };
        let students: Vec<Student> = [5, 1, 4, 2, 3, 6].map(student).into();
        let tiers = [(1, Tier::B), (2, Tier::S), (3, Tier::B), (6, Tier::D)];
        let ids = |found: Vec<&Student>| -> Vec<i64> { found.iter().map(|one| one.id).collect() };
        assert_eq!(
            ids(by_tier(students.iter().collect(), &tiers)),
            [2, 1, 3, 6, 5, 4],
            "S, then B as given (1 before 3), then D, then those of no tier as given"
        );
        assert_eq!(
            ids(by_tier(students.iter().collect(), &[])),
            [5, 1, 4, 2, 3, 6],
            "nobody in a tier: the order given"
        );
    }

    #[test]
    fn the_tables_rows_are_the_tiers_and_under_them_those_barred() {
        let names: Vec<_> = Row::ALL.into_iter().map(Row::name).collect();
        assert_eq!(names, ["S", "A", "B", "C", "D", "NG"]);
        // Every tier has its row, and those barred have one.
        for tier in Tier::ALL {
            assert!(Row::ALL.contains(&Row::Tier(tier)));
        }
        assert!(Row::ALL.contains(&Row::Barred));
    }

    #[test]
    fn a_student_stands_in_her_tiers_row_and_among_those_barred_at_once() {
        let (tiers, barred) = ([(3, Tier::A), (7, Tier::S)], [7, 9]);
        // Of a tier and barred: in both rows, and in no other tier's.
        let both = Sorted::of(&tiers, &barred, 7);
        let (tier, barred_too) = (Some(Tier::S), true);
        assert_eq!((both.tier, both.barred), (tier, barred_too));
        assert!(both.stands_in(Row::Tier(Tier::S)) && both.stands_in(Row::Barred));
        assert!(!both.stands_in(Row::Tier(Tier::A)));
        // Of a tier alone, and barred alone.
        let tiered = Sorted::of(&tiers, &barred, 3);
        assert!(tiered.stands_in(Row::Tier(Tier::A)) && !tiered.stands_in(Row::Barred));
        let out = Sorted::of(&tiers, &barred, 9);
        assert!(out.stands_in(Row::Barred));
        assert!(
            Tier::ALL
                .into_iter()
                .all(|tier| !out.stands_in(Row::Tier(tier)))
        );
        // Neither: in no row.
        let nowhere = Sorted::of(&tiers, &barred, 4);
        assert_eq!(nowhere, Sorted::default());
        assert!(Row::ALL.into_iter().all(|row| !nowhere.stands_in(row)));
    }

    #[test]
    fn a_drop_changes_the_one_thing_its_row_tells() {
        let (s, a, out) = (Row::Tier(Tier::S), Row::Tier(Tier::A), Row::Barred);
        let sorted = |tier, barred| Sorted { tier, barred };
        let (into_a, bar) = (Change::Tier(Some(Tier::A)), Change::Barred(true));
        // From the list into a row.
        let nowhere = Sorted::default();
        assert_eq!(dropped(nowhere, None, Some(a)), Some(into_a));
        assert_eq!(dropped(nowhere, None, Some(out)), Some(bar));
        // From a tier into another: the tier. Into those barred: barred, and nothing of
        // her tier.
        let tiered = sorted(Some(Tier::S), false);
        assert_eq!(dropped(tiered, Some(s), Some(a)), Some(into_a));
        assert_eq!(dropped(tiered, Some(s), Some(out)), Some(bar));
        // From among those barred into a tier: the tier, and nothing of her being barred.
        let barred = sorted(None, true);
        assert_eq!(dropped(barred, Some(out), Some(a)), Some(into_a));
        // Back onto the list: out of the row the tile was dragged from, and of that alone.
        let both = sorted(Some(Tier::S), true);
        assert_eq!(dropped(both, Some(s), None), Some(Change::Tier(None)));
        assert_eq!(dropped(both, Some(out), None), Some(Change::Barred(false)));
        // Nothing where nothing would change: a row she stands in already, whichever of her
        // tiles is dragged, and the list for one of the list.
        assert_eq!(dropped(both, Some(s), Some(s)), None);
        assert_eq!(dropped(both, Some(s), Some(out)), None);
        assert_eq!(dropped(both, Some(out), Some(s)), None);
        assert_eq!(dropped(nowhere, None, None), None);
    }
}
