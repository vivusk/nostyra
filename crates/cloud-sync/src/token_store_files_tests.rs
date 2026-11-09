use super::*;

fn account() -> StoredAccount {
    StoredAccount {
        provider: OAuthProvider::OneDrive,
        account_id: "abc123".into(),
        account_label: "player@example.com".into(),
        scopes: vec!["Files.ReadWrite.AppFolder".into(), "offline_access".into()],
        signed_in_at: 1_800_000_000,
    }
}

#[test]
fn sign_in_round_trip_rotation_and_sign_out() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    assert_eq!(load_account(data).unwrap(), None);
    assert_eq!(load_refresh_token(data), Err(CloudError::AuthExpired));

    save_signed_in_account(data, &account(), &SecretString::new("M.R3_first")).unwrap();
    assert_eq!(load_account(data).unwrap(), Some(account()));
    assert_eq!(load_refresh_token(data).unwrap().expose(), "M.R3_first");
    let json = std::fs::read_to_string(account_file(data)).unwrap();
    assert!(json.contains("\"onedrive\"") && !json.contains("M.R3"));

    store_refresh_token(data, &SecretString::new("M.R3_rotated")).unwrap();
    assert_eq!(load_refresh_token(data).unwrap().expose(), "M.R3_rotated");

    delete_account_files(data).unwrap();
    assert!(!account_file(data).exists() && !refresh_token_file(data).exists());
    delete_account_files(data).unwrap();
}

#[cfg(windows)]
#[test]
fn token_file_is_encrypted_and_tampering_means_sign_in_again() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    save_signed_in_account(data, &account(), &SecretString::new("1//refresh-plain")).unwrap();
    let mut sealed = std::fs::read(refresh_token_file(data)).unwrap();
    assert!(!String::from_utf8_lossy(&sealed).contains("refresh-plain"));
    let middle = sealed.len() / 2;
    sealed[middle] ^= 0xFF;
    std::fs::write(refresh_token_file(data), &sealed).unwrap();
    assert_eq!(load_refresh_token(data), Err(CloudError::AuthExpired));
}

#[test]
fn debug_output_has_no_token() {
    let token = SecretString::new("1//secret-refresh");
    assert!(!format!("{token:?} {:?}", account()).contains("secret-refresh"));
}
