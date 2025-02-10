use std::{cmp::Ordering, fmt};

pub use prost::{EncodeError, Message};

const LOWEST_DENOMINATION_FRACTION: u64 = 1_000_000;
const PAY_FOR_RESOURCE_TX_URL: &str = "/nillion.meta.v1.MsgPayFor";

/// A payment transaction for an operation in the nillion network.
#[derive(Clone, PartialEq, Message)]
pub struct PaymentTransactionMessage {
    /// The resource/nonce being paid for.
    #[prost(bytes, tag = "1")]
    pub resource: Vec<u8>,

    /// The address of the payer.
    #[prost(string, tag = "2")]
    pub from_address: String,

    /// The amounts being paid.
    #[prost(message, repeated, tag = "3")]
    pub amounts: Vec<PaymentAmountMessage>,
}

impl PaymentTransactionMessage {
    /// Construct a new operation payment.
    pub fn new(from_address: String, amount: TokenAmount, resource: Vec<u8>) -> Self {
        Self {
            resource,
            from_address,
            amounts: vec![PaymentAmountMessage {
                denom: TokenAmount::lowest_denomination().to_string(),
                amount: amount.to_unil().to_string(),
            }],
        }
    }

    /// Build this operation into a protobuf message.
    pub fn build(&self) -> Result<SerializedTransaction, EncodeError> {
        let mut payload = Vec::new();
        self.encode(&mut payload)?;

        Ok(SerializedTransaction { type_url: PAY_FOR_RESOURCE_TX_URL.to_string(), value: payload })
    }
}

/// A serialized transaction.
#[derive(Clone, Debug)]
pub struct SerializedTransaction {
    /// The URL that identifies the type that represents this serialized transaction.
    pub type_url: String,

    /// The serialized contents.
    pub value: Vec<u8>,
}

/// A token amount.
#[derive(Clone, PartialEq, Message)]
pub struct PaymentAmountMessage {
    /// The token denomination.
    #[prost(string, tag = "1")]
    pub denom: String,

    /// The amount.
    #[prost(string, tag = "2")]
    pub amount: String,
}

/// Token amount representation
#[derive(Clone, Copy, Debug)]
pub enum TokenAmount {
    /// Standard unit, where 1 nil = 1_000_000 unil
    Nil(u64),

    /// Smallest unit of nillion
    Unil(u64),
}

impl TokenAmount {
    pub fn lowest_denomination() -> &'static str {
        "unil"
    }

    /// Converts this amount into unil.
    pub fn to_unil(&self) -> u64 {
        match self {
            TokenAmount::Nil(amount) => amount * LOWEST_DENOMINATION_FRACTION,
            TokenAmount::Unil(amount) => *amount,
        }
    }
}

impl fmt::Display for TokenAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenAmount::Nil(amount) => write!(f, "{amount} nil"),
            TokenAmount::Unil(amount) => write!(f, "{amount} unil"),
        }
    }
}

impl PartialOrd for TokenAmount {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.to_unil().cmp(&other.to_unil()))
    }
}

impl Ord for TokenAmount {
    fn cmp(&self, other: &Self) -> Ordering {
        self.to_unil().cmp(&other.to_unil())
    }
}

impl Eq for TokenAmount {}

impl PartialEq for TokenAmount {
    fn eq(&self, other: &Self) -> bool {
        self.to_unil() == other.to_unil()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display() {
        let amount_in_nil = TokenAmount::Nil(1);
        let amount_in_unil = TokenAmount::Unil(2_000_000);

        assert_eq!(format!("{}", amount_in_nil), "1 nil");
        assert_eq!(format!("{}", amount_in_unil), "2000000 unil");
    }

    #[test]
    fn test_comparisons() {
        let amount1 = TokenAmount::Nil(5);
        let amount2 = TokenAmount::Unil(5_000_000);
        let amount3 = TokenAmount::Unil(6_000_000);

        assert_eq!(amount1, amount2);
        assert!(amount1 < amount3);
        assert!(amount3 > amount2);
    }
}
