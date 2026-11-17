//! The window as a user drives it, over what a test reports in place of the game (made as
//! Aoi's window tests).

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use futures::channel::mpsc;
use gpui::{Bounds, Entity, Modifiers, Pixels, TestAppContext, VisualTestContext, px, size};

use super::{Rio, Updates, theme::Theme};
use crate::{
    app::{Core, Places, Snapshot, UpdateCheck},
    opponent::Front,
    settings::{Settings, ThemeChoice},
    store::{Counsel, Fought, Kept, Player, Seen, Tried},
    watch::{Opponent, Screen, Status},
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

/// ユウカ in front with イロハ and ミチル（ドレス）; behind her the opponent last stood
/// マリナ, トキ and イオリ, another defense was seen once, and an attack of ホシノ, マリナ,
/// ミカ and イオリ with イロハ and アコ won two of three.
fn opponent() -> Opponent {
    Opponent {
        front: Front {
            first: 13010,
            specials: [20016, 20053],
        },
        name: None,
        counsel: Counsel {
            seen: vec![
                Seen {
                    rest: [10037, 10062, 10006],
                    battles: 2,
                    last_seen: "2026-11-17 13:14:04".to_owned(),
                },
                Seen {
                    rest: [10037, 10006, 10124],
                    battles: 1,
                    last_seen: "2026-11-16 21:00:00".to_owned(),
                },
            ],
            last: Some([10037, 10062, 10006]),
            attack: Some(Tried {
                attack: [10005, 10037, 10059, 10006, 20016, 20008],
                wins: 2,
                battles: 3,
            }),
        },
    }
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
            "対戦相手を選ぶと、防衛の編成の予測と、おすすめの攻撃がここに出ます。"
        ),
        "what the match tab waits for"
    );
    assert!(!shows(vcx, "おすすめの攻撃"));
}

#[gpui::test]
fn an_opponent_shows_the_defense_expected_and_the_attack_counselled(cx: &mut TestAppContext) {
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
    assert!(bounds(vcx, "defense:トキ").is_some(), "D3 as expected");
    assert!(shows(vcx, "この相手が最後に登録された防衛です。"));
    assert!(shows(vcx, "おすすめの攻撃"));
    assert!(shows(vcx, "3戦2勝"));
    let (first, special) = (
        bounds(vcx, "attack:ホシノ").expect("A1"),
        bounds(vcx, "attack:アコ").expect("the second special"),
    );
    assert!(first.left() < special.left(), "the team in its order");
    assert_eq!(first.top(), special.top(), "on one row at this width");
    assert!(shows(vcx, "ほかに見た防衛"));
    assert!(
        shows(vcx, "1回"),
        "the defense seen once, and not the expected one again"
    );
    assert!(!shows(vcx, "2回"));
}

#[gpui::test]
fn a_front_never_seen_says_so(cx: &mut TestAppContext) {
    let (_, core, vcx) = open(cx);
    set(&core, vcx, |snapshot| {
        let mut unseen = opponent();
        unseen.counsel = Counsel {
            seen: Vec::new(),
            last: None,
            attack: None,
        };
        snapshot.opponent = Some(unseen);
    });
    assert!(shows(
        vcx,
        "同じ D1 とスペシャルの防衛は、まだ登録がありません。"
    ));
    assert!(bounds(vcx, "defense:?").is_some(), "nobody is expected");
    assert!(!shows(vcx, "おすすめの攻撃"), "no attack without a defense");
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
