//! Utilities

use anyhow::anyhow;
use basic_types::PartyId;
use state_machine::errors::StateMachineError;

/// Sorted Parties.
///
/// Convenience implementation to handle
#[derive(Clone, Debug)]
pub(crate) struct SortedParties {
    parties: Vec<PartyId>,
}

impl SortedParties {
    pub(crate) fn new(parties: Vec<PartyId>) -> Self {
        let mut sorted_parties = parties.clone();
        sorted_parties.sort();
        Self { parties: sorted_parties }
    }

    pub(crate) fn party(&self, index: u16) -> Result<PartyId, StateMachineError> {
        Ok(self
            .parties
            .get(index as usize)
            .ok_or(StateMachineError::UnexpectedError(anyhow!("party not found from index")))?
            .clone())
    }

    pub(crate) fn index(&self, party: PartyId) -> Result<u16, StateMachineError> {
        self.parties
            .iter()
            .position(|p| p == &party)
            .ok_or(StateMachineError::UnexpectedError(anyhow!("party not found")))
            .and_then(|index| {
                u16::try_from(index)
                    .map_err(|_| StateMachineError::UnexpectedError(anyhow!("cluster too big as it should fir in u16")))
            })
    }

    pub(crate) fn len(&self) -> u16 {
        self.parties.len() as u16
    }

    pub(crate) fn parties(&self) -> Vec<PartyId> {
        self.parties.clone()
    }
}
