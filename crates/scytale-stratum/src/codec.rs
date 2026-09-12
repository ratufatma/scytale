use crate::error::StratumError;
use crate::protocol::StratumRpcMessage;
use tokio_util::bytes::BytesMut;
use tokio_util::codec::{Decoder, Encoder};

// Re-export the standard LinesCodec for direct compatibility
pub use tokio_util::codec::LinesCodec;

/// Maximum line length for Stratum framing to prevent memory exhaustion (64 KiB).
pub const MAX_FRAME_LENGTH: usize = 65536;

/// High-level framed codec for converting newline-delimited byte streams into typed `StratumRpcMessage`s.
#[derive(Debug, Clone)]
pub struct StratumCodec {
    inner: LinesCodec,
}

impl Default for StratumCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl StratumCodec {
    pub fn new() -> Self {
        Self {
            inner: LinesCodec::new_with_max_length(MAX_FRAME_LENGTH),
        }
    }
}

impl Decoder for StratumCodec {
    type Item = StratumRpcMessage;
    type Error = StratumError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        match self.inner.decode(src)? {
            Some(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    return Ok(None);
                }
                let msg: StratumRpcMessage = serde_json::from_str(trimmed)
                    .map_err(|e| StratumError::Serialization(e.to_string()))?;
                Ok(Some(msg))
            }
            None => Ok(None),
        }
    }
}

impl Encoder<StratumRpcMessage> for StratumCodec {
    type Error = StratumError;

    fn encode(&mut self, item: StratumRpcMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let serialized = serde_json::to_string(&item)
            .map_err(|e| StratumError::Serialization(e.to_string()))?;
        self.inner.encode(serialized, dst)?;
        Ok(())
    }
}
