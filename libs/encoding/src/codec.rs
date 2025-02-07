//! Message encoding.

use bincode::Options;

/// A codec for wire messages.
///
/// This codec is meant to allow different encodings. Each message is prefixed with a single byte
/// that indicates the message's encoding, followed by the message contents.
///
/// This currently only supports bincode encoding but more formats can be added in the future.
#[derive(Clone, Default)]
pub struct MessageCodec;

impl MessageCodec {
    /// Options for bincode.
    pub fn bincode_options() -> impl bincode::Options {
        #[allow(clippy::arithmetic_side_effects)]
        bincode::options()
            // 16MB max message size. This can be shrunk in the future, this is a large max just-in-case.
            .with_limit(16 * 1024 * 1024)
            // Allow trailing bytes so the sender can be more up to date in the protocol structure
            // than us.
            .allow_trailing_bytes()
            // Varint encoding so messages are smaller.
            .with_varint_encoding()
            // Little endian because that's what we likely use anyway.
            .with_little_endian()
    }
}

impl MessageCodec {
    /// Encode a transport message into a byte sequence.
    pub fn encode<T>(&self, message: &T) -> Result<Vec<u8>, EncodeError>
    where
        T: serde::Serialize,
    {
        let mut bytes = vec![Encoding::Bincode as u8];
        Self::bincode_options().serialize_into(&mut bytes, message)?;
        Ok(bytes)
    }

    /// Compute the total encoded size for the given message.
    pub fn encoded_size<T>(&self, message: &T) -> Result<u64, EncodeError>
    where
        T: serde::Serialize,
    {
        Ok(Self::bincode_options().serialized_size(message)?)
    }

    /// Decode a transport message from a byte sequence.
    pub fn decode<T>(&self, data: &[u8]) -> Result<T, DecodeError>
    where
        T: serde::de::DeserializeOwned,
    {
        let (encoding, data) =
            data.split_first().ok_or_else(|| Box::new(bincode::ErrorKind::Custom("empty input".to_string())))?;
        if *encoding == Encoding::Bincode as u8 {
            Ok(Self::bincode_options().deserialize(data)?)
        } else {
            Err(Box::new(bincode::ErrorKind::Custom(format!("unknown encoding: {encoding}"))).into())
        }
    }
}

#[repr(u8)]
enum Encoding {
    Bincode = 0,
}

/// An error during message encoding.
#[derive(Debug, thiserror::Error)]
#[error("encoding: {0}")]
pub struct EncodeError(#[from] Box<bincode::ErrorKind>);

/// An error during message decoding.
#[derive(Debug, thiserror::Error)]
#[error("decoding: {0}")]
pub struct DecodeError(#[from] Box<bincode::ErrorKind>);

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn encode_decode() {
        let message = "hello".to_string();
        let codec = MessageCodec::default();
        let encoded = codec.encode(&message).expect("encoding failed");
        assert_eq!(encoded[0], Encoding::Bincode as u8);

        let decoded = codec.decode::<String>(&encoded).expect("decoding failed");
        assert_eq!(decoded, message);
    }

    #[test]
    fn invalid_encoding() {
        let codec = MessageCodec::default();
        let data = [Encoding::Bincode as u8 + 1, 42];
        let result = codec.decode::<u8>(&data);
        assert!(result.is_err());
    }
}
