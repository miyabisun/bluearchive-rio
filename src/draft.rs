//! The two teams the user puts together by hand to register an attack that is told to beat
//! a defense: twelve places, the attack's six then the defense's six, each team its four
//! strikers then its two specials, and the place the next student goes into.

use crate::Squad;

/// The places of both teams.
pub const PLACES: usize = 12;
/// The places of one team.
pub const TEAM: usize = 6;

/// Whom a place takes: a team's first four its strikers, the last two its specials.
pub fn takes(place: usize) -> Squad {
    if place % TEAM < 4 {
        Squad::Main
    } else {
        Squad::Support
    }
}

/// Why a student did not go into a place: the place takes the other squad.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Refused {
    /// Whom the place takes.
    pub takes: Squad,
}

/// The teams as far as they are put together, and the place chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    places: [Option<i64>; PLACES],
    chosen: usize,
}

impl Default for Draft {
    fn default() -> Self {
        Self::new()
    }
}

impl Draft {
    /// Nobody anywhere, the attack's first striker chosen.
    pub fn new() -> Self {
        Self {
            places: [None; PLACES],
            chosen: 0,
        }
    }

    /// The place the next student goes into.
    pub fn chosen(&self) -> usize {
        self.chosen
    }

    /// Chooses a place.
    pub fn choose(&mut self, place: usize) {
        self.chosen = place % PLACES;
    }

    /// The student at a place.
    pub fn at(&self, place: usize) -> Option<i64> {
        self.places[place]
    }

    /// Puts a student into the place chosen and chooses the next: the place to the right,
    /// after a team's last the other team's first.
    pub fn put(&mut self, student: i64, squad: Squad) -> Result<(), Refused> {
        self.drop_on(self.chosen, student, squad)?;
        self.chosen = (self.chosen + 1) % PLACES;
        Ok(())
    }

    /// Puts a student into a place, as dropped there: the place chosen stays. A student
    /// stands once in a team: put in again, she leaves where she stood.
    pub fn drop_on(&mut self, place: usize, student: i64, squad: Squad) -> Result<(), Refused> {
        if takes(place) != squad {
            return Err(Refused {
                takes: takes(place),
            });
        }
        self.places[place] = Some(student);
        self.once(place);
        Ok(())
    }

    /// Takes the student who came to `place` out of the other places of that team.
    fn once(&mut self, place: usize) {
        let (came, team) = (self.places[place], place / TEAM);
        for other in (team * TEAM..(team + 1) * TEAM).filter(|other| *other != place) {
            if came.is_some() && self.places[other] == came {
                self.places[other] = None;
            }
        }
    }

    /// Changes two places' students for each other, across the teams too: strikers for
    /// strikers, specials for specials.
    pub fn swap(&mut self, from: usize, to: usize) -> Result<(), Refused> {
        if takes(from) != takes(to) {
            return Err(Refused { takes: takes(to) });
        }
        self.places.swap(from, to);
        self.once(from);
        self.once(to);
        Ok(())
    }

    /// The attack and the defense, once every place has its student.
    pub fn teams(&self) -> Option<([i64; TEAM], [i64; TEAM])> {
        let mut teams = [[0; TEAM]; 2];
        for (place, student) in self.places.iter().enumerate() {
            teams[place / TEAM][place % TEAM] = (*student)?;
        }
        Some((teams[0], teams[1]))
    }
}

#[cfg(test)]
mod tests {
    use super::{Draft, PLACES, Refused, takes};
    use crate::Squad::{Main, Support};

    #[test]
    fn a_team_is_four_strikers_and_two_specials() {
        let kinds: Vec<_> = (0..PLACES).map(takes).collect();
        let team = [Main, Main, Main, Main, Support, Support];
        assert_eq!(kinds, [team, team].concat());
    }

    #[test]
    fn a_student_goes_into_the_place_chosen_and_the_next_is_chosen() {
        let mut draft = Draft::new();
        assert_eq!(draft.chosen(), 0, "the attack's first striker at first");
        // The attack: four strikers, then the specials to their right.
        for student in 1..=4 {
            draft.put(student, Main).unwrap();
        }
        assert_eq!(
            draft.chosen(),
            4,
            "after the fourth striker, the left special"
        );
        draft.put(5, Support).unwrap();
        draft.put(6, Support).unwrap();
        assert_eq!(draft.chosen(), 6, "then the defense's first striker");
        assert_eq!(draft.teams(), None, "the defense is not put together yet");
        for student in 11..=14 {
            draft.put(student, Main).unwrap();
        }
        draft.put(15, Support).unwrap();
        draft.put(16, Support).unwrap();
        assert_eq!(draft.chosen(), 0, "and the attack's first striker again");
        assert_eq!(
            draft.teams(),
            Some(([1, 2, 3, 4, 5, 6], [11, 12, 13, 14, 15, 16]))
        );
        // A place chosen by hand takes the next student, and the one after it follows.
        draft.choose(8);
        draft.put(20, Main).unwrap();
        assert_eq!((draft.at(8), draft.chosen()), (Some(20), 9));
    }

    #[test]
    fn a_place_takes_its_own_squad_only() {
        let mut draft = Draft::new();
        assert_eq!(draft.put(5, Support), Err(Refused { takes: Main }));
        assert_eq!((draft.at(0), draft.chosen()), (None, 0), "nothing changed");
        draft.choose(4);
        assert_eq!(draft.put(1, Main), Err(Refused { takes: Support }));
        assert_eq!(draft.drop_on(11, 1, Main), Err(Refused { takes: Support }));
        assert_eq!(draft.drop_on(11, 5, Support), Ok(()));
        assert_eq!(
            (draft.at(11), draft.chosen()),
            (Some(5), 4),
            "dropped: the place chosen stays"
        );
    }

    #[test]
    fn a_student_stands_once_in_a_team() {
        let mut draft = Draft::new();
        draft.put(1, Main).unwrap();
        draft.put(2, Main).unwrap();
        // Put in again, she moves: from the first place to the third.
        draft.put(1, Main).unwrap();
        assert_eq!(
            (draft.at(0), draft.at(1), draft.at(2)),
            (None, Some(2), Some(1))
        );
        // The other team may have her too.
        draft.drop_on(6, 1, Main).unwrap();
        assert_eq!((draft.at(2), draft.at(6)), (Some(1), Some(1)));
        // Dropped onto where she stands, she stays.
        draft.drop_on(2, 1, Main).unwrap();
        assert_eq!(draft.at(2), Some(1));
    }

    #[test]
    fn two_places_change_students() {
        let mut draft = Draft::new();
        draft.put(1, Main).unwrap();
        draft.put(2, Main).unwrap();
        draft.swap(0, 1).unwrap();
        assert_eq!((draft.at(0), draft.at(1)), (Some(2), Some(1)));
        // With an empty place, and across the teams.
        draft.swap(1, 7).unwrap();
        assert_eq!((draft.at(1), draft.at(7)), (None, Some(1)));
        assert_eq!(draft.chosen(), 2, "the place chosen stays");
        // A striker and a special do not change places.
        assert_eq!(draft.swap(0, 4), Err(Refused { takes: Support }));
        assert_eq!(draft.swap(4, 0), Err(Refused { takes: Main }));
        assert_eq!(draft.at(0), Some(2));
        // Onto itself: nothing.
        assert_eq!(draft.swap(0, 0), Ok(()));
        assert_eq!(draft.at(0), Some(2));
        // A student who comes into a team that has her leaves her old place there.
        draft.drop_on(3, 9, Main).unwrap();
        draft.drop_on(8, 9, Main).unwrap();
        draft.swap(8, 0).unwrap();
        assert_eq!(
            (draft.at(0), draft.at(3), draft.at(8)),
            (Some(9), None, Some(2))
        );
    }
}
