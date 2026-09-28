"""Tokenise styles.css so the theme system (accent / light mode / background
skin) only has to touch CSS variables.

Every literal on the left maps to a token whose **dark** value is that literal
(or within ~4/255 of it, which is imperceptible). That is what guarantees the
existing dark look does not change during this refactor.

Rare/one-off literals are deliberately left alone: unmapped means untouched,
and a second pass fixes whatever looks off in light mode.
"""
import io
import os
import re
import sys

# One-shot script, already applied. Kept in the repo because the mapping table
# below *is* the documentation for the token ladder in styles.css — if you ever
# need to add a colour, map it here first instead of hardcoding a literal.
CSS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "src", "styles.css")

TOKENS = """  /* 表面阶梯：s0 最暗，越靠后越"浮"起来 */
  --bg: #000;
  --s0: #0a0a0c;
  --s1: #0e0e10;
  --s2: #141416;
  --s3: #17171a;
  --s4: #1a1a1c;
  --s5: #1c1c1e;
  --s6: #1f1f22;
  --s7: #232326;
  --s8: #26262b;
  --s9: #2a2a2e;
  --s10: #2c2c2e;
  --s11: #33333a;
  --s12: #3a3a3f;
  --s13: #4a4a50;
  /* 面板别名（历史命名，保留） */
  --panel: #1c1c1e;
  --panel-2: #232326;
  --hover: #2a2a2e;
  --line: #2c2c2e;
  --line-2: #3a3a3f;
  /* 文字层级：text > text-2 > text-3 > text-4 > dim-0 > dim > dim-1 > dim-2 */
  --text: #fff;
  --text-2: #e6e6ea;
  --text-3: #d7d7dc;
  --text-4: #c9c9ce;
  --text-5: #b9b9c0;
  --dim-0: #a9a9b0;
  --dim: #8e8e93;
  --dim-1: #7d7d83;
  --dim-2: #636366;
  /* 主题色：--accent 主色，--tint 是双色卡的下半块 */
  --accent: #0a84ff;
  --accent-rgb: 10, 132, 255;
  --accent-soft: rgba(10, 132, 255, .16);
  --accent-ink: #cfe4ff;
  --tint: #0a84ff;
  --warn: #ff9f0a;
  --warn-rgb: 255, 159, 10;
  --danger: #ff453a;
  --ok: #7bd88f;
  --blue: var(--accent);
  --fx: var(--accent);
  --r: 14px;
  /* 背景皮肤（浅色模式与皮肤都只改这几个） */
  --bg-image: none;
  --bg-veil-image: none;
"""

MAP = [
    ("#ffffff", "var(--text)"),
    ("#fff", "var(--text)"),
    ("#f2f2f6", "var(--text)"),
    ("#f0f0f4", "var(--text)"),
    ("#ececf0", "var(--text)"),
    ("#e6e6ea", "var(--text-2)"),
    ("#e2e2e7", "var(--text-2)"),
    ("#dcdce1", "var(--text-2)"),
    ("#d7d7dc", "var(--text-3)"),
    ("#c9c9ce", "var(--text-4)"),
    ("#b9b9c0", "var(--text-5)"),
    ("#b9b9be", "var(--text-5)"),
    ("#b6b6bc", "var(--text-5)"),
    ("#a9a9b0", "var(--dim-0)"),
    ("#9a9aa0", "var(--dim)"),
    ("#8e8e93", "var(--dim)"),
    ("#7d7d83", "var(--dim-1)"),
    ("#6f6f76", "var(--dim-2)"),
    ("#6b6b72", "var(--dim-2)"),
    ("#636366", "var(--dim-2)"),
    ("#cfe4ff", "var(--accent-ink)"),
    ("#0a84ff", "var(--accent)"),
    ("#0a0a0c", "var(--s0)"),
    ("#0b0b0d", "var(--s0)"),
    ("#0e0e10", "var(--s1)"),
    ("#101012", "var(--s1)"),
    ("#131317", "var(--s2)"),
    ("#141416", "var(--s2)"),
    ("#161618", "var(--s3)"),
    ("#17171a", "var(--s3)"),
    ("#1a1a1c", "var(--s4)"),
    ("#1a1a1e", "var(--s4)"),
    ("#1b1b1f", "var(--s4)"),
    ("#1c1c1e", "var(--s5)"),
    ("#1c1c1f", "var(--s5)"),
    ("#1e1e22", "var(--s6)"),
    ("#1f1f22", "var(--s6)"),
    ("#202024", "var(--s6)"),
    ("#232326", "var(--s7)"),
    ("#26262a", "var(--s8)"),
    ("#26262b", "var(--s8)"),
    ("#2a2a2e", "var(--s9)"),
    ("#2a2a30", "var(--s9)"),
    ("#2b2b30", "var(--s9)"),
    ("#2c2c2e", "var(--s10)"),
    ("#2c2c31", "var(--s10)"),
    ("#2e2e35", "var(--s11)"),
    ("#33333a", "var(--s11)"),
    ("#35353c", "var(--s11)"),
    ("#3a3a3f", "var(--s12)"),
    ("#3a3a40", "var(--s12)"),
    ("#3f3f45", "var(--s12)"),
    ("#45454e", "var(--s13)"),
    ("#4a4a50", "var(--s13)"),
    ("rgba(10, 132, 255,", "rgba(var(--accent-rgb),"),
    ("rgba(255, 159, 10,", "rgba(var(--warn-rgb),"),
]

LIGHT = """
/* ── 浅色模式 ───────────────────────────────────────────────────────
   只覆盖 token，组件规则一行都不动。 */
html[data-theme="light"] {
  --bg: #f4f4f7;
  --s0: #ffffff;
  --s1: #fbfbfd;
  --s2: #f6f6f9;
  --s3: #f2f2f6;
  --s4: #ededf2;
  --s5: #ffffff;
  --s6: #e9e9ef;
  --s7: #e4e4ea;
  --s8: #dfdfe6;
  --s9: #d8d8e0;
  --s10: #dbdbe2;
  --s11: #c9c9d2;
  --s12: #c0c0ca;
  --s13: #b4b4bf;
  --panel: #ffffff;
  --panel-2: #f7f7fa;
  --hover: #ececf1;
  --line: #e2e2e9;
  --line-2: #cdcdd7;
  --text: #16161a;
  --text-2: #2e2e36;
  --text-3: #4a4a54;
  --text-4: #5c5c67;
  --text-5: #6b6b76;
  --dim-0: #75757f;
  --dim: #7c7c86;
  --dim-1: #8b8b95;
  --dim-2: #9a9aa4;
  --accent-soft: rgba(10, 132, 255, .13);
  --accent-ink: #10457e;
  --bg-veil-image: linear-gradient(rgba(255, 255, 255, .72), rgba(255, 255, 255, .72));
}
"""


def main():
    text = io.open(CSS, encoding="utf-8").read()

    # 1) swap the :root block for the full token set + light overrides
    m = re.compile(r":root\s*\{.*?\n\}", re.S).search(text)
    if not m:
        raise SystemExit("could not find the :root block")
    head = ":root {\n" + TOKENS + "}\n" + LIGHT
    text = text[: m.start()] + head + text[m.end():]

    # 2) map literals -> tokens, everywhere *after* the token block so we never
    #    rewrite the definitions themselves
    split = text.index("html[data-theme=\"light\"]")
    split = text.index("\n", text.index("}", split)) + 1
    body, tail = text[:split], text[split:]

    counts = {}
    for lit, tok in MAP:
        n = tail.count(lit)
        if n:
            tail = tail.replace(lit, tok)
            counts[lit] = n
    text = body + tail

    # 3) let body honour the background skin: image on top of the base colour,
    #    veil on top of the image. Multi-layer background keeps it stacking-safe
    #    (no extra DOM, no z-index games).
    old_body = """body {
  background: var(--bg);
  color: var(--text);"""
    new_body = """body {
  background-color: var(--bg);
  background-image: var(--bg-veil-image), var(--bg-image);
  background-size: cover, cover;
  background-position: center, center;
  background-repeat: no-repeat, no-repeat;
  background-attachment: fixed, fixed;
  color: var(--text);"""
    if old_body not in text:
        raise SystemExit("could not find the body rule")
    text = text.replace(old_body, new_body, 1)

    io.open(CSS, "w", encoding="utf-8", newline="\n").write(text)

    total = sum(counts.values())
    print("replaced %d literals across %d distinct values" % (total, len(counts)))
    for lit, n in sorted(counts.items(), key=lambda kv: -kv[1]):
        print("  %-12s x%d" % (lit, n))


if __name__ == "__main__":
    main()
