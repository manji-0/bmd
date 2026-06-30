# bmd — TUI Markdown Viewer 設計計画

<!-- constrained-by ./Cargo.toml -->
<!-- derived-from #必須要件 -->

## 必須要件

1. 型安全なドメインモデルと遷移を前提とした設計・実装（Kamae Rust スタイル）。
2. vim キーバインドでの操作。
3. マークアップリッチなテキスト表示 + ASCII を使わないネイティブな mermaid レンダリング。
4. 画面サイズによってカラムが適切に wrap されるテーブル。
5. リンク上で特定のキーを押すことで macOS `open` によるブラウザ表示。

## 技術選定

| 目的 | クレート | 理由 |
|---|---|---|
| TUI フレームワーク | `ratatui` + `crossterm` | 事実上の標準。immediate-mode でイベント駆動。 |
| Markdown パース | `pulldown-cmark` | CommonMark + テーブル / 脚注 / タスクリスト対応。 |
| シンタックスハイライト | `syntect` | コードブロックをリッチに着色。 |
| mermaid ネイティブレンダリング | `merman` (`raster` feature) | ブラウザ / JS エンジン不要の Rust ネイティブ実装。SVG → PNG 出力。 |
| ターミナル画像表示 | `ratatui-image` | Kitty / iTerm2 / Sixel / ハーフブロックへの自動フォールバック。 |
| 画像ロード | `image` | PNG バイト列を `DynamicImage` に変換。 |
| エラー合成 | `thiserror` | ドメイン・ユースケース・UI 層の型付きエラー。 |
| テキスト幅 / wrap | `unicode-width`, `textwrap` | CJK 対応の幅計算と単語単位の折り返し。 |

### mermaid 表示戦略

` ```mermaid ` コードブロックを検出 → `merman::render::HeadlessRenderer::render_png_sync` で PNG バイト列を生成 → `image::load_from_memory` → `ratatui_image::Picker::new_protocol` → `ratatui_image::Image` ウィジェットとして描画。

ターミナルが画像プロトコル未対応の場合、`ratatui-image` はハーフブロック（Unicode）フォールバックを使う。これは「ASCII アート」ではない。

## モジュール構成

```text
src/
├── main.rs        # エントリポイント：引数読み込み、初期化、TUI ループ
├── app.rs         # App 状態とイベントハンドラ
├── domain.rs      # ドメインモデル・値オブジェクト・状態遷移
├── error.rs       # 層別エラー型
├── parse.rs       # pulldown-cmark → ドメインモデル変換（mermaid 分離含む）
├── render.rs      # ドメインモデル → ratatui Line/Widget 描画
├── keymap.rs      # vim キー → Command 変換
└── browser.rs     # macOS `open` アダプタ
```

## ドメインモデル

### 主要型

```rust
pub struct Document {
    pub blocks: Vec<Block>,
    pub links: Vec<Link>,
}

pub enum Block {
    Heading(Heading),
    Paragraph(Vec<Inline>),
    CodeBlock(CodeBlock),
    BlockQuote(Vec<Block>),
    List(List),
    Table(Table),
    Mermaid(MermaidDiagram),
    Rule,
}

pub enum Inline {
    Text(String),
    Strong(Vec<Inline>),
    Emphasis(Vec<Inline>),
    Code(String),
    Link(LinkId),
    HardBreak,
    SoftBreak,
}

pub struct Link {
    pub url: LinkUrl,
    pub title: Option<String>,
}
```

### 値オブジェクト・状態

- `LinkUrl`: 空文字列を許さない newtype。
- `TerminalSize`: width/height が 0 でないことを不変条件に持つ。
- `Scroll`: スクロール offset を newtype で包む。
- `ViewState`: `Scroll` + 選択中リンク + `TerminalSize`。

### 状態遷移

`ViewState` は所有権を消費するメソッドで遷移する（Kamae 推奨）。

- `scroll_down(self, n, max_scroll) -> Self`
- `scroll_up(self, n) -> Self`
- `half_page_down(self, max_scroll) -> Self`
- `half_page_up(self) -> Self`
- `jump_to_top(self) -> Self`
- `jump_to_bottom(self, max_scroll) -> Self`
- `resize(self, TerminalSize) -> Self`
- `select_next_link(self, &Document) -> Self`
- `select_prev_link(self, &Document) -> Self`

## レンダリングパイプライン

1. `parse.rs` で Markdown を `Document` に変換。同時にリンクをフラットな `links` ベクタに集約し、`Inline::Link(LinkId)` で参照。
2. `render.rs` は `Document` + `ViewState` を受け取り、スクロールに応じて可視ブロックを `Vec<Line>` / ウィジェットに変換。
3. テーブルは独自のカラム幅計算 + `textwrap` による折り返しを行うカスタムウィジェット。
4. mermaid ブロックは初回描画時（または初期化時）に `RenderedDocument` 内の `HashMap<block_index, Protocol>` へキャッシュ。
5. 選択中リンクは青色 + 下線 + 反転ハイライトで表示。

## テーブル折り返しアルゴリズム

1. 各カラムについて、セル内の最長単語幅を `min_width`、最長セル全体の幅を `ideal_width` とする。
2. 理想幅の合計 + 罫線分が端末幅に収まれば理想幅を採用。
3. 収まらなければ、最小幅をベースに残りの空き幅を `ideal - min` の比率で分配（必要に応じて 1 文字を下限とする）。
4. 決定したカラム幅で各セルを折り返し、行高はその行のセル最大行数。

## vim キーバインド

| キー | 動作 |
|---|---|
| `j` / `↓` | 1 行下へ |
| `k` / `↑` | 1 行上へ |
| `d` / `Ctrl-d` | 半ページ下へ |
| `u` / `Ctrl-u` | 半ページ上へ |
| `g` `g` | 先頭へ |
| `G` | 末尾へ |
| `Tab` / `n` | 次のリンクへ |
| `Shift-Tab` / `N` | 前のリンクへ |
| `o` / `Enter` | 選択中リンクを `open` で開く |
| `q` / `Ctrl-c` | 終了 |

## エラー戦略

- ドメイン層は `Result<T, DomainError>` を返す。
- ユースケース / UI 層はインフラエラーを `#[from]` で合成した `AppError` に変換。
- `unwrap()` / `expect()` はドメイン・ユースケースコードでは禁止。起動時のターミナル初期化失敗のみ許容。

## macOS 固有の考慮

- ブラウザ起動は `std::process::Command::new("open").arg(url)`。
- `ratatui-image` の `Picker::from_query_stdio()` はターミナル画像プロトコルを検出。検出失敗時はハーフブロックにフォールバック。
- iTerm2 / Kitty / WezTerm / Ghostty ではネイティブ画像プロトコルが使用される。

## 実装フェーズ

1. `Cargo.toml` 依存追加とビルド確認。
2. `domain.rs` + `error.rs` 実装。
3. `parse.rs`: pulldown-cmark → `Document`。
4. `render.rs`: ブロック → `Line` + テーブルカスタム描画。
5. `browser.rs`: `open` ラッパー。
6. `keymap.rs`: 入力 → `Command`。
7. `app.rs`: 状態 + イベントループ + mermaid 画像キャッシュ。
8. `main.rs`: 初期化と実行。
9. `cargo build / clippy` 確認。
