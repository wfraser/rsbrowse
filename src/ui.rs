use cursive::Cursive;
use cursive::event::Key;
use cursive::views;

use crate::analysis::{Analysis, CrateId, CrateType};

fn crate_label(c: &CrateId) -> String {
    match c.crate_type {
        CrateType::Bin => format!("{} (bin)", c.name),
        CrateType::ProcMacro => format!("{} (proc-macro)", c.name),
        CrateType::Lib => c.name.clone(),
    }
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
        ui.add_layer(
            views::Dialog::info(format!("{:#?}", item))
        );
    });

    ui.add_fullscreen_layer(
        views::ScrollView::new(select)
            .scroll_y(true)
    );

    ui.run();
}
