use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_SYNC_ITEMS: u32 = 2_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    pub protocol_version: u32,
    pub network_id: u32,
    pub genesis_hash: [u8; 32],
    pub node_id: String,
    pub best_height: u64,
    pub best_hash: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocatorRequest {
    pub request_id: u64,
    pub hello: Hello,
    pub locator: Vec<[u8; 32]>,
    pub stop_hash: Option<[u8; 32]>,
    pub max_items: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockRequest {
    pub request_id: u64,
    pub requester_id: String,
    pub hashes: Vec<[u8; 32]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeadersResponse {
    pub request_id: u64,
    pub hashes: Vec<[u8; 32]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlocksResponse {
    pub request_id: u64,
    pub blocks: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Heartbeat {
    pub node_id: String,
    pub height: u64,
    pub best_hash: [u8; 32],
}

impl Heartbeat {
    pub fn encode(&self) -> Result<Vec<u8>, bincode::Error> {
        bincode::serialize(self)
    }
}

pub fn encode<T: Serialize>(message: &T) -> Result<Vec<u8>, bincode::Error> {
    bincode::serialize(message)
}

pub fn decode<T: for<'de> Deserialize<'de>>(payload: &[u8]) -> Result<T, bincode::Error> {
    bincode::deserialize(payload)
}

#[cfg(test)]
mod tests {
    use super::{decode, encode, Heartbeat, Hello, LocatorRequest, PROTOCOL_VERSION};

    #[test]
    fn heartbeat_round_trips_with_bincode() {
        let heartbeat = Heartbeat {
            node_id: "node-a".to_owned(),
            height: 42,
            best_hash: [7; 32],
        };

        let encoded = bincode::serialize(&heartbeat).expect("heartbeat should encode");
        let decoded: Heartbeat = bincode::deserialize(&encoded).expect("heartbeat should decode");

        assert_eq!(decoded, heartbeat);
    }

    #[test]
    fn sync_request_round_trips_with_bincode() {
        let message = LocatorRequest {
            request_id: 7,
            hello: Hello {
                protocol_version: PROTOCOL_VERSION,
                network_id: 42,
                genesis_hash: [1; 32],
                node_id: "node-a".to_owned(),
                best_height: 8,
                best_hash: [2; 32],
            },
            locator: vec![[3; 32]],
            stop_hash: None,
            max_items: 100,
        };

        let decoded: LocatorRequest = decode(&encode(&message).expect("encode")).expect("decode");
        assert_eq!(decoded, message);
    }
}
