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
            .filter(|c| !self.analysis.stdlib_crates.contains(c))
            .map(|c| (crate_label(&c), c))
            .collect::<Vec<_>>();

        sort_by_label(&mut crates);

        crates
    }

    fn get_maybe_external_trait<'a>(&'a self, crate_id: &'a CrateId, trait_id: rls_data::Id)
        -> (bool, &'a CrateId, rls_data::Id)
    {
        if trait_id.krate != 0 {
            (
                true,
                self.analysis.get_external_crate_id(crate_id, trait_id)
                    .expect("nonexistent external crate"),
                rls_data::Id {
                    krate: 0,
                    index: trait_id.index,
                }
            )
        } else {
            (false, crate_id, trait_id)
        }
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
                        .map(|impl_details| {
                            let trait_name = match impl_details.trait_id {
                                Some(trait_id) => {
                                    let (is_external, trait_crate, trait_id)
                                        = self.get_maybe_external_trait(crate_id, trait_id);
                                    self.analysis.get_def(trait_crate, trait_id)
                                        .map(|t| if is_external {
                                            trait_crate.name.clone() + &t.qualname
                                        } else {
                                            t.qualname.clone()
                                        })
                                        .unwrap_or_else(|| {
                                            format!("{}::(unresolved trait at {}:{})",
                                                trait_crate.name,
                                                impl_details.span.file_name.display(),
                                                impl_details.span.line_start.0)
                                        })
                                }
                                None => "Self".to_owned(),
                            };
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
                    let (is_external, trait_crate, trait_id) = self.get_maybe_external_trait(
                        crate_id, trait_id);

                    let children = self.analysis.get_def(&trait_crate, trait_id)
                        .map(|def| &def.children[..])
                        .unwrap_or(&[]);

                    for id in children {
                        if let Some(method) = self.analysis.get_def(&trait_crate, *id) {
                            // Add to the map only if not existing (if it already exists it means
                            // the method has been overridden).
                            methods.entry(def_label(method))
                                .or_insert_with(|| {
                                    if is_external {
                                        Item::ExternalDef(trait_crate.to_owned(), method.clone())
                                    } else {
                                        Item::Def(method.clone())
                                    }
                                });
                        }
                    }
                }

                items.extend(methods.into_iter());
                sort_by_label(&mut items);

                for (ref mut label, ref item) in items.iter_mut() {
                    // Externally-defined items should get a suffix indicating the crate name.
                    // This lets users easily see which trait methods are overridden and which are
                    // defaults.
                    if let Item::ExternalDef(external_crate_id, _) = item {
                        *label += &format!(" ({})", external_crate_id.name);
                    }
                }
            }

            Item::ExternalDef(_crate, _def) => {
                // Currently this only represents default methods of external traits, so they can't
                // have any child items.
            }
        }

        items
    }

    pub fn get_info(&self, crate_id: &CrateId, item: &Item) -> String {
        let mut txt = String::new();
        match item {
            Item::Def(def) | Item::ExternalDef(_, def) => {
                if !def.docs.is_empty() {
                    txt += &def.docs;
                    txt.push('\n');
                }
                txt += &format!("defined in {:?}\nstarting on line {}",
                    def.span.file_name,
                    def.span.line_start.0);
            }
            Item::Impl(imp) => {
                if let Some(t) = imp.trait_id {
                    if let Some(tdef) = self.analysis.get_def(crate_id, t) {
                        txt += &format!("implementation of trait {}", tdef.qualname);
                    } else {
                        txt += "implementation of unresolved trait";
                    }
                } else {
                    txt += "inherent impl";
                    // nothing else to show really
                }
            }
            Item::Root => {
                txt += &format!("crate root of {:?}", crate_id);
            }
        }
        txt
    }

    pub fn get_debug_info(&self, crate_id: &CrateId, item: &Item) -> String {
        let mut txt = format!("{:#?}", item);
        let add_children = |txt: &mut String, crate_id, children: &[rls_data::Id]| {
            for child_id in children {
                if let Some(child) = self.analysis.get_def(crate_id, *child_id) {
                    *txt += &format!("\nchild {:?} = {:#?}", child_id, child);
                }
            }
        };
        match item {
            Item::Def(def) => {
                add_children(&mut txt, crate_id, &def.children);
            }
            Item::ExternalDef(ext_crate_id, def) => {
                txt += &format!("defined in external crate {}\n", crate_id.name);
                add_children(&mut txt, ext_crate_id, &def.children);
            }
            Item::Impl(impl_details) => {
                let imp = self.analysis.get_impl(crate_id, impl_details.impl_id).unwrap();
                txt += &format!("\nimpl: {:#?}", imp);
                add_children(&mut txt, crate_id, &imp.children);
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
        CrateType::Dylib => format!("{} (dylib)", c.name),
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
        DefKind::Const => return format!("const {}: {}", def.name, def.value),
        DefKind::Static => return format!("static {}: {}", def.name, def.value),
        DefKind::ForeignStatic => return format!("extern static {}: {}", def.name, def.value),
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
    ExternalDef(CrateId, rls_data::Def),
    Impl(ImplDetails),
}
