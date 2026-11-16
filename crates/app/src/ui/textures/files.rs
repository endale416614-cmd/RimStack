use super::Textures;
use crate::ui::rows::{Scrolling, bar, rows};
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, elide_start_in, icon, label, measure, with_alpha};
use crate::words::put;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, Pixels, Role,
    StatefulInteractiveElement, Styled, WeakEntity, Window, div, prelude::FluentBuilder,
};
use rimstack_core::base::sentence::Sentence;
use rimstack_core::base::sizes;
use rimstack_core::say_n;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

pub(crate) enum Note {
    Bytes(u64),
    Why(Sentence),
}

pub(crate) struct Item {
    pub shown: String,
    pub note: Note,
}

pub(crate) struct Group {
    pub name: String,
    pub bytes: Option<u64>,
    pub items: Vec<Item>,
}

#[derive(Clone, Copy)]
enum Line {
    Group { at: usize, open: bool },
    Item(usize, usize),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Which {
    Refused,
    Spare,
}

pub(crate) struct FileList {
    groups: Rc<Vec<Group>>,
    open: HashSet<usize>,
    lines: Rc<Vec<Line>>,
    tops: RefCell<Option<(Pixels, Rc<[Pixels]>)>>,
    scroll: Scrolling,
}

impl FileList {
    pub fn new(groups: Vec<Group>) -> Self {
        let mut list = Self {
            groups: Rc::new(groups),
            open: HashSet::new(),
            lines: Rc::default(),
            tops: RefCell::default(),
            scroll: Scrolling::default(),
        };
        list.relines();
        list
    }

    pub fn files(&self) -> usize {
        self.groups.iter().map(|g| g.items.len()).sum()
    }

    pub fn toggle(&mut self, group: usize) {
        if !self.open.remove(&group) {
            self.open.insert(group);
        }
        self.relines();
    }

    fn relines(&mut self) {
        let mut lines = Vec::new();
        for (at, group) in self.groups.iter().enumerate() {
            let open = self.open.contains(&at);
            lines.push(Line::Group { at, open });
            if open {
                lines.extend((0..group.items.len()).map(|item| Line::Item(at, item)));
            }
        }
        self.lines = Rc::new(lines);
        self.tops.take();
    }

    fn tops(&self, row: Pixels) -> Rc<[Pixels]> {
        let mut kept = self.tops.borrow_mut();
        if let Some((height, tops)) = kept.as_ref()
            && *height == row
        {
            return tops.clone();
        }
        let tops: Rc<[Pixels]> = (0..=self.lines.len())
            .map(|at| row * at as f32)
            .collect::<Vec<_>>()
            .into();
        *kept = Some((row, tops.clone()));
        tops
    }

    pub(super) fn draw(
        &self,
        which: Which,
        theme: &Theme,
        cx: &mut Context<Textures>,
    ) -> AnyElement {
        let row = theme.row();
        let tops = self.tops(row);
        let (groups, lines) = (self.groups.clone(), self.lines.clone());
        let (owned, me) = (theme.clone(), cx.entity().downgrade());
        let (id, rows_id) = match which {
            Which::Refused => ("refused-files", "refused-rows"),
            Which::Spare => ("spare-files", "spare-rows"),
        };
        let list = rows(
            rows_id,
            self.scroll.clone(),
            tops,
            row,
            cx.entity_id(),
            move |range, width, window, _| {
                range
                    .filter_map(|at| {
                        let line = *lines.get(at)?;
                        Some(draw_line(
                            &owned, &groups, line, at, width, which, &me, window,
                        ))
                    })
                    .collect()
            },
        );
        div()
            .id(id)
            .role(Role::List)
            .relative()
            .flex_1()
            .min_h_0()
            .child(list)
            .child(bar(theme, self.scroll.clone(), cx.entity_id()))
            .into_any_element()
    }
}

fn note_said(note: &Note) -> String {
    match note {
        Note::Bytes(bytes) => put(&sizes::of_bytes(*bytes)),
        Note::Why(why) => put(why),
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_line(
    theme: &Theme,
    groups: &[Group],
    line: Line,
    at: usize,
    width: Pixels,
    which: Which,
    me: &WeakEntity<Textures>,
    window: &Window,
) -> AnyElement {
    let c = &theme.colours;
    let hover = with_alpha(c.hover, 0.04);
    let row = div()
        .id(("file-line", at))
        .role(Role::ListItem)
        .w(width)
        .h(theme.row())
        .flex()
        .items_center()
        .gap(theme::GAP_SMALL)
        .px(theme::GAP_SMALL)
        .rounded(theme::CORNER)
        .hover(move |s| s.bg(hover));
    match line {
        Line::Group { at: group_at, open } => {
            let group = &groups[group_at];
            let files = say_n!(group.items.len(), "{} file", "{} files");
            let counted = match group.bytes {
                Some(bytes) => format!("{}, {}", put(&files), put(&sizes::of_bytes(bytes))),
                None => put(&files),
            };
            let chevron = match open {
                true => "chevron-down",
                false => "chevron-right",
            };
            let me = me.clone();
            row.aria_label(format!("{}, {}", group.name, counted))
                .cursor_pointer()
                .child(icon(chevron, theme.icon(), c.ink2))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .sized_text(theme, theme.body())
                        .font_weight(theme.semibold)
                        .text_color(c.ink)
                        .child(group.name.clone()),
                )
                .child(label(theme, theme.small(), c.ink2, counted))
                .on_click(move |_, _, cx| {
                    let _ = me.update(cx, |t, cx| t.toggle_files(which, group_at, cx));
                })
                .into_any_element()
        }
        Line::Item(group_at, item_at) => {
            let item = &groups[group_at].items[item_at];
            let note = note_said(&item.note);
            let indent = theme.icon() + theme::GAP_SMALL;
            let why = matches!(item.note, Note::Why(_));
            let said_width = measure(window, theme, &note, theme.small(), FontWeight::NORMAL);
            let note_room = match why {
                true => (width * 0.45).min(said_width),
                false => said_width,
            };
            let room = width - indent - note_room - theme::GAP_SMALL * 4.;
            let shown = elide_start_in(
                window,
                theme,
                theme.mono.clone(),
                &item.shown,
                theme.small(),
                room,
            );
            row.aria_label(format!("{}, {}", item.shown, note))
                .pl(theme::GAP_SMALL + indent)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .sized_text(theme, theme.small())
                        .font_family(theme.mono.clone())
                        .text_color(c.ink)
                        .child(shown),
                )
                .child(
                    div()
                        .w(note_room)
                        .flex_none()
                        .flex()
                        .when(!why, |d| d.justify_end())
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .sized_text(theme, theme.small())
                        .text_color(c.ink3)
                        .child(note),
                )
                .into_any_element()
        }
    }
}
