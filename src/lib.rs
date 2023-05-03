pub mod analysis_rls;
pub mod analysis_scip;
pub mod browser_rls;
pub mod browser_scip;
pub mod browser_trait;
pub mod scroll_pad;
pub mod ui;

pub(crate) fn cmp_labels<T>(a: &(String, T), b: &(String, T)) -> std::cmp::Ordering {
    a.0.cmp(&b.0)
}

pub(crate) fn sort_by_label<T>(vec: &mut Vec<(String, T)>) {
    vec.sort_unstable_by(cmp_labels);
}
