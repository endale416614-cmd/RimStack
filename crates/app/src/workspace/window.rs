use super::App;
use rimstack_core::base::error::Error;
use rimstack_core::base::window::{Area, centred, place};

const WIDTH: i32 = 1440;
const HEIGHT: i32 = 860;

impl App {
    pub fn window_start(&self, screens: &[Area]) -> (Area, bool) {
        let settings = self.settings();
        let kept = settings.get("window").and_then(Area::read);
        let maximized = settings.get("windowMaximized").is_some();
        (place(kept, WIDTH, HEIGHT, screens), maximized)
    }

    pub fn keep_window(&self, area: Area, maximized: bool) -> Result<(), Error> {
        if maximized {
            return self.set_setting("windowMaximized", "yes");
        }
        self.set_setting("window", &area.kept())?;
        self.set_setting("windowMaximized", "")
    }

    /// A run that leaves the kept window alone: the size asked for, or the first one.
    pub fn window_unkept(size: Option<(i32, i32)>, screens: &[Area]) -> Area {
        let (width, height) = size.unwrap_or((WIDTH, HEIGHT));
        centred(width, height, screens.first())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::tests::installed;

    const SCREEN: Area = Area {
        x: 0,
        y: 0,
        width: 2560,
        height: 1400,
    };

    #[test]
    fn the_window_opens_where_it_was_closed_and_maximized_keeps_the_area_before() {
        let (_tmp, app) = installed();
        let first = app.window_start(&[SCREEN]);
        assert_eq!(
            (first.0.width, first.0.height, first.1),
            (WIDTH, HEIGHT, false)
        );
        let moved = Area {
            x: 300,
            y: 200,
            width: 1200,
            height: 800,
        };
        app.keep_window(moved, false).unwrap();
        assert_eq!(app.window_start(&[SCREEN]), (moved, false));
        let full = Area {
            x: 0,
            y: 0,
            width: 2560,
            height: 1400,
        };
        app.keep_window(full, true).unwrap();
        assert_eq!(app.window_start(&[SCREEN]), (moved, true));
        app.keep_window(moved, false).unwrap();
        assert_eq!(app.window_start(&[SCREEN]), (moved, false));
    }

    #[test]
    fn a_run_that_leaves_the_kept_window_alone_takes_the_size_asked_for() {
        let small = Area {
            width: 800,
            height: 600,
            ..SCREEN
        };
        assert_eq!(App::window_unkept(Some((1000, 560)), &[small]).width, 1000);
        assert_eq!(App::window_unkept(None, &[small]).height, HEIGHT);
    }
}
