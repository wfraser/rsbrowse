#![feature(rustc_private)]

extern crate rustc_errors;
extern crate rustc_error_codes;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

pub mod analysis_driver;
pub mod analysis_rls;
pub mod analysis_scip;
pub mod browser_rls;
pub mod browser_scip;
pub mod browser_trait;
pub mod rustc_wrapper;
pub mod scroll_pad;
pub mod ui;

pub(crate) fn cmp_labels<T>(a: &(String, T), b: &(String, T)) -> std::cmp::Ordering {
    a.0.cmp(&b.0)
}

pub(crate) fn sort_by_label<T>(vec: &mut Vec<(String, T)>) {
    vec.sort_unstable_by(cmp_labels);
}
