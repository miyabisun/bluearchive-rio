//! The design tokens of `DESIGN.md`: Sumi (dark) and Kinari (light), chosen on the settings
//! page or following the OS (自動). The values are Aoi's.

use gpui::{App, FontFeatures, Global, Hsla, Pixels, WindowAppearance, px, rgb, rgba};

use crate::{Armor, Attack, settings::ThemeChoice};

pub const SP_1: Pixels = px(4.);
pub const SP_2: Pixels = px(8.);
pub const SP_3: Pixels = px(12.);
pub const SP_4: Pixels = px(16.);

pub const RADIUS_SM: Pixels = px(6.);

pub const FS_CAPTION: Pixels = px(12.);
pub const FS_SM: Pixels = px(14.);
pub const FS_MD: Pixels = px(16.);

/// The OS's UI font.
#[cfg(windows)]
pub const FONT: &str = "Yu Gothic UI";
#[cfg(not(windows))]
pub const FONT: &str = ".SystemUIFont";

/// The frames of a striker's and of a special's icon in a list of students, and the ground
/// of a class's mark: the same in both themes, as the types' colours are.
pub const STRIKER: u32 = 0x00d9_463e;
pub const SPECIAL: u32 = 0x003b_82d6;
pub const CLASS: u32 = 0x0048_5582;

/// The colour SchaleDB tells an attack type by.
pub fn attack_colour(attack: Attack) -> Hsla {
    let colour = match attack {
        Attack::Explosion => 0x00a7_0c19,
        Attack::Pierce => 0x00b2_6d1f,
        Attack::Mystic => 0x0021_6f9c,
        Attack::Sonic => 0x0094_31a5,
    };
    rgb(colour).into()
}

/// The colour SchaleDB tells an armor type by: that of the attack type strong against it.
pub fn armor_colour(armor: Armor) -> Hsla {
    let colour = match armor {
        Armor::LightArmor => 0x00a7_0c19,
        Armor::HeavyArmor => 0x00b2_6d1f,
        Armor::Unarmed => 0x0021_6f9c,
        Armor::ElasticArmor => 0x0094_31a5,
        Armor::CompositeArmor => 0x0013_7973,
    };
    rgb(colour).into()
}

/// Figures of the same width, so that counts line up.
pub fn tabular() -> FontFeatures {
    FontFeatures(std::sync::Arc::new(vec![("tnum".into(), 1)]))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub dark: bool,
    pub surface: Hsla,
    pub on_surface: Hsla,
    pub muted: Hsla,
    pub border: Hsla,
    pub accent: Hsla,
    /// A text selected in a field.
    pub accent_subtle: Hsla,
    /// The corners of the place chosen (yellow).
    pub cursor: Hsla,
    pub danger: Hsla,
    pub danger_subtle: Hsla,
    pub ok: Hsla,
    pub wash_base: Hsla,
}

impl Global for Theme {}

impl Theme {
    pub fn sumi() -> Self {
        Self {
            dark: true,
            surface: rgb(0x0019_1919).into(),
            on_surface: rgb(0x00e6_e6e6).into(),
            muted: rgb(0x009a_9a9a).into(),
            border: rgb(0x0033_3333).into(),
            accent: rgb(0x005c_b3ff).into(),
            accent_subtle: rgba(0x5cb3_ff40).into(),
            cursor: rgb(0x00ff_d84d).into(),
            danger: rgb(0x00ff_6b6b).into(),
            danger_subtle: rgb(0x003a_1a1a).into(),
            ok: rgb(0x006f_cf8a).into(),
            wash_base: rgb(0x0023_2323).into(),
        }
    }

    pub fn kinari() -> Self {
        Self {
            dark: false,
            surface: rgb(0x00fa_f6ef).into(),
            on_surface: rgb(0x003a_2f28).into(),
            muted: rgb(0x006f_6257).into(),
            border: rgb(0x00e3_d9c9).into(),
            accent: rgb(0x001a_5fa8).into(),
            accent_subtle: rgba(0x1a5f_a833).into(),
            cursor: rgb(0x00e0_a800).into(),
            danger: rgb(0x009c_2b1d).into(),
            danger_subtle: rgb(0x00f9_e9e4).into(),
            ok: rgb(0x001d_6b35).into(),
            wash_base: rgb(0x00f6_efe0).into(),
        }
    }

    /// The theme chosen on the settings page, the OS's for 自動.
    pub fn chosen(choice: ThemeChoice, appearance: WindowAppearance) -> Self {
        match choice {
            ThemeChoice::Light => Self::kinari(),
            ThemeChoice::Dark => Self::sumi(),
            ThemeChoice::Auto => match appearance {
                WindowAppearance::Light | WindowAppearance::VibrantLight => Self::kinari(),
                WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::sumi(),
            },
        }
    }

    pub fn get(cx: &App) -> Self {
        cx.try_global::<Self>().copied().unwrap_or_else(Self::sumi)
    }
}
