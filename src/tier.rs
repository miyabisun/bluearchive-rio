//! The tiers the user sorts the students they use often into (編成Tier), S the highest: the
//! students of a higher tier are offered first where a formation is put together by hand.

use crate::Student;

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

#[cfg(test)]
mod tests {
    use super::{Tier, by_tier, of};
    use crate::{Armor, Attack, Role, Squad, Student};

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
}
