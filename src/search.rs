//! Finding students to put into a formation by hand: by the name as it is typed (the IME
//! gives hiragana, the names are in katakana), and by class, attack type and armor type.

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

/// What narrows the students down, by what the user pressed: within one kind any of those
/// pressed fits, and each kind with something pressed has to fit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    pub roles: Vec<Role>,
    pub attacks: Vec<Attack>,
    pub armors: Vec<Armor>,
}

/// Presses `one` in a kind's list, or lets it up again.
pub fn toggle<T: PartialEq>(pressed: &mut Vec<T>, one: T) {
    match pressed.iter().position(|other| *other == one) {
        Some(at) => drop(pressed.remove(at)),
        None => pressed.push(one),
    }
}

impl Filter {
    /// Whether a student fits.
    pub fn fits(&self, student: &Student) -> bool {
        fn any<T: PartialEq>(pressed: &[T], one: &T) -> bool {
            pressed.is_empty() || pressed.contains(one)
        }
        any(&self.roles, &student.role)
            && any(&self.attacks, &student.attack)
            && any(&self.armors, &student.armor)
    }
}

/// The students of a squad (the one the place to fill takes: nobody is offered who cannot
/// go there) whose name (or its reading) holds the query and who fit the filter, by name:
/// by the reading where the name is not in kana.
pub fn found<'a>(
    students: &'a [Student],
    query: &str,
    filter: &Filter,
    squad: Squad,
) -> Vec<&'a Student> {
    let query = fold(query);
    // What a student is ordered by, and found by beside the name.
    let read = |student: &Student| fold(student.reading.as_deref().unwrap_or(&student.name));
    let mut found: Vec<(String, &Student)> = students
        .iter()
        .filter(|student| student.squad == squad && filter.fits(student))
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
        let none = Filter::default();
        assert_eq!(
            ids(found(&students, "", &none, Squad::Main)),
            [3, 4, 5, 1, 2],
            "all of them by name: アル, トキ, はつねミク by its reading, ホシノ, ホシノ（水着）"
        );
        assert_eq!(
            ids(found(&students, "ほし", &none, Squad::Main)),
            [1, 2],
            "hiragana"
        );
        assert_eq!(
            ids(found(&students, "シノ", &none, Squad::Main)),
            [1, 2],
            "a part"
        );
        assert_eq!(
            ids(found(&students, "みく", &none, Squad::Main)),
            [5],
            "the name's kana"
        );
        assert_eq!(
            ids(found(&students, "はつね", &none, Squad::Main)),
            [5],
            "the reading"
        );
        assert!(found(&students, "ひな", &none, Squad::Main).is_empty());
    }

    #[test]
    fn only_the_squad_asked_for_is_found() {
        let students = students();
        let none = Filter::default();
        // アコ, the special, is not offered for a striker's place, by her name either.
        assert_eq!(ids(found(&students, "あ", &none, Squad::Main)), [3]);
        assert_eq!(ids(found(&students, "", &none, Squad::Support)), [6]);
        assert_eq!(ids(found(&students, "あこ", &none, Squad::Support)), [6]);
        assert!(found(&students, "ほしの", &none, Squad::Support).is_empty());
    }

    #[test]
    fn a_filter_takes_any_of_one_kind_and_every_kind() {
        let students = students();
        let mut filter = Filter::default();
        toggle(&mut filter.attacks, Attack::Pierce);
        assert_eq!(ids(found(&students, "", &filter, Squad::Main)), [4, 1]);
        // Another of the same kind: either fits.
        toggle(&mut filter.attacks, Attack::Explosion);
        assert_eq!(
            ids(found(&students, "", &filter, Squad::Main)),
            [3, 4, 5, 1, 2]
        );
        // Another kind: both have to fit.
        toggle(&mut filter.roles, Role::DamageDealer);
        assert_eq!(ids(found(&students, "", &filter, Squad::Main)), [3, 4, 2]);
        toggle(&mut filter.armors, Armor::ElasticArmor);
        assert_eq!(ids(found(&students, "", &filter, Squad::Main)), [4]);
        // With the name as well.
        assert!(found(&students, "ほしの", &filter, Squad::Main).is_empty());
        // Pressed again, a button is let up.
        toggle(&mut filter.armors, Armor::ElasticArmor);
        toggle(&mut filter.roles, Role::DamageDealer);
        toggle(&mut filter.attacks, Attack::Explosion);
        assert_eq!(filter.attacks, [Attack::Pierce]);
        assert_eq!(ids(found(&students, "ほしの", &filter, Squad::Main)), [1]);
    }
}
