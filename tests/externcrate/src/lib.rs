pub trait ExternTrait {
    /// Implementations must implement this.
    fn required_method(&self) -> &'static str;

    /// Implementations may override this, but don't have to.
    fn default_method(&self) -> &'static str {
        "this is the default impl"
    }
}

fn free_func(s: &str) -> usize {
    s.len()
}

mod a {
    mod b {
        mod c {
            mod d {
                struct abcd {
                    abcd_f1: String,
                    abcd_f2: i32,
                }
            }
        }
    }
}