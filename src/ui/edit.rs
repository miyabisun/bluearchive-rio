//! The sub-tab the registered formations are looked over in (編集): each with its two teams,
//! the record it was introduced with and the page it comes from, and the buttons that change
//! it (in 追加's places) and take it away. One taken away can be put back right after.

use std::{collections::HashMap, time::Duration};

use gpui::{
    AnyElement, App, ClickEvent, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    Pixels, Role, SharedString, StatefulInteractiveElement as _, Styled as _, WeakEntity, Window,
    div, prelude::FluentBuilder as _, px,
};

use super::{
    Rio, SMALL, Sub, face, name_of,
    theme::{self, FS_CAPTION, FS_SM, SP_1, SP_2, SP_3, Theme},
    widgets::{self, text},
};
use crate::{source, store::Registered, view};

/// How tall a registration's row is: its two teams' icons, one over the other.
const ROW: Pixels = px(92.);
/// How long the toast that offers to put a registration back stays.
const UNDO_STAYS: Duration = Duration::from_secs(8);

impl Rio {
    /// Takes the registrations as the records have them now: an agent may have changed
    /// them (`mcp`).
    pub(super) fn reload(&mut self, cx: &mut Context<Self>) {
        match self.core.registered() {
            Ok(registered) => self.registered = registered.into(),
            Err(error) => self.not_kept = Some(error),
        }
        cx.notify();
    }

    /// Opens a sub-tab of the tab the formations are registered in; the list of the
    /// registrations is read anew as it opens.
    pub(super) fn open_sub(&mut self, sub: Sub, cx: &mut Context<Self>) {
        self.sub = sub;
        if sub == Sub::Edit {
            self.reload(cx);
        }
        cx.notify();
    }

    /// The sub-tab: the registrations, the one registered last first. Only the rows in
    /// sight are drawn.
    pub(super) fn edit_tab(&mut self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        if self.registered.is_empty() {
            let none = "「追加」で登録した編成が、ここに並びます。";
            return div()
                .flex_1()
                .min_h_0()
                .child(widgets::empty(theme, "edit", none))
                .into_any_element();
        }
        let (rows, names, rio) = (
            self.registered.clone(),
            self.names.clone(),
            cx.weak_entity(),
        );
        gpui::uniform_list("registered", rows.len(), move |range, _window, cx| {
            let theme = Theme::get(cx);
            range
                .map(|at| registered_row(&theme, &names, &rows[at], &rio))
                .collect()
        })
        .flex_1()
        .min_h_0()
        .into_any_element()
    }

    /// Starts changing the registration of a number, in 追加's places.
    fn edit_one(&mut self, id: i64, cx: &mut Context<Self>) {
        let one = self.registered.iter().find(|one| one.id == id).cloned();
        if let Some(one) = one {
            self.edit(&one, cx);
        }
    }

    /// Takes the registration of a number away, and offers to put it back.
    fn delete(&mut self, id: i64, cx: &mut Context<Self>) {
        let gone = self.core.forget(id);
        self.taken_away(gone, cx);
    }

    /// After a registration was asked to be taken away: gone, a toast offers to put it back
    /// for a while (a slip of the hand is undone); the list follows the records.
    pub(super) fn taken_away(
        &mut self,
        gone: Result<Option<Registered>, String>,
        cx: &mut Context<Self>,
    ) {
        match gone {
            Ok(Some(gone)) => {
                self.not_kept = None;
                // The one being changed is no more: there is nothing left to change.
                if (self.editing.as_ref()).is_some_and(|editing| editing.id == gone.id) {
                    self.leave_editing(cx);
                }
                self.notify_toast(Some("削除しました"), Some(UNDO_STAYS), cx);
                self.undo = Some(gone);
            }
            // An agent took it away already (`mcp`).
            Ok(None) => {
                let gone = "この登録は、もうありません";
                self.notify_toast(Some(gone), Some(super::TOAST_STAYS), cx);
            }
            Err(error) => self.not_kept = Some(error),
        }
        self.reload(cx);
    }

    /// Puts the registration taken away last back as it was.
    pub(super) fn put_back(&mut self, cx: &mut Context<Self>) {
        let Some(gone) = self.undo.take() else {
            return;
        };
        match self.core.restore(&gone) {
            Ok(back) => {
                self.not_kept = None;
                let said = if back {
                    "元に戻しました"
                } else {
                    "同じ攻めと防衛の登録が、もうあります"
                };
                self.notify_toast(Some(said), Some(super::TOAST_STAYS), cx);
            }
            Err(error) => self.not_kept = Some(error),
        }
        self.reload(cx);
    }
}

/// A registration of the list: the attack over the defense, beside them the record it was
/// introduced with over the page (a press opens it in the browser), and the buttons.
fn registered_row(
    theme: &Theme,
    names: &HashMap<i64, String>,
    registered: &Registered,
    rio: &WeakEntity<Rio>,
) -> impl IntoElement + use<> {
    let id = registered.id;
    let line = |role: &'static str, key: &'static str, team: [i64; 6]| {
        let faces = team.into_iter().enumerate().map(|(slot, student)| {
            let key = format!("registered-{id}-{key}-{slot}");
            face(theme, names, key, student, SMALL).when(slot == 4, |face| face.ml(SP_2))
        });
        div()
            .debug_selector(move || format!("registered:{id}:{key}"))
            .flex()
            .flex_none()
            .items_center()
            .gap(SP_2)
            .child(
                div()
                    .w(px(32.))
                    .flex_none()
                    .text_size(FS_CAPTION)
                    .text_color(theme.muted)
                    .child(role),
            )
            .child(div().flex().flex_none().gap(SP_1).children(faces))
    };
    let teams = div()
        .flex()
        .flex_col()
        .flex_none()
        .gap(SP_1)
        .child(line("攻め", "attack", registered.attack))
        .child(line("防衛", "defense", registered.defense));
    let source = &registered.source;
    let record = source.reported.map(|told| {
        text(view::reported(told))
            .font_features(theme::tabular())
            .whitespace_nowrap()
    });
    let page = source.url.clone().map(|url| {
        // Only an address of the web is opened: anything else (written into the records by
        // other hands) is shown and no more.
        let opens = source::of_web(&url);
        let shown = url.split_once("://").map_or(url.as_str(), |(_, rest)| rest);
        div()
            .id(SharedString::from(format!("page-{id}")))
            .debug_selector(move || format!("page:{id}"))
            .role(Role::Link)
            .aria_label(url.clone())
            .tab_stop(opens)
            // As wide as its words (cut where the row ends): no press beside them opens it.
            .max_w_full()
            .truncate()
            .text_color(theme.muted)
            .child(shown.to_owned())
            .when(opens, |link| {
                let told = format!("{url}\n押すとブラウザで開きます。");
                link.underline()
                    .cursor_pointer()
                    .tooltip(widgets::tooltip(told))
                    .on_click(move |_, _, cx| cx.open_url(&url))
            })
    });
    let told = div()
        .flex()
        .flex_col()
        .items_start()
        .flex_1()
        .min_w_0()
        .gap(SP_1)
        .text_size(FS_SM)
        .children(record)
        .children(page);
    let press = |what: fn(&mut Rio, i64, &mut Context<Rio>)| {
        let rio = rio.clone();
        move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
            let _ = rio.update(cx, |this, cx| what(this, id, cx));
        }
    };
    let buttons = div()
        .flex()
        .flex_col()
        .flex_none()
        .gap(SP_2)
        .child(
            widgets::outlined_button(theme, &format!("edit-{id}"), "編集", false)
                .on_click(press(Rio::edit_one)),
        )
        .child(
            widgets::outlined_button(theme, &format!("delete-{id}"), "削除", false)
                .on_click(press(Rio::delete)),
        );
    let named = |team: &[i64; 6]| -> String {
        let names: Vec<String> = team.iter().map(|id| name_of(names, *id)).collect();
        names.join("、")
    };
    let label = format!(
        "登録した編成: 攻め {} / 防衛 {}",
        named(&registered.attack),
        named(&registered.defense)
    );
    div()
        .id(SharedString::from(format!("registered-{id}")))
        .debug_selector(move || format!("registered:{id}"))
        .role(Role::ListItem)
        .aria_label(label)
        .h(ROW)
        .w_full()
        .flex()
        .items_center()
        .gap(SP_3)
        .border_b_1()
        .border_color(theme.border)
        .child(teams)
        .child(told)
        .child(buttons)
}
