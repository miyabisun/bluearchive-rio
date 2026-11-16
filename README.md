# Rio（リオ）

Rio はブルーアーカイブの戦術対抗戦（6 人対 6 人の PvP）の画面を横から見て、
対戦の記録を残すアプリです。[Aoi](https://github.com/miyabisun/bluearchive-aoi) の姉妹アプリで、
画面の見方は Aoi と同じです。

いまできるのは、戦闘履歴の画面を読んでデータベースに登録し、あとから一覧で見ることです。
対戦相手の防衛編成の予測と、攻撃する編成の並び順の案内は、これから作ります。

- 画面を見るだけで、ゲームは操作しません（クリックやキー入力を送りません）。
- 読み取った内容はこのパソコンの中だけに保存します。インターネットにはつなぎません。

## 必要なもの

- Windows 11 と、PC 版ブルーアーカイブ（日本版）
- ゲームのウィンドウの横幅が 1500 ピクセル以上あること
- [Rust](https://www.rust-lang.org/ja/tools/install)（配布用の実行ファイルはまだありません。自分でビルドします）

## ビルド

このリポジトリを手元に置き、そのフォルダでコマンドプロンプトか PowerShell を開いて、次を実行します。

```sh
cargo build --release
```

`target\release\bluearchive-rio.exe` ができます。下の説明の `bluearchive-rio` は、このファイルのことです。

## 使い方

1. ブルーアーカイブを起動します。
2. コマンドプロンプトか PowerShell で `target\release\bluearchive-rio.exe` を実行します。
   「ゲームのウィンドウを見ています。」と出れば準備完了です。ゲームとどちらが先でも構いません。
3. ゲームで「戦術対抗戦」→「対戦履歴」と進み、履歴を 1 件開きます。
4. 「戦闘履歴」の画面が 1 秒ほど止まると、Rio がその戦闘を登録して内容を表示します。
   同じ戦闘をもう一度開いても、登録は 1 回だけです。
5. 終わるときは、Rio を実行している画面で Ctrl+C を押します。

| コマンド | すること |
| --- | --- |
| `bluearchive-rio` | ゲームを見て、表示された戦闘履歴を登録する |
| `bluearchive-rio read <png>...` | スクリーンショットの戦闘履歴を登録する |
| `bluearchive-rio list` | 登録した戦闘を新しい順に表示する |
| `bluearchive-rio name <プレイヤー番号> <名前>` | プレイヤーに名前を付ける |
| `bluearchive-rio roster` | Aoi が書き出した手持ちの生徒を表示する |

一覧では、攻撃側のストライカーを左から A1〜A4、防衛側を D1〜D4、スペシャルを SP と書きます。

```text
#1 2026-11-17 03:19:17 攻撃側の勝ち
  攻撃 プレイヤー#1: A1 ホシノ 60812 / A2 マリナ 156315 / ... | SP イロハ 543051 / SP ミチル（ドレス） 0
  防衛 プレイヤー#2: D1 アツコ 162341 / D2 マリナ 121292 / ... | SP イロハ 573064 / SP ミチル（ドレス） 0
```

### プレイヤーの名前

Rio はプレイヤーの名前を文字としては読みません。画面に描かれた名前の形でプレイヤーを見分け、
同じ形なら同じプレイヤーとして番号を付けます。一覧に名前を出したいときは、
`bluearchive-rio name 2 ライバル` のように自分で付けます。

### 手持ちの生徒

手持ちの生徒は Aoi が読んだものを使います。Aoi が `ドキュメント\Aoi\aoi.yaml` に書き出す内容を
Rio が読むので、Rio で生徒を読み直す必要はありません。

## 登録されるデータ

データベースは SQLite で、`%APPDATA%\io.github.miyabisun.bluearchive-rio\rio.db` にあります。
環境変数 `RIO_DATA_DIR` にフォルダを指定すると、そこに作ります。

- `battles`: 1 行が 1 戦闘です。`seen_at`（登録した日時）、`attacker`・`defender`（プレイヤー番号）、
  `attacker_won`（攻撃側が勝ったら 1）と、生徒の列 `a1`〜`a4`・`asp1`・`asp2`・`d1`〜`d4`・`dsp1`・`dsp2`
  （SchaleDB の生徒 ID）、それぞれのダメージ（`a1_damage` など）を持ちます。
  スペシャルの 2 人は左右を区別せず、ID の小さい方を `asp1`・`dsp1` に入れます。
- `players`: プレイヤー番号 `id`、画面に描かれた名前の画像 `plate`、付けた名前 `name` を持ちます。

戦闘履歴の画面には日時が出ないので、プレイヤー・編成・ダメージがすべて同じ戦闘を同じ戦闘として扱います。

SQLite を読める道具（`sqlite3` コマンドなど）で、自分で引き出せます。
たとえば、プレイヤー 2 番が防衛したときの編成は次のように出します。

```sh
sqlite3 "%APPDATA%\io.github.miyabisun.bluearchive-rio\rio.db" "SELECT seen_at, attacker_won, d1, d2, d3, d4, dsp1, dsp2 FROM battles WHERE defender = 2"
```

## 読めないとき

Rio は、戦闘履歴のどこか 1 か所でも確実に読めなければ、その戦闘を登録しません。間違った記録を残さないためです。

- ウィンドウの横幅が 1500 ピクセルより狭いと読みません。
- 幅によっては数字を読み切れないことがあります。ウィンドウの大きさを少し変えると読めます。
- 実装されたばかりの生徒は、Rio が持つ生徒の一覧にまだいません。下の「生徒の一覧の更新」で取り込みます。
- 生徒が 6 人そろっていない編成は、まだ読めません。

## 開発

```sh
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

main へ push する前に同じ検査を走らせるには、クローンごとに 1 回 `git config core.hooksPath .githooks` を実行します。

- 生徒の一覧の更新: `powershell -File tools/schaledb/update.ps1` を実行します。
  [SchaleDB](https://schaledb.com/) から日本版の生徒の一覧とアイコンを `data/schaledb` に取り込みます。
- 読み取りの手本の作り直し: `cargo run --example make_refs` を実行します。
  `tests/fixtures/history.png` から、題字・攻撃と防衛の絵・ダメージの数字を `data/` に切り出します。

## ライセンス

プログラムは MIT（[LICENSE](LICENSE)）です。
`data/schaledb/student-icons` の生徒のアイコンと `tests/fixtures` の画面は、ブルーアーカイブの画像です。
これらの権利は NEXON Games と Yostar にあります。
