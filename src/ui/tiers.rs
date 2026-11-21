//! The sub-tab the user sorts the students in (Tier表): those they use often into tiers, to
//! pick them quickly where a formation is put together, and those they cannot send into the
//! row of the barred (編成NG). The table's rows stand over a search for the students in no
//! row. Everything is done by a drag: into a row, from a row to another, and back onto the
//! list to take a student out. A click does nothing.

use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    Pixels, Role, Stateful, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};

use super::{
    Rio, Sub,
    students::{Dragged, TILE},
    theme::{self, FS_CAPTION, FS_SM, RADIUS_SM, SP_1, SP_2, SP_3, Theme},
    widgets::{self, text},
};
use crate::{
    Student,
    search::{self, Filter},
    tier::{self, Row},
};

/// The sides of a student's icon in a row of the table: smaller than in the list, a row
/// holds many.
const TIERED: Pixels = px(44.);
/// How tall a row is at the least: a student's icon and the row's own edges, whether
/// anybody is in it or not, so that nothing moves as the first student comes into it. And
/// how wide its plate.
const ROW: Pixels = px(44. + 5.);
const PLATE: Pixels = px(28.);
/// How much of the window's height the table takes at most before it scrolls by itself:
/// the students to sort keep their room.
const TABLE_MOST: f32 = 0.45;

/// Whether what is dragged is a student (and no place of the teams put together).
fn a_student(dragged: &dyn std::any::Any) -> bool {
    matches!(
        dragged.downcast_ref::<Dragged>(),
        Some(Dragged::Student(..))
    )
}

impl Rio {
    /// The row a student stands in: among those barred, or in a tier.
    pub(super) fn row_of(&self, student: i64) -> Option<Row> {
        if self.shown.barred.contains(&student) {
            Some(Row::Barred)
        } else {
            tier::of(&self.shown.tiers, student).map(Row::Tier)
        }
    }

    /// The sub-tab: the table's rows (`Row::ALL`), the row that finds students, and the
    /// students in no row, who scroll.
    pub(super) fn tiers_tab(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // Every student by name, to stand in a row in that order.
        let all = search::found(&self.roster, "", &Filter::default());
        let nobody = self.shown.tiers.is_empty();
        let rows = Row::ALL.into_iter().enumerate().map(|(at, one)| {
            let students: Vec<&Student> = (all.iter().copied())
                .filter(|student| self.row_of(student.id) == Some(one))
                .collect();
            // With nobody in any tier yet, the first row says how one comes into it.
            let hint = (nobody && at == 0).then_some("一覧の生徒をドラッグして入れます");
            self.table_row(theme, (at, one), &students, hint, cx)
        });
        let most = px(f32::from(window.viewport_size().height) * TABLE_MOST);
        let table = div()
            .id("tiers")
            .debug_selector(|| "tiers".to_owned())
            .flex()
            .flex_col()
            .flex_none()
            .max_h(most)
            .overflow_y_scroll()
            .children(rows);

        let finder = self.finder(Sub::Tiers);
        let query = finder.query.read(cx).text().to_owned();
        let found = search::found(&self.roster, &query, &finder.filter);
        // Whoever stands in a row is there, not in the list.
        let tiles: Vec<_> = (found.into_iter())
            .filter(|student| self.row_of(student.id).is_none())
            .map(|student| {
                let frame = Self::squad_colour(student.squad);
                Self::tile(student, "tile", TILE, frame, None).cursor_grab()
            })
            .collect();
        let none = tiles.is_empty();
        let over = theme.wash_base;
        let students = div()
            .id("students")
            .debug_selector(|| "students".to_owned())
            .role(Role::List)
            .aria_label("Tier にも NG にも入れていない生徒")
            .flex_1()
            .min_h_0()
            .mt(SP_2)
            .overflow_y_scroll()
            .track_scroll(&finder.scroll)
            .child(div().flex().flex_wrap().gap(SP_1).pb(SP_2).children(tiles))
            .when(none, |list| {
                list.child(widgets::quiet(theme, "当てはまる生徒がいません。"))
            })
            // A student dragged back onto the list leaves the table.
            .can_drop(|dragged, _, _| a_student(dragged))
            .drag_over::<Dragged>(move |style, _, _, _| style.bg(over))
            .on_drop(cx.listener(|this, dragged: &Dragged, _, cx| {
                if let Dragged::Student(student, _) = *dragged {
                    this.sort(student, None, cx);
                }
            }));
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(table)
            .child(self.finding(theme, Sub::Tiers, true, window, cx).mt(SP_3))
            .child(students)
            .into_any_element()
    }

    /// A row of the table, the `at`th from its top: its plate (a tier's letter on the
    /// tier's colour, NG on the colour of danger), then its students by name. What is
    /// dropped on it comes into the row.
    fn table_row(
        &self,
        theme: &Theme,
        (at, row): (usize, Row),
        students: &[&Student],
        hint: Option<&'static str>,
        cx: &Context<Self>,
    ) -> Stateful<gpui::Div> {
        let name = row.name();
        let (plate, letters, label): (gpui::Hsla, gpui::Hsla, String) = match row {
            Row::Tier(tier) => (
                theme::tier_colour(tier),
                theme::ON_TIER.into(),
                format!("Tier {name}"),
            ),
            Row::Barred => (theme.danger, theme.surface, "編成NG".to_owned()),
        };
        let tiles = students.iter().map(|student| {
            let frame = Self::squad_colour(student.squad);
            Self::tile(student, "tiered", TIERED, frame, Some(row)).cursor_grab()
        });
        let over = theme.wash_base;
        div()
            .id(("tier", at))
            .debug_selector(move || format!("tier:{name}"))
            .role(Role::List)
            .aria_label(label)
            .flex()
            .flex_none()
            .gap(SP_2)
            .py(px(2.))
            .min_h(ROW)
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .w(PLATE)
                    .rounded(RADIUS_SM)
                    .bg(plate)
                    .text_size(if name.len() > 1 { FS_CAPTION } else { FS_SM })
                    .font_weight(FontWeight::BOLD)
                    .text_color(letters)
                    .child(text(name)),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap(SP_1)
                    .children(tiles)
                    .when_some(hint, |row, hint| {
                        row.child(
                            div()
                                .text_size(FS_CAPTION)
                                .text_color(theme.muted)
                                .child(text(hint)),
                        )
                    }),
            )
            .can_drop(|dragged, _, _| a_student(dragged))
            .drag_over::<Dragged>(move |style, _, _, _| style.bg(over))
            .on_drop(cx.listener(move |this, dragged: &Dragged, _, cx| {
                if let Dragged::Student(student, _) = *dragged {
                    this.sort(student, Some(row), cx);
                }
            }))
    }

    /// Puts a student into a row of the table, or takes the student out of it (see
    /// `Core::sort`); a failure stays in the page until the next thing asked is kept.
    fn sort(&mut self, student: i64, row: Option<Row>, cx: &mut Context<Self>) {
        self.not_kept = self.core.sort(student, row).err();
        cx.notify();
    }
}
