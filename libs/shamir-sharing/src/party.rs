//! Party identifiers.

pub use basic_types::PartyId;
use bimap::BiBTreeMap;
use math_lib::fields::Field;

/// A type that deterministically maps a party id to an abscissa and vice-versa, given all of the participants
/// are known at construction time.
#[derive(Clone)]
pub struct PartyMapper<F: Field> {
    party_abscissa: BiBTreeMap<PartyId, F::Inner>,
}

impl<F: Field> PartyMapper<F> {
    /// Constructs a mapper for the given parties.
    pub fn new(parties: Vec<PartyId>) -> Result<Self, TooManyParties> {
        let mut parties = parties;
        parties.sort();

        let inner_count = (parties.len() as u32).checked_add(1).ok_or(TooManyParties)?;
        let inner_values = F::inner_elements(inner_count).map_err(|_| TooManyParties)?;
        let mut party_abscissa = BiBTreeMap::new();
        // Skip the first element as that's abscissa 0 and we don't want it.
        for (party, inner) in parties.into_iter().zip(inner_values.into_iter().skip(1)) {
            party_abscissa.insert(party, inner);
        }
        Ok(PartyMapper { party_abscissa })
    }

    /// Gets the abscissa for a party.
    pub fn abscissa(&self, party_id: &PartyId) -> Option<&F::Inner> {
        self.party_abscissa.get_by_left(party_id)
    }

    /// Gets the party for an abscissa.
    pub fn party(&self, abscissa: &F::Inner) -> Option<&PartyId> {
        self.party_abscissa.get_by_right(abscissa)
    }

    /// Gets all the party ids. Elements are guaranteed to be sorted in ascending order.
    pub fn parties(&self) -> impl Iterator<Item = &PartyId> {
        self.party_abscissa.left_values()
    }

    /// Gets all the abscissas. Elements are guaranteed to be sorted in ascending order.
    pub fn abscissas(&self) -> impl Iterator<Item = &F::Inner> {
        self.party_abscissa.right_values()
    }

    /// Get the total number of parties.
    pub fn party_count(&self) -> usize {
        self.party_abscissa.len()
    }
}

/// Too many parties were provided during the mapper initialization.
#[derive(Debug, thiserror::Error)]
#[error("too many parties")]
pub struct TooManyParties;

#[cfg(test)]
mod tests {
    use math_lib::fields::BinaryExtField;

    use super::*;

    type Field = BinaryExtField;

    #[test]
    fn consistent_mapping() {
        let mapper =
            PartyMapper::<Field>::new(vec![PartyId::from(42), PartyId::from(1337), PartyId::from(13)]).unwrap();

        assert_eq!(mapper.abscissa(&PartyId::from(13)), Some(&1));
        assert_eq!(mapper.abscissa(&PartyId::from(42)), Some(&2));
        assert_eq!(mapper.abscissa(&PartyId::from(1337)), Some(&3));

        assert_eq!(mapper.party(&1), Some(&PartyId::from(13)));
        assert_eq!(mapper.party(&2), Some(&PartyId::from(42)));
        assert_eq!(mapper.party(&3), Some(&PartyId::from(1337)));
    }

    #[test]
    fn wrap_around_detected() {
        let sample = PartyId::from(13);

        // The boundary edge is fine.
        assert!(PartyMapper::<Field>::new(vec![sample.clone(); 254]).is_ok());

        // One past it is not.
        assert!(PartyMapper::<Field>::new(vec![sample.clone(); 255]).is_err());
    }

    #[test]
    fn element_iteration() {
        let mapper =
            PartyMapper::<Field>::new(vec![PartyId::from(42), PartyId::from(1337), PartyId::from(13)]).unwrap();

        assert_eq!(
            mapper.parties().collect::<Vec<_>>(),
            vec![&PartyId::from(13), &PartyId::from(42), &PartyId::from(1337)]
        );
        assert_eq!(mapper.abscissas().collect::<Vec<_>>(), vec![&1, &2, &3]);
    }
}
