//! Rio's command line.
//!   bluearchive-rio                       watches the game and keeps the battle records shown
//!   bluearchive-rio read <png>...         keeps the records of screenshots
//!   bluearchive-rio list                  the battles kept, the latest first
//!   bluearchive-rio name <player> <name>  names a player
//!   bluearchive-rio roster                the students held, as Aoi wrote them

use std::{collections::HashMap, error::Error, path::PathBuf};

use bluearchive_rio::{
    aoi,
    history::{Reader, Team},
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
/// (A1 to A4 and D1 to D4 the strikers from the left, SP the specials) with the damage.
fn describe(battle: &Kept, names: &HashMap<i64, String>) -> String {
    let player = |player: &Player| match &player.name {
        Some(name) => format!("{name}(#{})", player.id),
        None => format!("プレイヤー#{}", player.id),
    };
    let team = |team: &Team, side: char| {
        let fighter = |slot: usize| {
            let fighter = team[slot];
            let place = if slot < 4 {
                format!("{side}{}", slot + 1)
            } else {
                "SP".to_owned()
            };
            let name = names
                .get(&fighter.student)
                .cloned()
                .unwrap_or_else(|| format!("生徒#{}", fighter.student));
            format!("{place} {name} {}", fighter.damage)
        };
        let of = |slots: std::ops::Range<usize>| slots.map(fighter).collect::<Vec<_>>().join(" / ");
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

    use bluearchive_rio::{
        history::Fighter,
        store::{Kept, Player},
    };

    use super::describe;

    #[test]
    fn a_battle_is_shown_with_both_teams_in_the_players_order() {
        let team = |fighters: [(i64, u32); 6]| {
            fighters.map(|(student, damage)| Fighter { student, damage })
        };
        let battle = Kept {
            id: 3,
            seen_at: "2026-11-17 03:10:00".to_owned(),
            attacker: Player {
                id: 1,
                name: Some("シャーレの先生".to_owned()),
            },
            defender: Player { id: 2, name: None },
            attacker_won: true,
            attack: team([
                (1, 60812),
                (2, 156_315),
                (3, 720_028),
                (4, 516_569),
                (5, 543_051),
                (6, 0),
            ]),
            defense: team([
                (7, 162_341),
                (2, 121_292),
                (8, 194_176),
                (9, 296_470),
                (5, 573_064),
                (99, 0),
            ]),
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
  攻撃 シャーレの先生(#1): A1 ホシノ 60812 / A2 マリナ 156315 / A3 ヒビキ（応援団） 720028 / A4 ハスミ（水着） 516569 | SP イロハ 543051 / SP ミチル（ドレス） 0
  防衛 プレイヤー#2: D1 アツコ 162341 / D2 マリナ 121292 / D3 トキ 194176 / D4 イオリ 296470 | SP イロハ 573064 / SP 生徒#99 0"
        );
    }
}
