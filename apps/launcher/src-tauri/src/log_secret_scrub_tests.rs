//! Secret scrub forms (RA, OAuth, netplay) and lines that must stay.

use super::*;

#[test]
fn scrubs_url_env_json_and_debug_forms() {
    let text = "GET /dorequest.php?r=awardachievement&u=Tester&t=tok123&a=1\n\
                    NOSTYRA_RA_TOKEN=tok123 set\n\
                    {\"Success\":true,\"Token\":\"tok123\",\"User\":\"Tester\"}\n\
                    Credentials { user: \"Tester\", token: \"tok123\" } password=hunter2\n\
                    INFO login token=\"tok123\" ok\n";
    let scrubbed = scrub_secrets(text);
    assert!(
        !scrubbed.contains("tok123") && !scrubbed.contains("hunter2"),
        "{scrubbed}"
    );
    assert!(scrubbed.contains("u=Tester&t=<redacted>&a=1"), "{scrubbed}");
    assert!(
        scrubbed.contains("NOSTYRA_RA_TOKEN=<redacted> set"),
        "{scrubbed}"
    );
    assert!(
        scrubbed.contains("\"Token\":\"<redacted>\",\"User\":\"Tester\""),
        "{scrubbed}"
    );
    assert!(scrubbed.contains("token: \"<redacted>\" }"), "{scrubbed}");
    assert!(scrubbed.contains("token=\"<redacted>\" ok"), "{scrubbed}");
}

#[test]
fn scrubs_oauth_secrets_but_not_exit_codes() {
    let text = "POST token code_verifier=v3r1f13r&client_secret=s3cr3t&code=abc
                    {\"client_secret\":\"s3cr3t\"} game session ended exit_code=3
";
    let scrubbed = scrub_secrets(text);
    assert!(
        !scrubbed.contains("v3r1f13r") && !scrubbed.contains("s3cr3t"),
        "{scrubbed}"
    );
    assert!(scrubbed.contains("code_verifier=<redacted>&"), "{scrubbed}");
    assert!(scrubbed.contains("exit_code=3"), "{scrubbed}");
}

#[test]
fn leaves_ordinary_log_lines_alone() {
    let text = "2026-10-29 15:40:02 INFO set=3 at=5 tokens: 4 rom=\"C:\\Users\\Đức\\a.sfc\"\n";
    assert_eq!(scrub_secrets(text), text);
}

#[test]
fn scrubs_netplay_session_env_credentials_and_candidates() {
    let text = "spawn NOSTYRA_NETPLAY_SESSION={\"room_id\":\"r1\",\"token\":\"np.tok\",                \"ice_servers\":[{\"urls\":[\"turn:1.2.3.4:3478\"],\"credential\":\"CRED9\"}]} ok
                {\"credential\":\"CRED9\",\"username\":\"17:p\"}
                ice candidate:842163049 1 udp 1677729535 203.0.113.7 61234 typ srflx
                a=candidate:1 1 UDP 2122 192.168.1.20 5000 typ host
                join room_code=K7Q2ZP netplay_token=abc.def ice_servers=[{\"urls\":[\"stun:x\"]}]
                netplay: room ready players=2 exit_code=0
";
    let scrubbed = scrub_secrets(text);
    for secret in [
        "np.tok",
        "CRED9",
        "203.0.113.7",
        "192.168.1.20",
        "K7Q2ZP",
        "abc.def",
        "turn:1.2.3.4",
        "r1",
    ] {
        assert!(!scrubbed.contains(secret), "{secret} left in: {scrubbed}");
    }
    assert!(
        scrubbed.contains("NOSTYRA_NETPLAY_SESSION=<redacted> ok"),
        "{scrubbed}"
    );
    assert!(
        scrubbed.contains("netplay: room ready players=2 exit_code=0"),
        "{scrubbed}"
    );
    assert!(
        scrubbed.contains("\"username\":\"17:p\""),
        "username is not a key: {scrubbed}"
    );
}
