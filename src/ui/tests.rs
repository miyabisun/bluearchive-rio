//! The window as a user drives it, over what a test reports in place of the game (made as
//! Aoi's window tests).

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use futures::channel::mpsc;
use gpui::{
    Bounds, Entity, EntityInputHandler as _, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent,
    Pixels, TestAppContext, VisualTestContext, px, size,
};

use super::{Rio, Tab, Updates, text_input::TextInput, theme::Theme};
use crate::{
    Squad,
    app::{Core, Places, Snapshot, UpdateCheck},
    draft::Draft,
    opponent::Front,
    settings::{Settings, ThemeChoice},
    source::{Reported, Source},
    store::{Counsel, Fought, Ground, Kept, Likely, Player, Registered, Store, Tried},
    tier::Tier,
    watch::{Choice, Opponent, Screen, Status},
};

/// A folder of its own for each window, so what one keeps no other reads.
fn places() -> Places {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "rio-ui-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    // The reading makes the folder as it starts; no reading runs here.
    std::fs::create_dir_all(&dir).unwrap();
    Places {
        database: dir.join("rio.db"),
        settings: dir.join("settings.json"),
    }
}

/// The window, 720×820 as it opens, with nothing read yet. The game is not read: the tests
/// change the snapshot as the reading would.
fn open(cx: &mut TestAppContext) -> (Entity<Rio>, Arc<Core>, &mut VisualTestContext) {
    cx.update(|cx| {
        cx.set_global(Theme::sumi());
        super::bind_keys(cx);
    });
    let (changed, changes) = mpsc::unbounded();
    let core = Arc::new(Core::new(places(), changed));
    let shown = core.clone();
    let (rio, vcx) = cx.add_window_view(move |window, cx| {
        Rio::new(shown, changes, Settings::default(), window, cx)
    });
    vcx.simulate_resize(size(px(720.), px(820.)));
    vcx.run_until_parked();
    (rio, core, vcx)
}

/// The students barred, as the records keep them.
fn kept_barred(core: &Core) -> Vec<i64> {
    let records = Store::open(&core.places.database).unwrap();
    records.barred().unwrap()
}

/// Changes the snapshot as the reading would, and lets the window take it.
fn set(core: &Core, vcx: &mut VisualTestContext, change: impl FnOnce(&mut Snapshot)) {
    core.update(change);
    vcx.run_until_parked();
}

fn bounds(vcx: &mut VisualTestContext, selector: &str) -> Option<Bounds<Pixels>> {
    vcx.run_until_parked();
    vcx.debug_bounds(Box::leak(selector.to_owned().into_boxed_str()))
}

fn has(vcx: &mut VisualTestContext, selector: &str) -> bool {
    bounds(vcx, selector).is_some()
}

/// Whether this text is on the screen.
fn shows(vcx: &mut VisualTestContext, text: &str) -> bool {
    bounds(vcx, &format!("text:{text}")).is_some()
}

fn click(vcx: &mut VisualTestContext, selector: &str) {
    let at = bounds(vcx, selector)
        .unwrap_or_else(|| panic!("{selector} is not on the screen"))
        .center();
    vcx.simulate_click(at, Modifiers::none());
    vcx.run_until_parked();
}

/// A press of the right button, as on a student's icon for its menu.
fn right_click(vcx: &mut VisualTestContext, selector: &str) {
    let position = bounds(vcx, selector)
        .unwrap_or_else(|| panic!("{selector} is not on the screen"))
        .center();
    let (button, modifiers) = (MouseButton::Right, Modifiers::none());
    vcx.simulate_event(MouseDownEvent {
        button,
        position,
        modifiers,
        click_count: 1,
        first_mouse: false,
    });
    vcx.simulate_event(MouseUpEvent {
        button,
        position,
        modifiers,
        click_count: 1,
    });
    vcx.run_until_parked();
}

/// The first striker and the specials the opponent's dialog shows: ユウカ with イロハ and
/// ミチル（ドレス）.
const FRONT: Front = Front {
    first: 13010,
    specials: [20016, 20053],
};
/// The strikers of the two defenses likely behind them: マリナ, トキ and イオリ; and
/// マリナ, イオリ and another.
const BEHIND: [[i64; 3]; 2] = [[10037, 10062, 10006], [10037, 10006, 10124]];
/// Two attacks that won against the first: ホシノ, マリナ, ミカ and イオリ with イロハ and
/// アコ, and one with トキ in ミカ's place.
const ATTACKS: [[i64; 6]; 2] = [
    [10005, 10037, 10059, 10006, 20016, 20008],
    [10005, 10037, 10062, 10006, 20016, 20008],
];

/// An opponent with the front: the first defense is the opponent's own of this month, with
/// the two attacks (two of three won, one of two); the second is the trend, used by two
/// other players and by the user, and no attack won against it.
fn opponent() -> Opponent {
    let tried = |attack, wins, battles| Tried {
        attack,
        wins,
        battles,
        registered: false,
        reported: None,
    };
    let own = Likely {
        rest: BEHIND[0],
        ground: Ground::Theirs,
        their_last: Some("2026-11-17".to_owned()),
        others: 0,
        others_before: 0,
        mine: false,
        registered: false,
        attacks: vec![tried(ATTACKS[0], 2, 3), tried(ATTACKS[1], 1, 2)],
    };
    let others = Likely {
        rest: BEHIND[1],
        ground: Ground::Trend,
        their_last: None,
        others: 2,
        others_before: 0,
        mine: true,
        registered: false,
        attacks: Vec::new(),
    };
    Opponent {
        front: FRONT,
        name: None,
        counsel: Counsel {
            likely: vec![own, others],
        },
        held: None,
    }
}

/// ミカ, the third striker of the first attack (トキ stands there in the second).
const MIKA: i64 = 10059;

/// What the user chose of `opponent`: a defense, and an attack against it or none.
fn choice(rest: [i64; 3], attack: Option<[i64; 6]>) -> Option<Choice> {
    Some(Choice {
        front: FRONT,
        name: None,
        rest,
        attack,
    })
}

fn battle(id: i64, attacker: Option<&str>, attacker_won: bool) -> Kept {
    Kept {
        id,
        seen_at: format!("2026-11-17 13:1{id}:04"),
        attacker: attacker.map(|name| Player {
            id: 1,
            name: Some(name.to_owned()),
            plate: Vec::new(),
        }),
        defender: None,
        attacker_won,
        attack: [10005, 10037, 10059, 10006, 20016, 20008],
        defense: [13010, 10037, 10062, 10006, 20016, 20053],
        fought: None,
    }
}

#[gpui::test]
fn the_window_waits_for_an_opponent(cx: &mut TestAppContext) {
    let (_, _, vcx) = open(cx);
    assert!(shows(vcx, "ゲームを待っています"), "the status");
    assert!(
        shows(
            vcx,
            "対戦相手を選ぶと、防衛の編成の候補と、勝った攻撃の候補がここに出ます。"
        ),
        "what the match tab waits for"
    );
    assert!(!shows(vcx, "防衛の候補"));
}

#[gpui::test]
fn an_opponent_lists_the_defenses_likely_and_chooses_none(cx: &mut TestAppContext) {
    let (_, core, vcx) = open(cx);
    set(&core, vcx, |snapshot| {
        snapshot.status = Status::Watching(Some(Screen::Opponent));
        snapshot.opponent = Some(opponent());
    });
    assert!(shows(vcx, "対戦相手"), "the badge names the screen read");
    assert!(shows(vcx, "相手の防衛"));
    assert!(
        bounds(vcx, "defense:ユウカ").is_some(),
        "D1 as the game shows"
    );
    assert!(
        bounds(vcx, "defense:?").is_some() && bounds(vcx, "defense:トキ").is_none(),
        "nobody stands behind her until the user says who"
    );
    assert!(shows(vcx, "防衛の候補"));
    assert!(shows(vcx, "本人 11-17"));
    assert!(
        shows(vcx, "ほか 2人") && shows(vcx, "自分の防衛"),
        "players, not battles"
    );
    let (first, second) = (
        bounds(vcx, "candidate:likely-0").expect("the likeliest defense"),
        bounds(vcx, "candidate:likely-1").expect("the next"),
    );
    assert!(first.top() < second.top(), "the likeliest first");
    // Each stands under what makes it likely: the opponent's own, then the trend.
    let (theirs, trend) = (
        bounds(vcx, "ground:likely-0").expect("the opponent's own"),
        bounds(vcx, "ground:likely-1").expect("the trend"),
    );
    assert!(shows(vcx, "本人が使った編成（過去1か月）") && shows(vcx, "流行（過去2週間）"));
    assert!(theirs.top() < first.top() && first.top() < trend.top() && trend.top() < second.top());
    assert!(
        !shows(vcx, "それより前の記録"),
        "no heading without a defense"
    );
    // The attacks that won against the first stand under it, the best first.
    assert!(shows(vcx, "3戦2勝") && shows(vcx, "2戦1勝"));
    let (best, next) = (
        bounds(vcx, "candidate:attack-0-0").expect("the best attack"),
        bounds(vcx, "candidate:attack-0-1").expect("the next"),
    );
    assert!(first.top() < best.top() && best.top() < next.top() && next.top() < second.top());
    assert!(shows(vcx, "この防衛に勝った攻撃は、まだ登録がありません。"));
    assert_eq!(core.chosen(), None, "nothing is chosen for the user");
}

#[gpui::test]
fn a_likely_defense_shows_the_whole_team_and_marks_whom_rio_supplies(cx: &mut TestAppContext) {
    let (_, core, vcx) = open(cx);
    set(&core, vcx, |snapshot| snapshot.opponent = Some(opponent()));
    // D1 and the specials as the game shows them, D2 to D4 from the records, in the game's
    // order: nothing of the front is left to remember.
    let row = bounds(vcx, "candidate:likely-1").expect("a defense");
    let kinds = [
        "shown", "supplied", "supplied", "supplied", "shown", "shown",
    ];
    let mut left = row.left();
    for (slot, kind) in kinds.into_iter().enumerate() {
        let face = bounds(vcx, &format!("{kind}:likely-1-{slot}"))
            .unwrap_or_else(|| panic!("the icon of place {slot} is not {kind}"));
        assert!(
            face.left() > left,
            "place {slot} stands right of the one before"
        );
        assert!(face.top() >= row.top() && face.bottom() <= row.bottom());
        left = face.left();
    }
}

#[gpui::test]
fn a_click_chooses_an_attack_and_the_defense_it_is_against(cx: &mut TestAppContext) {
    let (_, core, vcx) = open(cx);
    set(&core, vcx, |snapshot| snapshot.opponent = Some(opponent()));
    click(vcx, "candidate:attack-0-1");
    assert_eq!(core.chosen(), choice(BEHIND[0], Some(ATTACKS[1])));
    assert!(has(vcx, "chosen:attack-0-1") && has(vcx, "chosen:likely-0"));
    assert!(has(vcx, "candidate:attack-0-0"), "the other attack is not");
    assert!(
        bounds(vcx, "defense:トキ").is_some(),
        "the defense chosen stands behind the front"
    );
    // The other attack against the same defense takes its place.
    click(vcx, "candidate:attack-0-0");
    assert_eq!(core.chosen(), choice(BEHIND[0], Some(ATTACKS[0])));
    // Clicked again, an attack is no longer chosen: its defense still is.
    click(vcx, "chosen:attack-0-0");
    assert_eq!(core.chosen(), choice(BEHIND[0], None));
    assert!(has(vcx, "chosen:likely-0") && has(vcx, "candidate:attack-0-0"));
    // A defense's own row chooses the defense alone, and gives it up again.
    click(vcx, "candidate:likely-1");
    assert_eq!(core.chosen(), choice(BEHIND[1], None));
    assert!(has(vcx, "chosen:likely-1") && has(vcx, "candidate:likely-0"));
    click(vcx, "chosen:likely-1");
    assert_eq!(core.chosen(), None);
    assert!(bounds(vcx, "defense:?").is_some());
}

#[gpui::test]
fn what_was_chosen_against_one_opponent_is_not_chosen_against_another(cx: &mut TestAppContext) {
    let (_, core, vcx) = open(cx);
    set(&core, vcx, |snapshot| snapshot.opponent = Some(opponent()));
    click(vcx, "candidate:attack-0-0");
    assert!(has(vcx, "chosen:attack-0-0"));
    // Another opponent, whose front the same defenses are likely behind.
    set(&core, vcx, |snapshot| {
        let mut other = opponent();
        other.front.first = 10005;
        snapshot.opponent = Some(other);
    });
    assert!(has(vcx, "candidate:likely-0") && has(vcx, "candidate:attack-0-0"));
    assert!(!has(vcx, "chosen:likely-0") && !has(vcx, "chosen:attack-0-0"));
    assert!(bounds(vcx, "defense:?").is_some());
}

#[gpui::test]
fn a_front_never_seen_says_so(cx: &mut TestAppContext) {
    let (_, core, vcx) = open(cx);
    set(&core, vcx, |snapshot| {
        let mut unseen = opponent();
        unseen.counsel = Counsel { likely: Vec::new() };
        snapshot.opponent = Some(unseen);
    });
    assert!(shows(
        vcx,
        "同じ D1 とスペシャルの防衛は、まだ登録がありません。"
    ));
    assert!(bounds(vcx, "defense:?").is_some(), "nobody is expected");
    assert!(!shows(vcx, "防衛の候補"), "no candidates without a defense");
}

#[gpui::test]
fn the_history_lists_the_battles_with_the_winner_marked(cx: &mut TestAppContext) {
    let (_, core, vcx) = open(cx);
    set(&core, vcx, |snapshot| {
        snapshot.battles = vec![
            battle(2, Some("先生"), false),
            battle(1, Some("先生"), true),
        ];
    });
    assert!(
        !shows(vcx, "\u{a0}2"),
        "the tab counts no battles: they only ever grow"
    );
    click(vcx, "tab:履歴");
    assert!(shows(vcx, "先生"), "the name the user gave");
    assert!(shows(vcx, "匿名"), "an anonymous defender");
    let (later, earlier) = (
        bounds(vcx, "battle:2").expect("the battle kept last"),
        bounds(vcx, "battle:1").expect("the battle kept first"),
    );
    assert!(later.top() < earlier.top(), "the latest first");
    // The winner's line carries the mark, at its middle.
    let on_line = |vcx: &mut VisualTestContext, mark: &str, line: &str| {
        let middle = |selector: &str, vcx: &mut VisualTestContext| {
            let found = bounds(vcx, selector).unwrap_or_else(|| panic!("{selector}"));
            found.center().y
        };
        (middle(mark, vcx) - middle(line, vcx)).abs() < px(1.)
    };
    assert!(
        on_line(vcx, "won:2:defense", "side:2:defense"),
        "the defender won the later battle"
    );
    assert!(bounds(vcx, "won:2:attack").is_none());
    assert!(on_line(vcx, "won:1:attack", "side:1:attack"));
}

#[gpui::test]
fn a_battle_shows_when_it_was_where_the_games_list_told(cx: &mut TestAppContext) {
    let (_, core, vcx) = open(cx);
    set(&core, vcx, |snapshot| {
        let told = Kept {
            fought: Some(Fought {
                about: "2026-11-17 22:30:00".to_owned(),
                within: 3600,
            }),
            ..battle(2, Some("先生"), false)
        };
        snapshot.battles = vec![told, battle(1, Some("先生"), true)];
    });
    click(vcx, "tab:履歴");
    assert!(shows(vcx, "11-17 22時ごろ"), "the battle's own time");
    assert!(
        !shows(vcx, "登録 11-17 13:12"),
        "and not when Rio kept it as well"
    );
    assert!(
        shows(vcx, "登録 11-17 13:11"),
        "a battle no row told of shows when it was kept"
    );
}

#[gpui::test]
fn the_history_says_how_a_battle_comes_into_it(cx: &mut TestAppContext) {
    let (_, _, vcx) = open(cx);
    click(vcx, "tab:履歴");
    assert!(shows(vcx, "ゲームで戦闘履歴を開くと、ここに登録されます。"));
}

#[gpui::test]
fn the_status_badge_pauses_and_resumes(cx: &mut TestAppContext) {
    let (_, core, vcx) = open(cx);
    click(vcx, "status");
    assert!(shows(vcx, "一時中断"));
    assert_eq!(core.snapshot().status, Status::Paused);
    click(vcx, "status");
    assert!(
        shows(vcx, "ゲームを待っています"),
        "until the reading says more"
    );
}

#[gpui::test]
fn a_failure_is_told_in_the_page(cx: &mut TestAppContext) {
    let (_, core, vcx) = open(cx);
    set(&core, vcx, |snapshot| {
        snapshot.status = Status::Failed("戦闘の記録を読み書きできません: disk full".to_owned());
    });
    assert!(shows(vcx, "読み取れません"), "the badge");
    assert!(bounds(vcx, "read-error").is_some(), "why, in the page");
}

#[gpui::test]
fn the_theme_chosen_is_kept(cx: &mut TestAppContext) {
    let (_, core, vcx) = open(cx);
    click(vcx, "settings-button");
    assert!(shows(vcx, "テーマ"));
    click(vcx, "choice-theme-ライト");
    assert!(!vcx.read(Theme::get).dark, "the window turns light");
    assert_eq!(
        Settings::load(&core.places.settings).theme,
        ThemeChoice::Light,
        "kept for the next start"
    );
    // A tab closes the page.
    click(vcx, "tab:対戦");
    assert!(!shows(vcx, "テーマ"));
}

fn answering(rio: &Entity<Rio>, vcx: &mut VisualTestContext, updates: Updates) {
    rio.update(vcx, |rio, _| rio.updates = updates);
}

/// How often the app was started again as the version taken.
static RESTARTS: AtomicUsize = AtomicUsize::new(0);

#[gpui::test]
fn the_settings_check_for_an_update_and_take_it(cx: &mut TestAppContext) {
    let (rio, _core, vcx) = open(cx);
    fn found(latest: Option<&str>) -> UpdateCheck {
        UpdateCheck {
            current: "v0.1.0".to_owned(),
            latest: latest.map(str::to_owned),
        }
    }
    answering(
        &rio,
        vcx,
        Updates {
            check: || Ok(found(None)),
            ..Updates::default()
        },
    );
    click(vcx, "settings-button");
    let (check, theme) = (
        bounds(vcx, "check").expect("the button to check"),
        bounds(vcx, "choice-theme-ライト").expect("the theme's choices"),
    );
    assert!(check.top() < theme.top(), "the update is the page's first");
    click(vcx, "check");
    assert!(
        shows(vcx, "最新のバージョンです（v0.1.0）"),
        "up to date, a toast says so"
    );
    vcx.executor()
        .advance_clock(std::time::Duration::from_secs(4));
    vcx.run_until_parked();
    assert!(!shows(vcx, "最新のバージョンです（v0.1.0）"), "and goes");

    answering(
        &rio,
        vcx,
        Updates {
            check: || Ok(found(Some("v0.1.1"))),
            apply: || Err("書き込めませんでした".to_owned()),
            restart: || {},
        },
    );
    click(vcx, "check");
    assert!(
        shows(vcx, "v0.1.1 があります（今 v0.1.0）"),
        "a newer version stays in the page"
    );
    click(vcx, "tab:履歴");
    assert!(has(vcx, "install"), "over a tab too, with the button");
    click(vcx, "install");
    assert!(
        shows(vcx, "アップデートできませんでした: 書き込めませんでした"),
        "a failure says why"
    );
    assert!(!has(vcx, "install"));

    answering(
        &rio,
        vcx,
        Updates {
            check: || Ok(found(Some("v0.1.1"))),
            apply: || Ok(()),
            restart: || {
                RESTARTS.fetch_add(1, Ordering::Relaxed);
            },
        },
    );
    click(vcx, "settings-button");
    click(vcx, "check");
    click(vcx, "install");
    assert_eq!(
        RESTARTS.load(Ordering::Relaxed),
        1,
        "taken, the app starts again as it"
    );

    answering(
        &rio,
        vcx,
        Updates {
            check: || Err("通信できませんでした".to_owned()),
            ..Updates::default()
        },
    );
    click(vcx, "check");
    assert!(
        shows(vcx, "アップデートできませんでした: 通信できませんでした"),
        "a check that fails says why"
    );
}

#[gpui::test]
fn a_right_click_bars_a_student_and_sets_the_attacks_with_her_aside(cx: &mut TestAppContext) {
    let (_, core, vcx) = open(cx);
    set(&core, vcx, |snapshot| snapshot.opponent = Some(opponent()));
    assert!(!has(vcx, "excluded:attack-0-1"), "nobody is left out yet");
    // The menu of ミカ, in the best attack: it names her and bars nobody by itself.
    right_click(vcx, "face:attack-0-0-2");
    assert!(has(vcx, "student-menu") && shows(vcx, "ミカ"));
    assert!(has(vcx, "menu:編成NG") && !has(vcx, "menu:編成NGを外す"));
    assert_eq!(core.chosen(), None, "the right button chooses nothing");
    click(vcx, "menu:編成NG");
    assert!(!has(vcx, "student-menu"), "the menu goes");
    assert_eq!(kept_barred(&core), [MIKA], "kept with the records");
    // The attack with her stands under the other, set aside, and she is the one marked.
    let (usable, aside) = (
        bounds(vcx, "candidate:attack-0-0").expect("the attack the user can take"),
        bounds(vcx, "excluded:attack-0-1").expect("the attack set aside"),
    );
    assert!(usable.top() < aside.top());
    assert!(has(vcx, "out:attack-0-1-2"), "ミカ is marked");
    assert!(
        has(vcx, "face:attack-0-1-0") && !has(vcx, "out:attack-0-1-0"),
        "ホシノ beside her is not"
    );
    click(vcx, "candidate:attack-0-0");
    assert_eq!(
        core.chosen(),
        choice(BEHIND[0], Some(ATTACKS[1])),
        "the attack with トキ is the one left to take"
    );
    // An attack set aside cannot be chosen.
    click(vcx, "excluded:attack-0-1");
    assert_eq!(core.chosen(), choice(BEHIND[0], Some(ATTACKS[1])));

    // Her menu lets her back in: the attacks stand as the records order them again.
    right_click(vcx, "out:attack-0-1-2");
    assert!(has(vcx, "menu:編成NGを外す") && !has(vcx, "menu:編成NG"));
    click(vcx, "menu:編成NGを外す");
    assert!(kept_barred(&core).is_empty());
    assert!(!has(vcx, "excluded:attack-0-1") && !has(vcx, "out:attack-0-1-2"));
    click(vcx, "candidate:attack-0-0");
    assert_eq!(core.chosen(), choice(BEHIND[0], Some(ATTACKS[0])));
}

#[gpui::test]
fn barring_a_student_of_the_attack_chosen_gives_the_attack_up(cx: &mut TestAppContext) {
    let (_, core, vcx) = open(cx);
    set(&core, vcx, |snapshot| snapshot.opponent = Some(opponent()));
    click(vcx, "candidate:attack-0-1");
    assert_eq!(core.chosen(), choice(BEHIND[0], Some(ATTACKS[1])));
    // ホシノ stands in both attacks: neither is left to take.
    right_click(vcx, "face:attack-0-1-0");
    click(vcx, "menu:編成NG");
    assert_eq!(
        core.chosen(),
        choice(BEHIND[0], None),
        "no student the user barred is marked over the game: the defense stays chosen"
    );
    assert!(has(vcx, "excluded:attack-0-0") && has(vcx, "excluded:attack-0-1"));
    assert!(has(vcx, "chosen:likely-0"));
}

#[gpui::test]
fn barring_another_student_keeps_the_attack_chosen(cx: &mut TestAppContext) {
    let (_, core, vcx) = open(cx);
    set(&core, vcx, |snapshot| snapshot.opponent = Some(opponent()));
    click(vcx, "candidate:attack-0-0");
    right_click(vcx, "face:attack-0-1-2");
    click(vcx, "menu:編成NG");
    assert_eq!(core.chosen(), choice(BEHIND[0], Some(ATTACKS[0])));
    assert!(has(vcx, "chosen:attack-0-0") && has(vcx, "excluded:attack-0-1"));
}

#[gpui::test]
fn an_attack_with_a_student_not_held_is_set_aside(cx: &mut TestAppContext) {
    let (_, core, vcx) = open(cx);
    set(&core, vcx, |snapshot| {
        // Aoi lists every student of the attacks but ミカ.
        let held = ATTACKS.concat().into_iter().filter(|one| *one != MIKA);
        snapshot.opponent = Some(Opponent {
            held: Some(held.collect()),
            ..opponent()
        });
    });
    assert!(has(vcx, "candidate:attack-0-0") && has(vcx, "excluded:attack-0-1"));
    assert!(
        has(vcx, "out:attack-0-1-2"),
        "ミカ, whom the user does not hold"
    );
    assert!(
        shows(vcx, "2戦1勝") && shows(vcx, "3戦2勝"),
        "both still told"
    );
}

#[gpui::test]
fn the_menu_closes_without_barring(cx: &mut TestAppContext) {
    let (rio, core, vcx) = open(cx);
    // The keys go to the window, as when the app opens it.
    rio.update_in(vcx, |rio, window, cx| rio.root_focus.focus(window, cx));
    set(&core, vcx, |snapshot| snapshot.opponent = Some(opponent()));
    right_click(vcx, "face:attack-0-0-2");
    assert!(has(vcx, "student-menu"));
    vcx.simulate_keystrokes("escape");
    assert!(!has(vcx, "student-menu"), "Escape closes it");
    right_click(vcx, "face:attack-0-0-2");
    click(vcx, "tab:対戦");
    assert!(!has(vcx, "student-menu"), "so does a press beside it");
    assert!(!has(vcx, "excluded:attack-0-0") && !has(vcx, "excluded:attack-0-1"));
    assert!(core.snapshot().barred.is_empty(), "nobody was barred");
    // The icons of a defense have no such menu.
    right_click(vcx, "supplied:likely-0-1");
    assert!(!has(vcx, "student-menu"));
}

#[gpui::test]
fn a_student_who_cannot_be_barred_says_why(cx: &mut TestAppContext) {
    let (_, core, vcx) = open(cx);
    set(&core, vcx, |snapshot| snapshot.opponent = Some(opponent()));
    // The records cannot be written: their folder is gone.
    std::fs::remove_dir_all(core.places.database.parent().unwrap()).unwrap();
    right_click(vcx, "face:attack-0-0-2");
    click(vcx, "menu:編成NG");
    assert!(has(vcx, "keep-error"), "why, in the page");
    assert!(!has(vcx, "excluded:attack-0-1"), "and nobody is barred");
}

#[gpui::test]
fn the_menu_goes_as_another_page_opens(cx: &mut TestAppContext) {
    let (rio, core, vcx) = open(cx);
    set(&core, vcx, |snapshot| snapshot.opponent = Some(opponent()));
    right_click(vcx, "face:attack-0-0-2");
    assert!(has(vcx, "student-menu"));
    // Another tab opened by the keys: no press beside the menu closes it.
    let turn = |vcx: &mut VisualTestContext, tab| {
        rio.update(vcx, |rio, cx| {
            rio.tab = tab;
            cx.notify();
        });
        vcx.run_until_parked();
    };
    turn(vcx, Tab::History);
    assert!(!has(vcx, "student-menu"), "the menu is not of this page");
    turn(vcx, Tab::Match);
    assert!(!has(vcx, "student-menu"), "and does not come back");
}

/// The window with the sub-tab open the formations are put together in (編成登録の「追加」,
/// the one the tab opens with).
fn compose(cx: &mut TestAppContext) -> (Entity<Rio>, Arc<Core>, &mut VisualTestContext) {
    let (rio, core, vcx) = open(cx);
    click(vcx, "tab:編成登録");
    (rio, core, vcx)
}

/// Whether what `inner` finds lies within what `outer` finds.
fn within(vcx: &mut VisualTestContext, inner: &str, outer: &str) -> bool {
    let (Some(inner), Some(outer)) = (bounds(vcx, inner), bounds(vcx, outer)) else {
        return false;
    };
    outer.contains(&inner.origin)
        && outer.contains(&(inner.bottom_right() - gpui::point(px(1.), px(1.))))
}

/// The teams as far as they are put together.
fn draft(rio: &Entity<Rio>, vcx: &mut VisualTestContext) -> Draft {
    rio.read_with(vcx, |rio, _| rio.draft.clone())
}

/// Drags what `from` finds onto what `to` finds.
fn drag(vcx: &mut VisualTestContext, from: &str, to: &str) {
    let middle = |vcx: &mut VisualTestContext, selector: &str| {
        let found = bounds(vcx, selector).unwrap_or_else(|| panic!("{selector} is not there"));
        found.center()
    };
    let (from, to) = (middle(vcx, from), middle(vcx, to));
    let none = Modifiers::none();
    vcx.simulate_mouse_down(from, MouseButton::Left, none);
    vcx.simulate_mouse_move(from + gpui::point(px(8.), px(8.)), MouseButton::Left, none);
    vcx.simulate_mouse_move(to, MouseButton::Left, none);
    vcx.simulate_mouse_up(to, MouseButton::Left, none);
    vcx.run_until_parked();
}

/// Types into the search field as the IME would, the text settled.
fn search(rio: &Entity<Rio>, vcx: &mut VisualTestContext, typed: &str) {
    let field = rio.read_with(vcx, |rio, _| rio.finder(rio.sub).query.clone());
    type_into(vcx, &field, typed);
}

/// Types into a field in place of what it holds, as the IME would, the text settled.
fn type_into(vcx: &mut VisualTestContext, field: &Entity<TextInput>, typed: &str) {
    field.update_in(vcx, |field, window, cx| {
        let all = 0..field.text().encode_utf16().count();
        field.replace_text_in_range(Some(all), typed, window, cx);
    });
    vcx.run_until_parked();
}

/// Types what a formation was told with: the battles and the wins of its record, the page.
fn tell(rio: &Entity<Rio>, vcx: &mut VisualTestContext, [battles, wins, url]: [&str; 3]) {
    let fields = rio.read_with(vcx, |rio, _| {
        let told = &rio.told;
        [told.battles.clone(), told.wins.clone(), told.url.clone()]
    });
    for (field, typed) in fields.iter().zip([battles, wins, url]) {
        type_into(vcx, field, typed);
    }
}

/// What the fields of what a formation was told with hold.
fn told(rio: &Entity<Rio>, vcx: &mut VisualTestContext) -> [String; 3] {
    rio.read_with(vcx, |rio, cx| rio.told.typed(cx))
}

/// The formations registered, as the records keep them: the one registered last first.
fn kept_registered(core: &Core) -> Vec<Registered> {
    let records = Store::open(&core.places.database).unwrap();
    records.registered().unwrap()
}

/// アイリ, the first student by name and a special; アイリ（バンド）, the first striker.
const AIRI: i64 = 23000;
const AIRI_BAND: i64 = 16015;

#[gpui::test]
fn the_compose_tab_starts_with_the_attacks_first_striker_chosen(cx: &mut TestAppContext) {
    let (_, _, vcx) = compose(cx);
    // The attack's six places over the defense's six, the button at their right.
    let place = |vcx: &mut VisualTestContext, at: usize| {
        bounds(vcx, &format!("place:{at}")).unwrap_or_else(|| panic!("no place {at}"))
    };
    let (first, under, last) = (place(vcx, 0), place(vcx, 6), place(vcx, 11));
    assert!(first.top() < under.top() && first.left() == under.left());
    assert!(shows(vcx, "攻め") && shows(vcx, "防衛"));
    let (fourth, special) = (place(vcx, 3), place(vcx, 4));
    assert!(
        special.left() - fourth.right() > fourth.left() - place(vcx, 2).right(),
        "the specials stand apart"
    );
    let register = bounds(vcx, "register").expect("the button");
    assert!(place(vcx, 5).right() < register.left() && last.right() < register.left());
    assert!(
        shows(vcx, "編成登録") && !has(vcx, "cancel"),
        "a new formation"
    );
    // Under them what the formation was told with: the record, then the page.
    let (battles, wins, page) = (
        bounds(vcx, "told-battles").expect("the battles' field"),
        bounds(vcx, "told-wins").expect("the wins' field"),
        bounds(vcx, "told-url").expect("the page's field"),
    );
    assert!(last.bottom() < battles.top());
    assert!(battles.right() < wins.left() && wins.right() < page.left());
    // Under them the search and the students, by name.
    let (field, students) = (
        bounds(vcx, "search").expect("the search field"),
        bounds(vcx, "students").expect("the list"),
    );
    assert!(battles.bottom() < field.top() && field.bottom() <= students.top());
    let (band, aoba) = (
        bounds(vcx, "tile:16015").expect("アイリ（バンド）"),
        bounds(vcx, "tile:16017").expect("アオバ"),
    );
    assert!(band.left() < aoba.left() && band.top() == aoba.top());
    assert!(band.top() >= students.top());
    // The place chosen has the corners; a click chooses another.
    assert!(within(vcx, "cursor", "place:0"));
    click(vcx, "place:8");
    assert!(within(vcx, "cursor", "place:8") && !within(vcx, "cursor", "place:0"));
}

#[gpui::test]
fn a_student_clicked_goes_into_the_place_chosen_and_the_next_is_chosen(cx: &mut TestAppContext) {
    let (rio, _, vcx) = compose(cx);
    // For a striker's place the strikers alone are offered: nobody who could not go there.
    assert!(has(vcx, &format!("tile:{AIRI_BAND}")) && !has(vcx, &format!("tile:{AIRI}")));
    click(vcx, &format!("tile:{AIRI_BAND}"));
    assert_eq!(draft(&rio, vcx).at(0), Some(AIRI_BAND));
    assert!(within(vcx, "cursor", "place:1"), "the place to the right");
    // After the fourth striker the left special is chosen, and the specials are offered.
    click(vcx, "place:3");
    click(vcx, &format!("tile:{AIRI_BAND}"));
    assert!(within(vcx, "cursor", "place:4"));
    assert_eq!(
        (draft(&rio, vcx).at(0), draft(&rio, vcx).at(3)),
        (None, Some(AIRI_BAND)),
        "she moved: a student stands once in a team"
    );
    assert!(has(vcx, &format!("tile:{AIRI}")) && !has(vcx, &format!("tile:{AIRI_BAND}")));
    // After the attack's right special, the defense's first striker: strikers again.
    click(vcx, "place:5");
    click(vcx, &format!("tile:{AIRI}"));
    assert_eq!(draft(&rio, vcx).at(5), Some(AIRI));
    assert!(within(vcx, "cursor", "place:6"));
    assert!(has(vcx, &format!("tile:{AIRI_BAND}")) && !has(vcx, &format!("tile:{AIRI}")));
}

#[gpui::test]
fn the_students_are_narrowed_by_the_name_and_the_buttons(cx: &mut TestAppContext) {
    let (rio, _, vcx) = compose(cx);
    let shown = |vcx: &mut VisualTestContext, students: &[i64]| -> Vec<bool> {
        let tile = |student: &i64| has(vcx, &format!("tile:{student}"));
        students.iter().map(tile).collect()
    };
    // ホシノ (a tank of pierce), her swimsuit (a supporter of explosion), and the two of
    // the last stand (a tank and an attacker of mystic); アル is another.
    let hoshinos = [10005, 10045, 10098, 10099, 10000];
    search(&rio, vcx, "ほしの");
    assert_eq!(shown(vcx, &hoshinos), [true, true, true, true, false]);
    // One class: the tanks.
    click(vcx, "filter:タンク");
    assert!(has(vcx, "pressed:タンク"), "the button stays pressed");
    assert_eq!(shown(vcx, &hoshinos), [true, false, true, false, false]);
    // Another class takes its place: one of a kind is pressed at a time.
    click(vcx, "filter:サポーター");
    assert!(has(vcx, "pressed:サポーター") && has(vcx, "filter:タンク"));
    assert!(!has(vcx, "pressed:タンク"), "the tanks' button is let up");
    assert_eq!(shown(vcx, &hoshinos), [false, true, false, false, false]);
    click(vcx, "filter:タンク");
    assert_eq!(shown(vcx, &hoshinos), [true, false, true, false, false]);
    // An attack type: with the class, both have to fit.
    click(vcx, "filter:神秘");
    assert_eq!(shown(vcx, &hoshinos), [false, false, true, false, false]);
    // An armor type none of them has: nobody, and the window says so.
    click(vcx, "filter:弾力装甲");
    assert_eq!(shown(vcx, &hoshinos), [false; 5]);
    assert!(shows(vcx, "当てはまる生徒がいません。"));
    // Pressed again, the buttons are let up; the name cleared, everybody is back.
    for pressed in ["弾力装甲", "神秘", "タンク"] {
        click(vcx, &format!("pressed:{pressed}"));
    }
    assert!(has(vcx, "filter:タンク") && !has(vcx, "pressed:タンク"));
    search(&rio, vcx, "");
    assert!(has(vcx, &format!("tile:{AIRI_BAND}")) && has(vcx, "tile:16017"));
}

#[gpui::test]
fn students_and_places_are_dragged(cx: &mut TestAppContext) {
    let (rio, _, vcx) = compose(cx);
    // A student of the list is thrown straight into a place: the place chosen stays.
    drag(vcx, &format!("tile:{AIRI_BAND}"), "place:2");
    assert_eq!(draft(&rio, vcx).at(2), Some(AIRI_BAND));
    assert!(within(vcx, "cursor", "place:0"));
    // A place of the other squad takes no drop, and nobody is told off.
    drag(vcx, &format!("tile:{AIRI_BAND}"), "place:4");
    assert_eq!(draft(&rio, vcx).at(4), None);
    // With a special's place chosen the specials are offered, to throw in as well.
    click(vcx, "place:4");
    drag(vcx, &format!("tile:{AIRI}"), "place:10");
    assert_eq!(draft(&rio, vcx).at(10), Some(AIRI));
    drag(vcx, &format!("tile:{AIRI}"), "place:1");
    assert_eq!(draft(&rio, vcx).at(1), None);
    // Two places change their students, across the teams too.
    drag(vcx, "place:2", "place:7");
    assert_eq!(
        (draft(&rio, vcx).at(2), draft(&rio, vcx).at(7)),
        (None, Some(AIRI_BAND))
    );
    click(vcx, "place:0");
    drag(vcx, &format!("tile:{}", 16017), "place:2");
    drag(vcx, "place:7", "place:2");
    assert_eq!(
        (draft(&rio, vcx).at(2), draft(&rio, vcx).at(7)),
        (Some(AIRI_BAND), Some(16017))
    );
    // A striker and a special do not.
    drag(vcx, "place:10", "place:7");
    assert_eq!(
        (draft(&rio, vcx).at(10), draft(&rio, vcx).at(7)),
        (Some(AIRI), Some(16017))
    );
}

/// Puts a team into the places from `first` on.
fn fill(rio: &Entity<Rio>, vcx: &mut VisualTestContext, first: usize, team: [i64; 6]) {
    rio.update(vcx, |rio, cx| {
        for (at, student) in team.into_iter().enumerate() {
            let squad = if at < 4 { Squad::Main } else { Squad::Support };
            rio.draft.drop_on(first + at, student, squad).unwrap();
        }
        cx.notify();
    });
    vcx.run_until_parked();
}

/// A defense behind `FRONT`.
const DEFENSE: [i64; 6] = [13010, 10037, 10062, 10006, 20016, 20053];

#[gpui::test]
fn a_formation_is_registered_once_it_is_whole_and_shows_against_its_front(cx: &mut TestAppContext) {
    let (rio, core, vcx) = compose(cx);
    // An opponent with the front, nothing known behind it.
    set(&core, vcx, |snapshot| {
        let mut unseen = opponent();
        unseen.counsel = Counsel { likely: Vec::new() };
        snapshot.opponent = Some(unseen);
    });
    let registered = |core: &Core| {
        let records = Store::open(&core.places.database).unwrap();
        records.counsel(&FRONT, None, 0).unwrap().likely
    };
    // Half put together, the button registers nothing.
    fill(&rio, vcx, 0, ATTACKS[0]);
    click(vcx, "register");
    assert!(registered(&core).is_empty());
    assert!(!shows(vcx, "登録しました"));
    // Whole, it does, once; the places keep their students.
    fill(&rio, vcx, 6, DEFENSE);
    click(vcx, "register");
    assert!(shows(vcx, "登録しました"));
    assert_eq!(registered(&core).len(), 1, "kept with the records");
    click(vcx, "register");
    assert!(shows(vcx, "登録済みです"));
    assert_eq!(registered(&core)[0].attacks.len(), 1);
    assert_eq!(draft(&rio, vcx).teams(), Some((ATTACKS[0], DEFENSE)));
    // Against the opponent with that front it is counselled at once.
    click(vcx, "tab:対戦");
    assert!(shows(vcx, "登録した編成"), "what makes the defense likely");
    assert!(has(vcx, "candidate:likely-0") && has(vcx, "candidate:attack-0-0"));
    assert!(has(vcx, "registered:attack-0-0") && shows(vcx, "登録"));
    click(vcx, "candidate:attack-0-0");
    // The records keep a team's specials the smaller id first.
    let kept = [10005, 10037, 10059, 10006, 20008, 20016];
    assert_eq!(
        core.chosen(),
        choice([10037, 10062, 10006], Some(kept)),
        "to choose like any other"
    );
    // Its word's menu takes the registration away.
    right_click(vcx, "registered:attack-0-0");
    assert!(has(vcx, "menu:登録を消す"));
    click(vcx, "menu:登録を消す");
    assert!(registered(&core).is_empty());
    assert!(!has(vcx, "candidate:likely-0"));
    assert!(shows(
        vcx,
        "同じ D1 とスペシャルの防衛は、まだ登録がありません。"
    ));
    // A slip of the hand: the toast puts it back, and it is counselled again.
    assert!(shows(vcx, "削除しました"));
    click(vcx, "undo");
    assert_eq!(registered(&core).len(), 1);
    assert!(has(vcx, "registered:attack-0-0") && !has(vcx, "undo"));
}

#[gpui::test]
fn the_least_window_shows_the_teams_the_search_and_three_rows_of_students(cx: &mut TestAppContext) {
    let (_, _, vcx) = compose(cx);
    let (wide, tall) = super::WINDOW_LEAST;
    vcx.simulate_resize(size(px(wide), px(tall)));
    vcx.run_until_parked();
    // The sub-tabs, the button beside the teams, the fields, the search and every button of
    // it are within the width.
    let mut parts = vec!["place:5", "place:11", "register", "search"];
    parts.extend(["sub:追加", "sub:Tier表", "sub:編集"]);
    parts.extend(["told-battles", "told-wins", "told-url"]);
    parts.extend(["filter:タンク", "filter:TS", "filter:爆発", "filter:振動"]);
    parts.extend(["filter:軽装備", "filter:弾力装甲"]);
    for part in parts {
        let found = bounds(vcx, part).unwrap_or_else(|| panic!("{part} is not there"));
        assert!(
            found.left() >= px(0.) && found.right() <= px(wide),
            "{part} is cut: {found:?}"
        );
    }
    assert!(bounds(vcx, "place:5").unwrap().right() < bounds(vcx, "register").unwrap().left());
    // Three rows of students show whole.
    let (students, first) = (
        bounds(vcx, "students").expect("the list"),
        bounds(vcx, &format!("tile:{AIRI_BAND}")).expect("the first student"),
    );
    // The table of tiers takes more over its students and decides how low the window is
    // made: here more than three rows show.
    let rows = (students.bottom() - first.top()) / (first.size.height + px(4.));
    assert!(rows >= 3., "{rows} rows of students show");
    assert!(students.bottom() <= px(tall));
}

/// The window with the sub-tab open the students are sorted in (編成登録の「Tier表」).
fn tiers(cx: &mut TestAppContext) -> (Entity<Rio>, Arc<Core>, &mut VisualTestContext) {
    let (rio, core, vcx) = open(cx);
    click(vcx, "tab:編成登録");
    click(vcx, "sub:Tier表");
    (rio, core, vcx)
}

/// The students in tiers, as the records keep them.
fn kept_tiers(core: &Core) -> Vec<(i64, Tier)> {
    let records = Store::open(&core.places.database).unwrap();
    records.tiers().unwrap()
}

#[gpui::test]
fn the_table_has_the_tiers_and_those_barred_over_the_students_of_both_squads(
    cx: &mut TestAppContext,
) {
    let (_, _, vcx) = tiers(cx);
    // S to D from the top, each with its letter, and under them those barred (NG).
    let mut top = px(0.);
    for name in ["S", "A", "B", "C", "D", "NG"] {
        let row = bounds(vcx, &format!("tier:{name}")).unwrap_or_else(|| panic!("no tier {name}"));
        assert!(row.top() > top, "tier {name} stands under the one before");
        assert!(shows(vcx, name));
        top = row.top();
    }
    assert!(
        shows(vcx, "一覧の生徒をドラッグして入れます"),
        "with nobody in a tier, how one comes into it"
    );
    // Under them the search with the squads' buttons before the others, and the students:
    // the specials and the strikers together, by name.
    let (field, students) = (
        bounds(vcx, "search").expect("the search field"),
        bounds(vcx, "students").expect("the list"),
    );
    assert!(top < field.top() && field.bottom() <= students.top());
    let (strikers, specials, classes) = (
        bounds(vcx, "filter:ストライカー").expect("the strikers' button"),
        bounds(vcx, "filter:スペシャル").expect("the specials' button"),
        bounds(vcx, "filter:タンク").expect("the classes' buttons"),
    );
    assert!(strikers.left() < specials.left() && specials.left() < classes.left());
    let (airi, band) = (
        bounds(vcx, &format!("tile:{AIRI}")).expect("アイリ, a special"),
        bounds(vcx, &format!("tile:{AIRI_BAND}")).expect("アイリ（バンド）, a striker"),
    );
    assert!(airi.left() < band.left() && airi.top() == band.top());
}

#[gpui::test]
fn a_student_is_dragged_into_a_tier_to_another_and_out_again(cx: &mut TestAppContext) {
    let (_, core, vcx) = tiers(cx);
    let (tile, tiered) = (format!("tile:{AIRI_BAND}"), format!("tiered:{AIRI_BAND}"));
    // A click does nothing: the students are sorted by dragging them.
    click(vcx, &tile);
    assert!(has(vcx, &tile) && kept_tiers(&core).is_empty());
    // Into a tier: she stands in its row, and no longer in the list.
    drag(vcx, &tile, "tier:S");
    assert!(within(vcx, &tiered, "tier:S"));
    assert!(!has(vcx, &tile), "gone from the list");
    assert_eq!(kept_tiers(&core), [(AIRI_BAND, Tier::S)], "kept");
    assert!(!shows(vcx, "一覧の生徒をドラッグして入れます"));
    click(vcx, &tiered);
    assert_eq!(
        kept_tiers(&core),
        [(AIRI_BAND, Tier::S)],
        "a click: nothing"
    );
    // A special beside her, into another tier.
    drag(vcx, &format!("tile:{AIRI}"), "tier:C");
    assert!(within(vcx, &format!("tiered:{AIRI}"), "tier:C"));
    // From a tier to another.
    drag(vcx, &tiered, "tier:B");
    assert!(within(vcx, &tiered, "tier:B") && !within(vcx, &tiered, "tier:S"));
    assert_eq!(
        kept_tiers(&core),
        [(AIRI_BAND, Tier::B), (AIRI, Tier::C)],
        "each in one tier"
    );
    // Back onto the list: out of the tiers.
    drag(vcx, &tiered, "students");
    assert!(has(vcx, &tile) && !has(vcx, &tiered));
    assert_eq!(kept_tiers(&core), [(AIRI, Tier::C)]);
}

#[gpui::test]
fn the_squads_buttons_narrow_the_tier_tabs_students_only(cx: &mut TestAppContext) {
    let (_, _, vcx) = tiers(cx);
    let shown = |vcx: &mut VisualTestContext| {
        let special = has(vcx, &format!("tile:{AIRI}"));
        (special, has(vcx, &format!("tile:{AIRI_BAND}")))
    };
    assert_eq!(shown(vcx), (true, true), "the special and the striker");
    click(vcx, "filter:ストライカー");
    assert!(has(vcx, "pressed:ストライカー"));
    assert_eq!(shown(vcx), (false, true), "the strikers alone");
    // The other squad takes its place: the strikers' button is let up.
    click(vcx, "filter:スペシャル");
    assert!(has(vcx, "pressed:スペシャル") && has(vcx, "filter:ストライカー"));
    assert!(!has(vcx, "pressed:ストライカー"));
    assert_eq!(shown(vcx), (true, false), "the specials alone");
    // Another kind as well: both have to fit (アイリ is a supporter, no tank).
    click(vcx, "filter:タンク");
    assert_eq!(shown(vcx), (false, false));
    // The sub-tab the teams are put together in has its own search: no squads' buttons (the
    // place chosen tells the squad), and nothing pressed here narrows it.
    click(vcx, "sub:追加");
    assert!(!has(vcx, "filter:ストライカー") && !has(vcx, "pressed:スペシャル"));
    assert!(has(vcx, "filter:タンク") && !has(vcx, "pressed:タンク"));
    assert_eq!(
        shown(vcx),
        (false, true),
        "the strikers, for a striker's place"
    );
}

#[gpui::test]
fn the_teams_are_put_together_from_the_higher_tiers_first(cx: &mut TestAppContext) {
    let (_, core, vcx) = compose(cx);
    let left_of = |vcx: &mut VisualTestContext, students: &[i64]| -> bool {
        let lefts: Vec<_> = (students.iter())
            .map(|student| {
                let tile = bounds(vcx, &format!("tile:{student}"));
                let tile = tile.unwrap_or_else(|| panic!("{student} is not offered"));
                (tile.top(), tile.left())
            })
            .collect();
        lefts.windows(2).all(|pair| pair[0] < pair[1])
    };
    // By name: アイリ（バンド）, アオバ, then アカネ.
    assert!(left_of(vcx, &[AIRI_BAND, 16017, 13000]));
    // ホシノ of the highest tier, アル and アカネ of the next: they come first, by tier and
    // then by name; a special's tier does not show among the strikers.
    set(&core, vcx, |snapshot| {
        snapshot.tiers = vec![
            (10000, Tier::A),
            (10005, Tier::S),
            (13000, Tier::A),
            (AIRI, Tier::S),
        ];
    });
    assert!(left_of(vcx, &[10005, 13000, 10000, AIRI_BAND, 16017]));
    assert!(!has(vcx, &format!("tile:{AIRI}")));
    // Among the specials, the special of a tier first.
    click(vcx, "place:4");
    assert!(
        left_of(vcx, &[AIRI, 20019]),
        "アイリ, then アカネ（バニーガール）"
    );
    // One barred is offered like anybody (a formation told of, and an opponent's, hold whom
    // they hold): アル keeps the place her tier gives her, and アオバ, of no tier, hers by
    // name before アカリ.
    click(vcx, "place:0");
    set(&core, vcx, |snapshot| snapshot.barred = vec![10000, 16017]);
    assert!(left_of(
        vcx,
        &[10005, 13000, 10000, AIRI_BAND, 16017, 13002]
    ));
}

/// トキ, a striker.
const TOKI: i64 = 10062;

#[gpui::test]
fn one_barred_is_put_into_a_formation_and_an_opponents_like_anybody(cx: &mut TestAppContext) {
    let (rio, core, vcx) = compose(cx);
    // An opponent with the front, nothing known behind it.
    set(&core, vcx, |snapshot| {
        let mut unseen = opponent();
        unseen.counsel = Counsel { likely: Vec::new() };
        snapshot.opponent = Some(unseen);
    });
    // トキ of a tier, whom the user cannot send: she stands where her tier puts her, before
    // the first by name.
    core.rank(TOKI, Some(Tier::A)).unwrap();
    core.bar(TOKI, true).unwrap();
    let toki = format!("tile:{TOKI}");
    let at = |vcx: &mut VisualTestContext, tile: &str| {
        let found = bounds(vcx, tile).unwrap_or_else(|| panic!("{tile} is not offered"));
        (found.top(), found.left())
    };
    assert!(at(vcx, &toki) < at(vcx, &format!("tile:{AIRI_BAND}")));
    // The teams but for her, with ミカ and ハスミ（水着） in her places.
    fill(&rio, vcx, 0, ATTACKS[0]);
    fill(&rio, vcx, 6, [13010, 10037, 10124, 10006, 20016, 20053]);
    // A click puts her into a place of the attack, and into one of the defense.
    click(vcx, "place:2");
    click(vcx, &toki);
    click(vcx, "place:8");
    click(vcx, &toki);
    assert_eq!(draft(&rio, vcx).teams(), Some((ATTACKS[1], DEFENSE)));
    // What the reading reports meanwhile changes nothing of it.
    set(&core, vcx, |snapshot| {
        snapshot.status = Status::Watching(None)
    });
    assert_eq!(draft(&rio, vcx).teams(), Some((ATTACKS[1], DEFENSE)));
    // The formation is registered like any other.
    click(vcx, "register");
    assert!(shows(vcx, "登録しました"));
    let kept = kept_registered(&core);
    assert_eq!(kept.len(), 1);
    assert!(kept[0].attack.contains(&TOKI) && kept[0].defense.contains(&TOKI));
    // Against the opponent, the defense with her is offered and chosen as any; the attack
    // with her is the one set aside: that she is barred tells of the user's attacks alone.
    click(vcx, "tab:対戦");
    assert!(has(vcx, "supplied:likely-0-2") && !has(vcx, "out:likely-0-2"));
    click(vcx, "candidate:likely-0");
    assert_eq!(core.chosen(), choice([10037, 10062, 10006], None));
    assert!(has(vcx, "excluded:attack-0-0") && has(vcx, "out:attack-0-0-2"));
    assert!(!has(vcx, "candidate:attack-0-0"));
}

#[gpui::test]
fn the_least_window_shows_the_tiers_the_search_and_three_rows_of_students(cx: &mut TestAppContext) {
    let (_, _, vcx) = tiers(cx);
    let (wide, tall) = super::WINDOW_LEAST;
    vcx.simulate_resize(size(px(wide), px(tall)));
    vcx.run_until_parked();
    // The sub-tabs, every row of the table, the search and every button of it are in sight.
    let mut parts = vec![
        "tier:S",
        "tier:D",
        "tier:NG",
        "search",
        "filter:ストライカー",
    ];
    parts.extend(["sub:追加", "sub:Tier表", "sub:編集"]);
    parts.extend(["filter:スペシャル", "filter:タンク", "filter:TS"]);
    parts.extend([
        "filter:爆発",
        "filter:振動",
        "filter:軽装備",
        "filter:弾力装甲",
    ]);
    for part in parts {
        let found = bounds(vcx, part).unwrap_or_else(|| panic!("{part} is not there"));
        assert!(
            found.left() >= px(0.) && found.right() <= px(wide) && found.bottom() <= px(tall),
            "{part} is cut: {found:?}"
        );
    }
    // All six rows are in sight, none left to scroll to.
    let (table, last) = (
        bounds(vcx, "tiers").expect("the table"),
        bounds(vcx, "tier:NG").expect("the last row"),
    );
    assert!(last.bottom() <= table.bottom(), "the last row is cut");
    // Three rows of students show whole.
    let (students, first) = (
        bounds(vcx, "students").expect("the list"),
        bounds(vcx, &format!("tile:{AIRI}")).expect("the first student"),
    );
    let rows = (students.bottom() - first.top()) / (first.size.height + px(4.));
    assert!((3. ..3.5).contains(&rows), "{rows} rows of students show");
    assert!(students.bottom() <= px(tall));
}

#[gpui::test]
fn a_row_of_the_table_is_as_tall_empty_as_with_a_student(cx: &mut TestAppContext) {
    let (_, _, vcx) = tiers(cx);
    let tall = |vcx: &mut VisualTestContext, row: &str| {
        let row = bounds(vcx, row).expect("the row");
        row.size.height
    };
    // A row has a student's room from the start: nothing moves as one comes into it. The
    // row of those barred as a tier's.
    let list = bounds(vcx, "students").expect("the list").top();
    let (empty, barred) = (tall(vcx, "tier:A"), tall(vcx, "tier:NG"));
    assert_eq!(barred, empty, "every row as tall");
    drag(vcx, &format!("tile:{AIRI_BAND}"), "tier:A");
    assert!(within(vcx, &format!("tiered:{AIRI_BAND}"), "tier:A"));
    drag(vcx, &format!("tile:{AIRI}"), "tier:NG");
    assert!(within(vcx, &format!("barred:{AIRI}"), "tier:NG"));
    assert_eq!(tall(vcx, "tier:A"), empty, "the row keeps its height");
    assert_eq!(tall(vcx, "tier:NG"), empty);
    let now = bounds(vcx, "students").expect("the list").top();
    assert_eq!(now, list, "and the list its place");
}

#[gpui::test]
fn the_window_has_three_tabs_and_the_third_has_its_sub_tabs(cx: &mut TestAppContext) {
    let (_, _, vcx) = open(cx);
    assert!(has(vcx, "tab:対戦") && has(vcx, "tab:履歴") && has(vcx, "tab:編成登録"));
    for gone in ["編成NG", "編成追加", "編成Tier"] {
        assert!(
            !has(vcx, &format!("tab:{gone}")),
            "{gone} is no tab of its own"
        );
    }
    assert!(!has(vcx, "sub:追加"), "the sub-tabs are of the third tab");
    click(vcx, "tab:編成登録");
    // The sub-tabs on one row under the tabs, over what the one open shows: 追加 at first.
    let row = ["sub:追加", "sub:Tier表", "sub:編集"];
    let places = |vcx: &mut VisualTestContext| row.map(|sub| bounds(vcx, sub).expect(sub));
    let [add, table, edit] = places(vcx);
    let tab = bounds(vcx, "tab:編成登録").expect("the tab");
    assert!(tab.bottom() <= add.top(), "under the tabs");
    assert!(add.right() <= table.left() && table.right() <= edit.left());
    assert!(
        add.top() == table.top() && table.top() == edit.top(),
        "on one row"
    );
    let first = bounds(vcx, "place:0").expect("追加 is open");
    assert!(add.bottom() <= first.top(), "over the sub-tab's content");
    // None moves as another is chosen (each as bold chosen or not, as Aoi's).
    click(vcx, "sub:Tier表");
    assert!(has(vcx, "tier:S") && !has(vcx, "place:0"));
    assert_eq!(places(vcx), [add, table, edit]);
    click(vcx, "sub:編集");
    assert!(shows(vcx, "「追加」で登録した編成が、ここに並びます。"));
    assert_eq!(places(vcx), [add, table, edit]);
    // The sub-tab open stays open while another tab shows.
    click(vcx, "tab:履歴");
    assert!(!has(vcx, "sub:編集"));
    click(vcx, "tab:編成登録");
    assert!(shows(vcx, "「追加」で登録した編成が、ここに並びます。"));
}

#[gpui::test]
fn a_student_is_barred_in_the_table_whatever_her_tier(cx: &mut TestAppContext) {
    let (_, core, vcx) = tiers(cx);
    let (tile, tiered, barred) = (
        format!("tile:{AIRI_BAND}"),
        format!("tiered:{AIRI_BAND}"),
        format!("barred:{AIRI_BAND}"),
    );
    // From the list into the row of those barred (編成NG): she stands there and no longer
    // in the list.
    drag(vcx, &tile, "tier:NG");
    assert!(within(vcx, &barred, "tier:NG") && !has(vcx, &tile));
    assert_eq!(kept_barred(&core), [AIRI_BAND], "kept");
    assert!(kept_tiers(&core).is_empty());
    // From there into a tier: of the tier and still barred, a tile in each row.
    drag(vcx, &barred, "tier:B");
    assert!(within(vcx, &tiered, "tier:B") && within(vcx, &barred, "tier:NG"));
    assert_eq!(kept_barred(&core), [AIRI_BAND]);
    assert_eq!(kept_tiers(&core), [(AIRI_BAND, Tier::B)]);
    // To another tier: her tier alone changes. What the reading reports meanwhile changes
    // nothing of the rows.
    drag(vcx, &tiered, "tier:S");
    set(&core, vcx, |snapshot| {
        snapshot.status = Status::Watching(None)
    });
    assert!(within(vcx, &tiered, "tier:S") && within(vcx, &barred, "tier:NG"));
    assert_eq!(kept_tiers(&core), [(AIRI_BAND, Tier::S)]);
    // Her tier's tile back onto the list: out of the tiers and still barred, so not in the
    // list.
    drag(vcx, &tiered, "students");
    assert!(!has(vcx, &tiered) && !has(vcx, &tile));
    assert!(within(vcx, &barred, "tier:NG"));
    assert!(kept_tiers(&core).is_empty());
    assert_eq!(kept_barred(&core), [AIRI_BAND]);
    // Another, of a tier, dragged into the row of those barred keeps her tier.
    let (other, other_barred) = (format!("tiered:{AIRI}"), format!("barred:{AIRI}"));
    drag(vcx, &format!("tile:{AIRI}"), "tier:A");
    drag(vcx, &other, "tier:NG");
    assert!(within(vcx, &other, "tier:A") && within(vcx, &other_barred, "tier:NG"));
    assert_eq!(kept_tiers(&core), [(AIRI, Tier::A)]);
    assert_eq!(kept_barred(&core), [AIRI_BAND, AIRI]);
    // Her tile among those barred back onto the list: let back in, her tier kept.
    drag(vcx, &other_barred, "students");
    assert!(!has(vcx, &other_barred) && within(vcx, &other, "tier:A"));
    assert_eq!(kept_barred(&core), [AIRI_BAND]);
    assert_eq!(kept_tiers(&core), [(AIRI, Tier::A)]);
    // The first, in no tier, let back in: in the list again.
    drag(vcx, &barred, "students");
    assert!(has(vcx, &tile) && !has(vcx, &barred));
    assert!(kept_barred(&core).is_empty());
}

#[gpui::test]
fn a_student_barred_by_her_menu_keeps_her_tier_and_stands_in_both_rows(cx: &mut TestAppContext) {
    let (_, core, vcx) = open(cx);
    // ミカ of a tier, in the best attack.
    Store::open(&core.places.database)
        .unwrap()
        .rank(MIKA, Some(Tier::S))
        .unwrap();
    set(&core, vcx, |snapshot| {
        snapshot.tiers = vec![(MIKA, Tier::S)];
        snapshot.opponent = Some(opponent());
    });
    right_click(vcx, "face:attack-0-0-2");
    click(vcx, "menu:編成NG");
    assert_eq!(kept_barred(&core), [MIKA]);
    assert_eq!(kept_tiers(&core), [(MIKA, Tier::S)], "her tier stays");
    click(vcx, "tab:編成登録");
    click(vcx, "sub:Tier表");
    let (tiered, barred) = (format!("tiered:{MIKA}"), format!("barred:{MIKA}"));
    assert!(within(vcx, &tiered, "tier:S") && within(vcx, &barred, "tier:NG"));
    // Her menu lets her back in: out of the row of those barred, in her tier as before.
    click(vcx, "tab:対戦");
    right_click(vcx, "out:attack-0-1-2");
    click(vcx, "menu:編成NGを外す");
    click(vcx, "tab:編成登録");
    assert!(within(vcx, &tiered, "tier:S") && !has(vcx, &barred));
    assert!(kept_barred(&core).is_empty());
    assert_eq!(kept_tiers(&core), [(MIKA, Tier::S)]);
}

/// What a formation was told with: its record and the page.
fn source(battles: u32, wins: u32, url: &str) -> Source {
    Source {
        reported: Reported::new(battles, wins),
        url: Some(url.to_owned()),
    }
}

const POST: &str = "https://x.com/someone/status/1";

#[gpui::test]
fn a_formation_is_registered_with_the_record_and_the_page_it_was_told_with(
    cx: &mut TestAppContext,
) {
    let (rio, core, vcx) = compose(cx);
    set(&core, vcx, |snapshot| {
        let mut unseen = opponent();
        unseen.counsel = Counsel { likely: Vec::new() };
        snapshot.opponent = Some(unseen);
    });
    fill(&rio, vcx, 0, ATTACKS[0]);
    fill(&rio, vcx, 6, DEFENSE);
    tell(&rio, vcx, ["20", "18", POST]);
    click(vcx, "register");
    assert!(shows(vcx, "登録しました"));
    let kept = kept_registered(&core);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].source, source(20, 18, POST), "kept with the teams");
    // The record was that attack's own; the page stays for the next one told of on it.
    assert_eq!(told(&rio, vcx), ["", "", POST].map(str::to_owned));
    assert_eq!(draft(&rio, vcx).teams(), Some((ATTACKS[0], DEFENSE)));
    // Against the opponent with that front, the attack says what it was told with.
    click(vcx, "tab:対戦");
    assert!(within(vcx, "text:紹介 20戦18勝", "registered:attack-0-0"));
}

#[gpui::test]
fn nothing_is_registered_while_the_record_or_the_page_cannot_be(cx: &mut TestAppContext) {
    let (rio, core, vcx) = compose(cx);
    fill(&rio, vcx, 0, ATTACKS[0]);
    fill(&rio, vcx, 6, DEFENSE);
    let pressed = |vcx: &mut VisualTestContext, core: &Core| {
        click(vcx, "register");
        // No word of blame: the button does nothing (on a hover it says what is missing).
        assert!(!has(vcx, "keep-error") && !has(vcx, "toast"));
        kept_registered(core).len()
    };
    // Half a record, more wins than battles, no battle; a page that is no address of the web.
    for half in [
        ["20", "", ""],
        ["", "18", ""],
        ["18", "20", ""],
        ["0", "0", ""],
    ] {
        tell(&rio, vcx, half);
        assert_eq!(pressed(vcx, &core), 0, "{half:?}");
    }
    for no_page in ["x.com/someone", "file:///C:/Windows/notepad.exe"] {
        tell(&rio, vcx, ["", "", no_page]);
        assert_eq!(pressed(vcx, &core), 0, "{no_page}");
    }
    // What is no digit cannot be entered into the record's fields; digits of full width are.
    tell(&rio, vcx, ["2a0戦", "１８", ""]);
    assert_eq!(told(&rio, vcx), ["20", "18", ""].map(str::to_owned));
    click(vcx, "register");
    assert!(shows(vcx, "登録しました"));
    assert_eq!(
        kept_registered(&core)[0].source.reported,
        Reported::new(20, 18)
    );
    assert_eq!(kept_registered(&core)[0].source.url, None);
}

/// Registers two formations against `DEFENSE` in the records, as an agent would (`mcp`):
/// the first with a record and a page, the second (with トキ) with nothing told. Their
/// numbers.
fn two_registered(core: &Core) -> (i64, i64) {
    let records = Store::open(&core.places.database).unwrap();
    let first = records.register(ATTACKS[0], DEFENSE, &source(20, 18, POST));
    let second = records.register(ATTACKS[1], DEFENSE, &Source::default());
    (first.unwrap().0, second.unwrap().0)
}

/// The window with the sub-tab open the registrations are listed in (編成登録の「編集」),
/// two formations registered (`two_registered`).
fn editing(
    cx: &mut TestAppContext,
) -> (Entity<Rio>, Arc<Core>, (i64, i64), &mut VisualTestContext) {
    let (rio, core, vcx) = open(cx);
    let registered = two_registered(&core);
    click(vcx, "tab:編成登録");
    click(vcx, "sub:編集");
    (rio, core, registered, vcx)
}

#[gpui::test]
fn the_registrations_are_listed_with_what_they_were_told_with(cx: &mut TestAppContext) {
    let (_, core, (first, second), vcx) = editing(cx);
    // The one registered last first; each with its two teams, the attack over the defense.
    let row = |vcx: &mut VisualTestContext, id: i64, part: &str| {
        let selector = format!("registered:{id}{part}");
        bounds(vcx, &selector).unwrap_or_else(|| panic!("{selector} is not there"))
    };
    assert!(row(vcx, second, "").top() < row(vcx, first, "").top());
    assert!(row(vcx, first, ":attack").bottom() <= row(vcx, first, ":defense").top());
    assert!(within(
        vcx,
        &format!("registered:{first}:defense"),
        &format!("registered:{first}")
    ));
    // The record it was introduced with, and the page: a press opens it in the browser.
    assert!(shows(vcx, "紹介 20戦18勝"));
    assert!(has(vcx, &format!("page:{first}")) && !has(vcx, &format!("page:{second}")));
    assert_eq!(vcx.opened_url(), None);
    // The page is as wide as its words: a press in the room beside them opens nothing.
    let (page, button) = (
        bounds(vcx, &format!("page:{first}")).expect("the page"),
        bounds(vcx, &format!("edit-{first}")).expect("the button"),
    );
    assert!(page.right() < button.left() - px(40.), "{page:?}");
    let beside = gpui::point(button.left() - px(20.), page.center().y);
    vcx.simulate_click(beside, Modifiers::none());
    vcx.run_until_parked();
    assert_eq!(vcx.opened_url(), None);
    click(vcx, &format!("page:{first}"));
    assert_eq!(vcx.opened_url().as_deref(), Some(POST));
    // What other hands wrote into the records as a page is shown, and opens nothing.
    let other = "file:///C:/Windows/notepad.exe";
    let records = rusqlite::Connection::open(&core.places.database).unwrap();
    let written = records.execute(
        "UPDATE registered SET url = ?1 WHERE id = ?2",
        (other, first),
    );
    assert_eq!(written.unwrap(), 1);
    click(vcx, "sub:追加");
    click(vcx, "sub:編集");
    click(vcx, &format!("page:{first}"));
    assert_eq!(
        vcx.opened_url().as_deref(),
        Some(POST),
        "nothing new was opened"
    );
}

#[gpui::test]
fn what_an_agent_registered_shows_as_the_list_opens_again(cx: &mut TestAppContext) {
    let (_, core, vcx) = open(cx);
    click(vcx, "tab:編成登録");
    click(vcx, "sub:編集");
    assert!(shows(vcx, "「追加」で登録した編成が、ここに並びます。"));
    // Registered meanwhile, by another process: nothing tells the window.
    let (first, second) = two_registered(&core);
    assert!(!has(vcx, &format!("registered:{first}")));
    // The sub-tab opened again, or its tab.
    click(vcx, "sub:Tier表");
    click(vcx, "sub:編集");
    assert!(has(vcx, &format!("registered:{first}")) && has(vcx, &format!("registered:{second}")));
    Store::open(&core.places.database)
        .unwrap()
        .forget(second)
        .unwrap();
    click(vcx, "tab:履歴");
    click(vcx, "tab:編成登録");
    assert!(has(vcx, &format!("registered:{first}")) && !has(vcx, &format!("registered:{second}")));
}

#[gpui::test]
fn a_registration_is_changed_in_the_places_and_what_they_held_comes_back(cx: &mut TestAppContext) {
    let (rio, core, (first, second), vcx) = editing(cx);
    // A formation half put together, and a page typed.
    click(vcx, "sub:追加");
    click(vcx, &format!("tile:{AIRI_BAND}"));
    tell(&rio, vcx, ["", "", "https://example.com/next"]);
    let before = (draft(&rio, vcx), told(&rio, vcx));
    // 編集 takes the registration into the places and the fields; the button saves the change.
    click(vcx, "sub:編集");
    click(vcx, &format!("edit-{first}"));
    assert!(
        has(vcx, "place:0") && !has(vcx, &format!("registered:{first}")),
        "in 追加"
    );
    // (The records keep a team's specials the smaller id first.)
    let attack = [10005, 10037, 10059, 10006, 20008, 20016];
    assert_eq!(draft(&rio, vcx).teams(), Some((attack, DEFENSE)));
    assert_eq!(told(&rio, vcx), ["20", "18", POST].map(str::to_owned));
    assert!(
        shows(vcx, "変更を保存") && !shows(vcx, "編成登録"),
        "the button says so"
    );
    assert!(has(vcx, "cancel"));
    // Another record and another fourth striker: under the same number.
    tell(&rio, vcx, ["10", "7", POST]);
    click(vcx, "place:3");
    click(vcx, &format!("tile:{AIRI_BAND}"));
    click(vcx, "register");
    assert!(shows(vcx, "変更しました"));
    let kept = kept_registered(&core);
    assert_eq!(kept.len(), 2, "no new registration");
    let changed = kept
        .iter()
        .find(|one| one.id == first)
        .expect("the same number");
    assert_eq!(changed.attack[3], AIRI_BAND);
    assert_eq!(changed.source, source(10, 7, POST));
    // The list shows again, with the change; and 追加 holds what it held before.
    assert!(has(vcx, &format!("registered:{first}")) && shows(vcx, "紹介 10戦7勝"));
    click(vcx, "sub:追加");
    assert_eq!((draft(&rio, vcx), told(&rio, vcx)), before);
    assert!(
        shows(vcx, "編成登録") && !has(vcx, "cancel"),
        "a new formation again"
    );

    // Given up: nothing changes, and what was there comes back as well.
    click(vcx, "sub:編集");
    click(vcx, &format!("edit-{second}"));
    tell(&rio, vcx, ["3", "3", ""]);
    click(vcx, "cancel");
    assert!(has(vcx, &format!("registered:{second}")), "the list again");
    assert_eq!(kept_registered(&core), kept);
    click(vcx, "sub:追加");
    assert_eq!((draft(&rio, vcx), told(&rio, vcx)), before);

    // Into the teams of the other registration: the button saves nothing and blames nobody
    // (it says why on a hover), and the registration stays to be changed.
    click(vcx, "sub:編集");
    click(vcx, &format!("edit-{second}"));
    fill(&rio, vcx, 0, changed.attack);
    vcx.executor()
        .advance_clock(std::time::Duration::from_secs(4));
    vcx.run_until_parked();
    assert!(!has(vcx, "toast"), "the toasts before are gone");
    click(vcx, "register");
    assert!(!has(vcx, "toast"), "no word of blame after a press");
    assert_eq!(kept_registered(&core), kept);
    assert!(has(vcx, "cancel"), "still being changed");
    // With another fourth striker it is no other's teams: it is saved.
    click(vcx, "place:3");
    click(vcx, "tile:16017");
    click(vcx, "register");
    assert!(shows(vcx, "変更しました"));
}

#[gpui::test]
fn a_registration_taken_away_is_put_back_from_the_toast(cx: &mut TestAppContext) {
    let (_, core, (first, second), vcx) = editing(cx);
    let was = kept_registered(&core);
    click(vcx, &format!("delete-{first}"));
    assert!(!has(vcx, &format!("registered:{first}")) && has(vcx, &format!("registered:{second}")));
    assert_eq!(kept_registered(&core).len(), 1, "gone from the records");
    // A slip of the hand: the toast puts it back as it was.
    assert!(shows(vcx, "削除しました") && has(vcx, "undo"));
    click(vcx, "undo");
    assert!(has(vcx, &format!("registered:{first}")) && shows(vcx, "元に戻しました"));
    assert!(!has(vcx, "undo"));
    assert_eq!(
        kept_registered(&core),
        was,
        "its number, its record and its page"
    );
    // Left alone, the toast goes with its button: the registration stays away.
    click(vcx, &format!("delete-{second}"));
    assert!(has(vcx, "undo"));
    vcx.executor()
        .advance_clock(std::time::Duration::from_secs(9));
    vcx.run_until_parked();
    assert!(!has(vcx, "undo") && !shows(vcx, "削除しました"));
    assert_eq!(kept_registered(&core).len(), 1);
    assert!(!has(vcx, &format!("registered:{second}")));
}

#[gpui::test]
fn the_one_being_changed_taken_away_leaves_nothing_to_change(cx: &mut TestAppContext) {
    let (rio, core, (first, _), vcx) = editing(cx);
    click(vcx, &format!("edit-{first}"));
    assert!(has(vcx, "cancel"));
    // Taken away from the list while its change is not saved.
    click(vcx, "sub:編集");
    click(vcx, &format!("delete-{first}"));
    click(vcx, "sub:追加");
    assert!(
        !has(vcx, "cancel") && shows(vcx, "編成登録"),
        "a new formation"
    );
    assert_eq!(
        draft(&rio, vcx),
        Draft::new(),
        "what the places held before"
    );
    assert_eq!(kept_registered(&core).len(), 1);
}

#[gpui::test]
fn an_attack_registered_is_offered_beside_the_five_with_what_was_told_of_it(
    cx: &mut TestAppContext,
) {
    let (_, core, vcx) = open(cx);
    set(&core, vcx, |snapshot| {
        let mut opponent = opponent();
        // Six attacks that won, each with another first striker, and after them one the
        // user registered: fought once and lost, told to win eighteen of twenty.
        let tried = |first: i64, registered: bool| Tried {
            attack: [first, 10037, 10059, 10006, 20016, 20008],
            wins: u32::from(!registered),
            battles: 1,
            registered,
            reported: registered.then(|| Reported::new(20, 18)).flatten(),
        };
        let firsts = [10005, 10000, 10045, 13000, 13002, 16017];
        let mut attacks: Vec<Tried> = firsts.map(|first| tried(first, false)).into();
        attacks.push(tried(AIRI_BAND, true));
        opponent.counsel.likely[0].attacks = attacks;
        snapshot.opponent = Some(opponent);
    });
    // Five of the others, and the registered one after them: never dropped.
    assert!(has(vcx, "candidate:attack-0-5") && !has(vcx, "candidate:attack-0-6"));
    assert!(has(vcx, "registered:attack-0-5"));
    assert!(within(vcx, "text:紹介 20戦18勝", "candidate:attack-0-5"));
    assert!(
        within(vcx, "text:1戦0勝", "registered:attack-0-5"),
        "its own battle too"
    );
    assert!(!has(vcx, "registered:attack-0-0"));
    click(vcx, "candidate:attack-0-5");
    let attack = [AIRI_BAND, 10037, 10059, 10006, 20016, 20008];
    assert_eq!(
        core.chosen(),
        choice(BEHIND[0], Some(attack)),
        "to choose like any other"
    );
}

#[gpui::test]
fn the_least_window_shows_a_registration_whole(cx: &mut TestAppContext) {
    let (_, _, (first, _), vcx) = editing(cx);
    let (wide, tall) = super::WINDOW_LEAST;
    vcx.simulate_resize(size(px(wide), px(tall)));
    vcx.run_until_parked();
    // The teams, the record, the page and the buttons of a row are within the width.
    let parts = [
        format!("registered:{first}"),
        format!("registered:{first}:attack"),
        format!("registered:{first}:defense"),
        "text:紹介 20戦18勝".to_owned(),
        format!("page:{first}"),
        format!("edit-{first}"),
        format!("delete-{first}"),
    ];
    for part in &parts {
        let found = bounds(vcx, part).unwrap_or_else(|| panic!("{part} is not there"));
        assert!(
            found.left() >= px(0.) && found.right() <= px(wide),
            "{part} is cut: {found:?}"
        );
    }
    let (record, button) = (
        bounds(vcx, "text:紹介 20戦18勝").unwrap(),
        bounds(vcx, &format!("edit-{first}")).unwrap(),
    );
    assert!(
        record.right() <= button.left(),
        "the record beside the buttons"
    );
    let teams = bounds(vcx, &format!("registered:{first}:attack")).unwrap();
    assert!(teams.right() <= record.left(), "and beside the teams");
}
