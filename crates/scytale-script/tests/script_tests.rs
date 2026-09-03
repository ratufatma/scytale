use ed25519_dalek::Signer;
use scytale_script::{
    builder::ScriptBuilder,
    context::ScriptContext,
    engine::{ScriptEngine, DEFAULT_MAX_OPS_BUDGET},
    error::ScriptError,
    opcode::OpCode,
    stack::ScriptStack,
};

#[test]
fn test_legacy_raw_matching() {
    let engine = ScriptEngine::new();
    let sighash = [0u8; 32];
    let ctx = ScriptContext::new(&sighash, 100);

    // 1. Exact raw match for 010203 (3 bytes <= 32 bytes)
    let legacy_script = vec![0x01, 0x02, 0x03];
    assert!(engine
        .execute(&legacy_script, &legacy_script, &ctx)
        .expect("legacy match should pass"));

    // 2. Mismatched raw bytes fails
    let wrong_script = vec![0x01, 0x02, 0x04];
    assert!(engine.execute(&wrong_script, &legacy_script, &ctx).is_err());
}

#[test]
fn test_arithmetic_ops() {
    let engine = ScriptEngine::new();
    let sighash = [0u8; 32];
    let ctx = ScriptContext::new(&sighash, 0);

    // 1. OP_ADD: 5 + 7 = 12
    let locking = ScriptBuilder::new()
        .push_opcode(OpCode::OpAdd)
        .push_int(12)
        .push_opcode(OpCode::OpEqual)
        .build();
    let unlocking = ScriptBuilder::new().push_int(5).push_int(7).build();
    assert!(engine.execute(&unlocking, &locking, &ctx).unwrap());

    // 2. OP_SUB: 20 - 8 = 12
    let locking_sub = ScriptBuilder::new()
        .push_opcode(OpCode::OpSub)
        .push_int(12)
        .push_opcode(OpCode::OpEqual)
        .build();
    let unlocking_sub = ScriptBuilder::new().push_int(20).push_int(8).build();
    assert!(engine.execute(&unlocking_sub, &locking_sub, &ctx).unwrap());

    // 3. Comparison: 5 < 10 (true), 10 > 5 (true)
    let comp_script = ScriptBuilder::new()
        .push_int(5)
        .push_int(10)
        .push_opcode(OpCode::OpLessThan)
        .push_int(10)
        .push_int(5)
        .push_opcode(OpCode::OpGreaterThan)
        .push_opcode(OpCode::OpEqual)
        .build();
    assert!(engine.execute(&[], &comp_script, &ctx).unwrap());

    // 4. Arithmetic overflow protection
    let overflow_script = ScriptBuilder::new()
        .push_int(i64::MAX)
        .push_int(1)
        .push_opcode(OpCode::OpAdd)
        .build();
    let err = engine.execute(&[], &overflow_script, &ctx).unwrap_err();
    assert_eq!(err, ScriptError::ArithmeticOverflow);
}

#[test]
fn test_stack_manipulation() {
    let engine = ScriptEngine::new();
    let sighash = [0u8; 32];
    let ctx = ScriptContext::new(&sighash, 0);

    // 1. OP_DUP: [42] -> [42, 42] -> OP_EQUAL -> [1]
    let dup_script = ScriptBuilder::new()
        .push_int(42)
        .push_opcode(OpCode::OpDup)
        .push_opcode(OpCode::OpEqual)
        .build();
    assert!(engine.execute(&[], &dup_script, &ctx).unwrap());

    // 2. OP_SWAP & OP_DROP: [1, 2] -> swap [2, 1] -> drop [2] -> check equal 2
    let swap_drop_script = ScriptBuilder::new()
        .push_int(1)
        .push_int(2)
        .push_opcode(OpCode::OpSwap)
        .push_opcode(OpCode::OpDrop)
        .push_int(2)
        .push_opcode(OpCode::OpEqual)
        .build();
    assert!(engine.execute(&[], &swap_drop_script, &ctx).unwrap());

    // 3. OP_ROT: [1, 2, 3] -> [2, 3, 1] -> top is 1
    let rot_script = ScriptBuilder::new()
        .push_int(1)
        .push_int(2)
        .push_int(3)
        .push_opcode(OpCode::OpRot)
        .push_int(1)
        .push_opcode(OpCode::OpEqual)
        .build();
    assert!(engine.execute(&[], &rot_script, &ctx).unwrap());

    // 4. OP_2DUP: [1, 2] -> [1, 2, 1, 2]
    let dup2_script = ScriptBuilder::new()
        .push_int(1)
        .push_int(2)
        .push_opcode(OpCode::Op2Dup)
        .push_opcode(OpCode::OpDrop)
        .push_int(1)
        .push_opcode(OpCode::OpEqual)
        .build();
    assert!(engine.execute(&[], &dup2_script, &ctx).unwrap());
}

#[test]
fn test_blake3_p2pkh_simulation() {
    let engine = ScriptEngine::new();
    let sighash = [7u8; 32];
    let ctx = ScriptContext::new(&sighash, 50);

    // Generate real Ed25519 keypair
    let secret = [0x5au8; 32];
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&secret);
    let verifying_key = signing_key.verifying_key();
    let pubkey_bytes = verifying_key.to_bytes();
    let pubkey_hash = blake3::hash(&pubkey_bytes);

    // Sign the transaction sighash
    let signature = signing_key.sign(&sighash);
    let signature_bytes = signature.to_bytes();

    // Locking script (P2PKH): OP_DUP OP_BLAKE3 <pubkey_hash> OP_EQUALVERIFY OP_CHECKSIG
    let locking_script = ScriptBuilder::new()
        .push_opcode(OpCode::OpDup)
        .push_opcode(OpCode::OpBlake3)
        .push_data(pubkey_hash.as_bytes())
        .push_opcode(OpCode::OpEqualVerify)
        .push_opcode(OpCode::OpCheckSig)
        .build();

    // Unlocking script (ScriptSig): <sig> <pubkey>
    let unlocking_script = ScriptBuilder::new()
        .push_data(&signature_bytes)
        .push_data(&pubkey_bytes)
        .build();

    // Evaluation must succeed
    assert!(engine
        .execute(&unlocking_script, &locking_script, &ctx)
        .expect("valid P2PKH script should succeed"));

    // Tampered sighash must fail
    let tampered_sighash = [8u8; 32];
    let bad_ctx = ScriptContext::new(&tampered_sighash, 50);
    assert!(engine
        .execute(&unlocking_script, &locking_script, &bad_ctx)
        .is_err());
}

#[test]
fn test_timelock_verify() {
    let engine = ScriptEngine::new();
    let sighash = [0u8; 32];

    // CLTV script: 100 OP_CHECKLOCKTIMEVERIFY OP_1
    let cltv_script = ScriptBuilder::new()
        .push_int(100)
        .push_opcode(OpCode::OpCheckLockTimeVerify)
        .push_opcode(OpCode::Op1)
        .build();

    // 1. Current block height 100 >= 100 -> PASS
    let ctx_valid = ScriptContext::new(&sighash, 100);
    assert!(engine.execute(&[], &cltv_script, &ctx_valid).unwrap());

    // 2. Current block height 150 > 100 -> PASS
    let ctx_future = ScriptContext::new(&sighash, 150);
    assert!(engine.execute(&[], &cltv_script, &ctx_future).unwrap());

    // 3. Current block height 99 < 100 -> FAIL
    let ctx_premature = ScriptContext::new(&sighash, 99);
    let err = engine
        .execute(&[], &cltv_script, &ctx_premature)
        .unwrap_err();
    assert_eq!(
        err,
        ScriptError::LockTimeNotMet {
            lock_height: 100,
            current_height: 99,
        }
    );
}

#[test]
fn test_conditional_if_else() {
    let engine = ScriptEngine::new();
    let sighash = [0u8; 32];
    let ctx = ScriptContext::new(&sighash, 0);

    // Script: OP_IF 10 OP_ELSE 20 OP_ENDIF
    let script = ScriptBuilder::new()
        .push_opcode(OpCode::OpIf)
        .push_int(10)
        .push_opcode(OpCode::OpElse)
        .push_int(20)
        .push_opcode(OpCode::OpEndIf)
        .build();

    // True branch: pushes 10
    let true_test = ScriptBuilder::new()
        .push_int(1)
        .push_raw(&script)
        .push_int(10)
        .push_opcode(OpCode::OpEqual)
        .build();
    assert!(engine.execute(&[], &true_test, &ctx).unwrap());

    // False branch: pushes 20
    let false_test = ScriptBuilder::new()
        .push_int(0)
        .push_raw(&script)
        .push_int(20)
        .push_opcode(OpCode::OpEqual)
        .build();
    assert!(engine.execute(&[], &false_test, &ctx).unwrap());
}

#[test]
fn test_budget_exceeded() {
    let engine = ScriptEngine::with_budget(DEFAULT_MAX_OPS_BUDGET);
    let sighash = [0u8; 32];
    let ctx = ScriptContext::new(&sighash, 0);

    // Build a script with 300 OP_DUP opcodes
    let mut builder = ScriptBuilder::new().push_int(1);
    for _ in 0..300 {
        builder = builder.push_opcode(OpCode::OpDup);
    }
    let script = builder.build();

    let err = engine.execute(&[], &script, &ctx).unwrap_err();
    assert_eq!(err, ScriptError::BudgetExceeded(DEFAULT_MAX_OPS_BUDGET));
}

#[test]
fn test_op_return_unspendable() {
    let engine = ScriptEngine::new();
    let sighash = [0u8; 32];
    let ctx = ScriptContext::new(&sighash, 0);

    let script = ScriptBuilder::new()
        .push_opcode(OpCode::OpReturn)
        .push_data(b"Burned data commitment")
        .build();

    let err = engine.execute(&[], &script, &ctx).unwrap_err();
    assert_eq!(err, ScriptError::OpReturnEncountered);
}

#[test]
fn test_stack_depth_limit() {
    let mut stack = ScriptStack::new();
    for _ in 0..1024 {
        stack.push(vec![1]).unwrap();
    }
    let err = stack.push(vec![1]).unwrap_err();
    assert_eq!(err, ScriptError::StackOverflow(1024));
}
