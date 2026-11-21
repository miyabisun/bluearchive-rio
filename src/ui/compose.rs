//! The tab the user puts an attack and the defense it is told to beat together in, by hand
//! (編成追加): the two teams' places over a search for students. A student of the list goes
//! into the place chosen by a click, or into any place by a drag; two places change their
//! students by a drag. Only the list scrolls.

use gpui::{
    AnyElement, AppContext as _, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    Pixels, Role, Stateful, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};

use super::{
    Rio, Tab, icons, name_of,
    students::{Dragged, Ghost, TILE},
    theme::{self, FS_CAPTION, RADIUS_SM, SP_1, SP_2, SP_3, Theme},
    widgets::{self, text},
};
use crate::{
    Squad,
    draft::{TEAM, takes},
    search::{self, Filter},
    tier,
};

/// The sides of a place's icon, at most and at least: the two teams stay beside the button
/// in the narrowest window.
const PLACE: (f32, f32) = (56., 36.);
/// What a team's row takes beside its six places: its name, the gaps, the specials'
/// distance; and the room of the button at the rows' right.
const ROW_BESIDE: f32 = 32. + 8. + 20. + 8.;
pub(super) const BUTTON_ROOM: f32 = 12. + 96.;

impl Rio {
    /// The tab: the attack's places over the defense's with the button to register them at
    /// their right, the search under them, and the students found, who alone scroll.
    pub(super) fn compose_tab(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let wide = f32::from(window.viewport_size().width - SP_3 * 2.);
        let size = px(((wide - ROW_BESIDE - BUTTON_ROOM) / TEAM as f32).clamp(PLACE.1, PLACE.0));
        let team = |team: usize, name: &'static str| {
            let places = (0..TEAM).map(|at| {
                let place = self.place(theme, team * TEAM + at, size, cx);
                // The specials stand apart from the strikers.
                place.when(at == 4, |place| place.ml(SP_2))
            });
            div()
                .flex()
                .items_center()
                .gap(SP_2)
                .child(
                    div()
                        .w(px(32.))
                        .flex_none()
                        .text_size(FS_CAPTION)
                        .text_color(theme.muted)
                        .child(text(name)),
                )
                .child(div().flex().flex_none().gap(SP_1).children(places))
        };
        let teams = div()
            .flex()
            .flex_col()
            .flex_none()
            .gap(SP_1)
            .child(team(0, "攻め"))
            .child(team(1, "防衛"));
        // Whole, the two teams can be registered.
        let whole = self.draft.teams().is_some();
        let how = "攻めと防衛の 12 人を入れると登録できます。";
        let register = widgets::outlined_button(theme, "register", "編成登録", whole)
            .when(!whole, |button| {
                button
                    .aria_description(how)
                    .tooltip(widgets::tooltip(how))
                    .text_color(theme.muted)
                    .cursor_default()
            })
            .on_click(cx.listener(|this, _, _, cx| this.register(cx)));
        let top = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .gap(SP_3)
            .child(teams)
            .child(register);

        let finder = self.finder(Tab::Compose);
        let query = finder.query.read(cx).text().to_owned();
        // Nobody is offered who cannot go into the place chosen: its squad alone.
        let offered = Filter {
            squads: vec![takes(self.draft.chosen())],
            ..finder.filter.clone()
        };
        // Those the user sorted into a tier first, the highest tier before the others; by
        // name within a tier, and among those in none.
        let found = search::found(&self.roster, &query, &offered);
        let found = tier::by_tier(found, &self.shown.tiers);
        let tiles: Vec<_> = (found.into_iter())
            .map(|student| {
                let (id, squad) = (student.id, student.squad);
                // The list is of one squad: the frame tells the tier.
                let tier = tier::of(&self.shown.tiers, id);
                let frame = tier.map_or(theme.border, theme::tier_colour);
                Self::tile(student, "tile", TILE, frame, tier)
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| this.put(id, squad, cx)))
            })
            .collect();
        let none = tiles.is_empty();
        let students = div()
            .id("students")
            .debug_selector(|| "students".to_owned())
            .role(Role::List)
            .aria_label("生徒")
            .flex_1()
            .min_h_0()
            .mt(SP_2)
            .overflow_y_scroll()
            .track_scroll(&finder.scroll)
            .child(div().flex().flex_wrap().gap(SP_1).pb(SP_2).children(tiles))
            .when(none, |list| {
                list.child(widgets::quiet(theme, "当てはまる生徒がいません。"))
            });
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(top)
            .child(
                self.finding(theme, Tab::Compose, false, window, cx)
                    .mt(SP_3),
            )
            .child(students)
            .into_any_element()
    }

    /// A place of the teams: its student's icon, or its name while nobody stands there; the
    /// corners of the place chosen are marked. A click chooses it; its student is dragged to
    /// another place, and what is dropped on it comes to stand there.
    fn place(
        &self,
        theme: &Theme,
        place: usize,
        size: Pixels,
        cx: &Context<Self>,
    ) -> Stateful<gpui::Div> {
        let (student, chosen) = (self.draft.at(place), self.draft.chosen() == place);
        let (side, letter) = if place < TEAM {
            ("攻め", 'A')
        } else {
            ("防衛", 'D')
        };
        let name = match place % TEAM {
            at @ 0..4 => format!("{letter}{}", at + 1),
            _ => "SP".to_owned(),
        };
        let who = student.map_or_else(
            || "空き".to_owned(),
            |student| name_of(&self.names, student),
        );
        // An L at each corner, nothing between them.
        let corner = |top: bool, left: bool| {
            let corner = div().absolute().size(px(10.)).border_color(theme.cursor);
            let corner = if top {
                corner.top_0().border_t_2()
            } else {
                corner.bottom_0().border_b_2()
            };
            if left {
                corner.left_0().border_l_2()
            } else {
                corner.right_0().border_r_2()
            }
        };
        let inside = match student {
            Some(student) => icons::student(student, size).into_any_element(),
            None => div()
                .text_size(FS_CAPTION)
                .text_color(theme.muted)
                .child(name.clone())
                .into_any_element(),
        };
        let over = theme.accent;
        div()
            .id(("place", place))
            .debug_selector(move || format!("place:{place}"))
            .role(Role::Button)
            .aria_label(format!("{side} {name}: {who}"))
            .aria_toggled(if chosen {
                gpui::Toggled::True
            } else {
                gpui::Toggled::False
            })
            .tab_stop(true)
            .when(student.is_some(), |place| {
                place.tooltip(widgets::tooltip(who))
            })
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(size)
            .rounded(RADIUS_SM)
            .overflow_hidden()
            .border_1()
            .border_color(theme.border)
            .when(student.is_none(), gpui::Styled::border_dashed)
            .bg(theme.wash_base)
            .cursor_pointer()
            .child(inside)
            .when(chosen, |place| {
                place
                    .child(corner(true, true).debug_selector(|| "cursor".to_owned()))
                    .child(corner(true, false))
                    .child(corner(false, true))
                    .child(corner(false, false))
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                let before = this.draft.chosen();
                this.draft.choose(place);
                this.chose(before, cx);
            }))
            .when_some(student, |element, student| {
                element.on_drag(Dragged::Place(place), move |_, _, _, cx| {
                    cx.new(|_| Ghost { student, size })
                })
            })
            // Only a student of the place's squad can be dropped here.
            .can_drop(move |dragged, _, _| {
                let dragged = dragged.downcast_ref::<Dragged>();
                dragged.is_some_and(|dragged| dragged.squad() == takes(place))
            })
            .drag_over::<Dragged>(move |style, _, _, _| style.border_color(over))
            .on_drop(cx.listener(move |this, dragged: &Dragged, _, cx| {
                this.dropped(place, *dragged, cx);
            }))
    }

    /// Puts a student of the list into the place chosen; the next place is chosen then.
    /// The list offers the place's squad alone, so the place takes whoever is clicked.
    fn put(&mut self, student: i64, squad: Squad, cx: &mut Context<Self>) {
        let before = self.draft.chosen();
        let _ = self.draft.put(student, squad);
        self.chose(before, cx);
    }

    /// Takes what was dropped on a place: a student of the list comes to stand there, a
    /// place's student changes places with this one's. A place takes no drop of the other
    /// squad (`can_drop`), so nothing is refused here.
    fn dropped(&mut self, place: usize, dragged: Dragged, cx: &mut Context<Self>) {
        let _ = match dragged {
            Dragged::Student(student, squad) => self.draft.drop_on(place, student, squad),
            Dragged::Place(from) => self.draft.swap(from, place),
        };
        cx.notify();
    }

    /// After another place was chosen than `before`: where the squad to offer changed, the
    /// students are another list, shown from its top.
    fn chose(&mut self, before: usize, cx: &mut Context<Self>) {
        if takes(before) != takes(self.draft.chosen()) {
            let top = gpui::point(px(0.), px(0.));
            self.finder(Tab::Compose).scroll.set_offset(top);
        }
        cx.notify();
    }

    /// Registers the attack as told to win against the defense, once both are whole. The
    /// places keep their students: another attack against the same defense is at hand.
    fn register(&mut self, cx: &mut Context<Self>) {
        let Some((attack, defense)) = self.draft.teams() else {
            return;
        };
        match self.core.register(attack, defense) {
            Ok(new) => {
                self.not_kept = None;
                let said = if new {
                    "登録しました"
                } else {
                    "登録済みです"
                };
                self.notify_toast(Some(said), false, cx);
            }
            Err(error) => {
                self.not_kept = Some(error);
                cx.notify();
            }
        }
    }
}
