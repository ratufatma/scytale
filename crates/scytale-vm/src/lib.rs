use scytale_sdk::TxContext;
use wasmi::*;

/// Maximum linear memory pages allowed for a smart contract instance (64 pages = 4 MiB).
pub const MAX_WASM_MEMORY_PAGES: u32 = 64;
/// Standard WebAssembly memory page size in bytes (64 KiB).
pub const WASM_PAGE_SIZE: usize = 65536;
/// Maximum linear memory in bytes allowed for a smart contract instance (4,194,304 bytes).
pub const MAX_WASM_MEMORY_BYTES: usize = (MAX_WASM_MEMORY_PAGES as usize) * WASM_PAGE_SIZE;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum VmError {
    CompilationFailed,
    InstantiationFailed,
    ExecutionTrapped,
    MemoryAccessViolation,
    OutOfGas,
    MemoryLimitExceeded { pages: u32, max_pages: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionResult {
    pub is_valid: bool,
    pub gas_consumed: u64,
}

struct VmState {
    limits: StoreLimits,
}

/// Mesin eksekusi ScyVM untuk memvalidasi transaksi berbasis eUTXO
pub struct ScyVM;

impl ScyVM {
    pub fn execute_validator(
        wasm_bytecode: &[u8],
        datum: &[u8],
        redeemer: &[u8],
        context: &TxContext,
        gas_limit: u64,
    ) -> Result<ExecutionResult, VmError> {
        let mut config = Config::default();
        config.consume_fuel(true);

        let engine = Engine::new(&config);
        let module = Module::new(&engine, wasm_bytecode).map_err(|_| VmError::CompilationFailed)?;

        let limits = StoreLimitsBuilder::new()
            .memory_size(MAX_WASM_MEMORY_BYTES)
            .trap_on_grow_failure(true)
            .build();

        let mut store = Store::new(&engine, VmState { limits });
        store.limiter(|state| &mut state.limits);
        store.add_fuel(gas_limit).map_err(|_| VmError::OutOfGas)?;

        let mut linker = <Linker<VmState>>::new(&engine);

        // Host Function 1: scytale_crypto_ed25519_verify
        linker
            .func_wrap(
                "env",
                "scytale_crypto_ed25519_verify",
                |mut caller: Caller<'_, VmState>,
                 pk_ptr: i32,
                 pk_len: i32,
                 sig_ptr: i32,
                 sig_len: i32,
                 msg_ptr: i32,
                 msg_len: i32|
                 -> i32 {
                    if caller.consume_fuel(200).is_err() {
                        return 0;
                    }

                    if pk_len != 32
                        || sig_len != 64
                        || pk_ptr < 0
                        || sig_ptr < 0
                        || msg_ptr < 0
                        || msg_len < 0
                    {
                        return 0;
                    }

                    let memory = match caller.get_export("memory").and_then(Extern::into_memory) {
                        Some(m) => m,
                        None => return 0,
                    };

                    let mut pk_bytes = [0u8; 32];
                    if memory.read(&caller, pk_ptr as usize, &mut pk_bytes).is_err() {
                        return 0;
                    }

                    let mut sig_bytes = [0u8; 64];
                    if memory.read(&caller, sig_ptr as usize, &mut sig_bytes).is_err() {
                        return 0;
                    }

                    let mut msg_bytes = vec![0u8; msg_len as usize];
                    if memory.read(&caller, msg_ptr as usize, &mut msg_bytes).is_err() {
                        return 0;
                    }

                    let verifying_key = match ed25519_dalek::VerifyingKey::from_bytes(&pk_bytes) {
                        Ok(k) => k,
                        Err(_) => return 0,
                    };
                    let signature = ed25519_dalek::Signature::from_bytes(&sig_bytes);

                    if verifying_key.verify_strict(&msg_bytes, &signature).is_ok() {
                        1
                    } else {
                        0
                    }
                },
            )
            .map_err(|_| VmError::InstantiationFailed)?;

        // Host Function 2: scytale_crypto_blake3
        linker
            .func_wrap(
                "env",
                "scytale_crypto_blake3",
                |mut caller: Caller<'_, VmState>,
                 data_ptr: i32,
                 data_len: i32,
                 out_ptr: i32| {
                    if data_ptr < 0 || data_len < 0 || out_ptr < 0 {
                        return;
                    }

                    let fuel = 15u64.saturating_add((data_len as u64).saturating_div(64));
                    if caller.consume_fuel(fuel).is_err() {
                        return;
                    }

                    let memory = match caller.get_export("memory").and_then(Extern::into_memory) {
                        Some(m) => m,
                        None => return,
                    };

                    let mut data = vec![0u8; data_len as usize];
                    if memory.read(&caller, data_ptr as usize, &mut data).is_err() {
                        return;
                    }

                    let hash = blake3::hash(&data);
                    let _ = memory.write(&mut caller, out_ptr as usize, hash.as_bytes());
                },
            )
            .map_err(|_| VmError::InstantiationFailed)?;

        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|_| VmError::InstantiationFailed)?
            .start(&mut store)
            .map_err(|_| VmError::InstantiationFailed)?;

        let memory = instance
            .get_memory(&store, "memory")
            .ok_or(VmError::MemoryAccessViolation)?;

        let initial_pages: u32 = u32::from(memory.current_pages(&store));
        if initial_pages > MAX_WASM_MEMORY_PAGES {
            return Err(VmError::MemoryLimitExceeded {
                pages: initial_pages,
                max_pages: MAX_WASM_MEMORY_PAGES,
            });
        }

        let ctx_bytes = bincode::serialize(context).map_err(|_| VmError::MemoryAccessViolation)?;

        // Alokasikan ruang data langsung di offset memori Wasm linear
        let datum_offset = 1024;
        let redeemer_offset = datum_offset + datum.len();
        let ctx_offset = redeemer_offset + redeemer.len();

        memory
            .write(&mut store, datum_offset, datum)
            .map_err(|_| VmError::MemoryAccessViolation)?;
        memory
            .write(&mut store, redeemer_offset, redeemer)
            .map_err(|_| VmError::MemoryAccessViolation)?;
        memory
            .write(&mut store, ctx_offset, &ctx_bytes)
            .map_err(|_| VmError::MemoryAccessViolation)?;

        let validate_fn = instance
            .get_typed_func::<(i32, i32, i32, i32, i32, i32), i32>(&store, "validate")
            .map_err(|_| VmError::InstantiationFailed)?;

        let result = validate_fn
            .call(
                &mut store,
                (
                    datum_offset as i32,
                    datum.len() as i32,
                    redeemer_offset as i32,
                    redeemer.len() as i32,
                    ctx_offset as i32,
                    ctx_bytes.len() as i32,
                ),
            )
            .map_err(|_| VmError::ExecutionTrapped)?;

        let final_pages: u32 = u32::from(memory.current_pages(&store));
        if final_pages > MAX_WASM_MEMORY_PAGES {
            return Err(VmError::MemoryLimitExceeded {
                pages: final_pages,
                max_pages: MAX_WASM_MEMORY_PAGES,
            });
        }

        let gas_consumed = store.fuel_consumed().unwrap_or(0);

        Ok(ExecutionResult {
            is_valid: result == 1,
            gas_consumed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use scytale_sdk::TxContext;
    use serde::Serialize;
    use std::process::Command;

    mod serde_sig {
        pub fn serialize<S>(sig: &[u8; 64], serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            serializer.serialize_bytes(sig)
        }
    }

    #[derive(Serialize)]
    enum TestVaultRedeemer {
        NormalWithdraw {
            #[serde(with = "serde_sig")]
            signature: [u8; 64],
        },
    }

    #[test]
    fn test_vault_eutxo_lifecycle() {
        // 1. Build contract Wasm
        let status = Command::new("cargo")
            .args([
                "build",
                "--target",
                "wasm32-unknown-unknown",
                "--release",
                "-p",
                "scytale-contract-vault",
            ])
            .status()
            .expect("Gagal mengompilasi kontrak vault");
        assert!(status.success());

        let wasm_bytes = std::fs::read(
            "../../target/wasm32-unknown-unknown/release/scytale_contract_vault.wasm",
        )
        .or_else(|_| {
            std::fs::read("target/wasm32-unknown-unknown/release/scytale_contract_vault.wasm")
        })
        .expect("File wasm tidak ditemukan");

        // 2. Setup Data & Ed25519 Keypair
        let secret = [42u8; 32];
        let signing_key = SigningKey::from_bytes(&secret);
        let owner_pubkey = signing_key.verifying_key().to_bytes();

        let datum_bytes = bincode::serialize(&(
            owner_pubkey,
            1750000000u64, // unlock_time
            [1u8; 32],     // emergency_key
            5000u64,       // penalty_fee
        ))
        .unwrap();

        let ctx_too_early = TxContext {
            tx_hash: [0xaa; 32],
            block_time: 1700000000, // Belum unlock
            input_amount: 100_000,
            fee_burned: 100,
        };

        let ctx_unlocked = TxContext {
            tx_hash: [0xaa; 32],
            block_time: 1800000000, // Sudah lewat unlock
            input_amount: 100_000,
            fee_burned: 100,
        };

        let valid_sig = signing_key.sign(&ctx_unlocked.tx_hash).to_bytes();
        let redeemer_valid = bincode::serialize(&TestVaultRedeemer::NormalWithdraw {
            signature: valid_sig,
        })
        .unwrap();

        let mut invalid_sig = valid_sig;
        invalid_sig[0] ^= 0xff; // Tamper signature
        let redeemer_invalid_sig = bincode::serialize(&TestVaultRedeemer::NormalWithdraw {
            signature: invalid_sig,
        })
        .unwrap();

        // 3. Eksekusi Test: Harus Ditolak (Waktu belum tiba)
        let res_early = ScyVM::execute_validator(
            &wasm_bytes,
            &datum_bytes,
            &redeemer_valid,
            &ctx_too_early,
            1_000_000,
        )
        .unwrap();
        assert!(!res_early.is_valid, "Kontrak harus menolak penarikan sebelum unlock time");

        // 4. Eksekusi Test: Harus Ditolak (Signature tidak sah)
        let res_invalid = ScyVM::execute_validator(
            &wasm_bytes,
            &datum_bytes,
            &redeemer_invalid_sig,
            &ctx_unlocked,
            1_000_000,
        )
        .unwrap();
        assert!(!res_invalid.is_valid, "Kontrak harus menolak penarikan dengan signature palsu");

        // 5. Eksekusi Test: Harus Sukses (Waktu lewat & signature valid)
        let res_success = ScyVM::execute_validator(
            &wasm_bytes,
            &datum_bytes,
            &redeemer_valid,
            &ctx_unlocked,
            1_000_000,
        )
        .unwrap();
        assert!(res_success.is_valid, "Kontrak harus meloloskan penarikan yang sah");
        assert!(res_success.gas_consumed > 0, "Gas harus terhitung");
    }
}
