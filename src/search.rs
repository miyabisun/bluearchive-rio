//! Finding students to put into a formation by hand: by the name as it is typed (the IME
//! gives hiragana, the names are in katakana), and by squad, class, attack type and armor
//! type.

use crate::{Armor, Attack, Role, Squad, Student};

/// Folds a text so a query matches however it is typed: hiragana into katakana, capitals
/// into small letters, spaces dropped.
pub fn fold(text: &str) -> String {
    let folded = text
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| match c {
            // ぁ to ゖ are their katakana less 0x60.
            'ぁ'..='ゖ' => char::from_u32(u32::from(c) + 0x60).unwrap_or(c),
            _ => c,
        });
    folded.collect::<String>().to_lowercase()
}

/// What narrows the students down, by what the user pressed: one of each kind at most
/// (`toggle`), and each kind with something pressed has to fit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    /// The squads: the strikers, the specials.
    pub squads: Vec<Squad>,
    pub roles: Vec<Role>,
    pub attacks: Vec<Attack>,
    pub armors: Vec<Armor>,
}

/// Presses `one` in a kind's list, the one pressed before let up (one of a kind is pressed
/// at a time); pressed again, `one` is let up and its kind asks for nothing.
pub fn toggle<T: PartialEq>(pressed: &mut Vec<T>, one: T) {
    let was = pressed.contains(&one);
    pressed.clear();
    if !was {
        pressed.push(one);
    }
}

impl Filter {
    /// Whether a student fits.
    pub fn fits(&self, student: &Student) -> bool {
        fn any<T: PartialEq>(pressed: &[T], one: &T) -> bool {
            pressed.is_empty() || pressed.contains(one)
        }
        any(&self.squads, &student.squad)
            && any(&self.roles, &student.role)
            && any(&self.attacks, &student.attack)
            && any(&self.armors, &student.armor)
    }
}

/// The students whose name (or its reading) holds the query and who fit the filter, by
/// name: by the reading where the name is not in kana.
pub fn found<'a>(students: &'a [Student], query: &str, filter: &Filter) -> Vec<&'a Student> {
    let query = fold(query);
    // What a student is ordered by, and found by beside the name.
    let read = |student: &Student| fold(student.reading.as_deref().unwrap_or(&student.name));
    let mut found: Vec<(String, &Student)> = students
        .iter()
        .filter(|student| filter.fits(student))
        .map(|student| (read(student), student))
        .filter(|(read, student)| read.contains(&query) || fold(&student.name).contains(&query))
        .collect();
    found.sort_by(|(read, student), (other_read, other)| {
        read.cmp(other_read).then(student.id.cmp(&other.id))
    });
    found.into_iter().map(|(_, student)| student).collect()
}

#[cfg(test)]
mod tests {
    use super::{Filter, fold, found, toggle};
    use crate::{Armor, Attack, Role, Squad, Student};

    fn student(id: i64, name: &str, role: Role, attack: Attack, armor: Armor) -> Student {
        Student {
            id,
            name: name.to_owned(),
            squad: Squad::Main,
            role,
            attack,
            armor,
            school: String::new(),
            reading: None,
        }
    }

    /// Five strikers, not in the order of their names, and a special.
    fn students() -> Vec<Student> {
        use {Armor::*, Attack::*, Role::*};
        let mut miku = student(5, "初音ミク", Supporter, Explosion, LightArmor);
        miku.reading = Some("はつねミク".to_owned());
        vec![
            student(1, "ホシノ", Tanker, Pierce, HeavyArmor),
            student(2, "ホシノ（水着）", DamageDealer, Explosion, LightArmor),
            student(3, "アル", DamageDealer, Explosion, LightArmor),
            student(4, "トキ", DamageDealer, Pierce, ElasticArmor),
            miku,
            Student {
                squad: Squad::Support,
                ..student(6, "アコ", Supporter, Explosion, LightArmor)
            },
        ]
    }

    /// The strikers alone, as for a striker's place.
    fn strikers() -> Filter {
        Filter {
            squads: vec![Squad::Main],
            ..Filter::default()
        }
    }

    fn ids(found: Vec<&Student>) -> Vec<i64> {
        found.into_iter().map(|student| student.id).collect()
    }

    #[test]
    fn a_text_is_folded_however_it_is_typed() {
        assert_eq!(fold("ほしの"), "ホシノ");
        assert_eq!(fold("ホシノ"), "ホシノ");
        assert_eq!(fold("ほシ の　"), "ホシノ", "mixed, with spaces");
        assert_eq!(fold("ぁゖー"), "ァヶー", "the first and the last hiragana");
        assert_eq!(fold("TS"), "ts");
    }

    #[test]
    fn students_are_found_by_kana_in_the_order_of_their_names() {
        let students = students();
        let strikers = strikers();
        assert_eq!(
            ids(found(&students, "", &strikers)),
            [3, 4, 5, 1, 2],
            "all of them by name: アル, トキ, はつねミク by its reading, ホシノ, ホシノ（水着）"
        );
        assert_eq!(ids(found(&students, "ほし", &strikers)), [1, 2], "hiragana");
        assert_eq!(ids(found(&students, "シノ", &strikers)), [1, 2], "a part");
        assert_eq!(
            ids(found(&students, "みく", &strikers)),
            [5],
            "the name's kana"
        );
        assert_eq!(
            ids(found(&students, "はつね", &strikers)),
            [5],
            "the reading"
        );
        assert!(found(&students, "ひな", &strikers).is_empty());
    }

    #[test]
    fn the_squads_pressed_are_found() {
        let students = students();
        // Nothing pressed: both squads, アコ the special before アル by name.
        let all = Filter::default();
        assert_eq!(ids(found(&students, "あ", &all)), [6, 3]);
        // The strikers alone: アコ is not found, by her name either.
        assert_eq!(ids(found(&students, "あ", &strikers())), [3]);
        let mut filter = Filter::default();
        toggle(&mut filter.squads, Squad::Support);
        assert_eq!(ids(found(&students, "", &filter)), [6]);
        assert!(found(&students, "ほしの", &filter).is_empty());
        // The other squad pressed takes its place.
        toggle(&mut filter.squads, Squad::Main);
        assert_eq!(filter, strikers());
        assert_eq!(ids(found(&students, "あ", &filter)), [3]);
    }

    #[test]
    fn a_filter_takes_one_of_a_kind_and_every_kind() {
        let students = students();
        let mut filter = strikers();
        toggle(&mut filter.attacks, Attack::Pierce);
        assert_eq!(ids(found(&students, "", &filter)), [4, 1]);
        // Another of the same kind takes its place: one of a kind is pressed at a time.
        toggle(&mut filter.attacks, Attack::Explosion);
        assert_eq!(filter.attacks, [Attack::Explosion]);
        assert_eq!(ids(found(&students, "", &filter)), [3, 5, 2]);
        // Another kind: both have to fit.
        toggle(&mut filter.roles, Role::DamageDealer);
        assert_eq!(ids(found(&students, "", &filter)), [3, 2]);
        toggle(&mut filter.armors, Armor::ElasticArmor);
        assert!(found(&students, "", &filter).is_empty());
        toggle(&mut filter.armors, Armor::LightArmor);
        assert_eq!(ids(found(&students, "", &filter)), [3, 2]);
        // With the name as well.
        assert_eq!(ids(found(&students, "ほしの", &filter)), [2]);
        // Pressed again, a button is let up, and its kind asks for nothing.
        toggle(&mut filter.armors, Armor::LightArmor);
        toggle(&mut filter.roles, Role::DamageDealer);
        toggle(&mut filter.attacks, Attack::Explosion);
        assert_eq!(filter, strikers());
        assert_eq!(ids(found(&students, "ほしの", &filter)), [1, 2]);
    }
}
