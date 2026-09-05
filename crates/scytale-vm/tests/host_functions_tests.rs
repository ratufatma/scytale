use ed25519_dalek::{Signer, SigningKey};
use scytale_sdk::TxContext;
use scytale_vm::ScyVM;

/// Constructs a Wasm binary test module that imports:
/// - `env.scytale_crypto_ed25519_verify`: (i32, i32, i32, i32, i32, i32) -> i32
/// - `env.scytale_crypto_blake3`: (i32, i32, i32) -> ()
///
/// And exports:
/// - `memory`
/// - `validate`: (i32, i32, i32, i32, i32, i32) -> i32
fn build_crypto_test_wasm(test_mode: &str) -> Vec<u8> {
    let mut wasm = Vec::new();
    // 1. Magic + Version
    wasm.extend_from_slice(&[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);

    // 2. Type section (id 1):
    // Type 0: (i32, i32, i32, i32, i32, i32) -> i32
    // Type 1: (i32, i32, i32) -> ()
    let mut type_body = Vec::new();
    type_body.push(0x02); // 2 types
    type_body.extend_from_slice(&[0x60, 0x06, 0x7f, 0x7f, 0x7f, 0x7f, 0x7f, 0x7f, 0x01, 0x7f]);
    type_body.extend_from_slice(&[0x60, 0x03, 0x7f, 0x7f, 0x7f, 0x00]);
    wasm.push(0x01);
    wasm.push(type_body.len() as u8);
    wasm.extend_from_slice(&type_body);

    // 3. Import section (id 2):
    let mut import_body = Vec::new();
    import_body.push(0x02); // 2 imports

    // Import 0: "env"."scytale_crypto_ed25519_verify" (type 0)
    let env_bytes = b"env";
    let fn1_bytes = b"scytale_crypto_ed25519_verify";
    import_body.push(env_bytes.len() as u8);
    import_body.extend_from_slice(env_bytes);
    import_body.push(fn1_bytes.len() as u8);
    import_body.extend_from_slice(fn1_bytes);
    import_body.push(0x00); // func
    import_body.push(0x00); // type 0

    // Import 1: "env"."scytale_crypto_blake3" (type 1)
    let fn2_bytes = b"scytale_crypto_blake3";
    import_body.push(env_bytes.len() as u8);
    import_body.extend_from_slice(env_bytes);
    import_body.push(fn2_bytes.len() as u8);
    import_body.extend_from_slice(fn2_bytes);
    import_body.push(0x00); // func
    import_body.push(0x01); // type 1

    wasm.push(0x02);
    wasm.push(import_body.len() as u8);
    wasm.extend_from_slice(&import_body);

    // 4. Function section (id 3): 1 func (type 0) -> func index 2
    let func_body = [0x01, 0x00];
    wasm.push(0x03);
    wasm.push(func_body.len() as u8);
    wasm.extend_from_slice(&func_body);

    // 5. Memory section (id 5): 1 memory, 1 page
    let mem_body = [0x01, 0x00, 0x01];
    wasm.push(0x05);
    wasm.push(mem_body.len() as u8);
    wasm.extend_from_slice(&mem_body);

    // 6. Export section (id 7): "memory" (mem 0), "validate" (func 2)
    let mut exp_body = Vec::new();
    exp_body.push(0x02); // 2 exports
    exp_body.push(0x06);
    exp_body.extend_from_slice(b"memory");
    exp_body.extend_from_slice(&[0x02, 0x00]);
    exp_body.push(0x08);
    exp_body.extend_from_slice(b"validate");
    exp_body.extend_from_slice(&[0x00, 0x02]); // func index 2

    wasm.push(0x07);
    wasm.push(exp_body.len() as u8);
    wasm.extend_from_slice(&exp_body);

    // 7. Code section (id 10)
    let mut code_body = Vec::new();
    code_body.push(0x00); // 0 locals

    match test_mode {
        "ed25519" => {
            // Forward arguments directly to scytale_crypto_ed25519_verify (func 0):
            // scytale_crypto_ed25519_verify(datum_ptr, datum_len, redeemer_ptr, redeemer_len, ctx_ptr, ctx_len)
            code_body.extend_from_slice(&[
                0x20, 0x00, // local.get 0
                0x20, 0x01, // local.get 1
                0x20, 0x02, // local.get 2
                0x20, 0x03, // local.get 3
                0x20, 0x04, // local.get 4
                0x20, 0x05, // local.get 5
                0x10, 0x00, // call 0 (ed25519_verify)
                0x0b,       // end
            ]);
        }
        "blake3" => {
            // Call scytale_crypto_blake3(redeemer_ptr, redeemer_len, datum_ptr + 32)
            // (hashes redeemer, writes 32B result into datum_ptr + 32)
            // Then compare datum_ptr[0..8] with (datum_ptr + 32)[0..8] (i64 comparison)
            code_body.extend_from_slice(&[
                0x20, 0x02,             // local.get 2 (redeemer_ptr)
                0x20, 0x03,             // local.get 3 (redeemer_len)
                0x20, 0x00,             // local.get 0 (datum_ptr)
                0x41, 0x20,             // i32.const 32
                0x6a,                   // i32.add (datum_ptr + 32)
                0x10, 0x01,             // call 1 (scytale_crypto_blake3)
                // Compare i64 at datum_ptr with i64 at datum_ptr + 32
                0x20, 0x00,             // local.get 0
                0x29, 0x03, 0x00,       // i64.load offset=0
                0x20, 0x00,             // local.get 0
                0x29, 0x03, 0x20,       // i64.load offset=32
                0x51,                   // i64.eq
                0x0b,                   // end
            ]);
        }
        _ => panic!("Unknown test mode"),
    }

    let mut func_code = Vec::new();
    func_code.push(code_body.len() as u8);
    func_code.extend_from_slice(&code_body);

    let mut code_sec = Vec::new();
    code_sec.push(0x01); // 1 function body
    code_sec.extend_from_slice(&func_code);

    wasm.push(0x0a);
    wasm.push(code_sec.len() as u8);
    wasm.extend_from_slice(&code_sec);

    wasm
}

fn dummy_context() -> TxContext {
    TxContext {
        tx_hash: [0x11; 32],
        block_time: 1700000000,
        input_amount: 100_000,
        fee_burned: 1_000,
    }
}

#[test]
fn test_host_function_ed25519_verify_valid_and_invalid() {
    let wasm = build_crypto_test_wasm("ed25519");

    let engine = wasmi::Engine::default();
    if let Err(e) = wasmi::Module::new(&engine, &mut &wasm[..]) {
        panic!("Wasmi module compile error: {:?}", e);
    }

    let secret = [0x55u8; 32];
    let signing_key = SigningKey::from_bytes(&secret);
    let public_key = signing_key.verifying_key().to_bytes();

    let ctx = dummy_context();
    let ctx_bytes = bincode::serialize(&ctx).unwrap();
    let signature = signing_key.sign(&ctx_bytes);
    let sig_bytes = signature.to_bytes();

    // 1. Valid signature
    let result_valid = ScyVM::execute_validator(
        &wasm,
        &public_key,
        &sig_bytes,
        &ctx,
        1_000_000,
    )
    .expect("Execution should succeed");

    assert!(result_valid.is_valid, "Valid Ed25519 signature must return is_valid=true");
    assert!(result_valid.gas_consumed >= 200, "Should consume at least 200 fuel for Ed25519 verify");

    // 2. Tampered signature
    let mut tampered_sig = sig_bytes;
    tampered_sig[0] ^= 0xff;

    let result_tampered = ScyVM::execute_validator(
        &wasm,
        &public_key,
        &tampered_sig,
        &ctx,
        1_000_000,
    )
    .expect("Execution should succeed");

    assert!(!result_tampered.is_valid, "Tampered signature must return is_valid=false");

    // 3. Wrong public key
    let wrong_pubkey = [0x12u8; 32];
    let result_wrong_pk = ScyVM::execute_validator(
        &wasm,
        &wrong_pubkey,
        &sig_bytes,
        &ctx,
        1_000_000,
    )
    .expect("Execution should succeed");

    assert!(!result_wrong_pk.is_valid, "Wrong public key must return is_valid=false");
}

#[test]
fn test_host_function_blake3_hashing() {
    let wasm = build_crypto_test_wasm("blake3");

    let payload = b"Cryptographic host function BLAKE3 execution in ScyVM sandbox";
    let expected_hash = blake3::hash(payload);

    // Prepare datum with 64 bytes: first 32 bytes = expected_hash, next 32 bytes = zero buffer for out_ptr
    let mut datum = vec![0u8; 64];
    datum[..32].copy_from_slice(expected_hash.as_bytes());

    let ctx = dummy_context();

    let result = ScyVM::execute_validator(
        &wasm,
        &datum,
        payload,
        &ctx,
        1_000_000,
    )
    .expect("Execution should succeed");

    assert!(result.is_valid, "BLAKE3 hash output must match expected digest");
    assert!(result.gas_consumed > 15, "Gas consumed should include BLAKE3 fuel");
}
