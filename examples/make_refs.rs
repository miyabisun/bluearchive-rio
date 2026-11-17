//! Cuts the references the reader compares a battle record with from screens showing
//! one, into `data/`: the title, the attacker's sword and the defender's shield, the
//! digits of the damage labels, and the "?" picture of an anonymous player.
//!   cargo run --example make_refs
//! The screen is `tests/fixtures/history.png`: the attacker on the left, whose labels show
//! every digit. The right side's labels are left out: the tests read them. The "?" is the
//! defender's picture in `tests/fixtures/anonymous.png`, and the opponent's dialog's title
//! that of `tests/fixtures/opponent.png`.

use bluearchive_rio::{
    history::{digit_cells, icon_shape, role_shape, title_shape},
    opponent,
};
use image::{GrayImage, imageops};

/// The widths the digits are drawn at: a smaller window blurs them, and the references
/// are their mean over the sizes read. From the 2370 wide screen alone, 4 of the 88 widths
/// from 1500 to 2370 (every 10 pixels) left a digit unread; drawn so, 1 did.
const WIDTHS: [u32; 4] = [1500, 1800, 2100, 2370];

/// The damage of the fixture's left side, slot by slot.
const DAMAGE: [&str; 6] = ["60812", "156315", "720028", "516569", "543051", "0"];

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
}
