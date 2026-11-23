//! What the sub-tabs that work with the students share (追加, Tier表): the row that finds
//! them (a name typed, buttons of their attributes), their icons in a list, and what a drag
//! of one carries.

use gpui::{
    AppContext as _, Context, Div, Entity, Focusable as _, Hsla, InteractiveElement as _,
    IntoElement, MouseButton, MouseDownEvent, ParentElement as _, Pixels, Render, Role,
    ScrollHandle, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Window,
    div, linear_color_stop, linear_gradient, prelude::FluentBuilder as _, px, rgb,
};

use super::{
    Field, Rio, Sub, icons,
    text_input::{self, TextInput},
    theme::{self, FS_SM, RADIUS_SM, SP_2, Theme},
    widgets::{self, Mark},
};
use crate::{
    ARMOR_MARK, ATTACK_MARK, Armor, Attack, Role as Class, Squad, Student,
    draft::takes,
    search::{self, Filter},
    tier::{Row, Sorted},
    view,
};

/// The sides of a student's icon in a list.
pub(super) const TILE: Pixels = px(56.);
/// The least the search field is wide.
const FIELD: Pixels = px(160.);

/// What finds students in a sub-tab: the name typed, the buttons pressed, and where the
/// students found are scrolled to. Each has its own: what narrows one's list is not seen in
/// the other.
pub(super) struct Finder {
    pub(super) query: Entity<TextInput>,
    pub(super) filter: Filter,
    pub(super) scroll: ScrollHandle,
}

impl Finder {
    pub(super) fn new(cx: &mut Context<Rio>) -> Self {
        let query = cx.new(|cx| TextInput::new("名前で検索", "", cx));
        cx.subscribe(&query, |_, _, _: &text_input::Changed, cx| cx.notify())
            .detach();
        Self {
            query,
            filter: Filter::default(),
            scroll: ScrollHandle::new(),
        }
    }
}

/// What is dragged: a student's tile, or the student standing at a place of the teams put
/// together. A tile tells the row of the table it was dragged from (none: a list) and where
/// the student stands in the table: a row takes what it would change, the list of the table
/// takes her out of the row her tile came from (`tier::dropped`).
#[derive(Debug, Clone, Copy)]
pub(super) enum Dragged {
    Student {
        id: i64,
        squad: Squad,
        from: Option<Row>,
        sorted: Sorted,
    },
    Place(usize),
}

impl Dragged {
    /// The squad of the student dragged: a place of the other squad takes no drop of it.
    pub(super) fn squad(self) -> Squad {
        match self {
            Self::Student { squad, .. } => squad,
            Self::Place(place) => takes(place),
        }
    }
}

/// What follows the pointer while a student is dragged: the icon.
pub(super) struct Ghost {
    pub(super) student: i64,
    pub(super) size: Pixels,
}

impl Render for Ghost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        icons::student(self.student, self.size).opacity(0.85)
    }
}

impl Rio {
    /// What finds the students in a sub-tab: the table's own, the other for the teams put
    /// together.
    pub(super) fn finder(&self, sub: Sub) -> &Finder {
        match sub {
            Sub::Tiers => &self.ranking,
            _ => &self.picking,
        }
    }

    fn finder_mut(&mut self, sub: Sub) -> &mut Finder {
        match sub {
            Sub::Tiers => &mut self.ranking,
            _ => &mut self.picking,
        }
    }

    /// The row that finds students in `sub`: the field for a name, then the buttons that
    /// narrow them down, a kind beside the other: the squads (where `squads`: a list of one
    /// squad already has none), the classes, the attack and the armor types.
    pub(super) fn finding(
        &self,
        theme: &Theme,
        sub: Sub,
        squads: bool,
        window: &Window,
        cx: &Context<Self>,
    ) -> Div {
        let finder = self.finder(sub);
        let focused = finder.query.focus_handle(cx).is_focused(window);
        let asked = Field::Search(sub);
        let field = div()
            .id("search")
            .debug_selector(|| "search".to_owned())
            .role(Role::SearchInput)
            .aria_label(asked.name())
            // A press of the right button opens the field's menu, to paste.
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    this.asks_field(asked, event.position, window, cx);
                }),
            )
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
            .child(finder.query.clone());
        let filter = &finder.filter;
        let squad = [
            (Squad::Main, "ストライカー", "ST", theme::STRIKER),
            (Squad::Support, "スペシャル", "SP", theme::SPECIAL),
        ];
        let squad = squad.into_iter().map(|(one, name, letters, colour)| {
            let (pressed, colour) = (filter.squads.contains(&one), rgb(colour).into());
            widgets::mark_button(theme, name, Mark::Letters(letters), colour, pressed).on_click(
                cx.listener(move |this, _, _, cx| {
                    search::toggle(&mut this.finder_mut(sub).filter.squads, one);
                    cx.notify();
                }),
            )
        });
        let class: Hsla = rgb(theme::CLASS).into();
        let classes = Class::ALL.into_iter().map(|one| {
            let pressed = filter.roles.contains(&one);
            widgets::mark_button(theme, one.name(), Mark::Icon(one.mark()), class, pressed)
                .on_click(cx.listener(move |this, _, _, cx| {
                    search::toggle(&mut this.finder_mut(sub).filter.roles, one);
                    cx.notify();
                }))
        });
        let attacks = Attack::ALL.into_iter().map(|one| {
            let (pressed, colour) = (filter.attacks.contains(&one), theme::attack_colour(one));
            widgets::mark_button(theme, one.name(), Mark::Icon(ATTACK_MARK), colour, pressed)
                .on_click(cx.listener(move |this, _, _, cx| {
                    search::toggle(&mut this.finder_mut(sub).filter.attacks, one);
                    cx.notify();
                }))
        });
        let armors = Armor::ALL.into_iter().map(|one| {
            let (pressed, colour) = (filter.armors.contains(&one), theme::armor_colour(one));
            widgets::mark_button(theme, one.name(), Mark::Icon(ARMOR_MARK), colour, pressed)
                .on_click(cx.listener(move |this, _, _, cx| {
                    search::toggle(&mut this.finder_mut(sub).filter.armors, one);
                    cx.notify();
                }))
        });
        let kind = || div().flex().flex_none().gap(px(2.));
        div()
            .flex()
            .flex_wrap()
            .flex_none()
            .items_center()
            .gap(SP_2)
            .child(field)
            .when(squads, |row| row.child(kind().children(squad)))
            .child(kind().children(classes))
            .child(kind().children(attacks))
            .child(kind().children(armors))
    }

    /// A student's icon, `size` square, to pick or to drag: in a frame of `frame`, over the
    /// attack type's colour at the upper left and the armor type's at the lower right. The
    /// name, the attributes and where the student stands in the table as `sorted` tells it
    /// (her tier, 編成NG) are on a hover. `from` is the row of the table the tile stands in
    /// (none: a list): a student of a tier and barred has a tile in each, so the tests find
    /// a list's as `tile:<id>`, a tier's as `tiered:<id>` and one among those barred as
    /// `barred:<id>`.
    pub(super) fn tile(
        student: &Student,
        from: Option<Row>,
        size: Pixels,
        frame: Hsla,
        sorted: Sorted,
    ) -> Stateful<Div> {
        let (id, squad) = (student.id, student.squad);
        let kind = match from {
            None => "tile",
            Some(Row::Tier(_)) => "tiered",
            Some(Row::Barred) => "barred",
        };
        // The two colours meet along the line from the upper right to the lower left, run
        // into each other over the middle tenth.
        let ground = linear_gradient(
            135.,
            linear_color_stop(theme::attack_colour(student.attack), 0.45),
            linear_color_stop(theme::armor_colour(student.armor), 0.55),
        );
        let mut told = format!(
            "{}\n{}・{}・{}",
            student.name,
            student.role.name(),
            student.attack.name(),
            student.armor.name()
        );
        for line in view::stands(sorted) {
            told.push('\n');
            told.push_str(&line);
        }
        div()
            .id(SharedString::from(format!("{kind}-{id}")))
            .debug_selector(move || format!("{kind}:{id}"))
            .role(Role::Button)
            .aria_label(student.name.clone())
            .tooltip(widgets::tooltip(told))
            .flex_none()
            .size(size)
            .rounded(RADIUS_SM)
            .overflow_hidden()
            .border_2()
            .border_color(frame)
            .bg(ground)
            .child(icons::student(id, size - px(4.)))
            .on_drag(
                Dragged::Student {
                    id,
                    squad,
                    from,
                    sorted,
                },
                move |_, _, _, cx| cx.new(|_| Ghost { student: id, size }),
            )
    }

    /// The colour of a squad's frame: red for the strikers, blue for the specials.
    pub(super) fn squad_colour(squad: Squad) -> Hsla {
        let colour = match squad {
            Squad::Main => theme::STRIKER,
            Squad::Support => theme::SPECIAL,
        };
        rgb(colour).into()
    }
}
