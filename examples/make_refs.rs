//! Cuts the references the reader compares a battle record with from screens showing
//! one, into `data/`: the title, the attacker's sword and the defender's shield, the
//! digits of the damage labels, and the "?" picture of an anonymous player.
//!   cargo run --example make_refs
//! The screen is `tests/fixtures/history.png`: the attacker on the left, whose labels show
//! every digit. The right side's labels are left out: the tests read them. The "?" is the
//! defender's picture in `tests/fixtures/anonymous.png`, the opponent's dialog's title
//! that of `tests/fixtures/opponent.png`, and the formation dialog's (it lies in the same
//! place) that of `tests/fixtures/formation.png`.
//!
//! The letters of the time a row of the list of records tells ("5分前") are cut from the
//! rows of `tests/fixtures/list.png`, `list-scrolled.png` and `list-days.png`. Those rows
//! show the digits 0, 2, 3, 5 and 6 only: the others are drawn from the countdown the game
//! shows over the list
//! (`tests/fixtures/timer.png`, ten frames of it, the last digit 0 to 9), whose digits are
//! the same letters drawn bolder and smaller.
//!
//! With the folder of Noto Sans JP's fonts (`NotoSansJP-Bold.otf` from
//! [noto-cjk](https://github.com/notofonts/noto-cjk/tree/main/Sans/SubsetOTF/JP), under the
//! SIL Open Font License) the words Rio says over the game are drawn too, into `data/words`:
//!   cargo run --example make_refs -- <folder of the fonts>

use ab_glyph::{Font, FontVec, PxScale, ScaleFont, point};
use bluearchive_rio::{
    guide::Words,
    history::{digit_cells, icon_shape, role_shape, title_shape},
    list::{self, DIGIT_SIZE, DIGIT_WIDE, KANJI, KANJI_SIZE},
    opponent,
};
use image::{GrayImage, Luma, RgbImage, imageops};

/// The widths the digits are drawn at: a smaller window blurs them, and the references
/// are their mean over the sizes read. From the 2370 wide screen alone, 4 of the 88 widths
/// from 1500 to 2370 (every 10 pixels) left a digit unread; drawn so, 1 did.
const WIDTHS: [u32; 4] = [1500, 1800, 2100, 2370];

/// The damage of the fixture's left side, slot by slot.
const DAMAGE: [&str; 6] = ["60812", "156315", "720028", "516569", "543051", "0"];

/// The rows of the list's fixtures: a row's top, and the time it tells.
const LISTS: [(&str, &[(f32, &str)]); 3] = [
    (
        "list",
        &[(398.0, "5分前"), (573.0, "2時間前"), (747.0, "5時間前")],
    ),
    (
        "list-scrolled",
        &[
            (376.0, "20時間前"),
            (551.0, "22時間前"),
            (726.0, "23時間前"),
        ],
    ),
    (
        "list-days",
        &[(396.0, "6日前"), (570.0, "6日前"), (745.0, "6日前")],
    ),
];

/// A frame of the countdown in `timer.png`, and the columns and rows of it the digits lie
/// in.
const TIMER_FRAME: u32 = 65;
const TIMER_DIGITS: (u32, u32, u32, u32) = (40, 10, 132, 52);
/// The countdown's digits beside a row's: 0.86 as large, and bolder by 0.1 of a pixel's
/// edge. Measured on 2026-11-18 against the rows' own 0, 2, 3 and 5: drawn so, a countdown
/// digit lay 15.9 at most from the row's same digit and 33.1 at least from another; as it
/// is (0.91 as large), 24.4 and 39.5.
const TIMER_SCALE: f32 = 0.86;
const TIMER_BOLDER: f32 = 0.1;
/// The brightest channel of a row's letters and of the ground behind them.
const ROW_INK: (f32, f32) = (60.0, 241.0);

/// The mean of the pictures added, one for each of some letters.
struct Means {
    sums: Vec<Vec<u32>>,
    counts: Vec<u32>,
    size: (u32, u32),
}

impl Means {
    fn new(letters: usize, size: (u32, u32)) -> Self {
        Self {
            sums: vec![vec![0; (size.0 * size.1) as usize]; letters],
            counts: vec![0; letters],
            size,
        }
    }

    fn add(&mut self, letter: usize, picture: &GrayImage) {
        assert_eq!(picture.dimensions(), self.size);
        for (sum, pixel) in self.sums[letter].iter_mut().zip(picture.iter()) {
            *sum += u32::from(*pixel);
        }
        self.counts[letter] += 1;
    }

    /// The means side by side, of the letters up to the first that has no picture.
    fn strip(&self) -> GrayImage {
        let known = self.counts.iter().take_while(|count| **count > 0).count();
        assert!(
            self.counts[known..].iter().all(|count| *count == 0),
            "a letter before one that has pictures has none"
        );
        let (width, height) = self.size;
        GrayImage::from_fn(width * known as u32, height, |x, y| {
            let letter = (x / width) as usize;
            let at = (y * width + x % width) as usize;
            Luma([(self.sums[letter][at] / self.counts[letter]) as u8])
        })
    }
}

/// The last digit of a frame of the countdown as a row `scale` large would draw it: the
/// digits thinned to a row's weight, on a row's ground.
fn timer_digit(timer: &RgbImage, frame: u32, scale: f32) -> GrayImage {
    let (x0, y0, x1, y1) = TIMER_DIGITS;
    let (width, height) = (x1 - x0, y1 - y0);
    let value = |x: u32, y: u32| {
        let pixel = timer.get_pixel(x0 + x, frame * TIMER_FRAME + y0 + y);
        f32::from(pixel.0.into_iter().max().unwrap_or(0))
    };
    let mut values: Vec<f32> = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .map(|(x, y)| value(x, y))
        .collect();
    values.sort_by(f32::total_cmp);
    let (letters, ground) = (values[values.len() / 50], values[values.len() / 2]);
    // How much of each pixel the letters cover; then blurred, so an edge slopes over two
    // pixels, and the slope moved inwards.
    let cover = |x: i64, y: i64| -> f32 {
        if x < 0 || y < 0 || x >= i64::from(width) || y >= i64::from(height) {
            return 0.0;
        }
        ((ground - value(x as u32, y as u32)) / (ground - letters).max(1.0)).clamp(0.0, 1.0)
    };
    let drawn = GrayImage::from_fn(width, height, |x, y| {
        let mut blurred = 0.0;
        for (down, tall) in [(-1, 0.25), (0, 0.5), (1, 0.25)] {
            for (across, wide) in [(-1, 0.25), (0, 0.5), (1, 0.25)] {
                blurred += cover(i64::from(x) + across, i64::from(y) + down) * tall * wide;
            }
        }
        let thin = ((blurred - 0.5) * 2.0 + 0.5 - TIMER_BOLDER).clamp(0.0, 1.0);
        Luma([(ROW_INK.1 - thin * (ROW_INK.1 - ROW_INK.0)).round() as u8])
    });
    let drawn = imageops::resize(
        &drawn,
        (width as f32 * scale).round() as u32,
        (height as f32 * scale).round() as u32,
        imageops::FilterType::Triangle,
    );
    // As `history::ink` has a row's letters.
    let ink = GrayImage::from_fn(drawn.width(), drawn.height(), |x, y| {
        let inked = (190 - i32::from(drawn.get_pixel(x, y).0[0])) * 2;
        Luma([inked.clamp(0, 255) as u8])
    });
    // The last digit: the last run of inked columns.
    let inked = |x: &u32| (0..ink.height()).any(|y| ink.get_pixel(*x, y).0[0] >= 128);
    let end = (0..ink.width()).rfind(inked).expect("the countdown") + 1;
    let start = (0..end).rfind(|x| !inked(x)).map_or(0, |blank| blank + 1);
    list::letter(
        &ink,
        (start, end),
        DIGIT_WIDE,
        TIMER_SCALE * scale,
        DIGIT_SIZE,
    )
    .expect("a digit")
}

/// How tall the pictures of the words are, and the size of their letters in them.
const WORDS_TALL: u32 = 64;
const WORDS_EM: f32 = 44.0;

/// Words as a picture: how much of each pixel the letters cover, the line as wide as the
/// words.
fn words(font: &FontVec, text: &str) -> GrayImage {
    let size = PxScale::from(
        WORDS_EM * font.height_unscaled() / font.units_per_em().expect("a font with an em"),
    );
    let scaled = font.as_scaled(size);
    // A kanji's middle lies 0.38 of the letters' size above the line it stands on.
    let line = WORDS_TALL as f32 / 2.0 + WORDS_EM * 0.38;
    let mut pen = 0.0;
    let mut glyphs = Vec::new();
    for letter in text.chars() {
        let glyph = scaled.glyph_id(letter);
        glyphs.push(glyph.with_scale_and_position(size, point(pen, line)));
        pen += scaled.h_advance(glyph);
    }
    let mut picture = GrayImage::new(pen.ceil() as u32, WORDS_TALL);
    for glyph in glyphs {
        let Some(outline) = font.outline_glyph(glyph) else {
            continue;
        };
        let bounds = outline.px_bounds();
        outline.draw(|x, y, cover| {
            let (x, y) = (
                bounds.min.x as i64 + i64::from(x),
                bounds.min.y as i64 + i64::from(y),
            );
            if x >= 0 && y >= 0 && x < i64::from(picture.width()) && y < i64::from(WORDS_TALL) {
                let pixel = picture.get_pixel_mut(x as u32, y as u32);
                pixel.0[0] = pixel.0[0].max((cover * 255.0).round() as u8);
            }
        });
    }
    picture
}

fn main() {
    let screen = image::open("tests/fixtures/history.png")
        .expect("the fixture")
        .to_rgb8();
    let save = |shape: Option<GrayImage>, name: &str| {
        shape
            .unwrap_or_else(|| panic!("the screen shows no {name}"))
            .save(format!("data/{name}.png"))
            .expect("data/ is written");
    };
    save(title_shape(&screen), "title");
    save(role_shape(&screen, false), "attack");
    save(role_shape(&screen, true), "defense");
    let hidden = image::open("tests/fixtures/anonymous.png")
        .expect("the fixture")
        .to_rgb8();
    save(icon_shape(&hidden, true), "anonymous");
    let dialog = image::open("tests/fixtures/opponent.png")
        .expect("the fixture")
        .to_rgb8();
    save(opponent::title_shape(&dialog), "opponent");
    let forming = image::open("tests/fixtures/formation.png")
        .expect("the fixture")
        .to_rgb8();
    save(opponent::title_shape(&forming), "formation");

    // Each digit is the mean of the cells showing it.
    let mut sums: Vec<Vec<u32>> = vec![Vec::new(); 10];
    let mut counts = [0u32; 10];
    let (mut width, mut height) = (0, 0);
    for drawn in WIDTHS {
        let tall = (screen.height() as f32 * drawn as f32 / screen.width() as f32).round() as u32;
        let screen = imageops::resize(&screen, drawn, tall, imageops::FilterType::Triangle);
        for (slot, shown) in DAMAGE.iter().enumerate() {
            let cells = digit_cells(&screen, false, slot).expect("a label over every slot");
            assert_eq!(
                cells.len(),
                shown.len(),
                "the label over slot {slot} at {drawn}"
            );
            for (cell, digit) in cells.iter().zip(shown.bytes()) {
                let digit = usize::from(digit - b'0');
                (width, height) = cell.dimensions();
                sums[digit].resize(cell.len(), 0);
                for (sum, pixel) in sums[digit].iter_mut().zip(cell.iter()) {
                    *sum += u32::from(*pixel);
                }
                counts[digit] += 1;
            }
        }
    }
    let mut digits = GrayImage::new(width * 10, height);
    for (digit, (sum, count)) in sums.iter().zip(counts).enumerate() {
        assert!(count > 0, "no label shows the digit {digit}");
        for (at, sum) in sum.iter().enumerate() {
            let (x, y) = (at as u32 % width, at as u32 / width);
            digits.put_pixel(
                digit as u32 * width + x,
                y,
                image::Luma([(sum / count) as u8]),
            );
        }
    }
    save(Some(digits), "digits");

    // The times of the list's rows: the digits the rows show, the other digits from the
    // countdown, and the kanji.
    let fixture = |name: &str| {
        image::open(format!("tests/fixtures/{name}.png"))
            .unwrap_or_else(|error| panic!("the fixture {name}: {error}"))
            .to_rgb8()
    };
    let (mut digits, mut kanji) = (
        Means::new(10, DIGIT_SIZE),
        Means::new(KANJI.len(), KANJI_SIZE),
    );
    for (name, rows) in LISTS {
        let screen = fixture(name);
        for drawn in WIDTHS {
            let tall =
                (screen.height() as f32 * drawn as f32 / screen.width() as f32).round() as u32;
            let screen = imageops::resize(&screen, drawn, tall, imageops::FilterType::Triangle);
            for (top, shown) in rows {
                let words = shown.chars().filter(|letter| !letter.is_ascii_digit());
                let letters = list::ago_letters(&screen, *top, words.count())
                    .unwrap_or_else(|| panic!("{shown} at {drawn}"));
                assert_eq!(letters.len(), shown.chars().count(), "{shown} at {drawn}");
                for (letter, shown) in letters.iter().zip(shown.chars()) {
                    match shown.to_digit(10) {
                        Some(digit) => digits.add(digit as usize, letter),
                        None => {
                            let at = KANJI.iter().position(|kanji| *kanji == shown);
                            kanji.add(at.expect("a kanji of a time"), letter);
                        }
                    }
                }
            }
        }
    }
    let timer = fixture("timer");
    for digit in 0..10 {
        if digits.counts[digit] == 0 {
            for drawn in WIDTHS {
                let letter = timer_digit(&timer, digit as u32, drawn as f32 / 2370.0);
                digits.add(digit, &letter);
            }
        }
    }
    save(Some(digits.strip()), "ago-digits");
    save(Some(kanji.strip()), "ago-kanji");

    // The words Rio says over the game.
    if let Some(fonts) = std::env::args().nth(1) {
        let path = format!("{fonts}/NotoSansJP-Bold.otf");
        let font = std::fs::read(&path).unwrap_or_else(|error| panic!("{path}: {error}"));
        let font = FontVec::try_from_vec(font).expect("a font");
        std::fs::create_dir_all("data/words").expect("data/ is written");
        for said in Words::ALL {
            save(
                Some(words(&font, said.text())),
                &format!("words/{}", said.name()),
            );
        }
    }
}
