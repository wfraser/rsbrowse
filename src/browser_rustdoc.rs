use crate::analysis::{Analysis, Item, ItemId};
use crate::browser_trait::Browser;
use std::fmt::Write;

pub struct RustdocBrowser {
    analysis: Analysis,
}

impl RustdocBrowser {
    pub fn new(analysis: Analysis) -> Self {
        Self { analysis }
    }

    fn item_label(&self, id: ItemId, item: &rustdoc_types::Item) -> String {
        use rustdoc_types::ItemEnum::*;
        let name = item.name.as_deref().unwrap_or("<unnamed>");
        let prefix = match &item.inner {
            Module(_) => "mod",
            ExternCrate { name, .. } => return format!("extern crate {name}"),
            Import(i) => return format!("use {}", i.source) + if i.glob { "::*" } else { "" },
            Union(_) => "union",
            Struct(_) => "struct",
            StructField(f) => return format!("{}: {}", name, type_label(f)),
            Enum(_) => "enum",
            Variant(_) => "variant",
            Function(_) => "fn", // TODO: include signature?
            Trait(_) => "trait",
            TraitAlias(_) => "trait alias",
            Impl(i) => {
                return if let Some(trait_) = &i.trait_ {
                    let full_path = self.analysis.get_path(id.crate_sibling(&trait_.id));
                    if full_path[0] == id.crate_name() {
                        // trait in local crate, use trait name
                        format!("impl {}", trait_.name)
                    } else {
                        // trait in foreign crate, use full path
                        format!("impl {}", full_path.join("::"))
                    }
                } else {
                    "impl Self".to_string()
                };
            }
            TypeAlias(_) => "type",
            OpaqueTy(_) => "opaque type",
            Constant(c) => return format!("const {}: {}", name, type_label(&c.type_)),
            Static(s) => return format!("static {}: {}", name, type_label(&s.type_)),
            ForeignType => "extern type",
            Macro(_) => "macro",
            ProcMacro(_) => "proc macro",
            Primitive(_) => "",
            AssocConst { type_, default } => {
                return if let Some(default) = default {
                    format!("const {name}: {} = {default}", type_label(type_))
                } else {
                    format!("const {name}: {}", type_label(type_))
                }
            }
            AssocType { default, .. } => {
                return if let Some(default) = default {
                    format!("type {name} = {}", type_label(default))
                } else {
                    format!("type {name}")
                };
            }
        };
        format!("{prefix} {name}")
    }
}

impl<'a> Browser for &'a RustdocBrowser {
    type Item = Item<'a>;
    type ItemId = ItemId<'a>;

    fn list_crates(&self) -> Vec<(String, ItemId<'a>)> {
        let mut crates = self
            .analysis
            .crate_ids()
            //.filter(|c| !self.analysis.stdlib_crates.contains(c))
            .map(|item_id| (crate_label(&item_id), item_id))
            .collect::<Vec<_>>();

        sort_by_label(&mut crates);

        crates
    }

    fn list_items(&self, parent_id: &ItemId<'a>) -> Vec<(String, (ItemId<'a>, Item<'a>))> {
        let mut items = self
            .analysis
            .items(parent_id)
            .filter_map(|(id, item)| {
                let inner = match item {
                    Item::Root => return None,
                    Item::Item(item) => item,
                };

                // Remove the clutter of automatically derived, blanket, and synthetic trait impls.
                use rustdoc_types::ItemEnum::*;
                if inner.attrs.iter().any(|a| a == "#[automatically_derived]") {
                    return None;
                }
                match &inner.inner {
                    Impl(i) if i.blanket_impl.is_some() || i.synthetic => None,
                    _ => Some((self.item_label(id.clone(), inner), (id, item))),
                }
            })
            .collect::<Vec<_>>();
        sort_by_label(&mut items);

        items
    }

    fn get_info(&self, item: &Item<'a>) -> String {
        let mut txt = String::new();
        match item {
            Item::Item(item) => {
                if let Some(docs) = &item.docs {
                    txt += &docs;
                    txt.push('\n');
                }
                if let Some(span) = &item.span {
                    write!(
                        txt,
                        "defined in {:?}\nstarting on line {}",
                        span.filename, span.begin.0
                    )
                    .unwrap();
                }
            }
            Item::Root => {
                write!(txt, "crate root").unwrap();
            }
        }
        txt
    }

    fn get_debug_info(&self, item: &Item) -> String {
        format!("{item:#?}")
    }

    fn get_source(&self, item: &Item) -> (String, Option<usize>) {
        match item {
            Item::Item(item) => {
                let (txt, line) = get_source_for_item(item);
                (txt, Some(line))
            }
            Item::Root => (String::new(), None),
        }
    }
}

fn get_source_for_item(item: &rustdoc_types::Item) -> (String, usize) {
    use std::fs::File;
    use std::io::{BufRead, BufReader};
    let Some(span) = &item.span else {
        return (String::new(), 0);
    };
    match File::open(&span.filename) {
        Ok(f) => {
            let mut txt = String::new();
            for (i, line) in BufReader::new(f).lines().enumerate() {
                write!(txt, "{}: ", i + 1).unwrap();
                txt += &line.unwrap_or_else(|e| format!("<Read Error: {e}>"));
                txt.push('\n');
            }
            let line = span.begin.0 - 1;
            (txt, line)
        }
        Err(e) => (format!("Error opening source: {e}"), 0),
    }
}

fn cmp_labels(a: &str, b: &str) -> std::cmp::Ordering {
    // Fields (assuming they contain ": ") go first
    a.contains(": ")
        .cmp(&b.contains(": "))
        .reverse() // less = goes first
        .then_with(|| a.cmp(b))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn cmp_test() {
        use std::cmp::Ordering::*;
        assert_eq!(cmp_labels("a: a", "b: b"), Less);
        assert_eq!(cmp_labels("a", "z: z"), Greater);
        assert_eq!(cmp_labels("a", "b"), Less);
    }
}

fn sort_by_label<T>(slice: &mut [(String, T)]) {
    slice.sort_unstable_by(|(a, _), (b, _)| cmp_labels(a, b));
}

fn crate_label(id: &ItemId) -> String {
    /*match c.crate_type {
        CrateType::Bin => format!("{} (bin)", c.name),
        CrateType::ProcMacro => format!("{} (proc-macro)", c.name),
        CrateType::Lib => c.name.clone(),
        CrateType::CDylib => format!("{} (cdylib)", c.name),
        CrateType::Dylib => format!("{} (dylib)", c.name),
    }*/
    id.crate_name().to_owned()
}

fn generic_label(g: &rustdoc_types::GenericArgs) -> String {
    use rustdoc_types::{GenericArg, GenericArgs};
    use std::borrow::Cow;
    let mut s = String::new();
    match g {
        GenericArgs::AngleBracketed { args, bindings } => {
            if args.is_empty() {
                return s;
            }
            s.push('<');
            s.push_str(
                &args
                    .iter()
                    .map(|arg| match arg {
                        GenericArg::Lifetime(s) => Cow::Borrowed(s.as_str()),
                        GenericArg::Type(ty) => Cow::Owned(type_label(ty)),
                        GenericArg::Const(c) => Cow::Owned(format!("{c:?}")),
                        GenericArg::Infer => Cow::Borrowed("_"),
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            // TODO: dunno what to do with these
            s.push_str(
                &bindings
                    .iter()
                    .map(|b| format!("{b:?}"))
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            s.push('>');
        }
        GenericArgs::Parenthesized { inputs, output } => {
            s.push('(');
            s.push_str(&inputs.iter().map(type_label).collect::<Vec<_>>().join(", "));
            s.push(')');
            if let Some(ty) = output {
                s.push_str(" -> ");
                s.push_str(&type_label(ty));
            }
        }
    }
    s
}

fn type_label(ty: &rustdoc_types::Type) -> String {
    use rustdoc_types::Type::*;
    match ty {
        ResolvedPath(p) => {
            let mut s = p.name.clone();
            if let Some(args) = &p.args {
                s.push_str(&generic_label(args));
            }
            s
        }
        DynTrait(dt) => {
            "dyn ".to_owned()
                + &dt
                    .traits
                    .iter()
                    .map(|t| {
                        t.trait_.name.clone()
                            + &t.trait_
                                .args
                                .as_ref()
                                .map(|g| generic_label(g))
                                .unwrap_or_default()
                    })
                    .collect::<Vec<_>>()
                    .join(" + ")
        }
        Generic(g) => g.to_owned(),
        Primitive(p) => p.to_owned(),
        FunctionPointer(fp) => {
            let args = fp
                .decl
                .inputs
                .iter()
                .map(|(name, ty)| format!("{name}: {}", type_label(ty)))
                .collect::<Vec<_>>()
                .join(", ");
            let ret = match &fp.decl.output {
                Some(ty) => format!(" -> {}", type_label(ty)),
                None => String::new(),
            };
            format!("fn({args}){ret}")
        }
        Tuple(types) => format!(
            "({})",
            types.iter().map(type_label).collect::<Vec<_>>().join(", ")
        ),
        Slice(ty) => format!("[{}]", type_label(ty)),
        Array { type_, len } => format!("[{}; {len}]", type_label(type_)),
        ImplTrait(t) => {
            use rustdoc_types::GenericBound::*;
            format!(
                "impl {}",
                t.iter()
                    .map(|g| match g {
                        TraitBound { trait_, .. } => trait_.name.as_str(),
                        Outlives(o) => o.as_str(),
                    })
                    .collect::<Vec<_>>()
                    .join(" + "),
            )
        }
        Infer => "_".to_owned(),
        RawPointer { mutable, type_ } => {
            format!(
                "*{} {}",
                if *mutable { "mut" } else { "const" },
                type_label(type_),
            )
        }
        BorrowedRef {
            lifetime,
            mutable,
            type_,
        } => {
            let mut s = "&".to_owned();
            if let Some(l) = lifetime {
                s.push_str(l);
                s.push(' ');
            }
            if *mutable {
                s.push_str("mut ");
            }
            s.push_str(&type_label(type_));
            s
        }
        QualifiedPath {
            name,
            args: _,
            self_type,
            trait_,
        } => {
            if let Some(trait_) = trait_ {
                format!("<{} as {}>::{name}", type_label(self_type), trait_.name)
            } else {
                format!("{}::{name}", type_label(self_type))
            }
        }
    }
}
