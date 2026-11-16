/// The application id a Flatpak runs the program under, and nothing outside one.
pub fn flatpak_id() -> Option<String> {
    std::env::var("FLATPAK_ID").ok().filter(|id| !id.is_empty())
}

/// Whether the program is inside a Flatpak, whose sandbox anything it starts shares.
pub fn sandboxed() -> bool {
    flatpak_id().is_some()
}
