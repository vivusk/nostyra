#![allow(clippy::unwrap_used)] // test code: panics are the failure signal

use super::*;
use crate::fake_server_transport::FakeTransport;
use crate::server_transport::BODY_OFFLINE;
use crate::{save_credentials, Credentials};

const SECRET: &str = "Pa$$ w&rd-7e1c";

#[test]
fn password_login_returns_the_token() {
    let fake = FakeTransport::new();
    let ok = login_with_password(&fake, "Tester", &Password::new(SECRET)).unwrap();
    assert_eq!(
        (ok.username.as_str(), ok.token.as_str(), ok.score),
        ("Tester", "tok123", 10)
    );
    assert_eq!(fake.posts(), ["r=login2&u=Tester&p=Pa%24%24+w%26rd-7e1c"]);
    assert!(!format!("{ok:?}").contains("tok123"));
}

#[test]
fn rejected_and_offline_logins_are_classified() {
    let fake = FakeTransport::new();
    fake.respond(
        "login2",
        401,
        "{\"Success\":false,\"Error\":\"Invalid User/Password combination. Please try again\",\
         \"Status\":401,\"Code\":\"invalid_credentials\"}",
    );
    let password = Password::new(SECRET);
    assert_eq!(
        login_with_password(&fake, "Tester", &password),
        Err(LoginError::InvalidCredentials)
    );
    fake.respond(
        "login2",
        403,
        "{\"Success\":false,\"Error\":\"The access token has expired. Please log in again.\",\
         \"Status\":401,\"Code\":\"expired_token\"}",
    );
    assert_eq!(
        login_with_password(&fake, "Tester", &password),
        Err(LoginError::ExpiredToken)
    );
    fake.respond("login2", -2, BODY_OFFLINE);
    assert_eq!(
        login_with_password(&fake, "Tester", &password),
        Err(LoginError::Network("offline".into()))
    );
}

/// Every file under `dir`, recursively.
fn files_under(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
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
fn password_never_reaches_the_data_dir() {
    let data = tempfile::tempdir().unwrap();
    let ok = login_with_password(&FakeTransport::new(), "Tester", &Password::new(SECRET)).unwrap();
    save_credentials(
        data.path(),
        &Credentials {
            user: ok.username,
            token: ok.token,
        },
    )
    .unwrap();
    let files = files_under(data.path());
    assert!(!files.is_empty());
    for file in files {
        let text = String::from_utf8_lossy(&std::fs::read(&file).unwrap()).into_owned();
        for needle in [SECRET, "Pa%24%24", "w&rd-7e1c", "w%26rd"] {
            assert!(
                !text.contains(needle),
                "{} contains the password",
                file.display()
            );
        }
    }
    assert_eq!(
        format!("{:?}", Password::new(SECRET)),
        "Password(<redacted>)"
    );
}
