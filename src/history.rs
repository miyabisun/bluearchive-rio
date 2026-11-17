//! The battle record (戦闘履歴) of the tactical challenge, read from a frame of the game:
//! who attacked whom, who won, and each team's six students with the damage they dealt.
//!
//! Every part is read right or the record is not read at all: a record is kept for good,
//! so a guess would stay. The parts lie where they were measured on a 2370×1333 window
//! (2026-11-17), scaled about the frame's middle; what is compared is brought to one size
//! first, so a window of another size reads the same.
//!
//! The references in `data/` (the title, the two roles' pictures and the digits) are cut
//! from a screen by `cargo run --example make_refs`; the students' faces are SchaleDB's icons.

use std::{
    ops::Range,
    time::{Duration, Instant},
};

use image::{
    GrayImage, Luma, Rgb, RgbImage,
    imageops::{self, FilterType},
};

use crate::Squad;

/// A student of a team and the damage dealt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fighter {
    pub student: i64,
    pub damage: u32,
}

/// A team in the players' order: the strikers from the left (A1 to A4 attacking, D1 to D4
/// defending), then the two specials, the lower id first: which of the two stands left
/// makes no difference in battle, so two teams of the same students are the same team.
pub type Team = [Fighter; 6];

/// A player's name as the record draws it, `PLATE_HEIGHT` tall. A name is not read into
/// letters (any letter may be in one): a player is known by the picture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plate(pub GrayImage);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Party {
    /// The name drawn, or `None` for an anonymous player: one the game shows behind a "?"
    /// icon. Two such players are not known to be one.
    pub plate: Option<Plate>,
    pub team: Team,
}

/// A battle. The two sides do not mirror each other (the field differs), so a team is only
/// ever an attack or a defense.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Battle {
    pub attacker: Party,
    pub defender: Party,
    pub attacker_won: bool,
    /// Whether the user attacked: a record shows the user's own side on the left, attack
    /// or defense.
    pub user_attacked: bool,
}

impl Battle {
    /// Whether two readings are of one record: the students, the damage and the winner as
    /// they are, and the names drawn the same (`Plate::same`). The winner's side wavers a
    /// shade from frame to frame, and a name on it is never twice the same pixels.
    pub fn alike(&self, other: &Self) -> bool {
        let same = |a: &Party, b: &Party| {
            a.team == b.team
                && match (&a.plate, &b.plate) {
                    (Some(a), Some(b)) => a.same(b),
                    (None, None) => true,
                    _ => false,
                }
        };
        self.attacker_won == other.attacker_won
            && self.user_attacked == other.user_attacked
            && same(&self.attacker, &other.attacker)
            && same(&self.defender, &other.defender)
    }
}

/// The screen the parts were measured on.
const MEASURED: (f32, f32) = (2370.0, 1333.0);
/// The least width a screen is read at, as Aoi's.
const SMALLEST: f32 = 1500.0;
/// The right side's parts lie this far right of the left side's.
const SIDE: f32 = 1179.0;

/// A part of the measured screen: left, top, right, bottom.
pub(crate) type Area = (f32, f32, f32, f32);
/// "戦闘履歴".
const TITLE: Area = (1070.0, 190.0, 1300.0, 260.0);
/// The sword of the attacker or the shield of the defender. The right side's lies
/// `ROLE_SHORT` short of `SIDE` to the right: on three screens the sword spanned 112 to 190
/// on the left and 108 to 186 past `SIDE` on the right, the shield 120 to 182 and 116 to
/// 179. Looked for a full `SIDE` to the right, the sword was 0.87 to 0.90 like itself.
const ROLE: Area = (100.0, 335.0, 200.0, 435.0);
const ROLE_SHORT: f32 = 4.0;
/// "Win" or "Lose".
const RESULT: Area = (215.0, 335.0, 440.0, 445.0);
/// The middle of the player's picture: of an anonymous player's, the "?" in its circle.
const ICON: Area = (507.0, 330.0, 607.0, 430.0);
/// The line "Lv.90 name".
const NAME: Area = (657.0, 312.0, 1085.0, 364.0);
/// The middle of a side's first face, the step to the next, and the side of the square
/// SchaleDB's icon is drawn in.
const FACE: (f32, f32) = (242.0, 1026.0);
const FACE_STEP: f32 = 139.3;
const FACE_SIDE: f32 = 82.0;
/// The rows a damage label lies in, above its bar, and how far it reaches either side of
/// its face's middle at most (short of the next label).
const LABELS: (f32, f32) = (480.0, 965.0);
const LABEL_HALF: f32 = 69.0;

/// The sizes the parts are compared at.
pub(crate) const TITLE_SIZE: (u32, u32) = (46, 14);
pub(crate) const ROLE_SIZE: (u32, u32) = (16, 16);
pub(crate) const ICON_SIZE: (u32, u32) = (16, 16);
const DIGIT_SIZE: (u32, u32) = (10, 14);
const FACE_SIZE: u32 = 24;
const PLATE_HEIGHT: u32 = 32;

/// A frame and how large it draws the game: the measured screen's parts scale with the
/// width of a 16:9 window (Aoi's `screen::unit`) about the frame's middle.
pub(crate) struct Sheet<'a> {
    pub(crate) frame: &'a RgbImage,
    pub(crate) scale: f32,
}

/// How large a screen of this size draws the game beside the measured one; `None` for a
/// screen too small to read, or upright.
pub(crate) fn scale(width: u32, height: u32) -> Option<f32> {
    let (width, height) = (width as f32, height as f32);
    let wide = width * 9.0 / 16.0;
    let unit = if (wide - height).abs() < 1.0 {
        height
    } else {
        wide
    };
    let drawn = unit * 16.0 / 9.0;
    (width > height && drawn >= SMALLEST).then_some(drawn / MEASURED.0)
}

/// Where a point of the measured screen lies on a screen of `size` drawn at `scale`.
pub(crate) fn spot((width, height): (u32, u32), scale: f32, (x, y): (f32, f32)) -> (f32, f32) {
    (
        width as f32 / 2.0 + (x - MEASURED.0 / 2.0) * scale,
        height as f32 / 2.0 + (y - MEASURED.1 / 2.0) * scale,
    )
}

/// The point of the measured screen that lies at a point of a screen of `size` drawn at
/// `scale`: `spot` the other way.
pub(crate) fn measured((width, height): (u32, u32), scale: f32, (x, y): (f32, f32)) -> (f32, f32) {
    (
        (x - width as f32 / 2.0) / scale + MEASURED.0 / 2.0,
        (y - height as f32 / 2.0) / scale + MEASURED.1 / 2.0,
    )
}

impl<'a> Sheet<'a> {
    /// `None` for a screen `scale` does not read.
    pub(crate) fn of(frame: &'a RgbImage) -> Option<Self> {
        scale(frame.width(), frame.height()).map(|scale| Self { frame, scale })
    }

    /// Where a point of the measured screen lies in the frame.
    pub(crate) fn spot(&self, point: (f32, f32)) -> (f32, f32) {
        spot(self.frame.dimensions(), self.scale, point)
    }

    /// The frame's pixels of `area`, moved `shift` to the right.
    pub(crate) fn crop(&self, (left, top, right, bottom): Area, shift: f32) -> Option<RgbImage> {
        let ((x0, y0), (x1, y1)) = (
            self.spot((left + shift, top)),
            self.spot((right + shift, bottom)),
        );
        let (x0, y0, x1, y1) = (x0.round(), y0.round(), x1.round(), y1.round());
        let inside = x0 >= 0.0
            && y0 >= 0.0
            && x1 <= self.frame.width() as f32
            && y1 <= self.frame.height() as f32
            && x1 > x0
            && y1 > y0;
        inside.then(|| {
            imageops::crop_imm(
                self.frame,
                x0 as u32,
                y0 as u32,
                (x1 - x0) as u32,
                (y1 - y0) as u32,
            )
            .to_image()
        })
    }
}

/// A pixel's brightest and darkest channel.
fn range(pixel: &Rgb<u8>) -> (i32, i32) {
    let [r, g, b] = pixel.0.map(i32::from);
    (r.max(g).max(b), r.min(g).min(b))
}

/// The dark grey of a damage label, and of the outline of "Lose".
fn dark(pixel: &Rgb<u8>) -> bool {
    let (max, min) = range(pixel);
    max < 100 && max - min < 30
}

/// How much of a pixel is the dark ink of the title and the names: none of the white and
/// the pink behind them, nor of the flowers around a player's picture. By degrees: cut at
/// one darkness, a letter's thin strokes came and went with the window's size.
pub(crate) fn ink(pixel: &Rgb<u8>) -> u8 {
    ((190 - range(pixel).0) * 2).clamp(0, 255) as u8
}
/// A column of a name's line holds a letter from this much ink.
pub(crate) const INKED: u8 = 128;

/// The yellow of "Win".
fn yellow(pixel: &Rgb<u8>) -> bool {
    let [r, g, b] = pixel.0.map(i32::from);
    r > 200 && g > 130 && b < 120 && r - b > 110
}

/// How blue a pixel is: the sword and the shield, on the attacker's white and the
/// defender's pink alike.
pub(crate) fn blue(pixel: &Rgb<u8>) -> u8 {
    (i32::from(pixel.0[2]) - i32::from(pixel.0[0])).clamp(0, 255) as u8
}

/// How much of `of` each pixel of `area` is, as a picture of `size`: a shape to compare
/// whatever the window's size.
pub(crate) fn shape(
    area: &RgbImage,
    of: fn(&Rgb<u8>) -> u8,
    (width, height): (u32, u32),
) -> GrayImage {
    let drawn = GrayImage::from_fn(area.width(), area.height(), |x, y| {
        Luma([of(area.get_pixel(x, y))])
    });
    imageops::resize(&drawn, width, height, FilterType::Triangle)
}

/// How alike two pictures of one size are: their correlation, -1 for a flat one.
pub(crate) fn likeness(a: &GrayImage, b: &GrayImage) -> f32 {
    let n = a.len() as f32;
    let (mut sa, mut sb, mut saa, mut sbb, mut sab) = (0.0f32, 0.0, 0.0, 0.0, 0.0);
    for (a, b) in a.iter().zip(b.iter()) {
        let (a, b) = (f32::from(*a), f32::from(*b));
        sa += a;
        sb += b;
        saa += a * a;
        sbb += b * b;
        sab += a * b;
    }
    let (var_a, var_b) = (saa - sa * sa / n, sbb - sb * sb / n);
    if var_a <= 0.0 || var_b <= 0.0 {
        return -1.0;
    }
    (sab - sa * sb / n) / (var_a * var_b).sqrt()
}

/// How far two pictures of one size are apart: the mean difference of their pixels.
pub(crate) fn distance(a: &GrayImage, b: &GrayImage) -> f32 {
    let sum: u32 = a
        .iter()
        .zip(b.iter())
        .map(|(a, b)| u32::from(a.abs_diff(*b)))
        .sum();
    sum as f32 / a.len() as f32
}

/// The nearest of `distances` if it is near enough and leads the next clearly: its index.
pub(crate) fn clear(distances: impl Iterator<Item = f32>, near: f32) -> Option<usize> {
    let (mut best, mut next) = ((f32::MAX, 0), f32::MAX);
    for (at, distance) in distances.enumerate() {
        if distance < best.0 {
            next = best.0;
            best = (distance, at);
        } else if distance < next {
            next = distance;
        }
    }
    (best.0 <= near && best.0 <= LEAD * next).then_some(best.1)
}

/// The nearest reference is taken when no other is within this share of its distance
/// (Aoi's ratio for its letters).
const LEAD: f32 = 0.6;

/// The shape of the title's place, for `make_refs` and the reader.
pub fn title_shape(frame: &RgbImage) -> Option<GrayImage> {
    let sheet = Sheet::of(frame)?;
    Some(shape(&sheet.crop(TITLE, 0.0)?, ink, TITLE_SIZE))
}

/// The shape of a side's role picture, for `make_refs` and the reader.
pub fn role_shape(frame: &RgbImage, right: bool) -> Option<GrayImage> {
    let sheet = Sheet::of(frame)?;
    let shift = if right { SIDE - ROLE_SHORT } else { 0.0 };
    Some(shape(&sheet.crop(ROLE, shift)?, blue, ROLE_SIZE))
}

/// The shape of the middle of a side's player picture, for `make_refs` and the reader.
pub fn icon_shape(frame: &RgbImage, right: bool) -> Option<GrayImage> {
    let sheet = Sheet::of(frame)?;
    Some(shape(&sheet.crop(ICON, shift(right))?, blue, ICON_SIZE))
}

fn shift(right: bool) -> f32 {
    if right { SIDE } else { 0.0 }
}

/// A label is `LABEL_BODY` tall, its digits in its middle, `DIGIT_STEP` apart and
/// `DIGIT_TALL` tall, with `LABEL_PAD` on either side. With the pointer under it, a label
/// is `LABEL_TALL.0` to `LABEL_TALL.1` tall.
const LABEL_BODY: f32 = 30.0;
const LABEL_PAD: f32 = 9.2;
const DIGIT_STEP: f32 = 16.3;
const DIGIT_TALL: f32 = 25.0;
const LABEL_TALL: (f32, f32) = (30.0, 42.0);
/// A label's width may be this far (in digits) off a whole number of digits.
const DIGIT_SLACK: f32 = 0.25;
/// A label lies at any fraction of a pixel, so its digits are cut on a label drawn this
/// many times as large, each about the middle of its own white: cut at whole pixels off
/// the label's edge, a "9" lay 21 from its reference where the next digit lay 29.
const FINE: u32 = 4;
/// A label's digits are white from this brightness.
const WHITE: u8 = 200;

/// The digits of the damage label over a side's `slot` (0 to 5), each brought to
/// `DIGIT_SIZE`, for `make_refs` and the reader. The label is the one dark thing in its
/// column: the bars are red and blue.
pub fn digit_cells(frame: &RgbImage, right: bool, slot: usize) -> Option<Vec<GrayImage>> {
    let sheet = Sheet::of(frame)?;
    let middle = FACE.0 + slot as f32 * FACE_STEP;
    let column = sheet.crop(
        (middle - LABEL_HALF, LABELS.0, middle + LABEL_HALF, LABELS.1),
        shift(right),
    )?;
    let (mut left, mut top, mut end, mut bottom) = (u32::MAX, u32::MAX, 0, 0);
    for (x, y, pixel) in column.enumerate_pixels() {
        if dark(pixel) {
            left = left.min(x);
            top = top.min(y);
            end = end.max(x + 1);
            bottom = bottom.max(y + 1);
        }
    }
    if end == 0 {
        return None;
    }
    let scale = sheet.scale;
    let tall = (bottom - top) as f32 / scale;
    let digits = ((end - left) as f32 / scale - LABEL_PAD * 2.0) / DIGIT_STEP;
    let count = digits.round();
    if !(LABEL_TALL.0..=LABEL_TALL.1).contains(&tall)
        || count < 1.0
        || (digits - count).abs() > DIGIT_SLACK
    {
        return None;
    }
    let body = ((LABEL_BODY * scale).round() as u32).min(column.height() - top);
    let label = imageops::grayscale(&*imageops::crop_imm(&column, left, top, end - left, body));
    let fine = imageops::resize(
        &label,
        label.width() * FINE,
        label.height() * FINE,
        FilterType::Triangle,
    );
    let (step, tall) = (
        DIGIT_STEP * scale * FINE as f32,
        DIGIT_TALL * scale * FINE as f32,
    );
    // Every digit is as tall as the others: the rows from the first to the last white one,
    // looked for between the label's rounded corners.
    let pad = (LABEL_PAD * scale * FINE as f32 / 2.0) as u32;
    let white = |y: &u32| (pad..fine.width() - pad).any(|x| fine.get_pixel(x, *y).0[0] >= WHITE);
    let (first, last) = (
        (0..fine.height()).find(white)?,
        (0..fine.height()).rfind(white)?,
    );
    let y = (first + last + 1) as f32 / 2.0 - tall / 2.0;
    (0..count as usize)
        .map(|digit| {
            // The digit's place by the label's edges, then the middle of the white in it.
            let from = fine.width() as f32 / 2.0 + (digit as f32 - count / 2.0) * step;
            let (x0, x1) = (
                from.round() as u32,
                ((from + step).round() as u32).min(fine.width()),
            );
            let (mut white, mut across) = (0.0f32, 0.0f32);
            for y in first..=last {
                for x in x0..x1 {
                    let lit = f32::from(fine.get_pixel(x, y).0[0].saturating_sub(128));
                    white += lit;
                    across += lit * x as f32;
                }
            }
            let x = across / white - step / 2.0;
            let (width, height) = (step.round() as u32, tall.round() as u32);
            let inside = white > 0.0
                && x >= 0.0
                && y >= 0.0
                && x.round() as u32 + width <= fine.width()
                && y.round() as u32 + height <= fine.height();
            inside.then(|| {
                let cell =
                    imageops::crop_imm(&fine, x.round() as u32, y.round() as u32, width, height);
                imageops::resize(&*cell, DIGIT_SIZE.0, DIGIT_SIZE.1, FilterType::Triangle)
            })
        })
        .collect()
}

/// A gap this wide (on the measured screen) parts the level from the name: within
/// "Lv.90" and within a name the gaps measured 8 at most, between the two 17 at least.
const NAME_GAP: f32 = 13.0;

/// The name of the line "Lv.90 name" in `line`: what follows the first wide gap.
pub(crate) fn plate(line: &RgbImage, scale: f32) -> Option<Plate> {
    let inked: Vec<bool> = (0..line.width())
        .map(|x| (0..line.height()).any(|y| ink(line.get_pixel(x, y)) >= INKED))
        .collect();
    let end = inked.iter().rposition(|&inked| inked)? + 1;
    let gap = (NAME_GAP * scale).round() as usize;
    let mut blank = 0;
    let mut seen = false;
    let start = (0..end).find(|&x| {
        if inked[x] {
            let starts = seen && blank >= gap;
            (seen, blank) = (true, 0);
            starts
        } else {
            blank += 1;
            false
        }
    })?;
    let name =
        imageops::crop_imm(line, start as u32, 0, (end - start) as u32, line.height()).to_image();
    let width = (name.width() as f32 * PLATE_HEIGHT as f32 / line.height() as f32).round() as u32;
    Some(Plate(shape(&name, ink, (width.max(1), PLATE_HEIGHT))))
}

/// Two plates of one name may differ this much in width, and in this many strays.
const PLATE_SLACK: u32 = 3;
const PLATE_STRAYS: u32 = 4;

impl Plate {
    /// Whether two plates draw the same name: nearly as wide, and every stroke of the one
    /// lies on or beside a stroke of the other. A window's size moves a stroke's edge by a
    /// pixel; another letter leaves whole strokes without a partner, however long the name.
    pub fn same(&self, other: &Plate) -> bool {
        let (a, b) = (&self.0, &other.0);
        if a.height() != b.height() || a.width().abs_diff(b.width()) > PLATE_SLACK {
            return false;
        }
        let b = imageops::resize(b, a.width(), a.height(), FilterType::Triangle);
        strays(a, &b) + strays(&b, a) <= PLATE_STRAYS
    }

    /// The plate as a PNG, to keep.
    pub fn png(&self) -> Vec<u8> {
        let mut png = Vec::new();
        self.0
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("a picture in memory is written");
        png
    }

    /// A plate kept as a PNG.
    pub fn from_png(png: &[u8]) -> Option<Self> {
        Some(Self(image::load_from_memory(png).ok()?.to_luma8()))
    }
}

/// The clearly inked pixels of `a` with no ink of `b` on or beside them.
fn strays(a: &GrayImage, b: &GrayImage) -> u32 {
    let (width, height) = (a.width() as i64, a.height() as i64);
    let near = |x: i64, y: i64| {
        (y - 1..=y + 1).any(|y| {
            (x - 1..=x + 1).any(|x| {
                x >= 0
                    && y >= 0
                    && x < width
                    && y < height
                    && b.get_pixel(x as u32, y as u32).0[0] >= 64
            })
        })
    };
    a.enumerate_pixels()
        .filter(|(x, y, pixel)| pixel.0[0] >= 160 && !near(i64::from(*x), i64::from(*y)))
        .count() as u32
}

/// "Win" is a fifth yellow and "Lose" has none; "Lose" is an eighth dark grey (its
/// outline), measured over `RESULT`.
const WIN_YELLOW: f32 = 0.1;
const LOSE_YELLOW: f32 = 0.005;
const LOSE_DARK: f32 = 0.05;

/// Whether the side won, from the word in `area`.
pub(crate) fn won(area: &RgbImage) -> Option<bool> {
    let share = |is: fn(&Rgb<u8>) -> bool| {
        area.pixels().filter(|pixel| is(pixel)).count() as f32
            / (area.width() * area.height()) as f32
    };
    let (yellow, dark) = (share(yellow), share(dark));
    if yellow >= WIN_YELLOW {
        Some(true)
    } else if yellow <= LOSE_YELLOW && dark >= LOSE_DARK {
        Some(false)
    } else {
        None
    }
}

/// The specials in the order a team is kept in (see `Team`).
fn in_order(mut team: Team) -> Team {
    if team[4].student > team[5].student {
        team.swap(4, 5);
    }
    team
}

/// A student's face: SchaleDB's icon at `FACE_SIZE`, its opaque middle (the game draws a
/// pattern behind the icon and cuts its corners).
struct Face {
    student: i64,
    squad: Squad,
    /// Where in the picture, and the colour.
    pixels: Vec<(usize, [u8; 3])>,
}

impl Face {
    fn of(student: &crate::Student) -> Self {
        let icon = crate::icon(student.id).expect("tools/schaledb/update.ps1 fetches every icon");
        let icon = image::load_from_memory(icon)
            .expect("an icon is a picture")
            .to_rgba8();
        let icon = imageops::resize(&icon, FACE_SIZE, FACE_SIZE, FilterType::Triangle);
        let mut pixels = Vec::new();
        for y in FACE_SIZE / 8..FACE_SIZE * 7 / 8 {
            for x in FACE_SIZE / 5..FACE_SIZE * 4 / 5 {
                let [r, g, b, alpha] = icon.get_pixel(x, y).0;
                if alpha >= 250 {
                    pixels.push(((y * FACE_SIZE + x) as usize, [r, g, b]));
                }
            }
        }
        Self {
            student: student.id,
            squad: student.squad,
            pixels,
        }
    }

    /// The mean difference from a square of the screen brought to `FACE_SIZE`, over the
    /// `rows` of the icon the screen shows.
    fn distance(&self, patch: &[[u8; 3]], rows: &Range<u32>) -> f32 {
        let (mut sum, mut count) = (0u32, 0u32);
        for (at, colour) in &self.pixels {
            if rows.contains(&(*at as u32 / FACE_SIZE)) {
                sum += (0..3)
                    .map(|c| u32::from(colour[c].abs_diff(patch[*at][c])))
                    .sum::<u32>();
                count += 3;
            }
        }
        sum as f32 / count.max(1) as f32
    }
}

/// Measured on 2026-11-17 over the fixture drawn 1500 to 2370 wide, every 3 pixels (291
/// widths, none read wrong, 2 not read for a digit short of its lead):
/// a face lay 7.9 at most from its student's icon and 15.9 at least from any other;
const FACE_NEAR: f32 = 10.0;
/// a digit 18.8 at most from its reference;
const DIGIT_NEAR: f32 = 25.0;
/// the title was 0.989 like its reference at least, a role's picture 0.99 like its own
/// and 0.63 like the other at most.
pub(crate) const LIKE: f32 = 0.9;
/// A player's picture this little like the "?" is another picture; one between is not read.
/// Over two screens drawn at the same 291 widths, the "?" was 0.987 like its reference at
/// least, and two players' own pictures 0.034 at most.
pub(crate) const UNLIKE: f32 = 0.5;

/// Where a face is looked for about its place: the sizes of the square against the usual
/// one, and the pixels off it across and down.
struct Places {
    sizes: &'static [f32],
    off: &'static [f32],
}

/// An icon's face fills its square more or less (マリナ's fitted 86 wide where the others
/// fitted 80 to 83).
const NEAR_BY: Places = Places {
    sizes: &[0.94, 1.0, 1.06],
    off: &[-1.5, 0.0, 1.5],
};
/// Looked at when the places near by do not tell the student: ツクヨ's icon fitted 79 wide,
/// 3 right of its place and 1 up. Measured on 2026-11-17: near by it lay 10.3 from her icon
/// (22.7 from the next student's), here 4.3.
const FARTHER: Places = Places {
    sizes: &[0.91, 0.94, 0.97, 1.0, 1.03, 1.06, 1.09],
    off: &[-4.5, -3.0, -1.5, 0.0, 1.5, 3.0, 4.5],
};

/// Only the students whose icons lie within this near by are looked for farther off: so a
/// square that shows no student costs no more, and one that shows a student a little.
const FACE_FAR: f32 = 25.0;

/// What a side of the record shows.
struct Side {
    attacks: bool,
    won: bool,
    party: Party,
}

/// Reads battle records with the references built into the program.
pub struct Reader {
    pub(crate) title: GrayImage,
    /// The attacker's sword and the defender's shield.
    pub(crate) roles: [GrayImage; 2],
    /// The "?" an anonymous player's picture shows.
    pub(crate) anonymous: GrayImage,
    /// The digits 0 to 9.
    digits: Vec<GrayImage>,
    faces: Vec<Face>,
    /// The digits 0 to 9 of the time a row of the list tells, and its kanji (see `list`).
    pub(crate) ago_digits: Vec<GrayImage>,
    pub(crate) ago_kanji: Vec<GrayImage>,
    /// "対戦相手", the title of the dialog `opponent` reads.
    pub(crate) front: GrayImage,
    /// "編成", the title of the dialog `formation` reads.
    pub(crate) forming: GrayImage,
}

impl Default for Reader {
    fn default() -> Self {
        Self::new()
    }
}

impl Reader {
    pub fn new() -> Self {
        let reference = |bytes: &[u8]| {
            image::load_from_memory(bytes)
                .expect("make_refs writes the references")
                .to_luma8()
        };
        let digits = reference(include_bytes!("../data/digits.png"));
        // A strip of pictures side by side, each `wide`.
        let strip = |bytes: &[u8], wide: u32| -> Vec<GrayImage> {
            let strip = reference(bytes);
            (0..strip.width() / wide)
                .map(|at| imageops::crop_imm(&strip, at * wide, 0, wide, strip.height()).to_image())
                .collect()
        };
        Self {
            ago_digits: strip(
                include_bytes!("../data/ago-digits.png"),
                crate::list::DIGIT_SIZE.0,
            ),
            ago_kanji: strip(
                include_bytes!("../data/ago-kanji.png"),
                crate::list::KANJI_SIZE.0,
            ),
            title: reference(include_bytes!("../data/title.png")),
            roles: [
                reference(include_bytes!("../data/attack.png")),
                reference(include_bytes!("../data/defense.png")),
            ],
            anonymous: reference(include_bytes!("../data/anonymous.png")),
            front: reference(include_bytes!("../data/opponent.png")),
            forming: reference(include_bytes!("../data/formation.png")),
            digits: (0..10)
                .map(|digit| {
                    imageops::crop_imm(&digits, digit * DIGIT_SIZE.0, 0, DIGIT_SIZE.0, DIGIT_SIZE.1)
                        .to_image()
                })
                .collect(),
            faces: crate::students().iter().map(Face::of).collect(),
        }
    }

    /// Whether a frame shows a battle record, by its title: so a record is known as one
    /// while a part of it does not read.
    pub fn titled(&self, frame: &RgbImage) -> bool {
        title_shape(frame).is_some_and(|title| likeness(&title, &self.title) >= LIKE)
    }

    /// The battle a frame's record shows, or `None` for any other screen and for a record
    /// with a part that does not read.
    pub fn read(&self, frame: &RgbImage) -> Option<Battle> {
        if !self.titled(frame) {
            return None;
        }
        let (left, right) = (self.side(frame, false)?, self.side(frame, true)?);
        if left.attacks == right.attacks || left.won == right.won {
            return None;
        }
        let user_attacked = left.attacks;
        let (attacker, defender) = if left.attacks {
            (left, right)
        } else {
            (right, left)
        };
        Some(Battle {
            user_attacked,
            attacker_won: attacker.won,
            attacker: attacker.party,
            defender: defender.party,
        })
    }

    fn side(&self, frame: &RgbImage, right: bool) -> Option<Side> {
        let sheet = Sheet::of(frame)?;
        let role = role_shape(frame, right)?;
        let [attack, defense] = self.roles.each_ref().map(|known| likeness(&role, known));
        if attack.max(defense) < LIKE || attack.min(defense) >= LIKE {
            return None;
        }
        let mut team = [Fighter {
            student: 0,
            damage: 0,
        }; 6];
        for (slot, fighter) in team.iter_mut().enumerate() {
            let squad = if slot < 4 {
                Squad::Main
            } else {
                Squad::Support
            };
            *fighter = Fighter {
                student: self.student(&sheet, right, slot, squad)?,
                damage: self.damage(frame, right, slot)?,
            };
        }
        // An anonymous player's name says nothing of who it is: the picture tells them.
        let hidden = likeness(&icon_shape(frame, right)?, &self.anonymous);
        let plate = if hidden >= LIKE {
            None
        } else if hidden <= UNLIKE {
            Some(plate(&sheet.crop(NAME, shift(right))?, sheet.scale)?)
        } else {
            return None;
        };
        Some(Side {
            attacks: attack > defense,
            won: won(&sheet.crop(RESULT, shift(right))?)?,
            party: Party {
                plate,
                team: in_order(team),
            },
        })
    }

    /// The student of `squad` whose face a side's `slot` shows. The square is tried a
    /// little larger, smaller and off its place: an icon's face fills its square more or
    /// less (マリナ's fitted 86 wide where the others fitted 80 to 83).
    fn student(&self, sheet: &Sheet, right: bool, slot: usize, squad: Squad) -> Option<i64> {
        let middle = (FACE.0 + slot as f32 * FACE_STEP + shift(right), FACE.1);
        self.face(sheet, middle, FACE_SIDE, &(0..FACE_SIZE), squad)
    }

    /// The student of `squad` whose icon a screen draws about `middle` in a square of
    /// `side`, showing the `rows` of it (of `FACE_SIZE`).
    pub(crate) fn face(
        &self,
        sheet: &Sheet,
        middle: (f32, f32),
        side: f32,
        rows: &Range<u32>,
        squad: Squad,
    ) -> Option<i64> {
        let known: Vec<&Face> = self
            .faces
            .iter()
            .filter(|face| face.squad == squad)
            .collect();
        // How far each of the students' icons is from the square, at the best of the places.
        let distances = |places: &Places, known: &[&Face]| {
            let mut patches = Vec::new();
            for size in places.sizes {
                for down in places.off {
                    for across in places.off {
                        let half = side * size / 2.0;
                        let (x, y) = (middle.0 + across, middle.1 + down);
                        let square = sheet.crop((x - half, y - half, x + half, y + half), 0.0)?;
                        let patch =
                            imageops::resize(&square, FACE_SIZE, FACE_SIZE, FilterType::Triangle);
                        patches.push(patch.pixels().map(|pixel| pixel.0).collect::<Vec<_>>());
                    }
                }
            }
            let nearest = |face: &&Face| {
                patches
                    .iter()
                    .map(|patch| face.distance(patch, rows))
                    .fold(f32::MAX, f32::min)
            };
            Some(known.iter().map(nearest).collect::<Vec<f32>>())
        };
        let near_by = distances(&NEAR_BY, &known)?;
        if let Some(at) = clear(near_by.iter().copied(), FACE_NEAR) {
            return Some(known[at].student);
        }
        // An icon drawn off its place: the students it may be are looked for farther off.
        let may_be: Vec<&Face> = known
            .iter()
            .zip(&near_by)
            .filter(|(_, distance)| **distance <= FACE_FAR)
            .map(|(face, _)| *face)
            .collect();
        clear(distances(&FARTHER, &may_be)?.into_iter(), FACE_NEAR).map(|at| may_be[at].student)
    }

    /// The damage on the label over a side's `slot`.
    fn damage(&self, frame: &RgbImage, right: bool, slot: usize) -> Option<u32> {
        let cells = digit_cells(frame, right, slot)?;
        let mut damage = 0u32;
        for (at, cell) in cells.iter().enumerate() {
            let digit = clear(
                self.digits.iter().map(|known| distance(cell, known)),
                DIGIT_NEAR,
            )? as u32;
            // A number does not start with 0.
            if at == 0 && digit == 0 && cells.len() > 1 {
                return None;
            }
            damage = damage.checked_mul(10)?.checked_add(digit)?;
        }
        Some(damage)
    }
}

/// A reading is taken once it has stayed the same this long: a record's bars grow as it
/// opens, and a record read in between must not be kept.
pub const STILL: Duration = Duration::from_secs(1);

/// Tells a reading once it has stood still (see `STILL`): the same from frame to frame, as
/// `same` has it.
pub struct Steady<T> {
    seen: Option<(T, Instant)>,
    told: bool,
    same: fn(&T, &T) -> bool,
}

impl<T> Steady<T> {
    pub fn new(same: fn(&T, &T) -> bool) -> Self {
        Self {
            seen: None,
            told: false,
            same,
        }
    }

    /// Whether the reading now seen has stood still and was told.
    pub fn told(&self) -> bool {
        self.told
    }

    /// What a frame read `now` showed; the reading, once, when it has stood still.
    pub fn see(&mut self, reading: Option<T>, now: Instant) -> Option<&T> {
        let since = match (&self.seen, &reading) {
            (Some((seen, since)), Some(reading)) if (self.same)(seen, reading) => *since,
            _ => {
                self.told = false;
                now
            }
        };
        self.seen = reading.map(|reading| (reading, since));
        let (reading, since) = self.seen.as_ref()?;
        let still = !self.told && now.duration_since(*since) >= STILL;
        self.told |= still;
        still.then_some(reading)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use image::{GrayImage, Luma};

    use super::{Battle, Fighter, Party, Plate, STILL, Steady, in_order, measured, spot};

    /// A name of one stroke, `shade` bright, `down` from the top.
    fn name(shade: u8, down: u32) -> Option<Plate> {
        let mut drawn = GrayImage::new(40, 32);
        for x in 4..36 {
            for y in down..down + 3 {
                drawn.put_pixel(x, y, Luma([shade]));
            }
        }
        Some(Plate(drawn))
    }

    fn battle(attacker: Option<Plate>, damage: u32) -> Battle {
        let team =
            [10005, 10037, 16010, 10124, 20016, 20053].map(|student| Fighter { student, damage });
        Battle {
            attacker: Party {
                plate: attacker,
                team,
            },
            defender: Party { plate: None, team },
            attacker_won: true,
            user_attacked: true,
        }
    }

    #[test]
    fn a_point_of_a_frame_is_found_on_the_measured_screen() {
        // A Full HD frame draws the game 0.81 as large about its middle.
        let (size, scale) = ((1920, 1080), 1920.0 / 2370.0);
        let (x, y) = measured(size, scale, (960.0, 540.0));
        assert!(
            (x - 1185.0).abs() < 0.01 && (y - 666.5).abs() < 0.01,
            "the middle"
        );
        let (x, y) = measured(size, scale, spot(size, scale, (1750.0, 480.0)));
        assert!(
            (x - 1750.0).abs() < 0.01 && (y - 480.0).abs() < 0.01,
            "and back"
        );
    }

    #[test]
    fn a_name_drawn_a_shade_apart_is_the_same_reading() {
        let read = battle(name(200, 10), 7);
        // The winner's side wavers: the same strokes, a shade apart (a record with such a
        // name never stood still, and was not kept: 2026-11-17).
        assert!(read.alike(&battle(name(201, 10), 7)), "a shade apart");
        assert!(!read.alike(&battle(name(200, 20), 7)), "another name");
        assert!(!read.alike(&battle(None, 7)), "nobody's name");
        assert!(!read.alike(&battle(name(200, 10), 8)), "another damage");
        let mut lost = battle(name(200, 10), 7);
        lost.attacker_won = false;
        assert!(!read.alike(&lost), "another winner");
    }

    #[test]
    fn a_reading_stands_still_by_its_own_measure() {
        let start = Instant::now();
        // The same in the tens.
        let mut steady = Steady::new(|a: &i32, b: &i32| a / 10 == b / 10);
        assert!(!steady.told(), "nothing seen");
        steady.see(Some(70), start);
        assert_eq!(steady.see(Some(71), start + STILL / 2), None, "not yet");
        assert!(!steady.told(), "not yet told");
        assert_eq!(steady.see(Some(72), start + STILL), Some(&72), "still");
        assert!(steady.told(), "told");
        assert_eq!(steady.see(Some(73), start + STILL * 2), None, "once");
        assert!(steady.told(), "and told while it stays");
        assert_eq!(steady.see(Some(80), start + STILL * 3), None, "another");
        assert!(!steady.told(), "the new one is not told yet");
    }

    #[test]
    fn a_reading_is_told_once_when_it_has_stood_still() {
        let start = Instant::now();
        let mut steady = Steady::new(i32::eq);
        assert_eq!(steady.see(Some(7), start), None, "just seen");
        assert_eq!(
            steady.see(Some(7), start + STILL / 2),
            None,
            "not yet still"
        );
        assert_eq!(steady.see(Some(7), start + STILL), Some(&7), "still");
        assert_eq!(steady.see(Some(7), start + STILL * 2), None, "told already");
    }

    #[test]
    fn a_reading_that_changes_starts_over() {
        let start = Instant::now();
        let mut steady = Steady::new(i32::eq);
        steady.see(Some(7), start);
        // The bars still grow: another number.
        assert_eq!(steady.see(Some(8), start + STILL), None, "changed");
        assert_eq!(
            steady.see(Some(8), start + STILL * 2),
            Some(&8),
            "still since the change"
        );
        // The record closed and opened again.
        let later = start + Duration::from_secs(60);
        assert_eq!(steady.see(None, later), None, "nothing to read");
        assert_eq!(steady.see(Some(8), later + STILL), None, "seen anew");
        assert_eq!(
            steady.see(Some(8), later + STILL * 2),
            Some(&8),
            "told anew"
        );
    }

    #[test]
    fn the_specials_are_kept_in_one_order() {
        let fighter = |student| Fighter {
            student,
            damage: student as u32,
        };
        let seen = [10005, 10037, 10002, 10124, 20053, 20016].map(fighter);
        let kept = [10005, 10037, 10002, 10124, 20016, 20053].map(fighter);
        assert_eq!(
            in_order(seen),
            kept,
            "the specials by id, each with its damage"
        );
        assert_eq!(in_order(kept), kept, "the strikers stay as they stand");
    }
}
