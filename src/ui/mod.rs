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
    app::{Core, Places, Snapshot, UpdateCheck, VERSION},
    settings::{Placement, Settings, ThemeChoice},
    store::{Kept, Player},
    view::{self, UpdateState},
    watch::{Choice, Status},
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
/// How long a toast stays before it goes.
const TOAST_STAYS: Duration = Duration::from_secs(3);

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
/// The room of a candidate's rank before its row.
const RANK: Pixels = px(16.);

/// The tabs, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tab {
    /// The opponent the game showed last: the defenses likely behind its front and the
    /// attacks that won against each, to choose from.
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

/// How the window asks for the latest release and takes it: GitHub's in the app, a test's own
/// in the tests.
#[derive(Clone, Copy)]
pub struct Updates {
    pub check: fn() -> Result<UpdateCheck, String>,
    pub apply: fn() -> Result<(), String>,
    /// Starts the app again, as the version taken.
    pub restart: fn(),
}

impl Default for Updates {
    fn default() -> Self {
        Self {
            check: crate::app::check_update,
            apply: crate::app::apply_update,
            restart: || crate::app::restart(),
        }
    }
}

pub struct Rio {
    core: Arc<Core>,
    updates: Updates,
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
    /// Where an update stands while it stays in the page: a newer version, taking it, a
    /// failure (checking and up to date are toasts).
    update: Option<UpdateState>,
    toast: Option<SharedString>,
    toast_timer: Option<Task<()>>,
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
            updates: Updates::default(),
            shown,
            settings,
            tab: Tab::Match,
            settings_open: false,
            root_focus: cx.focus_handle(),
            names: Rc::new(names),
            plates: HashMap::new(),
            content: ScrollHandle::new(),
            keeping_place: None,
            update: None,
            toast: None,
            toast_timer: None,
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

    /// Shows `text` as a toast, gone after a while unless it `stays`.
    fn notify_toast(&mut self, text: Option<&str>, stays: bool, cx: &mut Context<Self>) {
        self.toast = text.map(|t| SharedString::from(t.to_owned()));
        self.toast_timer = None;
        if text.is_some() && !stays {
            self.toast_timer = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(TOAST_STAYS).await;
                let _ = this.update(cx, |this, cx| {
                    this.toast = None;
                    cx.notify();
                });
            }));
        }
        cx.notify();
    }

    /// Checks for an update: while checking and when up to date, a toast says so; a newer
    /// version (with the button to take it) and a failure stay in the page.
    fn check_update(&mut self, cx: &mut Context<Self>) {
        self.update = None;
        self.notify_toast(Some(&view::update_text(&UpdateState::Checking)), true, cx);
        let check = self.updates.check;
        let found = cx.background_executor().spawn(async move { check() });
        cx.spawn(async move |this, cx| {
            let found = found.await;
            let _ = this.update(cx, |this, cx| match found {
                Ok(UpdateCheck {
                    current,
                    latest: Some(latest),
                }) => {
                    this.notify_toast(None, false, cx);
                    this.update = Some(UpdateState::Available { current, latest });
                }
                Ok(UpdateCheck { current, .. }) => {
                    let text = view::update_text(&UpdateState::Latest { current });
                    this.notify_toast(Some(&text), false, cx);
                }
                Err(error) => {
                    this.notify_toast(None, false, cx);
                    this.update = Some(UpdateState::Failed { error });
                }
            });
        })
        .detach();
    }

    /// Takes the update found; on success the app starts again as it.
    fn install(&mut self, cx: &mut Context<Self>) {
        let Some(UpdateState::Available { latest, .. }) = self.update.clone() else {
            return;
        };
        self.update = Some(UpdateState::Installing { latest });
        cx.notify();
        let apply = self.updates.apply;
        let applied = cx.background_executor().spawn(async move { apply() });
        cx.spawn(async move |this, cx| {
            let applied = applied.await;
            let _ = this.update(cx, |this, cx| match applied {
                Ok(()) => (this.updates.restart)(),
                Err(error) => {
                    this.update = Some(UpdateState::Failed { error });
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Where the update stands, over the page: a newer version with the button to take it,
    /// taking it, or why it failed.
    fn update_notice(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let update = self.update.as_ref()?;
        let told = view::update_text(update);
        Some(match update {
            UpdateState::Failed { .. } => {
                widgets::error(theme, "update-error", told).into_any_element()
            }
            UpdateState::Available { .. } => widgets::notice(theme, "update", told)
                .flex()
                .items_center()
                .child(
                    widgets::outlined_button(theme, "install", "更新して再起動", true)
                        .ml(SP_3)
                        .on_click(cx.listener(|this, _, _, cx| this.install(cx))),
                )
                .into_any_element(),
            _ => widgets::notice(theme, "update", told).into_any_element(),
        })
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
                div()
                    .id(SharedString::from(format!("tab-{}", tab.id())))
                    .debug_selector(move || format!("tab:{}", tab.name()))
                    .role(gpui::Role::Tab)
                    .aria_label(tab.name())
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

    /// The opponent the game showed last: its front as the game shows it, and the defenses
    /// that may stand behind it, the likeliest first, each with why it is likely and the
    /// attacks that won against it. A click chooses an attack with its defense (or, on a
    /// defense's own row, the defense alone), and a click on what is chosen gives it up:
    /// nothing is chosen for the user. What is chosen is marked over the game.
    fn match_tab(&mut self, theme: &Theme, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let page = scroll("match", &self.content);
        let wide = f32::from(window.viewport_size().width - SP_3 * 2.);
        let seat = px(((wide - TEAM_GAPS) / 6.).clamp(SEAT.1, SEAT.0));
        let Some(opponent) = self.shown.opponent.clone() else {
            let waiting = "対戦相手を選ぶと、防衛の編成の候補と、勝った攻撃の候補がここに出ます。";
            return page
                .child(widgets::empty(theme, "match", waiting))
                .into_any_element();
        };
        let chosen = self.core.chosen().filter(|choice| choice.of(&opponent));
        let (front, counsel) = (opponent.front, &opponent.counsel);
        let behind = chosen.as_ref().map(|choice| choice.rest);
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
        let state = match (counsel.likely.is_empty(), &chosen) {
            (true, _) => "同じ D1 とスペシャルの防衛は、まだ登録がありません。",
            (false, None) => "D2〜D4 は、下の候補から選ぶと出ます。",
            (false, Some(_)) => "D2〜D4 は、選んだ候補です。",
        };
        let mut page = page
            .child(group(theme, "相手の防衛", name))
            .child(team(theme, &names, "defense", &defense, seat))
            .child(widgets::quiet(theme, state).mt(SP_2).mb(SP_4));
        if !counsel.likely.is_empty() {
            let how = widgets::quiet(theme, "攻撃を選ぶと、編成の画面で案内します")
                .font_weight(FontWeight::NORMAL)
                .into_any_element();
            page = page.child(group(theme, "防衛の候補", Some(how)));
        }
        let named = |students: &[i64]| -> String {
            let name = |student: &i64| {
                let known = names.get(student).cloned();
                known.unwrap_or_else(|| format!("生徒#{student}"))
            };
            students.iter().map(name).collect::<Vec<_>>().join("、")
        };
        let mut ground = None;
        for (rank, likely) in counsel.likely.iter().enumerate() {
            // What makes the defenses from here on likely: the opponent's own, the trend...
            if ground != Some(likely.ground) {
                ground = Some(likely.ground);
                let selector = format!("ground:likely-{rank}");
                let heading = widgets::quiet(theme, view::ground(likely.ground))
                    .debug_selector(move || selector.clone())
                    .text_size(FS_CAPTION)
                    .when(rank > 0, |heading| heading.mt(SP_3));
                page = page.child(heading);
            }
            let picked = chosen.as_ref().filter(|choice| choice.rest == likely.rest);
            let choose = |attack: Option<[i64; 6]>| Choice {
                front,
                name: opponent.name.clone(),
                rest: likely.rest,
                attack,
            };
            // The defense's own row: its rank, the whole team as the game lines it up (the
            // front it shows, and behind the first striker those Rio supplies), and why.
            let alone = picked.is_some_and(|choice| choice.attack.is_none());
            let to = (!alone).then(|| choose(None));
            let [second, third, fourth] = likely.rest;
            let [left, right] = front.specials;
            let whole = [front.first, second, third, fourth, left, right];
            let faces = whole.into_iter().enumerate().map(|(slot, student)| {
                let supplied = (1..4).contains(&slot);
                let kind = if supplied { "supplied" } else { "shown" };
                let selector = format!("{kind}:likely-{rank}-{slot}");
                face(
                    theme,
                    &names,
                    format!("likely-{rank}-{slot}"),
                    student,
                    SMALL,
                )
                .debug_selector(move || selector.clone())
                .border_1()
                .border_color(if supplied { theme.accent } else { theme.border })
                .when(slot == 4, |face| face.ml(SP_2))
            });
            let label = format!("防衛の候補 {}: {}", rank + 1, named(&whole));
            let row =
                widgets::candidate(theme, &format!("likely-{rank}"), &label, picked.is_some())
                    .mt(SP_2)
                    .child(
                        div()
                            .w(RANK)
                            .flex_none()
                            .font_features(theme::tabular())
                            .child(format!("{}", rank + 1)),
                    )
                    .child(div().flex().flex_none().gap(SP_1).children(faces))
                    // Why: a part for each who used it, the row broken between parts only.
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .flex_1()
                            .min_w_0()
                            .gap_x(SP_3)
                            .text_color(theme.muted)
                            .children(
                                view::basis(likely)
                                    .into_iter()
                                    .map(|part| text(part).flex_none().whitespace_nowrap()),
                            ),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.choose(to.clone(), cx)));
            page = page.child(row);
            // Under it, the attacks that won against it, the best first.
            for (nth, tried) in likely.attacks.iter().enumerate() {
                let taken = picked.is_some_and(|choice| choice.attack == Some(tried.attack));
                let to = Some(choose((!taken).then_some(tried.attack)));
                let faces = tried.attack.into_iter().enumerate().map(|(slot, student)| {
                    let key = format!("attack-{rank}-{nth}-{slot}");
                    face(theme, &names, key, student, SMALL).when(slot == 4, |face| face.ml(SP_2))
                });
                let label = format!("攻撃の候補: {}", named(&tried.attack));
                let row = widgets::candidate(theme, &format!("attack-{rank}-{nth}"), &label, taken)
                    .ml(RANK + SP_2)
                    .child(div().flex().flex_none().gap(SP_1).children(faces))
                    .child(
                        div()
                            .flex_none()
                            .font_features(theme::tabular())
                            .child(text(view::fared(tried))),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.choose(to.clone(), cx)));
                page = page.child(row);
            }
            if likely.attacks.is_empty() {
                let none = "この防衛に勝った攻撃は、まだ登録がありません。";
                // Where an attack's icons would start.
                let indent = RANK + SP_2 * 3. + widgets::TICK;
                page = page.child(widgets::quiet(theme, none).ml(indent).py(SP_1));
            }
        }
        page.into_any_element()
    }

    /// Takes what the user chose against the opponent (`None`: nothing).
    fn choose(&mut self, choice: Option<Choice>, cx: &mut Context<Self>) {
        self.core.choose(choice);
        cx.notify();
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

    /// The settings, in the tabs' place: the update first (as in Aoi), the theme, where the
    /// records are kept.
    fn settings_page(&mut self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let update = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(SP_3)
            .mb(SP_4)
            .child(
                widgets::outlined_button(theme, "check", "アップデートを確認", false)
                    .on_click(cx.listener(|this, _, _, cx| this.check_update(cx))),
            )
            .child(widgets::quiet(theme, format!("今のバージョン v{VERSION}")));
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
            .child(group(theme, "アップデート", None))
            .child(update)
            .child(group(theme, "テーマ", None))
            .child(div().flex().flex_wrap().gap(SP_1).mb(SP_4).children(themes))
            .child(group(theme, "記録の保存先", None))
            .child(widgets::quiet(theme, records))
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
/// specials apart, each with its place under it. A student supplied from the records, not
/// shown by the game, has a frame of the accent; a place with nobody chosen shows "?".
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
                    theme.accent
                } else {
                    theme.border
                })
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
        // The word fits under an icon of the full size alone; the frame says it too.
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

/// A battle of the history: when it was (or, where no row of the game's list told, when it
/// was kept), then the attacker's line over the defender's, the winner's marked.
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
                .child(text(match &battle.fought {
                    Some(fought) => view::fought_at(fought),
                    None => format!("登録 {}", view::kept_at(&battle.seen_at)),
                })),
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
        let update = self.update_notice(&theme, cx);
        let content = if self.settings_open {
            self.settings_page(&theme, cx)
        } else {
            match self.tab {
                Tab::Match => self.match_tab(&theme, window, cx),
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
            .relative()
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
                    .children(update)
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
            .when_some(self.toast.clone(), |root, toast| {
                root.child(
                    div()
                        .absolute()
                        .left(SP_4)
                        .right(SP_4)
                        .bottom(SP_4)
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .id("toast")
                                .role(gpui::Role::Status)
                                .aria_label(toast.clone())
                                .px(SP_4)
                                .py(SP_2)
                                .border_1()
                                .border_color(theme.border)
                                .rounded(RADIUS_SM)
                                .bg(theme.surface)
                                .text_size(FS_SM)
                                .child(text(toast)),
                        ),
                )
            })
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
