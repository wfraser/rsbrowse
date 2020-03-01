use cursive::Cursive;
use cursive::event::Key;
use cursive::views;
use cursive::traits::*;
use rls_data::{Def, DefKind};
use crate::analysis::{Analysis, CrateId, CrateType, ImplDetails};

fn crate_label(c: &CrateId) -> String {
    match c.crate_type {
        CrateType::Bin => format!("{} (bin)", c.name),
        CrateType::ProcMacro => format!("{} (proc-macro)", c.name),
        CrateType::Lib => c.name.clone(),
        CrateType::CDylib => format!("{} (cdylib)", c.name),
    }
}

fn def_label(def: &Def) -> String {
    let prefix = match def.kind {
        DefKind::Mod => "mod",
        DefKind::Enum => "enum",
        DefKind::Struct => "struct",
        DefKind::Function | DefKind::Method => "fn", // TODO: include signature
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
        DefKind::TupleVariant | DefKind::StructVariant => return def.value.clone(),
        DefKind::Field => return format!("{}: {}", def.name, def.value),
        DefKind::Local => "local", // or should we return None?
    };
    format!("{} {}", prefix, def.name)
}

struct UserData {
    analysis: Analysis,
}

#[derive(Debug)]
enum Item {
    Root,
    Def(rls_data::Def),
    Impl(ImplDetails),
}

fn make_selectview(data: &mut UserData, crate_id: CrateId, parent: &Item, depth: usize)
    -> Option<views::SelectView<Item>>
{
    let mut select = views::SelectView::new();

    match parent {
        Item::Root | Item::Def(_) => {
            let parent_id = match parent {
                Item::Def(def) => Some(def.id),
                _ => None,
            };

            let mut defs = data.analysis.defs(&crate_id, parent_id)
                .map(|def| (def_label(def), def.clone()))
                .collect::<Vec<_>>();
            defs.sort_unstable_by(|(a, _), (b, _)| a.cmp(&b));

            for (label, def) in defs {
                select.add_item(label, Item::Def(def));
            }

            if let Some(id) = parent_id {
                let mut impls = data.analysis.impls(&crate_id, id)
                    .filter(|impl_details| {
                        // Filter out references to traits in other crates.
                        // TODO: handle these as well.
                        if let Some(trait_id) = impl_details.trait_id {
                            if trait_id.krate != id.krate {
                                return false;
                            }
                        }
                        true
                    })
                    .map(|impl_details| {
                        eprintln!("{:#?}", impl_details);
                        let trait_name = impl_details.trait_id
                            .map(|id| {
                                data.analysis.get_def(&crate_id, id)
                                    .unwrap_or_else(|| panic!("no such trait ID {:?}", id))
                                    .qualname
                                    .clone()
                            })
                            .unwrap_or_else(|| "Self".to_owned());

                        (format!("impl {}", trait_name), impl_details)
                    })
                    .collect::<Vec<_>>();
                impls.sort_unstable_by(|(a, _), (b, _)| a.cmp(&b));

                for (label, imp) in impls {
                    select.add_item(label, Item::Impl(imp));
                }
            }
        }

        Item::Impl(impl_details) => {
            let imp: &rls_data::Impl = data.analysis.get_impl(&crate_id, impl_details.impl_id)
                .unwrap_or_else(|| panic!("no such impl {:?}", impl_details));

            let mut methods = vec![];
            // It appears that imp.children has the methods for an inherent impl (impl Foo), whereas
            // the trait will have them if it is a trait impl. It doesn't look like it can be a mix.
            // But go through the impl children anyway, just in case.
            for id in &imp.children {
                if let Some(method) = data.analysis.get_def(&crate_id, *id) {
                    methods.push((def_label(method), method.clone()));
                }
            }
            if let Some(trait_id) = impl_details.trait_id {
                let def = data.analysis.get_def(&crate_id, trait_id).expect("no such trait");
                for id in &def.children {
                    if let Some(method) = data.analysis.get_def(&crate_id, *id) {
                        methods.push((def_label(method), method.clone()));
                    }
                }
            }
            methods.sort_unstable_by(|(a, _), (b, _)| a.cmp(&b));

            for (label, def) in methods {
                select.add_item(label, Item::Def(def));
            }
        }
    }

    if select.is_empty() {
        return None;
    }

    let crate_id2 = crate_id.clone();
    select.set_on_submit(move |ui, item| {
        let data = ui.user_data::<UserData>().unwrap();
        let mut txt = format!("{:#?}", item);
        match item {
            Item::Def(def) => {
                for child_id in &def.children {
                    if let Some(child) = data.analysis.get_def(&crate_id2, *child_id) {
                        txt += &format!("\nchild {:?} = {:#?}", child_id, child);
                    }
                }
            }
            Item::Impl(impl_details) => {
                let imp = data.analysis.get_impl(&crate_id2, impl_details.impl_id).unwrap();
                txt += &format!("\nimpl: {:#?}", imp);
                for child_id in &imp.children {
                    if let Some(child) = data.analysis.get_def(&crate_id2, *child_id) {
                        txt += &format!("\nchild {:?} = {:#?}", child_id, child);
                    }
                }
            }
            Item::Root => (),
        }
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

pub fn run(analysis: Analysis) {
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

    ui.set_user_data(UserData { analysis });
    ui.run();
}
