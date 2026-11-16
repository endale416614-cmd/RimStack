//! Lists are package ids, as `ModsConfig.xml` writes them.

use crate::base::sentence::Sentence;
use crate::say;
const KEPT_STEPS: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    saved: Vec<String>,
    /// Oldest first; never empty.
    steps: Vec<Vec<String>>,
    at: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Changes<'a> {
    pub moved: Vec<&'a str>,
    pub activated: Vec<&'a str>,
    pub deactivated: Vec<&'a str>,
}

impl Changes<'_> {
    pub fn is_empty(&self) -> bool {
        self.moved.is_empty() && self.activated.is_empty() && self.deactivated.is_empty()
    }

    pub fn said(&self) -> Sentence {
        let parts = [
            (self.moved.len(), say!("{} moved", self.moved.len())),
            (
                self.activated.len(),
                say!("{} activated", self.activated.len()),
            ),
            (
                self.deactivated.len(),
                say!("{} deactivated", self.deactivated.len()),
            ),
        ];
        let said = parts.into_iter().filter(|(n, _)| *n > 0);
        Sentence::listed(said.map(|(_, said)| said).collect())
    }
}

impl Session {
    pub fn new(saved: Vec<String>) -> Self {
        Self {
            steps: vec![saved.clone()],
            saved,
            at: 0,
        }
    }

    pub fn current(&self) -> &[String] {
        &self.steps[self.at]
    }

    pub fn is_dirty(&self) -> bool {
        self.current() != self.saved.as_slice()
    }

    pub fn can_undo(&self) -> bool {
        self.at > 0
    }

    pub fn can_redo(&self) -> bool {
        self.at + 1 < self.steps.len()
    }

    /// The same list as now is not a step; a new step drops whatever could have been redone.
    pub fn record(&mut self, list: Vec<String>) {
        if list == self.current() {
            return;
        }
        self.steps.truncate(self.at + 1);
        self.steps.push(list);
        if self.steps.len() > KEPT_STEPS {
            self.steps.remove(0);
        }
        self.at = self.steps.len() - 1;
    }

    pub fn undo(&mut self) -> Option<&[String]> {
        if !self.can_undo() {
            return None;
        }
        self.at -= 1;
        Some(self.current())
    }

    pub fn redo(&mut self) -> Option<&[String]> {
        if !self.can_redo() {
            return None;
        }
        self.at += 1;
        Some(self.current())
    }

    pub fn saved(&self) -> &[String] {
        &self.saved
    }

    pub fn changes(&self) -> Changes<'_> {
        use std::collections::HashMap;
        let now = self.current();
        let before: HashMap<&str, usize> = self.saved.iter().map(String::as_str).zip(0..).collect();
        let here: std::collections::HashSet<&str> = now.iter().map(String::as_str).collect();
        let (kept, activated): (Vec<&str>, Vec<&str>) = now
            .iter()
            .map(String::as_str)
            .partition(|id| before.contains_key(id));
        let deactivated = self.saved.iter().map(String::as_str);
        let places: Vec<usize> = kept.iter().map(|id| before[id]).collect();
        let moved = kept.iter().zip(crate::mods::sort::moved(&places));
        let moved = moved.filter(|(_, moved)| *moved);
        Changes {
            moved: moved.map(|(id, _)| *id).collect(),
            activated,
            deactivated: deactivated.filter(|id| !here.contains(id)).collect(),
        }
    }

    pub fn mark_saved_as(&mut self, list: Vec<String>) {
        self.saved = list;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn changes_name_the_fewest_mods_that_moved_and_those_switched() {
        let mut s = Session::new(ids(&["a", "b", "c", "d", "e"]));
        assert!(s.changes().is_empty());
        assert!(s.changes().said().is_empty());
        s.record(ids(&["e", "a", "c", "x", "d"]));
        let changes = s.changes();
        assert_eq!(changes.moved, ["e"]);
        assert_eq!(changes.activated, ["x"]);
        assert_eq!(changes.deactivated, ["b"]);
        assert_eq!(
            changes.said().to_string(),
            "1 moved, 1 activated, 1 deactivated"
        );
        assert_eq!(
            changes.said().to_kept().to_string(),
            r#"[",",["{} moved",1],["{} activated",1],["{} deactivated",1]]"#
        );
        s.mark_saved_as(s.current().to_vec());
        assert!(s.changes().is_empty());
    }

    #[test]
    fn a_swap_of_neighbours_is_one_move() {
        let mut s = Session::new(ids(&["a", "b", "c"]));
        s.record(ids(&["a", "c", "b"]));
        assert_eq!(s.changes().moved.len(), 1);
    }

    #[test]
    fn a_change_can_be_undone_and_redone() {
        let mut s = Session::new(ids(&["a", "b"]));
        s.record(ids(&["b", "a"]));
        assert!(s.is_dirty() && s.can_undo());
        assert_eq!(s.undo().unwrap(), ids(&["a", "b"]));
        assert!(!s.is_dirty() && s.can_redo());
        assert_eq!(s.redo().unwrap(), ids(&["b", "a"]));
        assert_eq!(s.redo(), None);
    }

    #[test]
    fn recording_the_same_list_is_not_a_step() {
        let mut s = Session::new(ids(&["a"]));
        s.record(ids(&["a"]));
        assert!(!s.can_undo());
    }

    #[test]
    fn a_change_after_undo_drops_what_could_be_redone() {
        let mut s = Session::new(ids(&["a"]));
        s.record(ids(&["b"]));
        s.undo();
        s.record(ids(&["c"]));
        assert!(!s.can_redo());
        assert_eq!(s.undo().unwrap(), ids(&["a"]));
    }

    #[test]
    fn saving_makes_the_current_list_clean_and_undo_makes_it_dirty_again() {
        let mut s = Session::new(ids(&["a"]));
        s.record(ids(&["b"]));
        s.mark_saved_as(s.current().to_vec());
        assert!(!s.is_dirty());
        s.undo();
        assert!(s.is_dirty());
    }

    #[test]
    fn old_steps_are_forgotten_past_the_limit() {
        let mut s = Session::new(ids(&["0"]));
        for i in 1..=KEPT_STEPS + 50 {
            s.record(vec![i.to_string()]);
        }
        let mut undone = 0;
        while s.undo().is_some() {
            undone += 1;
        }
        assert_eq!(undone, KEPT_STEPS - 1);
    }
}
