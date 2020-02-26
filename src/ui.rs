use cursive::Cursive;
use cursive::event::Key;
use cursive::views;
use cursive::traits::*;
use rls_data::{Def, DefKind};
use crate::analysis::{Analysis, CrateId, CrateType};

fn crate_label(c: &CrateId) -> String {
    match c.crate_type {
        CrateType::Bin => format!("{} (bin)", c.name),
        CrateType::ProcMacro => format!("{} (proc-macro)", c.name),
        CrateType::Lib => c.name.clone(),
    }
}

fn def_label(def: &Def) -> Option<String> {
    let prefix = match def.kind {
        DefKind::Mod => {
            if def.qualname == "::" {
                return None;
            } else {
                "mod"
            }
        }
        DefKind::Enum => "enum",
        DefKind::Struct => "struct",
        DefKind::Function => "fn",
        DefKind::Tuple => "tuple",
        DefKind::Union => "union",
        DefKind::Trait => "trait",
        DefKind::ForeignFunction => "extern fn",
        DefKind::Macro => "macro",
        DefKind::Type => "type",
        DefKind::ExternType => "extern type",
        DefKind::Const => "const",
        DefKind::Static => "static",
        DefKind::ForeignStatic => "extern static",
        DefKind::Local | DefKind::Method | DefKind::Field | DefKind::TupleVariant
            | DefKind::StructVariant => return None,
    };
    Some(format!("{} {}", prefix, def.name))
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

    let mut crates = analysis.crate_ids().collect::<Vec<_>>();
    crates.sort_unstable_by(|a, b| a.name.cmp(&b.name));

    let mut crates_select = views::SelectView::new();
    for c in crates {
        crates_select.add_item(crate_label(&c), c);
    }

    // TODO: implement a better live search than this
    crates_select.set_autojump(true);

    crates_select.set_on_submit(|ui, crate_id| {
        ui.call_on_name("horiz_layout", |view: &mut views::LinearLayout| {
            while view.len() > 1 {
                view.remove_child(view.len() - 1);
            }
        });

        let data = ui.user_data::<UserData>().unwrap();

        // TODO: module paths
        let mut defs = data.analysis.defs(crate_id, &[])
            .filter_map(|def| {
                def_label(def)
                    .map(|label| (label, def.clone()))
            })
            .collect::<Vec<_>>();
        defs.sort_unstable_by(|(a, _), (b, _)| a.cmp(&b));
        if defs.is_empty() {
            return;
        }

        let mut next = views::SelectView::new();
        for (label, def) in defs {
            next.add_item(label, def);
        }

        next.set_on_submit(|ui, def| {
            let txt = format!("{:#?}", def);
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

        ui.call_on_name("horiz_layout", |view: &mut views::LinearLayout| {
            view.add_child(
                views::ScrollView::new(next)
                    .scroll_y(true)
            );
            view.set_focus_index(1).unwrap();
        });
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

    ui.set_user_data(UserData { analysis });
    ui.run();
}
