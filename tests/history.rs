//! Reading the battle record (戦闘履歴) from a screen: every part is read right or the
//! record is not read at all. The fixtures are a 2370×1333 window's records, blacked out
//! around the parts read; other window sizes are drawn from them. They show players' names,
//! so the repository does not hold them: they lie in `tests/fixtures/` where Rio is
//! developed, and these tests fail without them.

use bluearchive_rio::history::{Battle, Fighter, Party, Plate, Reader};
use image::{Rgb, RgbImage, imageops};

fn screen(name: &str) -> RgbImage {
    let path = format!("tests/fixtures/{name}.png");
    image::open(&path)
        .unwrap_or_else(|error| panic!("{path} (kept out of the repository): {error}"))
        .to_rgb8()
}

/// A battle between two players with names.
fn fixture() -> RgbImage {
    screen("history")
}

/// A battle the same attacker won against an anonymous player.
fn anonymous() -> RgbImage {
    screen("anonymous")
}

/// A battle the other fixtures' attacker defended and won: the shield is on the left.
fn defended() -> RgbImage {
    screen("defended")
}

/// A battle the other fixtures' attacker defended and lost: "Lose" on the left, and the
/// winner's side on the right, whose ground wavers a shade from frame to frame.
fn lost() -> RgbImage {
    screen("lost")
}

/// A battle the other fixtures' attacker attacked and lost: the sword and "Lose" on the
/// left. ツクヨ's icon is drawn smaller than the others' and 3 pixels right of their place.
fn attacked() -> RgbImage {
    screen("attacked")
}

/// A screen as a window `width` wide draws it.
fn drawn(screen: &RgbImage, width: u32) -> RgbImage {
    let height = (screen.height() as f32 * width as f32 / screen.width() as f32).round() as u32;
    imageops::resize(screen, width, height, imageops::FilterType::Triangle)
}

/// A Full HD screen, a window on one, the least width read and some between.
const WIDTHS: [u32; 7] = [1500, 1520, 1600, 1760, 1830, 1920, 2222];
/// The widths every record must read at.
const READ: [u32; 2] = [1600, 1920];

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

/// The record against the anonymous player: the same attack, and マリナ, ハスミ（水着）,
/// ユウカ, トキ with ミチル（ドレス） and イロハ, who stand the other way round on the screen.
fn assert_the_anonymous_record(battle: &Battle, width: u32) {
    assert!(battle.attacker_won, "the attacker won, at {width}");
    let attack = team([
        (10005, 283_324),
        (10037, 102_209),
        (16010, 752_576),
        (10124, 193_943),
        (20016, 586_821),
        (20053, 0),
    ]);
    let defense = team([
        (10037, 158_553),
        (10124, 301_369),
        (13010, 173_623),
        (10062, 433_794),
        (20016, 736_063),
        (20053, 0),
    ]);
    assert_eq!(battle.attacker.team, attack, "the attack, at {width}");
    assert_eq!(battle.defender.team, defense, "the defense, at {width}");
    assert!(
        battle.defender.plate.is_none(),
        "the \"?\" picture, at {width}"
    );
    assert!(battle.attacker.plate.is_some(), "a picture, at {width}");
}

/// The record of the defense: ヒフミ（レーシング）, アツコ, イオリ, マリナ with イロハ and
/// シロコ（水着） attacked マリナ, ホシノ, ヒビキ（応援団）, ハスミ（水着） with ミチル（ドレス）
/// and イロハ, and lost.
fn assert_the_defense(battle: &Battle, width: u32) {
    assert!(!battle.attacker_won, "the defender won, at {width}");
    let attack = team([
        (10153, 722_844),
        (10042, 76585),
        (10006, 144_654),
        (10037, 64881),
        (20016, 496_008),
        (20027, 10764),
    ]);
    let defense = team([
        (10037, 91633),
        (10005, 88761),
        (16010, 585_640),
        (10124, 433_802),
        (20016, 615_109),
        (20053, 0),
    ]);
    assert_eq!(battle.attacker.team, attack, "the attack, at {width}");
    assert_eq!(battle.defender.team, defense, "the defense, at {width}");
}

/// The record of the defense lost: シュン, ユウカ, ミカ, エイミ with ミチル（ドレス） and
/// イロハ attacked マリナ, ホシノ, ヒビキ（応援団）, ハスミ（水着） with ミチル（ドレス） and
/// イロハ, and won. A damage of seven digits is on it.
fn assert_the_defeat(battle: &Battle, width: u32) {
    assert!(battle.attacker_won, "the attacker won, at {width}");
    let attack = team([
        (10011, 202_130),
        (13010, 314_616),
        (10059, 380_795),
        (10001, 299_288),
        (20016, 431_559),
        (20053, 0),
    ]);
    let defense = team([
        (10037, 326_786),
        (10005, 39386),
        (16010, 1_018_418),
        (10124, 369_883),
        (20016, 790_292),
        (20053, 0),
    ]);
    assert_eq!(battle.attacker.team, attack, "the attack, at {width}");
    assert_eq!(battle.defender.team, defense, "the defense, at {width}");
}

/// The record of the attack lost: ハルナ（正月）, ツクヨ, マリナ, ハスミ（水着） with イロハ
/// and ミチル（ドレス） attacked マリナ, トキ, アツコ, ハスミ（水着） with イロハ and
/// ミチル（ドレス）, and lost.
fn assert_the_attack_lost(battle: &Battle, width: u32) {
    assert!(!battle.attacker_won, "the defender won, at {width}");
    let attack = team([
        (10057, 359_801),
        (10040, 27188),
        (10037, 26769),
        (10124, 162_153),
        (20016, 287_041),
        (20053, 0),
    ]);
    let defense = team([
        (10037, 107_534),
        (10062, 704_975),
        (10042, 112_135),
        (10124, 155_859),
        (20016, 534_835),
        (20053, 0),
    ]);
    assert_eq!(battle.attacker.team, attack, "the attack, at {width}");
    assert_eq!(battle.defender.team, defense, "the defense, at {width}");
}

/// Every window size reads a screen as `check` has it or does not read it, and the sizes
/// of `READ` do read it.
fn assert_at_other_sizes(screen: &RgbImage, check: fn(&Battle, u32)) {
    let reader = Reader::new();
    for width in WIDTHS {
        let battle = reader.read(&drawn(screen, width));
        if let Some(battle) = &battle {
            check(battle, width);
        }
        assert!(
            battle.is_some() || !READ.contains(&width),
            "the record reads at {width}"
        );
    }
}

fn plate(party: &Party) -> Plate {
    party.plate.clone().expect("a name is drawn")
}

#[test]
fn the_record_is_read_as_the_screen_shows_it() {
    let battle = Reader::new().read(&fixture()).expect("the record reads");
    assert_the_record(&battle, 2370);
}

#[test]
fn other_window_sizes_read_the_same_or_nothing() {
    assert_at_other_sizes(&fixture(), assert_the_record);
    assert_at_other_sizes(&anonymous(), assert_the_anonymous_record);
    assert_at_other_sizes(&defended(), assert_the_defense);
    assert_at_other_sizes(&lost(), assert_the_defeat);
    assert_at_other_sizes(&attacked(), assert_the_attack_lost);
}

#[test]
fn a_player_is_known_by_the_name_drawn() {
    let reader = Reader::new();
    let native = reader.read(&fixture()).expect("the record reads");
    let smaller = reader
        .read(&drawn(&fixture(), 1920))
        .expect("the record reads");
    assert!(
        plate(&native.attacker).same(&plate(&smaller.attacker)),
        "the attacker at another window size"
    );
    assert!(
        plate(&native.defender).same(&plate(&smaller.defender)),
        "the defender at another window size"
    );
    assert!(
        !plate(&native.attacker).same(&plate(&native.defender)),
        "the attacker is not the defender"
    );
}

#[test]
fn an_anonymous_player_is_read_as_nobody() {
    let reader = Reader::new();
    let battle = reader.read(&anonymous()).expect("the record reads");
    assert_the_anonymous_record(&battle, 2370);
    let named = reader.read(&fixture()).expect("the record reads");
    assert!(
        plate(&battle.attacker).same(&plate(&named.attacker)),
        "the attacker of the other record"
    );
}

#[test]
fn a_defense_is_read_with_the_shield_on_the_left() {
    let reader = Reader::new();
    let battle = reader.read(&defended()).expect("the record reads");
    assert_the_defense(&battle, 2370);
    let named = reader.read(&fixture()).expect("the record reads");
    assert!(
        plate(&battle.defender).same(&plate(&named.attacker)),
        "the defender attacked in the other record"
    );
    for known in [&named.attacker, &named.defender] {
        assert!(
            !plate(&battle.attacker).same(&plate(known)),
            "the attacker is a third player"
        );
    }
    // The right side's sword lies 4 short of where the left side's does: looked for at the
    // left side's place, it was not known at these widths.
    for width in [1560, 1806, 2208, 2340] {
        let battle = reader.read(&drawn(&defended(), width));
        assert_the_defense(&battle.expect("the record reads"), width);
    }
}

#[test]
fn an_attack_lost_is_read_with_an_icon_off_its_place() {
    let reader = Reader::new();
    let battle = reader.read(&attacked()).expect("the record reads");
    assert_the_attack_lost(&battle, 2370);
    let named = reader.read(&fixture()).expect("the record reads");
    assert!(
        plate(&battle.attacker).same(&plate(&named.attacker)),
        "the attacker of the other record"
    );
    assert!(
        !plate(&battle.defender).same(&plate(&named.defender)),
        "another defender"
    );
}

#[test]
fn a_defense_lost_is_read_with_lose_on_the_left() {
    let reader = Reader::new();
    let battle = reader.read(&lost()).expect("the record reads");
    assert_the_defeat(&battle, 2370);
    let won = reader.read(&defended()).expect("the record reads");
    assert!(
        plate(&battle.defender).same(&plate(&won.defender)),
        "the defender of the defense won"
    );
    assert!(
        !plate(&battle.attacker).same(&plate(&won.attacker)),
        "another attacker"
    );
}

#[test]
fn the_users_own_side_is_the_left_one() {
    let reader = Reader::new();
    let own = |screen: &RgbImage| reader.read(screen).expect("the record").user_attacked;
    assert!(own(&fixture()), "the sword on the left");
    assert!(own(&attacked()), "the sword and \"Lose\"");
    assert!(!own(&defended()), "the shield on the left");
    assert!(!own(&lost()), "the shield and \"Lose\"");
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
    assert!(!reader.titled(&covered), "nor is it known as a record");
    assert_eq!(
        reader.read(&RgbImage::new(1920, 1080)),
        None,
        "a black screen"
    );
    assert_eq!(
        reader.read(&drawn(&fixture(), 1280)),
        None,
        "too small a window"
    );
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
    assert!(
        reader.titled(&unknown),
        "still known as a record, by its title"
    );
    // Both sides winning: the defender's "Lose" painted as the attacker's "Win".
    let mut both = fixture();
    let win = imageops::crop_imm(&both, 215, 335, 225, 110).to_image();
    imageops::replace(&mut both, &win, 215 + 1179, 335);
    assert_eq!(reader.read(&both), None, "two winners");
}
