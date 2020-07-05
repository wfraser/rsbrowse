pub trait ExternTrait {
    /// Implementations must implement this.
    fn required_method(&self) -> &'static str;

    /// Implementations may override this, but don't have to.
    fn default_method(&self) -> &'static str {
        "this is the default impl"
    }
}
