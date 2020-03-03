#[macro_use] extern crate lazy_static;

use rsbrowse::analysis::Analysis;
use rsbrowse::browser::{Browser, Item};
use std::path::Path;

lazy_static! {
    static ref BROWSER: Browser = {
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
        Browser::new(Analysis::load(&path))
    };
}

fn iter_labels<'a, T>(vec: &'a Vec<(String, T)>) -> impl Iterator<Item=&'a str> {
    vec.iter().map(|(label, _)| label.as_str())
}

trait VecExt<'a, T> {
    fn contains_label(&self, s: &str) -> bool;
    fn by_label(&'a self, s: &str) -> &'a T;
    fn labels(&self) -> Vec<String>;
}

impl<'a, T> VecExt<'a, T> for Vec<(String, T)> {
    fn contains_label(&self, s: &str) -> bool {
        iter_labels(self).find(|label| *label == s).is_some()
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
        Item::Root => {
            match b {
                Item::Root => true,
                _ => false,
            }
        }
        Item::Def(a) => {
            match b {
                Item::Def(b) => {
                    a.id == b.id
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

#[test]
fn it_loads() {
    let _force_it_to_load = &BROWSER;
}

#[test]
fn list_crates() {
    let crates = BROWSER.list_crates();
    assert!(crates.contains_label("testcrate"));
    assert!(crates.contains_label("log"));
}

#[test]
fn list_items() {
    let crates = BROWSER.list_crates();
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
    assert_eq!(x_s_items.labels(), &["impl Self"]);

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

    let y_s_self = y_s_items.by_label("impl Self");
    let y_s_self_items = BROWSER.list_items(crate_id, y_s_self);
    assert_eq!(y_s_self_items.labels(), &["fn g"]);

    let y_s_trait = y_s_items.by_label("impl ::Trait");
    let y_s_trait_items = BROWSER.list_items(crate_id, y_s_trait);
    // This is WRONG but describes current behavior.
    // It has items for both the implementation provided in the trait, as well as the one in the
    // impl specific to S, which overrides the other one.
    assert_eq!(y_s_trait_items.labels(), &["fn method", "fn method"]);

    let z_s_trait = z_s_items.by_label("impl ::Trait");
    let z_s_trait_items = BROWSER.list_items(crate_id, z_s_trait);
    // This one inherits the default, so it only has one.
    assert_eq!(z_s_trait_items.labels(), &["fn method"]);

    // One of the two <y::S as Trait>::method should be the same as <z::S as Trait>::method
    // FIXME: remove extra check when bug is fixed
    assert!(
        items_eq(
            &y_s_trait_items[0].1,
            &z_s_trait_items[0].1,
        )
        || items_eq(
            &y_s_trait_items[1].1,
            &z_s_trait_items[0].1,
        ));

    // But the two <y::S as Trait>::method should not be the same as each other. (FIXME: remove
    // when bug is fixed)
    assert!(!items_eq(&y_s_trait_items[0].1, &y_s_trait_items[1].1));

    // Pane 5 (all empty)
    for stuff in &[&x_s_self_items, &y_s_self_items, &y_s_trait_items, &z_s_trait_items] {
        for (_label, item) in *stuff {
            assert!(BROWSER.list_items(crate_id, &item).is_empty());
        }
    }
}
