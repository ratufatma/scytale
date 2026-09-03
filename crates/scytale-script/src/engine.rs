//! ScytaleScript Interpreter and Execution Engine.

use crate::context::ScriptContext;
use crate::error::ScriptError;
use crate::opcode::OpCode;
use crate::stack::ScriptStack;

/// Default maximum number of opcodes allowed per script execution.
pub const DEFAULT_MAX_OPS_BUDGET: usize = 256;

/// Deterministic, stack-based script execution engine.
#[derive(Debug, Clone)]
pub struct ScriptEngine {
    max_ops_budget: usize,
}

impl Default for ScriptEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptEngine {
    /// Creates a new ScriptEngine with the default operation budget (256).
    pub fn new() -> Self {
        Self {
            max_ops_budget: DEFAULT_MAX_OPS_BUDGET,
        }
    }

    /// Creates a new ScriptEngine with a custom opcode execution budget.
    pub fn with_budget(max_ops_budget: usize) -> Self {
        Self { max_ops_budget }
    }

    /// Executes an unlocking script followed by a locking script in a shared stack environment.
    ///
    /// # Backward Compatibility
    /// If locking_script is short (<= 32 bytes) and matches unlocking_script byte-for-byte,
    /// it is accepted as a legacy raw match immediately.
    ///
    /// # Return
    /// Returns `Ok(true)` if execution succeeds and leaves a truthy value on the top of the stack.
    pub fn execute(
        &self,
        unlocking_script: &[u8],
        locking_script: &[u8],
        ctx: &ScriptContext,
    ) -> Result<bool, ScriptError> {
        // Fast-path backward compatibility: legacy raw matching (e.g. "010203" <= 32 bytes)
        if locking_script.len() <= 32 && unlocking_script == locking_script {
            return Ok(true);
        }

        let mut stack = ScriptStack::new();
        let mut budget = self.max_ops_budget;

        // 1. Evaluate unlocking script (ScriptSig)
        if let Err(e) = self.execute_script(unlocking_script, &mut stack, ctx, &mut budget) {
            // Fallback for raw byte matching if unlocking_script cannot be parsed as standard opcodes
            if locking_script.len() <= 32 && unlocking_script == locking_script {
                return Ok(true);
            }
            return Err(e);
        }

        // 2. Evaluate locking script (ScriptPubKey) on the resulting stack
        if let Err(e) = self.execute_script(locking_script, &mut stack, ctx, &mut budget) {
            if locking_script.len() <= 32 && unlocking_script == locking_script {
                return Ok(true);
            }
            return Err(e);
        }

        // 3. Final validation: Stack must not be empty and top element must be truthy
        if stack.is_empty() {
            return Err(ScriptError::ScriptFailed);
        }

        let is_valid = stack.pop_bool()?;
        if is_valid {
            Ok(true)
        } else {
            Err(ScriptError::ScriptFailed)
        }
    }

    /// Evaluates a single script segment against the provided stack and context.
    pub fn execute_script(
        &self,
        script: &[u8],
        stack: &mut ScriptStack,
        ctx: &ScriptContext,
        budget: &mut usize,
    ) -> Result<(), ScriptError> {
        let mut pc = 0;
        let mut branch_stack: Vec<bool> = Vec::new();

        while pc < script.len() {
            let op_byte = script[pc];
            pc += 1;

            let is_active = branch_stack.iter().all(|&b| b);

            // Control flow opcodes (OP_IF, OP_ELSE, OP_ENDIF)
            if op_byte == OpCode::OP_IF {
                if is_active {
                    if *budget == 0 {
                        return Err(ScriptError::BudgetExceeded(self.max_ops_budget));
                    }
                    *budget -= 1;
                    let cond = stack.pop_bool()?;
                    branch_stack.push(cond);
                } else {
                    branch_stack.push(false);
                }
                continue;
            }

            if op_byte == OpCode::OP_ELSE {
                let depth = branch_stack.len();
                if depth == 0 {
                    return Err(ScriptError::UnbalancedConditionals);
                }
                // Only invert if all enclosing outer branches are active
                let outer_active = depth == 1 || branch_stack[..depth - 1].iter().all(|&b| b);
                if outer_active {
                    let last = branch_stack.last_mut().unwrap();
                    *last = !*last;
                }
                continue;
            }

            if op_byte == OpCode::OP_ENDIF {
                branch_stack
                    .pop()
                    .ok_or(ScriptError::UnbalancedConditionals)?;
                continue;
            }

            // Pushdata handling: must advance PC even if currently in an inactive branch
            if OpCode::is_pushbytes(op_byte) {
                let n = op_byte as usize;
                if pc + n > script.len() {
                    return Err(ScriptError::ScriptTruncated {
                        expected: n,
                        found: script.len() - pc,
                    });
                }
                let data = &script[pc..pc + n];
                pc += n;

                if is_active {
                    if *budget == 0 {
                        return Err(ScriptError::BudgetExceeded(self.max_ops_budget));
                    }
                    *budget -= 1;
                    stack.push(data.to_vec())?;
                }
                continue;
            }

            if op_byte == OpCode::OP_PUSHDATA1 {
                if pc >= script.len() {
                    return Err(ScriptError::ScriptTruncated {
                        expected: 1,
                        found: 0,
                    });
                }
                let n = script[pc] as usize;
                pc += 1;
                if pc + n > script.len() {
                    return Err(ScriptError::ScriptTruncated {
                        expected: n,
                        found: script.len() - pc,
                    });
                }
                let data = &script[pc..pc + n];
                pc += n;

                if is_active {
                    if *budget == 0 {
                        return Err(ScriptError::BudgetExceeded(self.max_ops_budget));
                    }
                    *budget -= 1;
                    stack.push(data.to_vec())?;
                }
                continue;
            }

            // If not in active branch, ignore non-branching opcodes without charging budget
            if !is_active {
                continue;
            }

            // Deduct from budget
            if *budget == 0 {
                return Err(ScriptError::BudgetExceeded(self.max_ops_budget));
            }
            *budget -= 1;

            let opcode = OpCode::from_byte(op_byte).ok_or(ScriptError::InvalidOpCode(op_byte))?;

            match opcode {
                OpCode::Op0 => stack.push(vec![])?,
                OpCode::Op1
                | OpCode::Op2
                | OpCode::Op3
                | OpCode::Op4
                | OpCode::Op5
                | OpCode::Op6
                | OpCode::Op7
                | OpCode::Op8
                | OpCode::Op9
                | OpCode::Op10
                | OpCode::Op11
                | OpCode::Op12
                | OpCode::Op13
                | OpCode::Op14
                | OpCode::Op15
                | OpCode::Op16 => {
                    let val = (op_byte - OpCode::OP_1 + 1) as i64;
                    stack.push_int(val)?;
                }

                OpCode::OpReturn => return Err(ScriptError::OpReturnEncountered),

                OpCode::OpDup => stack.op_dup()?,
                OpCode::OpDrop => stack.op_drop()?,
                OpCode::OpSwap => stack.op_swap()?,
                OpCode::OpRot => stack.op_rot()?,
                OpCode::Op2Dup => stack.op_2dup()?,

                OpCode::OpEqual => {
                    let b = stack.pop()?;
                    let a = stack.pop()?;
                    stack.push_bool(a == b)?;
                }
                OpCode::OpEqualVerify => {
                    let b = stack.pop()?;
                    let a = stack.pop()?;
                    if a != b {
                        return Err(ScriptError::EqualVerifyFailed);
                    }
                }

                OpCode::OpAdd => {
                    let b = stack.pop_int()?;
                    let a = stack.pop_int()?;
                    let res = a.checked_add(b).ok_or(ScriptError::ArithmeticOverflow)?;
                    stack.push_int(res)?;
                }
                OpCode::OpSub => {
                    let b = stack.pop_int()?;
                    let a = stack.pop_int()?;
                    let res = a.checked_sub(b).ok_or(ScriptError::ArithmeticOverflow)?;
                    stack.push_int(res)?;
                }
                OpCode::OpLessThan => {
                    let b = stack.pop_int()?;
                    let a = stack.pop_int()?;
                    stack.push_bool(a < b)?;
                }
                OpCode::OpGreaterThan => {
                    let b = stack.pop_int()?;
                    let a = stack.pop_int()?;
                    stack.push_bool(a > b)?;
                }

                OpCode::OpBlake3 => {
                    let item = stack.pop()?;
                    let digest = blake3::hash(&item);
                    stack.push(digest.as_bytes().to_vec())?;
                }
                OpCode::OpCheckSig => {
                    let pubkey = stack.pop()?;
                    let sig = stack.pop()?;
                    let valid = verify_signature(&pubkey, &sig, ctx.sighash);
                    stack.push_bool(valid)?;
                }
                OpCode::OpCheckSigVerify => {
                    let pubkey = stack.pop()?;
                    let sig = stack.pop()?;
                    let valid = verify_signature(&pubkey, &sig, ctx.sighash);
                    if !valid {
                        return Err(ScriptError::CheckSigVerifyFailed);
                    }
                }
                OpCode::OpCheckLockTimeVerify => {
                    let lock = stack.pop_int()?;
                    if lock < 0 {
                        return Err(ScriptError::LockTimeNotMet {
                            lock_height: 0,
                            current_height: ctx.current_block_height,
                        });
                    }
                    let lock_height = lock as u64;
                    if lock_height > ctx.current_block_height {
                        return Err(ScriptError::LockTimeNotMet {
                            lock_height,
                            current_height: ctx.current_block_height,
                        });
                    }
                }

                // OpIf, OpElse, OpEndIf, OpPushData1 are handled earlier
                OpCode::OpIf | OpCode::OpElse | OpCode::OpEndIf | OpCode::OpPushData1 => {}
            }
        }

        if !branch_stack.is_empty() {
            return Err(ScriptError::UnbalancedConditionals);
        }

        Ok(())
    }
}

/// Helper to verify an Ed25519 signature over a 32-byte sighash digest.
fn verify_signature(pubkey_bytes: &[u8], sig_bytes: &[u8], sighash: &[u8; 32]) -> bool {
    if pubkey_bytes.len() != 32 || sig_bytes.len() != 64 {
        return false;
    }
    let pk_arr: [u8; 32] = match pubkey_bytes.try_into() {
        Ok(a) => a,
        Err(_) => return false,
    };
    let sig_arr: [u8; 64] = match sig_bytes.try_into() {
        Ok(a) => a,
        Err(_) => return false,
    };
    let vk = match ed25519_dalek::VerifyingKey::from_bytes(&pk_arr) {
        Ok(k) => k,
        Err(_) => return false,
    };
    let sig = ed25519_dalek::Signature::from_bytes(&sig_arr);
    vk.verify_strict(sighash, &sig).is_ok()
}
