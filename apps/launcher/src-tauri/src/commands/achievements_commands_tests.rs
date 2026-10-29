//! Login / logout / account / runner env with the fake RA server. The password must
//! never reach the data folder: every file under it is scanned after a login.

use achievements::{credentials_path, FakeTransport, BODY_OFFLINE};

use super::*;

const SECRET: &str = "Sup3r-Secret pass";

/// Every file under `dir`, recursively.
fn files_under(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(files_under(&path));
        } else {
            files.push(path);
        }
    }
    files
}

#[test]
fn login_stores_user_and_token_only_and_never_the_password() {
    let data = tempfile::tempdir().unwrap();
    let fake = FakeTransport::new();
    let account = login_and_store(data.path(), &fake, " Tester ", &Password::new(SECRET)).unwrap();
    assert_eq!(
        account,
        RaAccount {
            user: "Tester".into(),
            points: Some(10),
            softcore_points: Some(3)
        }
    );
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(credentials_path(data.path())).unwrap()).unwrap();
    let keys: Vec<&String> = json.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["token", "user"]);
    for file in files_under(data.path()) {
        let bytes = std::fs::read(&file).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(!text.contains(SECRET), "password in {}", file.display());
    }
    assert!(!format!("{account:?}").contains("tok123"));
}

#[test]
fn failed_logins_store_nothing_and_are_classified() {
    let data = tempfile::tempdir().unwrap();
    let fake = FakeTransport::new();
    fake.respond(
        "login2",
        401,
        "{\"Success\":false,\"Error\":\"Invalid User/Password combination.\",\
         \"Status\":401,\"Code\":\"invalid_credentials\"}",
    );
    let err = login_and_store(data.path(), &fake, "Tester", &Password::new(SECRET)).unwrap_err();
    assert_eq!(
        (err.kind, err.message.as_str()),
        ("login", "Wrong username or password.")
    );
    fake.respond("login2", -2, BODY_OFFLINE);
    let err = login_and_store(data.path(), &fake, "Tester", &Password::new(SECRET)).unwrap_err();
    assert_eq!(err.kind, "network");
    let err = login_and_store(data.path(), &fake, "  ", &Password::new(SECRET)).unwrap_err();
    assert_eq!(err.kind, "invalid");
    assert!(!credentials_path(data.path()).exists());
}

#[test]
fn account_reads_the_file_and_uses_cached_points_for_the_same_user() {
    let data = tempfile::tempdir().unwrap();
    assert_eq!(stored_account(data.path(), None), None);
    let fake = FakeTransport::new();
    let account = login_and_store(data.path(), &fake, "Tester", &Password::new(SECRET)).unwrap();
    assert_eq!(
        stored_account(data.path(), Some(account.clone())),
        Some(account)
    );
    let other = RaAccount {
        user: "Other".into(),
        points: Some(99),
        softcore_points: Some(1),
    };
    assert_eq!(
        stored_account(data.path(), Some(other)),
        Some(RaAccount {
            user: "Tester".into(),
            points: None,
            softcore_points: None
        })
    );
    delete_credentials(data.path()).unwrap();
    assert_eq!(stored_account(data.path(), None), None);
}

#[test]
fn runner_env_needs_login_and_enabled_achievements() {
    let data = tempfile::tempdir().unwrap();
    assert!(runner_login_env(data.path()).is_empty(), "logged out");
    let fake = FakeTransport::new();
    login_and_store(data.path(), &fake, "Tester", &Password::new(SECRET)).unwrap();
    let env = runner_login_env(data.path());
    assert_eq!(
        env,
        [
            (OsString::from(USER_ENV), OsString::from("Tester")),
            (OsString::from(TOKEN_ENV), OsString::from("tok123")),
        ]
    );
    let global = global_config_path(data.path());
    config_layers::set_layer_value(&global, "achievements.enabled", Some(false.into())).unwrap();
    assert!(runner_login_env(data.path()).is_empty(), "disabled");
}
