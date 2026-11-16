//! The window's sections: Mods and Downloads made with the window, the others the first time
//! they are asked for; each told when it is shown or hidden, and one made while another shows
//! is told it is hidden. A section needs none of the others to be made.

use super::downloads::Downloads;
use super::find::Find;
use super::logs::Logs;
use super::mods::Mods;
use super::saves::Saves;
use super::section::{Section, Shown};
use super::settings::Settings;
use super::textures::Textures;
use super::workshop::Workshop;
use gpui::{AnyView, App as Cx, AppContext, Context, Entity};

pub struct Start {
    pub demo: bool,
    pub part: usize,
    pub view: usize,
}

pub trait SectionView: gpui::Render + Shown + Sized {
    const AT: Section;
    fn made(start: &Start, cx: &mut Context<Self>) -> Self;
}

struct Made {
    view: AnyView,
    tell: fn(&AnyView, bool, &mut Cx),
}

fn tell<V: SectionView>(view: &AnyView, shown: bool, cx: &mut Cx) {
    if let Ok(view) = view.clone().downcast::<V>() {
        view.update(cx, |view, cx| view.set_shown(shown, cx));
    }
}

pub struct Sections {
    pub(crate) start: Start,
    shown: Section,
    pub(crate) mods: Entity<Mods>,
    made: [Option<Made>; Section::ALL.len()],
}

impl Sections {
    pub fn new(start: Start, cx: &mut Cx) -> Self {
        let mut sections = Self {
            mods: cx.new(|cx| Mods::made(&start, cx)),
            start,
            shown: Section::Mods,
            made: Default::default(),
        };
        let mods = sections.mods.clone();
        sections.keep(mods);
        sections.get::<Downloads>(cx);
        sections
    }

    pub fn shown(&self) -> Section {
        self.shown
    }

    pub fn view_shown(&self) -> Option<AnyView> {
        let made = self.made[self.shown.index()].as_ref()?;
        Some(made.view.clone())
    }

    pub fn get<V: SectionView>(&mut self, cx: &mut Cx) -> Entity<V> {
        if let Some(view) = self.made::<V>() {
            return view;
        }
        let view = cx.new(|cx| V::made(&self.start, cx));
        self.keep(view.clone());
        if V::AT != self.shown {
            view.update(cx, |view, cx| view.set_shown(false, cx));
        }
        view
    }

    pub fn made<V: SectionView>(&self) -> Option<Entity<V>> {
        let made = self.made[V::AT.index()].as_ref()?;
        made.view.clone().downcast::<V>().ok()
    }

    fn keep<V: SectionView>(&mut self, view: Entity<V>) {
        self.made[V::AT.index()] = Some(Made {
            view: view.into(),
            tell: tell::<V>,
        });
    }

    pub fn show(&mut self, section: Section, cx: &mut Cx) {
        if section != self.shown {
            self.tell_shown(self.shown, false, cx);
        }
        self.shown = section;
        match section {
            Section::Mods => {}
            Section::Downloads => drop(self.get::<Downloads>(cx)),
            Section::Textures => drop(self.get::<Textures>(cx)),
            Section::Workshop => drop(self.get::<Workshop>(cx)),
            Section::Saves => drop(self.get::<Saves>(cx)),
            Section::Logs => drop(self.get::<Logs>(cx)),
            Section::Find => drop(self.get::<Find>(cx)),
            Section::Settings => drop(self.get::<Settings>(cx)),
        }
        self.tell_shown(section, true, cx);
    }

    fn tell_shown(&self, section: Section, shown: bool, cx: &mut Cx) {
        if let Some(made) = &self.made[section.index()] {
            (made.tell)(&made.view, shown, cx);
        }
    }
}

impl Shown for Mods {
    fn set_shown(&mut self, _: bool, _: &mut Context<Self>) {}
}

impl Shown for Downloads {
    fn set_shown(&mut self, _: bool, _: &mut Context<Self>) {}
}

impl SectionView for Mods {
    const AT: Section = Section::Mods;
    fn made(start: &Start, cx: &mut Context<Self>) -> Self {
        let mods = Mods::new(start.demo, cx);
        let list = mods.list.clone();
        cx.observe(&list, |_, list, cx| {
            super::around::from_list(&list, cx);
            super::order::from_list(&list, cx);
        })
        .detach();
        super::around::from_list(&list, cx);
        super::order::from_list(&list, cx);
        mods
    }
}

impl SectionView for Downloads {
    const AT: Section = Section::Downloads;
    fn made(start: &Start, cx: &mut Context<Self>) -> Self {
        cx.observe_self(|downloads, cx| {
            let work = downloads.tasks_said();
            super::around::change(cx, |around| around.downloads_work = work);
        })
        .detach();
        Downloads::new(start.demo, cx)
    }
}

impl SectionView for Textures {
    const AT: Section = Section::Textures;
    fn made(start: &Start, cx: &mut Context<Self>) -> Self {
        cx.observe_self(|textures, cx| {
            let work = textures.task_said();
            super::around::change(cx, |around| around.textures_work = work);
        })
        .detach();
        Textures::new(start.demo, cx)
    }
}

impl SectionView for Workshop {
    const AT: Section = Section::Workshop;
    fn made(start: &Start, cx: &mut Context<Self>) -> Self {
        Workshop::new(start.demo, cx)
    }
}

impl SectionView for Saves {
    const AT: Section = Section::Saves;
    fn made(start: &Start, cx: &mut Context<Self>) -> Self {
        Saves::new(start.demo, cx)
    }
}

impl SectionView for Logs {
    const AT: Section = Section::Logs;
    fn made(start: &Start, cx: &mut Context<Self>) -> Self {
        Logs::new(start.demo, start.view, cx)
    }
}

impl SectionView for Find {
    const AT: Section = Section::Find;
    fn made(start: &Start, cx: &mut Context<Self>) -> Self {
        Find::new(start.demo, cx)
    }
}

impl SectionView for Settings {
    const AT: Section = Section::Settings;
    fn made(start: &Start, cx: &mut Context<Self>) -> Self {
        cx.observe_self(|settings, cx| {
            let busy = settings.community_busy();
            super::around::change(cx, |around| around.community_busy = busy);
        })
        .detach();
        Settings::new(start.demo, start.part, cx)
    }
}
