use crate::browser_trait::Browser;
use crate::scroll_pad::ScrollPad;
use cursive::event::Key;
use cursive::traits::*;
use cursive::views::{Dialog, LinearLayout, ScrollView, SelectView, TextView};
use cursive::{Cursive, CursiveExt, XY};
use std::borrow::Cow;

/// How many lines to scroll to before a definition.
const SOURCE_LEADING_CONTEXT_LINES: usize = 5;

struct UserData<T> {
    browser: T,
}

/// Makes a selectview showing the children of the given parent item in the given crate.
/// Returns None if there are no children to display.
fn make_selectview<B: Browser + 'static>(
    data: &mut UserData<B>,
    parent_id: &B::ItemId,
    depth: usize,
) -> Option<SelectView<(B::ItemId, B::Item)>> {
    let items = data.browser.list_items(parent_id);
    if items.is_empty() {
        return None;
    }

    let mut select = SelectView::new();
    for (label, (id, item)) in items {
        select.add_item(label, (id, item));
    }

    select.set_on_submit(move |ui, (_id, item)| info_dialog::<B>(ui, item));

    select.set_on_select(move |ui, (id, _item)| {
        add_panel::<B>(ui, id, depth + 1);
    });

    Some(select)
}

fn info_dialog<B: Browser + 'static>(ui: &mut Cursive, item: &B::Item) {
    let data = ui.user_data::<UserData<B>>().unwrap();

    let info_txt = data.browser.get_info(item);
    let (source_txt, start_line) = data.browser.get_source(item);

    let item_dlg = item.clone();
    let info_dialog = Dialog::around(
        LinearLayout::vertical()
            .child(TextView::new(info_txt).scrollable())
            .child(
                TextView::new(source_txt)
                    .scrollable()
                    .with_name("source_scroll"),
            )
            .scrollable(),
    )
    .dismiss_button("ok")
    .button("debug", move |ui| {
        let data = ui.user_data::<UserData<B>>().unwrap();
        let dbg_txt = data.browser.get_debug_info(&item_dlg);
        let dbg_dialog = Dialog::around(TextView::new(dbg_txt).scrollable()).dismiss_button("ok");
        ui.add_layer(dbg_dialog);
    });

    ui.add_layer(info_dialog);

    if let Some(start_line) = start_line {
        let screen_size = ui.screen_size();
        ui.call_on_name("source_scroll", move |view: &mut ScrollView<TextView>| {
            // HAX: set_offset doesn't work on newly-added views until a layout is done
            view.layout(screen_size);
            view.set_offset(XY::new(
                0,
                start_line.saturating_sub(SOURCE_LEADING_CONTEXT_LINES),
            ));
        });
    }
}

fn add_panel<B: Browser + 'static>(ui: &mut Cursive, parent_id: &B::ItemId, depth: usize) {
    ui.call_on_name("horiz_layout", |view: &mut LinearLayout| {
        while view.len() > depth {
            view.remove_child(view.len() - 1);
        }
    });

    let data: &mut UserData<B> = ui.user_data().unwrap();

    // Expand out all panes to the right, using the first item in each pane, until we run out of
    // stuff to show.
    // Ideally we wouldn't need to do this immediately and could instead do it on focus changes
    // between panes, but Cursive doesn't have any way for a view to respond to being focused, nor
    // does its LinearLayout have a callback on switching views.
    // Importantly, it's not sufficient to just change things on selection change, because a pane
    // with a single item can never have its selection changed, so you'd be stuck there unable to
    // go deeper within the tree. So this is why we go ahead and create the next views *right
    // away*.
    let mut next = vec![];
    let mut local_depth = depth;
    let mut local_parent = Cow::Borrowed(parent_id);
    while let Some(view) = make_selectview(data, &local_parent, local_depth) {
        if let Some((_label, (id, _item))) = view.get_item(0) {
            local_depth += 1;
            local_parent = Cow::Owned(id.clone());
        }
        next.push(view);
    }

    if next.is_empty() {
        return;
    }

    ui.call_on_name("horiz_layout", |horiz_layout: &mut LinearLayout| {
        for view in next {
            horiz_layout.add_child(ScrollPad::new(
                ScrollView::new(view).scroll_y(true).show_scrollbars(true),
            ));
        }
    });
}

fn about(ui: &mut Cursive) {
    ui.add_layer(
        Dialog::around(
            TextView::new(format!(
                "rsbrowse/{}\n\
                    {}by Bill Fraser\n\
                    https://github.com/wfraser/rsbrowse",
                env!("CARGO_PKG_VERSION"),
                if let Some(git) = option_env!("GIT_COMMIT_HASH") {
                    format!("git:{git}\n")
                } else {
                    String::new()
                },
            ))
            .h_align(cursive::align::HAlign::Center),
        )
        .title("about")
        .dismiss_button("ok"),
    )
}

pub fn run<B: Browser + 'static>(browser: B) {
    let mut ui = Cursive::default();

    ui.menubar()
        .add_leaf("rsbrowse!", about)
        .add_delimiter()
        .add_leaf("Quit", |ui| ui.quit())
        .add_leaf("(ESC to activate menu)", |_| ());
    ui.set_autohide_menu(false);
    ui.add_global_callback(Key::Esc, |ui| ui.select_menubar());
    //ui.add_global_callback(Key::Esc, |ui| ui.quit());

    ui.set_theme(cursive::theme::Theme::default().with(|theme| {
        use cursive::theme::{
            BaseColor::{Black, Green, White},
            Color::{Dark, Light, Rgb},
            PaletteColor,
        };
        theme.palette[PaletteColor::Background] = Dark(Black);
        theme.palette[PaletteColor::View] = Rgb(32, 32, 32);
        theme.palette[PaletteColor::Shadow] = Light(Black);
        theme.palette[PaletteColor::Primary] = Dark(White);
        theme.palette[PaletteColor::Highlight] = Dark(Green);
    }));

    let mut crates_select = SelectView::new();
    for (label, crate_id) in browser.list_crates() {
        crates_select.add_item(label, crate_id);
    }

    let first_crate = crates_select
        .get_item(0)
        .map(|(_label, crate_id)| crate_id.clone());

    // TODO: implement a better live search than this
    crates_select.set_autojump(true);

    crates_select.set_on_select(|ui, crate_id| {
        add_panel::<B>(ui, crate_id, 1);
    });

    ui.add_fullscreen_layer(
        ScrollView::new(
            LinearLayout::horizontal()
                .child(ScrollPad::new(
                    ScrollView::new(crates_select).scroll_y(true),
                ))
                .with_name("horiz_layout"),
        )
        .scroll_x(true),
    );

    ui.set_user_data(UserData { browser });

    // Go ahead and expand the first crate in the list immediately.
    if let Some(crate_id) = first_crate {
        add_panel::<B>(&mut ui, &crate_id, 1);
    }

    ui.run();
}
