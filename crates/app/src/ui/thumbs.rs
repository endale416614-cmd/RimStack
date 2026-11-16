//! Previews decoded off the window's thread at the size they are shown, kept in memory up to a
//! size. GPUI's `img` keeps a picture whole: 2,000 mods' previews held that way took 150 MB.

use gpui::{App as Cx, EntityId, RenderImage};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

/// Bytes of decoded pictures kept at least: about 1,300 tiles at 2x. A longer list's pictures
/// are all kept, with `ROOM_BYTES` more beside them for those scrolled to, up to
/// `CEILING_BYTES`.
const MOST_BYTES: usize = 48 << 20;
const DECODING: usize = 4;
const ROOM_BYTES: usize = MOST_BYTES / 4;
/// Bytes kept at most, however long the list: a list of 2,000 at 2x and text size 1 is kept
/// whole (about 86 MB); past it, rows a jump of the scroll bar reaches are decoded as they show.
const CEILING_BYTES: usize = 128 << 20;
/// Pictures waiting to be decoded, the rows in view and three pages ahead of them; past it the
/// oldest asked, scrolled by, are dropped.
const WAITING: usize = 128;

type Size = (u32, u32);
type Key = (Size, String);
type Viewers = smallvec::SmallVec<[EntityId; 2]>;

struct Kept {
    image: Arc<RenderImage>,
    at: u64,
    by: Viewers,
}

pub struct Thumbs {
    shown: HashMap<Key, Kept>,
    /// Pictures let go, still in the window's atlas until `flush`, and the views to draw again
    /// without them.
    gone: Vec<(Arc<RenderImage>, Viewers)>,
    by_use: BTreeMap<u64, Key>,
    uses: u64,
    bytes: usize,
    /// Pictures that could not be read: not tried again until `renew`.
    unreadable: HashSet<Key>,
    /// Decoded from the end: the rows in view are pushed there, those ahead of them put first.
    waiting: Vec<Key>,
    /// Asked and not decoded yet, with the view to draw again once it is, if one shows it.
    asked: HashMap<Key, Option<EntityId>>,
    decoding: usize,
    /// The whole list's pictures, decoded from the end while nothing else waits, so that a
    /// jump of the scroll bar finds them; and which rows they were taken from.
    idle: Vec<Key>,
    idle_of: Option<(usize, Size)>,
    /// Bytes kept: `MOST_BYTES`, or the list's pictures and `ROOM_BYTES` when more, up to
    /// `CEILING_BYTES`.
    most: usize,
}

impl Default for Thumbs {
    fn default() -> Self {
        Self {
            shown: HashMap::new(),
            gone: Vec::new(),
            by_use: BTreeMap::new(),
            uses: 0,
            bytes: 0,
            unreadable: HashSet::new(),
            waiting: Vec::new(),
            asked: HashMap::new(),
            decoding: 0,
            idle: Vec::new(),
            idle_of: None,
            most: MOST_BYTES,
        }
    }
}

pub type Shared = Rc<RefCell<Thumbs>>;

impl Thumbs {
    /// The pictures may have changed on disk: each is read again when next shown.
    fn renew(&mut self) {
        let shown = std::mem::take(&mut self.shown);
        self.gone
            .extend(shown.into_values().map(|kept| (kept.image, kept.by)));
        self.by_use.clear();
        self.bytes = 0;
        self.unreadable.clear();
        self.idle_of = None;
    }

    /// `pictures`, last first, the list's to decode while idle, with room kept for them all;
    /// false when they are the ones already taken.
    fn take_list(&mut self, id: usize, size: Size, pictures: impl Iterator<Item = Key>) -> bool {
        if self.idle_of == Some((id, size)) {
            return false;
        }
        self.idle_of = Some((id, size));
        self.idle = pictures.collect();
        let whole = self.idle.len() * tile_bytes(size) + ROOM_BYTES;
        self.most = whole.clamp(MOST_BYTES, CEILING_BYTES);
        true
    }

    fn next_to_decode(&mut self) -> Option<Key> {
        if let Some(key) = self.waiting.pop() {
            return Some(key);
        }
        while self.bytes + ROOM_BYTES < self.most {
            let key = self.idle.pop()?;
            if !self.shown.contains_key(&key)
                && !self.unreadable.contains(&key)
                && !self.asked.contains_key(&key)
            {
                self.asked.insert(key.clone(), None);
                return Some(key);
            }
        }
        None
    }

    fn used(&mut self) -> u64 {
        self.uses += 1;
        self.uses
    }

    fn hit(&mut self, key: &Key, by: EntityId) -> Option<Arc<RenderImage>> {
        let now = self.uses + 1;
        let kept = self.shown.get_mut(key)?;
        let was = std::mem::replace(&mut kept.at, now);
        if !kept.by.contains(&by) {
            kept.by.push(by);
        }
        let shown = kept.image.clone();
        self.uses = now;
        if let Some(key) = self.by_use.remove(&was) {
            self.by_use.insert(now, key);
        }
        Some(shown)
    }

    fn keep(&mut self, key: Key, image: Arc<RenderImage>, by: Option<EntityId>) {
        let at = self.used();
        let bytes = tile_bytes(key.0);
        self.by_use.insert(at, key.clone());
        let by = by.into_iter().collect();
        match self.shown.insert(key, Kept { image, at, by }) {
            Some(was) => {
                self.by_use.remove(&was.at);
                self.gone.push((was.image, was.by));
            }
            None => self.bytes += bytes,
        }
        while self.bytes > self.most {
            let Some((_, old)) = self.by_use.pop_first() else {
                break;
            };
            if let Some(kept) = self.shown.remove(&old) {
                self.bytes -= tile_bytes(old.0);
                self.gone.push((kept.image, kept.by));
            }
        }
    }

    fn ask_shown(&mut self, key: Key, shown_by: EntityId) {
        if let Some(shows) = self.asked.get_mut(&key) {
            *shows = Some(shown_by);
            if let Some(at) = self.waiting.iter().position(|waits| *waits == key) {
                let key = self.waiting.remove(at);
                self.waiting.push(key);
            }
            return;
        }
        self.asked.insert(key.clone(), Some(shown_by));
        self.waiting.push(key);
        if self.waiting.len() > WAITING {
            let dropped = self.waiting.remove(0);
            self.asked.remove(&dropped);
        }
    }

    /// `key` asked for a row about to be scrolled to: after every row in view, and not at all
    /// when as many wait already.
    fn ask_ahead(&mut self, key: Key) -> bool {
        if self.waiting.len() >= WAITING
            || self.shown.contains_key(&key)
            || self.unreadable.contains(&key)
            || self.asked.contains_key(&key)
        {
            return false;
        }
        self.asked.insert(key.clone(), None);
        self.waiting.insert(0, key);
        true
    }
}

#[cfg(all(test, target_os = "linux"))]
impl Thumbs {
    pub(crate) fn awaited_in_view(&self) -> usize {
        self.asked.values().filter(|shows| shows.is_some()).count()
    }

    pub(crate) fn decoded(&self) -> usize {
        self.shown.len()
    }
}

/// The window's atlas keeps every picture ever drawn until it is told to let one go. A view
/// that drew one let go is drawn again in the same update, before any frame could copy its
/// last frame's sprites; so this runs only outside drawing.
fn flush(thumbs: &Shared, cx: &mut Cx) {
    let gone = std::mem::take(&mut thumbs.borrow_mut().gone);
    for (image, by) in gone {
        cx.drop_image(image, None);
        for view in by {
            cx.notify(view);
        }
    }
}

/// The pictures may have changed on disk: each is read again when next shown, and those held
/// leave the atlas.
pub fn renew(thumbs: &Shared, cx: &mut Cx) {
    thumbs.borrow_mut().renew();
    flush(thumbs, cx);
}

fn tile_bytes((width, height): Size) -> usize {
    width as usize * height as usize * 4
}

/// The picture at `path` cut to fill `size` device pixels, once decoded; `shown_by` is drawn
/// again when it is.
pub fn thumb(
    thumbs: &Shared,
    path: &str,
    size: Size,
    shown_by: EntityId,
    cx: &mut Cx,
) -> Option<Arc<RenderImage>> {
    let key = (size, path.to_string());
    let mut held = thumbs.borrow_mut();
    if let Some(shown) = held.hit(&key, shown_by) {
        return Some(shown);
    }
    if held.unreadable.contains(&key) {
        return None;
    }
    held.ask_shown(key, shown_by);
    drop(held);
    decode_next(thumbs, cx);
    None
}

pub fn ahead(thumbs: &Shared, path: &str, size: Size, cx: &mut Cx) {
    let asked = thumbs.borrow_mut().ask_ahead((size, path.to_string()));
    if asked {
        decode_next(thumbs, cx);
    }
}

/// Every picture of `rows`, the rows' `id` (the same until they change), decoded while the
/// window has nothing else to decode.
pub fn while_idle<'a>(
    thumbs: &Shared,
    id: usize,
    pictures: impl DoubleEndedIterator<Item = &'a str>,
    size: Size,
    cx: &mut Cx,
) {
    let pictures = pictures.rev().map(|path| (size, path.to_string()));
    if !thumbs.borrow_mut().take_list(id, size, pictures) {
        return;
    }
    for _ in 0..DECODING {
        decode_next(thumbs, cx);
    }
}

fn decode_next(thumbs: &Shared, cx: &mut Cx) {
    let mut held = thumbs.borrow_mut();
    if held.decoding >= DECODING {
        return;
    }
    let Some(key) = held.next_to_decode() else {
        return;
    };
    held.decoding += 1;
    drop(held);
    let (size, from) = (key.0, PathBuf::from(&key.1));
    let decoding = cx
        .background_executor()
        .spawn(async move { rimstack_core::base::thumbnail::crop(&from, size) });
    let thumbs = thumbs.clone();
    cx.spawn(async move |cx| {
        let made = decoding.await;
        let shown_by = {
            let mut held = thumbs.borrow_mut();
            held.decoding -= 1;
            let shown_by = held.asked.remove(&key).flatten();
            match made.and_then(|pixels| picture(pixels, size)) {
                Some(made) => held.keep(key, Arc::new(made), shown_by),
                None => {
                    held.unreadable.insert(key);
                }
            }
            shown_by
        };
        cx.update(|cx| {
            if let Some(view) = shown_by {
                cx.notify(view);
            }
            flush(&thumbs, cx);
            decode_next(&thumbs, cx);
        });
    })
    .detach();
}

fn picture(pixels: Vec<u8>, (width, height): Size) -> Option<RenderImage> {
    let frame = image::RgbaImage::from_raw(width, height, pixels)?;
    Some(RenderImage::new(smallvec::smallvec![image::Frame::new(
        frame
    )]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_oldest_pictures_go_past_the_most_bytes() {
        let mut thumbs = Thumbs::default();
        let size = (512, 512);
        let fits = MOST_BYTES / tile_bytes(size);
        for at in 0..fits + 3 {
            let made = picture(vec![0; tile_bytes(size)], size).unwrap();
            thumbs.keep((size, at.to_string()), Arc::new(made), None);
        }
        assert!(thumbs.bytes <= MOST_BYTES);
        assert_eq!(thumbs.shown.len(), fits);
        assert!(!thumbs.shown.contains_key(&(size, "0".to_string())));
        assert!(thumbs.shown.contains_key(&(size, (fits + 2).to_string())));
    }

    #[test]
    fn every_picture_of_a_list_longer_than_the_least_kept_is_kept() {
        let mut thumbs = Thumbs::default();
        let size = (512, 512);
        let count = MOST_BYTES / tile_bytes(size) * 2;
        let keys = (0..count).rev().map(|at| (size, at.to_string()));
        assert!(thumbs.take_list(1, size, keys));
        while let Some(key) = thumbs.next_to_decode() {
            thumbs.asked.remove(&key);
            let made = picture(vec![0; tile_bytes(size)], size).unwrap();
            thumbs.keep(key, Arc::new(made), None);
        }
        assert_eq!(thumbs.shown.len(), count);
        assert!(thumbs.bytes + ROOM_BYTES <= thumbs.most);
    }

    #[test]
    fn a_list_too_long_to_keep_whole_keeps_no_more_than_the_ceiling() {
        let mut thumbs = Thumbs::default();
        let size = (512, 512);
        let count = CEILING_BYTES / tile_bytes(size) * 2;
        let keys = (0..count).rev().map(|at| (size, at.to_string()));
        assert!(thumbs.take_list(1, size, keys));
        while let Some(key) = thumbs.next_to_decode() {
            thumbs.asked.remove(&key);
            let made = picture(vec![0; tile_bytes(size)], size).unwrap();
            thumbs.keep(key, Arc::new(made), None);
        }
        assert!(thumbs.bytes <= CEILING_BYTES);
        assert!(
            thumbs.shown.contains_key(&(size, "0".to_string())),
            "the top first"
        );
        assert!(thumbs.gone.is_empty(), "nothing decoded only to be dropped");
    }

    #[test]
    fn a_picture_let_go_leaves_with_the_views_that_drew_it_and_renew_lets_all_go() {
        let mut thumbs = Thumbs::default();
        let size = (512, 512);
        let fits = MOST_BYTES / tile_bytes(size);
        let key = |at: usize| (size, at.to_string());
        let (first, second) = (EntityId::from(1), EntityId::from(2));
        for at in 0..fits {
            let made = picture(vec![0; tile_bytes(size)], size).unwrap();
            thumbs.keep(key(at), Arc::new(made), Some(first));
        }
        assert!(thumbs.hit(&key(0), second).is_some());
        assert!(thumbs.gone.is_empty());
        let made = picture(vec![0; tile_bytes(size)], size).unwrap();
        thumbs.keep(key(fits), Arc::new(made), None);
        assert_eq!(thumbs.gone.len(), 1);
        assert_eq!(thumbs.gone[0].1.as_slice(), [first]);
        thumbs.renew();
        assert_eq!(thumbs.gone.len(), fits + 1);
        assert!(
            thumbs
                .gone
                .iter()
                .any(|(_, by)| by.as_slice() == [first, second])
        );
        assert!(thumbs.shown.is_empty() && thumbs.bytes == 0);
    }

    struct Drawer {
        thumbs: Shared,
        key: Key,
        drawn: usize,
    }

    impl gpui::Render for Drawer {
        fn render(
            &mut self,
            _: &mut gpui::Window,
            cx: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            use gpui::{ParentElement, Styled};
            self.drawn += 1;
            let shown = self.thumbs.borrow_mut().hit(&self.key, cx.entity_id());
            gpui::div()
                .size_full()
                .children(shown.map(|image| gpui::img(image).size(gpui::px(8.))))
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn renewed_pictures_leave_the_atlas_and_their_views_are_drawn_again() {
        let mut cx = gpui::HeadlessAppContext::with_asset_source(
            crate::ui::made_up::text_system(),
            Arc::new(crate::ui::assets::Assets),
        );
        let size = (4, 4);
        let key: Key = (size, "a".to_string());
        let image = Arc::new(picture(vec![255; tile_bytes(size)], size).unwrap());
        let thumbs = Shared::default();
        thumbs.borrow_mut().keep(key.clone(), image.clone(), None);
        let window = cx
            .open_window(gpui::size(gpui::px(40.), gpui::px(40.)), |_, cx| {
                gpui::AppContext::new(cx, |_| Drawer {
                    thumbs: thumbs.clone(),
                    key,
                    drawn: 0,
                })
            })
            .unwrap();
        let draw = |cx: &mut gpui::HeadlessAppContext| {
            cx.run_until_parked();
            cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
                .unwrap();
        };
        let in_atlas = |cx: &mut gpui::HeadlessAppContext| {
            cx.update_window(window.into(), |_, window, _| {
                window.has_image_atlas_entry(&image)
            })
            .unwrap()
        };
        let drawn =
            |cx: &mut gpui::HeadlessAppContext| cx.update(|cx| window.read(cx).unwrap().drawn);
        draw(&mut cx);
        assert!(in_atlas(&mut cx));
        let before = drawn(&mut cx);
        cx.update(|cx| renew(&thumbs, cx));
        assert!(!in_atlas(&mut cx));
        draw(&mut cx);
        assert_eq!(drawn(&mut cx), before + 1);
        assert!(!in_atlas(&mut cx));
    }

    #[test]
    fn a_picture_shown_again_is_kept_over_older_ones() {
        let mut thumbs = Thumbs::default();
        let size = (512, 512);
        let fits = MOST_BYTES / tile_bytes(size);
        let key = |at: usize| (size, at.to_string());
        for at in 0..fits {
            let made = picture(vec![0; tile_bytes(size)], size).unwrap();
            thumbs.keep(key(at), Arc::new(made), None);
        }
        assert!(thumbs.hit(&key(0), EntityId::from(1)).is_some());
        let made = picture(vec![0; tile_bytes(size)], size).unwrap();
        thumbs.keep(key(fits), Arc::new(made), None);
        assert!(thumbs.shown.contains_key(&key(0)));
        assert!(!thumbs.shown.contains_key(&key(1)));
        assert_eq!(thumbs.by_use.len(), thumbs.shown.len());
    }

    #[test]
    fn rows_ahead_wait_behind_the_rows_in_view_and_gain_a_view_once_shown() {
        let mut cx = gpui::HeadlessAppContext::with_asset_source(
            crate::ui::made_up::text_system(),
            Arc::new(crate::ui::assets::Assets),
        );
        let view = cx.update(|cx| gpui::AppContext::new(cx, |_| ()).entity_id());
        let mut thumbs = Thumbs::default();
        let key = |name: &str| ((4, 4), name.to_string());
        assert!(thumbs.ask_ahead(key("ahead")));
        thumbs.ask_shown(key("in view"), view);
        assert!(!thumbs.ask_ahead(key("in view")));
        assert_eq!(thumbs.waiting.last(), Some(&key("in view")));
        assert_eq!(thumbs.asked[&key("ahead")], None);
        thumbs.ask_shown(key("ahead"), view);
        assert_eq!(thumbs.waiting.last(), Some(&key("ahead")));
        assert_eq!(thumbs.asked[&key("ahead")], Some(view));
    }

    /// How long the pictures a page and a half ahead take to decode, page after page through
    /// both lists, at 2x and with as many decodes at once as the window runs.
    #[test]
    #[ignore = "reads this machine's own install; RIMSTACK_TIME_HOME names a scratch folder"]
    fn times_decoding_the_pages_ahead_on_this_machines_install() {
        use crate::workspace::load_order::{LoadOrder, Side};
        let Ok(scratch) = std::env::var("RIMSTACK_TIME_HOME") else {
            return;
        };
        let scratch = PathBuf::from(scratch);
        let home = PathBuf::from(std::env::var("HOME").unwrap());
        let app = crate::App::new(
            home,
            &scratch.join("settings"),
            &scratch.join("data"),
            Default::default(),
        );
        let order = LoadOrder::read(&app, true, None);
        const AHEAD: usize = 25 * 3 / 2;
        let size = (124, 76);
        for side in [Side::Active, Side::Inactive] {
            let rows = order.rows(&app, side);
            let pictures: Vec<PathBuf> = rows
                .iter()
                .filter(|row| !row.is_header && !row.picture.is_empty())
                .map(|row| PathBuf::from(&row.picture))
                .collect();
            let mut took: Vec<std::time::Duration> = pictures
                .chunks(AHEAD)
                .map(|ahead| {
                    let started = std::time::Instant::now();
                    std::thread::scope(|scope| {
                        for share in ahead.chunks(ahead.len().div_ceil(DECODING)) {
                            scope.spawn(move || {
                                for path in share {
                                    let _ = rimstack_core::base::thumbnail::crop(path, size);
                                }
                            });
                        }
                    });
                    started.elapsed()
                })
                .collect();
            took.sort();
            let at = |q: f32| took[((took.len() - 1) as f32 * q) as usize];
            eprintln!(
                "{side:?}: {} rows, {} pictures, {} pages ahead: median {:?}, p90 {:?}, worst {:?}",
                rows.len(),
                pictures.len(),
                took.len(),
                at(0.5),
                at(0.9),
                at(1.0)
            );
        }
    }
}
