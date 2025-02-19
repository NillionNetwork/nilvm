use super::OperationId;
use crate::{NadaFunction, Operation, ProgramMIR};
use std::collections::HashSet;

/// MIRProgram is malformed
#[derive(Debug, thiserror::Error)]
pub enum MIRProgramMalformed {
    /// Function recursion has been detected
    #[error("program malformed: recursion is not allowed")]
    FunctionRecursion(String),

    /// Called function is not found
    #[error("program malformed: missing called function")]
    MissingFunction,
}

impl ProgramMIR {
    /// Check if the program contains function recursion
    pub fn check_function_recursion(&self) -> Result<(), MIRProgramMalformed> {
        let mut checked_functions = HashSet::new();
        for function in self.functions.iter() {
            let mut visited_functions = HashSet::new();
            function.detect_recursion(self, &mut visited_functions, &mut checked_functions)?;
        }
        Ok(())
    }
}

impl NadaFunction {
    fn detect_recursion(
        &self,
        program: &ProgramMIR,
        visited: &mut HashSet<OperationId>,
        checked: &mut HashSet<OperationId>,
    ) -> Result<(), MIRProgramMalformed> {
        if !checked.contains(&self.id) {
            visited.insert(self.id);
            for operation in self.operations.values() {
                let function_id = match operation {
                    Operation::NadaFunctionCall(o) => Some(o.function_id),
                    Operation::Map(o) => Some(o.function_id),
                    Operation::Reduce(o) => Some(o.function_id),
                    _ => None,
                };
                if let Some(function_id) = function_id {
                    let function = program.function(function_id).ok_or(MIRProgramMalformed::MissingFunction)?;
                    if visited.contains(&function.id) {
                        return Err(MIRProgramMalformed::FunctionRecursion(function.name.clone()));
                    }
                    function.detect_recursion(program, visited, checked)?
                }
            }
            checked.insert(self.id);
            visited.remove(&self.id);
        }
        Ok(())
    }
}

/// Build a unary operation
#[macro_export]
macro_rules! unary_operation {
    ($name:ident, $name_str:literal) => {
        #[doc = concat!("MIR ", $name_str, " operation")]
        #[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
        #[cfg_attr(any(test, feature = "builder"), derive(Hash))]
        pub struct $name {
            /// Operation identifier is generated when the model is loaded.
            pub id: OperationId,
            /// The operand of the operation
            pub this: OperationId,
            /// Operation type
            #[serde(rename = "type")]
            pub ty: NadaType,
            /// Source file info related with this operation.
            pub source_ref_index: SourceRefIndex,
        }

        identifiable_element!($name, OperationId);
        named_element!($name, $name_str);
        source_info!($name);
        typed_element!($name);

        impl UnaryOperation for $name {
            fn operand(&self) -> OperationId {
                self.this
            }
        }

        impl HasOperands for $name {
            fn operands(&self) -> Vec<OperationId> {
                vec![self.this]
            }
        }

        impl $name {
            fn text_repr(&self) -> String {
                format!("oid({}) rty({}) = {} oid({})", self.id.0, self.ty, stringify!($name), self.this.0)
            }
        }
    };
}

/// Build a binary operation
#[macro_export]
macro_rules! binary_operation {
    ($name:ident, $name_str:literal, $public_output_only:literal) => {
        #[doc = concat!("MIR ", $name_str, " operation")]
        #[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
        #[cfg_attr(any(test, feature = "builder"), derive(Hash))]
        pub struct $name {
            /// Operation identifier is generated when the model is loaded.
            pub id: OperationId,
            /// Left operand of the operation
            pub left: OperationId,
            /// Right operand of the operation
            pub right: OperationId,
            /// Operation type
            #[serde(rename = "type")]
            pub ty: NadaType,
            /// Source file info related with this operation.
            pub source_ref_index: SourceRefIndex,
        }

        identifiable_element!($name, OperationId);
        named_element!($name, $name_str);
        source_info!($name);
        typed_element!($name);

        impl BinaryOperation for $name {
            fn left(&self) -> OperationId {
                self.left
            }

            fn right(&self) -> OperationId {
                self.right
            }

            fn public_output_only(&self) -> bool {
                $public_output_only
            }
        }

        impl HasOperands for $name {
            fn operands(&self) -> Vec<OperationId> {
                vec![self.left, self.right]
            }
        }

        impl $name {
            fn text_repr(&self) -> String {
                format!(
                    "oid({}) rty({}) = {} oid({}) oid({})",
                    self.id.0,
                    self.ty,
                    stringify!($name),
                    self.left.0,
                    self.right.0
                )
            }
        }
    };
}

/// Build a ternary operation
#[macro_export]
macro_rules! ternary_operation {
    ($name:ident, $name_str:literal) => {
        #[doc = concat!("MIR ", $name_str, " operation")]
        #[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
        #[cfg_attr(any(test, feature = "builder"), derive(Hash))]
        pub struct $name {
            /// Operation identifier is generated when the model is loaded.
            pub id: OperationId,
            /// The first operand of the operation
            pub this: OperationId,
            /// The second operand of the operation
            pub arg_0: OperationId,
            /// The third operand of the operation
            pub arg_1: OperationId,
            /// Operation type
            #[serde(rename = "type")]
            pub ty: NadaType,
            /// Source file info related with this operation.
            pub source_ref_index: SourceRefIndex,
        }

        identifiable_element!($name, OperationId);
        named_element!($name, $name_str);
        source_info!($name);
        typed_element!($name);

        impl HasOperands for $name {
            fn operands(&self) -> Vec<OperationId> {
                vec![self.this, self.arg_0, self.arg_1]
            }
        }

        impl $name {
            fn text_repr(&self) -> String {
                format!(
                    "oid({}) rty({}) = {} oid({}) oid({}) oid({})",
                    self.id.0,
                    self.ty,
                    stringify!($name),
                    self.this,
                    self.arg_0.0,
                    self.arg_1.0
                )
            }
        }
    };
}

/// Macro to simplify implementations of traits for [`Operation`]
#[macro_export]
macro_rules! delegate_to_inner {
    ($on:ident, $method:tt $(, $opt:expr)*) => {
        match $on {
            Operation::Reduce(o) => o.$method($($opt),*),
            Operation::Map(o) => o.$method($($opt),*),
            Operation::Unzip(o) => o.$method($($opt),*),
            Operation::Zip(o) => o.$method($($opt),*),
            Operation::Addition(o) => o.$method($($opt),*),
            Operation::Subtraction(o) => o.$method($($opt),*),
            Operation::Multiplication(o) => o.$method($($opt),*),
            Operation::LessThan(o) => o.$method($($opt),*),
            Operation::LessOrEqualThan(o) => o.$method($($opt),*),
            Operation::GreaterThan(o) => o.$method($($opt),*),
            Operation::GreaterOrEqualThan(o) => o.$method($($opt),*),
            Operation::PublicOutputEquality(o) => o.$method($($opt),*),
            Operation::Equals(o) => o.$method($($opt),*),
            Operation::Cast(o) => o.$method($($opt),*),
            Operation::InputReference(o) => o.$method($($opt),*),
            Operation::LiteralReference(o) => o.$method($($opt),*),
            Operation::NadaFunctionArgRef(o) => o.$method($($opt),*),
            Operation::Modulo(o) => o.$method($($opt),*),
            Operation::Power(o) => o.$method($($opt),*),
            Operation::Division(o) => o.$method($($opt),*),
            Operation::NadaFunctionCall(o) => o.$method($($opt),*),
            Operation::ArrayAccessor(o) => o.$method($($opt),*),
            Operation::TupleAccessor(o) => o.$method($($opt),*),
            Operation::New(o) => o.$method($($opt),*),
            Operation::Random(o) => o.$method($($opt),*),
            Operation::IfElse(o) => o.$method($($opt),*),
            Operation::Reveal(o) => o.$method($($opt),*),
            Operation::PublicKeyDerive(o) => o.$method($($opt),*),
            Operation::Not(o) => o.$method($($opt),*),
            Operation::LeftShift(o) => o.$method($($opt),*),
            Operation::RightShift(o) => o.$method($($opt),*),
            Operation::TruncPr(o) => o.$method($($opt),*),
            Operation::InnerProduct(o) => o.$method($($opt),*),
            Operation::NotEquals(o) => o.$method($($opt),*),
            Operation::BooleanAnd(o) => o.$method($($opt),*),
            Operation::BooleanOr(o) => o.$method($($opt),*),
            Operation::BooleanXor(o) => o.$method($($opt),*),
            Operation::EcdsaSign(o) => o.$method($($opt),*),
            Operation::EddsaSign(o) => o.$method($($opt),*),
        }
    };
}
