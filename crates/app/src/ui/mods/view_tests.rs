use super::edit_tests::opened_with;
use crate::workspace::load_order::Side;
use rimstack_core::mods::library::ModId;
use rimstack_core::mods::list::SortKey;
use std::time::{Duration, Instant};

#[test]
fn the_view_chosen_is_kept_and_read_again_with_the_mods() {
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            mods.set_headings(false, cx);
            assert!(!mods.list.read(cx).order.headings());
            mods.toggle_column(Side::Active, "updated", cx);
            mods.order_inactive("name down", cx);
            assert_eq!(
                mods.list.read(cx).order.inactive_by(),
                Some((SortKey::Name, true))
            );
            mods.heading_pressed(Side::Active, "name", cx);
            assert_eq!(mods.orders.active_key, "name");
            let kept = crate::ui::app(cx).mods_view();
            assert!(!kept.headings);
            assert_eq!(kept.columns, "author,version,load,updated");
            assert_eq!(kept.inactive_order, "name down");
            mods.load(true, cx);
            mods.edit_list(cx, |list, _| {
                list.order = crate::workspace::load_order::LoadOrder::demo(0, 0)
            });
        })
    });
    let started = Instant::now();
    while cx.update(|cx| mods.read(cx).list.read(cx).count(Side::Active) == 0) {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the mods were not read again"
        );
        crate::ui::made_up::step(&cx);
    }
    cx.update(|cx| {
        let mods = mods.read(cx);
        assert!(
            !mods.list.read(cx).order.headings(),
            "a reread keeps one numbered list"
        );
        assert_eq!(
            mods.list.read(cx).order.inactive_by(),
            Some((SortKey::Name, true))
        );
        assert_eq!(
            mods.list
                .read(cx)
                .order
                .order_of(Side::Active)
                .map(|(key, _)| key),
            Some(SortKey::Name),
            "a reread keeps the column the load order is shown by"
        );
    });
}

#[test]
fn the_load_order_shown_by_group_keeps_its_headings_and_is_turned_off_by_asking_again() {
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            mods.set_headings(false, cx);
            super::layout_tests::tag_apart(mods, cx);
            let before = mods.list.read(cx).order.list(Side::Active).mods().to_vec();
            mods.order_active("group", cx);
            assert_eq!(mods.orders.active_key, "group");
            let entry = &mods.orders.active_by["group"];
            assert_eq!(crate::words::put(&entry.label), "Group");
            assert_eq!(entry.then, "");
            assert!(!mods.orders.inactive_by.contains_key("group"));
            let order = &mods.list.read(cx).order;
            let rows = order.rows(&crate::ui::app(cx), Side::Active);
            assert!(rows[0].is_header, "headings are shown though turned off");
            let tags: Vec<(String, usize, bool)> = rows
                .iter()
                .filter(|row| row.is_sub)
                .map(|row| (crate::words::put(&row.name), row.count, row.again))
                .collect();
            assert_eq!(
                tags,
                [
                    ("Combat".into(), 2, false),
                    ("Quality of life".into(), 1, false)
                ],
                "a tag split in the load order is one heading"
            );
            assert_eq!(order.list(Side::Active).mods(), before);
        })
    });
}

#[test]
fn no_key_moves_a_mod_while_the_load_order_is_shown_by_group() {
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    let window = cx.update(|cx| cx.windows()[0]);
    let order = |cx: &mut gpui::HeadlessAppContext| -> Vec<Option<ModId>> {
        cx.update(|cx| {
            let list = mods.read(cx).list.read(cx).order.list(Side::Active).mods();
            list.iter().map(|m| m.installed).collect()
        })
    };
    let nudged = |cx: &mut gpui::HeadlessAppContext, then: &'static str| {
        cx.update(|cx| {
            mods.update(cx, |mods, cx| {
                super::layout_tests::tag_apart(mods, cx);
                mods.order_active(then, cx);
                let third = mods.list.read(cx).order.list(Side::Active).mods()[3].installed;
                mods.select(third, true, cx);
            })
        });
        cx.update_window(window, |_, window, cx| {
            let handle = mods.read(cx).list(Side::Active).read(cx).focus_handle();
            window.focus(&handle, cx);
        })
        .unwrap();
        cx.run_until_parked();
        let before = order(cx);
        cx.update_window(window, |_, window, cx| {
            window.dispatch_keystroke(gpui::Keystroke::parse("alt-down").unwrap(), cx);
        })
        .unwrap();
        cx.run_until_parked();
        before != order(cx)
    };
    assert!(!nudged(&mut cx, "group"), "shown by group, nothing moves");
    assert!(nudged(&mut cx, ""), "in load order the same key moves it");
}

#[test]
fn the_inactive_mods_fold_into_the_rail_the_dividers_keep_their_least_and_showing_a_mod_unfolds_them()
 {
    use super::panes::{Grip, pane_widths};
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    cx.update(|cx| {
        let theme = cx.global::<crate::ui::theme::Theme>().clone();
        mods.update(cx, |mods, cx| {
            mods.set_hidden(true, cx);
            assert!(crate::ui::app(cx).mods_view().inactive_hidden);
            let window = gpui::px(1440.);
            let (inactive, active, details) = pane_widths(&theme, window, mods.panes_wanted());
            assert_eq!(inactive, gpui::px(0.));
            assert_eq!(
                inactive
                    + active
                    + details
                    + super::panes::rail_width(&theme)
                    + crate::ui::theme::HANDLE,
                window
            );
            let area = gpui::Bounds::new(
                gpui::point(gpui::px(0.), gpui::px(0.)),
                gpui::size(window, gpui::px(800.)),
            );
            mods.drag_grip(
                Grip::Details,
                gpui::point(gpui::px(1400.), gpui::px(10.)),
                area,
                cx,
            );
            assert_eq!(
                mods.layout.details_width,
                theme.sized(260.),
                "the details keep their least width"
            );
            mods.drag_grip(
                Grip::Inactive,
                gpui::point(gpui::px(500.), gpui::px(10.)),
                area,
                cx,
            );
            assert_eq!(
                mods.layout.inactive_width,
                gpui::px(500.) - crate::ui::theme::HANDLE / 2.
            );
            let id = mods
                .inactive
                .rows
                .iter()
                .find(|r| !r.is_header)
                .unwrap()
                .mod_id
                .unwrap();
            mods.show_mod(id, cx);
            assert!(
                !mods.view.inactive_hidden && !mods.panes_wanted().hidden,
                "an inactive mod shown unfolds the list"
            );
            assert_eq!(mods.selected_id(cx), Some(id));
        })
    });
}

#[test]
fn the_mod_tab_and_filter_left_come_back_and_a_new_choice_is_kept() {
    let (_tmp, mut cx, mods) = opened_with(|root| {
        std::fs::create_dir_all(root.join("settings/RimStack")).unwrap();
        std::fs::write(
            root.join("settings/RimStack/RimStack.conf"),
            "[Mods]\nlastMod=fixture.mod010\nlastTab=2\nlastFilter=is:changed fixture\n",
        )
        .unwrap();
    });
    cx.run_until_parked();
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            assert_eq!(
                mods.filter_text(cx),
                "fixture",
                "is:changed is not brought back"
            );
            let app = crate::ui::app(cx);
            let chosen = mods.selected_id(cx);
            let key = chosen.and_then(|id| mods.list.read(cx).order.package_of(&app, id));
            assert_eq!(key.as_deref(), Some("fixture.mod010"));
            assert_eq!(mods.details_view.read(cx).tab, 2);
            let other = mods
                .active
                .rows
                .iter()
                .find(|r| !r.is_header && r.mod_id != chosen)
                .unwrap()
                .mod_id
                .unwrap();
            mods.choose(Side::Active, other, cx);
            let kept = app.mods_view().last_mod;
            assert_ne!(kept, "fixture.mod010");
            assert!(!kept.is_empty(), "the mod chosen now is kept");
        })
    });
}

#[test]
fn an_edge_sizes_the_column_left_of_it_as_explorer_does() {
    let drags = [
        ("author", -20.),
        ("author", 20.),
        ("name", 10.),
        ("name", -10.),
        ("version", 10.),
        ("version", -10.),
    ];
    edges_at(None, "author=0.2", &drags);
}

#[test]
fn an_edge_follows_the_pointer_at_a_text_size_between_whole_pixels() {
    edges_at(Some(1.25), "author=0.25", &[("author", -10.)]);
}

/// With the widths `shares`, the drags each move their edge by as much as asked: they ask no
/// more than there is room for.
fn edges_at(text_scale: Option<f32>, shares: &str, drags: &[(&str, f32)]) {
    use super::columns::ColumnHeld;
    use gpui::{MouseButton, PlatformInput, point, px};
    let (_tmp, mut cx, mods) = super::edit_tests::opened_at(text_scale, |_| {});
    let window = cx.update(|cx| cx.windows()[0]);
    let draw = |cx: &mut gpui::HeadlessAppContext| {
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
    };
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            mods.toggle_column(Side::Active, "updated", cx)
        })
    });
    draw(&mut cx);
    const HEADS: [&str; 4] = ["name", "author", "version", "updated"];
    let lefts = |cx: &mut gpui::HeadlessAppContext| {
        cx.update(|cx| HEADS.map(|key| f32::from(mods.read(cx).keys[key].1.get().left())))
    };
    let send = |cx: &mut gpui::HeadlessAppContext, input: PlatformInput| {
        cx.update_window(window, |_, window, cx| window.dispatch_event(input, cx))
            .unwrap();
    };
    let at = |x: f32, y: f32| point(px(x), px(y));
    let moved = |x: f32, y: f32, pressed| {
        PlatformInput::MouseMove(gpui::MouseMoveEvent {
            position: at(x, y),
            pressed_button: pressed,
            modifiers: Default::default(),
        })
    };
    let press = |x: f32, y: f32, clicks| {
        PlatformInput::MouseDown(gpui::MouseDownEvent {
            button: MouseButton::Left,
            position: at(x, y),
            modifiers: Default::default(),
            click_count: clicks,
            first_mouse: false,
        })
    };
    let release = |x: f32, y: f32, clicks| {
        PlatformInput::MouseUp(gpui::MouseUpEvent {
            button: MouseButton::Left,
            position: at(x, y),
            modifiers: Default::default(),
            click_count: clicks,
        })
    };
    // The edge right of a cell is over the gap before the next heading.
    let edge_after = |cx: &mut gpui::HeadlessAppContext, key: &str| {
        let next = HEADS[HEADS.iter().position(|k| *k == key).unwrap() + 1];
        let drawn = cx.update(|cx| mods.read(cx).keys[next].1.get());
        let gap = f32::from(crate::ui::theme::GAP_LARGE);
        (
            f32::from(drawn.left()) - gap / 2.,
            f32::from(drawn.center().y),
        )
    };
    let drag = |cx: &mut gpui::HeadlessAppContext, key: &str, path: &[f32]| {
        let (x, y) = edge_after(cx, key);
        send(cx, moved(x, y, None));
        draw(cx);
        send(cx, press(x, y, 1));
        for by in path {
            send(cx, moved(x + by, y, Some(MouseButton::Left)));
            draw(cx);
        }
        send(cx, release(x + path.last().copied().unwrap_or(0.), y, 1));
        draw(cx);
    };
    let first = lefts(&mut cx);
    drag(&mut cx, "author", &[10., 25., 40., 20., 0.]);
    let back = lefts(&mut cx);
    assert!(
        back.iter()
            .zip(first)
            .all(|(now, was)| (now - was).abs() < 1.),
        "dragged and brought back, nothing changed: {first:?} {back:?}"
    );
    let view_at = |cx: &mut gpui::HeadlessAppContext| {
        cx.update(|cx| mods.read(cx).layout.menu_buttons.view.get().left())
    };
    let view_first = view_at(&mut cx);
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            mods.set_hidden(true, cx);
            mods.keep_view(|view| view.column_sizes = shares.into(), cx);
            mods.sync_lists(cx);
            cx.notify();
        })
    });
    draw(&mut cx);
    for &(key, by) in drags {
        let was = lefts(&mut cx);
        drag(&mut cx, key, &[by / 2., by]);
        let now = lefts(&mut cx);
        let sized = HEADS.iter().position(|k| *k == key).unwrap();
        for (at, head) in HEADS.iter().enumerate() {
            let shift = if at > sized { by } else { 0. };
            assert!(
                (now[at] - was[at] - shift).abs() < 1.,
                "the edge after {key} dragged {by}: {head} went from {} to {}",
                was[at],
                now[at]
            );
        }
        assert_eq!(view_at(&mut cx), view_first, "View stays where it is");
    }
    let sizes = cx.update(|cx| mods.read(cx).view.column_sizes.clone());
    assert!(
        sizes.contains("name="),
        "the name keeps its width once an edge moved: {sizes}"
    );
    let (x, y) = edge_after(&mut cx, "name");
    send(&mut cx, press(x, y, 1));
    send(&mut cx, release(x, y, 1));
    send(&mut cx, press(x, y, 2));
    send(&mut cx, release(x, y, 2));
    draw(&mut cx);
    let sizes = cx.update(|cx| mods.read(cx).view.column_sizes.clone());
    assert!(
        !sizes.contains("name="),
        "clicked twice, the name fills the row again: {sizes}"
    );

    let was = lefts(&mut cx);
    drag(&mut cx, "version", &[300., 900.]);
    let now = lefts(&mut cx);
    let grew = now[3] - was[3];
    assert!(
        (0. ..900.).contains(&grew) && now[..3] == was[..3],
        "dragged far, Version grows only by what Updated can give: {was:?} {now:?}"
    );
    assert_eq!(view_at(&mut cx), view_first);
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            for column in ["downloaded", "activated", "size"] {
                mods.toggle_column(Side::Active, column, cx);
            }
            mods.set_hidden(false, cx);
            mods.sync_lists(cx);
            cx.notify();
        })
    });
    draw(&mut cx);
    let before = lefts(&mut cx);
    let list = cx.update(|cx| mods.read(cx).list(Side::Active).clone());
    let over = cx.update(|cx| list.read(cx).drawn_at(4) + px(2.));
    send(
        &mut cx,
        PlatformInput::Scroll(gpui::ScrollEvent {
            position: point(px(700.), over),
            delta: gpui::ScrollDelta::Pixels(point(px(-40.), px(0.))),
            ..gpui::ScrollEvent::default()
        }),
    );
    draw(&mut cx);
    let after = lefts(&mut cx);
    for (at, head) in HEADS.iter().enumerate() {
        assert!(
            (before[at] - after[at] - 40.).abs() < 1.,
            "wider than the list, the headings scroll sideways with it: {head} {} to {}",
            before[at],
            after[at]
        );
    }
    let x = cx.update(|cx| mods.read(cx).sideways[0].read(cx).x());
    assert_eq!(x, px(40.));

    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            mods.natural_width(Side::Active, "version", cx)
        })
    });
    draw(&mut cx);
    let (version, updated) = cx.update(|cx| {
        let keys = &mods.read(cx).keys;
        (keys["version"].1.get(), keys["updated"].1.get())
    });
    let past = (version.right() + updated.left()) / 2. + px(1.);
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            let held = ColumnHeld {
                side: Side::Active,
                key: "author",
            };
            mods.drop_column(held, version.center().x - px(1.), cx);
            assert_eq!(
                mods.view.columns,
                "author,version,load,updated,downloaded,activated,size"
            );
            mods.drop_column(held, past, cx);
            assert_eq!(
                mods.view.columns, "version,author,load,updated,downloaded,activated,size",
                "past Version's middle, Author goes after it"
            );
        })
    });
}

#[test]
fn the_inactive_mods_have_columns_of_their_own_that_order_them() {
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            mods.toggle_column(Side::Inactive, "size", cx);
            mods.toggle_column(Side::Inactive, "activated", cx);
            assert_eq!(mods.view.columns, "author,version,load");
            assert_eq!(mods.view.inactive_columns, "size,activated");
            assert_eq!(
                super::columns::chosen(&mods.view, &mods.orders, Side::Inactive),
                ["size"],
                "an inactive mod has no date it was activated"
            );
            mods.heading_pressed(Side::Inactive, "size", cx);
            assert_eq!(
                mods.list.read(cx).order.inactive_by(),
                Some((SortKey::Size, true))
            );
            assert_eq!(
                crate::ui::app(cx).mods_view().inactive_columns,
                "size,activated"
            );
        })
    });
}

#[test]
fn filter_terms_become_chips_and_are_taken_off_one_by_one() {
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            mods.filter_by("is:warning fixture tag:x", cx)
        })
    });
    cx.update(|cx| {
        let mods = mods.read(cx);
        assert_eq!(mods.filtering.read(cx).terms(), ["is:warning", "tag:x"]);
        assert_eq!(mods.filter.read(cx).text(), "fixture");
        assert_eq!(mods.filter_text(cx), "is:warning tag:x fixture");
    });
    let field = cx.update(|cx| mods.read(cx).filter.clone());
    cx.update(|cx| field.update(cx, |f, cx| f.set_text("fixture !is:error mod", cx)));
    cx.update(|cx| {
        let mods = mods.read(cx);
        assert_eq!(
            mods.filtering.read(cx).terms(),
            ["is:warning", "tag:x", "!is:error"],
            "a finished term becomes a chip"
        );
        assert_eq!(mods.filter.read(cx).text(), "fixture mod");
    });
    cx.update(|cx| {
        let filtering = mods.read(cx).filtering.clone();
        filtering.update(cx, |filter, cx| filter.drop_term(0, cx))
    });
    cx.update(|cx| assert_eq!(mods.read(cx).filter_text(cx), "tag:x !is:error fixture mod"));
    cx.update(|cx| mods.update(cx, |mods, cx| mods.clear_filter(cx)));
    cx.update(|cx| assert_eq!(mods.read(cx).filter_text(cx), ""));
}

#[test]
fn a_term_toggled_and_a_reread_keep_the_chips() {
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    cx.update(|cx| mods.update(cx, |mods, cx| mods.filter_by("tag:x fixture", cx)));
    cx.update(|cx| mods.update(cx, |mods, cx| mods.toggle_term("is:warning", cx)));
    cx.update(|cx| assert_eq!(mods.read(cx).filter_text(cx), "tag:x is:warning fixture"));
    cx.update(|cx| mods.update(cx, |mods, cx| mods.toggle_term("tag:x", cx)));
    cx.update(|cx| assert_eq!(mods.read(cx).filter_text(cx), "is:warning fixture"));
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            mods.load(true, cx);
            mods.edit_list(cx, |list, _| {
                list.order = crate::workspace::load_order::LoadOrder::demo(0, 0)
            });
        })
    });
    let started = Instant::now();
    while cx.update(|cx| mods.read(cx).list.read(cx).count(Side::Active) == 0) {
        assert!(started.elapsed() < Duration::from_secs(20));
        crate::ui::made_up::step(&cx);
    }
    let shown = cx.update(|cx| mods.read(cx).list.read(cx).order.filter_summary());
    let whole = cx.update(|cx| {
        let mods = mods.read(cx);
        (
            mods.list.read(cx).count(Side::Active),
            mods.list.read(cx).matching(Side::Active),
        )
    });
    assert!(
        whole.1 < whole.0,
        "the reread list is still filtered: {shown}"
    );
}

#[test]
fn the_hints_go_one_after_the_other_and_are_not_shown_again() {
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            mods.start_hints(cx);
            assert_eq!(mods.hints.step, 1);
            mods.next_hint(cx);
            mods.next_hint(cx);
            assert_eq!(mods.hints.step, 3);
            mods.next_hint(cx);
            assert_eq!(mods.hints.step, 0, "Done ends them");
            mods.start_hints(cx);
            assert_eq!(mods.hints.step, 0, "seen once, they do not start again");
        })
    });
}

#[test]
fn select_all_chooses_only_the_mods_the_filter_shows() {
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    cx.update(|cx| mods.update(cx, |mods, cx| mods.filter_by("Fixture Mod 04", cx)));
    cx.update(|cx| mods.update(cx, |mods, cx| mods.select_all(Side::Inactive, cx)));
    cx.update(|cx| {
        let mods = mods.read(cx);
        let mut shown: Vec<ModId> = mods
            .inactive
            .rows
            .iter()
            .filter(|r| !r.is_header)
            .filter_map(|r| r.mod_id)
            .collect();
        let mut chosen = mods.list.read(cx).selection.clone();
        shown.sort_unstable();
        chosen.sort_unstable();
        assert!(!shown.is_empty());
        assert_eq!(chosen, shown);
        assert!(chosen.len() < mods.list.read(cx).count(Side::Inactive));
    });
}

#[test]
fn a_mod_dropped_below_the_last_row_goes_to_the_end_of_the_load_order() {
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            let list = mods.list.read(cx).order.list(Side::Active).mods();
            let (first, count) = (list[3].installed.unwrap(), list.len());
            mods.drop_on((Side::Active, 3, vec![first]), (Side::Active, count), cx);
            let list = mods.list.read(cx).order.list(Side::Active).mods();
            assert_eq!(list.last().and_then(|m| m.installed), Some(first));
        })
    });
}

#[test]
fn a_column_and_a_divider_move_by_their_keys() {
    use super::panes::Grip;
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    cx.update(|cx| {
        let step = cx.global::<crate::ui::theme::Theme>().sized(24.);
        mods.update(cx, |mods, cx| {
            mods.move_column(Side::Active, "author", 1, cx);
            assert_eq!(mods.view.columns, "version,author,load");
            mods.move_column(Side::Active, "author", -1, cx);
            assert_eq!(mods.view.columns, "author,version,load");
            let before = mods.layout.inactive_width;
            let right = gpui::KeyDownEvent {
                keystroke: gpui::Keystroke::parse("right").unwrap(),
                is_held: false,
                prefer_character_input: false,
            };
            mods.nudge_grip(Grip::Inactive, &right, cx);
            assert_eq!(mods.layout.inactive_width, before + step);
        })
    });
}

#[test]
fn enter_on_a_heading_holding_the_keys_orders_the_list_once() {
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    let window = cx.update(|cx| cx.windows()[0]);
    let draw = |cx: &mut gpui::HeadlessAppContext| {
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
    };
    draw(&mut cx);
    let wanted = cx.update(|cx| mods.read(cx).orders.active_by["name"].then.clone());
    cx.update_window(window, |_, window, cx| {
        let handle = mods.read(cx).keys["name"].0.clone();
        window.focus(&handle, cx);
    })
    .unwrap();
    draw(&mut cx);
    let enter = gpui::Keystroke::parse("enter").unwrap();
    cx.update_window(window, |_, window, cx| {
        window.dispatch_event(
            gpui::PlatformInput::KeyDown(gpui::KeyDownEvent {
                keystroke: enter.clone(),
                is_held: false,
                prefer_character_input: false,
            }),
            cx,
        );
        window.dispatch_event(
            gpui::PlatformInput::KeyUp(gpui::KeyUpEvent { keystroke: enter }),
            cx,
        );
    })
    .unwrap();
    draw(&mut cx);
    cx.update(|cx| {
        assert_eq!(
            mods.read(cx).list.read(cx).order.order_of(Side::Active),
            rimstack_core::mods::columns::from_kept(&wanted),
            "one press, one step"
        );
    });
}

#[test]
fn the_row_under_the_pointer_is_the_one_pointed_at_as_the_list_scrolls() {
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    let window = cx.update(|cx| cx.windows()[0]);
    let draw = |cx: &mut gpui::HeadlessAppContext| {
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
    };
    draw(&mut cx);
    let list = cx.update(|cx| mods.read(cx).list(Side::Inactive).clone());
    let (y, x) = cx.update(|cx| (list.read(cx).drawn_at(3) + gpui::px(2.), gpui::px(100.)));
    cx.update_window(window, |_, window, cx| {
        window.dispatch_event(
            gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
                position: gpui::point(x, y),
                pressed_button: None,
                modifiers: Default::default(),
            }),
            cx,
        );
    })
    .unwrap();
    draw(&mut cx);
    assert_eq!(cx.update(|cx| list.read(cx).pointed()), Some(3));
    let row = cx.update(|cx| {
        let list = list.read(cx);
        let step = list.drawn_at(4) - list.drawn_at(3);
        list.scroll().borrow_mut().set_top(step);
        list.pointed()
    });
    assert_eq!(row, Some(4), "the list moved under a pointer that stayed");
}

#[test]
fn a_mod_moved_slides_from_where_it_was() {
    use crate::ui::widgets::watch;
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    let window = cx.update(|cx| cx.windows()[0]);
    let draw = |cx: &mut gpui::HeadlessAppContext| {
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
    };
    draw(&mut cx);
    let (name, from, to) = cx.update(|cx| {
        let mods = mods.read(cx);
        let list = mods.list.read(cx).order.list(Side::Active).mods();
        (list[5].name.clone(), 5, 6)
    });
    let top_of = |cx: &mut gpui::HeadlessAppContext| {
        watch::start();
        cx.update(|cx| {
            mods.read(cx)
                .list(Side::Active)
                .clone()
                .update(cx, |_, cx| cx.notify())
        });
        cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
        watch::take()
            .into_iter()
            .find(|seen| seen.text.as_ref() == name)
            .map(|seen| seen.bounds.top())
    };
    let before = top_of(&mut cx).expect("the mod is drawn");
    cx.update(|cx| mods.update(cx, |mods, cx| mods.move_mod(Side::Active, from, to, cx)));
    let sliding = top_of(&mut cx).expect("the mod is drawn while it slides");
    cx.advance_clock(std::time::Duration::from_millis(500));
    draw(&mut cx);
    let landed = top_of(&mut cx).expect("the mod is drawn where it went");
    assert!(landed > before, "it went down");
    assert!(
        sliding < landed - gpui::px(1.),
        "it starts above where it lands: {sliding:?} then {landed:?}"
    );
}

#[test]
fn the_other_sections_read_the_chosen_mod_and_the_active_list_from_the_order_store() {
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    cx.run_until_parked();
    let chosen = cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            let chosen = mods.list.read(cx).order.list(Side::Active).mods()[2]
                .installed
                .unwrap();
            mods.choose(Side::Active, chosen, cx);
            chosen
        })
    });
    cx.run_until_parked();
    let (order, active) = cx.update(|cx| {
        let active = mods.read(cx).list.read(cx).active_installed();
        (crate::ui::order::read(cx), active)
    });
    assert_eq!(order.selected, Some(chosen));
    assert_eq!(*order.active, active);
    let listing = cx.update(|cx| crate::ui::app(cx).listing(&order.active));
    let total = format!("Total # of mods: {}", active.len());
    assert!(listing.contains(&total), "{listing}");
}

#[test]
fn a_change_that_leaves_the_rows_and_details_as_they_were_draws_neither_again() {
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    let window = cx.update(|cx| cx.windows()[0]);
    let draw = |cx: &mut gpui::HeadlessAppContext| {
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
    };
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            let chosen = mods.list.read(cx).order.list(Side::Active).mods()[4]
                .installed
                .unwrap();
            mods.choose(Side::Active, chosen, cx);
        })
    });
    for _ in 0..3 {
        draw(&mut cx);
        std::thread::sleep(std::time::Duration::from_millis(400));
    }
    draw(&mut cx);
    let counted = || {
        (
            super::list::RENDERS.with(|renders| renders.replace(0)),
            super::chosen::RENDERS.with(|renders| renders.replace(0)),
        )
    };
    counted();
    for _ in 0..3 {
        cx.update(|cx| mods.update(cx, |mods, cx| mods.show(cx)));
        draw(&mut cx);
    }
    assert_eq!(
        counted(),
        (0, 0),
        "lists and details drawn again for nothing"
    );
}

#[test]
fn scrolling_the_load_order_draws_the_list_alone_again() {
    use super::frame::RENDERS;
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    let window = cx.update(|cx| cx.windows()[0]);
    let draw = |cx: &mut gpui::HeadlessAppContext| {
        cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
    };
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            let chosen = mods.list.read(cx).order.list(Side::Active).mods()[4]
                .installed
                .unwrap();
            mods.choose(Side::Active, chosen, cx);
        })
    });
    for _ in 0..3 {
        cx.run_until_parked();
        draw(&mut cx);
    }
    let list = cx.update(|cx| mods.read(cx).list(Side::Active).clone());
    RENDERS.with(|renders| renders.set(0));
    for _ in 0..30 {
        cx.update(|cx| {
            list.update(cx, |list, cx| {
                let top = list.scroll().borrow().top();
                list.scroll().borrow_mut().set_top(top + gpui::px(3.));
                cx.notify();
            })
        });
        draw(&mut cx);
    }
    let renders = RENDERS.with(|renders| renders.get());
    assert!(
        renders < 5,
        "the Mods tab was drawn again {renders} times in 30 frames of its list scrolling"
    );
}

#[test]
fn the_window_around_the_mods_tab_is_not_drawn_again_for_what_it_does_not_show() {
    use crate::ui::shell::RENDERS;
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    let window = cx.update(|cx| cx.windows()[0]);
    let draw = |cx: &mut gpui::HeadlessAppContext| {
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
    };
    for _ in 0..3 {
        draw(&mut cx);
    }
    RENDERS.with(|renders| renders.set(0));
    for at in 0..5 {
        cx.update(|cx| {
            mods.update(cx, |mods, cx| {
                let chosen = mods.list.read(cx).order.list(Side::Active).mods()[at]
                    .installed
                    .unwrap();
                mods.choose(Side::Active, chosen, cx);
            })
        });
        draw(&mut cx);
    }
    let chosen = RENDERS.with(|renders| renders.replace(0));
    for at in 0..5 {
        cx.update(|cx| crate::ui::tip::point_at(format!("tip {at}").into(), cx));
        draw(&mut cx);
    }
    let hinted = RENDERS.with(|renders| renders.get());
    assert_eq!(
        (chosen, hinted),
        (0, 0),
        "the window was drawn again for mods chosen and for hints"
    );
}

#[test]
fn what_the_window_shows_of_the_list_follows_an_edit_its_save_and_its_undo() {
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    let around = |cx: &mut gpui::HeadlessAppContext| {
        cx.run_until_parked();
        cx.update(|cx| crate::ui::around::read(cx))
    };
    assert!(!around(&mut cx).dirty);
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            let last = mods.list.read(cx).count(Side::Active) - 1;
            mods.take(Side::Active, last, None, cx);
        })
    });
    let edited = around(&mut cx);
    assert!(edited.dirty && edited.unsaved.is_some());
    assert!(
        edited.in_the_way.len() > usize::from(edited.errors) + usize::from(edited.sort_pending)
    );
    cx.update(|cx| mods.update(cx, |mods, cx| mods.save(None, cx)));
    let started = Instant::now();
    while cx.update(|cx| mods.read(cx).list.read(cx).saving) {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the save did not end"
        );
        crate::ui::made_up::step(&cx);
    }
    let saved = around(&mut cx);
    assert!(!saved.dirty && saved.unsaved.is_none());
    cx.update(|cx| mods.update(cx, |mods, cx| mods.undo(cx)));
    assert!(
        around(&mut cx).dirty,
        "an undo after a save leaves the list unsaved"
    );
}

#[test]
fn a_control_pointed_at_and_an_edit_draw_the_bars_again() {
    use crate::ui::shell::RENDERS;
    use crate::ui::status_note::DRAWN;
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    let window = cx.update(|cx| cx.windows()[0]);
    let draw = |cx: &mut gpui::HeadlessAppContext| {
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
    };
    for _ in 0..3 {
        draw(&mut cx);
    }
    let (strip, room) = cx.update(|cx| {
        let theme = cx.global::<crate::ui::theme::Theme>();
        (theme.strip(), theme.sized(30.))
    });
    let history = gpui::point(crate::ui::WINDOW.0 - room, crate::ui::WINDOW.1 - strip / 2.);
    DRAWN.with(|drawn| drawn.set(0));
    cx.update_window(window, |_, window, cx| {
        window.dispatch_event(
            gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
                position: history,
                ..Default::default()
            }),
            cx,
        );
    })
    .unwrap();
    draw(&mut cx);
    assert!(
        !cx.update(|cx| crate::ui::tip::hint(cx)).is_empty(),
        "History has a hint"
    );
    assert!(
        DRAWN.with(|drawn| drawn.get()) > 0,
        "the hint was not drawn"
    );
    RENDERS.with(|renders| renders.set(0));
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            let last = mods.list.read(cx).count(Side::Active) - 1;
            mods.take(Side::Active, last, None, cx);
        })
    });
    draw(&mut cx);
    assert!(
        RENDERS.with(|renders| renders.get()) > 0,
        "the bars did not show the list unsaved"
    );
}

fn with_previews() -> (
    tempfile::TempDir,
    gpui::HeadlessAppContext,
    gpui::Entity<super::Mods>,
) {
    opened_with(|root| {
        let preview = root.join("fx/game/Data/Core/About/Preview.png");
        for entry in std::fs::read_dir(root.join("fx/game/Mods")).unwrap() {
            let about = entry.unwrap().path().join("About");
            std::fs::copy(&preview, about.join("Preview.png")).unwrap();
        }
    })
}

#[test]
fn a_jump_of_the_scroll_bar_finds_the_pictures_decoded_while_idle() {
    let (_tmp, mut cx, mods) = with_previews();
    let window = cx.update(|cx| cx.windows()[0]);
    let draw = |cx: &mut gpui::HeadlessAppContext| {
        cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
    };
    for _ in 0..5 {
        cx.run_until_parked();
        draw(&mut cx);
    }
    let thumbs = cx.update(|cx| mods.read(cx).thumbs.clone());
    let list = cx.update(|cx| mods.read(cx).list(Side::Active).clone());
    cx.update(|cx| {
        list.update(cx, |list, cx| {
            let most = list.scroll().borrow().most();
            list.scroll().borrow_mut().set_top(most);
            cx.notify();
        })
    });
    draw(&mut cx);
    assert!(
        thumbs.borrow().decoded() > 40,
        "the list's pictures were not decoded"
    );
    assert_eq!(
        thumbs.borrow().awaited_in_view(),
        0,
        "the end of the list drew without its pictures"
    );
}

#[test]
fn the_pictures_of_the_next_page_are_decoded_before_it_is_scrolled_to() {
    let (_tmp, mut cx, mods) = with_previews();
    let window = cx.update(|cx| cx.windows()[0]);
    let draw = |cx: &mut gpui::HeadlessAppContext| {
        cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
    };
    for _ in 0..5 {
        cx.run_until_parked();
        draw(&mut cx);
    }
    let thumbs = cx.update(|cx| mods.read(cx).thumbs.clone());
    assert_eq!(thumbs.borrow().awaited_in_view(), 0);
    let shown_first = thumbs.borrow().decoded();
    let list = cx.update(|cx| mods.read(cx).list(Side::Active).clone());
    cx.update(|cx| {
        list.update(cx, |list, cx| {
            let (top, page) = {
                let scroll = list.scroll().borrow();
                (scroll.top(), scroll.shown())
            };
            list.scroll().borrow_mut().set_top(top + page);
            cx.notify();
        })
    });
    draw(&mut cx);
    assert!(shown_first > 0, "no picture was decoded");
    assert_eq!(
        thumbs.borrow().awaited_in_view(),
        0,
        "a row scrolled to by a page drew without its picture"
    );
}
