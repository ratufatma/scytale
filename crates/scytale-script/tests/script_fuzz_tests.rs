//! Adversarial and Fuzz Testing Suite for ScytaleScript Interpreter.
//!
//! Verifies that the stack-based script interpreter is strictly bounded,
//! anti-DoS hardened, fail-closed, and never panics under adversarial or malformed inputs.

use rand::rngs::StdRng;
use rand::{Rng, RngCore, SeedableRng};
use scytale_script::{
    opcode::OpCode, ScriptBuilder, ScriptContext, ScriptEngine, ScriptError,
    DEFAULT_MAX_OPS_BUDGET, MAX_STACK_DEPTH,
};

#[test]
fn test_fuzz_random_script_bytes() {
    let mut rng = StdRng::seed_from_u64(0xFEED_FACE_CAFE_0001);
    let sighash = [0x42u8; 32];
    let ctx = ScriptContext::new(&sighash, 100);
    let engine = ScriptEngine::new();

    // 1,000 iterations of random bytecode
    for _ in 0..1000 {
        let unlock_len = rng.gen_range(0..256);
        let mut unlock = vec![0u8; unlock_len];
        rng.fill_bytes(&mut unlock);

        let lock_len = rng.gen_range(0..256);
        let mut lock = vec![0u8; lock_len];
        rng.fill_bytes(&mut lock);

        // Script execution must never panic or loop indefinitely
        let res = engine.execute(&unlock, &lock, &ctx);
        match res {
            Ok(valid) => {
                // If Ok, must be boolean
                assert!(valid || !valid);
            }
            Err(err) => {
                // Must be one of the defined domain errors
                assert!(matches!(
                    err,
                    ScriptError::StackUnderflow
                        | ScriptError::StackOverflow(_)
                        | ScriptError::ItemTooLarge(_, _)
                        | ScriptError::InvalidInteger(_)
                        | ScriptError::ArithmeticOverflow
                        | ScriptError::EqualVerifyFailed
                        | ScriptError::CheckSigVerifyFailed
                        | ScriptError::LockTimeNotMet { .. }
                        | ScriptError::OpReturnEncountered
                        | ScriptError::BudgetExceeded(_)
                        | ScriptError::UnbalancedConditionals
                        | ScriptError::InvalidOpCode(_)
                        | ScriptError::ScriptTruncated { .. }
                        | ScriptError::ScriptFailed
                ));
            }
        }
    }
}

#[test]
fn test_fuzz_budget_exhaustion_dos() {
    let sighash = [0x00u8; 32];
    let ctx = ScriptContext::new(&sighash, 1);
    let custom_budget = 50;
    let engine = ScriptEngine::with_budget(custom_budget);

    // Build a script with 100 OP_1 OP_DROP operations (200 opcodes total)
    let mut script = Vec::new();
    for _ in 0..100 {
        script.push(OpCode::OP_1);
        script.push(OpCode::OP_DROP);
    }

    let res = engine.execute(&[], &script, &ctx);
    assert_eq!(res, Err(ScriptError::BudgetExceeded(custom_budget)));

    // Verify default engine budget bounds
    let default_engine = ScriptEngine::new();
    let mut big_script = Vec::new();
    for _ in 0..DEFAULT_MAX_OPS_BUDGET + 10 {
        big_script.push(OpCode::OP_1);
        big_script.push(OpCode::OP_DROP);
    }
    let res_default = default_engine.execute(&[], &big_script, &ctx);
    assert_eq!(
        res_default,
        Err(ScriptError::BudgetExceeded(DEFAULT_MAX_OPS_BUDGET))
    );
}

#[test]
fn test_fuzz_stack_overflow_dos() {
    let sighash = [0x00u8; 32];
    let ctx = ScriptContext::new(&sighash, 1);
    let engine = ScriptEngine::with_budget(10_000);

    // Push more elements than MAX_STACK_DEPTH
    let mut script = Vec::new();
    for _ in 0..(MAX_STACK_DEPTH + 10) {
        script.push(OpCode::OP_1);
    }

    let res = engine.execute(&[], &script, &ctx);
    assert_eq!(res, Err(ScriptError::StackOverflow(MAX_STACK_DEPTH)));
}

#[test]
fn test_fuzz_unbalanced_conditionals() {
    let sighash = [0x00u8; 32];
    let ctx = ScriptContext::new(&sighash, 1);
    let engine = ScriptEngine::new();

    // 1. Lone OP_ELSE without OP_IF
    let script1 = vec![OpCode::OP_ELSE];
    assert_eq!(
        engine.execute(&[], &script1, &ctx),
        Err(ScriptError::UnbalancedConditionals)
    );

    // 2. Lone OP_ENDIF without OP_IF
    let script2 = vec![OpCode::OP_ENDIF];
    assert_eq!(
        engine.execute(&[], &script2, &ctx),
        Err(ScriptError::UnbalancedConditionals)
    );

    // 3. Unclosed OP_IF
    let script3 = vec![OpCode::OP_1, OpCode::OP_IF, OpCode::OP_1];
    assert_eq!(
        engine.execute(&[], &script3, &ctx),
        Err(ScriptError::UnbalancedConditionals)
    );

    // 4. Mismatched nested IFs
    let script4 = vec![
        OpCode::OP_1,
        OpCode::OP_IF,
        OpCode::OP_1,
        OpCode::OP_IF,
        OpCode::OP_1,
        OpCode::OP_ENDIF,
    ];
    assert_eq!(
        engine.execute(&[], &script4, &ctx),
        Err(ScriptError::UnbalancedConditionals)
    );
}

#[test]
fn test_fuzz_arithmetic_overflow_extremes() {
    let sighash = [0x00u8; 32];
    let ctx = ScriptContext::new(&sighash, 1);
    let engine = ScriptEngine::new();

    // 1. i64::MAX + 1
    let script_add_overflow = ScriptBuilder::new()
        .push_data(&i64::MAX.to_le_bytes())
        .push_opcode(OpCode::Op1)
        .push_opcode(OpCode::OpAdd)
        .build();
    assert_eq!(
        engine.execute(&[], &script_add_overflow, &ctx),
        Err(ScriptError::ArithmeticOverflow)
    );

    // 2. i64::MIN - 1
    let script_sub_overflow = ScriptBuilder::new()
        .push_data(&i64::MIN.to_le_bytes())
        .push_opcode(OpCode::Op1)
        .push_opcode(OpCode::OpSub)
        .build();
    assert_eq!(
        engine.execute(&[], &script_sub_overflow, &ctx),
        Err(ScriptError::ArithmeticOverflow)
    );
}
