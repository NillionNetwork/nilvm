//! Outputs for the PRIVATE OUTPUT EQUALITY protocol.  
use math_lib::modular::{ModularNumber, SafePrime};

/// The output of this state machine.
pub struct PrivateOutputEqualityStateOutput<T> {
    /// The output of the protocol.
    pub outputs: Vec<T>,
}

impl<T> From<PrivateOutputEqualityStateOutput<PrivateOutputEqualityShares<T>>> for Vec<ModularNumber<T>>
where
    T: SafePrime,
{
    fn from(output: PrivateOutputEqualityStateOutput<PrivateOutputEqualityShares<T>>) -> Self {
        output.outputs.into_iter().map(|o| o.equality_output).collect()
    }
}

#[derive(Debug, Clone)]
/// The output of the PRIVATE OUTPUT EQUALITY protocol.
pub struct PrivateOutputEqualityShares<T>
where
    T: SafePrime,
{
    /// PRIVATE OUTPUT EQUALITY protocol output
    pub equality_output: ModularNumber<T>,
}
