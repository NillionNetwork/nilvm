//! This module provides [PartyJar], a type that collects an item from a set of pre-defined parties.

use crate::PartyId;
use std::collections::HashMap;

/// A jar where every party puts an element.
#[derive(Default, Debug, Clone)]
pub struct PartyJar<T> {
    elements: Vec<(PartyId, T)>,
    party_count: usize,
}

impl<T> PartyJar<T> {
    /// Constructs a new jar that expects the given number of parties.
    pub fn new(party_count: usize) -> Self {
        let elements = Vec::with_capacity(party_count);
        Self { elements, party_count }
    }

    /// Constructs a new jar with a set of elements.
    pub fn new_with_elements<I>(elements: I) -> Result<Self, DuplicatePartyShare>
    where
        I: IntoIterator<Item = (PartyId, T), IntoIter: ExactSizeIterator>,
    {
        let elements = elements.into_iter();
        let mut jar = Self::new(elements.len());
        for (party_id, element) in elements {
            jar.add_element(party_id, element)?;
        }
        jar.party_count = jar.elements.len();
        Ok(jar)
    }

    /// Check whether this jar is full.
    ///
    /// A jar becomes full when every party has put their element into it.
    pub fn is_full(&self) -> bool {
        self.elements.len() == self.party_count
    }

    /// Check whether this jar is empty.
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Check how many parties we have elements for.
    pub fn stored_party_count(&self) -> usize {
        self.elements.len()
    }

    /// Add an element for a party.
    ///
    /// This returns an error if the party has already provided an element.
    pub fn add_element(&mut self, party: PartyId, element: T) -> Result<(), DuplicatePartyShare> {
        let result = self.elements.binary_search_by(|element| element.0.cmp(&party));
        match result {
            Ok(_) => Err(DuplicatePartyShare(party)),
            Err(index) => {
                self.elements.insert(index, (party, element));
                Ok(())
            }
        }
    }

    /// Consume this jar and take the elements.
    ///
    /// The returned elements *are guaranteed to be sorted by party id*.
    pub fn into_elements(self) -> impl Iterator<Item = (PartyId, T)> {
        self.elements.into_iter()
    }

    /// Take a reference to the elements in this jar.
    ///
    /// The returned elements *are guaranteed to be sorted by party id*.
    pub fn elements(&self) -> impl Iterator<Item = &(PartyId, T)> {
        self.elements.iter()
    }
}

impl<T> From<PartyJar<T>> for HashMap<PartyId, T> {
    fn from(value: PartyJar<T>) -> Self {
        value.into_elements().collect::<HashMap<PartyId, T>>()
    }
}

/// An error indicating a single party provided more than one element.
#[derive(thiserror::Error, Debug)]
#[error("party {0} already provided element")]
pub struct DuplicatePartyShare(PartyId);

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn default() {
        let jar = PartyJar::<u32>::default();
        assert!(jar.is_empty());
        assert_eq!(jar.stored_party_count(), 0);
    }

    #[test]
    fn duplicate_party() {
        let party = PartyId::from(vec![1]);
        let mut jar = PartyJar::new(2);
        assert!(jar.add_element(party.clone(), 1).is_ok());
        assert!(jar.add_element(party, 1).is_err());
    }

    #[test]
    fn duplicate_party_when_multiple_inserted() {
        let mut jar = PartyJar::new(10);
        jar.add_element(PartyId::from(vec![3]), 3).unwrap();
        jar.add_element(PartyId::from(vec![1]), 1).unwrap();
        jar.add_element(PartyId::from(vec![2]), 2).unwrap();

        jar.add_element(PartyId::from(vec![1]), 1).unwrap_err();
        jar.add_element(PartyId::from(vec![2]), 2).unwrap_err();
        jar.add_element(PartyId::from(vec![3]), 3).unwrap_err();
    }

    #[test]
    fn full() {
        let mut jar = PartyJar::new(2);
        jar.add_element(PartyId::from(vec![1]), 1).unwrap();
        assert!(!jar.is_full());

        jar.add_element(PartyId::from(vec![2]), 2).unwrap();
        assert!(jar.is_full());
    }

    #[test]
    fn retrieve_elements() {
        let parties = vec![PartyId::from(vec![0]), PartyId::from(vec![1]), PartyId::from(vec![2])];
        let mut jar = PartyJar::new(3);
        jar.add_element(parties[2].clone(), 2).unwrap();
        jar.add_element(parties[0].clone(), 0).unwrap();
        jar.add_element(parties[1].clone(), 1).unwrap();

        let elements: Vec<_> = jar.into_elements().collect();
        let expected_elements = vec![(parties[0].clone(), 0), (parties[1].clone(), 1), (parties[2].clone(), 2)];
        assert_eq!(elements, expected_elements);
    }

    #[test]
    fn new_with_elements() {
        let jar =
            PartyJar::new_with_elements([(PartyId::from(vec![0]), 0), (PartyId::from(vec![1]), 1)].iter().cloned())
                .unwrap();
        assert!(jar.is_full());
        assert_eq!(jar.into_elements().count(), 2);
    }
}
