use super::menu::Entry;
use crate::words::put;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::say;

pub fn links() -> [(Sentence, &'static str); 4] {
    [
        (say!("Join the Discord"), "https://discord.gg/Za5CSzB52r"),
        (
            say!("Sponsor on GitHub"),
            "https://github.com/sponsors/TheForgotten69",
        ),
        (say!("Support on Ko-fi"), "https://ko-fi.com/theforgotten69"),
        (
            say!("Support on Patreon"),
            "https://www.patreon.com/theforgotten69",
        ),
    ]
}

pub fn menu() -> Vec<Entry> {
    let mut entries = vec![Entry::Note(
        put(&say!(
            "RimStack is free and made in spare time. If it saves you time, you can help keep it going."
        ))
        .into(),
    )];
    for (at, (name, address)) in links().into_iter().enumerate() {
        if at == 1 {
            entries.push(Entry::Line);
        }
        entries.push(
            Entry::item(put(&name), move |_, cx| cx.open_url(address))
                .icon("external-link")
                .tip(address)
                .into(),
        );
    }
    entries
}
