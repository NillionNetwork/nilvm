//! Bytecode utilities implementation

macro_rules! unary_operation_bytecode {
    ($ty:ident, $name:literal) => {
        #[doc = concat!("Bytecode ", stringify!($name), " operation")]
        #[derive(Clone, Debug)]
        #[cfg_attr(any(test, feature = "serde"), derive(Serialize, Deserialize))]
        #[cfg_attr(test, derive(PartialEq))]
        pub struct $ty {
            /// Address of the operand in the program's operations vector.
            pub operand: BytecodeAddress,
            /// Address of this operation in the program's operations vector.
            pub address: BytecodeAddress,
            /// Output type
            #[cfg_attr(any(test, feature = "serde"), serde(rename = "type"))]
            pub ty: NadaType,
            /// Source code info about this element.
            pub source_ref_index: SourceRefIndex,
        }

        impl std::fmt::Display for $ty {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{} rty({}) = {} {}", self.address, self.ty, stringify!($ty), self.operand)
            }
        }

        crate::source_info!($ty);
        typed_element!($ty);
        addressed_operation!($ty);
    };
}

macro_rules! binary_operation_bytecode {
    ($ty:ident, $name:literal) => {
        #[doc = concat!("Bytecode ", stringify!($name), " operation")]
        #[derive(Clone, Debug)]
        #[cfg_attr(any(test, feature = "serde"), derive(Serialize, Deserialize))]
        #[cfg_attr(test, derive(PartialEq))]
        pub struct $ty {
            /// Address of the left operand in the program's operations vector.
            pub left: BytecodeAddress,
            /// Address of the right operand in the program's operations vector.
            pub right: BytecodeAddress,
            /// Address of this operation in the program's operations vector.
            pub address: BytecodeAddress,
            /// Output type
            #[cfg_attr(any(test, feature = "serde"), serde(rename = "type"))]
            pub ty: NadaType,
            /// Source code info about this element.
            pub source_ref_index: SourceRefIndex,
        }

        impl std::fmt::Display for $ty {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{} rty({}) = {} {} {}", self.address, self.ty, stringify!($ty), self.left, self.right)
            }
        }

        crate::source_info!($ty);
        typed_element!($ty);
        addressed_operation!($ty);
    };
}

macro_rules! ternary_operation_bytecode {
    ($ty:ident, $name:literal) => {
        #[doc = concat!("Bytecode ", stringify!($name), " operation")]
        #[derive(Clone, Debug)]
        #[cfg_attr(any(test, feature = "serde"), derive(Serialize, Deserialize))]
        #[cfg_attr(test, derive(PartialEq))]
        pub struct $ty {
            /// Address of the first operand in the program's operations vector.
            pub first: BytecodeAddress,
            /// Address of the second operand in the program's operations vector.
            pub second: BytecodeAddress,
            /// Address of the third operand in the program's operations vector.
            pub third: BytecodeAddress,
            /// Address of this operation in the program's operations vector.
            pub address: BytecodeAddress,
            /// Output type
            #[cfg_attr(any(test, feature = "serde"), serde(rename = "type"))]
            pub ty: NadaType,
            /// Source code info about this element.
            pub source_ref_index: SourceRefIndex,
        }

        impl std::fmt::Display for $ty {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(
                    f,
                    "{} rty({}) = {} {} {} {}",
                    self.address,
                    self.ty,
                    stringify!($ty),
                    self.first,
                    self.second,
                    self.third
                )
            }
        }

        crate::source_info!($ty);
        typed_element!($ty);
        addressed_operation!($ty);
    };
}

pub(crate) use binary_operation_bytecode;
pub(crate) use ternary_operation_bytecode;
pub(crate) use unary_operation_bytecode;
