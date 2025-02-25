//! Extensions of the bytecode to protocol transformation for MPC
use crate::protocols::MPCProtocol;
use jit_compiler::{
    bytecode2protocol::{errors::Bytecode2ProtocolError, Bytecode2ProtocolContext, ProtocolFactory},
    models::bytecode::{
        memory::BytecodeAddress, Addition, Cast, Division, EcdsaSign, EddsaSign, Equals, IfElse, InnerProduct,
        LeftShift, LessThan, Modulo, Multiplication, Not, Power, PublicKeyDerive, PublicOutputEquality, Random, Reveal,
        RightShift, Subtraction, TruncPr,
    },
};
use nada_value::NadaType;

#[derive(Copy, Clone)]
pub(crate) struct MPCProtocolFactory;

impl ProtocolFactory<MPCProtocol> for MPCProtocolFactory {
    fn create_not(
        self,
        context: &mut Bytecode2ProtocolContext<MPCProtocol, Self>,
        o: &Not,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        crate::protocols::not::Not::transform(context, o)
    }

    /// Creates the protocols for an Addition operation
    fn create_addition(
        self,
        context: &mut Bytecode2ProtocolContext<MPCProtocol, Self>,
        o: &Addition,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        crate::protocols::addition::Addition::transform(context, o)
    }

    fn create_subtraction(
        self,
        context: &mut Bytecode2ProtocolContext<MPCProtocol, Self>,
        o: &Subtraction,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        crate::protocols::subtraction::Subtraction::transform(context, o)
    }

    fn create_multiplication(
        self,
        context: &mut Bytecode2ProtocolContext<MPCProtocol, Self>,
        o: &Multiplication,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        crate::protocols::multiplication::Multiplication::transform(context, o)
    }

    fn create_new_array(
        self,
        context: &mut Bytecode2ProtocolContext<MPCProtocol, Self>,
        bytecode_address: BytecodeAddress,
        inner_type: Box<NadaType>,
        size: usize,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        crate::protocols::new::NewArray::transform(context, bytecode_address, inner_type, size)
    }

    fn create_new_tuple(
        self,
        context: &mut Bytecode2ProtocolContext<MPCProtocol, Self>,
        bytecode_address: BytecodeAddress,
        left_type: Box<NadaType>,
        right_type: Box<NadaType>,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        crate::protocols::new::NewTuple::transform(context, bytecode_address, left_type, right_type)
    }

    fn create_modulo(
        self,
        context: &mut Bytecode2ProtocolContext<MPCProtocol, Self>,
        o: &Modulo,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        crate::protocols::modulo::Modulo::transform(context, o)
    }

    fn create_power(
        self,
        context: &mut Bytecode2ProtocolContext<MPCProtocol, Self>,
        o: &Power,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        crate::protocols::power::Power::transform(context, o)
    }

    fn create_left_shift(
        self,
        context: &mut Bytecode2ProtocolContext<MPCProtocol, Self>,
        o: &LeftShift,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        crate::protocols::left_shift::LeftShift::transform(context, o)
    }

    fn create_right_shift(
        self,
        context: &mut Bytecode2ProtocolContext<MPCProtocol, Self>,
        o: &RightShift,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        crate::protocols::right_shift::RightShift::transform(context, o)
    }

    fn create_division(
        self,
        context: &mut Bytecode2ProtocolContext<MPCProtocol, Self>,
        o: &Division,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        crate::protocols::division::Division::transform(context, o)
    }

    fn create_less_than(
        self,
        context: &mut Bytecode2ProtocolContext<MPCProtocol, Self>,
        o: &LessThan,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        crate::protocols::less_than::LessThan::transform(context, o)
    }

    fn create_equals(
        self,
        context: &mut Bytecode2ProtocolContext<MPCProtocol, Self>,
        o: &Equals,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        crate::protocols::equals::Equals::transform(context, o)
    }

    fn create_public_output_equality(
        self,
        context: &mut Bytecode2ProtocolContext<MPCProtocol, Self>,
        o: &PublicOutputEquality,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        crate::protocols::equals::PublicOutputEquality::transform(context, o)
    }

    fn create_if_else(
        self,
        context: &mut Bytecode2ProtocolContext<MPCProtocol, Self>,
        o: &IfElse,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        crate::protocols::if_else::IfElse::transform(context, o)
    }

    fn create_reveal(
        self,
        context: &mut Bytecode2ProtocolContext<MPCProtocol, Self>,
        o: &Reveal,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        crate::protocols::reveal::Reveal::transform(context, o)
    }

    fn create_public_key_derive(
        self,
        context: &mut Bytecode2ProtocolContext<MPCProtocol, Self>,
        o: &PublicKeyDerive,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        crate::protocols::public_key_derive::PublicKeyDerive::transform(context, o)
    }

    fn create_trunc_pr(
        self,
        context: &mut Bytecode2ProtocolContext<MPCProtocol, Self>,
        o: &TruncPr,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        crate::protocols::trunc_pr::TruncPr::transform(context, o)
    }

    fn create_ecdsa_sign(
        self,
        context: &mut Bytecode2ProtocolContext<MPCProtocol, Self>,
        o: &EcdsaSign,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        crate::protocols::ecdsa_sign::EcdsaSign::transform(context, o)
    }

    fn create_eddsa_sign(
        self,
        context: &mut Bytecode2ProtocolContext<MPCProtocol, Self>,
        o: &EddsaSign,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        crate::protocols::eddsa_sign::EddsaSign::transform(context, o)
    }

    fn create_random(
        self,
        context: &mut Bytecode2ProtocolContext<MPCProtocol, Self>,
        o: &Random,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        crate::protocols::random::Random::transform(context, o)
    }

    fn create_inner_product(
        self,
        context: &mut Bytecode2ProtocolContext<MPCProtocol, Self>,
        o: &InnerProduct,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        crate::protocols::inner_product::InnerProduct::transform(context, o)
    }

    fn create_cast(
        self,
        _: &mut Bytecode2ProtocolContext<MPCProtocol, Self>,
        _: &Cast,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        Err(Bytecode2ProtocolError::OperationNotSupported(String::from("cast")))
    }
}

#[cfg(test)]
mod tests {
    use crate::{tests::compile_protocols, MPCProtocol};
    use anyhow::Error;
    use itertools::Itertools;
    use jit_compiler::models::{
        memory::AddressType,
        protocols::{memory::ProtocolAddress, InputMemoryAllocation, InputMemoryScheme, ProtocolsModel},
    };
    use nada_value::NadaType;
    use rstest::rstest;

    #[rstest]
    #[case::input_single("input_single")]
    #[case::addition_simple("addition_simple")]
    #[case::multiplication_simple("multiplication_simple")]
    #[case::modulo_secret_public("modulo_secret_public")]
    #[case::modulo_public_public("modulo_public_public")]
    #[case::modulo_secret_secret("modulo_secret_secret")]
    #[case::modulo_public_secret("modulo_public_secret")]
    #[case::division_simple("division_simple")]
    #[case::circuit_simple("circuit_simple")]
    #[case::circuit_simple_2("circuit_simple_2")]
    #[case::import_file("import_file")]
    #[case::less_than("less_than")]
    #[case::less_or_equal_than("less_or_equal_than")]
    #[case::greater_than("greater_than")]
    #[case::greater_or_equal_than("greater_or_equal_than")]
    #[case::less_than("less_than_public_variables")]
    #[case::less_or_equal_than("less_or_equal_than_public_variables")]
    #[case::greater_than("greater_than_public_variables")]
    #[case::greater_or_equal_than("greater_or_equal_than_public_variables")]
    #[case::complex_operation_mix("complex_operation_mix")]
    #[case::reuse_simple_1("reuse_simple_1")]
    #[case::reuse_simple_2("reuse_simple_2")]
    #[case::subtraction_simple("subtraction_simple")]
    #[case::input_array("input_array")]
    #[case::if_else("if_else")]
    #[case::if_else_public_public("if_else_public_public")]
    #[case::if_else_public_secret("if_else_public_secret")]
    #[case::if_else_unsigned("if_else_unsigned")]
    #[case::if_else_unsigned_public_public("if_else_unsigned_public_public")]
    #[case::if_else_public_literal_public_literal("if_else_public_literal_public_literal")]
    #[case::if_else_secret_public_literal("if_else_secret_public_literal")]
    #[case::if_else_unsigned_secret_public_literal("if_else_unsigned_secret_public_literal")]
    #[case::if_else_public_cond_public_branches("if_else_public_cond_public_branches")]
    #[case::if_else_public_cond_secret_branches("if_else_public_cond_secret_branches")]
    #[case::reveal("reveal")]
    #[case::reveal_unsigned("reveal_unsigned")]
    #[case::reveal_many_operations("reveal_many_operations")]
    #[case::shift_left("shift_left")]
    #[case::shift_left_literal("shift_left_literal")]
    #[case::shift_left_after_add("shift_left_after_add")]
    #[case::shift_left_unsigned("shift_left_unsigned")]
    #[case::shift_left_unsigned_literal("shift_left_unsigned_literal")]
    #[case::shift_right("shift_right")]
    #[case::shift_right_literal("shift_right_literal")]
    #[case::shift_right_after_add("shift_right_after_add")]
    #[case::shift_right_unsigned("shift_right_unsigned")]
    #[case::shift_right_unsigned_literal("shift_right_unsigned_literal")]
    #[case::trunc_pr("trunc_pr")]
    #[case::trunc_pr_literal("trunc_pr_literal")]
    #[case::trunc_pr_after_add("trunc_pr_after_add")]
    #[case::trunc_pr_unsigned("trunc_pr_unsigned")]
    #[case::trunc_pr_unsigned_literal("trunc_pr_unsigned_literal")]
    #[case::equals("equals")]
    #[case::equals_public("equals_public")]
    #[case::array_inner_product("array_inner_product")]
    #[case::ecdsa_sign("ecdsa_sign")]
    #[case::eddsa_sign("eddsa_sign")]
    fn bytecode_to_protocol_compilation(#[case] test_id: &str) -> Result<(), Error> {
        compile_protocols(test_id)?;
        Ok(())
    }

    /// Verify that we are detecting the correct number of protocols
    #[rstest]
    /// A * B + C < B * D
    /// Protocols:
    /// - Multiplication of shares (A * B) -> temp1
    /// - Addition (temp1 + C) -> temp2
    /// - Multiplication of shares (B * D) -> temp3
    /// - LessThan (temp2 < temp3)
    #[case::less_than("less_than", 4)]
    /// A * B + C <= B * D
    /// Protocols:
    /// - Multiplication of shares (B * D) -> temp1
    /// - Multiplication of shares (A * B) -> temp2
    /// - Addition (temp2 + C) -> temp3
    /// - LessThan (temp1 >= temp3) -> temp4
    /// - Not (temp4)
    #[case::less_or_equal_than("less_or_equal_than", 5)]
    /// A * B + C < B * D
    /// Protocols:
    /// - Multiplication of shares (B * D) -> temp1
    /// - Multiplication of shares (A * B) -> temp2
    /// - Addition (temp2 + C) -> temp3
    /// - LessThan (temp1 < temp3)
    #[case::greater_than("greater_than", 4)]
    /// A * B + C >= B * D
    /// Protocols:
    /// - Multiplication of shares (A * B) -> temp1
    /// - Addition (temp1 + C) -> temp2
    /// - Multiplication of shares (B * D) -> temp3
    /// - LessThan (temp1 < temp3) -> temp4
    /// - Not (temp4)
    #[case::greater_or_equal_than("greater_or_equal_than", 5)]
    /// A * B + C == B * D
    /// Protocols:
    /// - Multiplication of shares (A * B) -> temp1
    /// - Addition (temp1 + C) -> temp2
    /// - Multiplication of shares (B * D) -> temp3
    /// - Equals (temp1 == temp3) -> temp4
    #[case::equals("equals", 4)]
    /// new_int = my_int1 % public_my_int2 (public_my_int2 is a public variable)
    /// Protocols:
    /// - Modulo (my_int1 % public_my_int2)
    #[case::modulo_secret_public("modulo_secret_public", 1)]
    /// new_int = my_int1 / my_int3 (my_int3 is a public variable)
    /// Protocols:
    /// - Division (my_int1 / my_int3)
    #[case::division_simple("division_simple", 1)]
    /// new_int = my_int1 * my_int2
    /// Protocols:
    /// - Multiplication of shares (my_int1 * my_int2) -> temp1
    #[case::multiplication_simple("multiplication_simple", 1)]
    /// For if else program we have 6:
    /// - left branch => 1 program (circuit)
    /// - right branch => 1 program (circuit)
    /// - LessThan
    /// - IfElse
    #[case::if_else("if_else", 2)]
    fn correct_number_of_protocols(#[case] test_id: &str, #[case] number_of_protocols: usize) -> Result<(), Error> {
        let program = compile_protocols(test_id)?;
        assert_eq!(program.protocols.len(), number_of_protocols);
        Ok(())
    }

    #[test]
    fn output_memory_scheme_single_input() -> Result<(), Error> {
        let program = compile_protocols("input_integer")?;
        assert_eq!(program.output_memory_scheme.len(), 1);
        let (output_name, memory_allocation) = program.output_memory_scheme.into_iter().next().unwrap();
        assert_eq!(output_name, "my_output".to_string());
        assert_eq!(memory_allocation.address, ProtocolAddress(0, AddressType::Input));
        assert_eq!(memory_allocation.ty, NadaType::ShamirShareInteger);
        Ok(())
    }

    #[test]
    fn output_memory_scheme_single_array() -> Result<(), Error> {
        let size: usize = 10;
        let program = compile_protocols("array_simple")?;
        assert_eq!(program.output_memory_scheme.len(), 1);
        let (output_name, memory_allocation) = program.output_memory_scheme.into_iter().next().unwrap();
        assert_eq!(output_name, "my_output".to_string());
        assert_eq!(memory_allocation.address, ProtocolAddress(size + 1, AddressType::Heap));
        assert_eq!(memory_allocation.ty, NadaType::Array { inner_type: Box::new(NadaType::ShamirShareInteger), size });
        Ok(())
    }

    #[test]
    fn output_memory_scheme_multiple_outputs() -> Result<(), Error> {
        let program = compile_protocols("multiple_outputs")?;
        assert_eq!(program.output_memory_scheme.len(), 5);
        let mut output_memory_scheme_iterator = program.output_memory_scheme.into_iter();

        // output 00
        let (output_name, memory_allocation) = output_memory_scheme_iterator.next().unwrap();
        assert_eq!(output_name, "output_00".to_string());
        assert_eq!(memory_allocation.address, ProtocolAddress(14, AddressType::Heap));
        assert_eq!(
            memory_allocation.ty,
            NadaType::Array { inner_type: Box::new(NadaType::ShamirShareInteger), size: 3 }
        );

        // output 01
        let (output_name, memory_allocation) = output_memory_scheme_iterator.next().unwrap();
        assert_eq!(output_name, "output_01".to_string());
        assert_eq!(memory_allocation.address, ProtocolAddress(4, AddressType::Input));
        assert_eq!(memory_allocation.ty, NadaType::ShamirShareInteger);

        // output 02
        let (output_name, memory_allocation) = output_memory_scheme_iterator.next().unwrap();
        assert_eq!(output_name, "output_02".to_string());
        assert_eq!(memory_allocation.address, ProtocolAddress(18, AddressType::Heap));
        assert_eq!(
            memory_allocation.ty,
            NadaType::Array { inner_type: Box::new(NadaType::ShamirShareInteger), size: 3 }
        );

        // output 03
        let (output_name, memory_allocation) = output_memory_scheme_iterator.next().unwrap();
        assert_eq!(output_name, "output_03".to_string());
        assert_eq!(memory_allocation.address, ProtocolAddress(22, AddressType::Heap));
        assert_eq!(
            memory_allocation.ty,
            NadaType::Array { inner_type: Box::new(NadaType::ShamirShareInteger), size: 3 }
        );

        // output 04
        let (output_name, memory_allocation) = output_memory_scheme_iterator.next().unwrap();
        assert_eq!(output_name, "output_04".to_string());
        assert_eq!(memory_allocation.address, ProtocolAddress(13, AddressType::Input));
        assert_eq!(memory_allocation.ty, NadaType::ShamirShareInteger);
        Ok(())
    }

    fn assert_input_array_at(input_address: usize, len: usize, program: &ProtocolsModel<MPCProtocol>) {
        assert_eq!(len + 1, program.input_memory_scheme[&ProtocolAddress(input_address, AddressType::Input)].sizeof);
    }

    fn assert_input_reference(memory_allocation: &InputMemoryAllocation, input_name: &str, size: usize) {
        assert_eq!(size, memory_allocation.sizeof);
        assert_eq!(input_name, memory_allocation.input);
    }

    fn find_input_by_name(memory: &InputMemoryScheme, name: &str) -> Option<InputMemoryAllocation> {
        for input in memory.values() {
            if input.input == name {
                return Some(input.clone());
            }
        }
        None
    }

    fn assert_input_memory_contains(memory: &InputMemoryScheme, input_names_and_sizes: &[(&str, usize)]) {
        for (name, size) in input_names_and_sizes {
            if let Some(input) = find_input_by_name(memory, name) {
                assert_eq!(*size, input.sizeof);
            } else {
                panic!("input {name} is missing from input memory");
            }
        }
    }

    #[test]
    /// Check the protocols model for Integer(1) * (A - B) is well-formed
    fn build_multiplication_subtraction() -> Result<(), Error> {
        let program = compile_protocols("multiplication_subtraction")?;

        // The input memory should contain 2 elements
        let inputs = program.input_memory_scheme;
        assert_eq!(2, inputs.len());
        let b0_input = inputs.get(&ProtocolAddress(1, AddressType::Input)).unwrap();
        let a0_input = inputs.get(&ProtocolAddress(0, AddressType::Input)).unwrap();
        assert_input_reference(b0_input, "B", 1);
        assert_input_reference(a0_input, "A", 1);

        // The output should be a shamir share integer,
        let output = program.output_memory_scheme["my_output"].clone();
        assert!(matches!(output.ty, NadaType::ShamirShareInteger));
        assert_eq!(ProtocolAddress(3, AddressType::Heap), output.address);

        // Firstly, we will find two protocols to solve the first branch (A - B). The first protocol should
        // be a circuit, and the other one should be a ShareToParticle
        let protocols = program.protocols.values().collect_vec();
        assert_eq!(2, protocols.len());
        assert!(matches!(protocols[0], MPCProtocol::Subtraction(_))); // A - B -> temp
        assert!(matches!(protocols[1], MPCProtocol::MultiplicationSharePublic(_))); // 1 * temp
        Ok(())
    }

    #[test]
    /// Check the protocols model for a + (b / Integer(2)) (shares) is well-formed
    fn build_addition_division() -> Result<(), Error> {
        let program = compile_protocols("addition_division")?;

        // The input memory should contain 2 elements
        let inputs = program.input_memory_scheme;
        assert_eq!(2, inputs.len());
        assert_input_memory_contains(&inputs, &[("A", 1), ("B", 1)]);

        // The output should be a shamir share integer,
        let output = program.output_memory_scheme["my_output"].clone();
        assert!(matches!(output.ty, NadaType::ShamirShareInteger));
        assert_eq!(ProtocolAddress(3, AddressType::Heap), output.address);

        let protocols = program.protocols.values().collect_vec();
        assert_eq!(2, program.protocols.len());
        assert!(
            matches!(protocols[0], MPCProtocol::DivisionIntegerSecretDividendPublicDivisor(_)),
            "protocol[0] is {:?}",
            protocols[0]
        ); // B / (Integer(2))
        assert!(matches!(protocols[1], MPCProtocol::Addition(_))); // A + B / Integer(2)
        Ok(())
    }

    #[test]
    /// Check the protocols model for my_int1 << amount is well-formed.
    fn build_shift_left() -> Result<(), Error> {
        let program = compile_protocols("shift_left")?;

        // The input memory should contain 2 elements
        let inputs = program.input_memory_scheme;
        assert_eq!(2, inputs.len());
        assert_input_memory_contains(&inputs, &[("my_int1", 1), ("amount", 1)]);

        // The output should be a shamir share integer,
        let output = program.output_memory_scheme["my_output"].clone();
        assert!(matches!(output.ty, NadaType::ShamirShareInteger));
        assert_eq!(ProtocolAddress(2, AddressType::Heap), output.address);

        let protocols = program.protocols.values().collect_vec();
        assert_eq!(1, program.protocols.len());
        assert!(matches!(protocols[0], MPCProtocol::LeftShiftShares(_))); // new_int = my_int1 << amount
        Ok(())
    }

    #[test]
    /// Check the protocols model for my_int1 >> amount is well-formed.
    fn build_shift_right() -> Result<(), Error> {
        let program = compile_protocols("shift_right")?;

        // The input memory should contain 2 elements
        let inputs = program.input_memory_scheme;
        assert_eq!(2, inputs.len());
        assert_input_memory_contains(&inputs, &[("my_int1", 1), ("amount", 1)]);

        // The output should be a shamir share integer,
        let output = program.output_memory_scheme["my_output"].clone();
        assert!(matches!(output.ty, NadaType::ShamirShareInteger));
        assert_eq!(ProtocolAddress(2, AddressType::Heap), output.address);

        let protocols = program.protocols.values().collect_vec();
        assert_eq!(1, program.protocols.len());
        assert!(matches!(protocols[0], MPCProtocol::RightShiftShares(_))); // new_int = my_int1 >> amount
        Ok(())
    }

    #[test]
    /// Check the protocols model for my_int1 >> amount is well-formed.
    fn build_random() -> Result<(), Error> {
        let program = compile_protocols("random_value_simple")?;

        // The input memory should contain 2 elements
        let inputs = program.input_memory_scheme;
        assert_eq!(0, inputs.len());

        // The output should be a shamir share integer,
        let output = program.output_memory_scheme["my_output"].clone();
        assert!(matches!(output.ty, NadaType::ShamirShareInteger));
        assert_eq!(ProtocolAddress(0, AddressType::Heap), output.address);

        let protocols = program.protocols.values().collect_vec();
        assert_eq!(1, program.protocols.len());
        assert!(matches!(protocols[0], MPCProtocol::RandomInteger(_))); // new_int = my_int1 >> amount
        Ok(())
    }

    #[test]
    /// Check the protocols model for my_int1.trunc_pr(amount) is well-formed.
    fn build_trunc_pr() -> Result<(), Error> {
        let program = compile_protocols("trunc_pr")?;

        // The input memory should contain 2 elements
        let inputs = program.input_memory_scheme;
        assert_eq!(2, inputs.len());
        assert_input_memory_contains(&inputs, &[("my_int1", 1), ("amount", 1)]);

        // The output should be a shamir share integer,
        let output = program.output_memory_scheme["my_output"].clone();
        assert!(matches!(output.ty, NadaType::ShamirShareInteger));
        assert_eq!(ProtocolAddress(2, AddressType::Heap), output.address);

        let protocols = program.protocols.values().collect_vec();
        assert_eq!(1, program.protocols.len());
        assert!(matches!(protocols[0], MPCProtocol::TruncPr(_))); // new_int = my_int1.trunc_pr(amount)
        Ok(())
    }

    #[test]
    /// my_array_1.zip(my_array_2)
    fn build_array_zip_protocol_model() -> Result<(), Error> {
        let size = 3;
        let program = compile_protocols("zip_simple")?;
        // The input memory should contain 2 elements
        assert_eq!(2, program.input_memory_scheme.len());
        let array_address_count = size + 1;
        assert_input_array_at(0, size, &program);
        assert_input_array_at(array_address_count, size, &program);

        assert_eq!(program.protocols.len(), 4);
        let protocols = program.protocols.values().collect_vec();
        for i in 0..size {
            assert!(matches!(protocols[i], MPCProtocol::NewTuple(_)));
        }
        assert!(matches!(protocols[3], MPCProtocol::NewArray(_)));
        Ok(())
    }

    #[test]
    /// unzip(my_array_1.zip(my_array_2))
    fn build_array_unzip_protocol_model() -> Result<(), Error> {
        let size = 3;
        let program = compile_protocols("unzip_simple")?;

        // The input memory should contain 2 elements
        assert_eq!(2, program.input_memory_scheme.len());
        let array_address_count = size + 1;
        assert_input_array_at(0, size, &program);
        assert_input_array_at(array_address_count, size, &program);

        assert_eq!(program.protocols.len(), 3);
        let protocols = program.protocols.values().collect_vec();
        // First Array
        assert!(matches!(protocols[0], MPCProtocol::NewArray(_)));

        // Second Array
        assert!(matches!(protocols[1], MPCProtocol::NewArray(_)));

        // Tuple of 2 Arrays of 3 elements
        assert!(matches!(protocols[2], MPCProtocol::NewTuple(_)));
        Ok(())
    }

    #[test]
    fn build_array_new_protocol_model() -> Result<(), Error> {
        let program = compile_protocols("array_new")?;
        // The input memory should contain 2 elements
        assert_eq!(2, program.input_memory_scheme.len());

        assert_eq!(program.protocols.len(), 1);
        let protocols = program.protocols.values().collect_vec();

        assert!(matches!(protocols[0], MPCProtocol::NewArray(_)));
        Ok(())
    }
}
