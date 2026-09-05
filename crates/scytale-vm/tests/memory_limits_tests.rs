use scytale_sdk::TxContext;
use scytale_vm::{ScyVM, VmError, MAX_WASM_MEMORY_PAGES};

fn encode_sleb128(mut val: i32) -> Vec<u8> {
    let mut result = Vec::new();
    let mut more = true;
    while more {
        let mut byte = (val & 0x7f) as u8;
        val >>= 7;
        if (val == 0 && (byte & 0x40) == 0) || (val == -1 && (byte & 0x40) != 0) {
            more = false;
        } else {
            byte |= 0x80;
        }
        result.push(byte);
    }
    result
}

fn build_test_wasm(initial_pages: u8, grow_pages: u8) -> Vec<u8> {
    let mut wasm = Vec::new();
    // Magic and version
    wasm.extend_from_slice(&[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);

    // Type section (id 1): 1 type (i32, i32, i32, i32, i32, i32) -> i32
    let type_body = [
        0x01, // 1 type
        0x60, // func
        0x06, 0x7f, 0x7f, 0x7f, 0x7f, 0x7f, 0x7f, // 6 params i32
        0x01, 0x7f, // 1 return i32
    ];
    wasm.push(0x01);
    wasm.push(type_body.len() as u8);
    wasm.extend_from_slice(&type_body);

    // Function section (id 3): 1 func of type 0
    let func_body = [0x01, 0x00];
    wasm.push(0x03);
    wasm.push(func_body.len() as u8);
    wasm.extend_from_slice(&func_body);

    // Memory section (id 5): 1 memory with initial_pages
    let mem_body = [0x01, 0x00, initial_pages];
    wasm.push(0x05);
    wasm.push(mem_body.len() as u8);
    wasm.extend_from_slice(&mem_body);

    // Export section (id 7): "memory" (mem 0), "validate" (func 0)
    let mut exp_body = Vec::new();
    exp_body.push(0x02); // 2 exports
    exp_body.push(0x06);
    exp_body.extend_from_slice(b"memory");
    exp_body.extend_from_slice(&[0x02, 0x00]);
    exp_body.push(0x08);
    exp_body.extend_from_slice(b"validate");
    exp_body.extend_from_slice(&[0x00, 0x00]);

    wasm.push(0x07);
    wasm.push(exp_body.len() as u8);
    wasm.extend_from_slice(&exp_body);

    // Code section (id 10)
    let mut code_body = Vec::new();
    code_body.push(0x00); // 0 locals
    if grow_pages > 0 {
        // i32.const grow_pages; memory.grow 0; drop;
        code_body.push(0x41);
        code_body.extend(encode_sleb128(grow_pages as i32));
        code_body.extend_from_slice(&[0x40, 0x00, 0x1a]);
    }
    // i32.const 1; end
    code_body.extend_from_slice(&[0x41, 0x01, 0x0b]);

    let mut func_code = Vec::new();
    func_code.push(code_body.len() as u8);
    func_code.extend_from_slice(&code_body);

    let mut code_sec_body = Vec::new();
    code_sec_body.push(0x01); // 1 function body
    code_sec_body.extend_from_slice(&func_code);

    wasm.push(0x0a);
    wasm.push(code_sec_body.len() as u8);
    wasm.extend_from_slice(&code_sec_body);

    wasm
}

fn dummy_context() -> TxContext {
    TxContext {
        tx_hash: [0u8; 32],
        block_time: 1700000000,
        input_amount: 100_000,
        fee_burned: 1_000,
    }
}

#[test]
fn test_normal_memory_execution() {
    let wasm = build_test_wasm(1, 0); // 1 page (64 KiB), well below 64 pages
    let ctx = dummy_context();
    let res = ScyVM::execute_validator(&wasm, b"datum", b"redeemer", &ctx, 1_000_000).unwrap();
    assert!(res.is_valid);
    assert!(res.gas_consumed > 0);
}

#[test]
fn test_reject_excessive_initial_memory() {
    // 65 pages > MAX_WASM_MEMORY_PAGES (64 pages)
    let wasm = build_test_wasm(65, 0);
    let ctx = dummy_context();
    let err = ScyVM::execute_validator(&wasm, b"datum", b"redeemer", &ctx, 1_000_000).unwrap_err();
    match err {
        VmError::MemoryLimitExceeded { pages, max_pages } => {
            assert_eq!(pages, 65);
            assert_eq!(max_pages, MAX_WASM_MEMORY_PAGES);
        }
        VmError::InstantiationFailed => {
            // wasmi StoreLimits can reject instantiation if memory exceeds limit
        }
        other => panic!("expected memory limit error, got: {:?}", other),
    }
}

#[test]
fn test_reject_memory_grow_beyond_upper_bound() {
    // Start with 1 page, attempt to grow by 70 pages (total 71 pages > 64)
    let wasm = build_test_wasm(1, 70);
    let ctx = dummy_context();
    // With trap_on_grow_failure(true), memory.grow traps or execution fails safely
    let res = ScyVM::execute_validator(&wasm, b"datum", b"redeemer", &ctx, 1_000_000);
    assert!(res.is_err(), "growing memory beyond upper bound must fail");
    match res.unwrap_err() {
        VmError::ExecutionTrapped
        | VmError::MemoryLimitExceeded { .. }
        | VmError::MemoryAccessViolation => {}
        other => panic!("unexpected error variant on memory grow: {:?}", other),
    }
}

#[test]
fn test_out_of_fuel_traps_safely() {
    let wasm = build_test_wasm(1, 0);
    let ctx = dummy_context();
    // Berikan gas_limit yang sangat kecil (hanya 1 fuel unit)
    let res = ScyVM::execute_validator(&wasm, b"datum", b"redeemer", &ctx, 1);
    assert!(res.is_err(), "eksekusi dengan fuel tidak mencukupi harus gagal");
    match res.unwrap_err() {
        VmError::ExecutionTrapped | VmError::OutOfGas => {}
        other => panic!("expected OutOfGas or ExecutionTrapped, got: {:?}", other),
    }
}

