use scytale_account::{
    decrypt_key, derive_candidate, encrypt_key, AccountNumber, AliasStore, StoreError,
};

#[test]
fn candidate_derivation_is_stable_and_retry_changes_candidate() {
    let first = derive_candidate(b"key-id", "passbook-42", 0);
    assert_eq!(first, derive_candidate(b"key-id", "passbook-42", 0));
    assert_ne!(first, derive_candidate(b"key-id", "passbook-42", 1));
    assert!(first.to_string().starts_with("SCY-"));
}

#[test]
fn conflicting_bind_is_rejected_without_overwriting_fcfs_binding() {
    let mut store = AliasStore::in_memory();
    let account: AccountNumber = "SCY-000042".parse().unwrap();
    store.bind(account.clone(), "passbook-a").unwrap();
    assert!(matches!(
        store.bind(account.clone(), "passbook-b"),
        Err(StoreError::AccountConflict)
    ));
    assert_eq!(store.by_account(&account), Some("passbook-a"));
}

#[test]
fn pin_vault_round_trip_and_wrong_pin_are_safe() {
    let envelope = encrypt_key(b"key-id-material", "123456").unwrap();
    let opened = decrypt_key(&envelope, "123456").unwrap();
    assert_eq!(&*opened, b"key-id-material");
    assert!(decrypt_key(&envelope, "654321").is_err());
    assert!(decrypt_key(&envelope, "12345").is_err());
}
