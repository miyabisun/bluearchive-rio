//! Rio's command line.
//!   bluearchive-rio                       watches the game: keeps the battle records shown,
//!                                         and marks over it what the records counsel
//!   bluearchive-rio read <png>...         keeps the records of screenshots
//!   bluearchive-rio predict <png>...      what the records counsel against a screenshot's opponent
//!   bluearchive-rio list                  the battles kept, the latest first
//!   bluearchive-rio name <player> <name>  names a player
//!   bluearchive-rio roster                the students held, as Aoi wrote them

use std::{collections::HashMap, error::Error, ops::Range, path::PathBuf};

use bluearchive_rio::{
    aoi,
    history::Reader,
    opponent::{self, Front},
    store::{Counsel, Kept, Player, Store},
    students,
};

const USAGE: &str = "使い方:
  bluearchive-rio                         ゲームを見て、戦闘履歴を登録し、編成をゲームの上に案内する
  bluearchive-rio read <png>...           スクリーンショットの戦闘履歴を登録する
  bluearchive-rio predict <png>...        スクリーンショットの対戦相手の、防衛の予測とおすすめの攻撃
  bluearchive-rio list                    登録した戦闘を新しい順に表示する
  bluearchive-rio name <プレイヤー番号> <名前>  プレイヤーに名前を付ける
  bluearchive-rio roster                  Aoi が書き出した手持ちの生徒を表示する";

/// Where Rio keeps its database: `RIO_DATA_DIR`, or the user's application data
/// (`%APPDATA%\io.github.miyabisun.bluearchive-rio`, beside Aoi's).
fn database() -> Result<PathBuf, Box<dyn Error>> {
    let dir = match std::env::var_os("RIO_DATA_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => dirs::data_dir()
            .ok_or("保存先のフォルダが分かりません")?
            .join("io.github.miyabisun.bluearchive-rio"),
    };
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("rio.db"))
}

/// A team in the players' terms: A1 to A4 or D1 to D4 the strikers from the left, SP the
/// specials.
fn team(team: &[i64; 6], side: char, names: &HashMap<i64, String>) -> String {
    let student = |slot: usize| {
        let place = if slot < 4 {
            format!("{side}{}", slot + 1)
        } else {
            "SP".to_owned()
        };
        let name = names
            .get(&team[slot])
            .cloned()
            .unwrap_or_else(|| format!("生徒#{}", team[slot]));
        format!("{place} {name}")
    };
    let of = |slots: Range<usize>| slots.map(student).collect::<Vec<_>>().join(" / ");
    format!("{} | {}", of(0..4), of(4..6))
}

/// A battle as the list shows it: the result, then each side's team.
fn describe(battle: &Kept, names: &HashMap<i64, String>) -> String {
    let player = |player: &Option<Player>| match player {
        None => "匿名".to_owned(),
        Some(Player {
            id,
            name: Some(name),
        }) => format!("{name}(#{id})"),
        Some(Player { id, name: None }) => format!("プレイヤー#{id}"),
    };
    let team = |students: &[i64; 6], side: char| team(students, side, names);
    format!(
        "#{} {} {}\n  攻撃 {}: {}\n  防衛 {}: {}",
        battle.id,
        battle.seen_at,
        if battle.attacker_won {
            "攻撃側の勝ち"
        } else {
            "防衛側の勝ち"
        },
        player(&battle.attacker),
        team(&battle.attack, 'A'),
        player(&battle.defender),
        team(&battle.defense, 'D'),
    )
}

/// What the records counsel against an opponent's front: the opponent's own last defense
/// behind it, the strikers seen behind the same first striker and specials (the most seen
/// first), and the attack that won most against the defense expected.
fn foretell(front: &Front, counsel: &Counsel, names: &HashMap<i64, String>) -> String {
    let name = |student: &i64| {
        names
            .get(student)
            .cloned()
            .unwrap_or_else(|| format!("生徒#{student}"))
    };
    let behind = |rest: &[i64; 3]| {
        let [second, third, fourth] = rest.each_ref().map(name);
        format!("D2 {second} / D3 {third} / D4 {fourth}")
    };
    let mut told = format!(
        "対戦相手の防衛: D1 {} | SP {} / SP {}",
        name(&front.first),
        name(&front.specials[0]),
        name(&front.specials[1])
    );
    if let Some(last) = &counsel.last {
        told.push_str(&format!("\n  この相手の最後の防衛: {}", behind(last)));
    }
    if counsel.seen.is_empty() {
        told.push_str("\n  同じ D1 とスペシャルの防衛は、まだ登録がありません");
    }
    for defense in &counsel.seen {
        told.push_str(&format!(
            "\n  {}回 {} (最後に登録 {})",
            defense.battles,
            behind(&defense.rest),
            defense.last_seen
        ));
    }
    match (&counsel.attack, counsel.behind()) {
        (Some(tried), _) => told.push_str(&format!(
            "\n  おすすめの攻撃 ({}戦{}勝): {}",
            tried.battles,
            tried.wins,
            team(&tried.attack, 'A', names)
        )),
        (None, Some(rest)) => told.push_str(&format!(
            "\n  {} の防衛に勝った攻撃は、まだ登録がありません",
            behind(&rest)
        )),
        (None, None) => {}
    }
    told
}

/// The students held, as Aoi wrote them; `None` (with the reason told) when its file does
/// not read: the counsel then takes any team, held or not.
fn held() -> Option<Vec<aoi::Held>> {
    let read = || -> Result<Vec<aoi::Held>, Box<dyn Error>> {
        let path = aoi::latest().ok_or("ドキュメントのフォルダが分かりません")?;
        let yaml = std::fs::read_to_string(&path)
            .map_err(|error| format!("{} を読めません: {error}", path.display()))?;
        Ok(aoi::roster(&yaml)?)
    };
    read()
        .inspect_err(|error| {
            eprintln!("Aoi の名簿を読めないので、手持ちにない生徒を含む攻撃も案内します ({error})");
        })
        .ok()
}

/// The ids of the students held.
fn ids(held: &Option<Vec<aoi::Held>>) -> Option<Vec<i64>> {
    let held = held.as_ref()?;
    Some(held.iter().filter_map(|student| student.id).collect())
}

fn names() -> HashMap<i64, String> {
    students()
        .into_iter()
        .map(|student| (student.id, student.name))
        .collect()
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        [] => watch(),
        ["read", paths @ ..] if !paths.is_empty() => {
            let (reader, store) = (Reader::new(), Store::open(&database()?)?);
            for path in paths {
                let outcome = match reader.read(&image::open(path)?.to_rgb8()) {
                    None => "戦闘履歴を読めませんでした",
                    Some(battle) if store.save(&battle)? => "登録しました",
                    Some(_) => "登録済みです",
                };
                println!("{path}: {outcome}");
            }
            Ok(())
        }
        ["predict", paths @ ..] if !paths.is_empty() => {
            let (reader, store, names) = (Reader::new(), Store::open(&database()?)?, names());
            let held = ids(&held());
            for path in paths {
                let screen = image::open(path)?.to_rgb8();
                match reader.front(&screen) {
                    None => println!("{path}: 対戦相手の画面を読めませんでした"),
                    Some(front) => {
                        let name = opponent::name(&screen);
                        let counsel = store.counsel(&front, name.as_ref(), held.as_deref())?;
                        println!("{path}: {}", foretell(&front, &counsel, &names));
                    }
                }
            }
            Ok(())
        }
        ["list"] => {
            let names = names();
            for battle in Store::open(&database()?)?.battles()? {
                println!("{}", describe(&battle, &names));
            }
            Ok(())
        }
        ["name", player, name] => {
            let player: i64 = player.parse().map_err(|_| USAGE)?;
            if Store::open(&database()?)?.name(player, name)? {
                println!("プレイヤー#{player} を「{name}」にしました");
                Ok(())
            } else {
                Err(format!("プレイヤー#{player} はいません").into())
            }
        }
        ["roster"] => {
            let path = aoi::latest().ok_or("ドキュメントのフォルダが分かりません")?;
            let yaml = std::fs::read_to_string(&path)
                .map_err(|error| format!("{} を読めません: {error}", path.display()))?;
            let held = aoi::roster(&yaml)?;
            for student in &held {
                let shown = |value: Option<u8>| value.map_or("?".to_owned(), |v| v.to_string());
                println!(
                    "{} Lv.{} ★{}",
                    student.name,
                    shown(student.level),
                    shown(student.rarity)
                );
            }
            println!("手持ちの生徒 {}人 ({})", held.len(), path.display());
            Ok(())
        }
        _ => Err(USAGE.into()),
    }
}

/// Watches the game's window: keeps every battle record that stands still on it, and marks
/// over the opponent's dialog and the formation dialog what the records counsel.
#[cfg(windows)]
fn watch() -> Result<(), Box<dyn Error>> {
    use std::{
        sync::mpsc::RecvTimeoutError,
        time::{Duration, Instant},
    };

    use bluearchive_rio::{
        capture,
        guide::{self, Rank, Visit},
        history::{Plate, Steady},
        overlay::Overlay,
    };

    let (reader, store, names) = (Reader::new(), Store::open(&database()?)?, names());
    let held = held();
    let roster = ids(&held);
    let ranks: HashMap<i64, Rank> = held
        .iter()
        .flatten()
        .filter_map(|student| {
            let rank = Rank {
                level: student.level?,
                rarity: student.rarity?,
                bond: student.bond?,
            };
            Some((student.id?, rank))
        })
        .collect();
    let mut overlay = Overlay::new();
    if overlay.is_none() {
        eprintln!("ゲームの上に重ねるウィンドウを作れませんでした。案内は文字だけで出します。");
    }
    let (mut steady, mut told, mut visit) = (Steady::new(), Steady::new(), Visit::default());
    // The opponent whose dialog was open last: the formation dialog shows the front again,
    // without the name.
    let mut met: Option<(Front, Option<Plate>)> = None;
    let mut counselled: Option<((Front, Option<Plate>), Counsel)> = None;
    println!(
        "戦闘履歴を開くと登録し、対戦相手を選ぶと防衛の編成を予測して、編成の画面で選ぶ生徒を案内します。終わるときは Ctrl+C を押してください。"
    );
    let frames = capture::frames(|message| println!("{message}"));
    loop {
        if let Some(overlay) = &overlay {
            overlay.pump();
        }
        let shot = match frames.recv_timeout(Duration::from_secs(1)) {
            Ok(shot) => shot,
            // The game's window is gone or hidden: nothing to lie over.
            Err(RecvTimeoutError::Timeout) => {
                if let Some(overlay) = &mut overlay {
                    overlay.hide();
                }
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
        };
        let (frame, now) = (&shot.image, Instant::now());
        if let Some(battle) = steady.see(reader.read(frame), now) {
            if store.save(battle)? {
                // The battle just kept is the latest; what was counselled may change with it.
                counselled = None;
                if let Some(kept) = store.battles()?.first() {
                    println!("登録しました\n{}", describe(kept, &names));
                }
            } else {
                println!("登録済みの戦闘です");
            }
        }

        let (dialog, forming) = (reader.front(frame), reader.formation(frame));
        if let Some(front) = dialog {
            met = Some((front, opponent::name(frame)));
        }
        let mut marks = Vec::new();
        let front = dialog.or(forming.as_ref().map(|shown| shown.front));
        let opponent = front.map(|front| {
            let name = met.as_ref().filter(|(met, _)| *met == front);
            (front, name.and_then(|(_, name)| name.clone()))
        });
        if let Some(opponent) = &opponent {
            if counselled.as_ref().is_none_or(|(of, _)| of != opponent) {
                let (front, name) = opponent;
                let counsel = store.counsel(front, name.as_ref(), roster.as_deref())?;
                counselled = Some((opponent.clone(), counsel));
            }
            if let Some((_, counsel)) = &counselled {
                marks = match &forming {
                    Some(shown) => guide::for_formation(
                        shown,
                        counsel.behind(),
                        counsel.attack.as_ref().map(|tried| tried.attack),
                        visit.enter(&shown.placed, now),
                        &ranks,
                    ),
                    None => counsel.behind().map_or_else(Vec::new, guide::for_opponent),
                };
            }
        }
        if let (Some((front, _)), Some((_, counsel))) = (told.see(opponent, now), &counselled) {
            println!("{}", foretell(front, counsel, &names));
        }
        if let Some(overlay) = &mut overlay {
            match shot.origin {
                Some(origin) if shot.in_front => overlay.show(&marks, origin, frame.dimensions()),
                _ => overlay.hide(),
            }
        }
    }
}

#[cfg(not(windows))]
fn watch() -> Result<(), Box<dyn Error>> {
    Err("ゲームのウィンドウを見られるのは Windows だけです。read でスクリーンショットを登録できます。".into())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use bluearchive_rio::{
        opponent::Front,
        store::{Counsel, Kept, Player, Seen, Tried},
    };

    use super::{describe, foretell};

    fn names() -> HashMap<i64, String> {
        [
            (1, "ホシノ"),
            (2, "マリナ"),
            (3, "ヒビキ（応援団）"),
            (4, "ハスミ（水着）"),
            (5, "イロハ"),
            (6, "ミチル（ドレス）"),
            (7, "アツコ"),
            (8, "トキ"),
            (9, "イオリ"),
        ]
        .into_iter()
        .map(|(id, name)| (id, name.to_owned()))
        .collect()
    }

    #[test]
    fn the_defenses_seen_behind_a_front_are_told_in_their_order() {
        let front = Front {
            first: 7,
            specials: [5, 6],
        };
        let seen = [
            Seen {
                rest: [2, 8, 9],
                battles: 2,
                last_seen: "2026-11-17 13:14:04".to_owned(),
            },
            Seen {
                rest: [2, 9, 99],
                battles: 1,
                last_seen: "2026-11-16 21:00:00".to_owned(),
            },
        ];
        let mut counsel = Counsel {
            seen: seen.to_vec(),
            last: None,
            attack: None,
        };
        assert_eq!(
            foretell(&front, &counsel, &names()),
            "対戦相手の防衛: D1 アツコ | SP イロハ / SP ミチル（ドレス）
  2回 D2 マリナ / D3 トキ / D4 イオリ (最後に登録 2026-11-17 13:14:04)
  1回 D2 マリナ / D3 イオリ / D4 生徒#99 (最後に登録 2026-11-16 21:00:00)
  D2 マリナ / D3 トキ / D4 イオリ の防衛に勝った攻撃は、まだ登録がありません"
        );
        // The opponent's own last defense is the one expected, and an attack won against it.
        counsel.last = Some([2, 9, 99]);
        counsel.attack = Some(Tried {
            attack: [1, 2, 3, 4, 5, 6],
            wins: 2,
            battles: 3,
        });
        assert_eq!(
            foretell(&front, &counsel, &names()),
            "対戦相手の防衛: D1 アツコ | SP イロハ / SP ミチル（ドレス）
  この相手の最後の防衛: D2 マリナ / D3 イオリ / D4 生徒#99
  2回 D2 マリナ / D3 トキ / D4 イオリ (最後に登録 2026-11-17 13:14:04)
  1回 D2 マリナ / D3 イオリ / D4 生徒#99 (最後に登録 2026-11-16 21:00:00)
  おすすめの攻撃 (3戦2勝): A1 ホシノ / A2 マリナ / A3 ヒビキ（応援団） / A4 ハスミ（水着） | SP イロハ / SP ミチル（ドレス）"
        );
        let none = Counsel {
            seen: Vec::new(),
            last: None,
            attack: None,
        };
        assert_eq!(
            foretell(&front, &none, &names()),
            "対戦相手の防衛: D1 アツコ | SP イロハ / SP ミチル（ドレス）
  同じ D1 とスペシャルの防衛は、まだ登録がありません"
        );
    }

    #[test]
    fn a_battle_is_shown_with_both_teams_in_the_players_order() {
        let battle = Kept {
            id: 3,
            seen_at: "2026-11-17 03:10:00".to_owned(),
            attacker: Some(Player {
                id: 1,
                name: Some("シャーレの先生".to_owned()),
            }),
            defender: Some(Player { id: 2, name: None }),
            attacker_won: true,
            attack: [1, 2, 3, 4, 5, 6],
            defense: [7, 2, 8, 9, 5, 99],
        };
        let names = names();
        assert_eq!(
            describe(&battle, &names),
            "#3 2026-11-17 03:10:00 攻撃側の勝ち
  攻撃 シャーレの先生(#1): A1 ホシノ / A2 マリナ / A3 ヒビキ（応援団） / A4 ハスミ（水着） | SP イロハ / SP ミチル（ドレス）
  防衛 プレイヤー#2: D1 アツコ / D2 マリナ / D3 トキ / D4 イオリ | SP イロハ / SP 生徒#99"
        );
        let hidden = Kept {
            defender: None,
            ..battle
        };
        assert!(
            describe(&hidden, &names).contains("\n  防衛 匿名: D1 アツコ"),
            "an anonymous player has no number"
        );
    }
}
