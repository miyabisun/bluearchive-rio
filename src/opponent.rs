//! The opponent's dialog (対戦相手) before a battle of the tactical challenge: of the
//! defense it shows the first striker and the two specials, and hides the strikers behind
//! them. Read as the battle record is: right, or not at all.

use std::ops::Range;

use image::{GrayImage, RgbImage};

use crate::{
    Squad,
    history::{Area, LIKE, Plate, Reader, Sheet, TITLE_SIZE, ink, likeness, plate, shape},
};

/// What the dialog shows of the opponent's defense: D1 and the specials, the lower id
/// first (see `history::Team`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Front {
    pub first: i64,
    pub specials: [i64; 2],
}

/// "対戦相手", measured on a 2370×1333 window as the record's parts are.
const TITLE: Area = (1070.0, 145.0, 1300.0, 215.0);
/// The middle of the icon on the first card of the opponent's row, the step to the next
/// card and the side of the square the icon is drawn in: on 2026-11-17 nine cards fitted
/// their students' icons 100 to 107 wide, 61 to 62 right and 48 to 51 below a card's corner.
pub(crate) const CARD: (f32, f32) = (1046.5, 381.0);
pub(crate) const CARD_STEP: f32 = 120.0;
pub(crate) const CARD_SIDE: f32 = 101.0;
/// The rows of an icon (of 24) a card shows clear of "Lv.90" above, and of the star and
/// the role's mark below.
pub(crate) const CARD_ROWS: Range<u32> = 6..17;
/// The line "Lv.90 name" under the opponent's cards. The dialog draws it as large as the
/// battle record draws its own (a name's letters measured 28, 31 and 28 wide and 33 tall
/// on both), so the two screens' plates of one name compare.
const NAME: Area = (482.0, 487.0, 910.0, 539.0);

/// The shape of the title's place, for `make_refs` and the reader.
pub fn title_shape(frame: &RgbImage) -> Option<GrayImage> {
    let sheet = Sheet::of(frame)?;
    Some(shape(&sheet.crop(TITLE, 0.0)?, ink, TITLE_SIZE))
}

/// The opponent's name as a frame's opponent dialog draws it: the plate the battle records
/// know a player by. The dialog is told by `Reader::front`.
pub fn name(frame: &RgbImage) -> Option<Plate> {
    let sheet = Sheet::of(frame)?;
    plate(&sheet.crop(NAME, 0.0)?, sheet.scale)
}

impl Reader {
    /// The front of the defense a frame's opponent dialog shows, or `None` for any other
    /// screen and for a dialog with a card that does not read.
    pub fn front(&self, frame: &RgbImage) -> Option<Front> {
        if likeness(&title_shape(frame)?, &self.front) < LIKE {
            return None;
        }
        let sheet = Sheet::of(frame)?;
        let card = |slot: usize, squad| {
            let middle = (CARD.0 + slot as f32 * CARD_STEP, CARD.1);
            self.face(&sheet, middle, CARD_SIDE, &CARD_ROWS, squad)
        };
        let mut specials = [card(4, Squad::Support)?, card(5, Squad::Support)?];
        specials.sort_unstable();
        Some(Front {
            first: card(0, Squad::Main)?,
            specials,
        })
    }
}
