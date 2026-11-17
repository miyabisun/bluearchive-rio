//! The parts the tabs share, as Aoi's: text the tests can find, hover tooltips, the settings
//! page's choices and buttons, notices, a failure's banner and what a tab says while it is
//! empty.

use gpui::{
    AnyView, App, AppContext as _, Div, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, Role, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Window,
    div, prelude::FluentBuilder as _, px,
};

use super::theme::{FS_SM, RADIUS_SM, SP_1, SP_2, SP_3, Theme};

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
