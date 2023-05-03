#[macro_use] extern crate lazy_static;

use rsbrowse::analysis_rls::Analysis;
use rsbrowse::browser_rls::{RlsBrowser, Item};
use rsbrowse::browser_trait::Browser;
use std::path::Path;

lazy_static! {
    static ref BROWSER: RlsBrowser = {
        let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/testcrate"));

        let status = std::process::Command::new("cargo")
            .arg("clean")
            .current_dir(&path)
            .status()
            .expect("Failed to run 'cargo clean' on test crate");
        if !status.success() {
            panic!("Failed to run 'cargo clean' on test crate");
        }

        Analysis::generate(&path).expect("Failed to generate analysis data.");
        RlsBrowser::new(Analysis::load(&path))
    };
}

fn iter_labels<T>(items: &[(String, T)]) -> impl Iterator<Item=&str> {
    items.iter().map(|(label, _)| label.as_str())
}

trait VecExt<'a, T> {
    fn contains_label(&self, s: &str) -> bool;
    fn by_label(&'a self, s: &str) -> &'a T;
    fn labels(&self) -> Vec<String>;
}

impl<'a, T> VecExt<'a, T> for Vec<(String, T)> {
    fn contains_label(&self, s: &str) -> bool {
        iter_labels(self).any(|label| label == s)
    }
    fn by_label(&'a self, s: &str) -> &'a T {
        &self.iter()
            .find(|(label, _)| label == s)
            .expect("not found")
            .1
    }
    fn labels(&self) -> Vec<String> {
        iter_labels(self).map(|s| s.to_owned()).collect()
    }
}

fn items_eq(a: &Item, b: &Item) -> bool {
    match a {
        Item::Root => matches!(b, Item::Root),
        Item::Def(a) => {
            match b {
                Item::Def(b) => {
                    a.id == b.id
                }
                _ => false,
            }
        }
        Item::ExternalDef(cr_a, a) => {
            match b {
                Item::ExternalDef(cr_b, b) => {
                    cr_a.disambiguator == cr_b.disambiguator
                        && a.id == b.id
                }
                _ => false,
            }
        }
        Item::Impl(a) => {
            match b {
                Item::Impl(b) => {
                    a.impl_id == b.impl_id
                        && a.trait_id == b.trait_id
                        && a.impl_on == b.impl_on
                }
                _ => false,
            }
        }
    }
}

trait ItemExt {
    fn unwrap_def(&self) -> &rls_data::Def;
}

impl ItemExt for Item {
    fn unwrap_def(&self) -> &rls_data::Def {
        match self {
            Item::Def(def) => def,
            _ => panic!("not an Item::Def"),
        }
    }
}

#[test]
fn list_items() {
    let crates = BROWSER.list_crates();
    assert_eq!(crates.labels(), &["externcrate", "testcrate", "testcrate (bin)"]);

    let crate_id = crates.by_label("testcrate");

    // Pane 1

    let root_items = BROWSER.list_items(&crate_id, &Item::Root);
    assert_eq!(root_items.labels(), &[
        "mod x",
        "mod y",
        "mod z",
        "trait Trait",
    ]);

    // Pane 2

    let mod_x = root_items.by_label("mod x");
    let mod_x_items = BROWSER.list_items(crate_id, mod_x);
    assert_eq!(mod_x_items.labels(), &["struct S"]);

    let mod_y = root_items.by_label("mod y");
    let mod_y_items = BROWSER.list_items(crate_id, mod_y);
    assert_eq!(mod_y_items.labels(), &["struct S"]);

    let mod_z = root_items.by_label("mod z");
    let mod_z_items = BROWSER.list_items(crate_id, mod_z);
    assert_eq!(mod_z_items.labels(), &["struct S"]);

    // Assert that the three "struct S" defs are not the same.
    assert!(!items_eq(
        mod_x_items.by_label("struct S"),
        mod_y_items.by_label("struct S")));
    assert!(!items_eq(
        mod_y_items.by_label("struct S"),
        mod_z_items.by_label("struct S")));

    let trait_trait = root_items.by_label("trait Trait");
    let trait_items = BROWSER.list_items(crate_id, trait_trait);
    assert_eq!(trait_items.labels(), &["fn method"]);

    // Pane 3

    let x_s = mod_x_items.by_label("struct S");
    let x_s_items = BROWSER.list_items(crate_id, x_s);
    assert_eq!(x_s_items.labels(), &[
        "impl Self",
        "impl core::fmt::Display",
        "impl externcrate::ExternTrait",
    ]);

    let y_s = mod_y_items.by_label("struct S");
    let y_s_items = BROWSER.list_items(crate_id, y_s);
    assert_eq!(y_s_items.labels(), &["impl ::Trait", "impl Self"]);

    let z_s = mod_z_items.by_label("struct S");
    let z_s_items = BROWSER.list_items(crate_id, z_s);
    assert_eq!(z_s_items.labels(), &["impl ::Trait"]);

    // Pane 4

    let x_s_self = x_s_items.by_label("impl Self");
    let x_s_self_items = BROWSER.list_items(crate_id, x_s_self);
    assert_eq!(x_s_self_items.labels(), &["fn f"]);

    let x_s_extern = x_s_items.by_label("impl externcrate::ExternTrait");
    let x_s_extern_items = BROWSER.list_items(crate_id, x_s_extern);
    assert_eq!(x_s_extern_items.labels(), &[
        "fn default_method (externcrate)",
        "fn required_method",
    ]);

    let y_s_self = y_s_items.by_label("impl Self");
    let y_s_self_items = BROWSER.list_items(crate_id, y_s_self);
    assert_eq!(y_s_self_items.labels(), &["fn g"]);

    let y_s_trait = y_s_items.by_label("impl ::Trait");
    let y_s_trait_items = BROWSER.list_items(crate_id, y_s_trait);
    assert_eq!(y_s_trait_items.labels(), &["fn method"]);

    // It has to be the overridden one, not the default on the trait (which is "::Trait::method").
    assert_eq!(y_s_trait_items[0].1.unwrap_def().qualname, "<S as Trait>::method");

    let z_s_trait = z_s_items.by_label("impl ::Trait");
    let z_s_trait_items = BROWSER.list_items(crate_id, z_s_trait);
    assert_eq!(z_s_trait_items.labels(), &["fn method"]);

    // This one inherits the default.
    assert_eq!(z_s_trait_items[0].1.unwrap_def().qualname, "::Trait::method");

    // Pane 5 (all empty)
    for stuff in &[&x_s_self_items, &y_s_self_items, &y_s_trait_items, &z_s_trait_items] {
        for (_label, item) in *stuff {
            assert!(BROWSER.list_items(crate_id, &item).is_empty());
        }
    }
}
