use crate::error::{AuthorizationError, TransactionError};
use scytale_primitives::{Hash256, OutPoint, TxOut};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Protocol default transaction version.
pub const TRANSACTION_VERSION_1: u32 = 1;

/// TxIn: References an unspent output and provides cryptographic authorization.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TxIn {
    pub previous_output: OutPoint,
    pub authorization: Vec<u8>,
}

impl TxIn {
    pub fn new(previous_output: OutPoint, authorization: Vec<u8>) -> Self {
        Self {
            previous_output,
            authorization,
        }
    }

    /// Creates a placeholder coinbase input.
    pub fn coinbase() -> Self {
        Self {
            previous_output: OutPoint::null(),
            authorization: Vec::new(),
        }
    }
}

/// Transaction: Represents an atomic state transition in Scytale.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transaction {
    pub version: u32,
    pub inputs: Vec<TxIn>,
    pub outputs: Vec<TxOut>,
    pub lock_time: u64,
}

impl Transaction {
    pub fn new(version: u32, inputs: Vec<TxIn>, outputs: Vec<TxOut>, lock_time: u64) -> Self {
        Self {
            version,
            inputs,
            outputs,
            lock_time,
        }
    }

    pub fn new_simple(version: u32, inputs: Vec<TxIn>, outputs: Vec<TxOut>) -> Self {
        Self::new(version, inputs, outputs, 0)
    }

    /// Determines whether the transaction is a coinbase issuance transaction
    /// (contains exactly 1 input whose previous_output is OutPoint::null()).
    pub fn is_coinbase(&self) -> bool {
        self.inputs.len() == 1 && self.inputs[0].previous_output.is_null()
    }

    /// Helper constructor to create a canonical coinbase transaction.
    pub fn new_coinbase(height: u64, outputs: Vec<TxOut>) -> Self {
        let input = TxIn::new(OutPoint::null(), height.to_le_bytes().to_vec());
        Self::new(TRANSACTION_VERSION_1, vec![input], outputs, 0)
    }

    /// Computes the unique 32-byte BLAKE3 transaction identifier (TxID).
    pub fn txid(&self) -> Hash256 {
        let bytes = crate::codec::CanonicalSerialize::to_canonical_bytes(self)
            .expect("canonical transaction serialization must not fail");
        Hash256::hash(&bytes)
    }

    /// Computes the raw 32-byte BLAKE3 transaction hash digest.
    pub fn compute_hash(&self) -> [u8; 32] {
        *self.txid().as_bytes()
    }

    /// Helper constructor to instantiate a transaction from eUTXO input/output models.
    pub fn from_eutxo(
        version: u32,
        inputs: &[TxInput],
        outputs: &[TxOutput],
        lock_time: u64,
    ) -> Self {
        Self {
            version,
            inputs: inputs.iter().map(|i| i.to_tx_in()).collect(),
            outputs: outputs.iter().map(|o| o.to_tx_out()).collect(),
            lock_time,
        }
    }

    /// Computes the 32-byte BLAKE3 sighash digest for an input being spent.
    ///
    /// Binds:
    /// - SCYTALE_SIGHASH_V1 domain tag
    /// - All inputs (txid + index)
    /// - All outputs (value + locking_condition)
    /// - Current input index (u32 LE)
    /// - Referenced prev_locking_script
    pub fn compute_sighash(&self, input_index: usize, prev_locking_script: &[u8]) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"SCYTALE_SIGHASH_V1");
        for input in &self.inputs {
            hasher.update(input.previous_output.txid.as_bytes());
            hasher.update(&input.previous_output.index.to_le_bytes());
        }
        for output in &self.outputs {
            hasher.update(&output.value.to_le_bytes());
            hasher.update(&output.locking_condition);
        }
        hasher.update(&(input_index as u32).to_le_bytes());
        hasher.update(prev_locking_script);
        *hasher.finalize().as_bytes()
    }

    /// Computes the exact canonical binary serialized size in bytes.
    pub fn serialized_size(&self) -> usize {
        crate::codec::CanonicalSerialize::to_canonical_bytes(self)
            .map(|b| b.len())
            .unwrap_or(0)
    }

    /// Computes the 32-byte BLAKE3 preimage digest for signing/verifying a specific input index.
    ///
    /// The canonical preimage binds:
    /// 1. Transaction version (`u32` little-endian)
    /// 2. Total inputs count (`u32` little-endian)
    /// 3. All previous output references (each `OutPoint` canonical 36 bytes)
    /// 4. Current input index being authorized (`u32` little-endian)
    /// 5. Total outputs count (`u32` little-endian)
    /// 6. All transaction outputs (canonical bytes: value + locking_condition)
    /// 7. Lock time (`u64` little-endian)
    pub fn signature_preimage_digest(
        &self,
        input_index: usize,
    ) -> Result<Hash256, AuthorizationError> {
        if input_index >= self.inputs.len() {
            return Err(AuthorizationError::InvalidInputIndex {
                index: input_index,
                total_inputs: self.inputs.len(),
            });
        }

        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.version.to_le_bytes());

        bytes.extend_from_slice(&(self.inputs.len() as u32).to_le_bytes());
        for input in &self.inputs {
            bytes.extend_from_slice(&input.previous_output.to_fixed_bytes());
        }

        bytes.extend_from_slice(&(input_index as u32).to_le_bytes());

        bytes.extend_from_slice(&(self.outputs.len() as u32).to_le_bytes());
        for output in &self.outputs {
            let out_bytes = crate::codec::CanonicalSerialize::to_canonical_bytes(output)
                .map_err(|e| AuthorizationError::PreimageSerializationFailure(e.to_string()))?;
            bytes.extend_from_slice(&out_bytes);
        }

        bytes.extend_from_slice(&self.lock_time.to_le_bytes());

        Ok(Hash256::hash(&bytes))
    }

    /// Computes the sum of all transaction outputs in quanta, checking for integer overflow.
    pub fn total_output_quanta(&self) -> Result<u64, TransactionError> {
        let mut total: u64 = 0;
        for output in &self.outputs {
            total = total
                .checked_add(output.value)
                .ok_or(TransactionError::OutputValueOverflow)?;
        }
        Ok(total)
    }

    /// Performs stateless transaction-local validation without querying external ledger state.
    pub fn validate_stateless(&self) -> Result<(), TransactionError> {
        if self.version != TRANSACTION_VERSION_1 {
            return Err(TransactionError::InvalidVersion(self.version));
        }
        if self.inputs.is_empty() {
            return Err(TransactionError::EmptyInputs);
        }
        if self.outputs.is_empty() {
            return Err(TransactionError::EmptyOutputs);
        }

        for output in &self.outputs {
            // Standard outputs must have value > 0; OP_RETURN (0x6a) outputs may have 0 value
            if output.value == 0 && output.locking_condition.first() != Some(&0x6a) {
                return Err(TransactionError::ZeroOutputValue);
            }
        }

        // Verify total output does not overflow u64::MAX
        let _ = self.total_output_quanta()?;

        // Verify absence of duplicate inputs within the same transaction
        let mut seen = HashSet::with_capacity(self.inputs.len());
        for input in &self.inputs {
            if !seen.insert(input.previous_output) {
                return Err(TransactionError::DuplicateInput(input.previous_output));
            }
        }

        Ok(())
    }
}

/// Calculates the transaction fee from total input quanta and total output quanta.
pub fn calculate_fee(
    total_input_quanta: u64,
    total_output_quanta: u64,
) -> Result<u64, TransactionError> {
    if total_output_quanta > total_input_quanta {
        return Err(TransactionError::InputValueDeficit {
            total_in: total_input_quanta,
            total_out: total_output_quanta,
        });
    }
    total_input_quanta
        .checked_sub(total_output_quanta)
        .ok_or(TransactionError::ArithmeticOverflow)
}

/// Represents the spending condition of an eUTXO output.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OutputLock {
    PublicKey([u8; 32]),
    Script {
        script_hash: [u8; 32],
        datum: Vec<u8>,
    },
}

impl OutputLock {
    pub const MAGIC_PREFIX: [u8; 4] = [0x53, 0x43, 0x59, 0x01]; // "SCY\x01"

    pub fn to_locking_condition(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(64);
        bytes.extend_from_slice(&Self::MAGIC_PREFIX);
        if let Ok(data) = bincode::serialize(self) {
            bytes.extend_from_slice(&data);
        }
        bytes
    }

    pub fn from_locking_condition(bytes: &[u8]) -> Option<Self> {
        if bytes.len() >= 4 && bytes[..4] == Self::MAGIC_PREFIX {
            bincode::deserialize(&bytes[4..]).ok()
        } else {
            bincode::deserialize(bytes).ok()
        }
    }
}

/// High-level eUTXO output specification.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxOutput {
    pub value: u64,
    pub lock: OutputLock,
}

impl TxOutput {
    pub fn new(value: u64, lock: OutputLock) -> Self {
        Self { value, lock }
    }

    pub fn to_tx_out(&self) -> TxOut {
        TxOut::new(self.value, self.lock.to_locking_condition())
    }

    pub fn from_tx_out(tx_out: &TxOut) -> Option<Self> {
        let lock = OutputLock::from_locking_condition(&tx_out.locking_condition)?;
        Some(Self {
            value: tx_out.value,
            lock,
        })
    }
}

/// High-level eUTXO input specification with optional signature, redeemer, and wasm bytecode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxInput {
    pub prev_tx_hash: [u8; 32],
    pub output_index: u32,
    /// Ed25519 signature bytes (64 bytes), stored as Vec<u8> for serde compatibility.
    pub signature: Option<Vec<u8>>,
    pub redeemer: Option<Vec<u8>>,
    pub script_source: Option<Vec<u8>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EutxoWitness {
    pub signature: Option<Vec<u8>>,
    pub redeemer: Option<Vec<u8>>,
    pub script_source: Option<Vec<u8>>,
}

impl TxInput {
    pub const MAGIC_PREFIX: [u8; 4] = [0x53, 0x43, 0x59, 0x02]; // "SCY\x02"

    pub fn new(
        prev_tx_hash: [u8; 32],
        output_index: u32,
        signature: Option<Vec<u8>>,
        redeemer: Option<Vec<u8>>,
        script_source: Option<Vec<u8>>,
    ) -> Self {
        Self {
            prev_tx_hash,
            output_index,
            signature,
            redeemer,
            script_source,
        }
    }

    pub fn to_authorization(&self) -> Vec<u8> {
        let witness = EutxoWitness {
            signature: self.signature.clone(),
            redeemer: self.redeemer.clone(),
            script_source: self.script_source.clone(),
        };
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&Self::MAGIC_PREFIX);
        if let Ok(data) = bincode::serialize(&witness) {
            bytes.extend_from_slice(&data);
        }
        bytes
    }

    pub fn from_tx_in(tx_in: &TxIn) -> Option<Self> {
        let bytes = &tx_in.authorization;
        let witness: EutxoWitness = if bytes.len() >= 4 && bytes[..4] == Self::MAGIC_PREFIX {
            bincode::deserialize(&bytes[4..]).ok()?
        } else {
            bincode::deserialize(bytes).ok()?
        };
        Some(Self {
            prev_tx_hash: *tx_in.previous_output.txid.as_bytes(),
            output_index: tx_in.previous_output.index,
            signature: witness.signature,
            redeemer: witness.redeemer,
            script_source: witness.script_source,
        })
    }

    pub fn to_tx_in(&self) -> TxIn {
        let prev_out = OutPoint::new(Hash256::new(self.prev_tx_hash), self.output_index);
        TxIn::new(prev_out, self.to_authorization())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_outpoint_equality_and_hashing() {
        let txid1 = Hash256::hash(b"tx_1");
        let txid2 = Hash256::hash(b"tx_2");

        let op1 = OutPoint::new(txid1, 0);
        let op2 = OutPoint::new(txid1, 0);
        let op3 = OutPoint::new(txid2, 0);
        let op4 = OutPoint::new(txid1, 1);

        assert_eq!(op1, op2);
        assert_ne!(op1, op3);
        assert_ne!(op1, op4);

        let mut map = HashMap::new();
        map.insert(op1, "outpoint_1");
        assert_eq!(map.get(&op2), Some(&"outpoint_1"));
        assert_eq!(map.get(&op3), None);
    }

    #[test]
    fn test_txid_deterministic() {
        let op = OutPoint::new(Hash256::hash(b"prev_tx"), 0);
        let input = TxIn::new(op, vec![1, 2, 3]);
        let output = TxOut::new(100_000_000, vec![4, 5, 6]);

        let tx1 = Transaction::new(
            TRANSACTION_VERSION_1,
            vec![input.clone()],
            vec![output.clone()],
            0,
        );
        let tx2 = Transaction::new(TRANSACTION_VERSION_1, vec![input], vec![output], 0);

        assert_eq!(tx1.txid(), tx2.txid());
        assert_ne!(tx1.txid(), Hash256::ZERO);
    }

    #[test]
    fn test_fee_calculation() {
        // Normal fee
        let fee = calculate_fee(1_000_000_000, 900_000_000).unwrap();
        assert_eq!(fee, 100_000_000);

        // Zero fee
        let zero_fee = calculate_fee(500_000_000, 500_000_000).unwrap();
        assert_eq!(zero_fee, 0);

        // Deficit error (outputs exceed inputs)
        let err = calculate_fee(400_000_000, 500_000_000).unwrap_err();
        assert_eq!(
            err,
            TransactionError::InputValueDeficit {
                total_in: 400_000_000,
                total_out: 500_000_000,
            }
        );
    }

    #[test]
    fn test_reject_empty_inputs_and_outputs() {
        let op = OutPoint::new(Hash256::hash(b"prev"), 0);
        let valid_in = TxIn::new(op, vec![]);
        let valid_out = TxOut::new(100_000_000, vec![]);

        let empty_in_tx =
            Transaction::new(TRANSACTION_VERSION_1, vec![], vec![valid_out.clone()], 0);
        assert_eq!(
            empty_in_tx.validate_stateless(),
            Err(TransactionError::EmptyInputs)
        );

        let empty_out_tx = Transaction::new(TRANSACTION_VERSION_1, vec![valid_in], vec![], 0);
        assert_eq!(
            empty_out_tx.validate_stateless(),
            Err(TransactionError::EmptyOutputs)
        );
    }

    #[test]
    fn test_reject_zero_output_value() {
        let op = OutPoint::new(Hash256::hash(b"prev"), 0);
        let valid_in = TxIn::new(op, vec![]);
        let zero_out = TxOut::new(0, vec![]);

        let tx = Transaction::new(TRANSACTION_VERSION_1, vec![valid_in], vec![zero_out], 0);
        assert_eq!(
            tx.validate_stateless(),
            Err(TransactionError::ZeroOutputValue)
        );
    }

    #[test]
    fn test_reject_output_overflow() {
        let op = OutPoint::new(Hash256::hash(b"prev"), 0);
        let valid_in = TxIn::new(op, vec![]);
        let huge_out_1 = TxOut::new(u64::MAX - 5, vec![]);
        let huge_out_2 = TxOut::new(10, vec![]);

        let tx = Transaction::new(
            TRANSACTION_VERSION_1,
            vec![valid_in],
            vec![huge_out_1, huge_out_2],
            0,
        );
        assert_eq!(
            tx.validate_stateless(),
            Err(TransactionError::OutputValueOverflow)
        );
    }

    #[test]
    fn test_reject_duplicate_inputs() {
        let op = OutPoint::new(Hash256::hash(b"prev"), 0);
        let in1 = TxIn::new(op, vec![1]);
        let in2 = TxIn::new(op, vec![2]);
        let valid_out = TxOut::new(100_000_000, vec![]);

        let tx = Transaction::new(TRANSACTION_VERSION_1, vec![in1, in2], vec![valid_out], 0);
        assert_eq!(
            tx.validate_stateless(),
            Err(TransactionError::DuplicateInput(op))
        );
    }

    #[test]
    fn test_reject_invalid_version() {
        let op = OutPoint::new(Hash256::hash(b"prev"), 0);
        let valid_in = TxIn::new(op, vec![]);
        let valid_out = TxOut::new(100_000_000, vec![]);

        let tx = Transaction::new(99, vec![valid_in], vec![valid_out], 0);
        assert_eq!(
            tx.validate_stateless(),
            Err(TransactionError::InvalidVersion(99))
        );
    }

    #[test]
    fn test_valid_transaction_passes_stateless() {
        let op1 = OutPoint::new(Hash256::hash(b"prev_1"), 0);
        let op2 = OutPoint::new(Hash256::hash(b"prev_2"), 1);
        let in1 = TxIn::new(op1, vec![1, 2]);
        let in2 = TxIn::new(op2, vec![3, 4]);

        let out1 = TxOut::new(100_000_000, vec![5, 6]);
        let out2 = TxOut::new(200_000_000, vec![7, 8]);

        let tx = Transaction::new(TRANSACTION_VERSION_1, vec![in1, in2], vec![out1, out2], 0);
        assert!(tx.validate_stateless().is_ok());
        assert_eq!(tx.total_output_quanta().unwrap(), 300_000_000);
    }

    #[test]
    fn test_compute_sighash_determinism_and_binding() {
        let op1 = OutPoint::new(Hash256::hash(b"prev_1"), 0);
        let in1 = TxIn::new(op1, vec![]);
        let out1 = TxOut::new(100_000_000, vec![0x01, 0x02, 0x03]);
        let tx = Transaction::new(TRANSACTION_VERSION_1, vec![in1], vec![out1], 0);

        let prev_lock = vec![0x01, 0x02, 0x03];
        let sighash1 = tx.compute_sighash(0, &prev_lock);
        let sighash2 = tx.compute_sighash(0, &prev_lock);
        assert_eq!(sighash1, sighash2, "sighash must be deterministic");

        // Changing input_index produces different sighash
        let sighash_idx1 = tx.compute_sighash(1, &prev_lock);
        assert_ne!(sighash1, sighash_idx1);

        // Changing prev_locking_script produces different sighash
        let sighash_diff_lock = tx.compute_sighash(0, &[0x01, 0x02, 0x04]);
        assert_ne!(sighash1, sighash_diff_lock);
    }

    #[test]
    fn test_op_return_zero_value_allowed() {
        let op = OutPoint::new(Hash256::hash(b"prev"), 0);
        let in1 = TxIn::new(op, vec![]);
        // OP_RETURN with 0 value is allowed
        let op_return_out = TxOut::new(0, vec![0x6a, 0x04, 0xde, 0xad, 0xbe, 0xef]);
        let tx = Transaction::new(TRANSACTION_VERSION_1, vec![in1], vec![op_return_out], 0);
        assert!(tx.validate_stateless().is_ok());

        // Non-OP_RETURN output with 0 value is rejected
        let op2 = OutPoint::new(Hash256::hash(b"prev2"), 0);
        let in2 = TxIn::new(op2, vec![]);
        let bad_out = TxOut::new(0, vec![0x01, 0x02]);
        let bad_tx = Transaction::new(TRANSACTION_VERSION_1, vec![in2], vec![bad_out], 0);
        assert_eq!(
            bad_tx.validate_stateless(),
            Err(TransactionError::ZeroOutputValue)
        );
    }
}
