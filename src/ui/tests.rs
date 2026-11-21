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

use super::{Rio, Tab, Updates, theme::Theme};
use crate::{
    Squad,
    app::{Core, Places, Snapshot, UpdateCheck},
    draft::Draft,
    opponent::Front,
    settings::{Settings, ThemeChoice},
    store::{Counsel, Fought, Ground, Kept, Likely, Player, Store, Tried},
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

/// ミカ and トキ, the third strikers of the two attacks.
const MIKA: i64 = 10059;
const TOKI: i64 = 10062;

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
fn the_barred_tab_lists_the_students_barred_to_let_them_back_in(cx: &mut TestAppContext) {
    let (_, core, vcx) = open(cx);
    click(vcx, "tab:編成NG");
    assert!(
        shows(
            vcx,
            "攻撃の候補の生徒を右クリックすると、編成NGにできます。"
        ),
        "with nobody barred, how to"
    );
    // Barred before this start: the reading reports whom the records keep.
    let records = Store::open(&core.places.database).unwrap();
    records.bar(TOKI, true).unwrap();
    records.bar(MIKA, true).unwrap();
    set(&core, vcx, |snapshot| {
        snapshot.barred = records.barred().unwrap();
        snapshot.opponent = Some(opponent());
    });
    // No candidate need show them: the tab always does, by name.
    assert!(has(vcx, "barred:10062") && shows(vcx, "トキ"));
    assert!(has(vcx, "barred:10059") && shows(vcx, "ミカ"));
    // The button of a row lets its student back in.
    click(vcx, "unbar-10059");
    assert!(!has(vcx, "barred:10059") && has(vcx, "barred:10062"));
    assert_eq!(kept_barred(&core), [TOKI]);
    // So does the student's menu, as on a candidate.
    right_click(vcx, "barred:10062");
    click(vcx, "menu:編成NGを外す");
    assert!(!has(vcx, "barred:10062"));
    assert!(kept_barred(&core).is_empty());
    click(vcx, "tab:対戦");
    assert!(has(vcx, "candidate:attack-0-1") && !has(vcx, "excluded:attack-0-1"));
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

/// The window with the tab open the formations are put together in (編成追加).
fn compose(cx: &mut TestAppContext) -> (Entity<Rio>, Arc<Core>, &mut VisualTestContext) {
    let (rio, core, vcx) = open(cx);
    click(vcx, "tab:編成追加");
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
    let field = rio.read_with(vcx, |rio, _| rio.finder(rio.tab).query.clone());
    field.update_in(vcx, |field, window, cx| {
        let all = 0..field.text().encode_utf16().count();
        field.replace_text_in_range(Some(all), typed, window, cx);
    });
    vcx.run_until_parked();
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
    // Under them the search and the students, by name.
    let (field, students) = (
        bounds(vcx, "search").expect("the search field"),
        bounds(vcx, "students").expect("the list"),
    );
    assert!(last.bottom() < field.top() && field.bottom() <= students.top());
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
}

#[gpui::test]
fn the_least_window_shows_the_teams_the_search_and_three_rows_of_students(cx: &mut TestAppContext) {
    let (_, _, vcx) = compose(cx);
    let (wide, tall) = super::WINDOW_LEAST;
    vcx.simulate_resize(size(px(wide), px(tall)));
    vcx.run_until_parked();
    // The button beside the teams, the search and every button of it are within the width.
    let mut parts = vec!["place:5", "place:11", "register", "search"];
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
    // (The tiers' tab takes more over its students and decides how low the window is made:
    // here a little more than three rows show.)
    let rows = (students.bottom() - first.top()) / (first.size.height + px(4.));
    // The tiers' tab asks for the taller window: here more than three rows show.
    assert!(rows >= 3., "{rows} rows of students show");
    assert!(students.bottom() <= px(tall));
}

/// The window with the tab open the students are sorted into tiers in (編成Tier).
fn tiers(cx: &mut TestAppContext) -> (Entity<Rio>, Arc<Core>, &mut VisualTestContext) {
    let (rio, core, vcx) = open(cx);
    click(vcx, "tab:編成Tier");
    (rio, core, vcx)
}

/// The students in tiers, as the records keep them.
fn kept_tiers(core: &Core) -> Vec<(i64, Tier)> {
    let records = Store::open(&core.places.database).unwrap();
    records.tiers().unwrap()
}

#[gpui::test]
fn the_tier_tab_has_the_five_tiers_over_the_students_of_both_squads(cx: &mut TestAppContext) {
    let (_, _, vcx) = tiers(cx);
    // S to D from the top, each with its letter.
    let mut top = px(0.);
    for name in ["S", "A", "B", "C", "D"] {
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
    // The tab the teams are put together in has its own search: no squads' buttons (the
    // place chosen tells the squad), and nothing pressed here narrows it.
    click(vcx, "tab:編成追加");
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
}

#[gpui::test]
fn the_least_window_shows_the_tiers_the_search_and_three_rows_of_students(cx: &mut TestAppContext) {
    let (_, _, vcx) = tiers(cx);
    let (wide, tall) = super::WINDOW_LEAST;
    vcx.simulate_resize(size(px(wide), px(tall)));
    vcx.run_until_parked();
    // Every tier, the search and every button of it are in sight.
    let mut parts = vec!["tier:S", "tier:D", "search", "filter:ストライカー"];
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
    // All five rows are in sight, none left to scroll to.
    let (table, last) = (
        bounds(vcx, "tiers").expect("the tiers"),
        bounds(vcx, "tier:D").expect("the last tier"),
    );
    assert!(last.bottom() <= table.bottom(), "the last tier is cut");
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
fn a_tiers_row_is_as_tall_empty_as_with_a_student(cx: &mut TestAppContext) {
    let (_, _, vcx) = tiers(cx);
    let tall = |vcx: &mut VisualTestContext| {
        let row = bounds(vcx, "tier:A").expect("the row");
        row.size.height
    };
    // A row has a student's room from the start: nothing moves as one comes into it.
    let (empty, list) = (tall(vcx), bounds(vcx, "students").expect("the list").top());
    drag(vcx, &format!("tile:{AIRI_BAND}"), "tier:A");
    assert!(within(vcx, &format!("tiered:{AIRI_BAND}"), "tier:A"));
    assert_eq!(tall(vcx), empty, "the row keeps its height");
    let now = bounds(vcx, "students").expect("the list").top();
    assert_eq!(now, list, "and the list its place");
}
