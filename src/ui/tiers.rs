//! The tab the user sorts the students they use often into tiers in (編成Tier), to pick them
//! quickly where a formation is put together: the five tiers' rows over a search for the
//! students in no tier. Everything is done by a drag: into a tier's row, from a row to
//! another, and back onto the list to take a student out. A click does nothing.

use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    Pixels, Role, Stateful, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};

use super::{
    Rio, Tab,
    students::{Dragged, TILE},
    theme::{self, FS_CAPTION, FS_SM, RADIUS_SM, SP_1, SP_2, SP_3, Theme},
    widgets::{self, text},
};
use crate::{
    Student,
    search::{self, Filter},
    tier::{self, Tier},
};

/// The sides of a student's icon in a tier's row: smaller than in the list, a tier holds
/// many.
const TIERED: Pixels = px(44.);
/// How tall a tier's row is at the least: a student's icon and the row's own edges, whether
/// anybody is in it or not, so that nothing moves as the first student comes into it. And
/// how wide its letter's plate.
const ROW: Pixels = px(44. + 5.);
const PLATE: Pixels = px(28.);
/// How much of the window's height the tiers take at most before they scroll by themselves:
/// the students to sort keep their room.
const TIERS_MOST: f32 = 0.45;

/// Whether what is dragged is a student (and no place of the teams put together).
fn a_student(dragged: &dyn std::any::Any) -> bool {
    matches!(
        dragged.downcast_ref::<Dragged>(),
        Some(Dragged::Student(..))
    )
}

impl Rio {
    /// The tab: the tiers' rows, the row that finds students, and the students in no tier,
    /// who scroll.
    pub(super) fn tiers_tab(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // Every student by name, to stand in a tier's row in that order.
        let all = search::found(&self.roster, "", &Filter::default());
        let nobody = self.shown.tiers.is_empty();
        let rows = Tier::ALL.into_iter().map(|one| {
            let of_tier =
                |student: &&&Student| tier::of(&self.shown.tiers, student.id) == Some(one);
            let students: Vec<&Student> = all.iter().filter(of_tier).copied().collect();
            // With nobody in any tier yet, the first row says how one comes into it.
            let hint = (nobody && one == Tier::S).then_some("一覧の生徒をドラッグして入れます");
            self.tier_row(theme, one, &students, hint, cx)
        });
        let most = px(f32::from(window.viewport_size().height) * TIERS_MOST);
        let table = div()
            .id("tiers")
            .debug_selector(|| "tiers".to_owned())
            .flex()
            .flex_col()
            .flex_none()
            .max_h(most)
            .overflow_y_scroll()
            .children(rows);

        let finder = self.finder(Tab::Tiers);
        let query = finder.query.read(cx).text().to_owned();
        let found = search::found(&self.roster, &query, &finder.filter);
        // Whoever is in a tier stands in its row, not in the list.
        let tiles: Vec<_> = (found.into_iter())
            .filter(|student| tier::of(&self.shown.tiers, student.id).is_none())
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
            .aria_label("Tier に入れていない生徒")
            .flex_1()
            .min_h_0()
            .mt(SP_2)
            .overflow_y_scroll()
            .track_scroll(&finder.scroll)
            .child(div().flex().flex_wrap().gap(SP_1).pb(SP_2).children(tiles))
            .when(none, |list| {
                list.child(widgets::quiet(theme, "当てはまる生徒がいません。"))
            })
            // A student dragged back onto the list leaves the tiers.
            .can_drop(|dragged, _, _| a_student(dragged))
            .drag_over::<Dragged>(move |style, _, _, _| style.bg(over))
            .on_drop(cx.listener(|this, dragged: &Dragged, _, cx| {
                if let Dragged::Student(student, _) = *dragged {
                    this.rank(student, None, cx);
                }
            }));
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(table)
            .child(self.finding(theme, Tab::Tiers, true, window, cx).mt(SP_3))
            .child(students)
            .into_any_element()
    }

    /// A tier's row: its letter on the tier's colour, then its students by name. What is
    /// dropped on it comes into the tier.
    fn tier_row(
        &self,
        theme: &Theme,
        tier: Tier,
        students: &[&Student],
        hint: Option<&'static str>,
        cx: &Context<Self>,
    ) -> Stateful<gpui::Div> {
        let name = tier.name();
        let tiles = students.iter().map(|student| {
            let frame = Self::squad_colour(student.squad);
            Self::tile(student, "tiered", TIERED, frame, Some(tier)).cursor_grab()
        });
        let over = theme.wash_base;
        div()
            .id(("tier", tier.rank() as usize))
            .debug_selector(move || format!("tier:{name}"))
            .role(Role::List)
            .aria_label(format!("Tier {name}"))
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
                    .bg(theme::tier_colour(tier))
                    .text_size(FS_SM)
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme::ON_TIER)
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
                    this.rank(student, Some(tier), cx);
                }
            }))
    }

    /// Puts a student into a tier, or takes the student out of the tiers (see `Core::rank`);
    /// a failure stays in the page until the next thing asked is kept.
    fn rank(&mut self, student: i64, tier: Option<Tier>, cx: &mut Context<Self>) {
        self.not_kept = self.core.rank(student, tier).err();
        cx.notify();
    }
}
