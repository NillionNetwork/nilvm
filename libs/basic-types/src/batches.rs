//! An abstraction over batches of elements.

use std::ops::{Deref, DerefMut};

/// This is a wrapper over a `Vec<Vec<T>>` that contains some useful operations like constructing
/// elements from a flattened list, flattening the batches, etc.
#[derive(Clone, Debug)]
pub struct Batches<T>(Vec<Vec<T>>);

impl<T> Batches<T> {
    /// Creates a Batches with N empty batches in it.
    pub fn empty(batch_count: usize) -> Self
    where
        T: Clone,
    {
        Self(vec![Vec::default(); batch_count])
    }

    /// Constructs a Batches containing a single batch.
    pub fn single(elements: Vec<T>) -> Self {
        Self(vec![elements])
    }

    /// Constructs a Batches from a sequence of T and the size of each batch.
    pub fn from_flattened(
        elements: impl IntoIterator<Item = T>,
        batch_sizes: &[usize],
    ) -> Result<Self, NotEnoughElements> {
        let mut elements = elements.into_iter();
        let mut batches = Vec::new();
        for &batch_size in batch_sizes {
            let batch: Vec<_> = elements.by_ref().take(batch_size).collect();
            if batch.len() != batch_size {
                return Err(NotEnoughElements);
            }
            batches.push(batch);
        }
        Ok(Self(batches))
    }

    /// Constructs a Batches from a sequence of T using a fixed size for each batch.
    pub fn from_flattened_fixed(
        elements: impl IntoIterator<Item = T>,
        batch_size: usize,
    ) -> Result<Self, NotEnoughElements> {
        let mut elements = elements.into_iter();
        let mut batches = Vec::new();
        loop {
            let batch: Vec<_> = elements.by_ref().take(batch_size).collect();
            if batch.is_empty() {
                break;
            } else if batch.len() != batch_size {
                return Err(NotEnoughElements);
            }
            batches.push(batch);
        }
        Ok(Self(batches))
    }

    /// Flattens the batch into a single flat Vec.
    pub fn flatten(self) -> Vec<T> {
        self.into_iter().flatten().collect()
    }

    /// Gets the size of each batch.
    pub fn batch_sizes(&self) -> impl Iterator<Item = usize> + '_ {
        self.iter().map(Vec::len)
    }
}

// Useful traits so we can use this as if it was a Vec<Vec<T>>

impl<T> Default for Batches<T> {
    fn default() -> Self {
        Self(Vec::default())
    }
}

impl<T> Deref for Batches<T> {
    type Target = Vec<Vec<T>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Batches<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> IntoIterator for Batches<T> {
    type Item = Vec<T>;
    type IntoIter = <Vec<Vec<T>> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<T> From<Vec<Vec<T>>> for Batches<T> {
    fn from(values: Vec<Vec<T>>) -> Self {
        Batches(values)
    }
}

/// An error indicating we needed more elements than we had available.
#[derive(Debug, thiserror::Error)]
#[error("not enough elements to build batches")]
pub struct NotEnoughElements;

#[allow(clippy::indexing_slicing)]
#[cfg(test)]
mod test {
    use super::Batches;

    #[test]
    fn batch_flattening() {
        let batches = Batches(vec![vec![1, 2], vec![3, 4, 5]]);
        let flattened = batches.flatten();
        assert_eq!(flattened, &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn batch_from_flattened() {
        let batches = Batches::from_flattened(vec![1, 2, 3, 4, 5], &[2, 3]).unwrap();
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0], &[1, 2]);
        assert_eq!(batches[1], &[3, 4, 5]);
    }

    #[test]
    fn batch_from_flattened_fixed() {
        let batches = Batches::from_flattened_fixed(vec![1, 2, 3, 4, 5, 6], 2).unwrap();
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0], &[1, 2]);
        assert_eq!(batches[1], &[3, 4]);
        assert_eq!(batches[2], &[5, 6]);
    }

    #[test]
    fn batch_from_not_enough_flattened() {
        assert!(Batches::from_flattened(vec![1, 2, 3, 4, 5], &[2, 4]).is_err());
        assert!(Batches::<u32>::from_flattened(vec![], &[1]).is_err());
        assert!(Batches::from_flattened_fixed(vec![1, 2, 3, 4, 5], 3).is_err());
        assert!(Batches::from_flattened_fixed(vec![1, 2, 3], 2).is_err());
    }

    #[test]
    fn empty_batches() {
        let batches = Batches::<u32>::empty(2);
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].len(), 0);
        assert_eq!(batches[1].len(), 0);
    }
}
