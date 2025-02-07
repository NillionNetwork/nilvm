//! A party id abstraction.

use std::{
    fmt,
    fmt::{Debug, Display, Formatter},
    hash::Hash,
    str::FromStr,
};
use thiserror::Error;
use uuid::Uuid;

/// Party ID decode error.
#[derive(Error, Debug)]
#[error("invalid party id: {0}")]
pub struct InvalidPartyId(String);

/// Represents a party identifier.
#[derive(Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PartyId(Vec<u8>);

impl FromStr for PartyId {
    type Err = InvalidPartyId;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = hex::decode(s).map_err(|e| InvalidPartyId(e.to_string()))?;
        Ok(Self(bytes))
    }
}

impl Display for PartyId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", hex::encode(&self.0))
    }
}

impl Debug for PartyId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "PartyId({})", self)
    }
}

impl AsRef<[u8]> for PartyId {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl From<Vec<u8>> for PartyId {
    fn from(data: Vec<u8>) -> Self {
        PartyId(data)
    }
}

impl From<&[u8]> for PartyId {
    fn from(data: &[u8]) -> Self {
        PartyId(data.to_vec())
    }
}

impl From<Uuid> for PartyId {
    fn from(id: Uuid) -> PartyId {
        PartyId::from(id.as_ref())
    }
}

impl From<usize> for PartyId {
    fn from(num: usize) -> PartyId {
        PartyId::from(num.to_le_bytes().to_vec())
    }
}

/// A message that was sent by a particular party.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PartyMessage<T> {
    /// The sender party id.
    pub sender: PartyId,

    /// The message itself.
    pub message: T,
}

impl<T> PartyMessage<T> {
    /// Construct a new party message.
    pub fn new(sender: PartyId, message: T) -> Self {
        Self { sender, message }
    }

    /// Decompose this party message into its sender and inner message.
    pub fn into_parts(self) -> (PartyId, T) {
        (self.sender, self.message)
    }

    /// Construct a new party message from another compatible message.
    pub fn from_message<I>(message: PartyMessage<I>) -> Self
    where
        T: From<I>,
    {
        Self { sender: message.sender, message: T::from(message.message) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_party_id_bytes_array() {
        let party_id_1 = PartyId::from(vec![1, 2, 3, 4]);
        let party_id_2 = PartyId::from(vec![1, 2, 3, 4]);
        let party_id_3 = PartyId::from(vec![1, 2, 3]);
        assert_eq!(party_id_1, party_id_2);
        assert_eq!(party_id_1.as_ref(), &[1, 2, 3, 4]);
        assert_ne!(party_id_1, party_id_3);
    }

    #[test]
    fn test_with_usize() {
        let party_id_1 = PartyId::from(1000);
        let party_id_2 = PartyId::from(1000);
        let party_id_3 = PartyId::from(1001);

        assert_eq!(party_id_1, party_id_2);
        assert_eq!(party_id_3.as_ref(), &1001usize.to_le_bytes());
    }

    #[test]
    fn test_with_uuid() {
        let uuid_str = "ad91b480-b32a-426d-966c-958607f185a7";
        let party_id_1 = PartyId::from(Uuid::parse_str(uuid_str).expect("Failed to parse UUID"));
        let party_id_2 = PartyId::from(Uuid::parse_str(uuid_str).expect("Failed to parse UUID"));

        assert_eq!(party_id_1, party_id_2);
        assert_eq!(
            party_id_1.as_ref(),
            <Uuid as AsRef<[u8]>>::as_ref(&Uuid::parse_str(uuid_str).expect("Failed to parse UUID"))
        );
    }
}
