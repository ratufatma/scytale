use crate::transaction::{OutputLock, Transaction, TxInput};
use crate::utxo::UtxoSet;
use scytale_primitives::Hash256;
use scytale_sdk::TxContext;
use scytale_vm::{ScyVM, VmError};
use thiserror::Error;

/// Maximum gas allowed for a single transaction (5 million fuel).
pub const MAX_TX_GAS: u64 = 5_000_000;

/// Maximum aggregate gas allowed per block (50 million fuel).
pub const MAX_BLOCK_GAS: u64 = 50_000_000;

#[derive(Debug, Error)]
pub enum EutxoValidationError {
    #[error("Missing UTXO for input {0}:{1}")]
    MissingUtxo(Hash256, u32),

    #[error("Script bytecode not provided in input")]
    MissingScriptSource,

    #[error("Redeemer not provided in input")]
    MissingRedeemer,

    #[error("Script bytecode hash mismatch (expected {expected}, got {actual})")]
    ScriptHashMismatch { expected: String, actual: String },

    #[error("ScyVM execution failed: {0:?}")]
    VmExecutionFailed(VmError),

    #[error("Smart contract rejected transaction (VALIDATION_REJECT)")]
    ValidationRejected,

    #[error("Transaction gas limit exceeded (consumed {consumed}, limit {limit})")]
    GasLimitExceeded { consumed: u64, limit: u64 },

    #[error("Block gas limit exceeded (consumed {consumed}, limit {limit})")]
    BlockGasLimitExceeded { consumed: u64, limit: u64 },

    #[error("Arithmetic overflow calculating fees or gas")]
    ArithmeticOverflow,
}

/// Constructs a deterministic `TxContext` for a transaction evaluation.
pub fn create_tx_context(
    tx: &Transaction,
    block_time: u64,
    total_input_amount: u64,
    total_output_amount: u64,
) -> TxContext {
    TxContext {
        tx_hash: tx.compute_hash(),
        block_time,
        input_amount: total_input_amount,
        fee_burned: total_input_amount.saturating_sub(total_output_amount),
    }
}

/// Validates all eUTXO script inputs in a transaction against their resolved UTXOs.
///
/// Returns total gas consumed across all script inputs on success.
/// Standard non-script inputs and coinbase transactions are ignored (consume 0 gas).
pub fn verify_transaction_eutxo(
    tx: &Transaction,
    block_time: u64,
    utxos: &UtxoSet,
    gas_limit: u64,
) -> Result<u64, EutxoValidationError> {
    if tx.is_coinbase() {
        return Ok(0);
    }

    // 1. Calculate total input value and total output value
    let mut total_in: u64 = 0;
    for input in &tx.inputs {
        let utxo = utxos.get(&input.previous_output).ok_or_else(|| {
            EutxoValidationError::MissingUtxo(
                input.previous_output.txid,
                input.previous_output.index,
            )
        })?;
        total_in = total_in
            .checked_add(utxo.output.value)
            .ok_or(EutxoValidationError::ArithmeticOverflow)?;
    }

    let total_out = tx
        .total_output_quanta()
        .map_err(|_| EutxoValidationError::ArithmeticOverflow)?;
    let tx_context = create_tx_context(tx, block_time, total_in, total_out);

    let mut total_gas_consumed: u64 = 0;

    for input in &tx.inputs {
        let utxo = utxos
            .get(&input.previous_output)
            .expect("UTXO checked above");

        if let Some(OutputLock::Script { script_hash, datum }) =
            OutputLock::from_locking_condition(&utxo.output.locking_condition)
        {
            let eutxo_in = TxInput::from_tx_in(input)
                .ok_or(EutxoValidationError::MissingScriptSource)?;

            let wasm_code = eutxo_in
                .script_source
                .as_deref()
                .ok_or(EutxoValidationError::MissingScriptSource)?;
            let redeemer = eutxo_in
                .redeemer
                .as_deref()
                .ok_or(EutxoValidationError::MissingRedeemer)?;

            // Verify Blake3 hash matches script_hash
            let calculated_hash = blake3::hash(wasm_code);
            if calculated_hash.as_bytes() != &script_hash {
                return Err(EutxoValidationError::ScriptHashMismatch {
                    expected: hex::encode(script_hash),
                    actual: hex::encode(calculated_hash.as_bytes()),
                });
            }

            let remaining_gas = gas_limit.saturating_sub(total_gas_consumed);
            let exec_res = ScyVM::execute_validator(
                wasm_code,
                &datum,
                redeemer,
                &tx_context,
                remaining_gas,
            )
            .map_err(EutxoValidationError::VmExecutionFailed)?;

            if !exec_res.is_valid {
                return Err(EutxoValidationError::ValidationRejected);
            }

            total_gas_consumed = total_gas_consumed
                .checked_add(exec_res.gas_consumed)
                .ok_or(EutxoValidationError::ArithmeticOverflow)?;

            if total_gas_consumed > gas_limit {
                return Err(EutxoValidationError::GasLimitExceeded {
                    consumed: total_gas_consumed,
                    limit: gas_limit,
                });
            }
        }
    }

    Ok(total_gas_consumed)
}
