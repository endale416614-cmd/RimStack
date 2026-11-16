use gpui::{Bounds, Pixels, Point, SharedString, TextRun, Window, WrappedLine, point, px};

pub struct Wrapped {
    lines: Vec<(usize, WrappedLine)>,
    line: Pixels,
}

impl Wrapped {
    pub fn shape(
        window: &Window,
        text: &str,
        size: Pixels,
        line: Pixels,
        run: TextRun,
        width: Pixels,
    ) -> Self {
        let mut lines = Vec::new();
        let mut start = 0;
        for piece in text.split('\n') {
            let run = TextRun {
                len: piece.len(),
                ..run.clone()
            };
            let shaped = window
                .text_system()
                .shape_text(
                    SharedString::from(piece.to_string()),
                    size,
                    &[run],
                    Some(width),
                    None,
                )
                .ok()
                .and_then(|mut shaped| shaped.drain(..).next())
                .unwrap_or_default();
            lines.push((start, shaped));
            start += piece.len() + 1;
        }
        Self { lines, line }
    }

    fn rows_of(line: &WrappedLine) -> usize {
        line.wrap_boundaries().len() + 1
    }

    pub fn rows(&self) -> usize {
        self.lines.iter().map(|(_, line)| Self::rows_of(line)).sum()
    }

    pub fn position(&self, at: usize) -> Point<Pixels> {
        let mut top = px(0.);
        for (start, line) in &self.lines {
            if at <= start + line.len() {
                let inside = line
                    .position_for_index(at - start, self.line)
                    .unwrap_or_default();
                return point(inside.x, top + inside.y);
            }
            top += self.line * Self::rows_of(line) as f32;
        }
        point(px(0.), top)
    }

    pub fn index_at(&self, at: Point<Pixels>) -> usize {
        let mut top = px(0.);
        for (start, line) in &self.lines {
            let tall = self.line * Self::rows_of(line) as f32;
            if at.y < top + tall {
                let inside = point(at.x.max(px(0.)), (at.y - top).max(px(0.)));
                let found = line.closest_index_for_position(inside, self.line);
                return start + found.unwrap_or_else(|end| end);
            }
            top += tall;
        }
        self.lines
            .last()
            .map_or(0, |(start, line)| start + line.len())
    }

    pub fn cover(&self, from: usize, to: usize, width: Pixels) -> Vec<Bounds<Pixels>> {
        let (start, end) = (self.position(from), self.position(to));
        let rows = ((end.y - start.y) / self.line).round() as usize;
        (0..=rows)
            .map(|row| {
                let top = start.y + self.line * row as f32;
                let left = if row == 0 { start.x } else { px(0.) };
                let right = if row == rows { end.x } else { width };
                Bounds::from_corners(
                    point(left, top),
                    point(right.max(left + px(3.)), top + self.line),
                )
            })
            .collect()
    }

    pub fn paint(&self, origin: Point<Pixels>, window: &mut Window, cx: &mut gpui::App) {
        let mut top = origin.y;
        for (_, line) in &self.lines {
            let _ = line.paint(
                point(origin.x, top),
                self.line,
                gpui::TextAlign::Left,
                None,
                window,
                cx,
            );
            top += self.line * Self::rows_of(line) as f32;
        }
    }
}
