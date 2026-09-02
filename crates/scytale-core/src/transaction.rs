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
            if output.value == 0 {
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
}
