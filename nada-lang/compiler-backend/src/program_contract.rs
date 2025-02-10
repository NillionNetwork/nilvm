//! This crate implements the Circuit Contract model

use mir_model::{Input as MIRInput, OutputElement, Party, ProgramMIR};
use nada_value::NadaType;
use std::collections::{HashMap, HashSet};

use super::literal_value::{LiteralValue, LiteralValueError};

/// Contains the information about a Circuit's input
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Input {
    /// Input name
    pub name: String,
    /// Index of the Party at the vector of parties is contained by the contract
    pub party: usize,
    /// Input type
    pub ty: NadaType,
    /// Input reading count
    pub readings: usize,
}

/// Contains the information about a Circuit's output
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]

pub struct Output {
    /// Output name
    pub name: String,
    /// Index of the Party at the vector of parties is contained by the contract
    pub party: usize,
    /// Output type
    pub ty: NadaType,
}

/// Contains the information about a Circuit's literal
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Literal {
    /// Input name
    pub name: String,
    /// Value
    pub value: LiteralValue,
}

/// The circuit contract contains all information about the circuit's inputs and outputs
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProgramContract {
    /// Parties that contain the inputs and will contain the outputs
    pub parties: Vec<Party>,
    /// Circuit's inputs
    pub inputs: Vec<Input>,
    /// Circuit's outputs
    pub outputs: Vec<Output>,
}

impl ProgramContract {
    /// Create a CircuitContract from a MIR program
    pub fn from_program_mir(program: &ProgramMIR) -> Result<ProgramContract, ProgramContractError> {
        Ok(ProgramContract {
            inputs: Self::build_inputs(program, &program.parties)?,
            outputs: Self::build_outputs(&program.outputs, &program.parties)?,
            parties: program.parties.clone(),
        })
    }

    fn build_inputs(program: &ProgramMIR, parties: &[Party]) -> Result<Vec<Input>, ProgramContractError> {
        let mut inputs = Vec::new();
        let inputs_readings = program.count_inputs_readings();
        for MIRInput { name, ty, party, .. } in &program.inputs {
            let readings = inputs_readings.get(name).copied().unwrap_or_default();
            let party = if party != "literals" { Self::try_party_index(parties, party)? } else { 0 };

            inputs.push(Input { name: name.clone(), ty: ty.clone(), party, readings })
        }
        Ok(inputs)
    }

    fn build_outputs<O: OutputElement>(
        mir_outputs: &[O],
        parties: &[Party],
    ) -> Result<Vec<Output>, ProgramContractError> {
        let mut outputs = Vec::new();
        for output in mir_outputs.iter() {
            let party = Self::try_party_index(parties, output.party())?;
            outputs.push(Output { name: output.name().to_string(), ty: output.ty().clone(), party })
        }
        Ok(outputs)
    }

    fn try_party_index(parties: &[Party], party_name: &str) -> Result<usize, ProgramContractError> {
        parties
            .iter()
            .enumerate()
            .find(|(_, p)| p.name == party_name)
            .map(|(index, _)| index)
            .ok_or(ProgramContractError::PartyNotFound(party_name.to_string()))
    }

    fn group_by_party_name<'a, T>(
        &'a self,
        items: &'a Vec<T>,
        get_party_id: fn(&'a T) -> usize,
    ) -> Result<HashMap<&'a String, Vec<&'a T>>, ProgramContractError> {
        let mut grouped_items: HashMap<&String, Vec<&T>> = HashMap::new();
        for item in items {
            let party_id = get_party_id(item);
            let party = self.parties.get(party_id).ok_or(ProgramContractError::PartyOutOfBound)?;
            grouped_items.entry(&party.name).or_default().push(item);
        }
        Ok(grouped_items)
    }

    /// This function return an iterator over the program's input secrets.
    pub fn inputs_iter(&self) -> impl Iterator<Item = &Input> {
        self.inputs.iter()
    }

    /// This function return an iterator over the program's input secrets.
    pub fn input_secrets_iter(&self) -> impl Iterator<Item = &Input> {
        self.inputs.iter().filter(|input| input.ty.is_secret())
    }

    /// Group the program's input by the party that contain them
    pub fn inputs_by_party_name(&self) -> Result<HashMap<&String, Vec<&Input>>, ProgramContractError> {
        self.group_by_party_name(&self.inputs, |i| i.party)
    }

    /// Group the program's input by the party that contain them
    pub fn outputs_by_party_name(&self) -> Result<HashMap<&String, Vec<&Output>>, ProgramContractError> {
        self.group_by_party_name(&self.outputs, |o| o.party)
    }

    fn collect_parties<'a, T>(
        &'a self,
        items: &'a Vec<T>,
        get_party: fn(&'a T) -> usize,
    ) -> Result<Vec<&'a Party>, ProgramContractError> {
        let mut parties = HashSet::new();
        for item in items {
            let input_party = self.parties.get(get_party(item)).ok_or(ProgramContractError::PartyOutOfBound)?;
            parties.insert(input_party);
        }
        if parties.is_empty() {
            Err(ProgramContractError::NoParties)?;
        }
        Ok(parties.into_iter().collect())
    }

    /// Get the input parties defined by the program
    pub fn input_parties(&self) -> Result<Vec<&Party>, ProgramContractError> {
        self.collect_parties(&self.inputs, |i| i.party)
    }

    /// Get the output parties defined by the program
    pub fn output_parties(&self) -> Result<Vec<&Party>, ProgramContractError> {
        self.collect_parties(&self.outputs, |o| o.party)
    }

    /// Get the input types
    pub fn input_types(&self) -> HashMap<String, NadaType> {
        self.inputs.iter().map(|input| (input.name.clone(), input.ty.clone())).collect()
    }

    /// Get the outputs types
    pub fn output_types(&self) -> HashMap<String, NadaType> {
        self.outputs.iter().map(|output| (output.name.clone(), output.ty.clone())).collect()
    }
}

/// An error during the Program Contract building.
#[derive(Debug, thiserror::Error)]
pub enum ProgramContractError {
    /// The party was not found.
    #[error("party {0} not found")]
    PartyNotFound(String),
    /// Party out of bound
    #[error("party out of bound")]
    PartyOutOfBound,
    /// Multi-parties program is found
    #[error("multi-parties programs are not supported")]
    MultiPartiesProgram,
    /// Program is not using any party
    #[error("program is not using any party")]
    NoParties,
    /// Literal value error
    #[error("failed parsing a literal value: {0}")]
    LiteralValueParsingFailed(#[from] LiteralValueError),
}
