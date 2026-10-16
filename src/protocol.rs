//! Protocol definition for communication between `s` client and server.

pub const MSG_DATA: u8 = 0x01;
pub const MSG_RESIZE: u8 = 0x02;
pub const MSG_DETACH: u8 = 0x03;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ClientMessage {
    Data(Vec<u8>),
    Resize { rows: u16, cols: u16 },
    Detach,
}

impl ClientMessage {
    /// Encode the message into a byte buffer to send over the Unix Domain Socket.
    #[allow(dead_code)]
    pub fn encode(&self) -> Vec<u8> {
        match self {
            ClientMessage::Data(bytes) => {
                let mut buf = Vec::with_capacity(3 + bytes.len());
                buf.push(MSG_DATA);
                let len = bytes.len().min(u16::MAX as usize) as u16;
                buf.extend_from_slice(&len.to_be_bytes());
                buf.extend_from_slice(&bytes[..len as usize]);
                buf
            }
            ClientMessage::Resize { rows, cols } => {
                let mut buf = Vec::with_capacity(5);
                buf.push(MSG_RESIZE);
                buf.extend_from_slice(&rows.to_be_bytes());
                buf.extend_from_slice(&cols.to_be_bytes());
                buf
            }
            ClientMessage::Detach => vec![MSG_DETACH],
        }
    }

    /// Try to decode a message from bytes (useful for testing and parsing).
    #[allow(dead_code)]
    pub fn decode(bytes: &[u8]) -> Result<Option<(Self, usize)>, &'static str> {
        if bytes.is_empty() {
            return Ok(None);
        }

        match bytes[0] {
            MSG_DATA => {
                if bytes.len() < 3 {
                    return Ok(None);
                }
                let len = u16::from_be_bytes([bytes[1], bytes[2]]) as usize;
                if bytes.len() < 3 + len {
                    return Ok(None);
                }
                let data = bytes[3..3 + len].to_vec();
                Ok(Some((ClientMessage::Data(data), 3 + len)))
            }
            MSG_RESIZE => {
                if bytes.len() < 5 {
                    return Ok(None);
                }
                let rows = u16::from_be_bytes([bytes[1], bytes[2]]);
                let cols = u16::from_be_bytes([bytes[3], bytes[4]]);
                Ok(Some((ClientMessage::Resize { rows, cols }, 5)))
            }
            MSG_DETACH => Ok(Some((ClientMessage::Detach, 1))),
            _ => Err("Invalid message header byte"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_data() {
        let original = ClientMessage::Data(b"hello world\n".to_vec());
        let encoded = original.encode();
        let (decoded, consumed) = ClientMessage::decode(&encoded).unwrap().unwrap();
        assert_eq!(decoded, original);
        assert_eq!(consumed, encoded.len());
    }

    #[test]
    fn test_encode_decode_resize() {
        let original = ClientMessage::Resize { rows: 40, cols: 120 };
        let encoded = original.encode();
        let (decoded, consumed) = ClientMessage::decode(&encoded).unwrap().unwrap();
        assert_eq!(decoded, original);
        assert_eq!(consumed, 5);
    }

    #[test]
    fn test_encode_decode_detach() {
        let original = ClientMessage::Detach;
        let encoded = original.encode();
        let (decoded, consumed) = ClientMessage::decode(&encoded).unwrap().unwrap();
        assert_eq!(decoded, original);
        assert_eq!(consumed, 1);
    }
}
