//! The tab the user puts an attack and the defense it is told to beat together in, by hand
//! (編成追加): the two teams' places over a search for students. A student of the list goes
//! into the place chosen by a click, or into any place by a drag; two places change their
//! students by a drag. Only the list scrolls.

use gpui::{
    AnyElement, AppContext as _, Context, Focusable as _, Hsla, InteractiveElement as _,
    IntoElement, ParentElement as _, Pixels, Render, Role, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, Window, div, linear_color_stop, linear_gradient,
    prelude::FluentBuilder as _, px, rgb,
};

use super::{
    Rio, icons, name_of,
    theme::{self, FS_CAPTION, FS_SM, RADIUS_SM, SP_1, SP_2, SP_3, Theme},
    widgets::{self, text},
};
use crate::{
    ARMOR_MARK, ATTACK_MARK, Armor, Attack, Role as Class, Squad, Student,
    draft::{Refused, TEAM},
    search, view,
};

/// The sides of a place's icon, at most and at least: the two teams stay beside the button
/// in the narrowest window. And of a student's icon in the list.
const PLACE: (f32, f32) = (56., 36.);
pub(super) const TILE: Pixels = px(56.);
/// What a team's row takes beside its six places: its name, the gaps, the specials'
/// distance; and the room of the button at the rows' right.
const ROW_BESIDE: f32 = 32. + 8. + 20. + 8.;
pub(super) const BUTTON_ROOM: f32 = 12. + 96.;
/// The least the search field is wide.
const FIELD: Pixels = px(160.);

/// What is dragged: a student of the list, or the student standing at a place.
#[derive(Debug, Clone, Copy)]
pub(super) enum Dragged {
    Student(i64),
    Place(usize),
}

/// What follows the pointer while a student is dragged: the icon.
struct Ghost {
    student: i64,
    size: Pixels,
}

impl Render for Ghost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        icons::student(self.student, self.size).opacity(0.85)
    }
}

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

        let focused = self.query.focus_handle(cx).is_focused(window);
        let field = div()
            .id("search")
            .debug_selector(|| "search".to_owned())
            .role(Role::SearchInput)
            .aria_label("名前で検索")
            // As wide as the row leaves it, from a width a name fits in: what the row breaks by.
            .flex_grow(1.)
            .flex_basis(FIELD)
            .min_w(FIELD)
            .h(px(32.))
            .px(SP_2)
            .border_1()
            .border_color(if focused { theme.accent } else { theme.border })
            .rounded(RADIUS_SM)
            .bg(theme.surface)
            .text_size(FS_SM)
            .child(self.query.clone());
        let class: Hsla = rgb(theme::CLASS).into();
        let classes = Class::ALL.into_iter().map(|one| {
            let pressed = self.filter.roles.contains(&one);
            widgets::mark_button(theme, one.name(), one.mark(), class, pressed).on_click(
                cx.listener(move |this, _, _, cx| {
                    search::toggle(&mut this.filter.roles, one);
                    cx.notify();
                }),
            )
        });
        let attacks = Attack::ALL.into_iter().map(|one| {
            let (pressed, colour) = (
                self.filter.attacks.contains(&one),
                theme::attack_colour(one),
            );
            widgets::mark_button(theme, one.name(), ATTACK_MARK, colour, pressed).on_click(
                cx.listener(move |this, _, _, cx| {
                    search::toggle(&mut this.filter.attacks, one);
                    cx.notify();
                }),
            )
        });
        let armors = Armor::ALL.into_iter().map(|one| {
            let (pressed, colour) = (self.filter.armors.contains(&one), theme::armor_colour(one));
            widgets::mark_button(theme, one.name(), ARMOR_MARK, colour, pressed).on_click(
                cx.listener(move |this, _, _, cx| {
                    search::toggle(&mut this.filter.armors, one);
                    cx.notify();
                }),
            )
        });
        let kind = || div().flex().flex_none().gap(px(2.));
        let find = div()
            .flex()
            .flex_wrap()
            .flex_none()
            .items_center()
            .gap(SP_2)
            .mt(SP_3)
            .child(field)
            .child(kind().children(classes))
            .child(kind().children(attacks))
            .child(kind().children(armors));

        let query = self.query.read(cx).text().to_owned();
        let found = search::found(&self.roster, &query, &self.filter);
        let tiles: Vec<_> = (found.into_iter())
            .map(|student| self.tile(student, cx))
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
            .track_scroll(&self.students)
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
            .child(find)
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
                this.draft.choose(place);
                cx.notify();
            }))
            .when_some(student, |element, student| {
                element.on_drag(Dragged::Place(place), move |_, _, _, cx| {
                    cx.new(|_| Ghost { student, size })
                })
            })
            .drag_over::<Dragged>(move |style, _, _, _| style.border_color(over))
            .on_drop(cx.listener(move |this, dragged: &Dragged, _, cx| {
                this.dropped(place, *dragged, cx);
            }))
    }

    /// A student of the list: the icon in a frame of the squad's colour, over the attack
    /// type's colour at the upper left and the armor type's at the lower right. A click puts
    /// the student into the place chosen; a drag, into the place it ends on.
    fn tile(&self, student: &Student, cx: &Context<Self>) -> Stateful<gpui::Div> {
        let id = student.id;
        let frame = rgb(match student.squad {
            Squad::Main => theme::STRIKER,
            Squad::Support => theme::SPECIAL,
        });
        // The two colours meet along the line from the upper right to the lower left, run
        // into each other over the middle tenth.
        let ground = linear_gradient(
            135.,
            linear_color_stop(theme::attack_colour(student.attack), 0.45),
            linear_color_stop(theme::armor_colour(student.armor), 0.55),
        );
        let told = format!(
            "{}\n{}・{}・{}",
            student.name,
            student.role.name(),
            student.attack.name(),
            student.armor.name()
        );
        div()
            .id(SharedString::from(format!("tile-{id}")))
            .debug_selector(move || format!("tile:{id}"))
            .role(Role::Button)
            .aria_label(student.name.clone())
            .tooltip(widgets::tooltip(told))
            .flex_none()
            .size(TILE)
            .rounded(RADIUS_SM)
            .overflow_hidden()
            .border_2()
            .border_color(frame)
            .bg(ground)
            .cursor_pointer()
            .child(icons::student(id, TILE - px(4.)))
            .on_click(cx.listener(move |this, _, _, cx| this.put(id, cx)))
            .on_drag(Dragged::Student(id), move |_, _, _, cx| {
                cx.new(|_| Ghost {
                    student: id,
                    size: TILE,
                })
            })
    }

    /// The squad a student fights in.
    fn squad_of(&self, student: i64) -> Squad {
        let known = self.roster.iter().find(|one| one.id == student);
        known.map_or(Squad::Main, |one| one.squad)
    }

    /// Puts a student of the list into the place chosen; the next place is chosen then.
    fn put(&mut self, student: i64, cx: &mut Context<Self>) {
        let done = self.draft.put(student, self.squad_of(student));
        self.placed(done, cx);
    }

    /// Takes what was dropped on a place: a student of the list comes to stand there, a
    /// place's student changes places with this one's.
    fn dropped(&mut self, place: usize, dragged: Dragged, cx: &mut Context<Self>) {
        let done = match dragged {
            Dragged::Student(student) => {
                (self.draft).drop_on(place, student, self.squad_of(student))
            }
            Dragged::Place(from) => self.draft.swap(from, place),
        };
        self.placed(done, cx);
    }

    /// Says why a student did not go into a place.
    fn placed(&mut self, done: Result<(), Refused>, cx: &mut Context<Self>) {
        if let Err(refused) = done {
            self.notify_toast(Some(view::refused(refused)), false, cx);
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
