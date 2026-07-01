# コードブロック

[← 目次](./00-index.md)

各言語ラベル付きフェンスドコードブロックのシンタックスハイライトを確認します。

## Rust

```rust
fn greet(name: &str) -> String {
    format!("Hello, {name}!")
}

fn main() {
    println!("{}", greet("bmd"));
}
```

## Python

```python
def fibonacci(n: int) -> list[int]:
    a, b = 0, 1
    result = []
    for _ in range(n):
        result.append(a)
        a, b = b, a + b
    return result
```

## JavaScript

```javascript
const sum = (items) => items.reduce((a, b) => a + b, 0);
console.log(sum([1, 2, 3, 4]));
```

## JSON

```json
{
  "name": "bmd",
  "features": ["markdown", "mermaid", "vim-keys"]
}
```

## Shell

```bash
#!/usr/bin/env bash
set -euo pipefail
bmd docs/func-check/00-index.md
```

## 言語ラベルなし

```
plain text block
no syntax highlighting expected
```

## 確認項目

- [ ] 言語ラベルがコードブロック上部に表示される
- [ ] Rust / Python / JS などでキーワード・文字列が着色される
- [ ] 言語ラベルなしブロックも表示される（無着色またはデフォルト）
