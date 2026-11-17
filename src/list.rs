//! The list of battle records (戦闘履歴, the dialog that lists the battles and opens a
//! record): for each row in full sight, whether the user attacked and won, the opponent, and
//! how long ago the game says the battle was. Read as the battle record is: right, or not at
//! all. The parts lie where they were measured on a 2370×1333 window (2026-11-18); the list
//! scrolls by any number of pixels.

use image::{GrayImage, Rgb, RgbImage};

use crate::history::{
    Area, ICON_SIZE, INKED, LIKE, Plate, ROLE_SIZE, Reader, Sheet, TITLE_SIZE, UNLIKE, blue, clear,
    distance, ink, likeness, plate, shape, won,
};

/// The unit a row tells a battle's time in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Minute,
    Hour,
    Day,
}

impl Unit {
    fn seconds(self) -> i64 {
        match self {
            Self::Minute => 60,
            Self::Hour => 3600,
            Self::Day => 86400,
        }
    }
}

/// How long ago a battle was, as its row tells it: "5分前", "2時間前", "1日前".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ago {
    pub count: u32,
    pub unit: Unit,
}

impl Ago {
    /// When the battle was, in seconds since 1970: the earliest and the latest the words
    /// allow, shown at some time from `since` (when the list came up) to `now`. The game
    /// counts the whole units gone by, so "2時間前" is two hours ago and not yet three; a
    /// list left open may tell the time as it was when it came up.
    pub fn range(self, since: i64, now: i64) -> (i64, i64) {
        let (unit, count) = (self.unit.seconds(), i64::from(self.count));
        (since - (count + 1) * unit, now - count * unit)
    }
}

/// What a row tells of a battle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// Whether the user attacked (or else defended), and won.
    pub attacked: bool,
    pub won: bool,
    /// The opponent's name as the game draws it (as large as a record draws it, so the two
    /// screens' plates of one name compare); `None` for an anonymous opponent.
    pub opponent: Option<Plate>,
    pub ago: Ago,
}

/// A row in full sight: where its top lies on the measured screen, and what it tells when
/// every part of it reads.
#[derive(Debug, Clone, PartialEq)]
pub struct Shown {
    pub top: f32,
    pub row: Option<Row>,
}

/// "戦闘履歴", lower than the record's own title and its letters a little farther apart (they
/// start at 1077, 1134, 1187 and 1242 here, at 1080, 1136, 1187 and 1240 there): looked for
/// over these columns, it was 0.990 like the record's; over as many as the record's, 0.861.
const TITLE: Area = (1066.0, 288.0, 1303.0, 358.0);
/// The rows of the screen the list shows its rows in.
const SIGHT: (f32, f32) = (387.0, 1049.0);
/// A column right of every row's button: white across a row, grey between two rows.
const ROW_X: f32 = 1897.0;
/// A row's height, and how many of a frame's pixels a run of white may be off it (an edge
/// between white and grey falls on either side of a pixel): a row cut by the list's edge is
/// shorter.
pub const ROW_TALL: f32 = 165.0;
const ROW_SLACK: f32 = 2.5;
/// A row from its left edge to its right.
pub const ROW_ACROSS: (f32, f32) = (463.0, 1907.0);
/// The button "履歴" that opens a row's record, in the row.
pub const BUTTON: Area = (1625.0, 18.0, 1885.0, 146.0);

/// The parts of a row, down from its top. "Win" or "Lose":
const RESULT: Area = (495.0, 8.0, 700.0, 102.0);
/// the time, in the middle of this line;
const AGO: Area = (480.0, 106.0, 712.0, 152.0);
/// the sword of an attack or the shield of a defense;
const ROLE: Area = (749.5, 27.0, 849.5, 127.0);
/// the middle of the opponent's picture: of an anonymous one, the "?" in its circle;
const ICON: Area = (1044.0, 32.5, 1134.0, 122.5);
/// and the line "Lv.90 name", the name as far under the line's top as the record's.
const NAME: Area = (1184.0, 9.0, 1612.0, 61.0);

/// The white of a row at `ROW_X`.
fn white(pixel: &Rgb<u8>) -> bool {
    pixel.0.into_iter().min().unwrap_or(0) >= 245
}

/// A part of the row whose top is at `top`.
fn part((left, up, right, down): Area, top: f32) -> Area {
    (left, top + up, right, top + down)
}

/// The tops of the rows in full sight, in a column of the list's pixels, `white` or not:
/// the runs of white as tall as a row (`tall` pixels, give or take `slack`). A row cut a
/// little at the list's first pixel is measured from its end.
fn tops(white: &[bool], tall: f32, slack: f32) -> Vec<f32> {
    let mut found = Vec::new();
    let mut start = None;
    for at in 0..=white.len() {
        match (start, white.get(at).copied().unwrap_or(false)) {
            (None, true) => start = Some(at),
            (Some(from), false) => {
                if ((at - from) as f32 - tall).abs() <= slack {
                    found.push(if from == 0 {
                        at as f32 - tall
                    } else {
                        from as f32
                    });
                }
                start = None;
            }
            _ => {}
        }
    }
    found
}

/// The time's letters stand side by side, the last at the line's right end: a kanji takes
/// `EM` of the line and a digit `DIGIT` (in "20時間前" the letters began at 526, 546, 568,
/// 601 and 634 and the last ended at 665). They are `AGO_TALL` high.
const EM: f32 = 33.4;
const DIGIT: f32 = 20.0;
const AGO_TALL: (f32, f32) = (27.0, 37.0);
/// The kanji of a time, in the order of their references (`data/ago-kanji.png`). No screen
/// at hand shows "日前" whole yet (2026-11-18): until its reference is cut, a row a day old
/// or older is not read.
pub const KANJI: [char; 5] = ['分', '時', '間', '前', '日'];
/// A letter is its reference from this near (the references' own letters lay 14.3 from
/// each other at most, another kanji 66.3 at least; a digit drawn from the game's bolder
/// digits lay 15.9 from the row's own, another digit 33.1 at least).
const LETTER_NEAR: f32 = 20.0;
/// How far off its place two letters may part.
const SNAP: f32 = 3.0;
/// The place a letter is cut from, on the measured screen, and the size it is compared at.
pub const LETTER_TALL: f32 = 36.0;
pub const DIGIT_WIDE: f32 = 22.0;
pub const KANJI_WIDE: f32 = 36.0;
pub const DIGIT_SIZE: (u32, u32) = (11, 18);
pub const KANJI_SIZE: (u32, u32) = (18, 18);
/// The points a pixel of a letter is the mean of, each way.
const FINE: u32 = 3;

/// The line of a row's time: how inked each pixel is, and the columns and rows the ink
/// lies in.
struct Line {
    ink: GrayImage,
    across: (u32, u32),
    down: (u32, u32),
    scale: f32,
}

impl Line {
    /// The line of the row whose top is at `top`; `None` where no ink as tall as the
    /// letters lies.
    fn of(sheet: &Sheet, top: f32) -> Option<Self> {
        let area = sheet.crop(part(AGO, top), 0.0)?;
        let ink = GrayImage::from_fn(area.width(), area.height(), |x, y| {
            image::Luma([ink(area.get_pixel(x, y))])
        });
        let inked = |x: u32, y: u32| ink.get_pixel(x, y).0[0] >= INKED;
        let column = |x: &u32| (0..ink.height()).any(|y| inked(*x, y));
        let row = |y: &u32| (0..ink.width()).any(|x| inked(x, *y));
        let across = (
            (0..ink.width()).find(column)?,
            (0..ink.width()).rfind(column)? + 1,
        );
        let down = (
            (0..ink.height()).find(row)?,
            (0..ink.height()).rfind(row)? + 1,
        );
        let tall = (down.1 - down.0) as f32 / sheet.scale;
        (AGO_TALL.0..=AGO_TALL.1).contains(&tall).then_some(Self {
            ink,
            across,
            down,
            scale: sheet.scale,
        })
    }

    /// Where two letters part near the column `at`: the column least inked within `SNAP`
    /// of it, and of several the nearest.
    fn parting(&self, at: f32) -> u32 {
        let (at, reach) = (at.round() as i64, (SNAP * self.scale).ceil() as i64);
        let inked = |x: &i64| -> u32 {
            (self.down.0..self.down.1)
                .map(|y| u32::from(self.ink.get_pixel(*x as u32, y).0[0]))
                .sum()
        };
        (at - reach..=at + reach)
            .filter(|x| (i64::from(self.across.0)..i64::from(self.across.1)).contains(x))
            .min_by_key(|x| (inked(x), (x - at).abs()))
            .map_or(self.across.0, |x| x as u32)
    }

    /// The columns the last `kanji` letters part at, from the left: where the first of
    /// them begins, to the ink's right end.
    fn parts(&self, kanji: usize) -> Vec<u32> {
        let end = self.across.1 as f32;
        let mut parts: Vec<u32> = (1..=kanji)
            .rev()
            .map(|back| self.parting(end - back as f32 * EM * self.scale))
            .collect();
        parts.push(self.across.1);
        parts
    }

    /// The letters between the columns of `parts`, each `wide` on the measured screen and
    /// brought to `size`.
    fn cut(&self, parts: &[u32], wide: f32, size: (u32, u32)) -> Option<Vec<GrayImage>> {
        parts
            .windows(2)
            .map(|pair| letter(&self.ink, (pair[0], pair[1]), wide, self.scale, size))
            .collect()
    }

    /// The last `kanji` letters of the line.
    fn words(&self, kanji: usize) -> Option<Vec<GrayImage>> {
        self.cut(&self.parts(kanji), KANJI_WIDE, KANJI_SIZE)
    }

    /// The digits before the last `kanji` letters: one or two.
    fn digits(&self, kanji: usize) -> Option<Vec<GrayImage>> {
        let words = self.parts(kanji)[0];
        let room = words.checked_sub(self.across.0)? as f32 / self.scale;
        let parts = if (DIGIT * 0.2..=DIGIT * 1.35).contains(&room) {
            vec![self.across.0, words]
        } else if room <= DIGIT * 2.35 {
            let middle = self.parting(words as f32 - DIGIT * self.scale);
            vec![self.across.0, middle, words]
        } else {
            return None;
        };
        self.cut(&parts, DIGIT_WIDE, DIGIT_SIZE)
    }
}

/// A letter as it is compared: `wide` by `LETTER_TALL` of a screen drawn at `scale`, about
/// the middle of the ink in the columns `from` to `to` of `ink` (how inked each pixel is),
/// brought to `size`. The ink's middle lies at any fraction of a pixel, and so does the
/// letter cut: cut by the ink's edges, one letter lay 31 from itself in another place.
pub fn letter(
    ink: &GrayImage,
    (from, to): (u32, u32),
    wide: f32,
    scale: f32,
    size: (u32, u32),
) -> Option<GrayImage> {
    let (from, to) = (from.min(ink.width()), to.min(ink.width()));
    let (mut weight, mut across, mut down) = (0.0f32, 0.0f32, 0.0f32);
    for y in 0..ink.height() {
        for x in from..to {
            let inked = f32::from(ink.get_pixel(x, y).0[0]);
            weight += inked;
            across += inked * (x as f32 + 0.5);
            down += inked * (y as f32 + 0.5);
        }
    }
    if weight <= 0.0 {
        return None;
    }
    let (wide, tall) = (wide * scale, LETTER_TALL * scale);
    let (left, top) = (across / weight - wide / 2.0, down / weight - tall / 2.0);
    // The ink at a point of the picture, between its four pixels; none outside the letter's
    // own columns.
    let at = |x: f32, y: f32| -> f32 {
        let (x, y) = (x - 0.5, y - 0.5);
        let (x0, y0) = (x.floor(), y.floor());
        let pixel = |x: f32, y: f32| {
            let inside = x >= from as f32 && x < to as f32 && y >= 0.0 && y < ink.height() as f32;
            if inside {
                f32::from(ink.get_pixel(x as u32, y as u32).0[0])
            } else {
                0.0
            }
        };
        let (fx, fy) = (x - x0, y - y0);
        pixel(x0, y0) * (1.0 - fx) * (1.0 - fy)
            + pixel(x0 + 1.0, y0) * fx * (1.0 - fy)
            + pixel(x0, y0 + 1.0) * (1.0 - fx) * fy
            + pixel(x0 + 1.0, y0 + 1.0) * fx * fy
    };
    // Each pixel of the letter is the mean of `FINE` by `FINE` points of its square.
    Some(GrayImage::from_fn(size.0, size.1, |x, y| {
        let mut sum = 0.0;
        for fine_y in 0..FINE {
            for fine_x in 0..FINE {
                let part = |pixel: u32, fine: u32, all: u32, length: f32| {
                    (pixel as f32 + (fine as f32 + 0.5) / FINE as f32) / all as f32 * length
                };
                sum += at(
                    left + part(x, fine_x, size.0, wide),
                    top + part(y, fine_y, size.1, tall),
                );
            }
        }
        image::Luma([(sum / (FINE * FINE) as f32).round().clamp(0.0, 255.0) as u8])
    }))
}

/// The letters of the time told by the row whose top is at `top` on a frame: its digits,
/// then its last `kanji` letters. For `make_refs`.
pub fn ago_letters(frame: &RgbImage, top: f32, kanji: usize) -> Option<Vec<GrayImage>> {
    let line = Line::of(&Sheet::of(frame)?, top)?;
    let mut letters = line.digits(kanji)?;
    letters.extend(line.words(kanji)?);
    Some(letters)
}

/// The shape of the title's place, for the reader.
fn title_shape(sheet: &Sheet) -> Option<GrayImage> {
    Some(shape(&sheet.crop(TITLE, 0.0)?, ink, TITLE_SIZE))
}

impl Reader {
    /// The rows in full sight of a frame's list of battle records, from the top, or `None`
    /// for any other screen.
    pub fn list(&self, frame: &RgbImage) -> Option<Vec<Shown>> {
        let sheet = Sheet::of(frame)?;
        if likeness(&title_shape(&sheet)?, &self.title) < LIKE {
            return None;
        }
        let ((x, top), (_, bottom)) = (sheet.spot((ROW_X, SIGHT.0)), sheet.spot((ROW_X, SIGHT.1)));
        let (x, top, bottom) = (x.round() as u32, top.round() as u32, bottom.round() as u32);
        if x >= frame.width() || bottom > frame.height() {
            return None;
        }
        let column: Vec<bool> = (top..bottom)
            .map(|y| white(frame.get_pixel(x, y)))
            .collect();
        let rows = tops(&column, ROW_TALL * sheet.scale, ROW_SLACK);
        Some(
            rows.into_iter()
                .map(|row| {
                    let top = SIGHT.0 + row / sheet.scale;
                    Shown {
                        top,
                        row: self.row(&sheet, top),
                    }
                })
                .collect(),
        )
    }

    /// What the row whose top is at `top` tells, or `None` when a part of it does not read.
    fn row(&self, sheet: &Sheet, top: f32) -> Option<Row> {
        let role = shape(&sheet.crop(part(ROLE, top), 0.0)?, blue, ROLE_SIZE);
        let [attack, defense] = self.roles.each_ref().map(|known| likeness(&role, known));
        if attack.max(defense) < LIKE || attack.min(defense) >= LIKE {
            return None;
        }
        let icon = shape(&sheet.crop(part(ICON, top), 0.0)?, blue, ICON_SIZE);
        let hidden = likeness(&icon, &self.anonymous);
        let opponent = if hidden >= LIKE {
            None
        } else if hidden <= UNLIKE {
            Some(plate(&sheet.crop(part(NAME, top), 0.0)?, sheet.scale)?)
        } else {
            return None;
        };
        Some(Row {
            attacked: attack > defense,
            won: won(&sheet.crop(part(RESULT, top), 0.0)?)?,
            opponent,
            ago: self.ago(sheet, top)?,
        })
    }

    /// The time the row whose top is at `top` tells: one or two digits, then "分前", "時間前"
    /// or "日前". A count the unit never shows is not read.
    fn ago(&self, sheet: &Sheet, top: f32) -> Option<Ago> {
        let line = Line::of(sheet, top)?;
        let kanji = |letter: &GrayImage| {
            let known = self.ago_kanji.iter().map(|known| distance(letter, known));
            clear(known, LETTER_NEAR).map(|at| KANJI[at])
        };
        let last = line.words(2)?;
        if kanji(&last[1])? != '前' {
            return None;
        }
        let (unit, letters) = match kanji(&last[0])? {
            '分' => (Unit::Minute, 2),
            '日' => (Unit::Day, 2),
            '間' if kanji(&line.words(3)?[0])? == '時' => (Unit::Hour, 3),
            _ => return None,
        };
        let mut count = 0;
        for (at, digit) in line.digits(letters)?.iter().enumerate() {
            let known = self.ago_digits.iter().map(|known| distance(digit, known));
            let digit = clear(known, LETTER_NEAR)? as u32;
            // A number does not start with 0.
            if at == 0 && digit == 0 {
                return None;
            }
            count = count * 10 + digit;
        }
        let most = match unit {
            Unit::Minute => 59,
            Unit::Hour => 23,
            Unit::Day => 99,
        };
        (count <= most).then_some(Ago { count, unit })
    }
}

#[cfg(test)]
mod tests {
    use super::{Ago, Unit, tops};
    use crate::history::{Reader, Sheet};

    /// A fixture: a screen kept out of the repository (see `tests/list.rs`).
    fn screen(name: &str) -> image::RgbImage {
        let path = format!("tests/fixtures/{name}.png");
        image::open(&path)
            .unwrap_or_else(|error| panic!("{path} (kept out of the repository): {error}"))
            .to_rgb8()
    }

    #[test]
    fn a_rows_name_is_the_name_a_record_draws() {
        // The first row of the scrolled list is cut by the list's top (so `Reader::list`
        // leaves it out), its parts all in sight: an attack won 20 hours ago against the
        // player the record `attacked` shows as the defender of another battle.
        let reader = Reader::new();
        let list = screen("list-scrolled");
        let sheet = Sheet::of(&list).expect("a screen that reads");
        let row = reader.row(&sheet, 376.0).expect("the row's parts read");
        assert_eq!(
            (row.attacked, row.won, row.ago.count, row.ago.unit),
            (true, true, 20, Unit::Hour)
        );
        let record = reader.read(&screen("attacked")).expect("the record");
        let (listed, recorded) = (row.opponent, record.defender.plate);
        assert!(
            listed.expect("a name").same(&recorded.expect("a name")),
            "one player on both screens"
        );
    }

    fn column(runs: &[(usize, usize)], tall: usize) -> Vec<bool> {
        let mut column = vec![false; tall];
        for &(from, to) in runs {
            column[from..to].fill(true);
        }
        column
    }

    #[test]
    fn the_rows_in_full_sight_are_found_by_their_height() {
        // The list at its top: three rows whole, the fourth cut by the list's end.
        let top = column(&[(11, 176), (186, 351), (360, 526), (535, 662)], 662);
        assert_eq!(tops(&top, 165.0, 2.0), [11.0, 186.0, 360.0]);
        // Scrolled: the first row cut by the list's top.
        let scrolled = column(&[(0, 154), (164, 329), (338, 504), (513, 662)], 662);
        assert_eq!(tops(&scrolled, 165.0, 2.0), [164.0, 338.0]);
        // A row with its last pixel at the list's end is whole.
        let end = column(&[(497, 662)], 662);
        assert_eq!(tops(&end, 165.0, 2.0), [497.0]);
        // A row cut by a pixel at the list's top lies where its end tells.
        let cut = column(&[(0, 164), (174, 339)], 662);
        assert_eq!(tops(&cut, 165.0, 2.0), [-1.0, 174.0]);
        // A smaller window draws a smaller row.
        let small = column(&[(7, 111), (117, 222)], 419);
        assert_eq!(tops(&small, 104.4, 1.5), [7.0, 117.0]);
        assert!(tops(&column(&[], 662), 165.0, 2.0).is_empty(), "no row");
    }

    #[test]
    fn a_time_told_allows_a_range() {
        let ago = |count, unit| Ago { count, unit };
        // Two hours ago and not yet three, read at one moment.
        assert_eq!(
            ago(2, Unit::Hour).range(100_000, 100_000),
            (100_000 - 3 * 3600, 100_000 - 2 * 3600)
        );
        assert_eq!(
            ago(5, Unit::Minute).range(100_000, 100_000),
            (100_000 - 360, 100_000 - 300)
        );
        assert_eq!(
            ago(1, Unit::Day).range(200_000, 200_000),
            (200_000 - 2 * 86400, 200_000 - 86400)
        );
        // A list left open for ten minutes: the words may be as old as the list.
        assert_eq!(
            ago(5, Unit::Minute).range(100_000, 100_600),
            (100_000 - 360, 100_600 - 300)
        );
    }
}
