//! The sub-tab the user puts an attack and the defense it is told to beat together in, by
//! hand (追加): the two teams' places and what the formation was told with (the record it
//! was introduced with, the page) over a search for students. A student of the list goes
//! into the place chosen by a click, or into any place by a drag; two places change their
//! students by a drag. Only the list scrolls. A registration being changed (編集) is put
//! together here too, in place of a new one.

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, Focusable as _, InteractiveElement as _,
    IntoElement, ParentElement as _, Pixels, Role, Stateful, StatefulInteractiveElement as _,
    Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use super::{
    Rio, Sub, icons, name_of,
    students::{Dragged, Ghost, TILE},
    text_input::{self, TextInput},
    theme::{self, FS_CAPTION, FS_SM, RADIUS_SM, SP_1, SP_2, SP_3, Theme},
    widgets::{self, text},
};
use crate::{
    Squad,
    draft::{Draft, TEAM, takes},
    search::{self, Filter},
    source::{self, Source},
    store::{self, Amended, Registered},
    tier::{self, Sorted},
};

/// The sides of a place's icon, at most and at least: the two teams stay beside the button
/// in the narrowest window.
const PLACE: (f32, f32) = (56., 36.);
/// What a team's row takes beside its six places: its name, the gaps, the specials'
/// distance; and the room of the buttons at the rows' right (the widest says 変更を保存).
const ROW_BESIDE: f32 = 32. + 8. + 20. + 8.;
pub(super) const BUTTON_ROOM: f32 = 12. + 96.;
/// How wide a field of the record is: four figures.
const FIGURES: Pixels = px(56.);

/// What the formation put together was told with, as it is typed: the battles and the wins
/// of the record it was introduced with, and the page it comes from.
pub(super) struct Told {
    pub(super) battles: Entity<TextInput>,
    pub(super) wins: Entity<TextInput>,
    pub(super) url: Entity<TextInput>,
}

impl Told {
    pub(super) fn new(cx: &mut Context<Rio>) -> Self {
        // A field of the record takes digits alone (`source::figures`): what else was
        // typed goes as soon as the IME has settled it.
        let figures = |cx: &mut Context<Rio>| {
            let field = cx.new(|cx| TextInput::new("", "", cx));
            cx.subscribe(&field, |_, field, _: &text_input::Changed, cx| {
                let (typed, settled) = {
                    let field = field.read(cx);
                    (field.text().to_owned(), !field.is_composing())
                };
                let kept = source::figures(&typed);
                if settled && kept != typed {
                    field.update(cx, |field, cx| field.set_text(&kept, cx));
                }
                cx.notify();
            })
            .detach();
            field
        };
        let url = cx.new(|cx| TextInput::new("元のページの URL", "", cx));
        cx.subscribe(&url, |_, _, _: &text_input::Changed, cx| cx.notify())
            .detach();
        Self {
            battles: figures(cx),
            wins: figures(cx),
            url,
        }
    }

    /// What the fields hold: the battles, the wins, the page.
    pub(super) fn typed(&self, cx: &App) -> [String; 3] {
        [&self.battles, &self.wins, &self.url].map(|field| field.read(cx).text().to_owned())
    }

    /// Puts `typed` into the fields.
    fn put(&self, typed: &[String; 3], cx: &mut App) {
        for (field, typed) in [&self.battles, &self.wins, &self.url]
            .into_iter()
            .zip(typed)
        {
            field.update(cx, |field, cx| field.set_text(typed, cx));
        }
    }
}

/// A registration being changed in place of a new formation put together: its number, and
/// what the places and the fields held before, to come back once it is saved or given up.
pub(super) struct Editing {
    pub(super) id: i64,
    before: (Draft, [String; 3]),
}

impl Rio {
    /// What is missing before the formation can be kept, as the button says on a hover; or
    /// the teams and what they were told with.
    fn whole(&self, cx: &App) -> Result<([i64; TEAM], [i64; TEAM], Source), &'static str> {
        let [battles, wins, url] = self.told.typed(cx);
        let teams = self.draft.teams();
        let (attack, defense) = teams.ok_or("攻めと防衛の 12 人を入れると登録できます。")?;
        // A registration is not changed into the teams of another: one registration a pair.
        if let Some(Editing { id, .. }) = &self.editing {
            let same = |other: &&Registered| {
                store::both(other.attack, other.defense) == store::both(attack, defense)
            };
            let mut others = self.registered.iter().filter(|other| other.id != *id);
            if others.any(|other| same(&other)) {
                return Err("同じ攻めと防衛の登録が、もうあります。");
            }
        }
        let reported = source::record(&battles, &wins)
            .map_err(|_| "紹介の戦績は、戦数と勝数の両方を入れます（勝数は戦数まで）。")?;
        let url = source::page(&url)
            .map_err(|_| "URL は、http:// か https:// で始まるものを入れます。")?;
        Ok((attack, defense, Source { reported, url }))
    }

    /// The sub-tab: the attack's places over the defense's with the buttons at their right,
    /// what the formation was told with, the search, and the students found, who alone
    /// scroll.
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
        // Whole, the formation can be kept: until then the button says what is missing, and
        // a press does nothing.
        let (missing, editing) = (self.whole(cx).err(), self.editing.is_some());
        let label = if editing {
            "変更を保存"
        } else {
            "編成登録"
        };
        let register = widgets::outlined_button(theme, "register", label, missing.is_none())
            .when_some(missing, |button, how| {
                button
                    .aria_description(how)
                    .tooltip(widgets::tooltip(how))
                    .text_color(theme.muted)
                    .cursor_default()
            })
            .on_click(cx.listener(|this, _, _, cx| this.register(cx)));
        let buttons = div()
            .flex()
            .flex_col()
            .flex_none()
            .items_end()
            .gap(SP_2)
            .child(register)
            .when(editing, |buttons| {
                let cancel = widgets::outlined_button(theme, "cancel", "やめる", false)
                    .on_click(cx.listener(|this, _, _, cx| this.leave_editing(cx)));
                buttons.child(cancel)
            });
        let top = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .gap(SP_3)
            .child(teams)
            .child(buttons);

        let finder = self.finder(Sub::Add);
        let query = finder.query.read(cx).text().to_owned();
        // Nobody is offered who cannot go into the place chosen: its squad alone.
        let offered = Filter {
            squads: vec![takes(self.draft.chosen())],
            ..finder.filter.clone()
        };
        // Those the user sorted into a tier first, the highest tier before the others; by
        // name within each. Whether a student is barred has no say here: a formation told
        // of, and an opponent's, hold whom they hold (the user, 2026-11-21).
        let found = search::found(&self.roster, &query, &offered);
        let found = tier::by_tier(found, &self.shown.tiers);
        let tiles: Vec<_> = (found.into_iter())
            .map(|student| {
                let (id, squad) = (student.id, student.squad);
                // The list is of one squad: the frame tells the tier, and so does the
                // hover, which tells nothing of those barred.
                let tier = tier::of(&self.shown.tiers, id);
                let frame = tier.map_or(theme.border, theme::tier_colour);
                let sorted = Sorted {
                    tier,
                    ..Sorted::default()
                };
                Self::tile(student, None, TILE, frame, sorted)
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
            .child(self.told_row(theme, window, cx).mt(SP_2))
            .child(self.finding(theme, Sub::Add, false, window, cx).mt(SP_3))
            .child(students)
            .into_any_element()
    }

    /// What the formation was told with, to type: the record it was introduced with as the
    /// window writes it (紹介 20戦18勝), then the page. Either may stay empty.
    fn told_row(&self, theme: &Theme, window: &Window, cx: &Context<Self>) -> gpui::Div {
        let field = |id: &'static str, label: &'static str, input: &Entity<TextInput>| {
            let focused = input.focus_handle(cx).is_focused(window);
            div()
                .id(id)
                .debug_selector(move || id.to_owned())
                .role(Role::TextInput)
                .aria_label(label)
                .h(px(32.))
                .px(SP_2)
                .border_1()
                .border_color(if focused { theme.accent } else { theme.border })
                .rounded(RADIUS_SM)
                .bg(theme.surface)
                .text_size(FS_SM)
                .child(input.clone())
        };
        let word = |word: &'static str| {
            div()
                .flex_none()
                .text_size(FS_CAPTION)
                .text_color(theme.muted)
                .child(text(word))
        };
        // What the fields are for is on a hover of their word alone: over the fields it
        // would lie on what is typed under them.
        let how = "紹介された戦績（何戦して何勝か）。空のままでも登録できます。";
        let told = word("紹介")
            .id("told-word")
            .tooltip(widgets::tooltip(how))
            .mr(SP_1);
        let record = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(SP_1)
            .child(told)
            .child(
                field("told-battles", "紹介の戦数", &self.told.battles)
                    .flex_none()
                    .w(FIGURES),
            )
            .child(word("戦"))
            .child(
                field("told-wins", "紹介の勝数", &self.told.wins)
                    .flex_none()
                    .w(FIGURES),
            )
            .child(word("勝"));
        let page = field("told-url", "元のページの URL", &self.told.url)
            .flex_1()
            .min_w_0();
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(SP_3)
            .child(record)
            .child(page)
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
            Dragged::Student { id, squad, .. } => self.draft.drop_on(place, id, squad),
            Dragged::Place(from) => self.draft.swap(from, place),
        };
        cx.notify();
    }

    /// After another place was chosen than `before`: where the squad to offer changed, the
    /// students are another list, shown from its top.
    fn chose(&mut self, before: usize, cx: &mut Context<Self>) {
        if takes(before) != takes(self.draft.chosen()) {
            self.show_from_the_top();
        }
        cx.notify();
    }

    /// Shows the students to pick from the list's top.
    fn show_from_the_top(&self) {
        let top = gpui::point(px(0.), px(0.));
        self.finder(Sub::Add).scroll.set_offset(top);
    }

    /// Keeps the formation put together, once it is whole (`whole`): registers it as told
    /// to win, or changes the registration being changed.
    fn register(&mut self, cx: &mut Context<Self>) {
        let Ok((attack, defense, source)) = self.whole(cx) else {
            return;
        };
        let Some(id) = self.editing.as_ref().map(|editing| editing.id) else {
            return self.add(attack, defense, &source, cx);
        };
        match self.core.amend(id, attack, defense, &source) {
            Ok(Amended::Done) => {
                self.not_kept = None;
                self.leave_editing(cx);
                self.notify_toast(Some("変更しました"), Some(super::TOAST_STAYS), cx);
            }
            // An agent took it away meanwhile (`mcp`): there is nothing left to change.
            Ok(Amended::Gone) => {
                self.leave_editing(cx);
                let gone = "この登録は、もうありません";
                self.notify_toast(Some(gone), Some(super::TOAST_STAYS), cx);
            }
            Ok(Amended::Taken(_)) => {
                let taken = "同じ攻めと防衛の登録が、もうあります";
                self.notify_toast(Some(taken), Some(super::TOAST_STAYS), cx);
            }
            Err(error) => {
                self.not_kept = Some(error);
                cx.notify();
            }
        }
    }

    /// Registers a new formation. The places keep their students and the page stays:
    /// another attack against the same defense, told of on the same page, is at hand. The
    /// record was this attack's own: its fields are emptied.
    fn add(
        &mut self,
        attack: [i64; TEAM],
        defense: [i64; TEAM],
        source: &Source,
        cx: &mut Context<Self>,
    ) {
        match self.core.register(attack, defense, source) {
            Ok(new) => {
                self.not_kept = None;
                let said = if new {
                    let [_, _, url] = self.told.typed(cx);
                    self.told.put(&[String::new(), String::new(), url], cx);
                    "登録しました"
                } else {
                    "登録済みです"
                };
                self.notify_toast(Some(said), Some(super::TOAST_STAYS), cx);
            }
            Err(error) => {
                self.not_kept = Some(error);
                cx.notify();
            }
        }
    }

    /// Takes a registration into the places and the fields, to change it; what they held
    /// comes back once the change is saved or given up.
    pub(super) fn edit(&mut self, registered: &Registered, cx: &mut Context<Self>) {
        let before = match self.editing.take() {
            Some(editing) => editing.before,
            None => (self.draft.clone(), self.told.typed(cx)),
        };
        self.draft = Draft::of(registered.attack, registered.defense);
        let figure = |figure: Option<u32>| figure.map_or_else(String::new, |n| n.to_string());
        let source = &registered.source;
        let typed = [
            figure(source.reported.map(|told| told.battles)),
            figure(source.reported.map(|told| told.wins)),
            source.url.clone().unwrap_or_default(),
        ];
        self.told.put(&typed, cx);
        let id = registered.id;
        self.editing = Some(Editing { id, before });
        self.sub = Sub::Add;
        self.show_from_the_top();
        cx.notify();
    }

    /// Leaves the registration being changed (saved, or given up): the places and the
    /// fields hold again what they held before, and the list of the registrations shows.
    pub(super) fn leave_editing(&mut self, cx: &mut Context<Self>) {
        if let Some(Editing { before, .. }) = self.editing.take() {
            let (draft, typed) = before;
            self.draft = draft;
            self.told.put(&typed, cx);
            self.show_from_the_top();
            self.open_sub(Sub::Edit, cx);
        }
    }
}
