//! Named entities.

/// A named entity.
pub struct Named<T> {
    /// The entity's name.
    pub name: String,

    /// The entity's value.
    pub value: T,
}
