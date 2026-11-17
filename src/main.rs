//! Rio's command line.
//!   bluearchive-rio                       watches the game and keeps the battle records shown
//!   bluearchive-rio read <png>...         keeps the records of screenshots
//!   bluearchive-rio list                  the battles kept, the latest first
//!   bluearchive-rio name <player> <name>  names a player
//!   bluearchive-rio roster                the students held, as Aoi wrote them

use std::{collections::HashMap, error::Error, path::PathBuf};

use bluearchive_rio::{
    aoi,
    history::Reader,
    store::{Kept, Player, Store},
    students,
};

const USAGE: &str = "使い方:
  bluearchive-rio                         ゲームを見て、表示された戦闘履歴を登録する
  bluearchive-rio read <png>...           スクリーンショットの戦闘履歴を登録する
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

/// A battle as the list shows it: the result, then each side's team in the players' terms
/// (A1 to A4 and D1 to D4 the strikers from the left, SP the specials).
fn describe(battle: &Kept, names: &HashMap<i64, String>) -> String {
    let player = |player: &Option<Player>| match player {
        None => "匿名".to_owned(),
        Some(Player {
            id,
            name: Some(name),
        }) => format!("{name}(#{id})"),
        Some(Player { id, name: None }) => format!("プレイヤー#{id}"),
    };
    let team = |team: &[i64; 6], side: char| {
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
        let of = |slots: std::ops::Range<usize>| slots.map(student).collect::<Vec<_>>().join(" / ");
        format!("{} | {}", of(0..4), of(4..6))
    };
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

/// Watches the game's window and keeps every battle record that stands still on it.
#[cfg(windows)]
fn watch() -> Result<(), Box<dyn Error>> {
    use bluearchive_rio::{capture, history::Steady};

    let (reader, store, names) = (Reader::new(), Store::open(&database()?)?, names());
    let mut steady = Steady::new();
    println!("戦闘履歴を開くと登録します。終わるときは Ctrl+C を押してください。");
    for frame in capture::frames(|message| println!("{message}")) {
        let Some(battle) = steady.see(reader.read(&frame), std::time::Instant::now()) else {
            continue;
        };
        if store.save(battle)? {
            // The battle just kept is the latest.
            if let Some(kept) = store.battles()?.first() {
                println!("登録しました\n{}", describe(kept, &names));
            }
        } else {
            println!("登録済みの戦闘です");
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn watch() -> Result<(), Box<dyn Error>> {
    Err("ゲームのウィンドウを見られるのは Windows だけです。read でスクリーンショットを登録できます。".into())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use bluearchive_rio::store::{Kept, Player};

    use super::describe;

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
        let names: HashMap<i64, String> = [
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
        .collect();
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
