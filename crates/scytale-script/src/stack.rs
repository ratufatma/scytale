//! LIFO Stack implementation for ScytaleScript execution.

use crate::error::ScriptError;

/// Maximum number of items allowed on the script stack.
pub const MAX_STACK_DEPTH: usize = 1024;

/// Maximum size in bytes of a single stack item.
pub const MAX_ITEM_SIZE: usize = 520;

/// LIFO execution stack for ScytaleScript.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScriptStack {
    items: Vec<Vec<u8>>,
}

impl ScriptStack {
    /// Creates a new, empty ScriptStack.
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Returns the number of items currently on the stack.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns true if the stack contains no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns a slice of the stack items (bottom to top).
    pub fn as_slice(&self) -> &[Vec<u8>] {
        &self.items
    }

    /// Pushes an item onto the stack, verifying depth and size constraints.
    pub fn push(&mut self, item: Vec<u8>) -> Result<(), ScriptError> {
        if self.items.len() >= MAX_STACK_DEPTH {
            return Err(ScriptError::StackOverflow(MAX_STACK_DEPTH));
        }
        if item.len() > MAX_ITEM_SIZE {
            return Err(ScriptError::ItemTooLarge(item.len(), MAX_ITEM_SIZE));
        }
        self.items.push(item);
        Ok(())
    }

    /// Pops the top item from the stack.
    pub fn pop(&mut self) -> Result<Vec<u8>, ScriptError> {
        self.items.pop().ok_or(ScriptError::StackUnderflow)
    }

    /// Peeks at the top item of the stack without removing it.
    pub fn peek(&self) -> Result<&Vec<u8>, ScriptError> {
        self.items.last().ok_or(ScriptError::StackUnderflow)
    }

    /// Pushes a 64-bit signed integer onto the stack in little-endian format.
    pub fn push_int(&mut self, val: i64) -> Result<(), ScriptError> {
        self.push(val.to_le_bytes().to_vec())
    }

    /// Pops a 64-bit signed integer from the stack.
    /// Supports empty vectors (evaluating to 0) and byte slices up to 8 bytes.
    pub fn pop_int(&mut self) -> Result<i64, ScriptError> {
        let item = self.pop()?;
        if item.is_empty() {
            return Ok(0);
        }
        if item.len() > 8 {
            return Err(ScriptError::InvalidInteger(item.len()));
        }
        let mut buf = [0u8; 8];
        buf[..item.len()].copy_from_slice(&item);
        Ok(i64::from_le_bytes(buf))
    }

    /// Pushes a boolean onto the stack.
    /// `true` is pushed as `vec![1]`, `false` as `vec![]`.
    pub fn push_bool(&mut self, val: bool) -> Result<(), ScriptError> {
        if val {
            self.push(vec![1])
        } else {
            self.push(vec![])
        }
    }

    /// Pops a boolean from the stack.
    /// An empty vector or a vector of all zeros evaluates to `false`.
    /// Any other non-zero byte sequence evaluates to `true`.
    pub fn pop_bool(&mut self) -> Result<bool, ScriptError> {
        let item = self.pop()?;
        if item.is_empty() || item.iter().all(|&b| b == 0) {
            Ok(false)
        } else {
            Ok(true)
        }
    }

    /// Duplicates the top stack item: `[x] -> [x, x]`.
    pub fn op_dup(&mut self) -> Result<(), ScriptError> {
        let top = self.peek()?.clone();
        self.push(top)
    }

    /// Drops the top stack item: `[x] -> []`.
    pub fn op_drop(&mut self) -> Result<(), ScriptError> {
        let _ = self.pop()?;
        Ok(())
    }

    /// Swaps the top two stack items: `[x1, x2] -> [x2, x1]`.
    pub fn op_swap(&mut self) -> Result<(), ScriptError> {
        let top = self.pop()?;
        let second = self.pop()?;
        self.push(top)?;
        self.push(second)
    }

    /// Rotates the top three stack items: `[x1, x2, x3] -> [x2, x3, x1]`.
    pub fn op_rot(&mut self) -> Result<(), ScriptError> {
        let x3 = self.pop()?;
        let x2 = self.pop()?;
        let x1 = self.pop()?;
        self.push(x2)?;
        self.push(x3)?;
        self.push(x1)
    }

    /// Duplicates the top two stack items: `[x1, x2] -> [x1, x2, x1, x2]`.
    pub fn op_2dup(&mut self) -> Result<(), ScriptError> {
        if self.items.len() < 2 {
            return Err(ScriptError::StackUnderflow);
        }
        let x2 = self.items[self.items.len() - 1].clone();
        let x1 = self.items[self.items.len() - 2].clone();
        self.push(x1)?;
        self.push(x2)
    }
}
