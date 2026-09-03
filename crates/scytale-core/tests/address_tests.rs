use scytale_core::{Address, AddressError};

#[test]
fn test_bech32_encode_decode_roundtrip() {
    let hash: [u8; 32] = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e,
        0x1f, 0x20,
    ];

    let addr = Address::new(hash);
    let bech32_str = addr.to_bech32().expect("bech32 encoding should succeed");

    assert!(
        bech32_str.starts_with("scy1"),
        "encoded address must start with scy1"
    );

    let parsed = Address::parse(&bech32_str).expect("bech32 decoding should succeed");
    assert_eq!(parsed.hash(), &hash);
    assert_eq!(parsed.hrp(), Address::DEFAULT_HRP);
    assert_eq!(parsed.to_string(), bech32_str);
}

#[test]
fn test_bech32_checksum_failure() {
    let hash: [u8; 32] = [0x42; 32];
    let addr = Address::new(hash);
    let bech32_str = addr.to_bech32().unwrap();

    // Mutate the last character (which is part of the 6-character BCH checksum)
    let last_char = bech32_str.chars().last().unwrap();
    let mutated_char = if last_char == 'p' { 'q' } else { 'p' };
    let mut mutated_str = bech32_str.clone();
    mutated_str.pop();
    mutated_str.push(mutated_char);

    let res = Address::parse(&mutated_str);
    assert!(
        matches!(res, Err(AddressError::Bech32Decode(_))),
        "mutating checksum must cause Bech32Decode error, got: {:?}",
        res
    );
}

#[test]
fn test_backward_compatible_hex_parsing() {
    let raw_hex = "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
    let prefixed_hex = format!("0x{raw_hex}");

    let parsed_raw = Address::parse(raw_hex).expect("raw hex without 0x should parse");
    let parsed_prefixed = Address::parse(&prefixed_hex).expect("prefixed hex with 0x should parse");

    assert_eq!(parsed_raw.hash(), parsed_prefixed.hash());
    assert_eq!(parsed_raw.hrp(), Address::DEFAULT_HRP);

    // Converting parsed legacy hex to bech32 produces a valid scy1... address
    let bech32_str = parsed_raw.to_bech32().unwrap();
    assert!(bech32_str.starts_with("scy1"));

    let redecoded = Address::parse(&bech32_str).unwrap();
    assert_eq!(redecoded.hash(), parsed_raw.hash());
}

#[test]
fn test_case_insensitive_handling() {
    let hash: [u8; 32] = [0xaa; 32];
    let addr = Address::new(hash);
    let lower = addr.to_bech32().unwrap();
    let upper = lower.to_uppercase();

    let parsed_lower = Address::parse(&lower).unwrap();
    let parsed_upper = Address::parse(&upper).unwrap();

    assert_eq!(parsed_lower.hash(), parsed_upper.hash());
}

#[test]
fn test_invalid_address_length_or_format() {
    // Too short hex
    let short_hex = "01020304";
    assert_eq!(
        Address::parse(short_hex),
        Err(AddressError::UnrecognizedFormat)
    );

    // Random non-address string
    let garbage = "hello_world_not_an_address";
    assert_eq!(
        Address::parse(garbage),
        Err(AddressError::UnrecognizedFormat)
    );
}

#[test]
fn test_serde_json_roundtrip() {
    let hash: [u8; 32] = [0x55; 32];
    let addr = Address::new(hash);

    let json = serde_json::to_string(&addr).unwrap();
    assert!(json.contains("scy1"));

    let deserialized: Address = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, addr);
}
