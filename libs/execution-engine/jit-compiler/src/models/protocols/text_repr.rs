//! This library implements the text representation for the protocols model

use crate::models::protocols::{InputMemoryAllocation, OutputMemoryAllocation, Protocol, ProtocolsModel};

impl<P: Protocol> ProtocolsModel<P> {
    /// Returns the text representation of the program
    pub fn text_repr(&self) -> String {
        let mut repr = self.contract_text_repr();
        repr.push_str("\n\n");
        repr.push_str(self.protocols_text_repr().as_str());
        repr
    }

    /// Returns the text representation of the contract (inputs, outputs and literals)
    pub fn contract_text_repr(&self) -> String {
        let mut repr = String::from("Literals:\n");
        for literal in self.literals.iter() {
            repr.push_str(&format!("{literal}\n"));
        }
        repr.push_str("\n\n");
        repr.push_str("Inputs:\n");
        for (address, allocation) in self.input_memory_scheme.iter() {
            let reads = self.reads_table.get(address).copied().unwrap_or_default();
            let allocation_txt = allocation.text_repr();
            repr.push_str(&format!("{address}: {allocation_txt}, reads: {reads}\n"));
            let mut inner_address = *address;
            for _ in 1..allocation.sizeof {
                if let Ok(new_address) = inner_address.next() {
                    inner_address = new_address;
                    let reads = self.reads_table.get(&inner_address).copied().unwrap_or_default();
                    repr.push_str(&format!("  - {inner_address}: reads: {reads}\n"));
                }
            }
        }
        repr.push_str("\n\n");
        repr.push_str("Outputs:\n");
        for (output, allocation) in self.output_memory_scheme.iter() {
            let allocation_txt = allocation.text_repr();
            repr.push_str(&format!("{allocation_txt}: {output}\n"))
        }
        repr
    }

    /// Returns the text representation of the protocols instructions
    pub fn protocols_text_repr(&self) -> String {
        let mut repr = String::from("Protocols:\n\n");
        for protocol in self.protocols.values() {
            repr.push_str(&protocol.text_repr(self));
            repr.push('\n');
        }
        repr
    }
}

impl InputMemoryAllocation {
    /// returns the text representation of the InputMemoryAllocation
    pub(crate) fn text_repr(&self) -> String {
        format!("{} (sizeof: {})", self.input, self.sizeof)
    }
}

impl OutputMemoryAllocation {
    /// returns the text representation of the OutputMemoryAllocation
    pub(crate) fn text_repr(&self) -> String {
        format!("{} ({})", self.address, self.ty)
    }
}
