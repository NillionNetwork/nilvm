//! Type definitions.

use heck::ToSnakeCase;
use itertools::Itertools;
use linked_hash_map::LinkedHashMap;
pub use nada_value::NadaTypeKind as Identifier;
use std::{collections::HashMap, fmt::Display, iter::repeat};
use strum::IntoEnumIterator;
use strum_macros::{EnumIter, IntoStaticStr};

/// A literal with its underlying types.
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy, IntoStaticStr, EnumIter)]
pub enum Literal {
    /// Integer.
    Integer,

    /// Unsigned integer.
    UnsignedInteger,

    /// Boolean.
    Boolean,
}

impl Literal {
    /// Returns a Python type for this literal.
    pub const fn python_type(&self) -> &'static str {
        match self {
            Literal::Integer => "int",
            Literal::UnsignedInteger => "int", // Python doesn't support unsigned integers.
            Literal::Boolean => "bool",
        }
    }
}

/// Underlying type; used to allow/disallow operations if their input underlying types are not the same.
/// For instance an addition between an integer and an unsigned integer is not allowed (casting should be used).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum UnderlyingType {
    /// Integer.
    Integer,

    /// Unsigned integer.
    UnsignedInteger,

    /// Boolean.
    Boolean,

    /// Blob.
    Blob,

    /// Ecdsa signature.
    EcdsaSignature,
}

/// Data type representation for the operations.
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum DataType {
    /// Literal.
    Literal(Literal),

    /// Identifier.
    Identifier(Identifier),
}

/// Represents a test value, can be zero or non-zero.
/// Zero values are disallowed in certain operations like division.
/// TODO: remove this and replace it with the program input provider or something more advanced. This is only kept to
/// keep the Nada automated frontend tests running.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum TestValue {
    /// Zero value.
    Zero(&'static str),

    /// Non-zero value.
    NonZero(&'static str),
}

impl TestValue {
    /// Returns true if this test value is a zero value.
    pub fn is_zero(&self) -> bool {
        matches!(self, Self::Zero(..))
    }
}

impl Display for TestValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestValue::Zero(value) => write!(f, "{}", value),
            TestValue::NonZero(value) => write!(f, "{}", value),
        }
    }
}

impl DataType {
    /// Data type name.
    pub fn name(&self) -> String {
        match self {
            DataType::Literal(literal) => {
                let name: &str = literal.into();
                name.to_string()
            }
            DataType::Identifier(identifier) => {
                let name: &str = identifier.into();
                match identifier {
                    Identifier::Integer | Identifier::UnsignedInteger | Identifier::Boolean => {
                        format!("Public{}", name)
                    }
                    _ => name.to_string(),
                }
            }
        }
    }

    /// Python type name, i.e. how this type is represented in Python code.
    pub const fn python_type(&self) -> &'static str {
        match self {
            DataType::Literal(literal) => literal.python_type(),
            DataType::Identifier(identifier) => {
                match identifier {
                    Identifier::Integer | Identifier::SecretInteger | Identifier::ShamirShareInteger => "int",
                    Identifier::UnsignedInteger
                    | Identifier::SecretUnsignedInteger
                    | Identifier::ShamirShareUnsignedInteger => "int", // Python doesn't support unsigned integers.
                    Identifier::Boolean | Identifier::SecretBoolean | Identifier::ShamirShareBoolean => "bool",
                    Identifier::SecretBlob => "bytes",
                    Identifier::EcdsaDigestMessage => "bytes",
                    Identifier::EcdsaPrivateKey => "bytes",
                    Identifier::EcdsaPublicKey => "bytes",
                    Identifier::StoreId => "bytes",
                    Identifier::EcdsaSignature => "dict",
                    Identifier::Array => "list",
                    Identifier::Tuple => "tuple",
                    Identifier::NTuple => "tuple",
                    Identifier::Object => "dict",
                }
            }
        }
    }

    /// Returns true if this DataType is an unsigned integer.
    pub const fn is_unsigned(&self) -> bool {
        match self {
            DataType::Literal(literal) => match literal {
                Literal::Integer => false,
                Literal::UnsignedInteger => true,
                Literal::Boolean => false,
            },
            DataType::Identifier(identifier) => match identifier {
                Identifier::Integer | Identifier::SecretInteger | Identifier::ShamirShareInteger => false,
                Identifier::UnsignedInteger
                | Identifier::SecretUnsignedInteger
                | Identifier::ShamirShareUnsignedInteger => true,
                Identifier::Boolean | Identifier::SecretBoolean | Identifier::ShamirShareBoolean => false,
                Identifier::SecretBlob => false,
                Identifier::EcdsaDigestMessage => false,
                Identifier::EcdsaPrivateKey => false,
                Identifier::EcdsaSignature => false,
                Identifier::EcdsaPublicKey => false,
                Identifier::StoreId => false,
                Identifier::Array | Identifier::Tuple | Identifier::NTuple | Identifier::Object => false,
            },
        }
    }

    /// Returns the underlying type for a DataType, if there is one.
    /// Compound types typically don't have underlying types.
    pub fn underlying_type(&self) -> Option<UnderlyingType> {
        Some(match self {
            DataType::Literal(literal) => match literal {
                Literal::Integer => UnderlyingType::Integer,
                Literal::UnsignedInteger => UnderlyingType::UnsignedInteger,
                Literal::Boolean => UnderlyingType::Boolean,
            },
            DataType::Identifier(identifier) => match identifier {
                Identifier::Integer | Identifier::SecretInteger | Identifier::ShamirShareInteger => {
                    UnderlyingType::Integer
                }
                Identifier::UnsignedInteger
                | Identifier::SecretUnsignedInteger
                | Identifier::ShamirShareUnsignedInteger => UnderlyingType::UnsignedInteger,
                Identifier::Boolean | Identifier::SecretBoolean | Identifier::ShamirShareBoolean => {
                    UnderlyingType::Boolean
                }
                Identifier::SecretBlob => UnderlyingType::Blob,
                Identifier::EcdsaDigestMessage => UnderlyingType::Blob,
                Identifier::EcdsaPrivateKey => UnderlyingType::Blob,
                Identifier::EcdsaSignature => UnderlyingType::EcdsaSignature,
                Identifier::EcdsaPublicKey => UnderlyingType::Blob,
                Identifier::StoreId => UnderlyingType::Blob,
                Identifier::Array | Identifier::Tuple | Identifier::NTuple | Identifier::Object => return None,
            },
        })
    }

    /// Name of the filename this type is defined (without extension).
    pub fn filename(&self) -> String {
        self.name().to_snake_case()
    }

    /// Nada module name, i.e. name of the module where this type is defined. Used for imports.
    pub fn nada_module(&self) -> String {
        format!("nada_dsl.nada_types.{}", self.filename())
    }

    /// All types: literals and identifiers.
    pub fn all_types() -> Vec<DataType> {
        let all_types = Literal::iter().map(DataType::Literal).chain(Identifier::iter().map(DataType::Identifier));
        all_types.collect()
    }

    /// An array of values. Used in tests.
    pub const fn test_values(&self) -> &[TestValue] {
        match self {
            DataType::Literal(Literal::Integer)
            | DataType::Identifier(Identifier::Integer)
            | DataType::Identifier(Identifier::SecretInteger)
            | DataType::Identifier(Identifier::ShamirShareInteger) => &[
                TestValue::NonZero("-2"),
                TestValue::NonZero("-3"),
                TestValue::Zero("0"),
                TestValue::NonZero("4"),
                TestValue::NonZero("5"),
            ],

            DataType::Literal(Literal::UnsignedInteger)
            | DataType::Identifier(Identifier::UnsignedInteger)
            | DataType::Identifier(Identifier::SecretUnsignedInteger)
            | DataType::Identifier(Identifier::ShamirShareUnsignedInteger) => {
                &[TestValue::NonZero("2"), TestValue::NonZero("3")]
            }

            DataType::Literal(Literal::Boolean)
            | DataType::Identifier(Identifier::Boolean)
            | DataType::Identifier(Identifier::SecretBoolean)
            | DataType::Identifier(Identifier::ShamirShareBoolean) => {
                &[TestValue::NonZero("True"), TestValue::NonZero("False")]
            }
            DataType::Identifier(Identifier::SecretBlob) => {
                &[TestValue::NonZero("bytes([])"), TestValue::NonZero("bytes([42, 43, 44])")]
            }
            DataType::Identifier(Identifier::Array)
            | DataType::Identifier(Identifier::Tuple)
            | DataType::Identifier(Identifier::NTuple)
            | DataType::Identifier(Identifier::Object)
            | DataType::Identifier(Identifier::EcdsaPrivateKey)
            | DataType::Identifier(Identifier::EcdsaDigestMessage)
            | DataType::Identifier(Identifier::EcdsaSignature)
            | DataType::Identifier(Identifier::EcdsaPublicKey)
            | DataType::Identifier(Identifier::StoreId) => unimplemented!(),
        }
    }

    /// Return a type's weight. Used to automatically determine an operation's outputs depending on its inputs.
    /// We also put a lower weight on unsigned types vs signed ones, so that signed gets priority.
    pub fn weight(&self) -> usize {
        match self {
            DataType::Literal(Literal::Boolean) => 0,
            DataType::Literal(Literal::UnsignedInteger) => 1,
            DataType::Literal(Literal::Integer) => 2,

            DataType::Identifier(Identifier::Boolean) => 3,
            DataType::Identifier(Identifier::UnsignedInteger) => 4,
            DataType::Identifier(Identifier::Integer) => 5,

            DataType::Identifier(Identifier::SecretBoolean) => 6,
            DataType::Identifier(Identifier::SecretUnsignedInteger) => 7,
            DataType::Identifier(Identifier::SecretInteger) => 8,

            DataType::Identifier(Identifier::SecretBlob) => 11,

            DataType::Identifier(Identifier::ShamirShareBoolean) => 12,
            DataType::Identifier(Identifier::ShamirShareUnsignedInteger) => 13,
            DataType::Identifier(Identifier::ShamirShareInteger) => 14,

            DataType::Identifier(Identifier::EcdsaPrivateKey) => 18,
            DataType::Identifier(Identifier::EcdsaDigestMessage) => 19,
            DataType::Identifier(Identifier::EcdsaSignature) => 20,
            DataType::Identifier(Identifier::EcdsaPublicKey) => 21,

            DataType::Identifier(Identifier::StoreId) => 22,

            DataType::Identifier(Identifier::Array)
            | DataType::Identifier(Identifier::Tuple)
            | DataType::Identifier(Identifier::NTuple)
            | DataType::Identifier(Identifier::Object) => unimplemented!(),
        }
    }
}

impl Display for DataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let prefix = match self {
            DataType::Literal(_) => "literal ",
            DataType::Identifier(_) => "",
        };
        write!(f, "{}{}", prefix, self.name().to_snake_case().replace('_', " "))
    }
}

/// Python shape of this operation.
#[derive(Debug, Clone)]
pub enum PythonShape {
    /// Operator shape, like `a + b`.
    BinaryOperator {
        /// Name.
        name: String,

        /// Symbol, like '+'.
        symbol: String,
    },

    /// Instance Method shape, like `a.fun(b)`.
    InstanceMethod {
        /// Name.
        name: String,
    },
}

impl PythonShape {
    /// Operator shape, like `a + b`.
    pub fn operator(name: &str, symbol: &str) -> Self {
        Self::BinaryOperator { name: name.to_string(), symbol: symbol.to_string() }
    }

    /// Method shape, like `a.fun(b)`.
    pub fn instance_method(name: &str) -> Self {
        Self::InstanceMethod { name: name.to_string() }
    }

    /// Returns true if this Python shape is an operator.
    pub fn is_operator(&self) -> bool {
        matches!(self, Self::BinaryOperator { .. })
    }

    /// Returns true if this Python shape is a method.
    pub fn is_method(&self) -> bool {
        matches!(self, Self::InstanceMethod { .. })
    }

    /// Returns the name of the [`PythonShape`]
    pub fn name(&self) -> &str {
        match self {
            PythonShape::BinaryOperator { name, .. } => name,
            PythonShape::InstanceMethod { name } => name,
        }
    }
}

impl Display for PythonShape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PythonShape::BinaryOperator { symbol, .. } => f.write_str(symbol),
            PythonShape::InstanceMethod { name } => f.write_str(name),
        }
    }
}

/// Common metadata for all operations.
#[derive(Debug, Clone)]
pub struct OperationMetadata {
    /// Operation name. In Nada this corresponds to a function called every time any of its operators is used.
    pub name: String,

    /// Python shape for this operation.
    pub python_shape: PythonShape,

    /// Is the zero value forbidden in either side of the operation's inputs?
    pub forbid_zero: Option<Side>,

    /// Should the output always be public?
    pub public_output_override: bool,
}

/// Inner reason for an impossible combination.
#[derive(Debug, Clone)]
pub enum InnerReason {
    /// Not yet implemented, will be later.
    NotYetImplemented,

    /// This combination is mathematically impossible.
    ImpossibleMath,

    /// Type error: operation not allowed for that type.
    TypeError,
}

impl Display for InnerReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InnerReason::NotYetImplemented => write!(f, "not yet implemented"),
            InnerReason::ImpossibleMath => write!(f, "impossible math"),
            InnerReason::TypeError => write!(f, "type error"),
        }
    }
}

/// Reason for a forbidden combination.
#[derive(Debug, Clone)]
pub struct Reason {
    /// Inner reason.
    pub inner: InnerReason,

    /// Optional description.
    pub description: Option<String>,
}

impl Reason {
    /// Not yet implemented, will be later.
    pub fn not_yet_implemented() -> Self {
        Self { inner: InnerReason::NotYetImplemented, description: None }
    }

    /// This combination is mathematically impossible.
    pub fn impossible_math() -> Self {
        Self { inner: InnerReason::ImpossibleMath, description: None }
    }

    /// Type error: operation not allowed for that type.
    pub fn type_error() -> Self {
        Self { inner: InnerReason::TypeError, description: None }
    }

    /// Adds a description for this reason.
    pub fn with_description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());

        self
    }
}

impl Display for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)?;

        if let Some(description) = &self.description {
            write!(f, ": {}", description)?;
        }

        Ok(())
    }
}

/// Basic representation for Class methods
///
/// Class methods are a special category of operations, they are
/// associated with a type but do not operate on any arguments, unlike other operations.
#[derive(Debug, Clone)]
pub struct ClassMethod {
    /// Name of the MIR operation
    operation_name: String,
    /// Name of the method in NADA
    method_name: String,
    /// The list of types with this [`ClassMethod`]`
    types: Vec<DataType>,
}

impl ClassMethod {
    /// Returns a new class method.
    pub fn new(operation_name: &str, method_name: &str) -> Self {
        Self { operation_name: operation_name.to_string(), method_name: method_name.to_string(), types: vec![] }
    }

    /// Add a type to the list of types that implement this [`ClassMethod`]
    pub fn add_type(mut self, output: DataType) -> Self {
        self.types.push(output);
        self
    }

    /// Builds this operation.
    pub fn build(self) -> ClassMethod {
        ClassMethod { operation_name: self.operation_name, method_name: self.method_name, types: self.types }
    }

    /// Returns the name of the operation referred by the class method
    pub fn operation_name(&self) -> &str {
        &self.operation_name
    }

    /// Returns the name of the class method generated
    pub fn method_name(&self) -> &str {
        &self.method_name
    }
}

/// Representation for Instance Methods.
#[derive(Debug, Clone)]
pub struct InstanceMethod {
    /// Name of the MIR operation
    operation_name: String,

    /// Name of the method in NADA
    method_name: String,

    /// Optional names of the arguments this method takes.
    method_field_names: Vec<String>,

    /// A HashMap of caller types with this  [`InstanceMethod`]`. Each caller
    /// type (e.g., caller.method()) can have multiple argument types and return
    /// types.
    method_instances: HashMap<DataType, InstanceMethodVariants>,

    // The number of arguments this method takes.
    num_args: Option<usize>,

    // Whether this method has a return type.
    has_return_type: bool,
}

impl InstanceMethod {
    /// Returns a new Instance method.
    pub fn new(operation_name: &str, method_name: &str, method_field_names: &[&str]) -> Self {
        Self {
            operation_name: operation_name.to_string(),
            method_name: method_name.to_string(),
            method_field_names: method_field_names.iter().map(|&s| s.to_string()).collect(),
            method_instances: HashMap::new(),
            num_args: None,
            has_return_type: false,
        }
    }

    /// Allow combinations of types to have this Instance method
    pub fn add_type(mut self, caller: InstanceMethodVariants) -> Self {
        let vec_of_possible_types = self.method_instances.entry(caller.caller_type).or_insert(caller);
        // Check that the number of arguments are all the same and that the
        // return types are consistent (i.e., either all have or none has).
        for (args, output) in &vec_of_possible_types.parameter_types {
            // If it's unset.
            match self.num_args {
                None => {
                    self.num_args = Some(args.len());
                    self.has_return_type = output.is_some();
                }
                Some(num_args) => {
                    assert_eq!(num_args, args.len(), "wrong number of arguments");
                    assert_eq!(self.has_return_type, output.is_some(), "mismatched return type");
                }
            }
        }

        self
    }

    /// Builds this operation.
    pub fn build(self) -> InstanceMethod {
        InstanceMethod {
            operation_name: self.operation_name,
            method_name: self.method_name,
            method_field_names: self.method_field_names,
            method_instances: self.method_instances,
            num_args: self.num_args,
            has_return_type: self.has_return_type,
        }
    }

    /// Returns the name of the operation referred by the instance method
    pub fn operation_name(&self) -> &str {
        &self.operation_name
    }

    /// Returns the name of the instance method generated
    pub fn method_name(&self) -> &str {
        &self.method_name
    }

    /// Returns the names of the arguments this method takes.
    pub fn method_field_names(&self) -> &Vec<String> {
        &self.method_field_names
    }

    /// Returns the HashMap containing the allowed types for this method.
    pub fn get_variants(&self) -> &HashMap<DataType, InstanceMethodVariants> {
        &self.method_instances
    }

    /// Returns the number of arguments this method takes.
    pub fn get_num_args(&self) -> usize {
        self.num_args.unwrap_or(0)
    }
}

/// Representation for multiple Instances of a specific method.
/// Each method can have multiple parameter types (arguments and return types).
#[derive(Debug, Clone)]
pub struct InstanceMethodVariants {
    /// Type of the method caller.
    caller_type: DataType,
    /// Number of arguments of the method.
    num_args: usize,

    /// Vector of parameter types (argument types, return type).
    parameter_types: Vec<(Vec<DataType>, Option<DataType>)>,
}

impl InstanceMethodVariants {
    /// Returns a new method parameters instance.
    pub fn new(caller_type: DataType, num_args: usize) -> Self {
        Self { caller_type, parameter_types: vec![], num_args }
    }

    /// Add parameters to a specific instance method type.
    pub fn with_parameters(mut self, argument_types: Vec<DataType>, return_type: Option<DataType>) -> Self {
        self.parameter_types.push((argument_types, return_type));

        self
    }

    /// Add parameters to a specific instance method type and also add all the
    /// permutations without repetition of the argument types provided.
    pub fn with_parameters_permutations(
        mut self,
        argument_types: Vec<DataType>,
        return_type: Option<DataType>,
    ) -> Self {
        for permutation in argument_types.iter().permutations(self.num_args).unique() {
            let perm = permutation.iter().map(|&p| *p).collect();
            self.parameter_types.push((perm, return_type));
        }

        self
    }

    /// Add parameters to a specific instance method type and also add all the
    /// permutations with repetition of the argument types provided.
    ///
    /// # Arguments
    /// * `argument_types` - List of data types that will be included in the permutations
    /// * `return_type` - The return type that corresponds to all the permutations
    pub fn with_parameters_permutations_with_repetition(
        mut self,
        argument_types: Vec<DataType>,
        return_type: Option<DataType>,
    ) -> Self {
        for permutation in repeat(argument_types.into_iter()).take(self.num_args).multi_cartesian_product() {
            self.parameter_types.push((permutation, return_type));
        }

        self
    }

    /// Builds this operation.
    pub fn build(self) -> InstanceMethodVariants {
        InstanceMethodVariants {
            caller_type: self.caller_type,
            parameter_types: self.parameter_types,
            num_args: self.num_args,
        }
    }

    /// Returns the vector containing the tuples of (arguments, return types)
    /// that are allowed for this method.
    pub fn get_parameter_types(&self) -> &Vec<(Vec<DataType>, Option<DataType>)> {
        &self.parameter_types
    }
}

#[cfg(test)]
mod method_tests {
    use super::*;

    #[test]
    fn class_methods() {
        let class_method = ClassMethod::new("MyFn", "my_fn").add_type(DataType::Literal(Literal::Integer)).build();

        assert_eq!(class_method.operation_name, "MyFn");

        assert_eq!(class_method.method_name, "my_fn");
    }

    #[test]
    fn permutations_with_repetition() {
        let imv = InstanceMethodVariants::new(DataType::Identifier(Identifier::Boolean), 2)
            .with_parameters_permutations_with_repetition(
                vec![
                    DataType::Identifier(Identifier::SecretInteger),
                    DataType::Identifier(Identifier::SecretUnsignedInteger),
                ],
                Some(DataType::Identifier(Identifier::SecretInteger)),
            );
        let arg_combinations: Vec<Vec<DataType>> =
            imv.parameter_types.into_iter().map(|(data_types, _return_type)| data_types).collect();
        assert_eq!(
            vec![
                vec![DataType::Identifier(Identifier::SecretInteger), DataType::Identifier(Identifier::SecretInteger)],
                vec![
                    DataType::Identifier(Identifier::SecretInteger),
                    DataType::Identifier(Identifier::SecretUnsignedInteger)
                ],
                vec![
                    DataType::Identifier(Identifier::SecretUnsignedInteger),
                    DataType::Identifier(Identifier::SecretInteger)
                ],
                vec![
                    DataType::Identifier(Identifier::SecretUnsignedInteger),
                    DataType::Identifier(Identifier::SecretUnsignedInteger)
                ]
            ],
            arg_combinations
        )
    }

    #[test]
    fn permutations_without_repetition() {
        let imv = InstanceMethodVariants::new(DataType::Identifier(Identifier::Boolean), 2)
            .with_parameters_permutations(
                vec![
                    DataType::Identifier(Identifier::SecretInteger),
                    DataType::Identifier(Identifier::SecretUnsignedInteger),
                ],
                Some(DataType::Identifier(Identifier::SecretInteger)),
            );
        let arg_combinations: Vec<Vec<DataType>> =
            imv.parameter_types.into_iter().map(|(data_types, _return_type)| data_types).collect();
        assert_eq!(
            vec![
                vec![
                    DataType::Identifier(Identifier::SecretInteger),
                    DataType::Identifier(Identifier::SecretUnsignedInteger),
                ],
                vec![
                    DataType::Identifier(Identifier::SecretUnsignedInteger),
                    DataType::Identifier(Identifier::SecretInteger)
                ],
            ],
            arg_combinations
        )
    }
}

/// A binary operation type.
#[derive(Debug, Clone, PartialEq)]
pub enum OperationType {
    /// Arithmetic operator like add, multiply, divide, etc.
    Arithmetic,

    /// Bitwise operator like left bit shift, etc.
    Bitwise,

    /// Logical operator like equals, greater than, etc.
    /// Those operators always return a boolean.
    Logical,
}

/// Represents a built unary operation.
#[derive(Debug)]
pub struct UnaryOperation {
    metadata: OperationMetadata,
    operation_type: OperationType,
    allowed_combinations: LinkedHashMap<DataType, Option<DataType>>,
    forbidden_combinations: LinkedHashMap<DataType, Reason>,
}

/// This function exists to force the output of an operation to be a boolean.
/// We spell out all the cases to make sure that when we add new types, the compiler will complain.
fn force_boolean(
    output: DataType,
    literal: DataType,
    primitive: DataType,
    secret: DataType,
    share: DataType,
) -> DataType {
    match output {
        DataType::Literal(Literal::Integer)
        | DataType::Literal(Literal::UnsignedInteger)
        | DataType::Literal(Literal::Boolean) => literal,

        DataType::Identifier(Identifier::Integer)
        | DataType::Identifier(Identifier::UnsignedInteger)
        | DataType::Identifier(Identifier::Boolean)
        | DataType::Identifier(Identifier::EcdsaDigestMessage)
        | DataType::Identifier(Identifier::EcdsaPublicKey)
        | DataType::Identifier(Identifier::StoreId) => primitive,

        DataType::Identifier(Identifier::SecretInteger)
        | DataType::Identifier(Identifier::SecretUnsignedInteger)
        | DataType::Identifier(Identifier::SecretBoolean)
        | DataType::Identifier(Identifier::SecretBlob)
        | DataType::Identifier(Identifier::EcdsaPrivateKey)
        | DataType::Identifier(Identifier::EcdsaSignature) => secret,

        DataType::Identifier(Identifier::ShamirShareInteger)
        | DataType::Identifier(Identifier::ShamirShareUnsignedInteger)
        | DataType::Identifier(Identifier::ShamirShareBoolean) => share,

        // Do nothing for compound types.
        DataType::Identifier(Identifier::Array)
        | DataType::Identifier(Identifier::Tuple)
        | DataType::Identifier(Identifier::NTuple)
        | DataType::Identifier(Identifier::Object) => output,
    }
}

impl UnaryOperation {
    /// Returns a new unary operation.
    pub fn new(operation_type: OperationType, name: &str, python_shape: PythonShape) -> Self {
        Self {
            metadata: OperationMetadata {
                name: name.to_string(),
                python_shape,
                forbid_zero: None,
                public_output_override: false,
            },
            operation_type,
            allowed_combinations: LinkedHashMap::default(),
            forbidden_combinations: LinkedHashMap::default(),
        }
    }

    /// Allows a combination of types.
    /// The output type is automatically deduced using the input type's weight.
    pub fn allow(mut self, input: DataType) -> Self {
        self.allowed_combinations.insert(input, None);

        self
    }

    /// Allows a combination of types.
    /// The output type is automatically deduced using the input type's weight.
    pub fn allow_multiple(mut self, inputs: &[DataType]) -> Self {
        for input in inputs {
            self.allowed_combinations.insert(*input, None);
        }
        self
    }

    /// Allows a combination of types.
    /// The output type is automatically deduced using the input type's weight.
    pub fn allow_with_output(mut self, input: DataType, output: DataType) -> Self {
        self.allowed_combinations.insert(input, Some(output));

        self
    }

    /// Forbids a list of types.
    /// The `reason` parameter allows to specify why that combination has to be removed.
    pub fn forbid(mut self, inputs: &[DataType], reason: Reason) -> Self {
        for input in inputs {
            self.forbidden_combinations.insert(*input, reason.clone());
        }

        self
    }

    /// Forbid zero values.
    pub fn forbid_zero(mut self) -> Self {
        self.metadata.forbid_zero = Some(Side::Both);

        self
    }

    /// Forces this operation to output a public value.
    pub fn force_public_output_override(mut self) -> Self {
        self.metadata.public_output_override = true;

        self
    }

    /// Builds this operation.
    pub fn build(self) -> BuiltUnaryOperation {
        let mut allowed_combinations = LinkedHashMap::new();

        // Add all possible combination, except when the underlying type is different.
        for input in DataType::all_types() {
            allowed_combinations.insert(input, input);
        }

        // Then remove any forbidden combination.
        for (input, _) in &self.forbidden_combinations {
            allowed_combinations.remove(input);
        }

        // And finally add any explicitly allowed combination.
        for (input, output) in self.allowed_combinations {
            let mut output = output.unwrap_or(input);
            // Force the data type to be a boolean if this is a logical operation.
            if self.operation_type == OperationType::Logical {
                output = force_boolean(
                    output,
                    DataType::Literal(Literal::Boolean),
                    DataType::Identifier(Identifier::Boolean),
                    DataType::Identifier(Identifier::SecretBoolean),
                    DataType::Identifier(Identifier::ShamirShareBoolean),
                )
            }

            // Apply public output override.
            if self.metadata.public_output_override {
                output = force_boolean(
                    output,
                    DataType::Literal(Literal::Boolean),
                    DataType::Identifier(Identifier::Boolean),
                    DataType::Identifier(Identifier::Boolean),
                    DataType::Identifier(Identifier::Boolean),
                )
            }

            allowed_combinations.insert(input, output);
        }

        BuiltUnaryOperation {
            metadata: self.metadata,
            operation_type: self.operation_type,
            allowed_combinations,
            forbidden_combinations: self.forbidden_combinations,
        }
    }
}

/// Left, right or both sides of an operator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Side {
    /// Left side.
    Left,

    /// Right side.
    Right,

    /// Both sides.
    Both,
}

/// Represents a binary operation, like add, multiply, greater than, etc.
#[derive(Debug)]
pub struct BinaryOperation {
    metadata: OperationMetadata,
    operation_type: OperationType,
    allowed_combinations: LinkedHashMap<(DataType, DataType), Option<DataType>>,
    forbidden_combinations: LinkedHashMap<(DataType, DataType), Reason>,
}

impl BinaryOperation {
    /// Returns a new binary operation.
    pub fn new(operation_type: OperationType, name: &str, python_shape: PythonShape) -> Self {
        Self {
            metadata: OperationMetadata {
                name: name.to_string(),
                python_shape,
                forbid_zero: None,
                public_output_override: false,
            },
            operation_type,
            allowed_combinations: LinkedHashMap::default(),
            forbidden_combinations: LinkedHashMap::default(),
        }
    }

    /// Allows a combination of left and right types.
    /// The output type is automatically deduced using the input type's weight.
    pub fn allow(mut self, left_input: DataType, right_input: DataType) -> Self {
        self.allowed_combinations.insert((left_input, right_input), None);

        self
    }

    /// Allows a combination of left and right types.
    /// The output type is automatically deduced using the input type's weight.
    pub fn allow_multiple(mut self, left_input: &[DataType], right_input: &[DataType]) -> Self {
        for left_type in left_input {
            for right_type in right_input {
                self.allowed_combinations.insert((*left_type, *right_type), None);
            }
        }
        self
    }

    /// Allows a combination of left, right and output types.
    pub fn allow_with_output(mut self, left_input: DataType, right_input: DataType, output: DataType) -> Self {
        self.allowed_combinations.insert((left_input, right_input), Some(output));

        self
    }

    /// Forbids a list of types on both the left and right sides of an operation.
    /// The `reason` parameter allows to specify why that combination has to be removed.
    pub fn forbid(mut self, inputs: &[DataType], reason: Reason) -> Self {
        self = self.forbid_left(inputs, reason.clone());
        self = self.forbid_right(inputs, reason);

        self
    }

    /// Forbids a list of types on the left side of an operation.
    /// The `reason` parameter allows to specify why that combination has to be removed.
    pub fn forbid_left(mut self, inputs: &[DataType], reason: Reason) -> Self {
        for left_type in inputs {
            for right_type in DataType::all_types() {
                self.forbidden_combinations.insert((*left_type, right_type), reason.clone());
            }
        }

        self
    }

    /// Forbids a list of types on the right side of an operation.
    /// The `reason` parameter allows to specify why that combination has to be removed.
    pub fn forbid_right(mut self, inputs: &[DataType], reason: Reason) -> Self {
        for left_type in DataType::all_types() {
            for right_type in inputs {
                self.forbidden_combinations.insert((left_type, *right_type), reason.clone());
            }
        }

        self
    }

    /// Forbid zero values on both the left and right sides of an operation.
    pub fn forbid_zero(mut self) -> Self {
        self.metadata.forbid_zero = Some(Side::Both);

        self
    }

    /// Forbid zero values on the left side of an operation.
    pub fn forbid_zero_left(mut self) -> Self {
        self.metadata.forbid_zero = Some(Side::Left);

        self
    }

    /// Forbid zero values on the right side of an operation.
    pub fn forbid_zero_right(mut self) -> Self {
        self.metadata.forbid_zero = Some(Side::Right);

        self
    }

    /// Forces this operation to output a public value.
    pub fn force_public_output_override(mut self) -> Self {
        self.metadata.public_output_override = true;

        self
    }

    /// Builds this operation.
    pub fn build(mut self) -> BuiltBinaryOperation {
        let mut allowed_combinations = LinkedHashMap::new();

        // Add all possible combination, except when the underlying type is different.
        for left in DataType::all_types() {
            for right in DataType::all_types() {
                if left.underlying_type() != right.underlying_type() {
                    self.forbidden_combinations
                        .entry((left, right))
                        .or_insert(Reason::type_error().with_description("same input types only"));
                    continue;
                }

                allowed_combinations.insert((left, right), None);
            }
        }

        // Then remove any forbidden combination.
        for (input, _) in &self.forbidden_combinations {
            allowed_combinations.remove(input);
        }

        // And finally add any explicitly allowed combination.
        for (input, output) in self.allowed_combinations {
            allowed_combinations.insert(input, output);
        }

        let mut allowed_combinations_with_output = LinkedHashMap::with_capacity(allowed_combinations.len());

        // Fill in the output type, either auto determined using weight or specified by the user.
        for ((left, right), output) in allowed_combinations {
            let output = if let Some(output) = output {
                // If we already have an output, keep it.
                output
            } else {
                // If we don't, we need to deduce it from the inputs.
                let mut output = if left.weight() > right.weight() { left } else { right };

                // Force the data type to be a boolean if this is a logical operation.
                if self.operation_type == OperationType::Logical {
                    output = force_boolean(
                        output,
                        DataType::Literal(Literal::Boolean),
                        DataType::Identifier(Identifier::Boolean),
                        DataType::Identifier(Identifier::SecretBoolean),
                        DataType::Identifier(Identifier::ShamirShareBoolean),
                    )
                }

                // Apply public output override.
                if self.metadata.public_output_override {
                    output = force_boolean(
                        output,
                        DataType::Literal(Literal::Boolean),
                        DataType::Identifier(Identifier::Boolean),
                        DataType::Identifier(Identifier::Boolean),
                        DataType::Identifier(Identifier::Boolean),
                    )
                }

                output
            };

            allowed_combinations_with_output.insert((left, right), output);
        }

        BuiltBinaryOperation {
            metadata: self.metadata,
            operation_type: self.operation_type,
            allowed_combinations: allowed_combinations_with_output,
            forbidden_combinations: self.forbidden_combinations,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nada_value::NadaTypeKind;
    use OperationType::*;

    #[test]
    fn unary_operation() {
        let operation = BinaryOperation::new(Arithmetic, "MyOp", PythonShape::operator("my_op", "$"))
            .forbid(
                &[
                    DataType::Identifier(NadaTypeKind::Array),
                    DataType::Identifier(NadaTypeKind::Tuple),
                    DataType::Identifier(NadaTypeKind::NTuple),
                    DataType::Identifier(NadaTypeKind::Object),
                ],
                Reason::not_yet_implemented(),
            )
            .forbid(
                &[
                    DataType::Identifier(NadaTypeKind::SecretInteger),
                    DataType::Identifier(NadaTypeKind::SecretUnsignedInteger),
                ],
                Reason::not_yet_implemented(),
            )
            .build();

        assert_eq!(operation.metadata.name, "MyOp");
        if let PythonShape::BinaryOperator { name, symbol } = operation.metadata.python_shape {
            assert_eq!(name, "my_op");
            assert_eq!(symbol, "$");
        } else {
            assert!(false, "unexpected python shape");
        }
        assert_eq!(operation.operation_type, Arithmetic);
        assert!(!operation.allowed_combinations.is_empty());
    }

    #[test]
    fn binary_operation() {
        let operation = BinaryOperation::new(Arithmetic, "MyOp", PythonShape::operator("my_op", "$"))
            .forbid(
                &[
                    DataType::Identifier(NadaTypeKind::Array),
                    DataType::Identifier(NadaTypeKind::Tuple),
                    DataType::Identifier(NadaTypeKind::NTuple),
                    DataType::Identifier(NadaTypeKind::Object),
                ],
                Reason::not_yet_implemented(),
            )
            .forbid(
                &[
                    DataType::Identifier(NadaTypeKind::SecretInteger),
                    DataType::Identifier(NadaTypeKind::SecretUnsignedInteger),
                ],
                Reason::not_yet_implemented(),
            )
            .build();

        assert_eq!(operation.metadata.name, "MyOp");
        if let PythonShape::BinaryOperator { name, symbol } = operation.metadata.python_shape {
            assert_eq!(name, "my_op");
            assert_eq!(symbol, "$");
        } else {
            assert!(false, "unexpected python shape");
        }
        assert_eq!(operation.operation_type, Arithmetic);
        assert!(!operation.allowed_combinations.is_empty());
    }

    #[test]
    fn instance_methods() {
        let meth = InstanceMethod::new("MyFn", "my_fn", &[])
            .add_type(
                InstanceMethodVariants::new(DataType::Identifier(Identifier::SecretBoolean), 2)
                    .with_parameters(
                        vec![
                            DataType::Identifier(Identifier::SecretInteger),
                            DataType::Identifier(Identifier::SecretInteger),
                        ],
                        Some(DataType::Identifier(Identifier::SecretInteger)),
                    )
                    .build(),
            )
            .build();

        assert_eq!(meth.operation_name, "MyFn");
        assert_eq!(meth.method_name, "my_fn");
        assert_eq!(meth.method_instances.len(), 1); // only one method caller
        assert!(meth.method_instances.get(&DataType::Identifier(Identifier::SecretInteger)).is_none());
        let method_caller = meth.method_instances.get(&DataType::Identifier(Identifier::SecretBoolean)).unwrap();
        assert_eq!(method_caller.parameter_types.len(), 1); // one parameter set
        assert_eq!(method_caller.parameter_types[0].0.len(), 2); // two arguments
        assert!(method_caller.parameter_types[0].1.is_some()); // one return type
    }

    #[test]
    #[should_panic(expected = "wrong number of arguments")]
    fn illegal_number_of_arguments() {
        let meth = InstanceMethod::new("MyFn", "my_fn", &[])
            .add_type(
                InstanceMethodVariants::new(DataType::Identifier(Identifier::SecretBoolean), 2)
                    .with_parameters(
                        vec![
                            DataType::Identifier(Identifier::SecretInteger),
                            DataType::Identifier(Identifier::SecretInteger),
                        ],
                        Some(DataType::Identifier(Identifier::SecretInteger)),
                    )
                    .with_parameters(
                        vec![DataType::Identifier(Identifier::SecretInteger)],
                        Some(DataType::Identifier(Identifier::SecretInteger)),
                    )
                    .build(),
            )
            .build();

        // Unreachable
        assert_eq!(meth.operation_name, "MyFn");
        assert_eq!(meth.method_name, "my_fn");
    }

    #[test]
    #[should_panic(expected = "mismatched return type")]
    fn illegal_return_types() {
        let meth = InstanceMethod::new("MyFn", "my_fn", &[])
            .add_type(
                InstanceMethodVariants::new(DataType::Identifier(Identifier::SecretBoolean), 2)
                    .with_parameters(
                        vec![
                            DataType::Identifier(Identifier::SecretInteger),
                            DataType::Identifier(Identifier::SecretInteger),
                        ],
                        Some(DataType::Identifier(Identifier::SecretInteger)),
                    )
                    .with_parameters(
                        vec![
                            DataType::Identifier(Identifier::SecretInteger),
                            DataType::Identifier(Identifier::SecretInteger),
                        ],
                        None,
                    )
                    .build(),
            )
            .build();

        // Unreachable
        assert_eq!(meth.operation_name, "MyFn");
        assert_eq!(meth.method_name, "my_fn");
    }
}

/// Represents a built unary operation.
#[derive(Debug, Clone)]
pub struct BuiltUnaryOperation {
    /// Operation metadata.
    pub metadata: OperationMetadata,

    /// Type of this operation: arithmetic, logical, etc.
    pub operation_type: OperationType,

    /// Allowed combinations of inputs.
    pub allowed_combinations: LinkedHashMap<DataType, DataType>,

    /// Forbidden combinations of inputs.
    pub forbidden_combinations: LinkedHashMap<DataType, Reason>,
}

/// Represents a built binary operation.
#[derive(Debug)]
pub struct BuiltBinaryOperation {
    /// Operation metadata.
    pub metadata: OperationMetadata,

    /// Type of this operation: arithmetic, logical, etc.
    pub operation_type: OperationType,

    /// Allowed combinations of inputs.
    pub allowed_combinations: LinkedHashMap<(DataType, DataType), DataType>,

    /// Forbidden combinations of inputs.
    pub forbidden_combinations: LinkedHashMap<(DataType, DataType), Reason>,
}

/// Represents a map of operations.
#[derive(Debug, Default)]
pub struct Operations {
    unary_operations: LinkedHashMap<String, BuiltUnaryOperation>,
    binary_operations: LinkedHashMap<String, BuiltBinaryOperation>,
    class_methods: LinkedHashMap<String, ClassMethod>,
    instance_methods: LinkedHashMap<String, InstanceMethod>,
}

impl Operations {
    /// Adds a unary operation.
    pub fn add_unary(mut self, operation: BuiltUnaryOperation) -> Self {
        self.unary_operations.insert(operation.metadata.name.clone(), operation);
        self
    }

    /// Adds a binary operation.
    pub fn add_binary(mut self, operation: BuiltBinaryOperation) -> Self {
        self.binary_operations.insert(operation.metadata.name.clone(), operation);

        self
    }

    /// Add adhoc function to operations
    pub fn add_class_method(mut self, operation: ClassMethod) -> Self {
        self.class_methods.insert(operation.method_name.clone(), operation);
        self
    }

    /// Add instance method to operations
    pub fn add_instance_method(mut self, operation: InstanceMethod) -> Self {
        self.instance_methods.insert(operation.method_name.clone(), operation);
        self
    }

    /// Build all operations.
    pub fn build(self) -> BuiltOperations {
        // Generate a list of type combinations per type.
        let mut type_operations = LinkedHashMap::new();
        let mut unary_operations_by_type = LinkedHashMap::new();
        let mut classmethods_by_type = LinkedHashMap::new();
        let mut obj_methods_by_type = LinkedHashMap::new();
        for data_type in DataType::all_types() {
            let data_type_operations = type_operations.entry(data_type).or_insert(LinkedHashMap::new());

            for (_, operation) in self.unary_operations.iter() {
                for (input, _) in operation.clone().allowed_combinations.iter() {
                    if input != &data_type {
                        continue;
                    }
                    let unary_op: &mut Vec<BuiltUnaryOperation> =
                        unary_operations_by_type.entry(data_type).or_insert(vec![]);
                    unary_op.push(operation.clone());
                }
            }

            for (name, operations) in self.binary_operations.iter() {
                for ((left_input, right_input), output) in operations.allowed_combinations.iter() {
                    if left_input != &data_type {
                        continue;
                    }

                    let operation = data_type_operations.entry(name.clone()).or_insert((
                        Vec::new(),
                        operations.metadata.clone(),
                        operations.operation_type.clone(),
                    ));
                    operation.0.push(((*left_input, *right_input), *output));
                }
            }
            for (_, class_method) in self.class_methods.iter() {
                if class_method.types.contains(&data_type) {
                    let classmethod_type: &mut Vec<ClassMethod> =
                        classmethods_by_type.entry(data_type).or_insert(vec![]);
                    classmethod_type.push(class_method.clone());
                }
            }
            for (_, obj_method) in self.instance_methods.iter() {
                if obj_method.method_instances.contains_key(&data_type) {
                    let method_type: &mut Vec<InstanceMethod> = obj_methods_by_type.entry(data_type).or_insert(vec![]);
                    method_type.push(obj_method.clone());
                }
            }
        }

        BuiltOperations {
            unary_operations: unary_operations_by_type,
            binary_operations: self.binary_operations,
            operations_by_type: type_operations,
            class_methods: classmethods_by_type,
            instance_methods: obj_methods_by_type,
        }
    }
}

/// Type for a map of operations keyed by data type.
pub type BinaryOperationsByType = LinkedHashMap<
    DataType,
    LinkedHashMap<String, (Vec<((DataType, DataType), DataType)>, OperationMetadata, OperationType)>,
>;

/// Built operations.
#[derive(Debug)]
pub struct BuiltOperations {
    /// List of unary operations by type.
    pub unary_operations: LinkedHashMap<DataType, Vec<BuiltUnaryOperation>>,

    /// List of binary operations.
    /// The key is the name of the operation
    pub binary_operations: LinkedHashMap<String, BuiltBinaryOperation>,

    /// List of operations by type.
    pub operations_by_type: BinaryOperationsByType,

    /// List of class methods by type
    pub class_methods: LinkedHashMap<DataType, Vec<ClassMethod>>,

    /// List of instance methods by type
    pub instance_methods: LinkedHashMap<DataType, Vec<InstanceMethod>>,
}
