//! Watches Blue Archive's tactical challenge (戦術対抗戦, the 6 on 6 PvP) and keeps what it
//! shows: the battle records (`history`) in a database (`store`), told with the students of
//! the SchaleDB snapshot, and the students held as Aoi wrote them (`aoi`). The list the
//! records are opened from (`list`) tells roughly when each battle was: its rows are kept
//! too (`listing`), and a record is the battle of the row it was opened from (what was told
//! of them is written down, `trail`). Before a battle
//! it reads what the opponent's dialog shows of the defense (`opponent`), to look up what
//! may stand behind it in the records and which attacks won against that; the user chooses
//! among them in the window, and in the formation dialog (`formation`) Rio marks over the
//! game (`guide`, `overlay`) the students of the attack chosen.
//! The reading runs in a thread (`watch`) and shows in a window (`app`, `ui`, `view`) made as
//! Aoi's.

pub mod aoi;
pub mod app;
#[cfg(windows)]
pub mod capture;
pub mod draft;
pub mod formation;
pub mod guide;
pub mod history;
pub mod list;
pub mod listing;
pub mod mcp;
pub mod opponent;
pub mod overlay;
pub mod search;
pub mod settings;
pub mod source;
pub mod store;
pub mod tier;
pub mod trail;
pub mod ui;
pub mod update;
pub mod view;
pub mod watch;

use include_dir::{Dir, include_dir};
use serde::Deserialize;

static ICONS: Dir = include_dir!("$CARGO_MANIFEST_DIR/data/schaledb/student-icons");
static MARKS: Dir = include_dir!("$CARGO_MANIFEST_DIR/data/schaledb/ui");

/// The squad a student fights in: a team's four strikers (`Main`) stand in a row, its two
/// specials (`Support`) behind them.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Squad {
    Main,
    Support,
}

/// A student's class (SchaleDB's `TacticRole`), in the order the window offers them.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Tanker,
    DamageDealer,
    Healer,
    Supporter,
    /// T.S., the tactical support.
    Vehicle,
}

impl Role {
    pub const ALL: [Self; 5] = [
        Self::Tanker,
        Self::DamageDealer,
        Self::Healer,
        Self::Supporter,
        Self::Vehicle,
    ];

    /// The class as the game names it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Tanker => "タンク",
            Self::DamageDealer => "アタッカー",
            Self::Healer => "ヒーラー",
            Self::Supporter => "サポーター",
            Self::Vehicle => "TS",
        }
    }

    /// The name of the class's icon among the marks (see `mark`).
    pub fn mark(self) -> &'static str {
        match self {
            Self::Tanker => "Role_Tanker",
            Self::DamageDealer => "Role_DamageDealer",
            Self::Healer => "Role_Healer",
            Self::Supporter => "Role_Supporter",
            Self::Vehicle => "Role_Vehicle",
        }
    }
}

/// A student's attack type (SchaleDB's `BulletType`), in the order the window offers them.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attack {
    Explosion,
    Pierce,
    Mystic,
    Sonic,
}

impl Attack {
    pub const ALL: [Self; 4] = [Self::Explosion, Self::Pierce, Self::Mystic, Self::Sonic];

    pub fn name(self) -> &'static str {
        match self {
            Self::Explosion => "爆発",
            Self::Pierce => "貫通",
            Self::Mystic => "神秘",
            Self::Sonic => "振動",
        }
    }
}

/// A student's armor type (SchaleDB's `ArmorType`), in the order the window offers them.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Armor {
    LightArmor,
    HeavyArmor,
    CompositeArmor,
    /// 特殊装甲.
    Unarmed,
    ElasticArmor,
}

impl Armor {
    pub const ALL: [Self; 5] = [
        Self::LightArmor,
        Self::HeavyArmor,
        Self::CompositeArmor,
        Self::Unarmed,
        Self::ElasticArmor,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::LightArmor => "軽装備",
            Self::HeavyArmor => "重装甲",
            Self::CompositeArmor => "複合装甲",
            Self::Unarmed => "特殊装甲",
            Self::ElasticArmor => "弾力装甲",
        }
    }
}

/// The names of the icon the attack types share and of the one the armor types share
/// (see `mark`): SchaleDB tells a type by the colour under its icon.
pub const ATTACK_MARK: &str = "Type_Attack";
pub const ARMOR_MARK: &str = "Type_Defense";

/// A student of the SchaleDB snapshot (`data/schaledb/students.json`, written by
/// `tools/schaledb/update.ps1`).
#[derive(Deserialize, Debug, Clone)]
pub struct Student {
    pub id: i64,
    pub name: String,
    pub squad: Squad,
    pub role: Role,
    pub attack: Attack,
    pub armor: Armor,
    /// How the name is read, where it is not written in kana (初音ミク: はつねミク).
    pub reading: Option<String>,
}

/// Every student of the snapshot, by id.
pub fn students() -> Vec<Student> {
    serde_json::from_str(include_str!("../data/schaledb/students.json"))
        .expect("the snapshot is written by tools/schaledb/update.ps1")
}

/// A student's icon (WebP), the face the game draws for them.
pub fn icon(id: i64) -> Option<&'static [u8]> {
    ICONS
        .get_file(format!("{id}.webp"))
        .map(include_dir::File::contents)
}

/// The icon (PNG, white on nothing) SchaleDB shows a class or the types with: by
/// `Role::mark`, `ATTACK_MARK` or `ARMOR_MARK`.
pub fn mark(name: &str) -> Option<&'static [u8]> {
    MARKS
        .get_file(format!("{name}.png"))
        .map(include_dir::File::contents)
}

#[cfg(test)]
mod tests {
    use super::{ARMOR_MARK, ATTACK_MARK, Role, icon, mark, students};

    #[test]
    fn every_student_has_an_icon() {
        let students = students();
        assert!(students.len() > 200, "the snapshot lists the students");
        for student in &students {
            assert!(icon(student.id).is_some(), "{} has no icon", student.name);
        }
    }

    #[test]
    fn a_name_not_in_kana_has_its_reading() {
        // The variant in brackets aside (水着), a name is kana, or the snapshot tells how
        // it is read: the search takes kana alone.
        let kana = |name: &str| {
            let bare = name.split('（').next().unwrap_or(name);
            bare.chars().all(|c| ('\u{3040}'..='\u{30ff}').contains(&c))
        };
        let students = students();
        for student in &students {
            assert!(
                kana(&student.name) || student.reading.is_some(),
                "{} cannot be found by kana",
                student.name
            );
        }
        let miku = students.iter().find(|student| student.name == "初音ミク");
        assert_eq!(miku.unwrap().reading.as_deref(), Some("はつねミク"));
    }

    #[test]
    fn the_workflows_are_read_as_yaml() {
        // GitHub refuses a workflow it cannot read, and tells so only after the push.
        let folder = concat!(env!("CARGO_MANIFEST_DIR"), "/.github/workflows");
        for entry in std::fs::read_dir(folder).expect("the workflows' folder") {
            let path = entry.expect("a file").path();
            let text = std::fs::read_to_string(&path).expect("a workflow");
            let read = serde_yaml::from_str::<serde_yaml::Value>(&text);
            assert!(read.is_ok(), "{}: {:?}", path.display(), read.err());
        }
    }

    #[test]
    fn every_class_and_type_has_its_mark() {
        for role in Role::ALL {
            assert!(mark(role.mark()).is_some(), "{} has no icon", role.name());
        }
        assert!(mark(ATTACK_MARK).is_some() && mark(ARMOR_MARK).is_some());
        assert!(mark("Role_Nobody").is_none());
    }
}
