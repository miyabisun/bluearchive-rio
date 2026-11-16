//! Reading the battle record (戦闘履歴) from a screen: every part is read right or the
//! record is not read at all. The fixture is a 2370×1333 window's record, blacked out
//! around the parts read; other window sizes are drawn from it.

use bluearchive_rio::history::{Battle, Fighter, Reader};
use image::{Rgb, RgbImage, imageops};

fn fixture() -> RgbImage {
    image::open("tests/fixtures/history.png")
        .expect("the fixture")
        .to_rgb8()
}

/// The fixture as a window `width` wide draws it.
fn drawn_at(width: u32) -> RgbImage {
    let fixture = fixture();
    let height = (fixture.height() as f32 * width as f32 / fixture.width() as f32).round() as u32;
    imageops::resize(&fixture, width, height, imageops::FilterType::Triangle)
}

fn team(fighters: [(i64, u32); 6]) -> [Fighter; 6] {
    fighters.map(|(student, damage)| Fighter { student, damage })
}

/// ホシノ, マリナ, ヒビキ（応援団）, ハスミ（水着） with イロハ and ミチル（ドレス）.
fn attack() -> [Fighter; 6] {
    team([
        (10005, 60812),
        (10037, 156_315),
        (16010, 720_028),
        (10124, 516_569),
        (20016, 543_051),
        (20053, 0),
    ])
}

/// アツコ, マリナ, トキ, イオリ with イロハ and ミチル（ドレス）.
fn defense() -> [Fighter; 6] {
    team([
        (10042, 162_341),
        (10037, 121_292),
        (10062, 194_176),
        (10006, 296_470),
        (20016, 573_064),
        (20053, 0),
    ])
}

fn assert_the_record(battle: &Battle, width: u32) {
    assert!(battle.attacker_won, "the attacker won, at {width}");
    assert_eq!(battle.attacker.team, attack(), "the attack, at {width}");
    assert_eq!(battle.defender.team, defense(), "the defense, at {width}");
}

#[test]
fn the_record_is_read_as_the_screen_shows_it() {
    let battle = Reader::new().read(&fixture()).expect("the record reads");
    assert_the_record(&battle, 2370);
}

#[test]
fn other_window_sizes_read_the_same_or_nothing() {
    let reader = Reader::new();
    // A Full HD screen, a window on one, and the least width read.
    for width in [1500, 1520, 1600, 1760, 1830, 1920, 2222] {
        let battle = reader.read(&drawn_at(width));
        if let Some(battle) = &battle {
            assert_the_record(battle, width);
        }
        assert!(
            battle.is_some() || ![1600, 1920].contains(&width),
            "the record reads at {width}"
        );
    }
}

#[test]
fn a_player_is_known_by_the_name_drawn() {
    let reader = Reader::new();
    let native = reader.read(&fixture()).expect("the record reads");
    let smaller = reader.read(&drawn_at(1920)).expect("the record reads");
    assert!(
        native.attacker.plate.same(&smaller.attacker.plate),
        "the attacker at another window size"
    );
    assert!(
        native.defender.plate.same(&smaller.defender.plate),
        "the defender at another window size"
    );
    assert!(
        !native.attacker.plate.same(&native.defender.plate),
        "the attacker is not the defender"
    );
}

#[test]
fn a_screen_without_the_record_is_not_read() {
    let reader = Reader::new();
    // The record's title covered: another dialog of the same layout.
    let mut covered = fixture();
    for y in 185..265 {
        for x in 1060..1310 {
            covered.put_pixel(x, y, Rgb([255, 255, 255]));
        }
    }
    assert_eq!(reader.read(&covered), None, "no title");
    assert_eq!(
        reader.read(&RgbImage::new(1920, 1080)),
        None,
        "a black screen"
    );
    assert_eq!(reader.read(&drawn_at(1280)), None, "too small a window");
}

#[test]
fn a_record_missing_a_part_is_not_read() {
    let reader = Reader::new();
    // A face that is no student's: the fourth striker of the attack.
    let mut unknown = fixture();
    for y in 985..1068 {
        for x in 619..701 {
            unknown.put_pixel(x, y, Rgb([120, 200, 90]));
        }
    }
    assert_eq!(reader.read(&unknown), None, "an unknown face");
    // Both sides winning: the defender's "Lose" painted as the attacker's "Win".
    let mut both = fixture();
    let win = imageops::crop_imm(&both, 215, 335, 225, 110).to_image();
    imageops::replace(&mut both, &win, 215 + 1179, 335);
    assert_eq!(reader.read(&both), None, "two winners");
}
