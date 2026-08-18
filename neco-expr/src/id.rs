macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(u32);

        impl $name {
            pub const fn new(value: u32) -> Self {
                Self(value)
            }

            pub const fn get(self) -> u32 {
                self.0
            }
        }
    };
}

id_type!(ExprId);
id_type!(AtomId);
id_type!(ConsumerId);
id_type!(AbsoluteBits);
