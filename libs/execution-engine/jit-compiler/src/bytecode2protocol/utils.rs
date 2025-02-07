/// Builds a function to convert a bytecode binary Operation into a protocol that consumes public
/// variables and returns a public variable. The operands and return types must be the same.
#[macro_export]
macro_rules! public_binary_protocol {
    ($operation:ident) => {
        pub(crate) fn public_protocol<
            P: $crate::models::protocols::Protocol + From<Self>,
            F: $crate::bytecode2protocol::ProtocolFactory<P>,
        >(
            context: &mut $crate::bytecode2protocol::Bytecode2ProtocolContext<P, F>,
            operation: &$operation,
        ) -> Result<P, $crate::bytecode2protocol::errors::Bytecode2ProtocolError> {
            let expected_type = operation.ty.as_public()?;
            let left = $crate::bytecode2protocol::Bytecode2Protocol::adapted_protocol(
                context,
                operation.left,
                &expected_type,
            )?;
            let right = $crate::bytecode2protocol::Bytecode2Protocol::adapted_protocol(
                context,
                operation.right,
                &expected_type,
            )?;
            let protocol = Self {
                address: $crate::models::protocols::memory::ProtocolAddress::default(),
                left,
                right,
                ty: expected_type,
                source_ref_index: operation.source_ref_index,
            };
            Ok(protocol.into())
        }
    };
}

/// Builds a function to convert a bytecode binary Operation into a protocol that consumes shares
/// and returns a share. The operands and return types must be the same.
#[macro_export]
macro_rules! share_binary_protocol {
    ($operation:ident) => {
        pub(crate) fn share_protocol<
            P: $crate::models::protocols::Protocol + From<Self>,
            F: $crate::bytecode2protocol::ProtocolFactory<P>,
        >(
            context: &mut $crate::bytecode2protocol::Bytecode2ProtocolContext<P, F>,
            operation: &$operation,
        ) -> Result<P, $crate::bytecode2protocol::errors::Bytecode2ProtocolError> {
            //
            let expected_type = operation.ty.as_shamir_share()?;
            let left = $crate::bytecode2protocol::Bytecode2Protocol::adapted_protocol(
                context,
                operation.left,
                &expected_type,
            )?;
            let right = $crate::bytecode2protocol::Bytecode2Protocol::adapted_protocol(
                context,
                operation.right,
                &expected_type,
            )?;
            let protocol = Self {
                address: $crate::models::protocols::memory::ProtocolAddress::default(),
                left,
                right,
                ty: expected_type,
                source_ref_index: operation.source_ref_index,
            };
            Ok(protocol.into())
        }
    };
}

/// Builds a function to convert a bytecode relational Operation into a protocol that consumes public
/// variables and returns a boolean. The operands and return types must be the same.
#[macro_export]
macro_rules! public_relational_protocol {
    ($operation:ident) => {
        pub(crate) fn public_protocol<
            P: $crate::models::protocols::Protocol + From<Self>,
            F: $crate::bytecode2protocol::ProtocolFactory<P>,
        >(
            context: &mut $crate::bytecode2protocol::Bytecode2ProtocolContext<P, F>,
            operation: &$operation,
            operand_type: &nada_value::NadaType,
        ) -> Result<P, $crate::bytecode2protocol::errors::Bytecode2ProtocolError> {
            let operand_type = operand_type.as_public()?;
            let left =
                $crate::bytecode2protocol::Bytecode2Protocol::adapted_protocol(context, operation.left, &operand_type)?;
            let right = $crate::bytecode2protocol::Bytecode2Protocol::adapted_protocol(
                context,
                operation.right,
                &operand_type,
            )?;
            let protocol = Self {
                address: $crate::models::protocols::memory::ProtocolAddress::default(),
                left,
                right,
                ty: nada_value::NadaType::Boolean,
                source_ref_index: operation.source_ref_index,
            };
            Ok(protocol.into())
        }
    };
}

/// Builds a function to convert a bytecode relational Operation into a protocol that consumes public
/// variables and returns a boolean. The operands and return types must be the same.
#[macro_export]
macro_rules! share_relational_protocol {
    ($operation:ident) => {
        pub(crate) fn share_protocol<
            P: $crate::models::protocols::Protocol + From<Self>,
            F: $crate::bytecode2protocol::ProtocolFactory<P>,
        >(
            context: &mut $crate::bytecode2protocol::Bytecode2ProtocolContext<P, F>,
            operation: &$operation,
            operand_type: &nada_value::NadaType,
        ) -> Result<P, $crate::bytecode2protocol::errors::Bytecode2ProtocolError> {
            let operand_type = operand_type.as_shamir_share()?;
            let left =
                $crate::bytecode2protocol::Bytecode2Protocol::adapted_protocol(context, operation.left, &operand_type)?;
            let right = $crate::bytecode2protocol::Bytecode2Protocol::adapted_protocol(
                context,
                operation.right,
                &operand_type,
            )?;
            let protocol = Self {
                address: $crate::models::protocols::memory::ProtocolAddress::default(),
                left,
                right,
                ty: nada_value::NadaType::ShamirShareBoolean,
                source_ref_index: operation.source_ref_index,
            };
            Ok(protocol.into())
        }
    };
}

/// Builds a function to convert a bytecode relational Operation into a protocol that consumes public
/// variables and returns a boolean. The operands and return types must be the same.
#[macro_export]
macro_rules! public_shift_protocol {
    ($operation:ident) => {
        pub(crate) fn public_protocol<
            P: $crate::models::protocols::Protocol + From<Self>,
            F: $crate::bytecode2protocol::ProtocolFactory<P>,
        >(
            context: &mut $crate::bytecode2protocol::Bytecode2ProtocolContext<P, F>,
            operation: &$operation,
        ) -> Result<P, $crate::bytecode2protocol::errors::Bytecode2ProtocolError> {
            let expected_type = operation.ty.as_public()?;
            let left = $crate::bytecode2protocol::Bytecode2Protocol::adapted_protocol(
                context,
                operation.left,
                &expected_type,
            )?;
            let right = $crate::bytecode2protocol::Bytecode2Protocol::adapted_protocol(
                context,
                operation.right,
                &nada_value::NadaType::UnsignedInteger,
            )?;
            let protocol = Self {
                address: $crate::models::protocols::memory::ProtocolAddress::default(),
                left,
                right,
                ty: expected_type,
                source_ref_index: operation.source_ref_index,
            };
            Ok(protocol.into())
        }
    };
}

/// Builds a function to convert a bytecode relational Operation into a protocol that consumes public
/// variables and returns a boolean. The operands and return types must be the same.
#[macro_export]
macro_rules! share_shift_protocol {
    ($operation:ident) => {
        pub(crate) fn share_protocol<
            P: $crate::models::protocols::Protocol + From<Self>,
            F: $crate::bytecode2protocol::ProtocolFactory<P>,
        >(
            context: &mut $crate::bytecode2protocol::Bytecode2ProtocolContext<P, F>,
            operation: &$operation,
        ) -> Result<P, $crate::bytecode2protocol::errors::Bytecode2ProtocolError> {
            let expected_type = operation.ty.as_shamir_share()?;
            let left = $crate::bytecode2protocol::Bytecode2Protocol::adapted_protocol(
                context,
                operation.left,
                &expected_type,
            )?;
            let right = $crate::bytecode2protocol::Bytecode2Protocol::adapted_protocol(
                context,
                operation.right,
                &nada_value::NadaType::UnsignedInteger,
            )?;
            let protocol = Self {
                address: $crate::models::protocols::memory::ProtocolAddress::default(),
                left,
                right,
                ty: expected_type,
                source_ref_index: operation.source_ref_index,
            };
            Ok(protocol.into())
        }
    };
}

/// Builds a function to convert a bytecode relational Operation into a protocol that consumes public
/// variables and returns a boolean. The operands and return types must be the same.
#[macro_export]
macro_rules! particle_shift_protocol {
    ($operation:ident) => {
        pub(crate) fn particle_protocol<
            P: $crate::models::protocols::Protocol + From<Self>,
            F: $crate::bytecode2protocol::ProtocolFactory<P>,
        >(
            context: &mut $crate::bytecode2protocol::Bytecode2ProtocolContext<P, F>,
            operation: &$operation,
        ) -> Result<P, $crate::bytecode2protocol::errors::Bytecode2ProtocolError> {
            let expected_type = operation.ty.as_shamir_particle()?;
            let left = $crate::bytecode2protocol::Bytecode2Protocol::adapted_protocol(
                context,
                operation.left,
                &expected_type,
            )?;
            let right = $crate::bytecode2protocol::Bytecode2Protocol::adapted_protocol(
                context,
                operation.right,
                &nada_value::NadaType::UnsignedInteger,
            )?;
            let protocol = Self {
                address: $crate::models::protocols::memory::ProtocolAddress::default(),
                left,
                right,
                ty: expected_type,
                source_ref_index: operation.source_ref_index,
            };
            Ok(protocol.into())
        }
    };
}
