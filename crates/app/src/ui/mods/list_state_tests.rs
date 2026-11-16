use super::edit_tests::opened_with;
use crate::workspace::load_order::Side;

#[test]
fn moving_a_divider_leaves_the_load_order_model_and_what_follows_it_alone() {
    use super::panes::Grip;
    use std::cell::Cell;
    use std::rc::Rc;
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    cx.run_until_parked();
    let told = Rc::new(Cell::new(0));
    cx.update(|cx| {
        let list = mods.read(cx).list.clone();
        let told = told.clone();
        cx.observe(&list, move |_, _| told.set(told.get() + 1))
            .detach();
    });
    let before = cx.update(|cx| crate::ui::order::read(cx));
    let right = gpui::KeyDownEvent {
        keystroke: gpui::Keystroke::parse("right").unwrap(),
        is_held: false,
        prefer_character_input: false,
    };
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            let was = mods.layout.inactive_width;
            mods.nudge_grip(Grip::Inactive, &right, cx);
            assert_ne!(mods.layout.inactive_width, was);
            let area = gpui::Bounds::new(
                gpui::point(gpui::px(0.), gpui::px(0.)),
                gpui::size(gpui::px(1440.), gpui::px(800.)),
            );
            mods.drag_grip(
                Grip::Details,
                gpui::point(gpui::px(1000.), gpui::px(10.)),
                area,
                cx,
            );
        })
    });
    cx.run_until_parked();
    assert_eq!(told.get(), 0, "a divider moved told the load order model");
    assert!(
        cx.update(|cx| crate::ui::order::read(cx)) == before,
        "a divider moved changed what the other sections read of the load order"
    );
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            let last = mods.list.read(cx).count(Side::Active) - 1;
            mods.take(Side::Active, last, None, cx);
        })
    });
    cx.run_until_parked();
    assert!(told.get() > 0, "an edit did not tell the load order model");
}
