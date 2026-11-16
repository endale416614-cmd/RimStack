use crate::changes::Changes;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Section {
    #[default]
    Mods,
    Downloads,
    Textures,
    Workshop,
    Saves,
    Logs,
    Find,
    Settings,
}

impl Section {
    pub const ALL: [Self; 8] = [
        Self::Mods,
        Self::Downloads,
        Self::Textures,
        Self::Workshop,
        Self::Saves,
        Self::Logs,
        Self::Find,
        Self::Settings,
    ];

    pub fn at(at: usize) -> Self {
        Self::ALL[at.min(Self::ALL.len() - 1)]
    }

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn before(self) -> Self {
        Self::at(self.index().saturating_sub(1))
    }

    pub fn after(self) -> Self {
        Self::at(self.index() + 1)
    }
}

/// What a section hears while it is hidden is kept, and told once it is shown again: a hidden
/// section reads nothing.
#[derive(Default)]
pub struct Unseen {
    hidden: bool,
    missed: Changes,
}

impl Unseen {
    pub fn pass(&mut self, changes: Changes) -> Option<Changes> {
        match self.hidden {
            true => {
                self.missed |= changes;
                None
            }
            false => Some(changes),
        }
    }

    pub fn shown(&mut self, shown: bool) -> Option<Changes> {
        self.hidden = !shown;
        let missed = std::mem::take(&mut self.missed);
        match shown && !missed.is_empty() {
            true => Some(missed),
            false => {
                self.missed = missed;
                None
            }
        }
    }
}

pub trait Shown {
    fn set_shown(&mut self, shown: bool, cx: &mut gpui::Context<Self>)
    where
        Self: Sized;
}

#[cfg(test)]
mod tests {
    use super::{Changes, Section, Unseen};

    #[test]
    fn what_is_heard_while_hidden_is_told_once_shown() {
        let mut unseen = Unseen::default();
        assert_eq!(unseen.pass(Changes::LOGS), Some(Changes::LOGS));
        assert_eq!(unseen.shown(false), None);
        assert_eq!(unseen.pass(Changes::LOGS), None);
        assert_eq!(unseen.pass(Changes::LOAD_ORDER), None);
        assert_eq!(unseen.shown(false), None);
        assert_eq!(
            unseen.shown(true),
            Some(Changes::LOGS | Changes::LOAD_ORDER)
        );
        assert_eq!(unseen.shown(true), None);
    }

    #[test]
    fn each_section_is_at_its_tab_and_the_ends_hold() {
        for (at, section) in Section::ALL.iter().enumerate() {
            assert_eq!((Section::at(at), section.index()), (*section, at));
        }
        assert_eq!(Section::Mods.before(), Section::Mods);
        assert_eq!(Section::Settings.after(), Section::Settings);
        assert_eq!(Section::at(99), Section::Settings);
    }
}
