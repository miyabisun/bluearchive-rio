//! Reading the opponent's dialog (対戦相手) from a screen. The fixture is a 2370×1333
//! window's dialog, blacked out around its title and the opponent's cards; like the battle
//! records' screens it is kept out of the repository (`tests/fixtures/`).

use bluearchive_rio::{history::Reader, opponent::Front};
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

/// ユウカ in front, with ミチル（ドレス） and イロハ, who are kept the lower id first.
const FRONT: Front = Front {
    first: 13010,
    specials: [20016, 20053],
};

#[test]
fn the_front_of_the_defense_is_read() {
    let reader = Reader::new();
    assert_eq!(reader.front(&screen("opponent")), Some(FRONT));
}

#[test]
fn other_window_sizes_read_the_same_or_nothing() {
    let (reader, dialog) = (Reader::new(), screen("opponent"));
    // A Full HD screen, a window on one, the least width read and some between.
    for width in [1500, 1520, 1600, 1760, 1830, 1920, 2222] {
        let front = reader.front(&drawn(&dialog, width));
        assert!(
            front == Some(FRONT) || front.is_none(),
            "{front:?} at {width}"
        );
        assert!(
            front.is_some() || ![1600, 1920].contains(&width),
            "the dialog reads at {width}"
        );
    }
}

#[test]
fn each_screen_is_read_as_itself_only() {
    let reader = Reader::new();
    assert_eq!(reader.front(&screen("history")), None, "a battle record");
    assert_eq!(
        reader.front(&RgbImage::new(1920, 1080)),
        None,
        "a black screen"
    );
    assert_eq!(
        reader.read(&screen("opponent")),
        None,
        "the dialog is no battle record"
    );
}
