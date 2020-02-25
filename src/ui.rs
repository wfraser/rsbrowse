use cursive::Cursive;
use cursive::event::Key;
use cursive::views;

use crate::analysis::Analysis;

pub fn ui_loop(_analysis: Analysis) {
    let mut ui = Cursive::default();

    ui.add_global_callback(Key::Esc, |ui| ui.quit());
    ui.add_fullscreen_layer(views::TextView::new("Press ESC to quit."));
    ui.add_layer(views::TextView::new("nothing here yet..."));

    ui.run();
}
