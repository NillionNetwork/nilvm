use super::{OperationId, OperationIdGenerator, SourceRefIndex};
use crate::{
    binary_operation, delegate_to_inner, identifiable_element, named_element, output_element, source_info,
    ternary_operation, typed_element, unary_operation, BinaryOperation, HasOperands, IdentifiableElement, Input,
    Literal, NamedElement, OutputElement, Party, SourceFiles, SourceInfo, SourceRef, TypedElement, UnaryOperation,
};
use anyhow::{anyhow, Result};
use nada_type::NadaType;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::{
    collections::{BTreeMap, HashMap},
    fmt::Display,
};

/// Type alias for the Operations map.
pub type OperationMap = BTreeMap<OperationId, Operation>;

/// A program's Medium Internal Representation (MIR).
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct ProgramMIR {
    /// List of the functions are used by the program
    pub functions: Vec<NadaFunction>,
    /// Program parties
    pub parties: Vec<Party>,
    /// Program inputs
    pub inputs: Vec<Input>,
    /// Program literals
    pub literals: Vec<Literal>,
    /// Program output
    pub outputs: Vec<Output>,
    /// Table of operations
    pub operations: OperationMap,
    /// Source file info related with the program.
    pub source_files: SourceFiles,
    /// Array of source references
    pub source_refs: Vec<SourceRef>,
}

impl ProgramMIR {
    /// Get the NadaFunction from its id
    pub fn function(&self, function_id: OperationId) -> Option<&NadaFunction> {
        self.functions.iter().find(|f| f.id == function_id)
    }

    /// Returns an operation ID generator that can be used to generate new IDs that will be consistent with the existing
    /// IDs within this program MIR.
    /// Note that this wraps around in case the maximum value of 9_223_372_036_854_775_807 is reached.
    pub fn operation_id_generator(&self) -> OperationIdGenerator {
        let next = if let Some(max) = self.operations.keys().max() {
            OperationId(max.0.wrapping_add(1))
        } else {
            OperationId::FIRST
        };
        OperationIdGenerator::with_next(next)
    }

    /// Returns a reference to an operation from its ID.
    pub fn operation(&self, id: OperationId) -> Result<&Operation> {
        self.operations.get(&id).ok_or(anyhow!("operation {id} not found in program MIR"))
    }

    /// Returns a source ref from an index.
    pub fn source_ref(&self, index: SourceRefIndex) -> Result<&SourceRef> {
        self.source_refs.get(index.0 as usize).ok_or(anyhow!("source ref with index {} not found", index.0))
    }

    /// Count inputs readings from an iterator of operations
    fn count_readings<'a, I: IntoIterator<Item = &'a Operation>>(counters: &mut HashMap<String, usize>, operations: I) {
        for operation in operations {
            if let Operation::InputReference(input_ref) = operation {
                let count: &mut usize = counters.entry(input_ref.refers_to.to_string()).or_default();
                *count = count.wrapping_add(1usize);
            }
        }
    }

    /// Count all inputs readings form the program
    pub fn count_inputs_readings(&self) -> HashMap<String, usize> {
        let mut counters = HashMap::default();
        Self::count_readings(&mut counters, self.operations.values());
        for function in self.functions.iter() {
            Self::count_readings(&mut counters, function.operations.values());
        }
        counters
    }
    pub fn get_source_line(&self, source_ref_index: SourceRefIndex) -> String {
        let Ok(src_ref) = self.source_ref(source_ref_index) else {
            return "".to_string();
        };
        format!("{}:{}", src_ref.file, src_ref.lineno)
    }
    /// Text representation of MIR
    pub fn text_repr(&self) -> String {
        let mut text = String::new();

        text.push_str("Parties:\n");
        for party in &self.parties {
            text.push_str(&format!("  {}:   # {}\n", party.name, self.get_source_line(party.source_ref_index)));
        }

        text.push_str("Inputs:\n");
        for input in &self.inputs {
            text.push_str(&format!(
                "  {}: ty({}) party({}) doc({})   # {}\n",
                input.name,
                input.ty,
                input.party,
                input.doc,
                self.get_source_line(input.source_ref_index)
            ));
        }

        text.push_str("Literals:\n");
        for literal in &self.literals {
            text.push_str(&format!("  {}: ty({}) val({})\n", literal.name, literal.ty, literal.value));
        }

        text.push_str("Outputs:\n");
        for output in &self.outputs {
            text.push_str(&format!(
                "  {}: ty({}) oid({}) party({})   # {}\n",
                output.name,
                output.ty,
                output.operation_id.0,
                output.party,
                self.get_source_line(output.source_ref_index)
            ));
        }

        text.push_str("Functions:\n");
        for function in &self.functions {
            let args = function
                .args
                .iter()
                .map(|arg| format!("{}: ty({})", arg.name, arg.ty))
                .collect::<Vec<String>>()
                .join(",");
            text.push_str(&format!(
                "  {}: args({}) rty({}) roid({})    # {}",
                function.name,
                args,
                function.return_type,
                function.return_operation_id.0,
                self.get_source_line(function.source_ref_index)
            ));

            for operation in function.operations.values() {
                text.push_str(&format!("    {}", operation.text_repr()));
                text.push('\n');
            }
        }

        text.push_str("Operations:\n");
        for operation in self.operations.values() {
            text.push_str(&operation.text_repr());
            text.push('\n');
        }
        text
    }
}

/// MIR output
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Output {
    /// Output name
    pub name: String,
    /// Output inner operation
    pub operation_id: OperationId,
    /// Party contains this output
    pub party: String,
    /// Output type
    #[serde(rename = "type")]
    pub ty: NadaType,
    /// Source file info related with this output.
    pub source_ref_index: SourceRefIndex,
}
source_info!(Output);
named_element!(Output);
typed_element!(Output);
output_element!(Output);

/// MIR NADA function argument
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NadaFunctionArg {
    /// Argument name
    pub name: String,
    /// Argument type
    #[serde(rename = "type")]
    pub ty: NadaType,
    /// Source code info about this element.
    pub source_ref_index: SourceRefIndex,
}
source_info!(NadaFunctionArg);
typed_element!(NadaFunctionArg);

/// MIR NADA function argument
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[cfg_attr(any(test, feature = "builder"), derive(Hash))]
pub struct NadaFunctionArgRef {
    /// Operation identifier is generated when the model is loaded.
    pub id: OperationId,
    /// Function owner of this argument
    pub function_id: OperationId,
    /// Referenced function argument
    pub refers_to: String,
    /// Argument type
    #[serde(rename = "type")]
    pub ty: NadaType,
    /// Source code info about this element.
    pub source_ref_index: SourceRefIndex,
}
identifiable_element!(NadaFunctionArgRef, OperationId);
source_info!(NadaFunctionArgRef);
typed_element!(NadaFunctionArgRef);
named_element!(NadaFunctionArgRef, "nada function argument reference");
impl NadaFunctionArgRef {
    fn text_repr(&self) -> String {
        format!("oid({}) rty({}) = NadaFunctionArgRef to({})", self.id.0, self.ty, self.refers_to)
    }
}
/// MIR NADA function
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NadaFunction {
    /// Function identifier
    pub id: OperationId,
    /// Function arguments
    pub args: Vec<NadaFunctionArg>,
    /// Function name
    #[serde(rename = "function")]
    pub name: String,
    /// Table of operations for the function
    pub operations: OperationMap,
    /// Identifier of the operation (in the operations map) that represents the return of this function
    pub return_operation_id: OperationId,
    /// Function return type
    pub return_type: NadaType,
    /// NadaFunction source file information.
    pub source_ref_index: SourceRefIndex,
}
source_info!(NadaFunction);

impl TypedElement for NadaFunction {
    fn ty(&self) -> &NadaType {
        &self.return_type
    }
}

/// MIR NADA function call
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[cfg_attr(any(test, feature = "builder"), derive(Hash))]
pub struct NadaFunctionCall {
    /// Operation identifier is generated when the model is loaded.
    pub id: OperationId,
    /// Function owner of this call
    pub function_id: OperationId,
    /// Function return type
    pub return_type: NadaType,
    /// NadaFunction source file information.
    pub source_ref_index: SourceRefIndex,
    /// Arguments of the call
    pub args: Vec<OperationId>,
}
identifiable_element!(NadaFunctionCall, OperationId);
source_info!(NadaFunctionCall);
named_element!(NadaFunctionCall, "nada function call");

impl TypedElement for NadaFunctionCall {
    fn ty(&self) -> &NadaType {
        &self.return_type
    }
}

impl NadaFunctionCall {
    fn text_repr(&self) -> String {
        let args = self.args.iter().map(|arg| format!("oid({})", arg.0)).collect::<Vec<String>>().join(", ");
        format!(
            "oid({}) rty({}) = NadaFunctionCall fn({}) args ({})",
            self.id.0, self.return_type, self.function_id, args
        )
    }
}

/// MIR NADA Literal Reference
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
#[cfg_attr(any(test, feature = "builder"), derive(Hash))]
pub struct LiteralReference {
    /// Operation identifier is generated when the model is loaded.
    pub id: OperationId,
    /// Reference to literal
    pub refers_to: String,
    /// Literal type
    #[serde(rename = "type")]
    pub ty: NadaType,
    /// Source code info about this element.
    pub source_ref_index: SourceRefIndex,
}
identifiable_element!(LiteralReference, OperationId);
source_info!(LiteralReference);
typed_element!(LiteralReference);
named_element!(LiteralReference, "literal");
impl LiteralReference {
    fn text_repr(&self) -> String {
        format!("oid({}) rty({}) = LiteralReference to({})", self.id.0, self.ty, self.refers_to)
    }
}
unary_operation!(Not, "not");
unary_operation!(Reveal, "reveal");
unary_operation!(Unzip, "unzip");
binary_operation!(LessThan, "less-than", false);
binary_operation!(LessOrEqualThan, "less-or-equal-than", false);
binary_operation!(GreaterThan, "greater-than", false);
binary_operation!(GreaterOrEqualThan, "greater-or-equal-than", false);
binary_operation!(PublicOutputEquality, "public-output-equality", true);
binary_operation!(Equals, "equals", false);
binary_operation!(NotEquals, "not-equals", false);
binary_operation!(TruncPr, "trunc-pr", false);
binary_operation!(Addition, "Addition", false);
binary_operation!(Subtraction, "Subtraction", false);
binary_operation!(Multiplication, "Multiplication", false);
binary_operation!(Modulo, "Modulo", false);
binary_operation!(Power, "Power", false);
binary_operation!(LeftShift, "left-shift", false);
binary_operation!(RightShift, "right-shift", false);
binary_operation!(Division, "Division", false);
binary_operation!(Zip, "zip", false);
binary_operation!(InnerProduct, "inner-product", false);
binary_operation!(BooleanAnd, "boolean-and", false);
binary_operation!(BooleanOr, "boolean-or", false);
binary_operation!(BooleanXor, "boolean-xor", false);
binary_operation!(EcdsaSign, "EcdsaSign", false);
ternary_operation!(IfElse, "if-else");

/// MIR Cast operation
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[cfg_attr(any(test, feature = "builder"), derive(Hash))]
pub struct Cast {
    /// Operation identifier is generated when the model is loaded.
    pub id: OperationId,
    /// Target type
    pub to: NadaType,
    /// Operation will be casted
    pub target: OperationId,
    /// Operation type
    #[serde(rename = "type")]
    pub ty: NadaType,
    /// Source file info related with this operation.
    pub source_ref_index: SourceRefIndex,
}
identifiable_element!(Cast, OperationId);
source_info!(Cast);
typed_element!(Cast);
named_element!(Cast, "cast");
impl Cast {
    fn text_repr(&self) -> String {
        format!("oid({}) rty({}) = Cast oid({})", self.id.0, self.ty, self.target.0)
    }
}
/// MIR Random Operation
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[cfg_attr(any(test, feature = "builder"), derive(Hash))]
pub struct Random {
    /// Operation identifier is generated when the model is loaded.
    pub id: OperationId,
    /// Operation type
    #[serde(rename = "type")]
    pub ty: NadaType,
    /// Source file info related with this operation.
    pub source_ref_index: SourceRefIndex,
}

identifiable_element!(Random, OperationId);
named_element!(Random, "random");
source_info!(Random);
typed_element!(Random);
impl Random {
    fn text_repr(&self) -> String {
        format!("oid({}) rty({}) = Random", self.id.0, self.ty)
    }
}
/// MIR Input Reference
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[cfg_attr(any(test, feature = "builder"), derive(Hash))]
pub struct InputReference {
    /// Operation identifier is generated when the model is loaded.
    pub id: OperationId,
    /// Reference to input
    pub refers_to: String,
    /// Input type
    #[serde(rename = "type")]
    pub ty: NadaType,
    /// Source code info about this element.
    pub source_ref_index: SourceRefIndex,
}
identifiable_element!(InputReference, OperationId);
source_info!(InputReference);
typed_element!(InputReference);
named_element!(InputReference, "input");
impl InputReference {
    fn text_repr(&self) -> String {
        format!("oid({}) rty({}) = InputReference to({})", self.id.0, self.ty, self.refers_to)
    }
}
/// MIR Array Accessor operation
///
/// Operation that represents and access to an array, can be used for read or write operations.
/// NOTE: This is an internal operation only, it is not supported by the language for now.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[cfg_attr(any(test, feature = "builder"), derive(Hash))]
pub struct ArrayAccessor {
    /// Operation identifier is generated when the model is loaded.
    pub id: OperationId,
    /// array index - for now an integer but eventually it could be the result of an operation
    pub index: usize,
    /// source - The Operation that represents the array we are accessing
    pub source: OperationId,
    /// The type of array elements
    #[serde(rename = "type")]
    pub ty: NadaType,
    /// Source code reference of this element.
    pub source_ref_index: SourceRefIndex,
}
identifiable_element!(ArrayAccessor, OperationId);
source_info!(ArrayAccessor);
typed_element!(ArrayAccessor);
named_element!(ArrayAccessor, "array accessor");
impl ArrayAccessor {
    fn text_repr(&self) -> String {
        format!("oid({}) rty({}) = ArrayAccessor oid({}) idx({})", self.id.0, self.ty, self.source.0, self.index)
    }
}
/// Simple enumeration to identify tuple indexes in [`TupleAccessor`].
#[derive(Copy, Clone, Debug, Serialize_repr, Deserialize_repr, Eq, PartialEq)]
#[cfg_attr(any(test, feature = "builder"), derive(Hash))]
#[repr(u8)]
pub enum TupleIndex {
    /// Left index
    Left = 0,
    /// Right index
    Right = 1,
}

impl Display for TupleIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TupleIndex::Left => write!(f, "left"),
            TupleIndex::Right => write!(f, "right"),
        }
    }
}

/// MIR Tuple Accessor operation
///
/// Operation that represents and access to a tuple, can be used for read or write operations.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[cfg_attr(any(test, feature = "builder"), derive(Hash))]
pub struct TupleAccessor {
    /// Operation identifier is generated when the model is loaded.
    pub id: OperationId,
    /// tuple index (left or right)
    pub index: TupleIndex,
    /// source - The Operation that represents the tuple we are accessing
    pub source: OperationId,
    /// The type of array elements
    #[serde(rename = "type")]
    pub ty: NadaType,
    /// Source code reference of this element.
    pub source_ref_index: SourceRefIndex,
}
identifiable_element!(TupleAccessor, OperationId);
source_info!(TupleAccessor);
typed_element!(TupleAccessor);
named_element!(TupleAccessor, "tuple accessor");
impl TupleAccessor {
    fn text_repr(&self) -> String {
        format!("oid({}) rty({}) = TupleAccessor oid({}) idx({})", self.id.0, self.ty, self.source.0, self.index)
    }
}
/// MIR New Operation
///
/// INTERNAL USE ONLY.
/// Operation that signals the bytecode that it needs to generate a new compound type operation
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[cfg_attr(any(test, feature = "builder"), derive(Hash))]
pub struct New {
    /// Operation identifier is generated when the model is loaded.
    pub id: OperationId,
    /// The compound type
    #[serde(rename = "type")]
    pub ty: NadaType,
    /// The elements of this compound type
    pub elements: Vec<OperationId>,
    /// Source code info about this element.
    pub source_ref_index: SourceRefIndex,
}
identifiable_element!(New, OperationId);
source_info!(New);
typed_element!(New);
named_element!(New, "new compound type");
impl New {
    fn text_repr(&self) -> String {
        let from = self.elements.iter().map(|e| format!("oid({})", e.0)).collect::<Vec<String>>().join(", ");
        format!("oid({}) rty({}) = New from({})", self.id.0, self.ty, from)
    }
}
/// MIR Map Operation
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[cfg_attr(any(test, feature = "builder"), derive(Hash))]
pub struct Map {
    /// Operation identifier is generated when the model is loaded.
    pub id: OperationId,
    /// Function to execute
    #[serde(rename = "fn")]
    pub function_id: OperationId,
    /// Map operation input
    pub inner: OperationId,
    /// Operation type
    #[serde(rename = "type")]
    pub ty: NadaType,
    /// Source file info related with this operation.
    pub source_ref_index: SourceRefIndex,
}
identifiable_element!(Map, OperationId);
named_element!(Map, "map");
source_info!(Map);
typed_element!(Map);
impl Map {
    fn text_repr(&self) -> String {
        format!("oid({}) rty({}) = Map fn({}) target({})", self.id.0, self.ty, self.function_id, self.inner)
    }
}
/// MIR Reduce Operation
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[cfg_attr(any(test, feature = "builder"), derive(Hash))]
pub struct Reduce {
    /// Operation identifier is generated when the model is loaded.
    pub id: OperationId,
    /// Function to execute
    #[serde(rename = "fn")]
    pub function_id: OperationId,
    /// Reduce operation input
    pub inner: OperationId,
    /// Initial accumulator value
    pub initial: OperationId,
    /// Operation type
    #[serde(rename = "type")]
    pub ty: NadaType,
    /// Source file info related with this operation.
    pub source_ref_index: SourceRefIndex,
}
identifiable_element!(Reduce, OperationId);
named_element!(Reduce, "reduce");
source_info!(Reduce);
typed_element!(Reduce);
impl Reduce {
    fn text_repr(&self) -> String {
        format!(
            "oid({}) rty({}) = Reduce fn({}) target({}) initial({})",
            self.id.0, self.ty, self.function_id, self.inner, self.initial
        )
    }
}
/// MIR operation types. New operations must be added in this enum as a new variant.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[cfg_attr(any(test, feature = "builder"), derive(Hash))]
#[repr(u8)]
pub enum Operation {
    /// Reduce operation variant
    Reduce(Reduce) = 0,
    /// Map operation variant
    Map(Map) = 1,
    /// Unzip operation variant
    Unzip(Unzip) = 2,
    /// Zip operation variant
    Zip(Zip) = 3,
    /// Addition operation variant
    Addition(Addition) = 4,
    /// Addition operation variant
    Subtraction(Subtraction) = 5,
    /// Multiplication operation variant
    Multiplication(Multiplication) = 6,
    /// Less-than comparison operation variant
    LessThan(LessThan) = 7,
    /// Less-or-equal-than comparison operation variant
    LessOrEqualThan(LessOrEqualThan) = 8,
    /// Greater-than comparison operation variant
    GreaterThan(GreaterThan) = 9,
    /// Greater-or-equal-than comparison operation variant
    GreaterOrEqualThan(GreaterOrEqualThan) = 10,
    /// Equals public output comparison operation variant
    PublicOutputEquality(PublicOutputEquality) = 11,
    /// Equals private output comparison operation variant also public-public comparisons
    Equals(Equals) = 12,
    /// Cast operation variant
    Cast(Cast) = 13,
    /// InputReference operation variant
    InputReference(InputReference) = 14,
    /// LiteralReference operation variant
    LiteralReference(LiteralReference) = 15,
    /// Nada function argument variant
    NadaFunctionArgRef(NadaFunctionArgRef) = 16,
    /// Modulo operation variant
    Modulo(Modulo) = 17,
    /// Power operation variant
    Power(Power) = 18,
    /// Division operation variant
    Division(Division) = 19,
    /// Nada function call variant
    NadaFunctionCall(NadaFunctionCall) = 20,
    /// Array accessor variant
    ArrayAccessor(ArrayAccessor) = 21,
    /// Tuple accessor variant
    TupleAccessor(TupleAccessor) = 22,
    /// New operation variant
    New(New) = 23,
    /// Random operation variant
    Random(Random) = 24,
    /// IfElse operation variant
    IfElse(IfElse) = 25,
    /// Reveal operation variant
    Reveal(Reveal) = 26,
    /// Not operation variant
    Not(Not) = 27,
    /// Left Shift operation variant
    LeftShift(LeftShift) = 28,
    /// Right Shift operation variant
    RightShift(RightShift) = 29,
    /// Probabilistic truncation operation variant
    TruncPr(TruncPr) = 30,
    /// Inner product operation
    InnerProduct(InnerProduct) = 31,
    /// Not equals operation
    NotEquals(NotEquals) = 32,
    /// Boolean AND operation variant
    BooleanAnd(BooleanAnd) = 33,
    /// Boolean OR operation variant
    BooleanOr(BooleanOr) = 34,
    /// Boolean XOR operation variant
    BooleanXor(BooleanXor) = 35,
    /// Boolean XOR operation variant
    EcdsaSign(EcdsaSign) = 36,
}

impl Operation {
    /// Returns all incoming values from other operations
    pub fn incoming_operations(&self) -> Vec<OperationId> {
        use Operation::*;
        match self {
            Reduce(o) => vec![o.inner],
            Map(o) => vec![o.inner],
            Unzip(o) => vec![o.this],
            Zip(o) => vec![o.left, o.right],
            Addition(o) => vec![o.left, o.right],
            Subtraction(o) => vec![o.left, o.right],
            LessThan(o) => vec![o.left, o.right],
            LessOrEqualThan(o) => vec![o.left, o.right],
            GreaterThan(o) => vec![o.left, o.right],
            GreaterOrEqualThan(o) => vec![o.left, o.right],
            PublicOutputEquality(o) => vec![o.left, o.right],
            Equals(o) => vec![o.left, o.right],
            NotEquals(o) => vec![o.left, o.right],
            Multiplication(o) => vec![o.left, o.right],
            Modulo(o) => vec![o.left, o.right],
            Power(o) => vec![o.left, o.right],
            LeftShift(o) => vec![o.left, o.right],
            RightShift(o) => vec![o.left, o.right],
            Division(o) => vec![o.left, o.right],
            Cast(o) => vec![o.target],
            Not(o) => vec![o.this],
            New(o) => o.elements.clone(),
            InputReference(_) | NadaFunctionArgRef(_) | LiteralReference(_) | NadaFunctionCall(_) => vec![],
            ArrayAccessor(o) => vec![o.source],
            TupleAccessor(o) => vec![o.source],
            Random(_) => vec![],
            IfElse(o) => vec![o.this, o.arg_0, o.arg_1],
            Reveal(o) => vec![o.this],
            TruncPr(o) => vec![o.left, o.right],
            InnerProduct(o) => vec![o.left, o.right],
            BooleanAnd(o) => vec![o.left, o.right],
            BooleanOr(o) => vec![o.left, o.right],
            BooleanXor(o) => vec![o.left, o.right],
            EcdsaSign(o) => vec![o.left, o.right],
        }
    }

    /// Get the identifier of an operation
    pub fn id(&self) -> OperationId {
        delegate_to_inner!(self, id)
    }

    /// Get the identifier of an operation
    pub fn set_id(&mut self, id: OperationId) {
        delegate_to_inner!(self, set_id, id)
    }

    /// Text representation of the operation
    pub fn text_repr(&self) -> String {
        delegate_to_inner!(self, text_repr)
    }
}

impl TypedElement for Operation {
    fn ty(&self) -> &NadaType {
        delegate_to_inner!(self, ty)
    }
}

impl SourceInfo for Operation {
    fn source_ref_index(&self) -> SourceRefIndex {
        delegate_to_inner!(self, source_ref_index)
    }
}

impl NamedElement for Operation {
    fn name(&self) -> &str {
        delegate_to_inner!(self, name)
    }
}
