#!/usr/bin/env python3
"""Split src/app.rs into src/app/ per dagayn hub boundaries (see transcript)."""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "src" / "app.rs"
OUT = ROOT / "src" / "app"

# Line ranges are 1-based inclusive from original app.rs
RANGES = {
    "layout.rs": (1, 69),
    "scroll.rs": (70, 179),
    "navigation.rs": (180, 246),
    "search.rs": (247, 330),
    "input.rs": (331, 463),
    "draw.rs": (464, 526),
    "mod.rs": (527, 655),  # struct + new + run only; tests extracted separately
    "tests.rs": (656, 704),
}


def extract(path: Path, start: int, end: int) -> str:
    lines = path.read_text().splitlines(keepends=True)
    return "".join(lines[start - 1 : end])


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    for name, (start, end) in RANGES.items():
        body = extract(SRC, start, end)
        if name == "mod.rs":
            # Strip nested tests module; mod.rs declares #[cfg(test)] mod tests;
            body = body.replace("#[cfg(test)]\nmod tests {\n", "")
            if body.rstrip().endswith("}"):
                body = body.rstrip()[:-1] + "\n"
        if name == "tests.rs":
            body = body.replace("#[cfg(test)]\nmod tests {\n", "").rstrip()
            if body.endswith("}"):
                body = body[:-1]
        (OUT / name).write_text(body)
    print(f"Wrote {len(RANGES)} files under {OUT}")
    print("Manual follow-up: pub(crate) on cross-module impl methods, free fns in scroll.rs")


if __name__ == "__main__":
    main()
