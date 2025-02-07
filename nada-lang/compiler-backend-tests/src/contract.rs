#[cfg(test)]
mod tests {
    use nada_compiler_backend::{
        mir::{Addition, InputReference, OperationIdGenerator, ProgramMIR},
        program_contract::ProgramContract,
    };
    use nada_value::NadaType;

    #[test]
    #[allow(clippy::indexing_slicing)]
    fn test_circuit_contract_creation() {
        let mut program = ProgramMIR::build();
        program.add_input("a", NadaType::Integer, "party");
        program.add_input("b", NadaType::Integer, "party");
        let mut id_generator = OperationIdGenerator::default();
        let left = program.add_operation(InputReference::build("a", NadaType::Integer, id_generator.next_id()));
        let right = program.add_operation(InputReference::build("b", NadaType::Integer, id_generator.next_id()));
        let addition = program.add_operation(Addition::build(left, right, NadaType::Integer, id_generator.next_id()));
        program.add_output("output", addition, NadaType::Integer, "party");

        let contract = ProgramContract::from_program_mir(&program).unwrap();
        assert_eq!(contract.parties.len(), 1);
        assert_eq!(&contract.parties[0].name, "party");

        assert_eq!(contract.inputs.len(), 2);
        assert_eq!(&contract.inputs[0].name, "a");
        assert_eq!(contract.inputs[0].party, 0);
        assert_eq!(&contract.inputs[1].name, "b");
        assert_eq!(contract.inputs[1].party, 0);

        assert_eq!(contract.outputs.len(), 1);
        assert_eq!(&contract.outputs[0].name, "output");
        assert_eq!(contract.outputs[0].party, 0);
    }
}
