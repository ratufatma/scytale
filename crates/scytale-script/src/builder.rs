//! Ergonomic builder for constructing ScytaleScript byte sequences.

use crate::opcode::OpCode;

/// Fluent builder for constructing valid ScytaleScript byte vectors.
#[derive(Debug, Clone, Default)]
pub struct ScriptBuilder {
    bytes: Vec<u8>,
}

impl ScriptBuilder {
    /// Creates a new, empty ScriptBuilder.
    pub fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    /// Appends a raw opcode byte to the script.
    pub fn push_opcode(mut self, opcode: OpCode) -> Self {
        self.bytes.push(opcode.as_byte());
        self
    }

    /// Appends a push instruction for a small integer 1..=16, or 0.
    pub fn push_int(self, val: i64) -> Self {
        match val {
            0 => self.push_opcode(OpCode::Op0),
            1..=16 => {
                let op_byte = OpCode::OP_1 + (val - 1) as u8;
                let mut s = self;
                s.bytes.push(op_byte);
                s
            }
            _ => self.push_data(&val.to_le_bytes()),
        }
    }

    /// Appends an appropriate push instruction (`OP_PUSHBYTES_N` or `OP_PUSHDATA1`)
    /// followed by the data bytes.
    pub fn push_data(mut self, data: &[u8]) -> Self {
        let len = data.len();
        if len == 0 {
            self.bytes.push(OpCode::OP_0);
        } else if len <= 75 {
            self.bytes.push(len as u8);
            self.bytes.extend_from_slice(data);
        } else if len <= 255 {
            self.bytes.push(OpCode::OP_PUSHDATA1);
            self.bytes.push(len as u8);
            self.bytes.extend_from_slice(data);
        } else {
            // Cap to max single item size 520
            self.bytes.push(OpCode::OP_PUSHDATA1);
            self.bytes.push(255);
            self.bytes.extend_from_slice(&data[..255]);
        }
        self
    }

    /// Appends raw bytes directly without any push instruction header.
    pub fn push_raw(mut self, raw: &[u8]) -> Self {
        self.bytes.extend_from_slice(raw);
        self
    }

    /// Builds and returns the completed script byte vector.
    pub fn build(self) -> Vec<u8> {
        self.bytes
    }
}
