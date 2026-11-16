#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    Unity,
    /// From 1.7: every texture, PNG or DDS, is compressed once and kept in the
    /// game's own cache, so a DDS beside a PNG no longer loads faster.
    Waylith,
}

impl Engine {
    /// `None` for a version that is not `major.minor…`.
    pub fn read(game_version: &str) -> Option<Self> {
        let (major, rest) = game_version.trim().split_once('.')?;
        let minor = rest.split(|c: char| !c.is_ascii_digit()).next()?;
        let version = (major.parse::<u32>().ok()?, minor.parse::<u32>().ok()?);
        Some(match version < (1, 7) {
            true => Engine::Unity,
            false => Engine::Waylith,
        })
    }

    /// A version that cannot be read is taken as Unity's.
    pub fn of(game_version: &str) -> Self {
        Self::read(game_version).unwrap_or(Engine::Unity)
    }

    pub fn dds_loads_faster(self) -> bool {
        self == Engine::Unity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_engine_follows_the_game_version() {
        for (version, engine) in [
            ("1.6", Engine::Unity),
            ("1.4", Engine::Unity),
            ("1.7", Engine::Waylith),
            ("1.10", Engine::Waylith),
            ("1.7.5005 rev1082", Engine::Waylith),
            ("2.0", Engine::Waylith),
            ("", Engine::Unity),
            ("unknown", Engine::Unity),
        ] {
            assert_eq!(Engine::of(version), engine, "{version}");
        }
        assert_eq!(Engine::read("unknown"), None);
    }
}
