use cursive::{Cursive, XY};
use cursive::event::Key;
use cursive::traits::*;
use cursive::views::{Dialog, LinearLayout, ScrollView, SelectView, TextView};
use crate::analysis::CrateId;
use crate::browser::{Browser, Item};
use crate::scroll_pad::ScrollPad;
use std::borrow::Cow;

struct UserData {
    browser: Browser,
}

/// Makes a selectview showing the children of the given parent item in the given crate.
/// Returns None if there are no children to display.
fn make_selectview(data: &mut UserData, crate_id: CrateId, parent: &Item, depth: usize)
    -> Option<SelectView<Item>>
{
    let items = data.browser.list_items(&crate_id, parent);
    if items.is_empty() {
        return None;
    }

    let mut select = SelectView::new();
    for (label, item) in items {
        select.add_item(label, item);
    }

    let crate_id2 = crate_id.clone();
    select.set_on_submit(move |ui, item| {
        let data = ui.user_data::<UserData>().unwrap();

        let info_txt = data.browser.get_info(&crate_id2, item);
        let (source_txt, span) = get_source(item);

        let crate_id_dlg = crate_id2.clone();
        let item_dlg = item.clone();
        let info_dialog = Dialog::around(
            LinearLayout::vertical()
                .child(TextView::new(info_txt).scrollable())
                .child(TextView::new(source_txt)
                    .scrollable()
                    .with_name("source_scroll"))
                .scrollable()
            )
            .dismiss_button("ok")
            .button("debug", move |ui| {
                let data = ui.user_data::<UserData>().unwrap();
                let dbg_txt = data.browser.get_debug_info(&crate_id_dlg, &item_dlg);
                let dbg_dialog = Dialog::around(
                    TextView::new(dbg_txt)
                        .scrollable()
                    )
                    .dismiss_button("ok");
                ui.add_layer(dbg_dialog);
            });

        ui.add_layer(info_dialog);

        if let Some(span) = span {
            ui.refresh(); // Need to force a layout before we can do a scroll.
            ui.call_on_name("source_scroll", move |view: &mut ScrollView<TextView>| {
                view.set_offset(XY::new(0, (span.line_start.0 - 1) as usize));
            });
        }

    });

    select.set_on_select(move |ui, item| {
        if let Item::Root = item {
            return;
        }
        add_panel(ui, crate_id.clone(), item, depth + 1);
    });

    Some(select)
}

fn get_source(item: &Item) -> (String, Option<rls_data::SpanData>) {
    use std::fs::File;
    use std::io::{BufRead, BufReader};
    let mut txt = String::new();
    match item {
        Item::Def(def) => {
            match File::open(&def.span.file_name) {
                Ok(f) => {
                    for (i, line) in BufReader::new(f)
                        .lines()
                        .enumerate()
                    {
                        txt += &format!("{}: ", i + 1);
                        txt += &line.unwrap_or_else(|e| format!("<Read Error: {}>", e));
                        txt.push('\n');
                    }
                }
                Err(e) => {
                    txt += &format!("error opening source: {}", e);
                }
            }
            return (txt, Some(def.span.clone()))
        }
        _ => txt += &format!("source listing unimplemented for {:?}", item),
    }
    (txt, None)
}

fn add_panel(ui: &mut Cursive, crate_id: CrateId, parent: &Item, depth: usize) {
    ui.call_on_name("horiz_layout", |view: &mut LinearLayout| {
        while view.len() > depth {
            view.remove_child(view.len() - 1);
        }
    });

    let data: &mut UserData = ui.user_data().unwrap();

    // Expand out all panes to the right, using the first item in each pane, until we run out of
    // stuff to show.
    // Ideally we wouldn't need to do this immediately and could instead do it on focus changes
    // between panes, but Cursive doesn't have any way for a view to respond to being focused, nor
    // does its LinearLayout have a callback on switching views.
    // Importantly, it's not sufficient to just change things on selection change, because a pane
    // with a single item can never have its selection changed, so you'd be stuck there unable to go
    // deeper within the tree. So this is why we go ahead and create the next views *right away*.
    let mut next = vec![];
    let mut local_depth = depth;
    let mut local_parent = Cow::Borrowed(parent);
    while let Some(view) = make_selectview(data, crate_id.clone(), &local_parent, local_depth) {
        if let Some((_label, item)) = view.get_item(0) {
            local_depth += 1;
            local_parent = Cow::Owned(item.clone());
        }
        next.push(view);
    }

    if next.is_empty() {
        return;
    }

    ui.call_on_name("horiz_layout", |horiz_layout: &mut LinearLayout| {
        for view in next {
            horiz_layout.add_child(
                ScrollPad::new(
                    ScrollView::new(view)
                        .scroll_y(true)
                        .show_scrollbars(true)
                )
            );
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

    let mut crates_select = SelectView::new();
    for (label, crate_id) in browser.list_crates() {
        crates_select.add_item(label, crate_id);
    }

    let first_crate = crates_select.get_item(0).map(|(_label, crate_id)| crate_id.clone());

    // TODO: implement a better live search than this
    crates_select.set_autojump(true);

    crates_select.set_on_select(|ui, crate_id| {
        add_panel(ui, crate_id.clone(), &Item::Root, 1);
    });

    ui.add_fullscreen_layer(
        ScrollView::new(
            LinearLayout::horizontal()
                .child(
                    ScrollPad::new(
                        ScrollView::new(crates_select)
                            .scroll_y(true)
                    )
                )
                .with_name("horiz_layout")
            )
            .scroll_x(true)
    );

    ui.set_user_data(UserData { browser });

    // Go ahead and expand the first crate in the list immediately.
    if let Some(crate_id) = first_crate {
        add_panel(&mut ui, crate_id, &Item::Root, 1);
    }

    ui.run();
}
