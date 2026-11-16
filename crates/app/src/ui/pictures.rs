//! Pictures drawn whole from their files. `img` keeps each file it loads in gpui's own store,
//! with its place in the window's atlas, until the program ends; a `Pictures` lets go those it
//! drew for one thing shown once another is shown. gpui's `ImageCache` would do it, but it is an
//! entity written while drawn, which draws the whole window again in gpui-fast.

use gpui::{App as Cx, Img, ImgResourceLoader, Resource, img};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

thread_local! {
    static SHOWN_BY: RefCell<HashMap<PathBuf, usize>> = RefCell::default();
    static LEFT: RefCell<Vec<PathBuf>> = RefCell::default();
}

#[derive(Default)]
struct Held {
    of: Option<String>,
    drawn: HashSet<PathBuf>,
}

impl Drop for Held {
    fn drop(&mut self) {
        let drawn = std::mem::take(&mut self.drawn);
        LEFT.with_borrow_mut(|left| left.extend(drawn));
    }
}

#[derive(Clone, Default)]
pub struct Pictures {
    held: Rc<RefCell<Held>>,
}

impl Pictures {
    /// The pictures of `of`, as it is drawn now. When `of` is not what was drawn before, the
    /// files drawn for that are let go once the frame is done, when no view draws them any more.
    pub fn of(&self, of: &str, cx: &mut Cx) -> Shown {
        let mut held = self.held.borrow_mut();
        if held.of.as_deref() != Some(of) {
            held.of = Some(of.to_string());
            let earlier = std::mem::take(&mut held.drawn);
            let left = LEFT.with_borrow_mut(std::mem::take);
            if !earlier.is_empty() || !left.is_empty() {
                cx.defer(move |cx| let_go(earlier.into_iter().chain(left), cx));
            }
        }
        Shown {
            held: self.held.clone(),
        }
    }
}

impl Pictures {
    pub fn kept(&self) -> Shown {
        Shown {
            held: self.held.clone(),
        }
    }
}

pub struct Shown {
    held: Rc<RefCell<Held>>,
}

impl Shown {
    pub fn img(&self, path: impl Into<PathBuf>) -> Img {
        let path = path.into();
        if self.held.borrow_mut().drawn.insert(path.clone()) {
            SHOWN_BY.with_borrow_mut(|shown| *shown.entry(path.clone()).or_default() += 1);
        }
        img(path)
    }
}

fn let_go(paths: impl Iterator<Item = PathBuf>, cx: &mut Cx) {
    for path in paths {
        let last = SHOWN_BY.with_borrow_mut(|shown| match shown.get_mut(&path) {
            Some(count) if *count > 1 => {
                *count -= 1;
                false
            }
            Some(_) => {
                shown.remove(&path);
                true
            }
            None => false,
        });
        if !last {
            continue;
        }
        let source = Resource::Path(path.into());
        if let Some(Ok(image)) = cx.fetch_asset::<ImgResourceLoader>(&source) {
            cx.drop_image(image, None);
        }
        cx.remove_asset::<ImgResourceLoader>(&source);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{IntoElement, ParentElement, Render, Styled, Window, div};
    use std::path::PathBuf;
    use std::sync::Arc;

    struct Shows {
        pictures: Pictures,
        path: PathBuf,
    }

    impl Render for Shows {
        fn render(&mut self, _: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
            let pictures = self.pictures.of(&self.path.to_string_lossy(), cx);
            div()
                .size_full()
                .child(pictures.img(self.path.clone()).size(gpui::px(8.)))
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_picture_shown_through_pictures_is_let_go_once_another_is_shown() {
        let tmp = tempfile::tempdir().unwrap();
        let paths: Vec<PathBuf> = ["a.png", "b.png"]
            .iter()
            .map(|name| {
                let path = tmp.path().join(name);
                image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 255]))
                    .save(&path)
                    .unwrap();
                path
            })
            .collect();
        let mut cx = gpui::HeadlessAppContext::with_asset_source(
            crate::ui::made_up::text_system(),
            Arc::new(crate::ui::assets::Assets),
        );
        let first = paths[0].clone();
        let window = cx
            .open_window(gpui::size(gpui::px(40.), gpui::px(40.)), |_, cx| {
                gpui::AppContext::new(cx, |_| Shows {
                    pictures: Pictures::default(),
                    path: first,
                })
            })
            .unwrap();
        let draw = |cx: &mut gpui::HeadlessAppContext| {
            for _ in 0..3 {
                cx.run_until_parked();
                cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
                    .unwrap();
            }
        };
        let source = |path: &PathBuf| Resource::Path(path.as_path().into());
        let stored = |path: &PathBuf, cx: &mut gpui::HeadlessAppContext| {
            cx.update(|cx| cx.has_asset::<ImgResourceLoader>(&source(path)))
        };
        draw(&mut cx);
        let first = cx
            .update(|cx| cx.fetch_asset::<ImgResourceLoader>(&source(&paths[0])))
            .unwrap()
            .unwrap();
        let in_atlas = |cx: &mut gpui::HeadlessAppContext| {
            cx.update_window(window.into(), |_, window, _| {
                window.has_image_atlas_entry(&first)
            })
            .unwrap()
        };
        assert!(stored(&paths[0], &mut cx) && in_atlas(&mut cx));
        let second = paths[1].clone();
        cx.update(|cx| {
            window
                .update(cx, |shows, _, cx| {
                    shows.path = second;
                    cx.notify();
                })
                .unwrap()
        });
        draw(&mut cx);
        assert!(!stored(&paths[0], &mut cx) && !in_atlas(&mut cx));
        assert!(stored(&paths[1], &mut cx));
    }
}
