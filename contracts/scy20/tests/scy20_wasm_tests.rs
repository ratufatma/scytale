use ed25519_dalek::{Signer, SigningKey};
use scy20::codec::{serialize_datum, serialize_redeemer};
use scy20::{Address, Scy20Datum, Scy20Redeemer, TokenId, TokenMetadata, TxContext};
use scytale_vm::ScyVM;
use std::process::Command;

fn load_scy20_wasm() -> Vec<u8> {
    // 1. Compile contract Wasm to target wasm32-unknown-unknown release
    let status = Command::new("cargo")
        .args([
            "build",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
            "-p",
            "scy20",
        ])
        .status()
        .expect("Gagal mengompilasi kontrak scy20 Wasm");
    assert!(status.success(), "Kompilasi scy20 Wasm harus sukses");

    std::fs::read("../../target/wasm32-unknown-unknown/release/scy20.wasm")
        .or_else(|_| std::fs::read("target/wasm32-unknown-unknown/release/scy20.wasm"))
        .expect("File scy20.wasm tidak ditemukan")
}

fn make_keypair(seed: u8) -> (SigningKey, Address) {
    let key = SigningKey::from_bytes(&[seed; 32]);
    let addr = key.verifying_key().to_bytes();
    (key, addr)
}

fn make_context(hash_byte: u8) -> TxContext {
    TxContext {
        tx_hash: [hash_byte; 32],
        block_time: 1_750_000_000,
        input_amount: 50_000,
        fee_burned: 500,
    }
}

#[test]
fn test_scy20_wasm_mint_authorized_and_unauthorized() {
    let wasm_bytes = load_scy20_wasm();
    let token_id: TokenId = [0x5A; 32];

    let (issuer_key, issuer_addr) = make_keypair(0x10);
    let (mallory_key, _mallory_addr) = make_keypair(0x99);
    let (_, alice_addr) = make_keypair(0x20);

    let anchor_datum = Scy20Datum {
        token_id,
        owner: issuer_addr,
        amount: 0,
    };
    let datum_bytes = serialize_datum(&anchor_datum).unwrap();

    let metadata = TokenMetadata {
        name: "Scytale USD".to_string(),
        symbol: "sUSD".to_string(),
        decimals: 6,
        max_supply: Some(1_000_000),
    };

    let ctx = make_context(0xA1);

    // 1. Authorized Mint: Minter resmi menandatangani transaksi
    let valid_mint_sig = issuer_key.sign(&ctx.tx_hash).to_bytes();
    let valid_mint_redeemer = Scy20Redeemer::Mint {
        amount: 500_000,
        signature: valid_mint_sig,
        outputs: vec![Scy20Datum {
            token_id,
            owner: alice_addr,
            amount: 500_000,
        }],
        metadata: Some(metadata.clone()),
        current_supply: 0,
    };
    let valid_redeemer_bytes = serialize_redeemer(&valid_mint_redeemer).unwrap();

    let res_valid = ScyVM::execute_validator(
        &wasm_bytes,
        &datum_bytes,
        &valid_redeemer_bytes,
        &ctx,
        1_000_000,
    )
    .expect("ScyVM execution should succeed");

    assert!(
        res_valid.is_valid,
        "Pencetakan token sah oleh minter resmi harus diterima (VALIDATION_SUCCESS)"
    );
    assert!(res_valid.gas_consumed > 0, "Gas harus terhitung");

    // 2. Unauthorized Mint: Mallory mencoba mencetak token tanpa izin
    let unauthorized_sig = mallory_key.sign(&ctx.tx_hash).to_bytes();
    let unauthorized_mint_redeemer = Scy20Redeemer::Mint {
        amount: 500_000,
        signature: unauthorized_sig,
        outputs: vec![Scy20Datum {
            token_id,
            owner: alice_addr,
            amount: 500_000,
        }],
        metadata: Some(metadata),
        current_supply: 0,
    };
    let unauth_redeemer_bytes = serialize_redeemer(&unauthorized_mint_redeemer).unwrap();

    let res_unauth = ScyVM::execute_validator(
        &wasm_bytes,
        &datum_bytes,
        &unauth_redeemer_bytes,
        &ctx,
        1_000_000,
    )
    .expect("ScyVM execution should succeed");

    assert!(
        !res_unauth.is_valid,
        "Pencetakan tanpa hak harus ditolak (VALIDATION_REJECT)"
    );
}

#[test]
fn test_scy20_wasm_transfer_conservation_and_balance_duplication() {
    let wasm_bytes = load_scy20_wasm();
    let token_id: TokenId = [0x5A; 32];

    let (alice_key, alice_addr) = make_keypair(0x20);
    let (_, bob_addr) = make_keypair(0x30);

    let alice_input_datum = Scy20Datum {
        token_id,
        owner: alice_addr,
        amount: 100_000,
    };
    let datum_bytes = serialize_datum(&alice_input_datum).unwrap();

    let ctx = make_context(0xB2);

    // 1. Valid Transfer: Input (100,000) = Output Bob (40,000) + Change Alice (59,000) + Fee (1,000)
    let valid_sig = alice_key.sign(&ctx.tx_hash).to_bytes();
    let valid_transfer_redeemer = Scy20Redeemer::Transfer {
        signature: valid_sig,
        outputs: vec![
            Scy20Datum {
                token_id,
                owner: bob_addr,
                amount: 40_000,
            },
            Scy20Datum {
                token_id,
                owner: alice_addr,
                amount: 59_000,
            },
        ],
        fee: 1_000,
    };
    let valid_transfer_bytes = serialize_redeemer(&valid_transfer_redeemer).unwrap();

    let res_valid = ScyVM::execute_validator(
        &wasm_bytes,
        &datum_bytes,
        &valid_transfer_bytes,
        &ctx,
        1_000_000,
    )
    .expect("ScyVM execution should succeed");

    assert!(
        res_valid.is_valid,
        "Transfer dengan konservasi saldo dan tanda tangan valid harus diterima (VALIDATION_SUCCESS)"
    );

    // 2. Balance Duplication Attempt: Mencoba menggandakan saldo Output (150,000) > Input (100,000)
    let dup_sig = alice_key.sign(&ctx.tx_hash).to_bytes();
    let duplication_redeemer = Scy20Redeemer::Transfer {
        signature: dup_sig,
        outputs: vec![
            Scy20Datum {
                token_id,
                owner: bob_addr,
                amount: 80_000,
            },
            Scy20Datum {
                token_id,
                owner: alice_addr,
                amount: 70_000,
            },
        ],
        fee: 0,
    };
    let dup_transfer_bytes = serialize_redeemer(&duplication_redeemer).unwrap();

    let res_dup = ScyVM::execute_validator(
        &wasm_bytes,
        &datum_bytes,
        &dup_transfer_bytes,
        &ctx,
        1_000_000,
    )
    .expect("ScyVM execution should succeed");

    assert!(
        !res_dup.is_valid,
        "Transfer yang mencoba menggandakan saldo (Output > Input) harus ditolak (VALIDATION_REJECT)"
    );

    // 3. Tampered Signature Attempt: Tanda tangan rusak
    let mut tampered_sig = valid_sig;
    tampered_sig[0] ^= 0xff;
    let tampered_redeemer = Scy20Redeemer::Transfer {
        signature: tampered_sig,
        outputs: vec![Scy20Datum {
            token_id,
            owner: bob_addr,
            amount: 100_000,
        }],
        fee: 0,
    };
    let tampered_bytes = serialize_redeemer(&tampered_redeemer).unwrap();

    let res_tampered = ScyVM::execute_validator(
        &wasm_bytes,
        &datum_bytes,
        &tampered_bytes,
        &ctx,
        1_000_000,
    )
    .expect("ScyVM execution should succeed");

    assert!(
        !res_tampered.is_valid,
        "Transfer dengan tanda tangan yang tidak valid harus ditolak (VALIDATION_REJECT)"
    );
}

#[test]
fn test_scy20_wasm_burn_token_supply() {
    let wasm_bytes = load_scy20_wasm();
    let token_id: TokenId = [0x5A; 32];

    let (alice_key, alice_addr) = make_keypair(0x20);

    let alice_input_datum = Scy20Datum {
        token_id,
        owner: alice_addr,
        amount: 50_000,
    };
    let datum_bytes = serialize_datum(&alice_input_datum).unwrap();

    let ctx = make_context(0xC3);

    // 1. Valid Burn: Membakar 20,000 token, sisa 30,000 token kembali ke Alice
    let burn_sig = alice_key.sign(&ctx.tx_hash).to_bytes();
    let valid_burn_redeemer = Scy20Redeemer::Burn {
        amount: 20_000,
        signature: burn_sig,
        outputs: vec![Scy20Datum {
            token_id,
            owner: alice_addr,
            amount: 30_000,
        }],
    };
    let valid_burn_bytes = serialize_redeemer(&valid_burn_redeemer).unwrap();

    let res_valid = ScyVM::execute_validator(
        &wasm_bytes,
        &datum_bytes,
        &valid_burn_bytes,
        &ctx,
        1_000_000,
    )
    .expect("ScyVM execution should succeed");

    assert!(
        res_valid.is_valid,
        "Pembakaran token yang sah harus diterima (VALIDATION_SUCCESS)"
    );

    // 2. Mismatched Burn: Mengklaim membakar 20,000 tetapi output sisa 40,000 (total 60,000 != input 50,000)
    let mismatch_burn_redeemer = Scy20Redeemer::Burn {
        amount: 20_000,
        signature: burn_sig,
        outputs: vec![Scy20Datum {
            token_id,
            owner: alice_addr,
            amount: 40_000,
        }],
    };
    let mismatch_burn_bytes = serialize_redeemer(&mismatch_burn_redeemer).unwrap();

    let res_mismatch = ScyVM::execute_validator(
        &wasm_bytes,
        &datum_bytes,
        &mismatch_burn_bytes,
        &ctx,
        1_000_000,
    )
    .expect("ScyVM execution should succeed");

    assert!(
        !res_mismatch.is_valid,
        "Pembakaran token yang tidak seimbang harus ditolak (VALIDATION_REJECT)"
    );
}
