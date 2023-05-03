use std::fs::File;
use std::io::{BufReader, BufRead};

use scip::types::{Symbol, SymbolRole};
use scip::types::descriptor::Suffix;

use crate::analysis_scip::{Analysis, CrateId};
use crate::browser_trait::{Browser, self};
use crate::sort_by_label;

pub struct ScipBrowser {
    analysis: Analysis,
}

impl ScipBrowser {
    pub fn new(analysis: Analysis) -> Self {
        Self { analysis }
    }

    fn item_label(&self, kind: Suffix, name: &str, _sym: &Symbol) -> Option<String> {
        use Suffix::*;
        let prefix = match kind {
            UnspecifiedSuffix | TypeParameter | Parameter | Local => return None, // not interested
            Namespace => "mod",
            Package => "package?", // TODO: idk what this is
            Type => "type", // TODO: differentiate btw struct, enum, etc.
            Term => {
                // Field
                // TODO: get the field type somehow
                return Some(format!("{name}: unknown"));
            },
            Method => "fn",
            Meta => "meta?", // TODO: idk what this is
            Macro => "macro",
        };
        Some(format!("{prefix} {name}"))
    }
}

impl Browser for ScipBrowser {
    type CrateId = CrateId;
    type Item = Item;

    fn list_crates(&self) -> Vec<(String, Self::CrateId)> {
        let mut crates: Vec<_> = self.analysis.crate_ids().into_iter().collect();
        crates.sort_unstable_by(|a, b| a.name.cmp(&b.name));
        crates.into_iter()
            .map(|c| (c.name.clone(), c))
            .collect()
    }

    fn list_items(&self, crate_id: &Self::CrateId, parent: &Self::Item) -> Vec<(String, Self::Item)> {
        let items: Vec<Symbol> = match parent {
            Item::Root => self.analysis.crate_symbols(crate_id).collect(),
            Item::Symbol(sym) => self.analysis.symbols_under(sym).collect(),
        };

        let mut items = items.into_iter()
            .filter_map(|sym| {
                let d = match sym.descriptors.last().as_ref() {
                    Some(d) => d.clone(),
                    None => panic!("symbol {sym:?} has no descriptors"),
                };
                let name = &d.name;
                let kind = match d.suffix.enum_value() {
                    Ok(v) => v,
                    Err(n) => panic!("symbol {sym:?} has unrecognized suffix value {n}"),
                };
                let label = self.item_label(kind, name, &sym)?;
                Some((label, Item::Symbol(sym)))
            })
            .collect();

        sort_by_label(&mut items);
        items
    }

    fn get_info(&self, _crate_id: &Self::CrateId, item: &Self::Item) -> String {
        let sym = match item {
            Item::Root => return String::new(),
            Item::Symbol(sym) => sym,
        };
        let sym_str = scip::symbol::format_symbol(sym.to_owned());
        let item = match self.analysis.specific(&sym_str) {
            None => return String::from("symbol not found"),
            Some(item) => item,
        };
        item.doc.join("\n")
    }

    fn get_debug_info(&self, _crate_id: &Self::CrateId, item: &Self::Item) -> String {
        if let Item::Symbol(sym) = item {
            let text  = scip::symbol::format_symbol(sym.to_owned());
            let item = self.analysis.specific(&text);
            return format!("{item:#?}");
        }
        format!("{item:#?}")
    }

    fn get_source(&self, item: &Self::Item) -> (String, Option<usize>) {
        let sym = match item {
            Item::Root => return (String::new(), None),
            Item::Symbol(sym) => sym,
        };
        let sym_str = scip::symbol::format_symbol(sym.to_owned());
        let item = match self.analysis.specific(&sym_str) {
            None => return (String::from("symbol not found"), Some(0)),
            Some(item) => item,
        };
        for (path, occ) in &item.occur {
            if occ.symbol_roles & SymbolRole::Definition as i32 != 0 {
                match File::open(&path) {
                    Ok(f) => {
                        let mut txt = String::new();
                        for (i, line) in BufReader::new(f)
                            .lines()
                            .enumerate()
                        {
                            txt += &format!("{}: ", i + 1);
                            txt += &line.unwrap_or_else(|e| format!("<Read Error: {}>", e));
                            txt.push('\n');
                        }
                        return (txt, Some(occ.range[0] as usize))
                    }
                    Err(e) => {
                        return (format!("Error opening source: {}", e), Some(0))
                    }
                }
            }
        }

        (String::from("No source found."), None)
    }
}

#[derive(Debug, Clone)]
pub enum Item {
    Root,
    Symbol(Symbol),
}

impl browser_trait::Item for Item {
    fn crate_root() -> Self {
        Self::Root
    }
}