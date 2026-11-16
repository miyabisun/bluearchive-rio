//! The students held, as Aoi (bluearchive-aoi) read them from the game and wrote them to
//! `Documents\Aoi\aoi.yaml`: Rio reads that file and keeps no roster of its own.

use std::path::PathBuf;

use serde::Deserialize;

/// A student held. What Aoi could not read is `None`; so is the id of a student SchaleDB
/// does not list yet.
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Held {
    pub id: Option<i64>,
    pub name: String,
    pub level: Option<u8>,
    /// The stars, 1 to 5.
    pub rarity: Option<u8>,
}

#[derive(Deserialize)]
struct Export {
    students: Vec<Held>,
}

/// The students of Aoi's YAML.
pub fn roster(yaml: &str) -> Result<Vec<Held>, serde_yaml::Error> {
    serde_yaml::from_str::<Export>(yaml).map(|export| export.students)
}

/// Where Aoi keeps the latest of what it read.
pub fn latest() -> Option<PathBuf> {
    Some(dirs::document_dir()?.join("Aoi").join("aoi.yaml"))
}

#[cfg(test)]
mod tests {
    use super::{Held, roster};

    #[test]
    fn the_students_are_read_from_aois_yaml() {
        // The shape Aoi v0.3.33 writes, with the parts Rio does not use.
        let yaml = r#"# Aoi の読み取り結果 2026-11-17 02:24:32
credits: 196149017
students:
  - id: 10005
    name: "ホシノ"
    level: 90
    rarity: 5
    bond: 26
    skills: [5, 10, 10, 10]
    weapon: {stars: 4, level: 60}
    equipment: [{tier: 10}, {tier: 10}, null]
    gear: 2
    ability: [0, 25, 0]
  - id: null
    name: "ツムギ"
    level: null
    rarity: 3
    bond: null
    skills: [null, 1, 1, 1]
    weapon: locked
    equipment: [null, null, null]
    gear: 0
    ability: [0, 0, 0]
items:
  - id: 10
    name: "初級レポート"
    count: 51K
"#;
        assert_eq!(
            roster(yaml).unwrap(),
            [
                Held {
                    id: Some(10005),
                    name: "ホシノ".to_owned(),
                    level: Some(90),
                    rarity: Some(5),
                },
                Held {
                    id: None,
                    name: "ツムギ".to_owned(),
                    level: None,
                    rarity: Some(3),
                },
            ]
        );
    }

    #[test]
    fn a_file_that_is_not_aois_is_refused() {
        assert!(roster("credits: 1\n").is_err());
    }
}
