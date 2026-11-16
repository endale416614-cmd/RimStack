use serde::Serialize;

/// In logical pixels. A window's position is that of its inside: the title bar is above it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

const TITLE: i32 = 40;

impl Area {
    pub fn read(text: &str) -> Option<Self> {
        let numbers: Vec<i32> = text
            .split(',')
            .map(|n| n.trim().parse().ok())
            .collect::<Option<_>>()?;
        match numbers[..] {
            [x, y, width, height] if width > 0 && height > 0 => Some(Self {
                x,
                y,
                width,
                height,
            }),
            _ => None,
        }
    }

    pub fn kept(&self) -> String {
        format!("{},{},{},{}", self.x, self.y, self.width, self.height)
    }

    fn holds(&self, x: i32, y: i32) -> bool {
        (self.x..self.x + self.width).contains(&x) && (self.y..self.y + self.height).contains(&y)
    }

    fn within(&self, other: &Area) -> bool {
        self.x >= other.x
            && self.y >= other.y
            && self.x + self.width <= other.x + other.width
            && self.y + self.height <= other.y + other.height
    }
}

/// `screens` are the parts of each screen a window may cover, the primary screen first.
/// A window kept inside a screen opens as it was; one whose top is on a screen is pulled
/// inside it; any other is centred on the primary screen.
pub fn place(kept: Option<Area>, width: i32, height: i32, screens: &[Area]) -> Area {
    let wanted = kept.unwrap_or(Area {
        x: 0,
        y: 0,
        width,
        height,
    });
    let on = kept.and_then(|k| screens.iter().find(|s| s.holds(k.x + k.width / 2, k.y)));
    let Some(screen) = on.or(screens.first()) else {
        return kept.unwrap_or_else(|| centred(width, height, None));
    };
    if on.is_some() && wanted.within(screen) {
        return wanted;
    }
    let width = wanted.width.min(screen.width);
    let height = wanted.height.min((screen.height - TITLE).max(1));
    if on.is_none() {
        return centred(width, height, Some(screen));
    }
    let (left, top) = (screen.x, screen.y + TITLE);
    let (right, bottom) = (
        screen.x + screen.width - width,
        screen.y + screen.height - height,
    );
    Area {
        x: wanted.x.clamp(left, right),
        y: wanted.y.clamp(top, bottom),
        width,
        height,
    }
}

/// At that size even where the screen is smaller, its title bar kept on the screen.
pub fn centred(width: i32, height: i32, screen: Option<&Area>) -> Area {
    let screen = screen.copied().unwrap_or_default();
    Area {
        x: screen.x + ((screen.width - width) / 2).max(0),
        y: screen.y + TITLE + ((screen.height - TITLE - height) / 2).max(0),
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn area(x: i32, y: i32, width: i32, height: i32) -> Area {
        Area {
            x,
            y,
            width,
            height,
        }
    }

    const LAPTOP: Area = area(0, 0, 1920, 1032);
    const BESIDE: Area = area(1920, 0, 2560, 1400);

    #[test]
    fn a_window_that_fits_where_it_was_opens_there() {
        let kept = area(300, 200, 1200, 700);
        assert_eq!(place(Some(kept), 1440, 860, &[LAPTOP]), kept);
        let there = area(2100, 300, 1600, 900);
        assert_eq!(place(Some(there), 1440, 860, &[LAPTOP, BESIDE]), there);
        let snapped = area(0, 0, 960, 1032);
        assert_eq!(place(Some(snapped), 1440, 860, &[LAPTOP]), snapped);
        let at_the_top = area(400, 8, 1200, 700);
        assert_eq!(place(Some(at_the_top), 1440, 860, &[LAPTOP]), at_the_top);
    }

    #[test]
    fn a_window_past_the_screen_s_bottom_is_moved_up_and_made_to_fit() {
        let placed = place(Some(area(200, 400, 1440, 900)), 1440, 860, &[LAPTOP]);
        assert_eq!(placed, area(200, 132, 1440, 900));
        let taller = place(Some(area(200, 100, 1440, 1400)), 1440, 860, &[LAPTOP]);
        assert_eq!(taller, area(200, TITLE, 1440, 1032 - TITLE));
    }

    #[test]
    fn a_window_on_a_screen_now_gone_is_centred_on_the_primary_one() {
        let kept = area(3000, 200, 1440, 860);
        assert_eq!(
            place(Some(kept), 1440, 860, &[LAPTOP]),
            area(240, TITLE + 66, 1440, 860)
        );
        let minimized = area(-32000, -32000, 160, 28);
        assert_eq!(place(Some(minimized), 1440, 860, &[LAPTOP]).x, 880);
    }

    #[test]
    fn a_first_window_is_centred_and_fits_its_screen_but_centring_alone_keeps_its_size() {
        assert_eq!(
            place(None, 1440, 860, &[LAPTOP]),
            area(240, TITLE + 66, 1440, 860)
        );
        let small = area(0, 0, 1280, 680);
        assert_eq!(
            place(None, 1440, 860, &[small]),
            area(0, TITLE, 1280, 680 - TITLE)
        );
        let narrow = area(0, 0, 800, 600);
        assert_eq!(centred(1440, 860, Some(&narrow)), area(0, TITLE, 1440, 860));
        assert_eq!(centred(400, 300, Some(&BESIDE)), area(3000, 570, 400, 300));
    }

    #[test]
    fn with_no_screen_known_the_window_is_left_as_kept() {
        let kept = area(10, 20, 800, 600);
        assert_eq!(place(Some(kept), 1440, 860, &[]), kept);
        assert_eq!(place(None, 1440, 860, &[]), area(0, TITLE, 1440, 860));
    }

    #[test]
    fn an_area_is_kept_as_four_numbers() {
        let kept = area(-8, 31, 1200, 700);
        assert_eq!(Area::read(&kept.kept()), Some(kept));
        for damaged in ["", "1,2,3", "1,2,3,x", "1,2,0,4", "1,2,3,4,5"] {
            assert_eq!(Area::read(damaged), None, "{damaged}");
        }
    }
}
