//! Reading the formation dialog (編成) from a screen, and the opponent's name from the
//! opponent's dialog. The fixtures are 2370×1333 windows; like the battle records' screens
//! they are kept out of the repository (`tests/fixtures/`).

use bluearchive_rio::{
    formation::{Formation, Tab},
    history::Reader,
    opponent::{self, Front},
};
use image::{RgbImage, imageops};

fn screen(name: &str) -> RgbImage {
    let path = format!("tests/fixtures/{name}.png");
    image::open(&path)
        .unwrap_or_else(|error| panic!("{path} (kept out of the repository): {error}"))
        .to_rgb8()
}

/// A screen as a window `width` wide draws it.
fn drawn(screen: &RgbImage, width: u32) -> RgbImage {
    let height = (screen.height() as f32 * width as f32 / screen.width() as f32).round() as u32;
    imageops::resize(screen, width, height, imageops::FilterType::Triangle)
}

/// ユウカ in front, with ミチル（ドレス） and イロハ.
const FRONT: Front = Front {
    first: 13010,
    specials: [20016, 20053],
};
/// ホシノ, マリナ, ヒビキ（応援団） and ハスミ（水着）, with イロハ and ミチル（ドレス）.
const PLACED: [Option<i64>; 6] = [
    Some(10005),
    Some(10037),
    Some(16010),
    Some(10124),
    Some(20016),
    Some(20053),
];
/// The list at its top: the team's strikers as the list orders them, ハナコ（水着） and
/// イロハ（水着）, then ヒナ（ドレス）, ミカ, ノゾミ, ネル（制服）, コハル（水着） and
/// シロコ＊テラー. The third row is cut by the list's foot.
const CARDS: [i64; 12] = [
    10005, 10037, 10124, 16010, 10074, 10148, 10086, 10059, 10118, 10111, 16013, 10100,
];

fn students(shown: &Formation) -> Vec<i64> {
    shown.cards.iter().map(|card| card.student).collect()
}

#[test]
fn the_formation_dialog_is_read() {
    let shown = Reader::new()
        .formation(&screen("formation"))
        .expect("the dialog reads");
    assert_eq!(shown.front, FRONT);
    assert_eq!(shown.tab, Tab::Striker);
    assert_eq!(shown.placed, PLACED);
    assert_eq!(students(&shown), CARDS);
    let (first, last) = (shown.cards[0].middle, shown.cards[11].middle);
    assert!(
        (first.0 - 1144.0).abs() < 2.0 && (first.1 - 462.0).abs() < 2.0,
        "the first card's icon lies at {first:?}"
    );
    assert!(
        (last.0 - 2172.5).abs() < 2.0 && (last.1 - 744.0).abs() < 2.0,
        "the last card's icon lies at {last:?}"
    );
}

#[test]
fn other_window_sizes_read_the_same_or_nothing() {
    let (reader, dialog) = (Reader::new(), screen("formation"));
    for width in [1500, 1520, 1600, 1760, 1830, 1920, 2222] {
        let shown = reader.formation(&drawn(&dialog, width));
        if let Some(shown) = &shown {
            assert_eq!(
                (shown.front, shown.tab, shown.placed),
                (FRONT, Tab::Striker, PLACED),
                "at {width}"
            );
            // A card that does not read is left out; none is read as another student.
            let mut known = CARDS.iter();
            for student in students(shown) {
                assert!(
                    known.any(|card| *card == student),
                    "{student} out of its place at {width}"
                );
            }
        }
        assert!(
            shown.is_some() || ![1600, 1920].contains(&width),
            "the dialog reads at {width}"
        );
    }
}

#[test]
fn each_screen_is_read_as_itself_only() {
    let reader = Reader::new();
    assert_eq!(reader.formation(&screen("opponent")), None, "the opponent");
    assert_eq!(reader.formation(&screen("history")), None, "a record");
    assert_eq!(
        reader.front(&screen("formation")),
        None,
        "no opponent's dialog"
    );
    assert_eq!(reader.read(&screen("formation")), None, "no battle record");
}

#[test]
fn the_opponents_name_is_the_plate_of_the_battle_records() {
    let (reader, dialog) = (Reader::new(), screen("opponent"));
    let name = opponent::name(&dialog).expect("the opponent's name");
    // The user's own row, 368 lower, shows the name a battle record shows of its attacker.
    let mut own = dialog.clone();
    let line = imageops::crop_imm(&dialog, 470, 487 + 368, 460, 52).to_image();
    imageops::replace(&mut own, &line, 470, 487);
    let own = opponent::name(&own).expect("the user's own name");
    let record = reader.read(&screen("history")).expect("the record reads");
    let attacker = record.attacker.plate.expect("a named attacker");
    assert!(own.same(&attacker), "one name on both screens");
    assert!(!name.same(&attacker), "the opponent is another player");
    for width in [1600, 1920] {
        let small = opponent::name(&drawn(&dialog, width)).expect("the name at {width}");
        assert!(small.same(&name), "the same name at {width}");
    }
}
