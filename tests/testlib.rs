#[macro_use] extern crate lazy_static;

use rsbrowse::analysis::Analysis;
use rsbrowse::browser::Browser;
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

trait VecExt {
    fn contains_label(&self, s: &str) -> bool;
}

impl<T> VecExt for Vec<(String, T)> {
    fn contains_label(&self, s: &str) -> bool {
        iter_labels(self).find(|label| *label == s).is_some()
    }
}

#[test]
fn it_loads() {
    let _foo = &BROWSER;
}

#[test]
fn list_crates() {
    let crates = BROWSER.list_crates();
    assert!(crates.contains_label("testcrate"));
    assert!(crates.contains_label("log"));
}
