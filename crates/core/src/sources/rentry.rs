use crate::base::error::Error;
use crate::mods::share;
use crate::said;

const SITE: &str = "https://rentry.co";
const PAGE_LIMIT: u64 = 2_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Published {
    pub url: String,
    pub edit_code: String,
}

/// The listing with RimSort's `{packageid: x}` token after each mod: RimSort reads
/// a page by that token and by `?id=` links alone.
pub fn document(listing: &str) -> String {
    let mut text = String::new();
    for line in listing.lines() {
        text += line;
        if line.contains('[')
            && let [id] = share::ids_in(line).as_slice()
        {
            text += &format!(" {{packageid: {id}}}");
        }
        text.push('\n');
    }
    text
}

/// The page's name in a link to rentry.co over https, whatever follows it (`/edit`, `/raw`, a query).
pub fn link_id(text: &str) -> Option<String> {
    let rest = text.trim().strip_prefix("https://rentry.co/")?;
    let id = rest.split(['/', '?', '#']).next()?;
    let plain = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_');
    (!id.is_empty() && id.chars().all(plain)).then(|| id.to_string())
}

/// Format: a `Set-Cookie` value is `name=value; attributes`.
pub fn csrf_token(set_cookies: &[&str]) -> Option<String> {
    set_cookies
        .iter()
        .filter_map(|cookie| cookie.split(';').next()?.trim().strip_prefix("csrftoken="))
        .find(|token| !token.is_empty())
        .map(String::from)
}

fn encoded(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(byte))
            }
            _ => out += &format!("%{byte:02X}"),
        }
    }
    out
}

pub fn form(token: &str, text: &str) -> String {
    format!(
        "csrfmiddlewaretoken={}&text={}",
        encoded(token),
        encoded(text)
    )
}

/// Format: `{"status": "200", "url": …, "edit_code": …}`; on failure the status is another
/// number and `content` or `errors` says why.
pub fn published_from(json: &str) -> Result<Published, Error> {
    let answer: serde_json::Value = serde_json::from_str(json)
        .map_err(|_| Error::network(said!("Rentry.co answered in a way RimStack cannot read")))?;
    let text = |key: &str| answer[key].as_str().map(str::to_string);
    let status = text("status").or_else(|| answer["status"].as_i64().map(|n| n.to_string()));
    if status.as_deref() == Some("200")
        && let Some(url) = text("url")
    {
        return Ok(Published {
            url,
            edit_code: text("edit_code").unwrap_or_default(),
        });
    }
    let why = text("content")
        .or_else(|| text("errors"))
        .or_else(|| {
            answer["errors"].as_array().map(|all| {
                all.iter()
                    .filter_map(|e| e.as_str())
                    .collect::<Vec<_>>()
                    .join("; ")
            })
        })
        .filter(|why| !why.is_empty())
        .unwrap_or_else(|| "no reason given".to_string());
    Err(Error::network(said!("Rentry.co refused it: {}", why)))
}

fn unescaped(text: &str) -> String {
    let entities = [
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&#039;", "'"),
        ("&#x27;", "'"),
        ("&#39;", "'"),
        ("&amp;", "&"),
    ];
    entities
        .iter()
        .fold(text.to_string(), |text, (from, to)| text.replace(from, to))
}

/// Format: the edit page holds the text in its one `<textarea>`.
pub fn text_of_edit_page(page: &str) -> Option<String> {
    let at = page.find("<textarea")?;
    let after = &page[at..];
    let body = &after[after.find('>')? + 1..];
    let text = &body[..body.find("</textarea>")?];
    Some(unescaped(text.strip_prefix('\n').unwrap_or(text)))
}

/// Blocks on the network.
pub fn publish(listing: &str) -> Result<Published, Error> {
    let agent = crate::base::web::agent(30);
    let home = agent
        .get(SITE)
        .call()
        .map_err(|_| Error::network(said!("Rentry.co could not be reached")))?;
    let cookies: Vec<&str> = home
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .collect();
    let token = csrf_token(&cookies)
        .ok_or_else(|| Error::network(said!("Rentry.co gave no token to publish with")))?;
    let mut answer = match agent
        .post(format!("{SITE}/api/new"))
        .header("Referer", SITE)
        .header("Cookie", format!("csrftoken={token}"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .send(form(&token, &document(listing)))
    {
        Ok(answer) => answer,
        Err(ureq::Error::StatusCode(code)) => {
            return Err(Error::network(said!("Rentry.co answered {}", code)));
        }
        Err(_) => return Err(Error::network(said!("Rentry.co could not be reached"))),
    };
    let body =
        crate::base::web::read_text(&mut answer, PAGE_LIMIT).map_err(crate::base::web::failed)?;
    published_from(&body)
}

/// The text of the page named `id`. Blocks on the network.
pub fn fetch(id: &str) -> Result<String, Error> {
    let mut answer = match crate::base::web::agent(30)
        .get(format!("{SITE}/{id}/edit"))
        .call()
    {
        Ok(answer) => answer,
        Err(ureq::Error::StatusCode(404)) => {
            return Err(Error::network(said!(
                "There is no Rentry.co page at that link"
            )));
        }
        Err(ureq::Error::StatusCode(code)) => {
            return Err(Error::network(said!("Rentry.co answered {}", code)));
        }
        Err(_) => return Err(Error::network(said!("Rentry.co could not be reached"))),
    };
    let page =
        crate::base::web::read_text(&mut answer, PAGE_LIMIT).map_err(crate::base::web::failed)?;
    text_of_edit_page(&page)
        .ok_or_else(|| Error::network(said!("That Rentry.co page holds no text RimStack can read")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_https_link_to_rentry_co_is_a_page() {
        assert_eq!(
            link_id(" https://rentry.co/abc-12/ \n").as_deref(),
            Some("abc-12")
        );
        assert_eq!(
            link_id("https://rentry.co/abc/edit?x=1").as_deref(),
            Some("abc")
        );
        for no in [
            "http://rentry.co/abc",
            "https://rentry.org/abc",
            "https://rentry.co.evil.com/abc",
            "https://rentry.co@evil.com/abc",
            "https://rentry.co/",
            "https://www.rentry.co/abc",
            "ludeon.rimworld",
        ] {
            assert_eq!(link_id(no), None, "{no}");
        }
    }

    #[test]
    fn the_token_comes_from_the_cookie_that_names_it() {
        let cookies = [
            "sessionid=x; Path=/",
            "csrftoken=tok123; expires=Fri; Path=/; SameSite=Lax",
        ];
        assert_eq!(csrf_token(&cookies).as_deref(), Some("tok123"));
        assert_eq!(csrf_token(&["sessionid=x"]), None);
        assert_eq!(csrf_token(&[]), None);
    }

    #[test]
    fn the_form_is_percent_encoded() {
        assert_eq!(
            form("t k", "A [b.c]\n&"),
            "csrfmiddlewaretoken=t%20k&text=A%20%5Bb.c%5D%0A%26"
        );
    }

    #[test]
    fn an_answer_gives_the_link_or_the_reason() {
        let ok = r#"{"status":"200","url":"https://rentry.co/xyz","edit_code":"e1"}"#;
        assert_eq!(
            published_from(ok).unwrap(),
            Published {
                url: "https://rentry.co/xyz".into(),
                edit_code: "e1".into()
            }
        );
        let refused = r#"{"status":"400","content":"Text is too long"}"#;
        assert!(
            published_from(refused)
                .unwrap_err()
                .to_string()
                .contains("Text is too long")
        );
        let listed = r#"{"status":"400","errors":["a","b"]}"#;
        assert!(
            published_from(listed)
                .unwrap_err()
                .to_string()
                .contains("a; b")
        );
        assert!(published_from("<html>").is_err());
    }

    #[test]
    fn the_text_is_read_out_of_the_edit_page() {
        let page = "<form><textarea name=\"text\" rows=3>\nA &lt;b&gt; &amp;amp; [x.y]\nz.w\n</textarea></form>";
        assert_eq!(
            text_of_edit_page(page).as_deref(),
            Some("A <b> &amp; [x.y]\nz.w\n")
        );
        assert_eq!(text_of_edit_page("<p>none</p>"), None);
    }

    #[test]
    fn the_uploaded_page_is_read_by_rimsort_and_by_rimstack() {
        let listing = "Created with RimStack\nTotal # of mods: 2\n\nHarmony [brrainz.harmony][https://x.y]\nRimWorld [ludeon.rimworld][No url specified]\n";
        let page = document(listing);
        assert!(
            page.contains("Harmony [brrainz.harmony][https://x.y] {packageid: brrainz.harmony}\n")
        );
        assert!(page.starts_with("Created with RimStack\nTotal # of mods: 2\n"));
        assert_eq!(share::ids_in(&page), ["brrainz.harmony", "ludeon.rimworld"]);
    }
}
