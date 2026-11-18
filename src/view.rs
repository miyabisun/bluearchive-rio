//! What the window derives from the snapshot to show: the status badge's words, why a
//! defense is likely, and how an attack fared. Pure, so the tests read them without a
//! window.

use crate::{
    store::{Fought, Ground, Likely, Tried},
    watch::{Screen, Status},
};

/// How the status badge is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// Nothing to read yet.
    Idle,
    /// The game is read.
    Active,
    Paused,
    Danger,
}

/// The status badge: its words, what a hover explains, and its tone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusView {
    pub label: String,
    pub title: String,
    pub tone: Tone,
}

/// The badge for a status. On a screen Rio reads, the badge names the screen as the game
/// does.
pub fn status_view(status: &Status) -> StatusView {
    let (label, title, tone) = match status {
        Status::NoGame => (
            "ゲームを待っています",
            "ブルーアーカイブのウィンドウが開くのを待っています。押すと読み取りを一時中断します。"
                .to_owned(),
            Tone::Idle,
        ),
        // A record says whether it is kept: read and kept look alike in the game.
        Status::Watching(Some(Screen::Record { kept: false })) => (
            "戦闘履歴を読み取り中",
            "この戦闘を読んでいます。画面が 1 秒止まると登録します。\n読み取り中のままのときは、ゲームのウィンドウの大きさを少し変えてください。\n押すと読み取りを一時中断します。"
                .to_owned(),
            Tone::Idle,
        ),
        Status::Watching(Some(Screen::Record { kept: true })) => (
            "戦闘履歴・登録済み",
            "この戦闘は登録済みです。「履歴」タブにあります。\n押すと読み取りを一時中断します。"
                .to_owned(),
            Tone::Active,
        ),
        Status::Watching(Some(Screen::List)) => (
            "戦闘履歴の一覧",
            "一覧の行を読んでいます。ゲームの上の「読み取り済み」「未読」は、その行の戦闘を登録したかどうかです。\n行の「履歴」から開いた戦闘には、行の時刻が付きます。\n押すと読み取りを一時中断します。"
                .to_owned(),
            Tone::Active,
        ),
        Status::Watching(Some(screen @ (Screen::Opponent | Screen::Formation))) => (
            if *screen == Screen::Opponent {
                "対戦相手"
            } else {
                "編成"
            },
            "ゲームのウィンドウを読んでいます。押すと読み取りを一時中断します。".to_owned(),
            Tone::Active,
        ),
        // Any other screen: "reading" there would say a record is still being read.
        Status::Watching(None) => (
            "対象外の画面",
            "この画面は読み取りません。戦闘履歴・対戦相手・編成を開くと読み取ります。\n押すと読み取りを一時中断します。"
                .to_owned(),
            Tone::Idle,
        ),
        Status::Paused => (
            "一時中断",
            "読み取りを止めています。押すと再開します。".to_owned(),
            Tone::Paused,
        ),
        Status::Failed(message) => ("読み取れません", message.clone(), Tone::Danger),
    };
    StatusView {
        label: label.to_owned(),
        title,
        tone,
    }
}

/// Why a defense is likely behind an opponent's front, in the window's words, a part for
/// each who used it: the day the opponent themself last did, the other players who do now
/// (or, if none does, those who did before), the user. Players, never battles: see `Likely`.
pub fn basis(likely: &Likely) -> Vec<String> {
    let mut said = Vec::new();
    if let Some(day) = &likely.their_last {
        // "11-17" of "2026-11-17".
        said.push(format!("本人 {}", day.get(5..).unwrap_or(day)));
    }
    if likely.others > 0 {
        said.push(format!("ほか {}人", likely.others));
    } else if likely.others_before > 0 {
        said.push(format!("ほか {}人（2週間より前）", likely.others_before));
    }
    if likely.mine {
        said.push("自分の防衛".to_owned());
    }
    said
}

/// What the window calls the defenses standing on a ground.
pub fn ground(ground: Ground) -> &'static str {
    match ground {
        Ground::Theirs => "本人が使った編成（過去1か月）",
        Ground::Trend => "流行（過去2週間）",
        Ground::Before => "それより前の記録",
        Ground::Mine => "自分の防衛だけにある編成",
    }
}

/// How an attack fared against the defense: "3戦2勝".
pub fn fared(tried: &Tried) -> String {
    format!("{}戦{}勝", tried.battles, tried.wins)
}

/// When a battle was kept, short: "11-17 13:14" of "2026-11-17 13:14:04".
pub fn kept_at(seen_at: &str) -> &str {
    seen_at.get(5..16).unwrap_or(seen_at)
}

/// When a battle was, as closely as the game's list told it: "11-18 00:10 ごろ" where the
/// row told the minute, "11-17 22時ごろ" where the hour, "11-16 ごろ" where the day.
pub fn fought_at(fought: &Fought) -> String {
    // "2026-11-18 00:10:30": the day, the hour, the minute.
    let about = fought.about.as_str();
    let (Some(day), Some(hour), Some(minute)) =
        (about.get(5..10), about.get(11..13), about.get(11..16))
    else {
        return about.to_owned();
    };
    if fought.within <= MINUTES {
        format!("{day} {minute} ごろ")
    } else if fought.within <= HOURS {
        format!("{day} {hour}時ごろ")
    } else {
        format!("{day} ごろ")
    }
}

/// A time known within a quarter of an hour is told to the minute, one within three hours
/// to the hour: a row tells a minute or an hour, and a list left open a little more.
const MINUTES: i64 = 15 * 60;
const HOURS: i64 = 3 * 3600;

/// Where checking for an update and taking it stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateState {
    Checking,
    Latest { current: String },
    Available { current: String, latest: String },
    Installing { latest: String },
    Failed { error: String },
}

/// Where checking for an update and taking it stands, as the window tells it (Aoi's words).
pub fn update_text(update: &UpdateState) -> String {
    match update {
        UpdateState::Checking => "アップデートを確認しています…".to_owned(),
        UpdateState::Latest { current } => format!("最新のバージョンです（{current}）"),
        UpdateState::Available { current, latest } => {
            format!("{latest} があります（今 {current}）")
        }
        UpdateState::Installing { latest } => format!("{latest} に更新しています…"),
        UpdateState::Failed { error } => format!("アップデートできませんでした: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{Tone, basis, fared, fought_at, kept_at, status_view};
    use crate::{
        store::{Fought, Ground, Likely, Tried},
        watch::{Screen, Status},
    };

    #[test]
    fn the_badge_names_the_screen_read() {
        let view = status_view(&Status::Watching(Some(Screen::Record { kept: false })));
        assert_eq!(
            (view.label.as_str(), view.tone),
            ("戦闘履歴を読み取り中", Tone::Idle),
            "a record not kept yet"
        );
        let view = status_view(&Status::Watching(Some(Screen::Record { kept: true })));
        assert_eq!(
            (view.label.as_str(), view.tone),
            ("戦闘履歴・登録済み", Tone::Active),
            "a record kept"
        );
        let view = status_view(&Status::Watching(Some(Screen::Formation)));
        assert_eq!((view.label.as_str(), view.tone), ("編成", Tone::Active));
        let view = status_view(&Status::Watching(Some(Screen::List)));
        assert_eq!(
            (view.label.as_str(), view.tone),
            ("戦闘履歴の一覧", Tone::Active)
        );
        assert!(
            view.title.contains("読み取り済み"),
            "what the words over the rows say"
        );
        // Any other screen of the game: nothing is being read there (Aoi's words).
        let view = status_view(&Status::Watching(None));
        assert_eq!(
            (view.label.as_str(), view.tone),
            ("対象外の画面", Tone::Idle)
        );
        assert_eq!(status_view(&Status::NoGame).tone, Tone::Idle);
        assert_eq!(status_view(&Status::Paused).label, "一時中断");
        let failed = status_view(&Status::Failed("窓を取り込めません".to_owned()));
        assert_eq!(
            (failed.title.as_str(), failed.tone),
            ("窓を取り込めません", Tone::Danger),
            "the hover says why"
        );
    }

    #[test]
    fn a_likely_defense_says_who_used_it_and_never_how_many_battles() {
        let mut likely = Likely {
            rest: [1, 2, 3],
            ground: Ground::Theirs,
            their_last: Some("2026-11-17".to_owned()),
            others: 0,
            others_before: 0,
            mine: false,
            attacks: Vec::new(),
        };
        assert_eq!(basis(&likely), ["本人 11-17"]);
        (likely.others, likely.others_before, likely.mine) = (3, 1, true);
        assert_eq!(
            basis(&likely),
            ["本人 11-17", "ほか 3人", "自分の防衛"],
            "the players of now, not those of before as well"
        );
        (likely.their_last, likely.others) = (None, 0);
        assert_eq!(basis(&likely), ["ほか 1人（2週間より前）", "自分の防衛"]);
        likely.others_before = 0;
        assert_eq!(basis(&likely), ["自分の防衛"]);
    }

    #[test]
    fn an_attack_is_told_by_its_battles_and_wins() {
        let tried = Tried {
            attack: [1, 2, 3, 4, 5, 6],
            wins: 2,
            battles: 3,
        };
        assert_eq!(fared(&tried), "3戦2勝");
    }

    #[test]
    fn a_battles_time_is_told_as_closely_as_the_list_told_it() {
        let fought = |about: &str, within| Fought {
            about: about.to_owned(),
            within,
        };
        assert_eq!(
            fought_at(&fought("2026-11-18 00:10:30", 60)),
            "11-18 00:10 ごろ",
            "a row that told the minute"
        );
        assert_eq!(
            fought_at(&fought("2026-11-17 22:30:00", 3600)),
            "11-17 22時ごろ",
            "a row that told the hour"
        );
        assert_eq!(
            fought_at(&fought("2026-11-16 12:00:00", 86400)),
            "11-16 ごろ",
            "a row that told the day"
        );
        // A row seen again as its words changed tells more closely than its words.
        assert_eq!(
            fought_at(&fought("2026-11-17 22:41:00", 240)),
            "11-17 22:41 ごろ"
        );
        assert_eq!(fought_at(&fought("soon", 60)), "soon", "another shape");
    }

    #[test]
    fn the_time_kept_is_shortened() {
        assert_eq!(kept_at("2026-11-17 13:14:04"), "11-17 13:14");
        assert_eq!(kept_at("soon"), "soon", "another shape is shown whole");
    }
}
