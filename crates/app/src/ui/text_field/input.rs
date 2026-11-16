//! What an input method asks of a field: its text and selection counted in UTF-16, and where
//! a character is drawn.

use super::*;

impl TextField {
    fn to_utf16(&self, offset: usize) -> usize {
        self.text[..offset.min(self.text.len())]
            .encode_utf16()
            .count()
    }

    fn offset_of_utf16(&self, offset: usize) -> usize {
        let mut counted = 0;
        for (at, c) in self.text.char_indices() {
            if counted >= offset {
                return at;
            }
            counted += c.len_utf16();
        }
        self.text.len()
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.to_utf16(range.start)..self.to_utf16(range.end)
    }

    fn range_of_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_of_utf16(range.start)..self.offset_of_utf16(range.end)
    }
}

impl EntityInputHandler for TextField {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_of_utf16(&range);
        adjusted.replace(self.range_to_utf16(&range));
        Some(self.text[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected),
            reversed: self.reversed,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.as_ref().map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked = None;
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let range = range
            .as_ref()
            .map(|range| self.range_of_utf16(range))
            .or(self.marked.clone())
            .unwrap_or(self.selected.clone());
        self.text = format!(
            "{}{new}{}",
            &self.text[..range.start],
            &self.text[range.end..]
        );
        let at = range.start + new.len();
        self.selected = at..at;
        self.reversed = false;
        self.marked = None;
        cx.emit(Edited);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let range = range
            .as_ref()
            .map(|range| self.range_of_utf16(range))
            .or(self.marked.clone())
            .unwrap_or(self.selected.clone());
        self.text = format!(
            "{}{new}{}",
            &self.text[..range.start],
            &self.text[range.end..]
        );
        self.marked = (!new.is_empty()).then(|| range.start..range.start + new.len());
        self.selected = selected
            .as_ref()
            .map(|inside| range.start + inside.start..range.start + inside.end)
            .unwrap_or(range.start + new.len()..range.start + new.len());
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.range_of_utf16(&range);
        if let Some((wrapped, _)) = &*self.wrapped.borrow() {
            let (from, to) = (wrapped.position(range.start), wrapped.position(range.end));
            let line = bounds.size.height / self.rows_drawn.get().max(1) as f32;
            return Some(Bounds::from_corners(
                bounds.origin + from,
                bounds.origin + point(to.x.max(from.x), to.y + line),
            ));
        }
        let shaped = self.shaped.borrow();
        let (shaped, _) = shaped.as_ref()?;
        Some(Bounds::from_corners(
            point(
                bounds.left() + shaped.x_for_index(self.shown_at(range.start)),
                bounds.top(),
            ),
            point(
                bounds.left() + shaped.x_for_index(self.shown_at(range.end)),
                bounds.bottom(),
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        at: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        if let Some((wrapped, bounds)) = &*self.wrapped.borrow() {
            return Some(self.to_utf16(wrapped.index_at(at - bounds.origin)));
        }
        let shaped = self.shaped.borrow();
        let (shaped, bounds) = shaped.as_ref()?;
        let index = shaped
            .index_for_x(at.x - bounds.left())
            .map_or(self.text.len(), |shown| self.text_at(shown));
        Some(self.to_utf16(index))
    }
}
