---
version: alpha
name: Sumi / bluearchive-rio
description: ゲームの横に置いて、戦術対抗戦の相手の予測と記録を見る小さな窓のデザイン。
colors:
  primary: "#1a5fa8"
  accent: "#1a5fa8"
  surface: "#faf6ef"
  on-surface: "#3a2f28"
  muted: "#6f6257"
  border: "#e3d9c9"
  danger: "#9c2b1d"
  danger-subtle: "#f9e9e4"
  wash-base: "#f6efe0"
typography:
  body:
    fontFamily: OS の UI フォント
    fontSize: 16px
    fontWeight: 400
    lineHeight: 1.6
  body-sm:
    fontFamily: OS の UI フォント
    fontSize: 14px
    fontWeight: 400
    lineHeight: 1.5
  caption:
    fontFamily: OS の UI フォント
    fontSize: 12px
    fontWeight: 400
    lineHeight: 1.4
rounded:
  sm: 6px
  full: 9999px
spacing:
  sp-1: 4px
  sp-2: 8px
  sp-3: 12px
  sp-4: 16px
components:
  app-header:
    backgroundColor: "{colors.wash-base}"
    textColor: "{colors.on-surface}"
    height: 40px
  status-badge:
    textColor: "{colors.muted}"
    typography: "{typography.body-sm}"
    rounded: "{rounded.full}"
  error-banner:
    backgroundColor: "{colors.danger-subtle}"
    textColor: "{colors.danger}"
    typography: "{typography.body-sm}"
    rounded: "{rounded.sm}"
    padding: 12px
---

# bluearchive-rio

## Overview

ゲームの横に置き、戦術対抗戦で選んだ相手の防衛の予測とおすすめの攻撃、登録した戦闘を見る窓。
操作はタブの切り替えと読み取りの一時中断だけで、主役は生徒のアイコンで並べた編成である。
ゲームの画面の上に重ねる印（選ぶ生徒の枠など）は、この窓とは別のものとして扱う。

姉妹アプリの Aoi（bluearchive-aoi）と同じ作りにする（2026-11-17 の user の依頼。窓は GPUI、
配色と部品は Aoi の DESIGN.md、2026-11-17 参照）。派生後はこの文書を正とし、Aoi の更新は差分を見て取り込む。

## Colors

暗色は Sumi、明色は Kinari とし、既定は OS の明暗設定に従う。設定ページで OS に従うか明暗のどちらかを選べ、
選択は次回の起動にも残す。色は `src/ui/theme.rs` の一箇所に置く。

| 役割 | Kinari | Sumi |
|---|---|---|
| surface | #faf6ef | #191919 |
| on-surface | #3a2f28 | #e6e6e6 |
| muted | #6f6257 | #9a9a9a |
| border | #e3d9c9 | #333333 |
| accent / primary | #1a5fa8 | #5cb3ff |
| danger | #9c2b1d | #ff6b6b |
| danger-subtle | #f9e9e4 | #3a1a1a |
| wash-base | #f6efe0 | #232323 |
| ok | #1d6b35 | #6fcf8a |

- アクセントの用途は、読めている状態のランプと、選択中のタブの下線だけとする。面は塗らない。
- フォーカスに輪郭は描かない。枠のある部品は枠の色だけを変え、枠の無いものは何も変えない。
- 状態は色だけに頼らない。勝敗は文字で書き、ok の色はその文字に使う。予測した生徒は破線の枠にする。
  失敗は danger の文字で原因を書く。
- ゲームが描いたプレイヤーの名前（名前の絵）は、テーマの on-surface の色で出す。

## Typography

書体は OS の UI フォント（Windows は Yu Gothic UI）。本文は body（16px）、見出し・状態バッジ・名前は
body-sm（14px。見出しは 600）、編成の位置・日時・役割は caption（12px、muted）。数は等幅数字にする。

## Layout

- 上端は全幅・高さ 40px の 1 行とし、左からタブ、右端に状態バッジと設定のアイコンボタン（☰。名前を accessible name に持たせる）を置く。
  アプリ名はウィンドウのタイトルバーにあるので出さない。上端の行は wash-base と 1px の下境界を使う。
- タブはどれも同じ太さにし、選択中は下線と文字色で示す（太さを変えると後ろのタブが動く）。
- 状態バッジは押すと読み取りを一時中断し、もう一度押すと再開する。ランプは取り込み元（ゲームのウィンドウ）の
  アイコンとし、読めている画面の名前はゲームと同じ言葉で書く。説明はホバーで示す。
- 設定はメニューでなく、タブの本文の場所に出すページにする。設定のボタンで開き、もう一度押すか、タブを選ぶか、
  Escape で閉じる。項目は見出し（body-sm・600、下に 1px の border）ごとのグループにまとめる。
- 本文は 1 列で、縦にスクロールする。件数が増える一覧は、見えている行だけを描く。
- 編成は、ゲームと同じ並び（ストライカー 4 人、少し離してスペシャル 2 人）で 1 行に置き、折り返さない。
  窓が狭ければアイコンを小さくして 1 行を保つ。生徒の名前はアイコンのホバーと accessible name に持たせ、常設しない。
- 一覧の行では、窓の幅で伸び縮みするのは名前の欄だけとし、編成のアイコンは右に固定の幅で並べる。
- 窓は最小の大きさを持ち、それより狭くならない。開く位置と大きさは前回のものを覚える。
  開くときにゲームからフォーカスを奪わない。

## Shapes

角の半径は 6px（アイコン、バナー、ボタン）。状態バッジだけ full。
