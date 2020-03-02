use cursive::Cursive;
use cursive::event::Key;
use cursive::views;
use cursive::traits::*;
use crate::analysis::CrateId;
use crate::browser::{Browser, Item};

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

    let next = match make_selectview(data, crate_id, parent, depth) {
        Some(view) => view,
        None => return,
    };

    ui.call_on_name("horiz_layout", |view: &mut views::LinearLayout| {
        view.add_child(
            views::ScrollView::new(next)
                .scroll_y(true)
                .show_scrollbars(true)
        );

        // If this is the first one, we were called from pressing Enter, so focus it.
        if depth == 1 {
            view.set_focus_index(1).unwrap();
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
