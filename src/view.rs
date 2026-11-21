//! What the window derives from the snapshot to show: the status badge's words, why a
//! defense is likely, and how an attack fared. Pure, so the tests read them without a
//! window.

use crate::{
    source::Reported,
    store::{Fought, Ground, Likely, SHOWN, Tried},
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
    if likely.registered {
        said.push("登録".to_owned());
    }
    said
}

/// What the window calls the defenses standing on a ground.
pub fn ground(ground: Ground) -> &'static str {
    match ground {
        Ground::Theirs => "本人が使った編成（過去1か月）",
        Ground::Registered => "登録した編成",
        Ground::Trend => "流行（過去2週間）",
        Ground::Before => "それより前の記録",
        Ground::Mine => "自分の防衛だけにある編成",
    }
}

/// How an attack fared against the defense, in one piece (see `fared_parts`):
/// "3戦2勝・紹介 20戦18勝".
pub fn fared(tried: &Tried) -> String {
    fared_parts(tried).join("・")
}

/// How an attack fared against the defense, a part for each thing told: the user's own
/// battles ("3戦2勝"), and of one the user registered the record it was introduced with
/// ("紹介 20戦18勝"), or "登録" before the rest where it has none.
pub fn fared_parts(tried: &Tried) -> Vec<String> {
    let mut said = Vec::new();
    if tried.registered && tried.reported.is_none() {
        said.push("登録".to_owned());
    }
    if tried.battles > 0 || !tried.registered {
        said.push(format!("{}戦{}勝", tried.battles, tried.wins));
    }
    if let Some(told) = tried.reported {
        said.push(reported(told));
    }
    said
}

/// A record a formation was introduced with, as the window writes it: "紹介 20戦18勝".
pub fn reported(told: Reported) -> String {
    format!("紹介 {}戦{}勝", told.battles, told.wins)
}

/// Why a student is left out of the attacks the user may take: not among the students Aoi
/// lists as held, or barred by the user (編成NG). Neither: the student is not left out.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Out {
    pub not_held: bool,
    pub barred: bool,
}

impl Out {
    /// Whether the student is left out at all.
    pub fn any(self) -> bool {
        self.not_held || self.barred
    }
}

/// Whether `student` is left out, and why. `held` is Aoi's roster where it reads: without
/// it nobody is known to be missing.
pub fn out(student: i64, held: Option<&[i64]>, barred: &[i64]) -> Out {
    Out {
        not_held: held.is_some_and(|held| !held.contains(&student)),
        barred: barred.contains(&student),
    }
}

/// The attacks as the window offers them, each with whether it is set aside (one of its
/// students is left out): those the user can take first, then those set aside, each kind in
/// the order given. Of a kind, those the user registered are all offered, and `SHOWN` of
/// the others at most: what was registered against the defense is never dropped.
pub fn offered<'a>(
    attacks: &'a [Tried],
    held: Option<&[i64]>,
    barred: &[i64],
) -> Vec<(&'a Tried, bool)> {
    let aside = |tried: &Tried| {
        let mut team = tried.attack.iter();
        team.any(|student| out(*student, held, barred).any())
    };
    let of = |kind: bool| {
        let mut others = 0;
        let of_kind = attacks.iter().filter(move |tried| aside(tried) == kind);
        let told = of_kind.filter(move |tried| {
            others += usize::from(!tried.registered);
            tried.registered || others <= SHOWN
        });
        told.map(move |tried| (tried, kind))
    };
    of(false).chain(of(true)).collect()
}

/// What a hover says of a student in an attack: the name, and under it why the student is
/// left out.
pub fn left_out(name: &str, out: Out) -> String {
    let mut said = name.to_owned();
    for reason in reasons(out) {
        said.push('\n');
        said.push_str(reason);
    }
    said
}

/// Why a student is left out, in the window's words, with what the user can do about it.
pub fn reasons(out: Out) -> Vec<&'static str> {
    let mut said = Vec::new();
    if out.not_held {
        said.push("お迎えしていません（Aoi の生徒一覧にいません）");
    }
    if out.barred {
        said.push("編成NG（右クリックで外せます）");
    }
    said
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
    use super::{
        Out, Tone, basis, fared, fared_parts, fought_at, kept_at, left_out, offered, out, reported,
        status_view,
    };
    use crate::{
        source::Reported,
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
            registered: false,
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
        likely.registered = true;
        assert_eq!(basis(&likely), ["自分の防衛", "登録"], "registered by hand");
    }

    #[test]
    fn a_student_is_left_out_when_not_held_or_barred() {
        let held = [1, 2, 3];
        let fine = Out::default();
        assert_eq!(out(1, Some(&held), &[]), fine, "held and not barred");
        assert!(!fine.any());
        let missing = out(9, Some(&held), &[]);
        assert_eq!((missing.not_held, missing.barred), (true, false));
        assert!(missing.any());
        let barred = out(2, Some(&held), &[2]);
        assert_eq!((barred.not_held, barred.barred), (false, true));
        let both = out(9, Some(&held), &[9]);
        assert_eq!((both.not_held, both.barred), (true, true));
        // Without Aoi's roster nobody is known to be missing: the user's word still counts.
        assert_eq!(out(9, None, &[]), fine);
        assert!(out(9, None, &[9]).barred);
    }

    #[test]
    fn attacks_with_a_student_left_out_stand_under_the_others() {
        let tried = |first: i64| Tried {
            attack: [first, 2, 3, 4, 5, 6],
            wins: 1,
            battles: 1,
            registered: false,
            reported: None,
        };
        // The best first, as the records order them: the first and the third need a student
        // the user does not hold or has barred.
        let attacks = [tried(90), tried(1), tried(80), tried(7)];
        let held = [1, 2, 3, 4, 5, 6, 7, 80];
        let firsts = |offered: Vec<(&Tried, bool)>| -> Vec<(i64, bool)> {
            let told = offered.into_iter();
            told.map(|(tried, aside)| (tried.attack[0], aside))
                .collect()
        };
        assert_eq!(
            firsts(offered(&attacks, Some(&held), &[80])),
            [(1, false), (7, false), (90, true), (80, true)],
            "those the user can take first, each kind in the order given"
        );
        assert_eq!(
            firsts(offered(&attacks, None, &[])),
            [(90, false), (1, false), (80, false), (7, false)],
            "nobody left out: the order given"
        );
        // Five of each kind at most: an attack set aside never takes a usable one's place.
        let many: Vec<Tried> = (10..17).chain(90..97).map(tried).collect();
        let held: Vec<i64> = (1..20).collect();
        let told = firsts(offered(&many, Some(&held), &[]));
        assert_eq!(
            told,
            [10, 11, 12, 13, 14]
                .map(|first| (first, false))
                .into_iter()
                .chain([90, 91, 92, 93, 94].map(|first| (first, true)))
                .collect::<Vec<_>>()
        );
        // Those the user registered are all told beside the five, where they rank: none of
        // them is dropped, and none takes another's place.
        let mut many: Vec<Tried> = (10..19).chain(90..99).map(tried).collect();
        for registered in [1, 8, 9, 17] {
            many[registered].registered = true;
        }
        let told = firsts(offered(&many, Some(&held), &[]));
        assert_eq!(
            told,
            [10, 11, 12, 13, 14, 15, 18]
                .map(|first| (first, false))
                .into_iter()
                .chain([90, 91, 92, 93, 94, 95, 98].map(|first| (first, true)))
                .collect::<Vec<_>>(),
            "11 and 18, 90 and 98 are registered"
        );
    }

    #[test]
    fn a_student_left_out_says_why() {
        let why = |not_held, barred| left_out("トキ", Out { not_held, barred });
        assert_eq!(
            why(false, false),
            "トキ",
            "nothing to say of one who is not"
        );
        assert_eq!(why(false, true), "トキ\n編成NG（右クリックで外せます）");
        assert_eq!(
            why(true, false),
            "トキ\nお迎えしていません（Aoi の生徒一覧にいません）"
        );
        assert_eq!(
            why(true, true),
            "トキ\nお迎えしていません（Aoi の生徒一覧にいません）\n編成NG（右クリックで外せます）"
        );
    }

    #[test]
    fn an_attack_is_told_by_its_battles_and_by_what_was_told_of_it() {
        let mut tried = Tried {
            attack: [1, 2, 3, 4, 5, 6],
            wins: 2,
            battles: 3,
            registered: false,
            reported: None,
        };
        assert_eq!(fared(&tried), "3戦2勝");
        assert_eq!(fared_parts(&tried), ["3戦2勝"]);
        // One the user registered says so, with its battles if it has any.
        tried.registered = true;
        assert_eq!(fared(&tried), "登録・3戦2勝");
        assert_eq!(fared_parts(&tried), ["登録", "3戦2勝"]);
        // The record it was introduced with says it is one told of: after the user's own.
        tried.reported = Reported::new(20, 18);
        assert_eq!(fared(&tried), "3戦2勝・紹介 20戦18勝");
        assert_eq!(fared_parts(&tried), ["3戦2勝", "紹介 20戦18勝"]);
        (tried.wins, tried.battles) = (0, 0);
        assert_eq!(fared(&tried), "紹介 20戦18勝");
        // Neither: registered, and no more is known.
        tried.reported = None;
        assert_eq!(fared(&tried), "登録");
    }

    #[test]
    fn a_record_told_is_written_as_the_battles_are() {
        let told = Reported::new(20, 18).unwrap();
        assert_eq!(reported(told), "紹介 20戦18勝");
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
