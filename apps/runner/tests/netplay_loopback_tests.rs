//! Three windowed runners (hidden windows) in one netplay room through a local
//! signaling server + loopback WebRTC (F8 / F24): host (scripted B, autostart once
//! one spectator joined), player (scripted `joypad:13`), spectator; each stops its
//! input at frame 600, waits for the frame to be confirmed and prints
//! `ram_crc=<hex> input_crc=<hex> frames=600`. The three lines must be equal. The
//! test ROMs keep input out of RAM (spike 00, plan OQ4: `ram_crc` equals an idle
//! run), so `input_crc` (a running digest of every input the core received up to
//! frame 600) proves the inputs crossed: equal on all peers, and different from a
//! baseline room with no input.
//!
//! Gated: `NOSTYRA_NETPLAY_LOOPBACK=1` and `NOSTYRA_NETPLAY_SERVER_URL=http://127.0.0.1:3536`
//! (the `apps/netplay-server` binary run locally). The test does the REST calls
//! itself over plain HTTP (the shared client is HTTPS-only, plan OQ9).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};

use netplay_session::netplay_session_env::{NetplaySessionEnv, ENV_KEY};
use netplay_session::room_api_types::{IceServer, SeatRole};
use serde_json::{json, Value};

/// POSTs `body` to `http://host:port{path}`; returns the JSON reply.
fn post(base: &str, path: &str, body: &Value) -> Value {
    let host = base.trim_start_matches("http://").trim_end_matches('/');
    let mut stream = TcpStream::connect(host).expect("server reachable");
    let body = body.to_string();
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut reply = String::new();
    stream.read_to_string(&mut reply).unwrap();
    let json = reply.split("\r\n\r\n").nth(1).expect("http body");
    serde_json::from_str(json).unwrap_or_else(|_| panic!("bad reply: {reply}"))
}

fn session_env(base: &str, reply: &Value, role: SeatRole, max_players: u8) -> String {
    let ws = base
        .replacen("http://", "ws://", 1)
        .trim_end_matches('/')
        .to_owned()
        + "/v1/ws";
    let ice: Vec<IceServer> = serde_json::from_value(reply["ice_servers"].clone()).unwrap();
    NetplaySessionEnv {
        server_ws_url: ws,
        room_id: reply["room_id"].as_str().unwrap().into(),
        code: reply["code"].as_str().map(str::to_owned),
        token: reply["token"].as_str().unwrap().into(),
        role,
        max_players,
        ice_servers: ice,
        expires_at: reply["expires_at"].as_u64().unwrap(),
    }
    .to_json()
    .unwrap()
}

fn spawn(core: &std::path::Path, env: String, tag: &str, args: &[&str]) -> Child {
    let data = support::scratch(&format!("netplay-{tag}"));
    Command::new(env!("CARGO_BIN_EXE_nostyra-runner"))
        .args(["--core", core.to_str().unwrap(), "--rom"])
        .arg(support::repo("test-assets/roms/nostyra-test.gen"))
        .args([
            "--system",
            "genesis",
            "--data-dir",
            data.to_str().unwrap(),
            "--window-hidden",
        ])
        .args(args)
        .env(ENV_KEY, env)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("runner starts")
}

/// The `ram_crc=` line of a finished runner (exit 0 required).
fn crc_line(child: Child) -> String {
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "runner failed ({:?}): {stdout} {stderr}",
        output.status
    );
    let line = stdout
        .lines()
        .find(|l| l.starts_with("ram_crc="))
        .expect("ram_crc line");
    line.to_owned()
}

/// One room of host + player + spectator with these scripted inputs.
fn room(base: &str, core: &std::path::Path, inputs: [&str; 2]) -> [String; 3] {
    let game = json!({"content_sha1": "0123456789abcdef0123456789abcdef01234567", "title": "t", "system": "genesis",
                      "core_id": "genesis_plus_gx", "core_version": "x"});
    let create =
        json!({"nickname": "host", "game": game, "max_players": 2, "allow_spectators": true});
    let host = post(base, "/v1/rooms", &create);
    let code = host["code"].as_str().unwrap().to_owned();
    let join = |role: &str| {
        post(
            base,
            &format!("/v1/rooms/{code}/join"),
            &json!({"nickname": role, "role": role}),
        )
    };
    let (player, watcher) = (join("player"), join("spectator"));
    let common = ["--netplay-frames-limit", "600"];
    let host_args = [
        "--netplay",
        "host",
        "--netplay-players",
        "2",
        "--netplay-autostart",
        "1",
    ];
    let host_run = spawn(
        core,
        session_env(base, &host, SeatRole::Host, 2),
        "host",
        &[
            &host_args[..],
            &common,
            &["--netplay-scripted-input", inputs[0]],
        ]
        .concat(),
    );
    let player_run = spawn(
        core,
        session_env(base, &player, SeatRole::Player, 2),
        "player",
        &[
            &["--netplay", "join"][..],
            &common,
            &["--netplay-scripted-input", inputs[1]],
        ]
        .concat(),
    );
    let watch_run = spawn(
        core,
        session_env(base, &watcher, SeatRole::Spectator, 2),
        "watch",
        &[&["--netplay", "watch"][..], &common].concat(),
    );
    [
        crc_line(host_run),
        crc_line(player_run),
        crc_line(watch_run),
    ]
}

#[test]
fn three_runners_agree_on_confirmed_frame_600() {
    if std::env::var("NOSTYRA_NETPLAY_LOOPBACK").as_deref() != Ok("1") {
        eprintln!("skipped: set NOSTYRA_NETPLAY_LOOPBACK=1 and NOSTYRA_NETPLAY_SERVER_URL");
        return;
    }
    let base = std::env::var("NOSTYRA_NETPLAY_SERVER_URL").expect("NOSTYRA_NETPLAY_SERVER_URL");
    let Some(core) = support::registry_core("genesis_plus_gx", "fetch-gpgx-core") else {
        return;
    };
    let crcs = room(&base, &core, ["b", "joypad:13"]);
    eprintln!("LOOPBACK {crcs:?}");
    assert!(crcs.iter().all(|c| c == &crcs[0]), "{crcs:?}");
    let idle = room(&base, &core, ["none", "none"]);
    assert!(idle.iter().all(|c| c == &idle[0]), "{idle:?}");
    assert_ne!(crcs[0], idle[0], "scripted inputs reached every peer");
}
