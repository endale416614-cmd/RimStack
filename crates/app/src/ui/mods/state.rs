use super::{chosen, columns, hints, notes};
use crate::jobs::Lane;
use gpui::Pixels;
use rimstack_core::mods::library::ModId;
use std::rc::Rc;

#[derive(Default)]
pub struct Status {
    pub save_problem: String,
    pub upkeep_problem: String,
    pub file_changed: bool,
    pub folder_problem: String,
    pub unhide_problem: String,
    pub closed_note: String,
    pub note_timer: String,
}

pub struct Layout {
    pub inactive_width: Pixels,
    pub details_width: Pixels,
    pub drawer_open: bool,
    pub drawer_turn: usize,
    pub edge_drag: Option<columns::EdgeDrag>,
    /// The columns each list showed when last drawn.
    pub shown: [Vec<&'static str>; 2],
    pub menu_buttons: columns::Placed,
}

impl Layout {
    pub fn new(inactive_width: Pixels, details_width: Pixels) -> Self {
        Self {
            inactive_width,
            details_width,
            drawer_open: false,
            drawer_turn: 0,
            edge_drag: None,
            shown: Default::default(),
            menu_buttons: Default::default(),
        }
    }
}

#[derive(Default)]
pub struct Hinting {
    /// The first-run hint shown, 1 to 3; 0 for none.
    pub step: u8,
    pub anchors: hints::Anchors,
}

#[derive(Default)]
pub struct ChosenReads {
    pub read: Lane,
    pub files: Option<(ModId, Rc<chosen::FilesRead>)>,
    pub commits: Option<(ModId, Rc<Vec<rimstack_core::sources::changes::Note>>)>,
    pub repository: Option<(ModId, String)>,
    pub commits_read: Lane,
    pub picking: Option<notes::Picking>,
    pub drafts: notes::Drafts,
    pub notes_saved: bool,
}

#[derive(Default)]
pub struct Reads {
    pub conflicts: usize,
    pub fresh: usize,
}

#[derive(Default)]
pub struct ProblemNav {
    pub row: Option<usize>,
    pub open: bool,
}
