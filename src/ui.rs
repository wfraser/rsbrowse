use cursive::Cursive;
use cursive::event::Key;
use cursive::views;
use cursive::traits::*;

use crate::analysis::{Analysis, CrateId, CrateType};

fn crate_label(c: &CrateId) -> String {
    match c.crate_type {
        CrateType::Bin => format!("{} (bin)", c.name),
        CrateType::ProcMacro => format!("{} (proc-macro)", c.name),
        CrateType::Lib => c.name.clone(),
    }
}

struct UserData {
    analysis: Analysis,
}

pub fn ui_loop(analysis: Analysis) {
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

    //ui.add_fullscreen_layer(views::TextView::new("Press ESC to quit."));
    //ui.add_layer(views::TextView::new("nothing here yet..."));

    let mut crates = analysis.crate_ids().collect::<Vec<_>>();
    crates.sort_unstable_by(|a, b| a.name.cmp(&b.name));

    let mut select = views::SelectView::new();
    for c in crates {
        select.add_item(crate_label(&c), c);
    }

    // TODO: implement a better live search than this
    select.set_autojump(true);

    select.set_on_submit(|ui, item| {
        let data = ui.user_data::<UserData>().unwrap();

        let dialog_txt = format!("{:#?}\n{:#?}", item, data.analysis.get_crate(item));
        //ui.add_layer(views::Dialog::info(dialog_txt));
        ui.add_layer(
            views::Dialog::around(
                views::ScrollView::new(
                    views::TextView::new(dialog_txt)
                    )
                    .scroll_y(true)
                )
                .dismiss_button("ok")
        );

        let mut next = views::SelectView::new();
        // replace this with adding all the defs in the selected crate
        for n in 0 .. 50 {
            next.add_item(format!("{}", n), n);
        }

        ui.call_on_name("horiz_layout", |view: &mut views::LinearLayout| {
            view.add_child(
                views::ScrollView::new(next)
                    .scroll_y(true)
            );
            view.set_focus_index(view.get_focus_index() + 1).unwrap();
        });
    });

    ui.add_fullscreen_layer(
        views::ScrollView::new(
            views::LinearLayout::horizontal()
                .child(
                    views::ScrollView::new(select)
                        .scroll_y(true)
                )
                .with_name("horiz_layout")
            )
            .scroll_x(true)
    );

    ui.set_user_data(UserData { analysis });
    ui.run();
}
