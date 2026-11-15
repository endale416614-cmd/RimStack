//! The drawings are from Lucide (lucide.dev), ISC licence; see licences/lucide.txt.

use gpui::{AssetSource, SharedString};
use std::borrow::Cow;

macro_rules! icons {
    ($($name:literal),* $(,)?) => {
        const ICONS: &[(&str, &[u8])] = &[
            $(($name, include_bytes!(concat!("../../assets/icons/", $name, ".svg")))),*
        ];
    };
}

icons![
    "arrow-down",
    "arrow-down-to-line",
    "arrow-up",
    "arrow-up-down",
    "check",
    "chevron-down",
    "chevron-right",
    "chevron-up",
    "circle-alert",
    "cloud",
    "clock",
    "copy",
    "corner-down-right",
    "download",
    "ellipsis",
    "external-link",
    "eye-off",
    "folder-open",
    "funnel",
    "gamepad-2",
    "git-branch",
    "hard-drive",
    "heart",
    "history",
    "image",
    "info",
    "keyboard",
    "layers",
    "layout-dashboard",
    "library",
    "link",
    "list",
    "minus",
    "package",
    "pause",
    "play",
    "plug",
    "plus",
    "power",
    "puzzle",
    "redo-2",
    "refresh-cw",
    "save",
    "scroll-text",
    "search",
    "settings",
    "share-2",
    "star",
    "tag",
    "trash-2",
    "triangle-alert",
    "undo-2",
    "wrench",
    "x",
    "zap",
];

const BRAND: &[u8] = include_bytes!("../../assets/rimstack.svg");

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        if path == "rimstack.svg" {
            return Ok(Some(Cow::Borrowed(BRAND)));
        }
        let name = path
            .strip_prefix("icons/")
            .and_then(|p| p.strip_suffix(".svg"));
        Ok(ICONS
            .iter()
            .find(|(n, _)| Some(*n) == name)
            .map(|(_, svg)| Cow::Borrowed(*svg)))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(match path {
            "icons" | "icons/" => ICONS
                .iter()
                .map(|(n, _)| format!("icons/{n}.svg").into())
                .collect(),
            _ => Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_a_view_names_is_served() {
        for name in crate::ui::theme::CATEGORY_ICONS.iter().chain(&[
            "play",
            "search",
            "chevron-down",
            "history",
        ]) {
            let found = Assets.load(&format!("icons/{name}.svg")).unwrap();
            assert!(found.is_some_and(|svg| svg.starts_with(b"<svg")), "{name}");
        }
        assert!(Assets.load("rimstack.svg").unwrap().is_some());
    }
}
