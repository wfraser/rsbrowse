#![feature(rustc_private)]


#[cfg(feature = "rustc-wrapper")]
extern crate rustc_errors;

#[cfg(feature = "rustc-wrapper")]
extern crate rustc_error_codes;

#[cfg(feature = "rustc-wrapper")]
extern crate rustc_driver;

#[cfg(feature = "rustc-wrapper")]
extern crate rustc_hir;

#[cfg(feature = "rustc-wrapper")]
extern crate rustc_interface;

#[cfg(feature = "rustc-wrapper")]
extern crate rustc_middle;

#[cfg(feature = "rustc-wrapper")]
extern crate rustc_session;

#[cfg(feature = "rustc-wrapper")]
extern crate rustc_span;

pub mod analysis_driver;
pub mod analysis_rls;
pub mod analysis_scip;
pub mod browser_driver;
pub mod browser_rls;
pub mod browser_scip;
pub mod browser_trait;
pub mod scroll_pad;
pub mod ui;

#[cfg(feature = "rustc-wrapper")]
pub mod rustc_wrapper;

pub(crate) fn cmp_labels<T>(a: &(String, T), b: &(String, T)) -> std::cmp::Ordering {
    a.0.cmp(&b.0)
}

pub(crate) fn sort_by_label<T>(vec: &mut Vec<(String, T)>) {
    vec.sort_unstable_by(cmp_labels);
}
