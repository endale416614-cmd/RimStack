//! Workshop descriptions are BBCode; the game's own tags (`<b>`, `<color=…>`) are read too.

mod blocks;

pub use blocks::{Block, Kind, Run};
use blocks::{Reader, Tag};

fn without_game_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('<') {
        out += &rest[..open];
        let inside = &rest[open + 1..];
        let named = inside.starts_with(|c: char| c.is_ascii_alphabetic() || c == '/');
        let Some(close) = inside.find('>').filter(|_| named) else {
            out.push('<');
            rest = inside;
            continue;
        };
        let tag = inside[..close].to_lowercase();
        if matches!(tag.as_str(), "b" | "/b" | "i" | "/i") {
            out += &format!("[{tag}]");
        }
        rest = &inside[close + 1..];
    }
    out + rest
}

fn safe_link(url: &str) -> bool {
    ["https://", "http://", "steam://"]
        .iter()
        .any(|s| url.starts_with(s))
}

/// A link is closed only if it was opened: one to an address that is not a
/// page never is. False when `tag` is no tag, and its bracket is text.
fn read_tag(reader: &mut Reader, tag: &str, in_link: &mut bool) -> bool {
    let (closing, name) = match tag.strip_prefix('/') {
        Some(name) => (true, name),
        None => (false, tag),
    };
    let (name, value) = name.split_once('=').unwrap_or((name, ""));
    let name = name.to_lowercase();
    let paired = match name.as_str() {
        "b" => Some(Tag::Bold),
        "i" | "spoiler" => Some(Tag::Italic),
        "u" => Some(Tag::Underline),
        "strike" => Some(Tag::Strike),
        "h1" => Some(Tag::Heading(1)),
        "h2" | "h3" => Some(Tag::Heading(2)),
        "list" => Some(Tag::List { ordered: false }),
        "olist" => Some(Tag::List { ordered: true }),
        "quote" => Some(Tag::Quote),
        "code" => Some(Tag::Code),
        _ => None,
    };
    if let Some(tag) = paired {
        match closing {
            true => reader.close(tag),
            false => reader.open(tag),
        }
        return true;
    }
    match (name.as_str(), closing) {
        ("*", false) => reader.open(Tag::Item),
        ("hr", false) => reader.open(Tag::Rule),
        ("url", false) if safe_link(value) => {
            *in_link = true;
            reader.link(Some(value.to_string()));
        }
        ("url", true) if std::mem::take(in_link) => reader.link(None),
        ("hr" | "table" | "tr" | "td" | "th" | "noparse" | "url", _) => {}
        _ => return false,
    }
    true
}

pub fn blocks(text: &str) -> Vec<Block> {
    let text = without_game_tags(text).replace("\r\n", "\n");
    let mut reader = Reader::default();
    let mut rest = text.trim();
    let mut in_link = false;
    while let Some(open) = rest.find('[') {
        reader.text(&rest[..open]);
        rest = &rest[open..];
        let Some(close) = rest.find(']') else {
            break;
        };
        let tag = &rest[1..close];
        let after = &rest[close + 1..];
        if tag.eq_ignore_ascii_case("img") {
            rest = after.find("[/img]").map_or(after, |end| &after[end + 6..]);
        } else if tag.eq_ignore_ascii_case("url") {
            let end = after.find("[/url]").unwrap_or(after.len());
            let url = &after[..end];
            let page = safe_link(url);
            if page {
                reader.link(Some(url.to_string()));
            }
            reader.text(url);
            if page {
                reader.link(None);
            }
            rest = after.get(end + 6..).unwrap_or("");
        } else if read_tag(&mut reader, tag, &mut in_link) {
            rest = after;
        } else {
            reader.text("[");
            rest = &rest[1..];
        }
    }
    reader.text(rest);
    reader.finish()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    Text(Vec<Block>),
    Image(String),
}

/// Pictures past `most`, and any whose address is not `https`, are left out.
pub fn pieces(text: &str, most: usize) -> Vec<Piece> {
    let lower = text.to_ascii_lowercase();
    let mut found = Vec::new();
    let (mut at, mut images) = (0, 0);
    let text_of = |chunk: &str, found: &mut Vec<Piece>| {
        let read = blocks(chunk);
        if !read.is_empty() {
            found.push(Piece::Text(read));
        }
    };
    while let Some(open) = lower[at..].find("[img]").map(|o| o + at) {
        let from = open + "[img]".len();
        let Some(close) = lower[from..].find("[/img]").map(|c| c + from) else {
            break;
        };
        text_of(&text[at..open], &mut found);
        let url = text[from..close].trim();
        let plain = url.starts_with("https://") && !url.contains(char::is_whitespace);
        if plain && images < most {
            found.push(Piece::Image(url.to_string()));
            images += 1;
        }
        at = close + "[/img]".len();
    }
    text_of(&text[at..], &mut found);
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn said(text: &str) -> Vec<(Kind, String)> {
        blocks(text).iter().map(|b| (b.kind, b.text())).collect()
    }

    fn runs(text: &str) -> Vec<(String, bool, Option<String>)> {
        blocks(text)
            .into_iter()
            .flat_map(|b| b.runs)
            .map(|r| (r.text, r.bold, r.link))
            .collect()
    }

    #[test]
    fn titles_are_two_sizes_and_lines_stay_in_their_paragraph() {
        assert_eq!(
            said("[h1]Title[/h1]\n[h2]Sub[/h2][h3]Sub[/h3]\n[b]Bold[/b] and [i]slanted[/i]\nNext"),
            [
                (Kind::Heading(1), "Title".into()),
                (Kind::Heading(2), "Sub".into()),
                (Kind::Heading(2), "Sub".into()),
                (Kind::Paragraph, "Bold and slanted\nNext".into()),
            ]
        );
    }

    #[test]
    fn links_open_only_pages() {
        let link =
            |text: &str, to: Option<&str>| vec![(text.to_string(), false, to.map(String::from))];
        assert_eq!(
            runs("[url=https://example.org/?a=1&b=2]Page[/url]"),
            link("Page", Some("https://example.org/?a=1&b=2"))
        );
        assert_eq!(
            runs("[url]https://example.org[/url]"),
            link("https://example.org", Some("https://example.org"))
        );
        assert_eq!(runs("[url=javascript:x]Bad[/url]"), link("Bad", None));
        assert_eq!(runs("[url]file:///etc[/url]"), link("file:///etc", None));
    }

    #[test]
    fn pictures_are_left_out_and_other_brackets_stay() {
        let text = |t: &str| said(t).into_iter().map(|(_, t)| t).collect::<Vec<_>>();
        assert_eq!(text("[img]REPLACE_HERO[/img]\nText"), ["Text"]);
        assert_eq!(text("[CF] Mod [1.6] a[0]"), ["[CF] Mod [1.6] a[0]"]);
        assert_eq!(text("open [ never closed"), ["open [ never closed"]);
        assert_eq!(text("[img]\nRequires Harmony."), ["Requires Harmony."]);
    }

    #[test]
    fn the_game_s_own_tags_are_read_and_html_is_not() {
        assert_eq!(
            runs("<b>Bold</b> <color=#ff0000>red</color>"),
            [("Bold".into(), true, None), (" red".into(), false, None)]
        );
        assert_eq!(said("a < b & c")[0].1, "a < b & c");
        assert_eq!(said("x [script]y")[0].1, "x [script]y");
    }

    #[test]
    fn a_description_is_cut_at_its_https_pictures_up_to_a_count() {
        let text = "[b]Hi[/b]\n[IMG] https://a.example/1.png [/IMG]\nMid[img]http://a.example/2.png[/img][img]https://a.example/3.png[/img] end";
        let shown = |text: &str, most| -> Vec<String> {
            pieces(text, most)
                .iter()
                .map(|piece| match piece {
                    Piece::Text(read) => read.iter().map(Block::text).collect(),
                    Piece::Image(url) => format!("picture {url}"),
                })
                .collect()
        };
        assert_eq!(
            shown(text, 5),
            [
                "Hi",
                "picture https://a.example/1.png",
                "Mid",
                "picture https://a.example/3.png",
                "end"
            ]
        );
        assert_eq!(shown(text, 1).len(), 4);
        assert_eq!(shown(text, 0), ["Hi", "Mid", "end"]);
        assert_eq!(shown("[img]https://x/y", 3), ["https://x/y"]);
    }
}
