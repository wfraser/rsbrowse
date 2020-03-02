pub mod x {
    pub struct S;

    impl S {
        pub fn f(&self) {}
    }
}

pub mod y {
    pub struct S;

    impl S {
        pub fn g(&self) {}
    }

    impl crate::Trait<u64> for S {
        fn method(&self) -> u64 {
            42
        }
    }
}

pub mod z {
    pub struct S;

    impl crate::Trait<String> for S {
        // inherit default implementation for method
    }
}

pub trait Trait<T: Default> {
    fn method(&self) -> T {
        Default::default()
    }
}
