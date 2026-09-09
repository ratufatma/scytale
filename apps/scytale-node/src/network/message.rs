use serde::{Deserialize, Serialize};

pub const BLOCKS_SUBJECT: &str = "scytale.v1.blocks.new";
pub const TRANSACTIONS_SUBJECT: &str = "scytale.v1.mempool.tx";
pub const HEARTBEAT_SUBJECT: &str = "scytale.v1.nodes.heartbeat";

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

#[cfg(test)]
mod tests {
    use super::Heartbeat;

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
}
