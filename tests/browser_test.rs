#[macro_use]
extern crate lazy_static;

use rsbrowse::analysis::{Analysis, Item};
use rsbrowse::browser_rustdoc::RustdocBrowser;
use rsbrowse::browser_trait::Browser;
use std::path::Path;

lazy_static! {
    static ref BROWSER_STATIC: RustdocBrowser = {
        let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/testcrate"));

        let status = std::process::Command::new("cargo")
            .arg("clean")
            .current_dir(&path)
            .status()
            .expect("Failed to run 'cargo clean' on test crate");
        if !status.success() {
            panic!("Failed to run 'cargo clean' on test crate");
        }

        Analysis::generate(&path, Some("nightly")).expect("Failed to generate analysis data.");
        RustdocBrowser::new(Analysis::load(&path).expect("Failed to load analysis"))
    };
    static ref BROWSER: &'static RustdocBrowser = &BROWSER_STATIC;
}

fn iter_labels<T>(items: &[(String, T)]) -> impl Iterator<Item = &str> {
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
        &self
            .iter()
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
        Item::Item(a) => match b {
            Item::Item(b) => a.id == b.id,
            _ => false,
        },
    }
}

trait ItemExt {
    fn unwrap_item(&self) -> &rustdoc_types::Item;
}

impl<'a> ItemExt for Item<'a> {
    fn unwrap_item(&self) -> &rustdoc_types::Item {
        match self {
            Item::Item(item) => item,
            _ => panic!("not an Item::Item"),
        }
    }
}

#[test]
fn list_items() {
    let crates = BROWSER.list_crates();
    assert_eq!(crates.labels(), &["anyhow", "externcrate", "testcrate"]);

    let crate_id = crates.by_label("testcrate");

    // Pane 1

    let root_items = BROWSER.list_items(&crate_id);
    assert_eq!(
        root_items.labels(),
        &["mod x", "mod y", "mod z", "trait Trait",]
    );

    // Pane 2

    let mod_x = root_items.by_label("mod x");
    let mod_x_items = BROWSER.list_items(&mod_x.0);
    assert_eq!(mod_x_items.labels(), &["enum E", "struct S"]);

    let mod_y = root_items.by_label("mod y");
    let mod_y_items = BROWSER.list_items(&mod_y.0);
    assert_eq!(mod_y_items.labels(), &["struct S"]);

    let mod_z = root_items.by_label("mod z");
    let mod_z_items = BROWSER.list_items(&mod_z.0);
    assert_eq!(mod_z_items.labels(), &["struct S"]);

    // Assert that the three "struct S" defs are not the same.
    assert!(!items_eq(
        &mod_x_items.by_label("struct S").1,
        &mod_y_items.by_label("struct S").1
    ));
    assert!(!items_eq(
        &mod_y_items.by_label("struct S").1,
        &mod_z_items.by_label("struct S").1
    ));

    let trait_trait = root_items.by_label("trait Trait");
    let trait_items = BROWSER.list_items(&trait_trait.0);
    assert_eq!(trait_items.labels(), &["fn method"]);

    // Pane 3

    let x_e = mod_x_items.by_label("enum E");
    let x_e_items = BROWSER.list_items(&x_e.0);
    assert_eq!(
        x_e_items.labels(),
        &[
            "variant StructVariant",
            "variant TupleVariant(S)",
            "variant UnitVariant",
        ]
    );

    let x_s = mod_x_items.by_label("struct S");
    let x_s_items = BROWSER.list_items(&x_s.0);
    assert_eq!(
        x_s_items.labels(),
        &[
            "fn_field: Box<dyn Fn(usize, String) -> Option<i32>>",
            "int_field: i32",
            "opt_field: Option<Result<i32, std::io::Error>>",
            "string_field: String",
            "impl Self",
            "impl core::fmt::Display",
            "impl externcrate::ExternTrait",
        ]
    );

    let y_s = mod_y_items.by_label("struct S");
    let y_s_items = BROWSER.list_items(&y_s.0);
    assert_eq!(y_s_items.labels(), &["impl Self", "impl Trait<u64>",]);

    let z_s = mod_z_items.by_label("struct S");
    let z_s_items = BROWSER.list_items(&z_s.0);
    assert_eq!(z_s_items.labels(), &["impl Trait<String>"]);

    // Pane 4

    let x_e_unit = x_e_items.by_label("variant UnitVariant");
    let x_e_unit_items = BROWSER.list_items(&x_e_unit.0);
    assert_eq!(x_e_unit_items.labels(), &[] as &[&str]);

    let x_e_tuple = x_e_items.by_label("variant TupleVariant(S)");
    let x_e_tuple_items = BROWSER.list_items(&x_e_tuple.0);
    // Skip the struct field and the struct, go straight to its items:
    assert_eq!(x_e_tuple_items.labels(), x_s_items.labels());

    let x_e_struct = x_e_items.by_label("variant StructVariant");
    let x_e_struct_items = BROWSER.list_items(&x_e_struct.0);
    assert_eq!(x_e_struct_items.labels(), &["a: S"]);

    let x_s_self = x_s_items.by_label("impl Self");
    let x_s_self_items = BROWSER.list_items(&x_s_self.0);
    assert_eq!(x_s_self_items.labels(), &["fn f"]);

    let x_s_extern = x_s_items.by_label("impl externcrate::ExternTrait");
    let x_s_extern_items = BROWSER.list_items(&x_s_extern.0);
    assert_eq!(
        x_s_extern_items.labels(),
        &["fn required_method", "trait ExternTrait"]
    );

    let y_s_self = y_s_items.by_label("impl Self");
    let y_s_self_items = BROWSER.list_items(&y_s_self.0);
    assert_eq!(y_s_self_items.labels(), &["fn spoopadoop"]);

    let y_s_trait = y_s_items.by_label("impl Trait<u64>");
    let y_s_trait_items = BROWSER.list_items(&y_s_trait.0);
    // includes "fn method" because it overrides the default in the trait:
    assert_eq!(y_s_trait_items.labels(), &["fn method", "trait Trait"]);

    let z_s_trait = z_s_items.by_label("impl Trait<String>");
    let z_s_trait_items = BROWSER.list_items(&z_s_trait.0);
    // doesn't include "fn method" because it didn't override it:
    assert_eq!(z_s_trait_items.labels(), &["trait Trait"]);

    // Pane 5
    let x_s_self_f = x_s_self_items.by_label("fn f");
    let x_s_self_f_items = BROWSER.list_items(&x_s_self_f.0);
    assert_eq!(
        x_s_self_f_items.labels(),
        &["self: &Self", "e_arg: E", "-> S",]
    );
}
