//! The window, made as Aoi's (bluearchive-aoi): the tabs, the status and the settings button
//! on one row over the tab's content. It shows the snapshot `Core` keeps current (`app`);
//! what it derives for display comes from `view`.

pub mod icons;
// The window's tests find what is drawn by its `debug_selector`, which GPUI records in debug
// builds alone: built with `--release` they would find nothing, so they are not built there.
#[cfg(test)]
#[cfg(debug_assertions)]
mod tests;
pub mod theme;
mod widgets;

use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash as _, Hasher as _},
    io::Cursor,
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use futures::{StreamExt as _, channel::mpsc};
use gpui::{
    AnyElement, App, AppContext as _, AsyncApp, Bounds, Context, FocusHandle, Focusable,
    FontWeight, InteractiveElement as _, IntoElement, KeyBinding, ParentElement as _, Pixels,
    Render, ScrollHandle, SharedString, StatefulInteractiveElement as _, Styled as _, Task,
    TitlebarOptions, WeakEntity, Window, WindowBounds, WindowOptions, actions, div, img, point,
    prelude::FluentBuilder as _, px, size,
};

use crate::{
    app::{Core, Places, Snapshot, VERSION},
    settings::{Placement, Settings, ThemeChoice},
    store::{Kept, Player},
    view,
    watch::Status,
};

use self::{
    icons::{Assets, Glyph, glyph},
    theme::{FONT, FS_CAPTION, FS_MD, FS_SM, RADIUS_SM, SP_1, SP_2, SP_3, SP_4, Theme},
    widgets::text,
};

actions!(rio, [FocusNext, FocusPrev, Escape]);

/// The window's size as it first opens (Aoi's, to stand beside the game the same way), and
/// the least it is made: a history row's icons and a short name still fit.
const WINDOW: (f32, f32) = (720., 820.);
const WINDOW_LEAST: (f32, f32) = (420., 360.);
/// How long the window's place has to stay put before it is kept: a drag moves it many times
/// a second.
const PLACE_SETTLES: Duration = Duration::from_millis(500);

/// The sides of a student's icon: in the match tab's teams (at most and at least: a team
/// stays on one row, its icons as large as the window lets them), in its list of other
/// defenses and in the history's rows; and how tall a history row is.
const SEAT: (f32, f32) = (64., 36.);
const SMALL: Pixels = px(36.);
const BATTLE_ROW: Pixels = px(116.);
/// What a team's row takes beside its six icons: the gaps, the specials' distance and the
/// icons' frames.
const TEAM_GAPS: f32 = 68.;
/// How tall a name drawn by the game is shown.
const PLATE: Pixels = px(20.);

/// The tabs, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tab {
    /// The opponent the game showed last: the defense expected and the attack counselled.
    Match,
    /// The battles kept.
    History,
}

impl Tab {
    pub const ALL: [Self; 2] = [Self::Match, Self::History];

    pub fn name(self) -> &'static str {
        match self {
            Self::Match => "対戦",
            Self::History => "履歴",
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::Match => "match",
            Self::History => "history",
        }
    }
}

/// A student's place in a team as the window shows it.
struct Seat {
    place: &'static str,
    /// None where the game hides the student and nothing is expected.
    student: Option<i64>,
    /// Expected from the records, not shown by the game.
    guessed: bool,
}

/// How a battle's player is shown: by the name the user gave, by the name as the game draws
/// it (its picture and its width for one of height), or as nobody.
#[derive(Clone)]
enum Shown {
    Anonymous,
    Named(String),
    Drawn(Arc<gpui::Image>, f32, i64),
}

pub struct Rio {
    core: Arc<Core>,
    /// The snapshot shown, taken from `core` on every change.
    shown: Snapshot,
    settings: Settings,
    tab: Tab,
    /// Whether the settings page shows in the tabs' place.
    settings_open: bool,
    /// The window's own focus, where it starts.
    root_focus: FocusHandle,
    /// The students' names, by id.
    names: Rc<HashMap<i64, String>>,
    /// The names drawn by the game, as pictures in the theme's text colour: by what the
    /// plate holds and whether the theme is dark.
    plates: HashMap<(u64, bool), (Arc<gpui::Image>, f32)>,
    content: ScrollHandle,
    /// Keeps the window's place once it stops moving (see `PLACE_SETTLES`).
    keeping_place: Option<Task<()>>,
    _changes: Task<()>,
}

impl Rio {
    pub fn new(
        core: Arc<Core>,
        mut changes: mpsc::UnboundedReceiver<()>,
        settings: Settings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let shown = core.snapshot();
        let changes = cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            while changes.next().await.is_some() {
                // Every change since is in the one snapshot taken.
                while changes.try_recv().is_ok() {}
                if this.update(cx, Rio::refresh).is_err() {
                    break;
                }
            }
        });
        cx.observe_window_appearance(window, |this: &mut Self, window, cx| {
            this.apply_theme(window, cx);
        })
        .detach();
        cx.observe_window_bounds(window, |this: &mut Self, window, cx| {
            this.place_changed(window, cx);
        })
        .detach();
        let names = crate::students()
            .into_iter()
            .map(|student| (student.id, student.name))
            .collect();
        Self {
            core,
            shown,
            settings,
            tab: Tab::Match,
            settings_open: false,
            root_focus: cx.focus_handle(),
            names: Rc::new(names),
            plates: HashMap::new(),
            content: ScrollHandle::new(),
            keeping_place: None,
            _changes: changes,
        }
    }

    /// Takes the snapshot now.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.shown = self.core.snapshot();
        cx.notify();
    }

    fn apply_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.set_global(Theme::chosen(self.settings.theme, window.appearance()));
        cx.notify();
    }

    fn keep_settings(&self) {
        let _ = self.settings.save(&self.core.places.settings);
    }

    /// Notes where the window is now, to open there next time, and keeps it once it settles.
    fn place_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(display) = window.display(cx).and_then(|d| d.uuid().ok()) else {
            return;
        };
        // Maximized, the place it goes back to.
        let bounds = window.window_bounds().get_bounds();
        let whole = |p: Pixels| f32::from(p).round() as i32;
        self.settings.window = Some(Placement {
            display: display.to_string(),
            x: whole(bounds.origin.x),
            y: whole(bounds.origin.y),
            width: whole(bounds.size.width),
            height: whole(bounds.size.height),
        });
        // A newer change replaces (and so cancels) the wait for the older one.
        self.keeping_place = Some(cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                cx.background_executor().timer(PLACE_SETTLES).await;
                let _ = this.update(cx, |this, _| this.keep_settings());
            },
        ));
    }

    /// A name as the game draws it (a plate's PNG: its ink bright on black) as a picture in
    /// the theme's text colour, with its width for a height of one.
    fn plate(&mut self, png: &[u8], theme: &Theme) -> Option<(Arc<gpui::Image>, f32)> {
        let mut hasher = DefaultHasher::new();
        png.hash(&mut hasher);
        let key = (hasher.finish(), theme.dark);
        if let Some(known) = self.plates.get(&key) {
            return Some(known.clone());
        }
        let ink = image::load_from_memory(png).ok()?.to_luma8();
        let colour = gpui::Rgba::from(theme.on_surface);
        let [r, g, b] = [colour.r, colour.g, colour.b].map(|c| (c * 255.).round() as u8);
        let picture = image::RgbaImage::from_fn(ink.width(), ink.height(), |x, y| {
            image::Rgba([r, g, b, ink.get_pixel(x, y).0[0]])
        });
        let mut bytes = Vec::new();
        picture
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .ok()?;
        let drawn = (
            Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Png, bytes)),
            ink.width() as f32 / ink.height().max(1) as f32,
        );
        self.plates.insert(key, drawn.clone());
        Some(drawn)
    }

    /// How a battle's player is shown.
    fn player(&mut self, player: Option<&Player>, theme: &Theme) -> Shown {
        match player {
            None => Shown::Anonymous,
            Some(Player {
                name: Some(name), ..
            }) => Shown::Named(name.clone()),
            Some(player) => match self.plate(&player.plate, theme) {
                Some((picture, wide)) => Shown::Drawn(picture, wide, player.id),
                None => Shown::Named(format!("プレイヤー#{}", player.id)),
            },
        }
    }

    fn header(&mut self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let battles = self.shown.battles.len();
        let tabs = div()
            .id("tabs")
            .role(gpui::Role::TabList)
            .flex()
            .flex_1()
            .min_w_0()
            .px(SP_1)
            .overflow_x_scroll()
            .children(Tab::ALL.into_iter().map(|tab| {
                let selected = self.tab == tab && !self.settings_open;
                let counted = tab == Tab::History && battles > 0;
                let name = if counted {
                    format!("{} {battles}", tab.name())
                } else {
                    tab.name().to_owned()
                };
                div()
                    .id(SharedString::from(format!("tab-{}", tab.id())))
                    .debug_selector(move || format!("tab:{}", tab.name()))
                    .role(gpui::Role::Tab)
                    .aria_label(name)
                    .aria_selected(selected)
                    .tab_stop(true)
                    .flex()
                    .flex_none()
                    .items_center()
                    .h(px(40.))
                    .px(SP_3)
                    .border_b_2()
                    .border_color(if selected {
                        theme.accent
                    } else {
                        gpui::transparent_black()
                    })
                    .text_size(FS_SM)
                    .text_color(if selected {
                        theme.on_surface
                    } else {
                        theme.muted
                    })
                    // Every tab as bold as the chosen one: a bolder name alone would widen it
                    // and move the tabs after it. The rule and the colour tell the chosen.
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor_pointer()
                    .child(tab.name())
                    .when(counted, |t| {
                        t.child(
                            div()
                                .font_weight(FontWeight::NORMAL)
                                .text_color(theme.muted)
                                .font_features(theme::tabular())
                                .child(text(format!("\u{a0}{battles}"))),
                        )
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.tab = tab;
                        this.settings_open = false;
                        cx.notify();
                    }))
            }));
        let status = view::status_view(&self.shown.status);
        let paused = self.shown.status == Status::Paused;
        let (lamp, words) = match status.tone {
            view::Tone::Active => (theme.accent, theme.on_surface),
            view::Tone::Danger => (theme.danger, theme.danger),
            view::Tone::Paused => (theme.danger, theme.on_surface),
            view::Tone::Idle => (theme.muted, theme.muted),
        };
        let hover = theme.muted;
        let badge = div()
            .id("status")
            .debug_selector(|| "status".to_owned())
            .role(gpui::Role::Button)
            .aria_label(status.label.clone())
            .aria_description(status.title.clone())
            .aria_toggled(if paused {
                gpui::Toggled::True
            } else {
                gpui::Toggled::False
            })
            .tab_stop(true)
            .flex()
            .flex_none()
            .items_center()
            .gap(SP_2)
            .px(SP_3)
            .py(px(2.))
            .border_1()
            .border_color(theme.border)
            .rounded_full()
            .text_size(FS_SM)
            .text_color(words)
            .whitespace_nowrap()
            .cursor_pointer()
            .hover(move |s| s.border_color(hover))
            .focus(move |s| s.border_color(hover))
            .tooltip(widgets::tooltip(status.title.clone()))
            .child(glyph(Glyph::Window, px(14.), lamp))
            .child(text(status.label.clone()))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.core.set_paused(!paused);
                cx.notify();
            }));
        let settings_button = div()
            .id("settings-button")
            .debug_selector(|| "settings-button".to_owned())
            .role(gpui::Role::Button)
            .aria_label("設定")
            .when(self.settings_open, |b| b.aria_description("開いています"))
            .tab_stop(true)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(32.))
            .rounded(RADIUS_SM)
            .cursor_pointer()
            .child(glyph(Glyph::Menu, px(20.), theme.on_surface))
            .on_click(cx.listener(|this, _, _, cx| {
                this.settings_open = !this.settings_open;
                cx.notify();
            }));
        div()
            .flex_none()
            .bg(theme.wash_base)
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(SP_2)
                    .pr(SP_3)
                    .child(tabs)
                    .child(
                        div()
                            .id("status-slot")
                            .ml_auto()
                            .flex_none()
                            .role(gpui::Role::Status)
                            .child(badge),
                    )
                    .child(div().flex_none().child(settings_button)),
            )
    }

    /// The opponent the game showed last: the defense expected behind the front, the attack
    /// the records counsel against it, and the other defenses seen behind the same front.
    fn match_tab(&mut self, theme: &Theme, window: &Window) -> AnyElement {
        let page = scroll("match", &self.content);
        let wide = f32::from(window.viewport_size().width - SP_3 * 2.);
        let seat = px(((wide - TEAM_GAPS) / 6.).clamp(SEAT.1, SEAT.0));
        let Some(opponent) = self.shown.opponent.clone() else {
            let waiting = "対戦相手を選ぶと、防衛の編成の予測と、おすすめの攻撃がここに出ます。";
            return page
                .child(widgets::empty(theme, "match", waiting))
                .into_any_element();
        };
        let (front, counsel) = (opponent.front, &opponent.counsel);
        let behind = counsel.behind();
        let hidden = |place, at: usize| Seat {
            place,
            student: behind.map(|behind| behind[at]),
            guessed: true,
        };
        let shown = |place, student| Seat {
            place,
            student: Some(student),
            guessed: false,
        };
        let defense = [
            shown("D1", front.first),
            hidden("D2", 0),
            hidden("D3", 1),
            hidden("D4", 2),
            shown("SP", front.specials[0]),
            shown("SP", front.specials[1]),
        ];
        let name = opponent
            .name
            .as_ref()
            .and_then(|name| self.plate(&name.png(), theme))
            .map(|(picture, wide)| plate_picture(picture, wide, "相手の名前"));
        let names = self.names.clone();
        let mut page = page
            .child(group(theme, "相手の防衛", name))
            .child(team(theme, &names, "defense", &defense, seat))
            .child(
                widgets::quiet(theme, view::expected_from(counsel))
                    .mt(SP_2)
                    .mb(SP_4),
            );
        if behind.is_some() {
            let fared = counsel.attack.as_ref().map(|tried| {
                div()
                    .text_size(FS_SM)
                    .font_weight(FontWeight::NORMAL)
                    .font_features(theme::tabular())
                    .child(text(view::fared(tried)))
                    .into_any_element()
            });
            page = page.child(group(theme, "おすすめの攻撃", fared));
            page = match &counsel.attack {
                Some(tried) => {
                    let places = ["A1", "A2", "A3", "A4", "SP", "SP"];
                    let attack: Vec<Seat> = places
                        .into_iter()
                        .zip(tried.attack)
                        .map(|(place, student)| shown(place, student))
                        .collect();
                    page.child(team(theme, &names, "attack", &attack, seat).mb(SP_4))
                }
                None => {
                    let none = "この防衛に勝った攻撃は、まだ登録がありません。";
                    page.child(widgets::quiet(theme, none).mb(SP_4))
                }
            };
        }
        let others: Vec<_> = counsel
            .seen
            .iter()
            .filter(|seen| Some(seen.rest) != behind)
            .collect();
        if !others.is_empty() {
            page = page.child(group(theme, "ほかに見た防衛", None));
            for (at, seen) in others.into_iter().enumerate() {
                let faces = seen.rest.into_iter().enumerate().map(|(slot, student)| {
                    face(theme, &names, format!("seen-{at}-{slot}"), student, SMALL)
                });
                page = page.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(SP_2)
                        .mb(SP_2)
                        .child(
                            div()
                                .w(px(40.))
                                .flex_none()
                                .text_size(FS_SM)
                                .font_features(theme::tabular())
                                .child(text(format!("{}回", seen.battles))),
                        )
                        .child(div().flex().gap(SP_1).children(faces))
                        .child(
                            div()
                                .text_size(FS_CAPTION)
                                .text_color(theme.muted)
                                .font_features(theme::tabular())
                                .child(format!("最後に登録 {}", view::kept_at(&seen.last_seen))),
                        ),
                );
            }
        }
        page.into_any_element()
    }

    /// The battles kept, the latest first: who attacked whom with which students, and who
    /// won. Only the rows in sight are drawn.
    fn history_tab(&mut self, theme: &Theme) -> AnyElement {
        if self.shown.battles.is_empty() {
            let none = "ゲームで戦闘履歴を開くと、ここに登録されます。";
            return scroll("history", &self.content)
                .child(widgets::empty(theme, "history", none))
                .into_any_element();
        }
        let battles = self.shown.battles.clone();
        let rows: Vec<(Kept, Shown, Shown)> = battles
            .into_iter()
            .map(|battle| {
                let attacker = self.player(battle.attacker.as_ref(), theme);
                let defender = self.player(battle.defender.as_ref(), theme);
                (battle, attacker, defender)
            })
            .collect();
        let (rows, names) = (Rc::new(rows), self.names.clone());
        gpui::uniform_list("history", rows.len(), move |range, _window, cx| {
            let theme = Theme::get(cx);
            range
                .map(|at| {
                    let (battle, attacker, defender) = &rows[at];
                    battle_row(&theme, &names, battle, attacker, defender)
                })
                .collect()
        })
        .flex_1()
        .min_h_0()
        .into_any_element()
    }

    /// The settings, in the tabs' place: the theme, where the records are kept, the version.
    fn settings_page(&mut self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let themes = ThemeChoice::ALL.into_iter().map(|(choice, name)| {
            widgets::choice(
                theme,
                &format!("theme-{name}"),
                name,
                self.settings.theme == choice,
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.settings.theme = choice;
                this.keep_settings();
                this.apply_theme(window, cx);
            }))
        });
        let records = self.core.places.database.display().to_string();
        scroll("settings", &self.content)
            .child(group(theme, "テーマ", None))
            .child(div().flex().flex_wrap().gap(SP_1).mb(SP_4).children(themes))
            .child(group(theme, "記録の保存先", None))
            .child(widgets::quiet(theme, records).mb(SP_4))
            .child(group(theme, "バージョン", None))
            .child(widgets::quiet(theme, format!("Rio v{VERSION}")))
            .into_any_element()
    }
}

/// A group of a page under its heading (body-sm, bold, over a rule), with what stands at the
/// heading's right end.
fn group(theme: &Theme, heading: &str, aside: Option<AnyElement>) -> gpui::Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_between()
        .gap(SP_2)
        .mb(SP_2)
        .pb(SP_1)
        .border_b_1()
        .border_color(theme.border)
        .text_size(FS_SM)
        .font_weight(FontWeight::SEMIBOLD)
        .child(text(heading.to_owned()))
        .children(aside)
}

/// A name as the game draws it, `PLATE` tall.
fn plate_picture(picture: Arc<gpui::Image>, wide: f32, label: &str) -> AnyElement {
    div()
        .id(SharedString::from(format!("plate-{label}")))
        .role(gpui::Role::Image)
        .aria_label(label.to_owned())
        .flex_none()
        .child(img(picture).h(PLATE).w(PLATE * wide))
        .into_any_element()
}

/// A student's icon in a square of `size`, the name on a hover; `key` tells it from the
/// window's other icons.
fn face(
    theme: &Theme,
    names: &HashMap<i64, String>,
    key: String,
    student: i64,
    size: Pixels,
) -> gpui::Stateful<gpui::Div> {
    let name = names
        .get(&student)
        .cloned()
        .unwrap_or_else(|| format!("生徒#{student}"));
    div()
        .id(SharedString::from(format!("face-{key}")))
        .role(gpui::Role::Image)
        .aria_label(name.clone())
        .tooltip(widgets::tooltip(name))
        .flex_none()
        .size(size)
        .rounded(RADIUS_SM)
        .overflow_hidden()
        .bg(theme.wash_base)
        .child(icons::student(student, size))
}

/// A team as the game lines it up, its icons `size` square: the four strikers, then the two
/// specials apart, each with its place under it. A student expected from the records, not
/// shown by the game, has a dashed frame; a place with nobody expected shows "?".
fn team(
    theme: &Theme,
    names: &HashMap<i64, String>,
    key: &str,
    seats: &[Seat],
    size: Pixels,
) -> gpui::Div {
    let seat = |(at, seat): (usize, &Seat)| {
        let named = seat.student.map(|student| {
            names
                .get(&student)
                .cloned()
                .unwrap_or_else(|| format!("生徒#{student}"))
        });
        let selector = format!("{key}:{}", named.unwrap_or_else(|| "?".to_owned()));
        let picture = match seat.student {
            Some(student) => face(theme, names, format!("{key}-{at}"), student, size)
                .border_1()
                .border_color(if seat.guessed {
                    theme.muted
                } else {
                    theme.border
                })
                .when(seat.guessed, gpui::Styled::border_dashed)
                .into_any_element(),
            None => div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(size)
                .rounded(RADIUS_SM)
                .border_1()
                .border_dashed()
                .border_color(theme.border)
                .text_color(theme.muted)
                .child("?")
                .into_any_element(),
        };
        // The word fits under an icon of the full size alone; the dashed frame says it too.
        let place = if seat.guessed && seat.student.is_some() && size >= px(SEAT.0) {
            format!("{} 予測", seat.place)
        } else {
            seat.place.to_owned()
        };
        div()
            .debug_selector(move || selector.clone())
            .flex()
            .flex_col()
            .flex_none()
            .items_center()
            .gap(px(2.))
            // The specials stand apart from the strikers.
            .when(at == 4, |seat| seat.ml(SP_3))
            .child(picture)
            .child(
                div()
                    .text_size(FS_CAPTION)
                    .text_color(theme.muted)
                    .whitespace_nowrap()
                    .child(place),
            )
    };
    div()
        .flex()
        .flex_none()
        .gap(SP_2)
        .children(seats.iter().enumerate().map(seat))
}

/// One side of a battle: its role, the mark of the winner, the player (in the room the window
/// leaves, cut where it ends), and the team.
fn side_line(
    theme: &Theme,
    names: &HashMap<i64, String>,
    (battle, role, key): (i64, &'static str, &'static str),
    player: &Shown,
    won: bool,
    team: [i64; 6],
) -> gpui::Div {
    let who = match player {
        Shown::Anonymous => div()
            .text_color(theme.muted)
            .child(text("匿名"))
            .into_any_element(),
        Shown::Named(name) => div()
            .truncate()
            .child(text(name.clone()))
            .into_any_element(),
        Shown::Drawn(picture, wide, player) => {
            plate_picture(picture.clone(), *wide, &format!("プレイヤー#{player}"))
        }
    };
    let faces = team.into_iter().enumerate().map(|(slot, student)| {
        face(
            theme,
            names,
            format!("{battle}-{key}-{slot}"),
            student,
            SMALL,
        )
        .when(slot == 4, |face| face.ml(SP_2))
    });
    div()
        .debug_selector(move || format!("side:{battle}:{key}"))
        .flex()
        .items_center()
        .gap(SP_2)
        .w_full()
        .child(
            div()
                .w(px(32.))
                .flex_none()
                .text_size(FS_CAPTION)
                .text_color(theme.muted)
                .child(role),
        )
        .child(
            div()
                .w(px(36.))
                .flex_none()
                .text_size(FS_SM)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.ok)
                .when(won, |mark| {
                    mark.debug_selector(move || format!("won:{battle}:{key}"))
                        .child("勝ち")
                }),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .text_size(FS_SM)
                .child(who),
        )
        .child(div().flex().flex_none().gap(SP_1).children(faces))
}

/// A battle of the history: when it was kept, then the attacker's line over the defender's,
/// the winner's marked.
fn battle_row(
    theme: &Theme,
    names: &HashMap<i64, String>,
    battle: &Kept,
    attacker: &Shown,
    defender: &Shown,
) -> impl IntoElement + use<> {
    let id = battle.id;
    div()
        .id(SharedString::from(format!("battle-{id}")))
        .debug_selector(move || format!("battle:{id}"))
        .role(gpui::Role::ListItem)
        .h(BATTLE_ROW)
        .w_full()
        .flex()
        .flex_col()
        .justify_center()
        .gap(px(2.))
        .border_b_1()
        .border_color(theme.border)
        .child(
            div()
                .text_size(FS_CAPTION)
                .text_color(theme.muted)
                .font_features(theme::tabular())
                .child(format!("登録 {}", view::kept_at(&battle.seen_at))),
        )
        .child(side_line(
            theme,
            names,
            (id, "攻撃", "attack"),
            attacker,
            battle.attacker_won,
            battle.attack,
        ))
        .child(side_line(
            theme,
            names,
            (id, "防衛", "defense"),
            defender,
            !battle.attacker_won,
            battle.defense,
        ))
}

/// The box a tab's content scrolls in.
fn scroll(id: &str, handle: &ScrollHandle) -> gpui::Stateful<gpui::Div> {
    div()
        .id(SharedString::from(id.to_owned()))
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(handle)
}

impl Focusable for Rio {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.root_focus.clone()
    }
}

impl Render for Rio {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let header = self.header(&theme, cx);
        let failure = match &self.shown.status {
            Status::Failed(message) => Some(widgets::error(&theme, "read-error", message.clone())),
            _ => None,
        };
        let content = if self.settings_open {
            self.settings_page(&theme, cx)
        } else {
            match self.tab {
                Tab::Match => self.match_tab(&theme, window),
                Tab::History => self.history_tab(&theme),
            }
        };
        div()
            .id("rio")
            .key_context("Rio")
            .track_focus(&self.root_focus)
            .on_action(cx.listener(|_, _: &FocusNext, window, cx| window.focus_next(cx)))
            .on_action(cx.listener(|_, _: &FocusPrev, window, cx| window.focus_prev(cx)))
            .on_action(cx.listener(|this, _: &Escape, _, cx| {
                this.settings_open = false;
                cx.notify();
            }))
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.surface)
            .text_color(theme.on_surface)
            .font_family(FONT)
            .text_size(FS_MD)
            .line_height(gpui::relative(1.6))
            .child(header)
            .child(
                div()
                    .id("list")
                    .role(gpui::Role::TabPanel)
                    .aria_label(if self.settings_open {
                        "設定"
                    } else {
                        self.tab.name()
                    })
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .pt(SP_4)
                    .px(SP_3)
                    .children(failure)
                    // The tab's pictures are kept while it shows, and dropped as another opens.
                    .child(
                        gpui::image_cache(gpui::retain_all(SharedString::from(format!(
                            "images-{:?}-{}",
                            self.tab, self.settings_open
                        ))))
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h_0()
                        .child(content),
                    ),
            )
    }
}

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("tab", FocusNext, None),
        KeyBinding::new("shift-tab", FocusPrev, None),
        KeyBinding::new("escape", Escape, None),
    ]);
}

/// Where the window opens, when it was `left` on one of `displays` (each by its id in the
/// settings): on that one, where it was and as big; `None` when nothing was kept or that
/// monitor is gone, to open as at first.
pub fn opening_place<D: Copy>(
    left: Option<&Placement>,
    displays: &[(String, D)],
) -> Option<(D, Bounds<Pixels>)> {
    let left = left?;
    let (_, display) = displays.iter().find(|(id, _)| *id == left.display)?;
    let bounds = Bounds::new(
        point(px(left.x as f32), px(left.y as f32)),
        size(px(left.width as f32), px(left.height as f32)),
    );
    Some((*display, bounds))
}

/// Opens the window over what `core` keeps, its changes signalled on `changes`: where it was
/// left (`opening_place`), or else centred on the primary monitor.
pub fn open(
    core: Arc<Core>,
    changes: mpsc::UnboundedReceiver<()>,
    settings: Settings,
    cx: &mut App,
) -> gpui::WindowHandle<Rio> {
    let displays: Vec<_> = cx
        .displays()
        .iter()
        .filter_map(|d| Some((d.uuid().ok()?.to_string(), d.id())))
        .collect();
    let (display_id, bounds) = match opening_place(settings.window.as_ref(), &displays) {
        Some((display, bounds)) => (Some(display), bounds),
        None => (
            None,
            Bounds::centered(None, size(px(WINDOW.0), px(WINDOW.1)), cx),
        ),
    };
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(WINDOW_LEAST.0), px(WINDOW_LEAST.1))),
            display_id,
            // Shown without taking the focus from the game being played, as Aoi is.
            focus: false,
            titlebar: Some(TitlebarOptions {
                title: Some("Rio".into()),
                ..Default::default()
            }),
            app_id: Some(crate::app::IDENTIFIER.into()),
            ..Default::default()
        },
        |window, cx| {
            cx.set_global(Theme::chosen(settings.theme, window.appearance()));
            let rio = cx.new(|cx| Rio::new(core, changes, settings, window, cx));
            rio.focus_handle(cx).focus(window, cx);
            rio
        },
    )
    .expect("the window opens")
}

/// Runs the app: reads the game, and shows what it finds in the window.
pub fn run(places: Places) {
    let settings = Settings::load(&places.settings);
    gpui_platform::application()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            bind_keys(cx);
            let (changed, changes) = mpsc::unbounded();
            let core = Arc::new(Core::new(places, changed));
            core.watch();
            let _ = open(core, changes, settings, cx);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
        });
}
