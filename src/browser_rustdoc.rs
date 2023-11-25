use crate::analysis::{Analysis, CrateId};
use crate::browser_trait::{self, Browser};
use std::fmt::Write;

pub struct RustdocBrowser {
    analysis: Analysis,
}

impl RustdocBrowser {
    pub fn new(analysis: Analysis) -> Self {
        Self {
            analysis,
        }
    }
}

impl Browser for RustdocBrowser {
    type CrateId = CrateId;
    type Item = Item;

    fn list_crates(&self) -> Vec<(String, CrateId)> {
        let mut crates = self.analysis.crate_ids()
            //.filter(|c| !self.analysis.stdlib_crates.contains(c))
            .map(|c| (crate_label(&c), c))
            .collect::<Vec<_>>();

        sort_by_label(&mut crates);

        crates
    }

    fn list_items(&self, crate_id: &CrateId, parent: &Item) -> Vec<(String, Item)> {
        let parent_id = match parent {
            Item::Item(item) => Some(item.id.clone()),
            _ => None,
        };
        
        let mut items = self.analysis.items(crate_id, parent_id)
            .map(|item| (item_label(item), Item::Item(item.clone())))
            .collect::<Vec<_>>();
        sort_by_label(&mut items);

        items
    }

    fn get_info(&self, crate_id: &CrateId, item: &Item) -> String {
        let mut txt = String::new();
        match item {
            Item::Item(item) => {
                if let Some(docs) = &item.docs {
                    txt += &docs;
                    txt.push('\n');
                }
                if let Some(span) = &item.span {
                    write!(txt, "defined in {:?}\nstarting on line {}",
                        span.filename,
                        span.begin.0).unwrap();
                }
            }
            Item::Root => {
                write!(txt, "crate root of {crate_id:?}").unwrap();
            }
        }
        txt
    }

    fn get_debug_info(&self, crate_id: &CrateId, item: &Item) -> String {
        format!("{crate_id:?}: {item:#?}")
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
            for (i, line) in BufReader::new(f)
                .lines()
                .enumerate()
            {
                write!(txt, "{}: ", i + 1).unwrap();
                txt += &line.unwrap_or_else(|e| format!("<Read Error: {e}>"));
                txt.push('\n');
            }
            let line = span.begin.0 - 1;
            (txt, line)
        }
        Err(e) => {
            (format!("Error opening source: {e}"), 0)
        }
    }
}

fn cmp_labels<T>(a: &(String, T), b: &(String, T)) -> std::cmp::Ordering {
    a.0.cmp(&b.0)
}

fn sort_by_label<T>(slice: &mut [(String, T)]) {
    slice.sort_unstable_by(cmp_labels);
}

fn crate_label(c: &CrateId) -> String {
    /*match c.crate_type {
        CrateType::Bin => format!("{} (bin)", c.name),
        CrateType::ProcMacro => format!("{} (proc-macro)", c.name),
        CrateType::Lib => c.name.clone(),
        CrateType::CDylib => format!("{} (cdylib)", c.name),
        CrateType::Dylib => format!("{} (dylib)", c.name),
    }*/
    c.name.clone()
}

fn item_label(item: &rustdoc_types::Item) -> String {
    use rustdoc_types::ItemEnum::*;
    let name = item.name.as_deref().unwrap_or("<unnamed>");
    let prefix = match &item.inner {
        Module(_) => "mod",
        ExternCrate { name, .. } => return format!("extern crate {name}"),
        Import(i) => return format!("use {}", i.source), // TODO: globs
        Union(_) => "union",
        Struct(_) => "struct",
        StructField(f) => return format!("{}: {}", name, type_label(f)),
        Enum(_) => "enum",
        Variant(_) => "enum variant",
        Function(_) => "fn", // TODO: include signature?
        Trait(_) => "trait",
        TraitAlias(_) => "trait alias",
        Impl(i) => {
            let name = type_label(&i.for_);
            return if let Some(trait_) = &i.trait_ {
                format!("impl {} for {name}", trait_.name)
            } else {
                format!("impl {name}")
            };
        }
        TypeAlias(_) => "type",
        OpaqueTy(_) => "opaque type",
        Constant(c) => return format!("const {}: {}", name, c.expr),
        Static(s) => return format!("static {}: {}", name, s.expr),
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

fn type_label(ty: &rustdoc_types::Type) -> String {
    use rustdoc_types::Type::*;
    match ty {
        ResolvedPath(p) => p.name.clone(),
        DynTrait(dt) => "dyn ".to_owned() + &dt.traits.iter().map(|t| t.trait_.name.clone()).collect::<Vec<_>>().join(" + "),
        Generic(g) => g.to_owned(),
        Primitive(p) => p.to_owned(),
        FunctionPointer(fp) => {
            let args = fp.decl.inputs.iter()
                .map(|(name, ty)| format!("{name}: {}", type_label(ty)))
                .collect::<Vec<_>>()
                .join(", ");
            let ret = match &fp.decl.output {
                Some(ty) => format!(" -> {}", type_label(ty)),
                None => String::new(),
            };
            format!("fn({args}){ret}")
        },
        Tuple(types) => format!("({})",
            types.iter().map(type_label).collect::<Vec<_>>().join(", ")),
        Slice(ty) => format!("&[{}]", type_label(ty)),
        Array { type_, len } => format!("[{}; {len}]", type_label(type_)),
        ImplTrait(t) => {
            use rustdoc_types::GenericBound::*;
            format!("impl {}",
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
            format!("*{} {}",
                if *mutable { "mut" } else { "const" },
                type_label(type_),
            )
        },
        BorrowedRef { lifetime, mutable, type_ } => {
            format!("&{}{} {}",
                lifetime.as_deref().unwrap_or_default(),
                if *mutable { "mut" } else { "" },
                type_label(type_),
            )
        },
        QualifiedPath { name, args:_ , self_type, trait_ } => {
            if let Some(trait_) = trait_ {
                format!("<{} as {}>::{name}", type_label(self_type), trait_.name)
            } else {
                format!("{}::{name}", type_label(self_type))
            }
        },
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum Item {
    Root,
    Item(rustdoc_types::Item),
}

impl browser_trait::Item for Item {
    fn crate_root() -> Self {
        Item::Root
    }
}
