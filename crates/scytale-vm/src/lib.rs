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

        let linker = <Linker<VmState>>::new(&engine);
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
    use scytale_sdk::TxContext;
    use std::process::Command;

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

        // 2. Setup Data
        let datum_bytes = bincode::serialize(&(
            [0u8; 32],     // owner_pubkey
            1750000000u64, // unlock_time
            [1u8; 32],     // emergency_key
            5000u64,       // penalty_fee
        ))
        .unwrap();

        let _redeemer_invalid = bincode::serialize(&(0u32, false)).unwrap(); // NormalWithdraw, false
        let redeemer_valid = bincode::serialize(&(0u32, true)).unwrap();   // NormalWithdraw, true

        let ctx_too_early = TxContext {
            tx_hash: [0u8; 32],
            block_time: 1700000000, // Belum unlock
            input_amount: 100_000,
            fee_burned: 100,
        };

        let ctx_unlocked = TxContext {
            tx_hash: [0u8; 32],
            block_time: 1800000000, // Sudah lewat unlock
            input_amount: 100_000,
            fee_burned: 100,
        };

        // 3. Eksekusi Test: Harus Ditolak (Waktu belum tiba)
        let res_early = ScyVM::execute_validator(
            &wasm_bytes,
            &datum_bytes,
            &redeemer_valid,
            &ctx_early_or_invalid(&ctx_too_early),
            1_000_000,
        )
        .unwrap();
        assert!(!res_early.is_valid, "Kontrak harus menolak penarikan sebelum unlock time");

        // 4. Eksekusi Test: Harus Sukses
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

    fn ctx_early_or_invalid(ctx: &TxContext) -> TxContext {
        ctx.clone()
    }
}
