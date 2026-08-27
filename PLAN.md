# bmd — TUI Markdown Viewer 設計計画

<!-- constrained-by ./Cargo.toml -->
<!-- derived-from #必須要件 -->

## 必須要件

1. 型安全なドメインモデルと遷移を前提とした設計・実装（Kamae Rust スタイル）。
2. vim キーバインドでの操作。
3. マークアップリッチなテキスト表示 + ASCII を使わないネイティブな mermaid レンダリング。
4. 画面サイズによってカラムが適切に wrap されるテーブル。
5. リンク上で特定のキーを押すことで macOS `open` / Linux `xdg-open` によるブラウザ表示。

## 技術選定

| 目的 | クレート | 理由 |
|---|---|---|
| TUI フレームワーク | `ratatui` + `crossterm` | 事実上の標準。immediate-mode でイベント駆動。 |
| Markdown パース | `pulldown-cmark` | CommonMark + テーブル / 脚注 / タスクリスト対応。 |
| reStructuredText パース | `parserst` | RST → 共有 DTO。 |
| AsciiDoc パース | `acdc-parser` | AsciiDoc → 共有 DTO。 |
| シンタックスハイライト | `syntect` | コードブロックをリッチに着色。 |
| mermaid ネイティブレンダリング | `merman` (`raster` feature) | ブラウザ / JS エンジン不要の Rust ネイティブ実装。SVG → PNG 出力。 |
| ターミナル画像表示 | `ratatui-image` | Kitty / iTerm2 / Sixel / ハーフブロックへの自動フォールバック。 |
| 画像ロード | `image` | PNG バイト列を `DynamicImage` に変換。 |
| エラー合成 | `thiserror` | ドメイン・ユースケース・UI 層の型付きエラー。 |
| テキスト幅 | `unicode-width`, `unicode-segmentation` | CJK 対応の幅計算。 |
| 設定 | `toml` + `serde` | `~/.config/bmd/config.toml`。 |
| クリップボード | `arboard` | yank。 |
| GitHub fetch | `ureq` | blob / PR を HTTP で取得。 |

### mermaid 表示戦略

<!-- dagayn: implemented-by src/render/mermaid.rs::RenderedDocument -->

` ```mermaid ` コードブロックを検出 → `merman::render::HeadlessRenderer::render_png_sync` で PNG バイト列を生成 → `image::load_from_memory` → `ratatui_image::Picker::new_protocol` → `ratatui_image::Image` ウィジェットとして描画。

ターミナルが画像プロトコル未対応の場合、`ratatui-image` はハーフブロック（Unicode）フォールバックを使う。これは「ASCII アート」ではない。

## モジュール構成

<!-- constrained-by ./src/lib.rs -->
<!-- dagayn: implemented-by src/lib.rs -->

依存は内側へ向ける。`domain` は `app` / `parse` / `render` / `github` / `config` / `keymap` を import しない。

```text
src/
├── main.rs          # CLI: 引数、stdin、GitHub URL、TUI 初期化
├── lib.rs           # クレート境界
├── domain/          # 値オブジェクト、Document、状態遷移、slug
├── parse/           # Markdown / RST / AsciiDoc → DTO → domain
├── render/          # domain → ratatui widgets（parse は import しない）
├── app/             # イベントループ、合成ルート
├── fs.rs            # DocumentFs の std::fs アダプタ
├── keymap.rs        # Command とキー解決（config を import しない）
├── config.rs        # TOML → Theme + Keymap
├── github/          # url（純粋）/ fetch（HTTP）/ listing / rewrite
├── browser.rs       # open / xdg-open
├── clipboard.rs     # yank アダプタ
└── error.rs         # AppError
```

## 層ルール

<!-- derived-from #モジュール構成 -->

| から | 向いてよい先 |
|---|---|
| `parse` | `domain` |
| `render` | `domain` |
| `keymap` | `domain`, `error` |
| `config` | `keymap`, `render`, `error` |
| `github/url` | なし（純粋） |
| `github` その他 | `domain`, `parse` |
| `fs` | `domain` |
| `app` | `domain`, `parse`, `render`, `keymap`, `config`, `github`, `browser`, `clipboard`, `fs` |
| `main` | `app`, `parse`, `github`, `error` |

禁止: `domain` → 外側、`keymap` → `config`、`render` → `parse`。見出し slug は `domain::slugify_heading` が正本。

## ドメインモデル

<!-- dagayn: implemented-by src/domain/markdown.rs::Document -->
<!-- dagayn: implemented-by src/domain/view.rs::ViewState -->

`Document` のフィールドは `pub(crate)`。構築は `Document::new` のみ（dangling link / mermaid / footnote を検証）。リンクは `Inline::Link(LinkId)` でフラットな `links` を参照する。

### 値オブジェクト・状態

- `LinkUrl`: 空文字列を許さない newtype。
- `TerminalSize`: width/height が 0 でないことを不変条件に持つ。
- `Scroll`: スクロール offset を newtype で包む。
- `ViewState`: `Scroll` + 選択中リンク + `TerminalSize`。遷移は `self` を消費する。
- `NavStack` / `LinkJumpStack`: リンクジャンプ時に prior を固定。ライブな現在位置はスタックの外。
- `MermaidRenderSession` / `ImageRenderSession`: プレビューを `Idle → Queued → Rendering → Ready | Failed` で追跡。

## レンダリングパイプライン

1. `parse` が markup を `ParsedDocument` に落とし、`into_domain` で `Document` にする。
2. `render` は `Document` + `ViewState` を受け取り、スクロールに応じて可視ブロックを描画する。
3. テーブルは独自のカラム幅計算で折り返す。
4. mermaid / 画像は `RenderedDocument` にキャッシュし、スクロール中は描画を止める。
5. 選択中リンクは反転ハイライト。

## テーブル折り返しアルゴリズム

1. 各カラムについて、セル内の最長単語幅を `min_width`、最長セル全体の幅を `ideal_width` とする。
2. 理想幅の合計 + 罫線分が端末幅に収まれば理想幅を採用。
3. 収まらなければ、最小幅をベースに残りの空き幅を `ideal - min` の比率で分配（必要に応じて 1 文字を下限とする）。
4. 決定したカラム幅で各セルを折り返し、行高はその行のセル最大行数。

## vim キーバインド

<!-- constrained-by ./README.md#keybindings -->

キーの正本は [README の Keybindings](./README.md#keybindings)。`keymap` は入力を `Command` に変換する。TOML オーバーライドのパースも `keymap` が所有し、`config` はファイル読み込みだけ行う。

## エラー戦略

- ドメイン層は型付きの小さな error enum を返す。
- ユースケース / UI 層はインフラエラーを `#[from]` で合成した `AppError` に変換。
- `unwrap()` / `expect()` はドメイン・ユースケースコードでは禁止。起動時のターミナル初期化失敗のみ許容。

## 残作業

<!-- derived-from #層ルール -->

合意した層整理は完了。`Document` と `RenderedDocument` は split-borrow のため `App` 上に残している。追加で切るなら `domain/markdown.rs`。
