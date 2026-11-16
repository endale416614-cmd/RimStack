#!/usr/bin/env python3
"""Lists the interface's texts: the `say!`, `said!` and `say_n!` of the Rust.

Run after adding or changing a text:  tools/words.py
It writes crates/app/assets/words.json, the list a translation starts from.
A text already listed stays listed: some are put in words from a value
(a category's or a colour's name).
"""
import glob, json, os, re

root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "crates", "app")
literal = r'"((?:[^"\\\n]|\\.)*)"'
words = set()
# The sentences written in Rust: `say!("…")`, `said!("…")` and `named("…")`.
rust = glob.glob(os.path.join(root, "..", "*", "src", "**", "*.rs"), recursive=True)
# A file declared `#[cfg(test)] mod name;` is a test: beside `main.rs`,
# `lib.rs` or `mod.rs`, otherwise in the folder named after its parent.
tests = set()
for path in rust:
    for name in re.findall(r"#\[cfg\(test\)\]\s*mod (\w+);", open(path, encoding="utf-8").read()):
        stem, folder = os.path.splitext(os.path.basename(path))[0], os.path.dirname(path)
        inside = folder if stem in ("main", "lib", "mod") else os.path.join(folder, stem)
        tests.add(os.path.normpath(os.path.join(inside, name + ".rs")))
for path in rust:
    if os.path.normpath(path) in tests:
        continue
    source = re.split(r"#\[cfg\(test\)\]\s*mod \w+\s*\{", open(path, encoding="utf-8").read())[0]
    words.update(json.loads(f'"{t}"') for t in re.findall(r"(?<!\w)(?:say!|said!|named)\(\s*" + literal, source))
    for pair in re.findall(r"say_n!\([^\"]*" + literal + r"\s*,\s*" + literal, source):
        words.update(json.loads(f'"{t}"') for t in pair)

# The language's locale, for its plural rules and to be found from the
# system's settings: "fr", "pt-BR".
words.add("@locale")
out = os.path.join(root, "assets", "words.json")
words.update(json.load(open(out, encoding="utf-8")) if os.path.exists(out) else {})
json.dump({w: "" for w in sorted(words)}, open(out, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
english = os.path.join(root, "assets", "languages", "English.json")
kept = json.load(open(english, encoding="utf-8")) if os.path.exists(english) else {}
said = {w: kept.get(w) or ("en" if w == "@locale" else w) for w in sorted(words)}
json.dump(said, open(english, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
print(len(words), "texts")
