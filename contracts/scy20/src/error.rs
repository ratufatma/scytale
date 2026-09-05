use core::fmt;

#[derive(Debug, PartialEq, Eq)]
pub enum Scy20Error {
    SupplyMismatch { input: u128, output: u128 },
    InvalidTokenId,
    MissingSignature([u8; 32]),
    MaxSupplyExceeded,
    ZeroAmount,
    DeserializationFailed,
}

impl fmt::Display for Scy20Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SupplyMismatch { input, output } => {
                write!(
                    f,
                    "Jumlah token input ({input}) tidak seimbang dengan output ({output})"
                )
            }
            Self::InvalidTokenId => write!(f, "Token ID tidak cocok dengan datum yang terkunci"),
            Self::MissingSignature(addr) => write!(
                f,
                "Tanda tangan pemilik ({addr:?}) tidak valid atau tidak disertakan"
            ),
            Self::MaxSupplyExceeded => write!(f, "Jumlah minting melebihi batas suplai maksimum"),
            Self::ZeroAmount => write!(f, "Jumlah transfer harus lebih besar dari 0"),
            Self::DeserializationFailed => {
                write!(f, "Gagal melakukan deserialisasi Datum atau Redeemer")
            }
        }
    }
}
