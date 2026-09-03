//! OpCode definitions for ScytaleScript.

/// OpCodes supported by the ScytaleScript stack-based interpreter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum OpCode {
    // --- Constants & Push ---
    /// Push empty vector onto stack.
    Op0 = 0x00,

    /// Push 1-byte length prefix data.
    OpPushData1 = 0x4c,

    /// Push small integers 1 through 16.
    Op1 = 0x51,
    Op2 = 0x52,
    Op3 = 0x53,
    Op4 = 0x54,
    Op5 = 0x55,
    Op6 = 0x56,
    Op7 = 0x57,
    Op8 = 0x58,
    Op9 = 0x59,
    Op10 = 0x5a,
    Op11 = 0x5b,
    Op12 = 0x5c,
    Op13 = 0x5d,
    Op14 = 0x5e,
    Op15 = 0x5f,
    Op16 = 0x60,

    // --- Control Flow ---
    /// If top stack item is true, execute branch.
    OpIf = 0x63,
    /// Invert branch execution.
    OpElse = 0x67,
    /// End of conditional block.
    OpEndIf = 0x68,
    /// Mark transaction output as provably unspendable.
    OpReturn = 0x6a,

    // --- Stack Operations ---
    /// Duplicate the top stack item.
    OpDup = 0x73,
    /// Drop the top stack item.
    OpDrop = 0x75,
    /// Swap the top two stack items.
    OpSwap = 0x76,
    /// Rotate top three stack items [x1, x2, x3] -> [x2, x3, x1].
    OpRot = 0x77,
    /// Duplicate top two stack items [x1, x2] -> [x1, x2, x1, x2].
    Op2Dup = 0x78,

    // --- Logic & Comparison ---
    /// Pushes 1 if the top two items are equal, 0 otherwise.
    OpEqual = 0x87,
    /// OpEqual, then verify true; aborts if false.
    OpEqualVerify = 0x88,

    // --- Math (i64 Checked) ---
    /// Pop b, pop a, push a + b.
    OpAdd = 0x93,
    /// Pop b, pop a, push a - b.
    OpSub = 0x94,
    /// Pop b, pop a, push 1 if a < b else 0.
    OpLessThan = 0x9b,
    /// Pop b, pop a, push 1 if a > b else 0.
    OpGreaterThan = 0x9c,

    // --- Crypto & Verification ---
    /// Pop element, hash with BLAKE3, push 32-byte digest.
    OpBlake3 = 0xa0,
    /// Pop 32-byte pubkey, pop 64-byte signature, verify against sighash.
    OpCheckSig = 0xac,
    /// OpCheckSig, then verify true; aborts if false.
    OpCheckSigVerify = 0xad,
    /// Pop lock_height. If lock_height > current_height, fail.
    OpCheckLockTimeVerify = 0xb1,
}

impl OpCode {
    /// Associated constant aliases matching the specification names.
    pub const OP_0: u8 = 0x00;
    pub const OP_PUSHDATA1: u8 = 0x4c;
    pub const OP_1: u8 = 0x51;
    pub const OP_2: u8 = 0x52;
    pub const OP_3: u8 = 0x53;
    pub const OP_4: u8 = 0x54;
    pub const OP_5: u8 = 0x55;
    pub const OP_6: u8 = 0x56;
    pub const OP_7: u8 = 0x57;
    pub const OP_8: u8 = 0x58;
    pub const OP_9: u8 = 0x59;
    pub const OP_10: u8 = 0x5a;
    pub const OP_11: u8 = 0x5b;
    pub const OP_12: u8 = 0x5c;
    pub const OP_13: u8 = 0x5d;
    pub const OP_14: u8 = 0x5e;
    pub const OP_15: u8 = 0x5f;
    pub const OP_16: u8 = 0x60;

    pub const OP_IF: u8 = 0x63;
    pub const OP_ELSE: u8 = 0x67;
    pub const OP_ENDIF: u8 = 0x68;
    pub const OP_RETURN: u8 = 0x6a;

    pub const OP_DUP: u8 = 0x73;
    pub const OP_DROP: u8 = 0x75;
    pub const OP_SWAP: u8 = 0x76;
    pub const OP_ROT: u8 = 0x77;
    pub const OP_2DUP: u8 = 0x78;

    pub const OP_EQUAL: u8 = 0x87;
    pub const OP_EQUALVERIFY: u8 = 0x88;
    pub const OP_ADD: u8 = 0x93;
    pub const OP_SUB: u8 = 0x94;
    pub const OP_LESSTHAN: u8 = 0x9b;
    pub const OP_GREATERTHAN: u8 = 0x9c;

    pub const OP_BLAKE3: u8 = 0xa0;
    pub const OP_CHECKSIG: u8 = 0xac;
    pub const OP_CHECKSIGVERIFY: u8 = 0xad;
    pub const OP_CHECKLOCKTIMEVERIFY: u8 = 0xb1;

    /// Checks if a byte represents an OP_PUSHBYTES_N instruction (1..=75).
    pub const fn is_pushbytes(byte: u8) -> bool {
        byte >= 0x01 && byte <= 0x4b
    }

    /// Converts byte to OpCode if known.
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x00 => Some(Self::Op0),
            0x4c => Some(Self::OpPushData1),
            0x51 => Some(Self::Op1),
            0x52 => Some(Self::Op2),
            0x53 => Some(Self::Op3),
            0x54 => Some(Self::Op4),
            0x55 => Some(Self::Op5),
            0x56 => Some(Self::Op6),
            0x57 => Some(Self::Op7),
            0x58 => Some(Self::Op8),
            0x59 => Some(Self::Op9),
            0x5a => Some(Self::Op10),
            0x5b => Some(Self::Op11),
            0x5c => Some(Self::Op12),
            0x5d => Some(Self::Op13),
            0x5e => Some(Self::Op14),
            0x5f => Some(Self::Op15),
            0x60 => Some(Self::Op16),

            0x63 => Some(Self::OpIf),
            0x67 => Some(Self::OpElse),
            0x68 => Some(Self::OpEndIf),
            0x6a => Some(Self::OpReturn),

            0x73 => Some(Self::OpDup),
            0x75 => Some(Self::OpDrop),
            0x76 => Some(Self::OpSwap),
            0x77 => Some(Self::OpRot),
            0x78 => Some(Self::Op2Dup),

            0x87 => Some(Self::OpEqual),
            0x88 => Some(Self::OpEqualVerify),
            0x93 => Some(Self::OpAdd),
            0x94 => Some(Self::OpSub),
            0x9b => Some(Self::OpLessThan),
            0x9c => Some(Self::OpGreaterThan),

            0xa0 => Some(Self::OpBlake3),
            0xac => Some(Self::OpCheckSig),
            0xad => Some(Self::OpCheckSigVerify),
            0xb1 => Some(Self::OpCheckLockTimeVerify),
            _ => None,
        }
    }

    /// Returns the raw byte value of the opcode.
    pub const fn as_byte(&self) -> u8 {
        *self as u8
    }
}
