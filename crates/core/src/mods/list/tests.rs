use super::*;

fn id(n: usize) -> ModId {
    ModId::new(0, n)
}

fn m(name: &str, author: &str, category: Category) -> Mod {
    Mod {
        id: String::new(),
        tags: Vec::new(),
        local: false,
        workshop: false,
        git: false,
        code: false,
        patched: Default::default(),
        sub: String::new(),
        load_micros: None,
        filed: category,
        chosen: false,
        held_by: String::new(),
        name: name.into(),
        author: author.into(),
        version: "1.0".into(),
        category,
        installed: None,
    }
}

fn sample() -> GroupedList {
    GroupedList::new(vec![
        m("Harmony", "pardeike", Category::Loaders),
        m("Core", "Ludeon", Category::Game),
        m("Royalty", "Ludeon", Category::Game),
        m("HugsLib", "UnlimitedHugs", Category::Libraries),
        m("Dubs Bad Hygiene", "Dubwise", Category::Content),
        m("Rimefeller", "Dubwise", Category::Content),
    ])
}

fn header(category: Category, count: usize, collapsed: bool) -> Row {
    Row::Header {
        category,
        count,
        collapsed,
    }
}

#[test]
fn groups_mods_under_headers_in_load_order() {
    let list = sample();
    assert_eq!(
        list.rows(),
        &[
            header(Category::Loaders, 1, false),
            Row::Mod { index: 0 },
            header(Category::Game, 2, false),
            Row::Mod { index: 1 },
            Row::Mod { index: 2 },
            header(Category::Libraries, 1, false),
            Row::Mod { index: 3 },
            header(Category::Content, 2, false),
            Row::Mod { index: 4 },
            Row::Mod { index: 5 },
        ]
    );
}

#[test]
fn flat_list_has_no_headers_filters_and_appends() {
    let mut mods = sample().mods().to_vec();
    mods.swap(0, 5);
    let mut list = GroupedList::flat(mods);
    assert_eq!(list.rows().len(), 6);
    assert!(list.rows().iter().all(|r| matches!(r, Row::Mod { .. })));
    assert_eq!(list.insert_in_category(m("New", "x", Category::Loaders)), 6);
    list.set_filter("dubwise");
    assert_eq!(list.rows(), &[Row::Mod { index: 0 }, Row::Mod { index: 4 }]);
    assert_eq!(list.toggle(Category::Content), Change::Reset);
}

#[test]
fn replacing_the_mods_keeps_the_filter() {
    let mut list = GroupedList::flat(sample().mods().to_vec());
    list.set_filter("dubwise");
    let mut mods = list.mods().to_vec();
    mods.reverse();
    list.set_mods(mods);
    assert_eq!(list.mods()[0].name, "Rimefeller");
    assert_eq!(list.rows(), &[Row::Mod { index: 0 }, Row::Mod { index: 1 }]);
}

#[test]
fn neighbours_in_a_sub_category_of_yours_sit_under_its_row() {
    let mut mods = vec![
        m("A", "", Category::Content),
        m("B", "", Category::Content),
        m("C", "", Category::Content),
        m("D", "", Category::Content),
        m("E", "", Category::Textures),
    ];
    for (i, m) in mods.iter_mut().enumerate() {
        m.installed = Some(id(i));
    }
    let mut list = GroupedList::new(mods);
    let genes = ["Genes".to_string()];
    assert!(list.set_tags(id(1), &genes));
    assert!(list.set_tags(id(2), &genes));
    assert!(!list.set_tags(id(99), &genes));
    let content = Row::Header {
        category: Category::Content,
        count: 4,
        collapsed: false,
    };
    assert_eq!(
        list.rows()[..6],
        [
            content,
            Row::Mod { index: 0 },
            Row::Sub {
                first: 1,
                count: 2,
                collapsed: false,
            },
            Row::Mod { index: 1 },
            Row::Mod { index: 2 },
            Row::Mod { index: 3 },
        ]
    );
    assert_eq!(list.subs(Category::Content), ["Genes"]);
    assert!(list.subs(Category::Textures).is_empty());
    assert_eq!(list.toggle(Category::Content), Change::Reset);
    assert_eq!(list.rows().len(), 3);
    list.toggle(Category::Content);
    list.set_filter("b");
    let sub = |first, count, collapsed| Row::Sub {
        first,
        count,
        collapsed,
    };
    assert_eq!(list.rows()[1], sub(1, 1, false));
    list.set_filter("");
    assert_eq!(list.in_sub_of(1), [1, 2]);
    list.toggle_sub(1);
    assert_eq!(list.rows()[2], sub(1, 2, true));
    assert_eq!(list.rows()[3], Row::Mod { index: 3 });
    list.toggle_sub(1);
    assert_eq!(list.rows()[3], Row::Mod { index: 1 });
    list.toggle_sub(0);
    assert!(list.in_sub_of(0).is_empty());
    assert!(list.set_tags(id(1), &[]));
    assert_eq!(list.rows()[2], Row::Mod { index: 1 });
    assert_eq!(list.rows()[3], sub(2, 1, false));
}

#[test]
fn a_list_can_be_shown_by_a_column_without_changing_the_load_order() {
    let mut mods = vec![
        m("Beta", "zed", Category::Content),
        m("alpha", "Amy", Category::Textures),
        m("Gamma", "bob", Category::Content),
    ];
    mods[0].load_micros = Some(5_000);
    mods[2].load_micros = Some(900_000);
    let mut list = GroupedList::new(mods);
    let shown = |list: &GroupedList| -> Vec<usize> {
        let at = |r: &Row| match r {
            Row::Mod { index } => Some(*index),
            _ => None,
        };
        list.rows().iter().filter_map(at).collect()
    };
    list.show_by(Some((SortKey::Name, false)));
    assert_eq!(list.rows().len(), 3, "no headings");
    assert_eq!(shown(&list), [1, 0, 2]);
    list.show_by(Some((SortKey::Author, true)));
    assert_eq!(shown(&list), [0, 2, 1]);
    list.show_by(Some((SortKey::Load, true)));
    assert_eq!(shown(&list), [2, 0, 1]);
    list.set_filter("a");
    assert_eq!(shown(&list), [2, 0, 1]);
    list.set_filter("gamma");
    assert_eq!(shown(&list), [2]);
    list.set_filter("");
    list.show_by(None);
    assert_eq!(list.mods()[0].name, "Beta");
    assert_eq!(list.rows().len(), 6);
}

#[test]
fn a_patches_term_narrows_to_mods_that_patch_such_a_method() {
    let mut mods = sample().mods().to_vec();
    mods[3].patched = ["verse.pawn.tick".to_string()].into();
    mods[4].patched = [
        "verse.pawn.kill".to_string(),
        "verse.thing.tick".to_string(),
    ]
    .into();
    let mut list = GroupedList::flat(mods);
    let shown = |list: &GroupedList| -> Vec<String> {
        let name = |r: &Row| match r {
            Row::Mod { index } => Some(list.mods()[*index].name.clone()),
            _ => None,
        };
        list.rows().iter().filter_map(name).collect()
    };
    list.set_filter("patches:Pawn.Tick");
    assert_eq!(shown(&list), ["HugsLib"]);
    list.set_filter("patches:tick");
    assert_eq!(shown(&list), ["HugsLib", "Dubs Bad Hygiene"]);
    list.set_filter("patches:kill patches:thing.");
    assert_eq!(shown(&list), ["Dubs Bad Hygiene"]);
    list.set_filter("patches:");
    assert!(shown(&list).is_empty());
}

#[test]
fn typed_filter_terms_narrow_by_author_tag_source_and_code() {
    let mut mods = sample().mods().to_vec();
    mods[3].local = true; // HugsLib
    mods[3].code = true;
    mods[4].code = true; // Dubs Bad Hygiene
    mods[4].installed = Some(id(40));
    let mut list = GroupedList::flat(mods);
    let shown = |list: &GroupedList| -> Vec<String> {
        let name = |r: &Row| match r {
            Row::Mod { index } => Some(list.mods()[*index].name.clone()),
            _ => None,
        };
        list.rows().iter().filter_map(name).collect()
    };
    list.set_filter("author:DUB");
    assert_eq!(shown(&list), ["Dubs Bad Hygiene", "Rimefeller"]);
    list.set_filter("author:dub hygiene");
    assert_eq!(shown(&list), ["Dubs Bad Hygiene"]);
    list.set_filter("has:code");
    assert_eq!(shown(&list), ["HugsLib", "Dubs Bad Hygiene"]);
    list.set_filter("has:code is:local");
    assert_eq!(shown(&list), ["HugsLib"]);

    list.set_filter("tag:fav");
    assert!(shown(&list).is_empty());
    assert!(list.set_tags(id(40), &["Favourite".to_string(), "medieval".to_string()]));
    assert_eq!(shown(&list), ["Dubs Bad Hygiene"]);
    assert!(!list.set_tags(id(999), &[]));

    list.set_filter("is:nothing");
    assert!(shown(&list).is_empty());
    list.set_filter("tag:");
    assert!(shown(&list).is_empty());
    list.set_filter("  ");
    assert_eq!(shown(&list).len(), 6);
}

#[test]
fn source_terms_tell_the_workshop_from_git_checkouts_and_the_game() {
    let mut mods = sample().mods().to_vec();
    mods[3].local = true; // HugsLib
    mods[3].git = true;
    mods[4].workshop = true; // Dubs Bad Hygiene
    let mut list = GroupedList::flat(mods);
    let names = |list: &GroupedList| -> Vec<String> {
        let rows = list.rows().iter();
        let index = rows.filter_map(|r| match r {
            Row::Mod { index } => Some(*index),
            _ => None,
        });
        index.map(|i| list.mods()[i].name.clone()).collect()
    };
    list.set_filter("is:workshop");
    assert_eq!(names(&list), ["Dubs Bad Hygiene"]);
    list.set_filter("is:git");
    assert_eq!(names(&list), ["HugsLib"]);
    list.set_filter("is:git is:workshop");
    assert!(names(&list).is_empty());
}

#[test]
fn a_side_term_empties_the_other_list_and_both_empty_both() {
    let mut active = sample();
    let mut inactive = sample();
    let count = |list: &GroupedList| list.matching();

    active.set_filter_on(Side::Active, "is:active");
    inactive.set_filter_on(Side::Inactive, "is:active");
    assert_eq!((count(&active), count(&inactive)), (6, 0));
    assert!(inactive.rows().is_empty());

    active.set_filter_on(Side::Active, "is:inactive royalty");
    inactive.set_filter_on(Side::Inactive, "is:inactive royalty");
    assert_eq!((count(&active), count(&inactive)), (0, 1));

    active.set_filter_on(Side::Active, "is:active is:inactive");
    inactive.set_filter_on(Side::Inactive, "is:inactive is:active");
    assert_eq!((count(&active), count(&inactive)), (0, 0));

    active.set_filter("is:inactive");
    assert_eq!(count(&active), 6);
}

#[test]
fn is_terms_ask_about_the_flags_given_and_forget_them_when_the_order_changes() {
    let mut list = GroupedList::flat(sample().mods().to_vec());
    list.set_filter("is:error");
    assert!(list.rows().is_empty());
    list.set_flags(vec![
        0,
        flag::ERROR,
        flag::WARNING | flag::CONFLICT,
        0,
        flag::ERROR | flag::UPDATED,
        0,
    ]);
    assert_eq!(list.rows(), &[Row::Mod { index: 1 }, Row::Mod { index: 4 }]);
    list.set_filter("is:error is:updated");
    assert_eq!(list.rows(), &[Row::Mod { index: 4 }]);
    list.set_filter("is:conflict is:warning");
    assert_eq!(list.rows(), &[Row::Mod { index: 2 }]);
    list.move_mod(0, 3);
    assert!(list.rows().is_empty());
}

#[test]
fn a_list_can_be_arranged_by_name_or_by_category() {
    let mut list = sample();
    list.arrange(true);
    let names: Vec<&str> = list.mods().iter().map(|m| m.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Core",
            "Dubs Bad Hygiene",
            "Harmony",
            "HugsLib",
            "Rimefeller",
            "Royalty"
        ]
    );
    assert!(!list.is_grouped() && list.rows().len() == 6);
    list.arrange(false);
    assert_eq!(list.mods()[0].name, "Harmony");
    assert!(list.is_grouped());
}

#[test]
fn headings_can_be_turned_on_and_off() {
    let mut list = GroupedList::flat(sample().mods().to_vec());
    assert!(!list.is_grouped());
    list.set_grouped(true);
    assert!(list.is_grouped());
    assert_eq!(list.rows().len(), 10);
    list.set_grouped(false);
    assert_eq!(list.rows().len(), 6);
}

#[test]
fn a_category_is_found_again_by_its_label() {
    for category in Category::ALL {
        assert_eq!(Category::from_label(category.label()), Some(category));
    }
    assert_eq!(Category::from_label("interface"), None);
    for category in Category::ALL {
        assert_eq!(Category::from_kept(category.key()), Some(category));
        assert_eq!(Category::from_kept(category.label()), Some(category));
    }
    assert_eq!(Category::from_kept("Performance"), Some(Category::Tweaks));
    assert_eq!(Category::from_kept(""), None);
    assert_eq!(Category::from_label("Performance"), Some(Category::Tweaks));
    assert_eq!(Category::from_label(""), None);
}

#[test]
fn runs_count_the_headings_a_list_needs() {
    let mut mods = sample().mods().to_vec();
    assert_eq!(runs(&mods), 4);
    mods.swap(0, 5);
    assert_eq!(runs(&mods), 5);
    assert_eq!(runs(&[]), 0);
}

#[test]
fn empty_list_has_no_rows() {
    let mut list = GroupedList::new(Vec::new());
    assert!(list.rows().is_empty());
    list.set_filter("x");
    assert!(list.rows().is_empty());
    assert_eq!(list.toggle(Category::Content), Change::Reset);
}

#[test]
fn collapsing_removes_the_rows_under_one_header() {
    let mut list = sample();
    let change = list.toggle(Category::Game);
    assert_eq!(change, Change::Removed { first: 3, count: 2 });
    assert_eq!(list.rows()[2], header(Category::Game, 2, true));
    assert_eq!(list.rows()[3], header(Category::Libraries, 1, false));
}

#[test]
fn expanding_puts_them_back() {
    let mut list = sample();
    let before = list.rows().to_vec();
    list.toggle(Category::Game);
    let change = list.toggle(Category::Game);
    assert_eq!(change, Change::Inserted { first: 3, count: 2 });
    assert_eq!(list.rows(), before.as_slice());
}

#[test]
fn toggling_a_category_that_is_not_shown_asks_for_a_reset() {
    let mut list = sample();
    assert_eq!(list.toggle(Category::Textures), Change::Reset);
    assert_eq!(list.rows().len(), 10);
}

#[test]
fn filter_keeps_matches_and_drops_empty_groups() {
    let mut list = sample();
    list.set_filter("dub");
    assert_eq!(
        list.rows(),
        &[
            header(Category::Content, 2, false),
            Row::Mod { index: 4 },
            Row::Mod { index: 5 }
        ]
    );
}

#[test]
fn filter_matches_author_and_ignores_case() {
    let mut list = sample();
    list.set_filter("LUDEON");
    assert_eq!(
        list.rows(),
        &[
            header(Category::Game, 2, false),
            Row::Mod { index: 1 },
            Row::Mod { index: 2 }
        ]
    );
}

#[test]
fn filter_ignores_case_for_non_ascii_letters() {
    let mut list = GroupedList::new(vec![m("Élevage Étendu", "Zoé", Category::Content)]);
    list.set_filter("élevage");
    assert_eq!(list.rows().len(), 2);
    list.set_filter("ZOÉ");
    assert_eq!(list.rows().len(), 2);
}

#[test]
fn filter_with_no_match_shows_nothing_and_clearing_restores() {
    let mut list = sample();
    list.set_filter("zzz");
    assert!(list.rows().is_empty());
    list.set_filter("");
    assert_eq!(list.rows().len(), 10);
}

#[test]
fn moving_a_mod_changes_load_order_and_regroups() {
    let mut list = sample();
    list.move_mod(5, 0);
    assert_eq!(list.mods()[0].name, "Rimefeller");
    assert_eq!(list.rows()[0], header(Category::Content, 1, false));
    assert_eq!(list.rows()[2], header(Category::Loaders, 1, false));
}

#[test]
fn moving_with_a_bad_index_changes_nothing() {
    let mut list = sample();
    let before = list.rows().to_vec();
    list.move_mod(99, 0);
    list.move_mod(0, 99);
    list.move_mod(2, 2);
    assert_eq!(list.rows(), before.as_slice());
}

#[test]
fn inserting_by_category_lands_after_the_last_mod_of_that_category() {
    let mut list = sample();
    let at = list.insert_in_category(m(
        "Vanilla Expanded Framework",
        "Oskar",
        Category::Libraries,
    ));
    assert_eq!(at, 4);
    assert_eq!(list.mods()[4].name, "Vanilla Expanded Framework");
}

#[test]
fn inserting_a_category_with_no_members_lands_after_earlier_categories() {
    let mut list = sample();
    let at = list.insert_in_category(m("Chairs", "Jin", Category::Textures));
    assert_eq!(at, 6);
    let at = list.insert_in_category(m("Prepatcher", "Zetrith", Category::Loaders));
    assert_eq!(at, 1);
}

#[test]
fn removing_returns_the_mod_and_regroups() {
    let mut list = sample();
    let gone = list.remove(0).unwrap();
    assert_eq!(gone.name, "Harmony");
    assert_eq!(list.rows()[0], header(Category::Game, 2, false));
    assert!(list.remove(99).is_none());
}

#[test]
fn a_category_s_stretch_has_a_first_and_a_last_place() {
    let list = sample();
    let mods = list.mods();
    assert_eq!(edge_of_run(mods, 2, true), 1);
    assert_eq!(edge_of_run(mods, 1, false), 2);
    assert_eq!(edge_of_run(mods, 0, true), 0);
    assert_eq!(edge_of_run(mods, 0, false), 0);
    assert_eq!(edge_of_run(mods, 4, false), 5);
}

#[test]
fn a_group_in_two_places_is_split_and_one_in_a_row_is_not() {
    let mut mods = vec![
        m("a", "", Category::Content),
        m("b", "", Category::Content),
        m("c", "", Category::Content),
        m("d", "", Category::Content),
        m("e", "", Category::Content),
    ];
    for at in [0, 1, 3] {
        mods[at].sub = "Genes".into();
    }
    mods[4].sub = "Animals".into();
    assert_eq!(split_groups(&mods), ["Genes"]);
    mods[2].sub = "Genes".into();
    assert!(split_groups(&mods).is_empty());
    mods[3].category = Category::Textures;
    assert!(split_groups(&mods).is_empty());
}

#[test]
fn folding_a_category_with_a_group_in_it_is_not_one_row_per_mod() {
    let mut mods = sample().mods().to_vec();
    mods[4].sub = "Dubs".into();
    mods[5].sub = "Dubs".into();
    let mut list = GroupedList::new(mods);
    let before = list.rows().len();
    assert_eq!(list.toggling(Category::Content), Change::Reset);
    assert_eq!(list.toggle(Category::Content), Change::Reset);
    assert_eq!(before - list.rows().len(), 3);
}

#[test]
fn what_folding_would_change_is_known_before_it_happens() {
    let mut list = sample();
    let told = list.toggling(Category::Game);
    assert_eq!(told, Change::Removed { first: 3, count: 2 });
    assert_eq!(list.rows().len(), 10);
    assert_eq!(list.toggle(Category::Game), told);
    assert_eq!(
        list.toggling(Category::Game),
        Change::Inserted { first: 3, count: 2 }
    );
}

#[test]
fn mods_that_share_a_name_are_twins_whatever_the_case() {
    let mods = [
        m("Rimefeller", "a", Category::Content),
        m("HugsLib", "b", Category::Libraries),
        m("RIMEFELLER", "c", Category::Content),
    ];
    assert_eq!(twins(&mods), [true, false, true]);
}

fn installed_sample() -> GroupedList {
    let mut mods = sample().mods().to_vec();
    for (at, m) in mods.iter_mut().enumerate() {
        m.installed = (m.name != "Royalty").then_some(id(100 + at));
    }
    GroupedList::new(mods)
}

#[test]
fn stepping_goes_to_the_next_installed_mod_past_headings_and_stops_at_the_ends() {
    let list = installed_sample();
    assert_eq!(list.installed_on(1), Some(id(100)));
    assert_eq!(
        (
            list.installed_on(0),
            list.installed_on(4),
            list.installed_on(99)
        ),
        (None, None, None)
    );
    assert_eq!(list.step(1, 1), Some(3));
    assert_eq!(list.step(3, 1), Some(6));
    assert_eq!(list.step(6, -1), Some(3));
    assert_eq!((list.step(9, 1), list.step(1, -1)), (None, None));
    assert_eq!((list.step(-1, 1), list.step(10, -1)), (Some(1), Some(9)));
}

#[test]
fn a_stretch_of_rows_gives_its_installed_mods_whichever_end_comes_first() {
    let list = installed_sample();
    assert_eq!(list.installed_between(3, 8), [id(101), id(103), id(104)]);
    assert_eq!(list.installed_between(8, 3), [id(101), id(103), id(104)]);
    assert!(list.installed_between(0, 0).is_empty());
    assert_eq!(list.installed_between(9, 99), [id(105)]);
}

#[test]
fn the_mod_beside_a_row_is_the_one_below_then_above_then_two_away() {
    let list = installed_sample();
    assert_eq!(list.beside(8), Some(id(105)));
    assert_eq!(list.beside(9), Some(id(104)));
    assert_eq!(list.beside(6), Some(id(104)));
    assert_eq!(GroupedList::new(Vec::new()).beside(0), None);
}

#[test]
fn a_mod_is_found_by_its_place_and_its_row_and_a_hidden_one_has_no_row() {
    let mut list = installed_sample();
    assert_eq!(list.position_of(id(103)), Some(3));
    assert_eq!((list.row_of(3), list.row_of(99)), (Some(6), None));
    assert_eq!(list.position_of(id(999)), None);
    list.set_filter("rimefeller");
    assert_eq!(list.row_of(list.position_of(id(103)).unwrap()), None);
}

#[test]
fn a_step_from_a_mod_skips_headings_and_from_no_mod_starts_at_an_end() {
    let list = installed_sample();
    assert_eq!(list.step_from(Some(id(101)), 1), Some(6));
    assert_eq!(list.step_from(Some(id(103)), -1), Some(3));
    assert_eq!(list.step_from(None, 1), Some(1));
    assert_eq!(list.step_from(None, -1), Some(9));
    assert_eq!(list.step_from(Some(id(999)), 1), Some(1));
    assert_eq!(list.beside_mod(id(104)), Some(id(105)));
}

#[test]
fn the_mods_through_a_stretch_leave_out_the_one_it_starts_from() {
    let mut list = installed_sample();
    assert_eq!(
        list.others_through(id(101), id(104)),
        Some(vec![id(103), id(104)])
    );
    assert_eq!(
        list.others_through(id(104), id(101)),
        Some(vec![id(101), id(103)])
    );
    list.set_filter("hygiene");
    assert_eq!(list.others_through(id(101), id(104)), None);
}

#[test]
fn two_mods_come_in_load_order_and_a_nudge_stays_inside_the_list() {
    let list = installed_sample();
    assert_eq!(
        list.in_load_order(id(104), id(100)),
        Some((id(100), id(104)))
    );
    assert_eq!(list.in_load_order(id(100), id(999)), None);
    assert_eq!(list.nudged(id(100), 1), Some((0, 1)));
    assert_eq!(list.nudged(id(100), -1), None);
    assert_eq!(list.nudged(id(105), 1), None);
    assert_eq!(list.nudged(id(105), -2), Some((5, 3)));
}

#[test]
fn mods_are_found_by_key_in_the_order_asked_and_unknown_keys_are_left_out() {
    let list = installed_sample();
    let key_of = |id: ModId| Some(format!("key{}", id.index()));
    let keys = ["key105", "nobody", "key100"].map(String::from);
    assert_eq!(list.with_keys(&keys, &key_of), [id(105), id(100)]);
    assert!(list.with_keys(&[], &key_of).is_empty());
}

#[test]
fn a_mod_goes_below_another_only_when_that_one_loads_after_it() {
    let list = installed_sample();
    assert_eq!(list.below(id(100), id(104)), Some((0, 4)));
    assert_eq!(list.below(id(104), id(100)), None);
    assert_eq!(list.below(id(100), id(999)), None);
}
