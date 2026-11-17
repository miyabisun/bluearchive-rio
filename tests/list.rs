//! Reading the list of battle records (戦闘履歴, the dialog that lists the battles) from a
//! screen: a row is read right or not at all. The fixtures are a 2370×1333 window's lists,
//! blacked out around the parts read; other window sizes are drawn from them. They show
//! players' names, so the repository does not hold them: they lie in `tests/fixtures/` where
//! Rio is developed, and these tests fail without them.

use bluearchive_rio::{
    history::Reader,
    list::{Row, Shown, Unit},
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

/// A Full HD screen, a window on one, the least width read and some between.
const WIDTHS: [u32; 7] = [1500, 1520, 1600, 1760, 1830, 1920, 2222];
/// The widths every row must read at.
const READ: [u32; 2] = [1600, 1920];

/// What a row tells but for the opponent's name: defended or attacked, won or lost, whether
/// the opponent has a name, and the time.
type Told = (bool, bool, bool, u32, Unit);

fn told(row: &Row) -> Told {
    (
        row.attacked,
        row.won,
        row.opponent.is_some(),
        row.ago.count,
        row.ago.unit,
    )
}

/// The list at its top: three rows whole, the fourth cut by the list's end. The user
/// defended thrice: won against an anonymous player 5 minutes ago, lost 2 hours ago and won
/// 5 hours ago against players with names.
const TOP: [(f32, Told); 3] = [
    (398.0, (false, true, false, 5, Unit::Minute)),
    (573.0, (false, false, true, 2, Unit::Hour)),
    (747.0, (false, true, true, 5, Unit::Hour)),
];

/// The list scrolled by 22 pixels: the first row cut by the list's top, two rows whole (a
/// defense won 22 hours ago, and one won against an anonymous player 23 hours ago), the
/// fourth cut by the list's end.
const SCROLLED: [(f32, Told); 2] = [
    (551.0, (false, true, true, 22, Unit::Hour)),
    (726.0, (false, true, false, 23, Unit::Hour)),
];

/// Another list, scrolled: two attacks won 21 hours ago, the first against an anonymous
/// player. No reference is cut from this screen, and none of the others shows a "1": its
/// reference is drawn from the countdown's digits (see `make_refs`).
const ATTACKS: [(f32, Told); 2] = [
    (560.0, (true, true, false, 21, Unit::Hour)),
    (735.0, (true, true, true, 21, Unit::Hour)),
];

/// The list right after a battle: the attack just won tells "0分前", under attacks 2 and 5
/// minutes old. No reference is cut from this screen either.
const FRESH: [(f32, Told); 3] = [
    (398.0, (true, true, true, 0, Unit::Minute)),
    (572.0, (true, true, true, 2, Unit::Minute)),
    (747.0, (true, false, true, 5, Unit::Minute)),
];

/// Three attacks 2, 3 and 6 minutes old, the last two against one player. No reference is
/// cut from this screen, and no screen the references are cut from shows a "6".
const SIX: [(f32, Told); 3] = [
    (398.0, (true, true, true, 2, Unit::Minute)),
    (573.0, (true, true, true, 3, Unit::Minute)),
    (747.0, (true, false, true, 6, Unit::Minute)),
];

/// The list's end: three attacks won 6 days ago. The reference of "日" is cut from another
/// screen of the same list, scrolled elsewhere.
const END: [(f32, Told); 3] = [
    (509.0, (true, true, true, 6, Unit::Day)),
    (684.0, (true, true, true, 6, Unit::Day)),
    (858.0, (true, true, true, 6, Unit::Day)),
];

fn assert_rows(shown: &[Shown], rows: &[(f32, Told)], what: &str) {
    let tops: Vec<f32> = shown.iter().map(|shown| shown.top.round()).collect();
    let expected: Vec<f32> = rows.iter().map(|(top, _)| *top).collect();
    assert_eq!(tops, expected, "{what}: the rows in full sight");
    for (shown, (top, row)) in shown.iter().zip(rows) {
        let read = shown.row.as_ref().map(told);
        assert_eq!(read, Some(*row), "{what}: the row at {top}");
    }
}

#[test]
fn the_rows_in_full_sight_are_read() {
    let reader = Reader::new();
    let shown = reader.list(&screen("list")).expect("the list");
    assert_rows(&shown, &TOP, "the list at its top");
    let shown = reader.list(&screen("list-scrolled")).expect("the list");
    assert_rows(&shown, &SCROLLED, "the list scrolled");
    let shown = reader.list(&screen("list-attacks")).expect("the list");
    assert_rows(&shown, &ATTACKS, "a list of attacks");
    let shown = reader.list(&screen("list-fresh")).expect("the list");
    assert_rows(&shown, &FRESH, "the list right after a battle");
    let shown = reader.list(&screen("list-six")).expect("the list");
    assert_rows(&shown, &SIX, "a row six minutes old");
    let shown = reader.list(&screen("list-end")).expect("the list");
    assert_rows(&shown, &END, "rows days old, at the list's end");
}

#[test]
fn two_rows_of_two_players_show_two_names() {
    let reader = Reader::new();
    let shown = reader.list(&screen("list")).expect("the list");
    let name = |at: usize| {
        let row = shown[at].row.as_ref().expect("the row reads");
        row.opponent.clone().expect("an opponent with a name")
    };
    assert!(!name(1).same(&name(2)), "two players");
    assert!(name(1).same(&name(1)), "one player");
    // Two rows of one player.
    let shown = reader.list(&screen("list-six")).expect("the list");
    let name = |at: usize| {
        let row = shown[at].row.as_ref().expect("the row reads");
        row.opponent.clone().expect("an opponent with a name")
    };
    assert!(name(1).same(&name(2)), "one player on two rows");
    assert!(!name(0).same(&name(1)), "and another");
}

#[test]
fn other_window_sizes_read_the_same_or_nothing() {
    let reader = Reader::new();
    let lists = [
        ("list", &TOP[..]),
        ("list-scrolled", &SCROLLED[..]),
        ("list-attacks", &ATTACKS[..]),
        ("list-fresh", &FRESH[..]),
        ("list-six", &SIX[..]),
        ("list-end", &END[..]),
    ];
    for (name, rows) in lists {
        let screen = screen(name);
        for width in WIDTHS {
            let shown = reader
                .list(&drawn(&screen, width))
                .unwrap_or_else(|| panic!("{name} at {width}: the list"));
            assert_eq!(shown.len(), rows.len(), "{name} at {width}: the rows");
            for (shown, (top, row)) in shown.iter().zip(rows) {
                assert!(
                    (shown.top - top).abs() <= 2.0,
                    "{name} at {width}: the row at {top} lies at {}",
                    shown.top
                );
                let read = shown.row.as_ref().map(told);
                if READ.contains(&width) {
                    assert_eq!(read, Some(*row), "{name} at {width}: the row at {top}");
                } else {
                    assert!(
                        read.is_none() || read == Some(*row),
                        "{name} at {width}: the row at {top} read wrong: {read:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn a_record_is_not_a_list_and_a_list_no_record() {
    let reader = Reader::new();
    for record in ["history", "attacked"] {
        assert!(reader.list(&screen(record)).is_none(), "{record}: a record");
    }
    for list in ["list", "list-scrolled"] {
        let list = screen(list);
        assert!(!reader.titled(&list), "a list is no record");
        assert!(reader.read(&list).is_none());
    }
}
