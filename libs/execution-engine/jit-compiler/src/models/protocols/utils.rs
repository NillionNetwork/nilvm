#[macro_export]
/// Builds an unary protocol
macro_rules! unary_protocol {
    ($ty:ident, $op:literal, $execution_line:expr, $requirement_type:ty, $requirements:expr) => {
        #[doc = concat!("A protocol that performs ", stringify!($ty))]
        #[doc = ""]
        #[doc = concat!("This protocol will return the output of `", stringify!($op), " operand`")]
        #[derive(Debug, Clone)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        pub struct $ty {
            /// Address of the protocol
            pub address: $crate::models::protocols::memory::ProtocolAddress,
            /// The operand protocol address.
            pub operand: $crate::models::protocols::memory::ProtocolAddress,
            /// The protocol output type
            pub ty: nada_value::NadaType,
            /// Source code info about this element.
            pub source_ref_index: $crate::models::SourceRefIndex,
        }

        impl $crate::models::protocols::ProtocolDependencies for $ty {
            fn dependencies(&self) -> Vec<$crate::models::protocols::memory::ProtocolAddress> {
                vec![self.operand]
            }
        }

        $crate::protocol!($ty, $requirement_type, $requirements, $execution_line);

        impl std::fmt::Display for $ty {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{} - rty({}) = {} {}", self.address, self.ty, stringify!($op), self.operand)
            }
        }
    };
    ($ty:ident, $op:literal, $execution_line:expr, $requirement_type:ty) => {
        unary_protocol!($ty, $op, $execution_line, $requirement_type, &[]);
    };
}

#[macro_export]
/// Builds a binary protocol
macro_rules! binary_protocol {
    ($ty:ident, $op:literal, $execution_line:expr, $requirement_type:ty, $requirements:expr) => {
        #[doc = concat!("A protocol that performs ", stringify!($ty))]
        #[doc = ""]
        #[doc = concat!("This protocol will return the output of `", stringify!($op), " left right`")]
        #[derive(Debug, Clone)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        pub struct $ty {
            /// Address of the protocol
            pub address: $crate::models::protocols::memory::ProtocolAddress,
            /// The left protocol address.
            pub left: $crate::models::protocols::memory::ProtocolAddress,
            /// The right protocol address.
            pub right: $crate::models::protocols::memory::ProtocolAddress,
            /// The protocol output type
            pub ty: nada_value::NadaType,
            /// Source code info about this element.
            pub source_ref_index: $crate::models::SourceRefIndex,
        }

        impl $crate::models::protocols::ProtocolDependencies for $ty {
            fn dependencies(&self) -> Vec<$crate::models::protocols::memory::ProtocolAddress> {
                vec![self.left, self.right]
            }
        }

        $crate::protocol!($ty, $requirement_type, $requirements, $execution_line);

        impl std::fmt::Display for $ty {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{} - rty({}) = {} {} {}", self.address, self.ty, stringify!($op), self.left, self.right)
            }
        }
    };
    ($ty:ident, $op:literal, $execution_line:expr, $requirement_type:ty) => {
        binary_protocol!($ty, $op, $execution_line, $requirement_type, &[]);
    };
}

#[macro_export]
/// Builds an if_else protocol
macro_rules! if_else {
    ($ty:ident, $op:literal, $execution_line:expr, $requirement_type:ty, $requirements:expr) => {
        #[doc = concat!("A protocol that performs ", stringify!($ty))]
        #[doc = ""]
        #[doc = concat!("This protocol will return the output of `", stringify!($op), "cond then left else right`")]
        #[derive(Debug, Clone)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        pub struct $ty {
            /// Address of the protocol
            pub address: $crate::models::protocols::memory::ProtocolAddress,
            /// The conditions protocol address.
            pub cond: $crate::models::protocols::memory::ProtocolAddress,
            /// The left protocol address.
            pub left: $crate::models::protocols::memory::ProtocolAddress,
            /// The right protocol address.
            pub right: $crate::models::protocols::memory::ProtocolAddress,
            /// The protocol output type
            pub ty: nada_value::NadaType,
            /// Source code info about this element.
            pub source_ref_index: $crate::models::SourceRefIndex,
        }

        impl $crate::models::protocols::ProtocolDependencies for $ty {
            fn dependencies(&self) -> Vec<$crate::models::protocols::memory::ProtocolAddress> {
                vec![self.cond, self.left, self.right]
            }
        }

        $crate::protocol!($ty, $requirement_type, $requirements, $execution_line);

        impl std::fmt::Display for $ty {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(
                    f,
                    "{} - rty({}) = {} {} {} {}",
                    self.address,
                    self.ty,
                    stringify!($op),
                    self.cond,
                    self.left,
                    self.right
                )
            }
        }
    };
    ($ty:ident, $op:literal, $execution_line:expr, $requirement_type:ty) => {
        if_else!($ty, $op, $execution_line, $requirement_type, &[]);
    };
}
