//! The formation dialog (編成) of the tactical challenge, where the attacking team is
//! chosen: the opponent's front again (the "VS" row), the team as it stands, and the list
//! of the students held, on the tab of the strikers or of the specials. Read as the battle
//! record is: right, or not at all. The parts lie where they were measured on a 2370×1333
//! window (2026-11-17).

use image::{Rgb, RgbImage};

use crate::{
    Squad,
    history::{Area, LIKE, Reader, Sheet, likeness},
    opponent::{CARD_ROWS, Front, title_shape},
};

/// The list shows the strikers or the specials.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Striker,
    Special,
}

/// A card of the list: its student, and the middle of its icon on the measured screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Card {
    pub student: i64,
    pub middle: (f32, f32),
}

/// What the dialog shows.
#[derive(Debug, Clone, PartialEq)]
pub struct Formation {
    pub front: Front,
    pub tab: Tab,
    /// The team as it stands: the four strikers' places, then the specials'. `None` for a
    /// place that shows no student.
    pub placed: [Option<i64>; 6],
    /// The list's cards in sight, in the list's order.
    pub cards: Vec<Card>,
}

/// The middles of the icons of the "VS" row: D1 and the two specials, then the three
/// strikers the game hides. Their icons fitted 116 wide.
const FRONT: [(f32, f32); 3] = [(232.0, 875.0), (773.0, 875.0), (906.0, 875.0)];
pub(crate) const HIDDEN: [(f32, f32); 3] = [(365.0, 875.0), (498.0, 875.0), (631.0, 875.0)];
pub(crate) const FRONT_SIDE: f32 = 116.0;
/// The middles of the icons of the team's row, and the side they fitted (163 to 175).
pub(crate) const TEAM: [(f32, f32); 6] = [
    (156.0, 1111.0),
    (347.0, 1111.0),
    (540.0, 1111.0),
    (730.0, 1111.0),
    (937.0, 1111.0),
    (1131.0, 1111.0),
];
pub(crate) const TEAM_SIDE: f32 = 169.0;
/// The tabs "STRIKER" and "SPECIAL".
pub(crate) const TABS: [Area; 2] = [
    (1037.0, 259.0, 1254.0, 330.0),
    (1256.0, 259.0, 1472.0, 330.0),
];
/// The rows the list shows its cards in: it scrolls by any number of pixels.
pub(crate) const LIST: (f32, f32) = (351.0, 962.0);
/// The middle of the first column's icons, the step to the next column and to the next
/// row, and a side between those the icons fitted (143 to 155: `Reader::face` tries a
/// square a little smaller and larger).
const COLUMN: f32 = 1144.0;
const COLUMN_STEP: f32 = 205.7;
const ROW_STEP: f32 = 282.0;
const CARD_SIDE: f32 = 147.0;
/// A card's name stands on a navy label under its icon: `LABEL_TALL` tall, its top
/// `LABEL_BELOW` under the icon's middle. `LABEL_X` crosses the first column's labels left
/// of any letter.
const LABEL_X: f32 = 1080.0;
const LABEL_TALL: f32 = 66.0;
const LABEL_BELOW: f32 = 96.0;
/// A card around its icon's middle: to the sides, above and below.
pub(crate) const CARD_AROUND: (f32, f32, f32) = (98.0, 90.0, 178.0);
/// The rows of an icon the list must show for it to be read: `CARD_ROWS` of it with the
/// room `Reader::face` tries it in.
const ICON_IN_SIGHT: (f32, f32) = (42.0, 36.0);

/// The navy of a name's label.
fn navy(pixel: &Rgb<u8>) -> bool {
    let [r, g, b] = pixel.0.map(i32::from);
    (r - 67).abs() <= 20 && (g - 82).abs() <= 20 && (b - 102).abs() <= 20
}

/// A tab is open when its label is filled with its colour: this share of it at least is
/// not the pale ground of a closed tab. The open "STRIKER" measured 0.80, the closed
/// "SPECIAL" 0.20 (its letters); "SPECIAL" open is not measured yet.
const FILLED: f32 = 0.5;

/// The share of a tab's label that is not pale.
fn filled(label: &RgbImage) -> f32 {
    let dyed = label
        .pixels()
        .filter(|pixel| pixel.0.into_iter().min().unwrap_or(255) < 190)
        .count();
    dyed as f32 / (label.width() * label.height()) as f32
}

/// The tab that is open, from how much of each label is filled: one and only one.
fn open(filled: [f32; 2]) -> Option<Tab> {
    match filled.map(|share| share >= FILLED) {
        [true, false] => Some(Tab::Striker),
        [false, true] => Some(Tab::Special),
        _ => None,
    }
}

/// Where a label starts in a column of the list's pixels, `navy` or not: the first run of
/// navy as tall as a label (`tall` pixels) that the list shows whole. Something else navy,
/// or a label cut by the list's edge, is another height.
fn label_top(navy: &[bool], tall: f32) -> Option<usize> {
    let slack = (tall / 10.0).max(2.0);
    let mut start = None;
    for (at, &inked) in navy.iter().enumerate() {
        match (start, inked) {
            (None, true) => start = Some(at),
            (Some(from), false) => {
                if from > 0 && ((at - from) as f32 - tall).abs() <= slack {
                    return Some(from);
                }
                start = None;
            }
            _ => {}
        }
    }
    None
}

/// The middles of the icons of the rows the list shows enough of to read, given the top of
/// one of its labels.
fn rows(label: f32) -> Vec<f32> {
    (-3..=3)
        .map(|row| label - LABEL_BELOW + row as f32 * ROW_STEP)
        .filter(|middle| middle - ICON_IN_SIGHT.0 >= LIST.0 && middle + ICON_IN_SIGHT.1 <= LIST.1)
        .collect()
}

impl Reader {
    /// What a frame's formation dialog shows, or `None` for any other screen and for a
    /// dialog whose front, tab or list does not read. A card of the list that does not
    /// read is left out.
    pub fn formation(&self, frame: &RgbImage) -> Option<Formation> {
        if likeness(&title_shape(frame)?, &self.forming) < LIKE {
            return None;
        }
        let sheet = Sheet::of(frame)?;
        let face = |middle, side, squad| self.face(&sheet, middle, side, &CARD_ROWS, squad);
        let mut specials = [
            face(FRONT[1], FRONT_SIDE, Squad::Support)?,
            face(FRONT[2], FRONT_SIDE, Squad::Support)?,
        ];
        specials.sort_unstable();
        let front = Front {
            first: face(FRONT[0], FRONT_SIDE, Squad::Main)?,
            specials,
        };
        let [strikers, others] = TABS.map(|tab| sheet.crop(tab, 0.0));
        let tab = open([filled(&strikers?), filled(&others?)])?;
        let mut placed = [None; 6];
        for (slot, place) in placed.iter_mut().enumerate() {
            let squad = if slot < 4 {
                Squad::Main
            } else {
                Squad::Support
            };
            *place = face(TEAM[slot], TEAM_SIDE, squad);
        }

        // The rows are found by the first column's labels: every row has a card there.
        let ((x, top), (_, bottom)) =
            (sheet.spot((LABEL_X, LIST.0)), sheet.spot((LABEL_X, LIST.1)));
        let (x, top, bottom) = (x.round() as u32, top.round() as u32, bottom.round() as u32);
        if x >= frame.width() || bottom > frame.height() {
            return None;
        }
        let column: Vec<bool> = (top..bottom).map(|y| navy(frame.get_pixel(x, y))).collect();
        let label = label_top(&column, LABEL_TALL * sheet.scale)?;
        let label = LIST.0 + label as f32 / sheet.scale;
        let squad = match tab {
            Tab::Striker => Squad::Main,
            Tab::Special => Squad::Support,
        };
        let mut cards = Vec::new();
        for row in rows(label) {
            for column in 0..6 {
                let middle = (COLUMN + column as f32 * COLUMN_STEP, row);
                if let Some(student) = face(middle, CARD_SIDE, squad) {
                    cards.push(Card { student, middle });
                }
            }
        }
        Some(Formation {
            front,
            tab,
            placed,
            cards,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Tab, label_top, open, rows};

    #[test]
    fn the_open_tab_is_the_one_filled() {
        assert_eq!(open([0.80, 0.20]), Some(Tab::Striker));
        assert_eq!(open([0.20, 0.80]), Some(Tab::Special));
        assert_eq!(open([0.80, 0.80]), None, "both filled: another screen");
        assert_eq!(open([0.20, 0.20]), None, "neither");
    }

    #[test]
    fn a_label_is_found_by_its_height() {
        let column = |runs: &[(usize, usize)]| {
            let mut column = vec![false; 600];
            for &(from, to) in runs {
                column[from..to].fill(true);
            }
            column
        };
        assert_eq!(label_top(&column(&[(200, 266)]), 66.0), Some(200));
        assert_eq!(
            label_top(&column(&[(0, 40), (256, 322)]), 66.0),
            Some(256),
            "a label cut by the list's top is passed over"
        );
        assert_eq!(
            label_top(&column(&[(100, 130), (300, 364)]), 66.0),
            Some(300),
            "dark hair is another height"
        );
        assert_eq!(label_top(&column(&[(100, 130)]), 66.0), None);
        assert_eq!(
            label_top(&column(&[(560, 600)]), 66.0),
            None,
            "cut by the bottom"
        );
        // A smaller window draws a smaller label.
        assert_eq!(label_top(&column(&[(120, 162)]), 41.8), Some(120));
    }

    #[test]
    fn the_rows_in_sight_follow_the_scroll() {
        // The list at its top: two rows whole, the third cut.
        assert_eq!(rows(558.0), [462.0, 744.0]);
        // Scrolled 160 up: the first row's icon is cut, the third row's is in sight.
        assert_eq!(rows(398.0), [584.0, 866.0]);
    }
}
