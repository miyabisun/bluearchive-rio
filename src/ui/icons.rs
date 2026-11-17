//! What the window draws from pictures, served to GPUI as assets: the students' icons built
//! into the program (`student/<id>.webp`) and the line icons drawn in the text colour
//! (`glyph/<name>.svg`).

use std::borrow::Cow;

use gpui::{AssetSource, Hsla, Img, Pixels, SharedString, Styled, Svg, img, svg};

/// A line icon, drawn in the text colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    /// The game's window: where the frames read come from.
    Window,
    Menu,
}

impl Glyph {
    fn name(self) -> &'static str {
        match self {
            Self::Window => "window",
            Self::Menu => "menu",
        }
    }

    fn svg(self) -> &'static str {
        match self {
            Self::Window => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="black" stroke-width="1.5" stroke-linecap="round"><rect x="1.25" y="2.25" width="13.5" height="9" rx="1"/><path d="M8 11.25v3M5 14.5h6"/></svg>"#
            }
            Self::Menu => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M4 7h16M4 12h16M4 17h16" stroke="black" stroke-width="2" stroke-linecap="round"/></svg>"#
            }
        }
    }

    const ALL: [Self; 2] = [Self::Window, Self::Menu];
}

/// `glyph` at `size`, in `color`.
pub fn glyph(glyph: Glyph, size: Pixels, color: Hsla) -> Svg {
    svg()
        .path(format!("glyph/{}.svg", glyph.name()))
        .size(size)
        .flex_none()
        .text_color(color)
}

/// A student's icon, `size` square.
pub fn student(id: i64, size: Pixels) -> Img {
    img(SharedString::from(format!("student/{id}.webp")))
        .size(size)
        .flex_none()
}

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if let Some(id) = path
            .strip_prefix("student/")
            .and_then(|name| name.strip_suffix(".webp"))
        {
            let icon = id.parse().ok().and_then(crate::icon);
            return Ok(icon.map(Cow::Borrowed));
        }
        let Some(name) = path
            .strip_prefix("glyph/")
            .and_then(|path| path.strip_suffix(".svg"))
        else {
            return Ok(None);
        };
        Ok(Glyph::ALL
            .into_iter()
            .find(|glyph| glyph.name() == name)
            .map(|glyph| Cow::Borrowed(glyph.svg().as_bytes())))
    }

    fn list(&self, _path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}
