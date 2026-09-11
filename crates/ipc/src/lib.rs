#![forbid(unsafe_code)]

pub const IPC_PROTOCOL_MAJOR: u32 = 1;

pub const MAX_ENVELOPE_BYTES: usize = 1 << 20;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IpcError {
    TooLarge,
    Decode,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Envelope {
    #[prost(uint32, tag = "1")]
    pub protocol_major: u32,
    #[prost(string, tag = "2")]
    pub request_id: String,
    #[prost(bytes = "vec", tag = "3")]
    pub payload: Vec<u8>,
    #[prost(string, tag = "4")]
    pub auth_token: String,
}

/// Encodes an envelope while enforcing the 1 MiB payload boundary.
///
/// # Errors
///
/// Returns `TooLarge` when the encoded envelope exceeds the boundary.
pub fn encode(envelope: &Envelope) -> Result<Vec<u8>, IpcError> {
    let encoded = prost::Message::encode_to_vec(envelope);
    if encoded.len() > MAX_ENVELOPE_BYTES {
        return Err(IpcError::TooLarge);
    }
    Ok(encoded)
}

/// Decodes an envelope and ignores forward-compatible unknown fields.
///
/// # Errors
///
/// Returns `TooLarge` for oversized input or `Decode` for malformed protobuf.
pub fn decode(bytes: &[u8]) -> Result<Envelope, IpcError> {
    if bytes.len() > MAX_ENVELOPE_BYTES {
        return Err(IpcError::TooLarge);
    }
    prost::Message::decode(bytes).map_err(|_| IpcError::Decode)
}

#[cfg(test)]
mod tests {
    use super::{Envelope, IpcError, MAX_ENVELOPE_BYTES, decode};
    use prost::Message;

    #[test]
    fn envelope_roundtrips_with_protobuf() {
        let original = Envelope {
            protocol_major: 1,
            request_id: "req-1".into(),
            payload: b"ping".to_vec(),
            auth_token: "token".into(),
        };
        let encoded = original.encode_to_vec();
        let decoded = Envelope::decode(encoded.as_slice()).expect("valid envelope");
        assert_eq!(decoded, original);
    }

    #[test]
    fn unknown_fields_are_ignored_and_oversized_frames_are_rejected() {
        let original = Envelope {
            protocol_major: 1,
            request_id: "req-2".into(),
            payload: b"ok".to_vec(),
            auth_token: "token".into(),
        };
        let mut encoded = original.encode_to_vec();
        encoded.extend([0x28, 0x01]);
        assert_eq!(
            decode(&encoded).expect("unknown fields are compatible"),
            original
        );
        assert_eq!(
            decode(&vec![0; MAX_ENVELOPE_BYTES + 1]),
            Err(IpcError::TooLarge)
        );
    }
}
