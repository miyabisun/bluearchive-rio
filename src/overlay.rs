//! The marks of `guide` drawn over the game: a picture as large as the game's screen
//! (`draw`), shown in a window that lies over the game's and lets every click through to
//! it (`Overlay`, on Windows). The game is only covered: nothing is sent to it.

use image::{
    GrayImage, Rgba, RgbaImage,
    imageops::{self, FilterType},
};

use crate::{
    guide::{Mark, SEEK_SIDE, Way, Words},
    history::{Area, scale, spot},
};

/// A card to pick, a card to take out and a student expected: the game marks its own
/// choice in yellow green, and none of these is near it.
const PICK: Rgba<u8> = Rgba([255, 0, 160, 255]);
const DROP: Rgba<u8> = Rgba([235, 30, 45, 255]);
const GUESS: Rgba<u8> = Rgba([255, 205, 0, 255]);
/// Over a card to take out, and behind an icon.
const VEIL: Rgba<u8> = Rgba([0, 0, 0, 140]);
const GROUND: Rgba<u8> = Rgba([24, 28, 44, 235]);
const ARROW: Rgba<u8> = Rgba([255, 255, 255, 255]);
/// The grounds of the words for a record kept (or a row's record read), and for one kept
/// before: the words tell them apart too.
const DONE: Rgba<u8> = Rgba([18, 132, 72, 245]);
const BEFORE: Rgba<u8> = Rgba([36, 92, 168, 245]);
/// The room left and right of words on their ground, for a ground one high.
const SAY_ROOM: f32 = 0.3;
/// How thick a frame is, and how an arrow is drawn beside a sought student's icon: its
/// gap to the icon, its height and its width. On the measured screen.
const FRAME: f32 = 8.0;
const ARROW_GAP: f32 = 6.0;
const ARROW_TALL: f32 = 30.0;
const ARROW_WIDE: f32 = 56.0;

/// The words' pictures (`data/words`, drawn by `make_refs`), in the order of `Words::ALL`.
const WORDS: [&[u8]; 6] = [
    include_bytes!("../data/words/reading.png"),
    include_bytes!("../data/words/kept.png"),
    include_bytes!("../data/words/known.png"),
    include_bytes!("../data/words/read.png"),
    include_bytes!("../data/words/unread.png"),
    include_bytes!("../data/words/reopen.png"),
];

/// The digits and the unit the rows to a student sought are written with (`data/words`,
/// drawn by `make_refs`), and how tall they stand at the foot of the student's icon.
const DIGITS: [&[u8]; 10] = [
    include_bytes!("../data/words/digit-0.png"),
    include_bytes!("../data/words/digit-1.png"),
    include_bytes!("../data/words/digit-2.png"),
    include_bytes!("../data/words/digit-3.png"),
    include_bytes!("../data/words/digit-4.png"),
    include_bytes!("../data/words/digit-5.png"),
    include_bytes!("../data/words/digit-6.png"),
    include_bytes!("../data/words/digit-7.png"),
    include_bytes!("../data/words/digit-8.png"),
    include_bytes!("../data/words/digit-9.png"),
];
const ROWS_UNIT: &[u8] = include_bytes!("../data/words/rows.png");
const ROWS_TALL: f32 = 38.0;

struct Canvas {
    picture: RgbaImage,
    scale: f32,
}

impl Canvas {
    /// The picture's pixels of an area of the measured screen: left, top, right, bottom.
    fn pixels(&self, (left, top, right, bottom): Area) -> (u32, u32, u32, u32) {
        let size = self.picture.dimensions();
        let ((x0, y0), (x1, y1)) = (
            spot(size, self.scale, (left, top)),
            spot(size, self.scale, (right, bottom)),
        );
        let within = |at: f32, most: u32| (at.round().max(0.0) as u32).min(most);
        (
            within(x0, size.0),
            within(y0, size.1),
            within(x1, size.0),
            within(y1, size.1),
        )
    }

    fn fill(&mut self, area: Area, colour: Rgba<u8>) {
        let (x0, y0, x1, y1) = self.pixels(area);
        for y in y0..y1 {
            for x in x0..x1 {
                self.picture.put_pixel(x, y, colour);
            }
        }
    }

    /// The edge of an area, `FRAME` thick inside it.
    fn frame(&mut self, (left, top, right, bottom): Area, colour: Rgba<u8>) {
        self.fill((left, top, right, top + FRAME), colour);
        self.fill((left, bottom - FRAME, right, bottom), colour);
        self.fill((left, top, left + FRAME, bottom), colour);
        self.fill((right - FRAME, top, right, bottom), colour);
    }

    /// A student's icon in the square of `side` about `middle`, on a dark ground.
    fn icon(&mut self, student: i64, middle: (f32, f32), side: f32) -> Area {
        let half = side / 2.0;
        let square = (
            middle.0 - half,
            middle.1 - half,
            middle.0 + half,
            middle.1 + half,
        );
        self.fill(square, GROUND);
        let (x0, y0, x1, y1) = self.pixels(square);
        let icon = crate::icon(student).and_then(|icon| image::load_from_memory(icon).ok());
        if let (Some(icon), true) = (icon, x1 > x0 && y1 > y0) {
            let icon = imageops::resize(&icon.to_rgba8(), x1 - x0, y1 - y0, FilterType::Triangle);
            imageops::overlay(&mut self.picture, &icon, i64::from(x0), i64::from(y0));
        }
        square
    }

    /// Words on their ground, `tall` high: the ground's left end and middle at `at`.
    fn say(&mut self, words: Words, at: (f32, f32), tall: f32) {
        let (letters, ground) = match words {
            Words::Reading => (WORDS[0], GROUND),
            Words::Kept => (WORDS[1], DONE),
            Words::Known => (WORDS[2], BEFORE),
            Words::Read => (WORDS[3], DONE),
            Words::Unread => (WORDS[4], PICK),
            Words::Reopen => (WORDS[5], GROUND),
        };
        let Ok(letters) = image::load_from_memory(letters) else {
            return;
        };
        let letters = letters.to_luma8();
        // The picture's height is the ground's.
        let drawn = tall * self.scale / letters.height() as f32;
        let (wide, high) = (
            (letters.width() as f32 * drawn).round().max(1.0) as u32,
            (letters.height() as f32 * drawn).round().max(1.0) as u32,
        );
        let letters = imageops::resize(&letters, wide, high, FilterType::Triangle);
        let room = tall * SAY_ROOM;
        let top = at.1 - tall / 2.0;
        let right = at.0 + room * 2.0 + wide as f32 / self.scale;
        self.fill((at.0, top, right, top + tall), ground);
        let (left, top, _, _) = self.pixels((at.0 + room, top, right, top + tall));
        self.ink(&letters, left, top);
    }

    /// Letters in white over what the picture holds, as much as each pixel of `letters`
    /// covers, their top left corner at the picture's pixel (`left`, `top`).
    fn ink(&mut self, letters: &GrayImage, left: u32, top: u32) {
        for (x, y, cover) in letters.enumerate_pixels() {
            let (x, y) = (left + x, top + y);
            if x < self.picture.width() && y < self.picture.height() {
                let cover = u32::from(cover.0[0]);
                let pixel = self.picture.get_pixel_mut(x, y);
                pixel.0 = pixel
                    .0
                    .map(|colour| ((u32::from(colour) * (255 - cover) + 255 * cover) / 255) as u8);
            }
        }
    }

    /// How many rows away a sought student is: the number and its unit on the dark ground,
    /// over the foot of the student's icon about `middle`, as wide as the icon.
    fn rows(&mut self, rows: u8, middle: (f32, f32)) {
        let half = SEEK_SIDE / 2.0;
        let foot = middle.1 + half;
        let strip = (middle.0 - half, foot - ROWS_TALL, middle.0 + half, foot);
        let digits = rows.to_string();
        let pictures = digits
            .bytes()
            .map(|digit| DIGITS[usize::from(digit - b'0')])
            .chain([ROWS_UNIT]);
        let mut letters = Vec::new();
        for picture in pictures {
            let Ok(picture) = image::load_from_memory(picture) else {
                return;
            };
            let picture = picture.to_luma8();
            // Each picture's height is the strip's.
            let drawn = ROWS_TALL * self.scale / picture.height() as f32;
            let wide = (picture.width() as f32 * drawn).round().max(1.0) as u32;
            let high = (picture.height() as f32 * drawn).round().max(1.0) as u32;
            letters.push(imageops::resize(&picture, wide, high, FilterType::Triangle));
        }
        self.fill(strip, GROUND);
        let (left, top, right, _) = self.pixels(strip);
        let wide: u32 = letters.iter().map(GrayImage::width).sum();
        let mut left = left + (right - left).saturating_sub(wide) / 2;
        for letters in &letters {
            self.ink(letters, left, top);
            left += letters.width();
        }
    }

    /// An arrow's head with its tip at `tip`, pointing up or down.
    fn arrow(&mut self, tip: (f32, f32), up: bool) {
        let size = self.picture.dimensions();
        let (x, y) = spot(size, self.scale, tip);
        let (tall, wide) = (ARROW_TALL * self.scale, ARROW_WIDE * self.scale);
        for row in 0..tall.round() as i64 {
            let half = (row as f32 + 0.5) / tall * wide / 2.0;
            let line = y.round() as i64 + if up { row } else { -1 - row };
            for across in (x - half).round() as i64..(x + half).round() as i64 {
                if (0..i64::from(size.0)).contains(&across)
                    && (0..i64::from(size.1)).contains(&line)
                {
                    self.picture.put_pixel(across as u32, line as u32, ARROW);
                }
            }
        }
    }
}

/// The marks as a picture of a screen `width` by `height`: clear wherever nothing is
/// marked, and all clear for a screen too small to read.
pub fn draw(marks: &[Mark], width: u32, height: u32) -> RgbaImage {
    let mut canvas = Canvas {
        picture: RgbaImage::new(width, height),
        scale: 0.0,
    };
    let Some(scale) = scale(width, height) else {
        return canvas.picture;
    };
    canvas.scale = scale;
    for mark in marks {
        match *mark {
            Mark::Guess {
                student,
                middle,
                side,
            } => {
                let square = canvas.icon(student, middle, side);
                canvas.frame(square, GUESS);
            }
            Mark::Pick(area) => canvas.frame(area, PICK),
            Mark::Drop(area) => {
                canvas.fill(area, VEIL);
                canvas.frame(area, DROP);
            }
            Mark::Seek {
                student,
                way,
                middle,
            } => {
                let (half, arrow) = (SEEK_SIDE / 2.0 + FRAME, ARROW_GAP + ARROW_TALL + FRAME);
                let (up, down) = (way != Way::Down, way != Way::Up);
                let ground = (
                    middle.0 - half,
                    middle.1 - half - if up { arrow } else { 0.0 },
                    middle.0 + half,
                    middle.1 + half + if down { arrow } else { 0.0 },
                );
                canvas.fill(ground, GROUND);
                canvas.frame(ground, PICK);
                canvas.icon(student, middle, SEEK_SIDE);
                let reach = SEEK_SIDE / 2.0 + ARROW_GAP + ARROW_TALL;
                if up {
                    canvas.arrow((middle.0, middle.1 - reach), true);
                }
                if down {
                    canvas.arrow((middle.0, middle.1 + reach), false);
                }
            }
            Mark::Rows { rows, middle } => canvas.rows(rows, middle),
            Mark::Say { words, at, tall } => canvas.say(words, at, tall),
        }
    }
    canvas.picture
}

#[cfg(windows)]
pub use window::Overlay;

#[cfg(windows)]
mod window {
    use std::ptr::{null, null_mut};

    use image::RgbaImage;
    use windows_sys::{
        Win32::{
            Foundation::{HWND, POINT, SIZE},
            Graphics::Gdi::{
                AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
                CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject,
                GetDC, RGBQUAD, ReleaseDC, SelectObject,
            },
            UI::{
                HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext},
                WindowsAndMessaging::{
                    CreateWindowExW, DestroyWindow, DispatchMessageW, MSG, PM_REMOVE, PeekMessageW,
                    SW_HIDE, SW_SHOWNOACTIVATE, ShowWindow, ULW_ALPHA, UpdateLayeredWindow,
                    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
                    WS_EX_TRANSPARENT, WS_POPUP,
                },
            },
        },
        w,
    };

    use super::draw;
    use crate::guide::Mark;

    /// A window over the game's screen that shows the marks and takes no click: every click
    /// goes to the window under it. It belongs to the thread that made it, which must call
    /// `pump` now and then.
    pub struct Overlay {
        window: HWND,
        shown: Option<Shown>,
    }

    /// What is shown: the marks, and where and how large the game's screen is.
    #[derive(PartialEq)]
    struct Shown {
        marks: Vec<Mark>,
        origin: (i32, i32),
        size: (u32, u32),
    }

    impl Overlay {
        /// `None` when Windows makes no window.
        pub fn new() -> Option<Self> {
            // SAFETY: the class is Windows' own, the title and the parents are null; the
            // thread measures in the screen's pixels from here on, as the capture does.
            let window = unsafe {
                SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
                CreateWindowExW(
                    WS_EX_LAYERED
                        | WS_EX_TRANSPARENT
                        | WS_EX_TOPMOST
                        | WS_EX_NOACTIVATE
                        | WS_EX_TOOLWINDOW,
                    w!("STATIC"),
                    null(),
                    WS_POPUP,
                    0,
                    0,
                    0,
                    0,
                    null_mut(),
                    null_mut(),
                    null_mut(),
                    null(),
                )
            };
            (!window.is_null()).then_some(Self {
                window,
                shown: None,
            })
        }

        /// Shows the marks over the game's screen, whose top left corner is at `origin` on
        /// the desktop and which is `size` large; hides the window for no marks.
        pub fn show(&mut self, marks: &[Mark], origin: (i32, i32), size: (u32, u32)) {
            if marks.is_empty() {
                return self.hide();
            }
            let next = Shown {
                marks: marks.to_vec(),
                origin,
                size,
            };
            if self.shown.as_ref() == Some(&next) {
                return;
            }
            if self.put(&draw(marks, size.0, size.1), origin) {
                // SAFETY: the window is this overlay's.
                unsafe { ShowWindow(self.window, SW_SHOWNOACTIVATE) };
                self.shown = Some(next);
            }
        }

        pub fn hide(&mut self) {
            if self.shown.take().is_some() {
                // SAFETY: the window is this overlay's.
                unsafe { ShowWindow(self.window, SW_HIDE) };
            }
        }

        /// Answers what Windows asks of the window.
        pub fn pump(&self) {
            // SAFETY: the message outlives the calls.
            unsafe {
                let mut message: MSG = std::mem::zeroed();
                while PeekMessageW(&raw mut message, null_mut(), 0, 0, PM_REMOVE) != 0 {
                    DispatchMessageW(&raw const message);
                }
            }
        }

        /// Makes the window the picture, at `origin`.
        fn put(&self, picture: &RgbaImage, origin: (i32, i32)) -> bool {
            let (width, height) = picture.dimensions();
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width as i32,
                    // Rows from the top.
                    biHeight: -(height as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD {
                    rgbBlue: 0,
                    rgbGreen: 0,
                    rgbRed: 0,
                    rgbReserved: 0,
                }],
            };
            let (position, size, source) = (
                POINT {
                    x: origin.0,
                    y: origin.1,
                },
                SIZE {
                    cx: width as i32,
                    cy: height as i32,
                },
                POINT { x: 0, y: 0 },
            );
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            // SAFETY: every handle is checked or released here; `bits` is the bitmap's
            // memory, `width * height` pixels of four bytes, while the bitmap lives.
            unsafe {
                let screen = GetDC(null_mut());
                let memory = CreateCompatibleDC(screen);
                let mut bits = null_mut();
                let bitmap = CreateDIBSection(
                    memory,
                    &raw const info,
                    DIB_RGB_COLORS,
                    &raw mut bits,
                    null_mut(),
                    0,
                );
                let mut put = false;
                if !bitmap.is_null() && !bits.is_null() {
                    let pixels = std::slice::from_raw_parts_mut(
                        bits.cast::<u8>(),
                        width as usize * height as usize * 4,
                    );
                    // Windows takes blue first, and each colour times its alpha.
                    let quads = pixels.as_chunks_mut::<4>().0.iter_mut();
                    for (to, from) in quads.zip(picture.pixels()) {
                        let [r, g, b, alpha] = from.0.map(u32::from);
                        let lit = |colour: u32| (colour * alpha / 255) as u8;
                        *to = [lit(b), lit(g), lit(r), alpha as u8];
                    }
                    let before = SelectObject(memory, bitmap);
                    put = UpdateLayeredWindow(
                        self.window,
                        screen,
                        &raw const position,
                        &raw const size,
                        memory,
                        &raw const source,
                        0,
                        &raw const blend,
                        ULW_ALPHA,
                    ) != 0;
                    SelectObject(memory, before);
                    DeleteObject(bitmap);
                }
                DeleteDC(memory);
                ReleaseDC(null_mut(), screen);
                put
            }
        }
    }

    impl Drop for Overlay {
        fn drop(&mut self) {
            // SAFETY: the window is this overlay's, and is not used again.
            unsafe { DestroyWindow(self.window) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GROUND, GUESS, PICK, draw};
    use crate::guide::{Mark, Way, Words};

    const CARD: Mark = Mark::Pick((1000.0, 400.0, 1200.0, 600.0));

    #[test]
    fn a_card_to_pick_is_framed_and_left_clear() {
        let picture = draw(&[CARD], 2370, 1333);
        assert_eq!(picture.get_pixel(1002, 500), &PICK, "the frame's left side");
        assert_eq!(
            picture.get_pixel(1100, 500).0[3],
            0,
            "the card shows through"
        );
        assert_eq!(picture.get_pixel(990, 500).0[3], 0, "nothing beside it");
    }

    #[test]
    fn the_marks_follow_the_windows_size() {
        // On a Full HD screen the measured (1002, 500) lies at (811.8, 405.1).
        let picture = draw(&[CARD], 1920, 1080);
        assert_eq!(picture.get_pixel(812, 405), &PICK, "the frame's left side");
        assert_eq!(
            picture.get_pixel(1002, 500).0[3],
            0,
            "not where 2370 wide draws it"
        );
        let small = draw(&[CARD], 800, 450);
        assert!(
            small.pixels().all(|pixel| pixel.0[3] == 0),
            "a screen too small to read is not marked"
        );
    }

    #[test]
    fn a_student_expected_is_drawn_as_the_icon_in_a_frame() {
        let guess = Mark::Guess {
            student: 10005,
            middle: (365.0, 875.0),
            side: 116.0,
        };
        let picture = draw(&[guess], 2370, 1333);
        assert_eq!(picture.get_pixel(309, 875), &GUESS, "the frame");
        assert_eq!(
            picture.get_pixel(365, 875).0[3],
            255,
            "the icon covers the \"?\""
        );
        assert_eq!(picture.get_pixel(365, 700).0[3], 0, "nothing above");
    }

    /// The rows to a student sought stand at the foot of its icon: white letters on the
    /// dark ground, the icon above them as it was.
    #[test]
    fn the_rows_to_a_student_are_written_at_the_foot_of_its_icon() {
        let middle = (2232.0, 918.0);
        let seek = Mark::Seek {
            student: 10005,
            way: Way::Down,
            middle,
        };
        let with = |rows| draw(&[seek, Mark::Rows { rows, middle }], 2370, 1333);
        let (plain, two) = (draw(&[seek], 2370, 1333), with(2));
        // The icon is 100 a side about its middle; the letters' strip is its lowest 38 rows.
        let white = |picture: &image::RgbaImage| {
            let strip = (2182..2282).flat_map(|x| (930..968).map(move |y| (x, y)));
            strip
                .filter(|&(x, y)| picture.get_pixel(x, y).0 == [255; 4])
                .count()
        };
        assert_eq!(two.get_pixel(2184, 932), &GROUND, "the strip's ground");
        assert_eq!(two.get_pixel(2279, 966), &GROUND, "as wide as the icon");
        assert!(white(&two) > 60, "letters: {} white pixels", white(&two));
        assert_eq!(
            two.get_pixel(2232, 900),
            plain.get_pixel(2232, 900),
            "the icon above the strip stays"
        );
        assert_eq!(
            two.get_pixel(2232, 1000),
            plain.get_pixel(2232, 1000),
            "the arrow below stays"
        );
        // Another number is other letters; two digits take more room and still fit.
        assert_ne!(two, with(3));
        let twelve = with(12);
        assert!(white(&twelve) > white(&two));
        assert_eq!(
            twelve.get_pixel(2184, 932),
            &GROUND,
            "room left of two digits"
        );
        // A smaller window draws them smaller, at the icon's foot there.
        let small = draw(&[seek, Mark::Rows { rows: 2, middle }], 1920, 1080);
        assert_ne!(small.get_pixel(1808, 744).0[3], 0, "the icon's place");
        assert_eq!(small.get_pixel(1770, 756), &GROUND, "the strip's ground");
    }

    /// Words beside a record's title, and at a row's button.
    fn say(words: Words) -> Mark {
        Mark::Say {
            words,
            at: (1322.0, 226.0),
            tall: 56.0,
        }
    }

    #[test]
    fn words_stand_on_a_ground_of_their_own() {
        let kept = draw(&[say(Words::Kept)], 2370, 1333);
        let ground = *kept.get_pixel(1325, 226);
        assert!(
            ground.0[3] >= 200,
            "the ground begins at the words' left end"
        );
        assert_eq!(kept.get_pixel(1310, 226).0[3], 0, "nothing left of it");
        assert_eq!(kept.get_pixel(1340, 190).0[3], 0, "nothing above it");
        assert_eq!(kept.get_pixel(1340, 262).0[3], 0, "nothing under it");
        let white = |picture: &image::RgbaImage| {
            (1322..1700).any(|x| (198..254).any(|y| picture.get_pixel(x, y).0 == [255; 4]))
        };
        assert!(white(&kept), "the letters are white on the ground");
        // The ground tells the words apart as the letters do.
        for other in [Words::Known, Words::Reading] {
            let picture = draw(&[say(other)], 2370, 1333);
            assert_ne!(*picture.get_pixel(1325, 226), ground, "{other:?}");
            assert!(white(&picture), "{other:?}");
        }
    }

    #[test]
    fn the_ground_is_as_wide_as_the_words() {
        let at = |words| Mark::Say {
            words,
            at: (1635.0, 420.0),
            tall: 40.0,
        };
        let (read, unread) = (
            draw(&[at(Words::Read)], 2370, 1333),
            draw(&[at(Words::Unread)], 2370, 1333),
        );
        assert!(read.get_pixel(1785, 420).0[3] > 0, "six letters reach here");
        assert_eq!(unread.get_pixel(1785, 420).0[3], 0, "two do not");
        assert!(unread.get_pixel(1640, 420).0[3] > 0);
    }

    #[test]
    fn words_follow_the_windows_size() {
        // On a Full HD screen the measured (1325, 226) lies at (1073.4, 183.1).
        let picture = draw(&[say(Words::Kept)], 1920, 1080);
        assert!(picture.get_pixel(1074, 183).0[3] >= 200);
        assert_eq!(
            picture.get_pixel(1325, 226).0[3],
            0,
            "not where 2370 wide draws it"
        );
    }
}
