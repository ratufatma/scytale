use crate::error::AuthorizationError;
use crate::transaction::Transaction;
use crate::utxo::UtxoEntry;
use scytale_primitives::Hash256;

/// Stateless cryptographic verification interface for evaluating UTXO spending authorizations.
pub trait AuthorizationVerifier {
    /// Verifies that an authorization proof satisfies a locking condition for the given preimage digest.
    fn verify(
        &self,
        preimage_digest: &Hash256,
        locking_condition: &[u8],
        authorization_proof: &[u8],
    ) -> Result<(), AuthorizationError>;
}

/// Verifies authorization proofs for all inputs in a transaction against their resolved UTXO entries.
///
/// Fails closed if any input proof is missing, malformed, or invalid.
pub fn verify_transaction_authorization<V: AuthorizationVerifier>(
    tx: &Transaction,
    resolved_utxos: &[UtxoEntry],
    verifier: &V,
) -> Result<(), AuthorizationError> {
    if tx.inputs.len() != resolved_utxos.len() {
        return Err(AuthorizationError::MismatchedUtxoCount {
            input_count: tx.inputs.len(),
            utxo_count: resolved_utxos.len(),
        });
    }

    for (index, input) in tx.inputs.iter().enumerate() {
        if input.authorization.is_empty() {
            return Err(AuthorizationError::EmptyAuthorization);
        }

        let preimage_digest = tx.signature_preimage_digest(index)?;
        let locking_condition = &resolved_utxos[index].output.locking_condition;

        verifier.verify(&preimage_digest, locking_condition, &input.authorization)?;
    }

    Ok(())
}

/// Canonical consensus script verifier executing ScytaleScript via `ScriptEngine`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ConsensusScriptVerifier {
    pub current_block_height: u64,
}

impl ConsensusScriptVerifier {
    pub const fn new(current_block_height: u64) -> Self {
        Self {
            current_block_height,
        }
    }
}

impl AuthorizationVerifier for ConsensusScriptVerifier {
    fn verify(
        &self,
        preimage_digest: &Hash256,
        locking_condition: &[u8],
        authorization_proof: &[u8],
    ) -> Result<(), AuthorizationError> {
        let ctx = scytale_script::ScriptContext::new(
            preimage_digest.as_bytes(),
            self.current_block_height,
        );
        let engine = scytale_script::ScriptEngine::default();
        let valid = engine
            .execute(authorization_proof, locking_condition, &ctx)
            .map_err(|e| AuthorizationError::MalformedProof(e.to_string()))?;
        if valid {
            Ok(())
        } else {
            Err(AuthorizationError::SignatureMismatch)
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::transaction::{TxIn, TRANSACTION_VERSION_1};
    use scytale_primitives::{OutPoint, TxOut};

    /// Mock authorization verifier for testing consensus boundary mechanics.
    ///
    /// Proof format: `[key_len: u32_le] || [key_bytes] || [signature: 32 bytes]`
    /// Signature is valid if `signature == BLAKE3(preimage_digest || key_bytes)`.
    pub struct MockAuthorizationVerifier;

    impl AuthorizationVerifier for MockAuthorizationVerifier {
        fn verify(
            &self,
            preimage_digest: &Hash256,
            locking_condition: &[u8],
            authorization_proof: &[u8],
        ) -> Result<(), AuthorizationError> {
            if authorization_proof.is_empty() {
                return Err(AuthorizationError::EmptyAuthorization);
            }
            if authorization_proof.len() < 4 + 32 {
                return Err(AuthorizationError::MalformedProof(
                    "proof too short for key length and signature".to_string(),
                ));
            }

            let key_len =
                u32::from_le_bytes(authorization_proof[0..4].try_into().map_err(|_| {
                    AuthorizationError::MalformedProof("invalid key len bytes".to_string())
                })?) as usize;

            if authorization_proof.len() < 4 + key_len + 32 {
                return Err(AuthorizationError::MalformedProof(
                    "proof truncated for specified key length".to_string(),
                ));
            }

            let key_bytes = &authorization_proof[4..4 + key_len];
            let signature_bytes = &authorization_proof[4 + key_len..4 + key_len + 32];

            // Verify key satisfies locking condition
            if key_bytes != locking_condition {
                return Err(AuthorizationError::KeyConditionMismatch);
            }

            // Verify mock signature: BLAKE3(preimage_digest || key_bytes)
            let mut preimage = Vec::with_capacity(32 + key_len);
            preimage.extend_from_slice(preimage_digest.as_bytes());
            preimage.extend_from_slice(key_bytes);
            let expected_sig = Hash256::hash(&preimage);

            if signature_bytes != expected_sig.as_bytes() {
                return Err(AuthorizationError::SignatureMismatch);
            }

            Ok(())
        }
    }

    /// Helper to construct mock authorization proof.
    pub fn create_mock_proof(preimage_digest: &Hash256, key: &[u8]) -> Vec<u8> {
        let mut preimage = Vec::with_capacity(32 + key.len());
        preimage.extend_from_slice(preimage_digest.as_bytes());
        preimage.extend_from_slice(key);
        let sig = Hash256::hash(&preimage);

        let mut proof = Vec::with_capacity(4 + key.len() + 32);
        proof.extend_from_slice(&(key.len() as u32).to_le_bytes());
        proof.extend_from_slice(key);
        proof.extend_from_slice(sig.as_bytes());
        proof
    }

    #[test]
    fn test_context_digest_determinism() {
        let op = OutPoint::new(Hash256::hash(b"prev_tx"), 0);
        let input = TxIn::new(op, vec![]);
        let output = TxOut::new(100_000_000, vec![1, 2, 3]);

        let tx1 = Transaction::new(
            TRANSACTION_VERSION_1,
            vec![input.clone()],
            vec![output.clone()],
            0,
        );
        let tx2 = Transaction::new(TRANSACTION_VERSION_1, vec![input], vec![output], 0);

        let d1 = tx1.signature_preimage_digest(0).unwrap();
        let d2 = tx2.signature_preimage_digest(0).unwrap();

        assert_eq!(d1, d2);
        assert_ne!(d1, Hash256::ZERO);
    }

    #[test]
    fn test_anti_replay_amount_mutation() {
        let op = OutPoint::new(Hash256::hash(b"prev_tx"), 0);
        let input = TxIn::new(op, vec![]);

        let tx_orig = Transaction::new(
            TRANSACTION_VERSION_1,
            vec![input.clone()],
            vec![TxOut::new(100_000_000, vec![1, 2, 3])],
            0,
        );
        let tx_mutated = Transaction::new(
            TRANSACTION_VERSION_1,
            vec![input],
            vec![TxOut::new(100_000_001, vec![1, 2, 3])],
            0,
        );

        let d_orig = tx_orig.signature_preimage_digest(0).unwrap();
        let d_mut = tx_mutated.signature_preimage_digest(0).unwrap();

        assert_ne!(d_orig, d_mut);
    }

    #[test]
    fn test_anti_replay_recipient_mutation() {
        let op = OutPoint::new(Hash256::hash(b"prev_tx"), 0);
        let input = TxIn::new(op, vec![]);

        let tx_orig = Transaction::new(
            TRANSACTION_VERSION_1,
            vec![input.clone()],
            vec![TxOut::new(100_000_000, vec![1, 2, 3])],
            0,
        );
        let tx_mutated = Transaction::new(
            TRANSACTION_VERSION_1,
            vec![input],
            vec![TxOut::new(100_000_000, vec![1, 2, 4])],
            0,
        );

        let d_orig = tx_orig.signature_preimage_digest(0).unwrap();
        let d_mut = tx_mutated.signature_preimage_digest(0).unwrap();

        assert_ne!(d_orig, d_mut);
    }

    #[test]
    fn test_anti_replay_cross_index() {
        let op1 = OutPoint::new(Hash256::hash(b"prev_1"), 0);
        let op2 = OutPoint::new(Hash256::hash(b"prev_2"), 1);

        let tx = Transaction::new(
            TRANSACTION_VERSION_1,
            vec![TxIn::new(op1, vec![]), TxIn::new(op2, vec![])],
            vec![TxOut::new(500_000_000, vec![])],
            0,
        );

        let d_in0 = tx.signature_preimage_digest(0).unwrap();
        let d_in1 = tx.signature_preimage_digest(1).unwrap();

        assert_ne!(d_in0, d_in1);
    }

    #[test]
    fn test_valid_multi_input_verification() {
        let verifier = MockAuthorizationVerifier;

        let key1 = b"owner_pubkey_alpha";
        let key2 = b"owner_pubkey_beta";

        let op1 = OutPoint::new(Hash256::hash(b"prev_tx_1"), 0);
        let op2 = OutPoint::new(Hash256::hash(b"prev_tx_2"), 0);

        let utxo1 = UtxoEntry::new(TxOut::new(300_000_000, key1.to_vec()), 1, false);
        let utxo2 = UtxoEntry::new(TxOut::new(200_000_000, key2.to_vec()), 1, false);

        // Preliminary transaction without proofs to derive preimage digests
        let mut tx = Transaction::new(
            TRANSACTION_VERSION_1,
            vec![TxIn::new(op1, vec![]), TxIn::new(op2, vec![])],
            vec![TxOut::new(490_000_000, vec![9, 9, 9])],
            0,
        );

        let digest0 = tx.signature_preimage_digest(0).unwrap();
        let digest1 = tx.signature_preimage_digest(1).unwrap();

        let proof0 = create_mock_proof(&digest0, key1);
        let proof1 = create_mock_proof(&digest1, key2);

        tx.inputs[0].authorization = proof0;
        tx.inputs[1].authorization = proof1;

        let result = verify_transaction_authorization(&tx, &[utxo1, utxo2], &verifier);
        assert!(result.is_ok());
    }

    #[test]
    fn test_fail_on_single_tampered_input() {
        let verifier = MockAuthorizationVerifier;

        let key1 = b"owner_pubkey_alpha";
        let key2 = b"owner_pubkey_beta";
        let key3 = b"owner_pubkey_gamma";

        let op1 = OutPoint::new(Hash256::hash(b"prev_1"), 0);
        let op2 = OutPoint::new(Hash256::hash(b"prev_2"), 0);
        let op3 = OutPoint::new(Hash256::hash(b"prev_3"), 0);

        let utxos = vec![
            UtxoEntry::new(TxOut::new(100_000_000, key1.to_vec()), 1, false),
            UtxoEntry::new(TxOut::new(100_000_000, key2.to_vec()), 1, false),
            UtxoEntry::new(TxOut::new(100_000_000, key3.to_vec()), 1, false),
        ];

        let mut tx = Transaction::new(
            TRANSACTION_VERSION_1,
            vec![
                TxIn::new(op1, vec![]),
                TxIn::new(op2, vec![]),
                TxIn::new(op3, vec![]),
            ],
            vec![TxOut::new(290_000_000, vec![])],
            0,
        );

        let d0 = tx.signature_preimage_digest(0).unwrap();
        let d1 = tx.signature_preimage_digest(1).unwrap();
        let d2 = tx.signature_preimage_digest(2).unwrap();

        let mut proof1_tampered = create_mock_proof(&d1, key2);
        // Tamper with the signature bytes in proof 1
        let last = proof1_tampered.len() - 1;
        proof1_tampered[last] ^= 0xFF;

        tx.inputs[0].authorization = create_mock_proof(&d0, key1);
        tx.inputs[1].authorization = proof1_tampered;
        tx.inputs[2].authorization = create_mock_proof(&d2, key3);

        let result = verify_transaction_authorization(&tx, &utxos, &verifier);
        assert_eq!(result, Err(AuthorizationError::SignatureMismatch));
    }

    #[test]
    fn test_reject_empty_authorization_proof() {
        let verifier = MockAuthorizationVerifier;
        let op = OutPoint::new(Hash256::hash(b"prev"), 0);
        let utxo = UtxoEntry::new(TxOut::new(100_000_000, b"key".to_vec()), 1, false);

        let tx = Transaction::new(
            TRANSACTION_VERSION_1,
            vec![TxIn::new(op, vec![])],
            vec![TxOut::new(90_000_000, vec![])],
            0,
        );

        let result = verify_transaction_authorization(&tx, &[utxo], &verifier);
        assert_eq!(result, Err(AuthorizationError::EmptyAuthorization));
    }

    #[test]
    fn test_reject_mismatched_utxo_count() {
        let verifier = MockAuthorizationVerifier;
        let op1 = OutPoint::new(Hash256::hash(b"prev_1"), 0);
        let op2 = OutPoint::new(Hash256::hash(b"prev_2"), 0);
        let utxo1 = UtxoEntry::new(TxOut::new(100_000_000, b"key".to_vec()), 1, false);

        let tx = Transaction::new(
            TRANSACTION_VERSION_1,
            vec![TxIn::new(op1, vec![1]), TxIn::new(op2, vec![2])],
            vec![TxOut::new(190_000_000, vec![])],
            0,
        );

        let result = verify_transaction_authorization(&tx, &[utxo1], &verifier);
        assert_eq!(
            result,
            Err(AuthorizationError::MismatchedUtxoCount {
                input_count: 2,
                utxo_count: 1,
            })
        );
    }

    #[test]
    fn test_reject_invalid_input_index() {
        let op = OutPoint::new(Hash256::hash(b"prev"), 0);
        let tx = Transaction::new(
            TRANSACTION_VERSION_1,
            vec![TxIn::new(op, vec![])],
            vec![TxOut::new(90_000_000, vec![])],
            0,
        );

        let result = tx.signature_preimage_digest(5);
        assert_eq!(
            result,
            Err(AuthorizationError::InvalidInputIndex {
                index: 5,
                total_inputs: 1,
            })
        );
    }
}
