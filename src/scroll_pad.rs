use cursive::view::{View, ViewWrapper};
use cursive::Vec2;

/// Adds one unit of padding to the right side of any view that doesn't require scrolling.
/// This is used to prevent views from having text immediately adjacent to each other (which is
/// hard to read) in the absence of scrollbars separating them.
/// If the view needs a scrollbar, this padding is omitted, because the scrollbar will provide the
/// needed separation.
pub struct ScrollPad<V> {
    inner: V,
}

impl<V: View> ScrollPad<V> {
    pub fn new(inner: V) -> Self {
        Self { inner }
    }
}

impl<V: View> ViewWrapper for ScrollPad<V> {
    cursive::wrap_impl!(self.inner: V);

    fn wrap_required_size(&mut self, constraint: Vec2) -> Vec2 {
        let mut calc = self.inner.required_size(constraint);
        if calc.y <= constraint.y {
            // View fits, no scrollbar will be used, so add one unit of padding for separation.
            calc.x += 1;
        }
        calc
    }
}
