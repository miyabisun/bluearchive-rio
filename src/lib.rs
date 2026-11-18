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
pub mod formation;
pub mod guide;
pub mod history;
pub mod list;
pub mod listing;
pub mod opponent;
pub mod overlay;
pub mod settings;
pub mod store;
pub mod trail;
pub mod ui;
pub mod update;
pub mod view;
pub mod watch;

use include_dir::{Dir, include_dir};
use serde::Deserialize;

static ICONS: Dir = include_dir!("$CARGO_MANIFEST_DIR/data/schaledb/student-icons");

/// The squad a student fights in: a team's four strikers (`Main`) stand in a row, its two
/// specials (`Support`) behind them.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Squad {
    Main,
    Support,
}

/// A student of the SchaleDB snapshot (`data/schaledb/students.json`, written by
/// `tools/schaledb/update.ps1`).
#[derive(Deserialize, Debug, Clone)]
pub struct Student {
    pub id: i64,
    pub name: String,
    pub squad: Squad,
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

#[cfg(test)]
mod tests {
    use super::{icon, students};

    #[test]
    fn every_student_has_an_icon() {
        let students = students();
        assert!(students.len() > 200, "the snapshot lists the students");
        for student in &students {
            assert!(icon(student.id).is_some(), "{} has no icon", student.name);
        }
    }
}
