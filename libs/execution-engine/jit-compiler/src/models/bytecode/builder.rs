//! This create implements the extensions to build a program's bytecode programmatically.

use crate::models::{
    bytecode::{
        memory::{BytecodeAddress, BytecodeMemoryError},
        Addition, Input, Load, Multiplication, Operation, Output, ProgramBytecode,
    },
    memory::AddressType,
    Party, SourceRefIndex,
};
use nada_type::NadaType;

use super::Modulo;

impl ProgramBytecode {
    /// Create a new Party
    pub fn create_new_party(&mut self, name: String) -> usize {
        let party = Party { name, source_ref_index: SourceRefIndex::default() };
        let party_address = self.parties.len();
        self.parties.push(party);
        party_address
    }

    /// Create a new Input
    pub fn create_new_input(
        &mut self,
        name: String,
        party_id: usize,
        ty: NadaType,
    ) -> Result<BytecodeAddress, BytecodeMemoryError> {
        let input_address = BytecodeAddress(self.operations_count(), AddressType::Input);
        let heap_address = input_address.as_heap();
        let input = Input {
            name,
            party_id,
            address: input_address,
            ty: ty.clone(),
            source_ref_index: SourceRefIndex::default(),
        };
        let input_id = self.add_input(input)?;
        let input_ref = Operation::Load(Load {
            input_address: input_id,
            address: heap_address,
            ty,
            source_ref_index: SourceRefIndex::default(),
        });
        Ok(self.add_operation(input_ref))
    }

    /// Create a new multiplication
    pub fn create_new_multiplication(
        &mut self,
        left: BytecodeAddress,
        right: BytecodeAddress,
        ty: NadaType,
    ) -> BytecodeAddress {
        let address = BytecodeAddress(self.operations_count(), AddressType::Heap);
        let multiplication = Operation::Multiplication(Multiplication {
            address,
            left,
            right,
            ty,
            source_ref_index: SourceRefIndex::default(),
        });
        self.add_operation(multiplication)
    }

    /// Create a new addition
    pub fn create_new_addition(
        &mut self,
        left: BytecodeAddress,
        right: BytecodeAddress,
        ty: NadaType,
    ) -> Result<BytecodeAddress, BytecodeMemoryError> {
        let address = BytecodeAddress(self.operations_count(), AddressType::Heap);
        let addition =
            Operation::Addition(Addition { address, left, right, ty, source_ref_index: SourceRefIndex::default() });
        Ok(self.add_operation(addition))
    }

    /// Create a new modulo
    pub fn create_new_modulo(
        &mut self,
        left: BytecodeAddress,
        right: BytecodeAddress,
        ty: NadaType,
    ) -> Result<BytecodeAddress, BytecodeMemoryError> {
        let address = BytecodeAddress(self.operations_count(), AddressType::Heap);
        let modulo =
            Operation::Modulo(Modulo { address, left, right, ty, source_ref_index: SourceRefIndex::default() });
        Ok(self.add_operation(modulo))
    }

    /// Create a new output
    pub fn create_new_output(
        &mut self,
        name: String,
        inner: BytecodeAddress,
        ty: NadaType,
        party_id: usize,
    ) -> Result<BytecodeAddress, BytecodeMemoryError> {
        let output = Output::new(party_id, name, inner, ty, SourceRefIndex::default());
        self.memory.add_output(output)
    }
}
