//! The parts the tabs share, as Aoi's: text the tests can find, hover tooltips, the settings
//! page's choices and buttons, notices, a failure's banner and what a tab says while it is
//! empty.

use gpui::{
    AnyView, App, AppContext as _, Div, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, Role, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Window,
    div, prelude::FluentBuilder as _, px,
};

use super::{
    icons,
    theme::{FS_SM, RADIUS_SM, SP_1, SP_2, SP_3, Theme},
};

/// A piece of text that the window's tests can find (`text:<the text>`).
pub fn text(text: impl Into<SharedString>) -> Div {
    let text = text.into();
    let selector = text.clone();
    div()
        .debug_selector(move || format!("text:{selector}"))
        .child(text)
}

/// What a hover shows: lines of text in the surface's face.
pub struct Tooltip {
    text: SharedString,
}

impl Render for Tooltip {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        div()
            .font_family(super::theme::FONT)
            .max_w(px(360.))
            .px(SP_2)
            .py(SP_1)
            .border_1()
            .border_color(theme.border)
            .rounded(RADIUS_SM)
            .bg(theme.surface)
            .text_color(theme.on_surface)
            .text_size(FS_SM)
            .children(self.text.lines().map(|line| div().child(line.to_owned())))
    }
}

/// A tooltip of `text` (one line each), for `.tooltip(...)`.
pub fn tooltip(
    text: impl Into<SharedString>,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = text.into();
    move |_window, cx| cx.new(|_| Tooltip { text: text.clone() }).into()
}

/// One of a group's choices on the settings page: a tick before the chosen one, its room
/// kept before the others.
pub fn choice(theme: &Theme, id: &str, label: &str, checked: bool) -> Stateful<Div> {
    let hover = theme.wash_base;
    let selector = format!("choice-{id}");
    div()
        .id(SharedString::from(selector.clone()))
        .debug_selector(move || selector.clone())
        .role(Role::RadioButton)
        .aria_label(label.to_owned())
        .aria_toggled(if checked {
            gpui::Toggled::True
        } else {
            gpui::Toggled::False
        })
        .tab_stop(true)
        .flex()
        .px(SP_3)
        .py(SP_2)
        .rounded(RADIUS_SM)
        .text_size(FS_SM)
        .text_color(theme.on_surface)
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .focus(move |s| s.bg(hover))
        .child(
            div()
                .w(px(14. * 1.4))
                .flex_none()
                .when(!checked, gpui::Styled::invisible)
                .child("✓"),
        )
        .child(label.to_owned())
}

/// The room of a tick before a choice or a candidate.
pub const TICK: gpui::Pixels = px(14. * 1.4);

/// A candidate the user may choose, as a row: a tick before the chosen one, its room kept
/// before the others (as `choice`), then what the caller puts into the row. A button that
/// stays pressed, not a radio button: pressed again, the candidate is given up. The tests
/// find it as `chosen:<id>` or `candidate:<id>`.
pub fn candidate(theme: &Theme, id: &str, label: &str, chosen: bool) -> Stateful<Div> {
    let hover = theme.wash_base;
    let kind = if chosen { "chosen" } else { "candidate" };
    row(id, kind)
        .role(Role::Button)
        .aria_label(label.to_owned())
        .aria_toggled(if chosen {
            gpui::Toggled::True
        } else {
            gpui::Toggled::False
        })
        .tab_stop(true)
        .text_color(theme.on_surface)
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .focus(move |s| s.bg(hover))
        .child(
            div()
                .w(TICK)
                .flex_none()
                .when(!chosen, gpui::Styled::invisible)
                .child("✓"),
        )
}

/// A candidate set aside, which the user cannot choose: `candidate`'s row in muted words,
/// the tick's room kept so its icons stand under the others', without the hand, the hover
/// or the press. `why` tells what sets it aside. The tests find it as `excluded:<id>`.
pub fn set_aside(theme: &Theme, id: &str, label: &str, why: &str) -> Stateful<Div> {
    row(id, "excluded")
        .role(Role::Group)
        .aria_label(label.to_owned())
        .aria_description(why.to_owned())
        .text_color(theme.muted)
        .child(div().w(TICK).flex_none())
}

/// The row of a candidate, found by the tests as `<kind>:<id>`.
fn row(id: &str, kind: &str) -> Stateful<Div> {
    let selector = format!("{kind}:{id}");
    div()
        .id(SharedString::from(format!("candidate-{id}")))
        .debug_selector(move || selector.clone())
        .flex()
        .items_center()
        .gap(SP_2)
        .px(SP_2)
        .py(SP_1)
        .rounded(RADIUS_SM)
        .text_size(FS_SM)
}

/// The sides of a `mark_button`'s face, and how far the face stands over its edge.
pub const MARK: gpui::Pixels = px(28.);
const RAISED: gpui::Pixels = px(3.);

/// A button that stays pressed, told by a white mark on its colour (a class, a type): up, its
/// face stands over an edge of a darker shade; pressed, the face is sunk onto the edge's
/// place, darker, in a ring of the text colour. The name is on a hover. The tests find it as
/// `filter:<label>`, pressed as `pressed:<label>`.
pub fn mark_button(
    theme: &Theme,
    label: &str,
    mark: &str,
    colour: gpui::Hsla,
    pressed: bool,
) -> Stateful<Div> {
    let shade = |by: f32| gpui::Hsla {
        l: colour.l * by,
        ..colour
    };
    let selector = format!("{}:{label}", if pressed { "pressed" } else { "filter" });
    let face = div()
        .flex()
        .items_center()
        .justify_center()
        .size(MARK)
        .rounded(RADIUS_SM)
        .child(icons::mark(mark, px(18.)));
    let face = if pressed {
        face.mt(RAISED)
            .bg(shade(0.6))
            .border_2()
            .border_color(theme.on_surface)
    } else {
        face.bg(colour)
    };
    div()
        .id(SharedString::from(format!("filter-{label}")))
        .debug_selector(move || selector.clone())
        .role(Role::Button)
        .aria_label(label.to_owned())
        .aria_toggled(if pressed {
            gpui::Toggled::True
        } else {
            gpui::Toggled::False
        })
        .tab_stop(true)
        .tooltip(tooltip(label.to_owned()))
        .flex_none()
        .w(MARK)
        .h(MARK + RAISED)
        .rounded(RADIUS_SM)
        .cursor_pointer()
        .when(!pressed, |button| button.bg(shade(0.5)))
        .child(face)
}

/// A failure in the page: danger on danger-subtle.
pub fn error(theme: &Theme, id: &str, message: impl Into<SharedString>) -> Stateful<Div> {
    let message = message.into();
    let selector = id.to_owned();
    div()
        .id(SharedString::from(id.to_owned()))
        .debug_selector(move || selector.clone())
        .role(Role::Alert)
        .aria_label(message.clone())
        .flex_none()
        .mb(px(16.))
        .p(SP_3)
        .rounded(RADIUS_SM)
        .bg(theme.danger_subtle)
        .text_color(theme.danger)
        .text_size(FS_SM)
        .child(text(message))
}

/// A quiet line in the page: body-sm, muted.
pub fn quiet(theme: &Theme, message: impl Into<SharedString>) -> Div {
    div()
        .text_size(FS_SM)
        .text_color(theme.muted)
        .child(text(message))
}

/// A quiet notice over the page, found by its id.
pub fn notice(theme: &Theme, id: &str, message: impl Into<SharedString>) -> Stateful<Div> {
    let message = message.into();
    let selector = id.to_owned();
    div()
        .id(SharedString::from(id.to_owned()))
        .debug_selector(move || selector.clone())
        .role(Role::Status)
        .aria_label(message.clone())
        .flex_none()
        .mb(px(16.))
        .text_size(FS_SM)
        .text_color(theme.muted)
        .child(text(message))
}

/// A button drawn by its outline: the accent's for the page's main action.
pub fn outlined_button(theme: &Theme, id: &str, label: &str, primary: bool) -> Stateful<Div> {
    let (border, color) = if primary {
        (theme.accent, theme.accent)
    } else {
        (theme.border, theme.on_surface)
    };
    let focused = if primary { theme.accent } else { theme.muted };
    let selector = id.to_owned();
    div()
        .id(SharedString::from(id.to_owned()))
        .debug_selector(move || selector.clone())
        .role(Role::Button)
        .aria_label(label.to_owned())
        .tab_stop(true)
        .flex_none()
        .px(SP_3)
        .py(px(2.))
        .border_1()
        .border_color(border)
        .rounded(RADIUS_SM)
        .text_size(FS_SM)
        .text_color(color)
        .cursor_pointer()
        .focus(move |s| s.border_color(focused))
        .child(label.to_owned())
}

/// What a tab says before there is anything to show in it.
pub fn empty(theme: &Theme, id: &str, message: &str) -> Stateful<Div> {
    div()
        .id(SharedString::from(format!("empty-{id}")))
        .role(Role::Label)
        .aria_label(message.to_owned())
        .text_color(theme.muted)
        .child(text(message.to_owned()))
}
