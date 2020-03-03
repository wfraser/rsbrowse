use cursive::Cursive;
use cursive::event::Key;
use cursive::views;
use cursive::traits::*;
use crate::analysis::CrateId;
use crate::browser::{Browser, Item};
use std::borrow::Cow;

struct UserData {
    browser: Browser,
}

fn make_selectview(data: &mut UserData, crate_id: CrateId, parent: &Item, depth: usize)
    -> Option<views::SelectView<Item>>
{
    let items = data.browser.list_items(&crate_id, parent);
    if items.is_empty() {
        return None;
    }

    let mut select = views::SelectView::new();
    for (label, item) in items {
        select.add_item(label, item);
    }

    let crate_id2 = crate_id.clone();
    select.set_on_submit(move |ui, item| {
        let data = ui.user_data::<UserData>().unwrap();
        let txt = data.browser.get_debug_info(&crate_id2, item);
        ui.add_layer(
            views::Dialog::around(
                views::ScrollView::new(
                    views::TextView::new(txt)
                    )
                    .scroll_y(true)
                )
                .dismiss_button("ok")
        );
    });

    select.set_on_select(move |ui, item| {
        if let Item::Root = item {
            return;
        }
        add_panel(ui, crate_id.clone(), item, depth + 1);
    });

    Some(select)
}

fn add_panel(ui: &mut Cursive, crate_id: CrateId, parent: &Item, depth: usize) {
    ui.call_on_name("horiz_layout", |view: &mut views::LinearLayout| {
        while view.len() > depth {
            view.remove_child(view.len() - 1);
        }
    });

    let data: &mut UserData = ui.user_data().unwrap();

    // Mega-hax here: instatiating the next pane is done when selection changes on the current pane.
    // If the current pane only has one item, there's no way to change selection, and so no way to
    // browse deeper within the tree. Ideally we'd just do this when focus between panes changes,
    // but Cursive doesn't have any way for a view to respond to being focused, nor does its
    // LinearLayout have a callback on switching views. So instead, if the view is going to have
    // only one item, we go ahead and create the next view *right away*. This continues until we run
    // out of stuff or have a pane with >1 item.
    let mut next = vec![];
    let mut local_depth = depth;
    let mut local_parent = Cow::Borrowed(parent);
    loop {
        let view = match make_selectview(data, crate_id.clone(), &local_parent, local_depth) {
            Some(view) => view,
            None => break,
        };

        // Only one item in the view; continue to loop, using the single item in this view as the
        // parent for the next pane.
        if view.len() == 1 {
            if let Some((_label, item)) = view.get_item(0) {
                local_depth += 1;
                local_parent = Cow::Owned(item.clone());
            }
            next.push(view);
        } else {
            next.push(view);
            break;
        }
    }

    if next.is_empty() {
        return;
    }

    ui.call_on_name("horiz_layout", |horiz_layout: &mut views::LinearLayout| {
        for view in next {
            horiz_layout.add_child(
                views::ScrollView::new(view)
                    .scroll_y(true)
                    .show_scrollbars(true)
            );
        }

        // If this is the first one, we were called from pressing Enter, so focus it.
        if depth == 1 {
            horiz_layout.set_focus_index(1).unwrap();
        }
    });
}

pub fn run(browser: Browser) {
    let mut ui = Cursive::default();

    /*
    ui.menubar()
        .add_leaf("rsbrowse!", |_|())
        .add_delimiter()
        .add_leaf("Quit", |ui| ui.quit())
        .add_leaf("(ESC to activate menu)", |_|());
    ui.set_autohide_menu(false);
    ui.add_global_callback(Key::Esc, |ui| ui.select_menubar());
    */

    ui.add_global_callback(Key::Esc, |ui| ui.quit());

    let mut crates_select = views::SelectView::new();
    for (label, crate_id) in browser.list_crates() {
        crates_select.add_item(label, crate_id);
    }

    // TODO: implement a better live search than this
    crates_select.set_autojump(true);

    crates_select.set_on_submit(|ui, crate_id| {
        add_panel(ui, crate_id.clone(), &Item::Root, 1);
    });

    ui.add_fullscreen_layer(
        views::ScrollView::new(
            views::LinearLayout::horizontal()
                .child(
                    views::ScrollView::new(crates_select)
                        .scroll_y(true)
                )
                .with_name("horiz_layout")
            )
            .scroll_x(true)
    );

    ui.set_user_data(UserData { browser });
    ui.run();
}
