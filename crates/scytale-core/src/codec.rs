use crate::error::SerializationError;
use crate::transaction::{Transaction, TxIn};
use scytale_primitives::{Hash256, OutPoint, Quanta, TxOut};
use std::io::{Cursor, Read, Write};

/// Maximum allowed vector length (16 MB) to prevent out-of-memory denial-of-service on untrusted streams.
pub const MAX_VECTOR_LENGTH: usize = 16 * 1024 * 1024;

/// Trait for deterministic canonical binary serialization.
pub trait CanonicalSerialize {
    /// Serializes the data structure to a writer using canonical deterministic encoding.
    fn serialize_canonical<W: Write>(&self, writer: &mut W) -> Result<(), SerializationError>;

    /// Serializes the data structure to a canonical byte vector.
    fn to_canonical_bytes(&self) -> Result<Vec<u8>, SerializationError> {
        let mut buf = Vec::new();
        self.serialize_canonical(&mut buf)?;
        Ok(buf)
    }
}

/// Trait for deterministic canonical binary deserialization with fail-closed bounds checking.
pub trait CanonicalDeserialize: Sized {
    /// Deserializes a data structure from a reader.
    fn deserialize_canonical<R: Read>(reader: &mut R) -> Result<Self, SerializationError>;

    /// Deserializes a data structure from a canonical byte slice, verifying zero trailing unparsed bytes.
    fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, SerializationError> {
        let mut cursor = Cursor::new(bytes);
        let obj = Self::deserialize_canonical(&mut cursor)?;
        let pos = cursor.position() as usize;
        if pos < bytes.len() {
            return Err(SerializationError::TrailingBytes(bytes.len() - pos));
        }
        Ok(obj)
    }
}

// ---------------------------------------------------------------------------
// Primitive Implementations
// ---------------------------------------------------------------------------

impl CanonicalSerialize for u8 {
    fn serialize_canonical<W: Write>(&self, writer: &mut W) -> Result<(), SerializationError> {
        writer.write_all(&[*self])?;
        Ok(())
    }
}

impl CanonicalDeserialize for u8 {
    fn deserialize_canonical<R: Read>(reader: &mut R) -> Result<Self, SerializationError> {
        let mut buf = [0u8; 1];
        reader.read_exact(&mut buf)?;
        Ok(buf[0])
    }
}

impl CanonicalSerialize for u16 {
    fn serialize_canonical<W: Write>(&self, writer: &mut W) -> Result<(), SerializationError> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }
}

impl CanonicalDeserialize for u16 {
    fn deserialize_canonical<R: Read>(reader: &mut R) -> Result<Self, SerializationError> {
        let mut buf = [0u8; 2];
        reader.read_exact(&mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }
}

impl CanonicalSerialize for u32 {
    fn serialize_canonical<W: Write>(&self, writer: &mut W) -> Result<(), SerializationError> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }
}

impl CanonicalDeserialize for u32 {
    fn deserialize_canonical<R: Read>(reader: &mut R) -> Result<Self, SerializationError> {
        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }
}

impl CanonicalSerialize for u64 {
    fn serialize_canonical<W: Write>(&self, writer: &mut W) -> Result<(), SerializationError> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }
}

impl CanonicalDeserialize for u64 {
    fn deserialize_canonical<R: Read>(reader: &mut R) -> Result<Self, SerializationError> {
        let mut buf = [0u8; 8];
        reader.read_exact(&mut buf)?;
        Ok(u64::from_le_bytes(buf))
    }
}

impl CanonicalSerialize for bool {
    fn serialize_canonical<W: Write>(&self, writer: &mut W) -> Result<(), SerializationError> {
        let b = if *self { 1u8 } else { 0u8 };
        writer.write_all(&[b])?;
        Ok(())
    }
}

impl CanonicalDeserialize for bool {
    fn deserialize_canonical<R: Read>(reader: &mut R) -> Result<Self, SerializationError> {
        let mut buf = [0u8; 1];
        reader.read_exact(&mut buf)?;
        match buf[0] {
            0x00 => Ok(false),
            0x01 => Ok(true),
            _ => Err(SerializationError::InvalidEncoding),
        }
    }
}

impl CanonicalSerialize for Hash256 {
    fn serialize_canonical<W: Write>(&self, writer: &mut W) -> Result<(), SerializationError> {
        writer.write_all(self.as_bytes())?;
        Ok(())
    }
}

impl CanonicalDeserialize for Hash256 {
    fn deserialize_canonical<R: Read>(reader: &mut R) -> Result<Self, SerializationError> {
        let mut buf = [0u8; 32];
        reader.read_exact(&mut buf)?;
        Ok(Hash256::new(buf))
    }
}

impl CanonicalSerialize for Vec<u8> {
    fn serialize_canonical<W: Write>(&self, writer: &mut W) -> Result<(), SerializationError> {
        if self.len() > MAX_VECTOR_LENGTH {
            return Err(SerializationError::LengthExceedsLimit {
                length: self.len(),
                max: MAX_VECTOR_LENGTH,
            });
        }
        (self.len() as u32).serialize_canonical(writer)?;
        writer.write_all(self)?;
        Ok(())
    }
}

impl CanonicalDeserialize for Vec<u8> {
    fn deserialize_canonical<R: Read>(reader: &mut R) -> Result<Self, SerializationError> {
        let len = u32::deserialize_canonical(reader)? as usize;
        if len > MAX_VECTOR_LENGTH {
            return Err(SerializationError::LengthExceedsLimit {
                length: len,
                max: MAX_VECTOR_LENGTH,
            });
        }
        let mut buf = vec![0u8; len];
        reader.read_exact(&mut buf)?;
        Ok(buf)
    }
}

// ---------------------------------------------------------------------------
// Protocol Structures Implementations
// ---------------------------------------------------------------------------

impl CanonicalSerialize for OutPoint {
    fn serialize_canonical<W: Write>(&self, writer: &mut W) -> Result<(), SerializationError> {
        self.txid.serialize_canonical(writer)?;
        self.index.serialize_canonical(writer)?;
        Ok(())
    }
}

impl CanonicalDeserialize for OutPoint {
    fn deserialize_canonical<R: Read>(reader: &mut R) -> Result<Self, SerializationError> {
        let txid = Hash256::deserialize_canonical(reader)?;
        let index = u32::deserialize_canonical(reader)?;
        Ok(OutPoint::new(txid, index))
    }
}

impl CanonicalSerialize for TxIn {
    fn serialize_canonical<W: Write>(&self, writer: &mut W) -> Result<(), SerializationError> {
        self.previous_output.serialize_canonical(writer)?;
        self.authorization.serialize_canonical(writer)?;
        Ok(())
    }
}

impl CanonicalDeserialize for TxIn {
    fn deserialize_canonical<R: Read>(reader: &mut R) -> Result<Self, SerializationError> {
        let previous_output = OutPoint::deserialize_canonical(reader)?;
        let authorization = Vec::<u8>::deserialize_canonical(reader)?;
        Ok(TxIn::new(previous_output, authorization))
    }
}

impl CanonicalSerialize for TxOut {
    fn serialize_canonical<W: Write>(&self, writer: &mut W) -> Result<(), SerializationError> {
        self.value.serialize_canonical(writer)?;
        self.locking_condition.serialize_canonical(writer)?;
        Ok(())
    }
}

impl CanonicalDeserialize for TxOut {
    fn deserialize_canonical<R: Read>(reader: &mut R) -> Result<Self, SerializationError> {
        let value = Quanta::deserialize_canonical(reader)?;
        let locking_condition = Vec::<u8>::deserialize_canonical(reader)?;
        Ok(TxOut::new(value, locking_condition))
    }
}

impl CanonicalSerialize for Transaction {
    fn serialize_canonical<W: Write>(&self, writer: &mut W) -> Result<(), SerializationError> {
        self.version.serialize_canonical(writer)?;
        (self.inputs.len() as u32).serialize_canonical(writer)?;
        for input in &self.inputs {
            input.serialize_canonical(writer)?;
        }
        (self.outputs.len() as u32).serialize_canonical(writer)?;
        for output in &self.outputs {
            output.serialize_canonical(writer)?;
        }
        self.lock_time.serialize_canonical(writer)?;
        Ok(())
    }
}

impl CanonicalDeserialize for Transaction {
    fn deserialize_canonical<R: Read>(reader: &mut R) -> Result<Self, SerializationError> {
        let version = u32::deserialize_canonical(reader)?;
        let input_count = u32::deserialize_canonical(reader)? as usize;
        if input_count > MAX_VECTOR_LENGTH {
            return Err(SerializationError::LengthExceedsLimit {
                length: input_count,
                max: MAX_VECTOR_LENGTH,
            });
        }
        let mut inputs = Vec::with_capacity(input_count.min(1024));
        for _ in 0..input_count {
            inputs.push(TxIn::deserialize_canonical(reader)?);
        }

        let output_count = u32::deserialize_canonical(reader)? as usize;
        if output_count > MAX_VECTOR_LENGTH {
            return Err(SerializationError::LengthExceedsLimit {
                length: output_count,
                max: MAX_VECTOR_LENGTH,
            });
        }
        let mut outputs = Vec::with_capacity(output_count.min(1024));
        for _ in 0..output_count {
            outputs.push(TxOut::deserialize_canonical(reader)?);
        }

        let lock_time = u64::deserialize_canonical(reader)?;
        Ok(Transaction::new(version, inputs, outputs, lock_time))
    }
}
