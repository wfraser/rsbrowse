use crate::analysis::{Analysis, CrateId, CrateType, ImplDetails};
use rls_data::{Def, DefKind};
use std::collections::hash_map::*;

pub struct Browser {
    analysis: Analysis,
}

impl Browser {
    pub fn new(analysis: Analysis) -> Self {
        Self {
            analysis,
        }
    }

    pub fn list_crates(&self) -> Vec<(String, CrateId)> {
        let mut crates = self.analysis.crate_ids()
            .map(|c| (crate_label(&c), c))
            .collect::<Vec<_>>();

        sort_by_label(&mut crates);

        crates
    }

    pub fn list_items(&self, crate_id: &CrateId, parent: &Item) -> Vec<(String, Item)> {
        let mut items = vec![];
        match parent {
            Item::Root | Item::Def(_) => {
                let parent_id = match parent {
                    Item::Def(def) => Some(def.id),
                    _ => None,
                };

                let mut defs = self.analysis.defs(crate_id, parent_id)
                    .map(|def| (def_label(def), Item::Def(def.clone())))
                    .collect::<Vec<_>>();
                sort_by_label(&mut defs);

                items.extend(defs);

                if let Some(id) = parent_id {
                    let mut impls = self.analysis.impls(crate_id, id)
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
                            let trait_name = impl_details.trait_id
                                .map(|id| {
                                    self.analysis.get_def(crate_id, id)
                                        .expect("invalid trait ID")
                                        .qualname
                                        .clone()
                                })
                                .unwrap_or_else(|| "Self".to_owned());

                            (format!("impl {}", trait_name), Item::Impl(impl_details))
                        })
                        .collect::<Vec<_>>();

                    sort_by_label(&mut impls);
                    items.extend(impls);
                }
            }

            Item::Impl(impl_details) => {
                let imp = self.analysis.get_impl(crate_id, impl_details.impl_id)
                    .expect("invalid impl ID");

                let mut methods = HashMap::new();

                // imp.children has methods for inherent impls (impl Foo) and overrides of trait
                // methods.
                for id in &imp.children {
                    if let Some(method) = self.analysis.get_def(&crate_id, *id) {
                        methods.insert(def_label(method), Item::Def(method.clone()));
                    }
                }

                // Trait methods.
                if let Some(trait_id) = impl_details.trait_id {
                    let def = self.analysis.get_def(&crate_id, trait_id).expect("no such trait");
                    for id in &def.children {
                        if let Some(method) = self.analysis.get_def(&crate_id, *id) {
                            // Add to the map only if not existing (if it already exists it means
                            // the method has been overridden).
                            methods.entry(def_label(method)).or_insert_with(|| Item::Def(method.clone()));
                        }
                    }
                }

                items.extend(methods.into_iter());
                sort_by_label(&mut items);
            }
        }

        items
    }

    pub fn get_debug_info(&self, crate_id: &CrateId, item: &Item) -> String {
        let mut txt = format!("{:#?}", item);
        match item {
            Item::Def(def) => {
                for child_id in &def.children {
                    if let Some(child) = self.analysis.get_def(crate_id, *child_id) {
                        txt += &format!("\nchild {:?} = {:#?}", child_id, child);
                    }
                }
            }
            Item::Impl(impl_details) => {
                let imp = self.analysis.get_impl(crate_id, impl_details.impl_id).unwrap();
                txt += &format!("\nimpl: {:#?}", imp);
                for child_id in &imp.children {
                    if let Some(child) = self.analysis.get_def(crate_id, *child_id) {
                        txt += &format!("\nchild {:?} = {:#?}", child_id, child);
                    }
                }
            }
            Item::Root => (),
        }
        txt
    }
}

fn cmp_labels<T>(a: &(String, T), b: &(String, T)) -> std::cmp::Ordering {
    a.0.cmp(&b.0)
}

fn sort_by_label<T>(vec: &mut Vec<(String, T)>) {
    vec.sort_unstable_by(cmp_labels);
}

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

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum Item {
    Root,
    Def(rls_data::Def),
    Impl(ImplDetails),
}
