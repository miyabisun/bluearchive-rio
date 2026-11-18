//! Rio: without arguments, the window (`app::run`); with arguments, a command.
//!   bluearchive-rio                       opens the window: watches the game, keeps the battle
//!                                         records shown, and marks over it what the user chooses
//!   bluearchive-rio read <png>...         keeps the records of screenshots
//!   bluearchive-rio predict <png>...      what the records say against a screenshot's opponent
//!   bluearchive-rio list                  the battles kept, the latest first
//!   bluearchive-rio name <player> <name>  names a player
//!   bluearchive-rio roster                the students held, as Aoi wrote them

// The release build has no console window: a command's words go to the console it was started
// from (`attach_console`).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{collections::HashMap, error::Error, ops::Range, path::PathBuf};

use bluearchive_rio::{
    aoi,
    history::Reader,
    opponent::{self, Front},
    store::{Counsel, Kept, Player, Store},
    students, view,
};

const USAGE: &str = "使い方:
  bluearchive-rio                         ウィンドウを開き、ゲームを見て戦闘履歴を登録し、編成を案内する
  bluearchive-rio read <png>...           スクリーンショットの戦闘履歴を登録する
  bluearchive-rio predict <png>...        スクリーンショットの対戦相手の、防衛の候補と勝った攻撃の候補
  bluearchive-rio list                    登録した戦闘を新しい順に表示する
  bluearchive-rio name <プレイヤー番号> <名前>  プレイヤーに名前を付ける
  bluearchive-rio roster                  Aoi が書き出した手持ちの生徒を表示する";

/// Where Rio keeps its database (see `app::places`), its folder made.
fn database() -> Result<PathBuf, Box<dyn Error>> {
    let database = bluearchive_rio::app::places()?.database;
    if let Some(dir) = database.parent() {
        std::fs::create_dir_all(dir)?;
    }
    Ok(database)
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
            ..
        }) => format!("{name}(#{id})"),
        Some(Player { id, name: None, .. }) => format!("プレイヤー#{id}"),
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

/// What the records say against an opponent's front: the defenses that may stand behind
/// it, the likeliest first, each with why it is likely and the attacks that won against
/// it. Which to take is the user's to say.
fn foretell(front: &Front, counsel: &Counsel, names: &HashMap<i64, String>) -> String {
    let name = |student: &i64| {
        names
            .get(student)
            .cloned()
            .unwrap_or_else(|| format!("生徒#{student}"))
    };
    let mut told = format!(
        "対戦相手の防衛: D1 {} | SP {} / SP {}",
        name(&front.first),
        name(&front.specials[0]),
        name(&front.specials[1])
    );
    if counsel.likely.is_empty() {
        told.push_str("\n  同じ D1 とスペシャルの防衛は、まだ登録がありません");
    }
    for (at, likely) in counsel.likely.iter().enumerate() {
        let [second, third, fourth] = likely.rest.each_ref().map(name);
        told.push_str(&format!(
            "\n  {}. D2 {second} / D3 {third} / D4 {fourth} （{}）",
            at + 1,
            view::basis(likely)
        ));
        if likely.attacks.is_empty() {
            told.push_str("\n     勝った攻撃は、まだ登録がありません");
        }
        for tried in &likely.attacks {
            told.push_str(&format!(
                "\n     {} {}",
                view::fared(tried),
                team(&tried.attack, 'A', names)
            ));
        }
    }
    told
}

/// The ids of the students held, as Aoi wrote them; `None` (with the reason told) when its
/// file does not read: the counsel then takes any team, held or not.
fn held() -> Option<Vec<i64>> {
    let held = aoi::held()
        .inspect_err(|error| {
            eprintln!("Aoi の名簿を読めないので、手持ちにない生徒を含む攻撃も案内します ({error})");
        })
        .ok()?;
    Some(held.iter().filter_map(|student| student.id).collect())
}

fn names() -> HashMap<i64, String> {
    students()
        .into_iter()
        .map(|student| (student.id, student.name))
        .collect()
}

/// Lets a command's words reach the console it was started from: a release build is a
/// window's program, which Windows starts with no console of its own.
#[cfg(all(windows, not(debug_assertions)))]
fn attach_console() {
    use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
    // SAFETY: the call takes a constant; without a console to attach to it does nothing.
    unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
}

#[cfg(not(all(windows, not(debug_assertions))))]
fn attach_console() {}

fn main() {
    if std::env::args().len() <= 1 {
        return bluearchive_rio::app::run();
    }
    attach_console();
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
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
            let held = held();
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
            let held = aoi::held()?;
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use bluearchive_rio::{
        opponent::Front,
        store::{Counsel, Kept, Likely, Player, Tried},
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
    fn the_defenses_likely_behind_a_front_are_told_with_the_attacks_against_each() {
        let front = Front {
            first: 7,
            specials: [5, 6],
        };
        let likely = |rest, theirs, others: (u32, u32), attacks: Vec<Tried>| Likely {
            rest,
            theirs,
            their_last: (theirs > 0).then(|| "2026-11-17 13:14:04".to_owned()),
            others: others.0,
            other_battles: others.1,
            mine: 0,
            attacks,
        };
        let won = Tried {
            attack: [1, 2, 3, 4, 5, 6],
            wins: 2,
            battles: 3,
        };
        let counsel = Counsel {
            likely: vec![
                likely([2, 9, 99], 1, (0, 0), vec![won]),
                likely([2, 8, 9], 0, (2, 3), Vec::new()),
            ],
        };
        assert_eq!(
            foretell(&front, &counsel, &names()),
            "対戦相手の防衛: D1 アツコ | SP イロハ / SP ミチル（ドレス）
  1. D2 マリナ / D3 イオリ / D4 生徒#99 （この相手 1戦（最後 11-17 13:14））
     3戦2勝 A1 ホシノ / A2 マリナ / A3 ヒビキ（応援団） / A4 ハスミ（水着） | SP イロハ / SP ミチル（ドレス）
  2. D2 マリナ / D3 トキ / D4 イオリ （ほか 2人・3戦）
     勝った攻撃は、まだ登録がありません"
        );
        let none = Counsel { likely: Vec::new() };
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
                plate: Vec::new(),
            }),
            defender: Some(Player {
                id: 2,
                name: None,
                plate: Vec::new(),
            }),
            attacker_won: true,
            attack: [1, 2, 3, 4, 5, 6],
            defense: [7, 2, 8, 9, 5, 99],
            fought: None,
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
