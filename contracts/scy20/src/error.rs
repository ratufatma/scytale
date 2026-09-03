use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum Scy20Error {
    #[error("Jumlah token input ({input}) tidak seimbang dengan output ({output})")]
    SupplyMismatch { input: u128, output: u128 },

    #[error("Token ID tidak cocok dengan datum yang terkunci")]
    InvalidTokenId,

    #[error("Tanda tangan pemilik ({0:?}) tidak valid atau tidak disertakan")]
    MissingSignature([u8; 32]),

    #[error("Jumlah minting melebihi batas suplai maksimum")]
    MaxSupplyExceeded,

    #[error("Jumlah transfer harus lebih besar dari 0")]
    ZeroAmount,

    #[error("Gagal melakukan deserialisasi Datum atau Redeemer")]
    DeserializationFailed,
}
