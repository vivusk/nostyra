use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::atomic::AtomicBool;
use std::thread::JoinHandle;
use std::time::Duration;

use super::*;
use crate::oauth_endpoints::RedirectHost;

fn any_port() -> LoopbackRedirect {
    LoopbackRedirect {
        host: RedirectHost::Ipv4Loopback,
        ports: PortRule::Any,
    }
}

/// Sends `GET <target>` to the listener from a real socket; returns the reply.
fn get_once(port: u16, target: &str) -> String {
    let request = format!("GET {target} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n");
    let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut reply = String::new();
    let _ = stream.read_to_string(&mut reply);
    reply
}

/// A browser thread sending `targets` one after the other; returns the replies.
fn browser(port: u16, targets: &[&str]) -> JoinHandle<Vec<String>> {
    let targets: Vec<String> = targets.iter().map(|t| t.to_string()).collect();
    std::thread::spawn(move || targets.iter().map(|t| get_once(port, t)).collect())
}

fn browser_get(port: u16, target: &str) -> JoinHandle<Vec<String>> {
    browser(port, &[target])
}

fn port_of(listener: &LoopbackListener) -> u16 {
    listener.listener.local_addr().unwrap().port()
}

fn wait(listener: &LoopbackListener) -> Result<SecretString, SignInError> {
    listener.wait_for_code("st8", Duration::from_secs(10), &AtomicBool::new(false))
}

#[test]
fn right_state_returns_code_and_ignores_favicon() {
    let listener = LoopbackListener::bind(&any_port()).unwrap();
    let port = port_of(&listener);
    assert_eq!(listener.redirect_uri(), format!("http://127.0.0.1:{port}/"));
    let replies = browser(
        port,
        &["/favicon.ico", "/?code=4%2F0Ab-x%2By&state=st8&scope=a+b"],
    );
    let code = wait(&listener).unwrap();
    assert_eq!(code.expose(), "4/0Ab-x+y");
    let replies = replies.join().unwrap();
    assert!(replies[0].starts_with("HTTP/1.1 404"), "{}", replies[0]);
    assert!(replies[1].starts_with("HTTP/1.1 200 OK"), "{}", replies[1]);
    assert!(replies[1].contains("You can close this tab"));
}

#[test]
fn wrong_state_is_an_error() {
    let listener = LoopbackListener::bind(&any_port()).unwrap();
    let browser = browser_get(port_of(&listener), "/?code=abc&state=other");
    assert_eq!(wait(&listener).unwrap_err(), SignInError::StateMismatch);
    assert!(browser.join().unwrap()[0].starts_with("HTTP/1.1 400"));
}

#[test]
fn access_denied_is_cancelled_and_other_errors_refused() {
    let listener = LoopbackListener::bind(&any_port()).unwrap();
    let browser = browser_get(port_of(&listener), "/?error=access_denied&state=st8");
    assert_eq!(wait(&listener).unwrap_err(), SignInError::Cancelled);
    browser.join().unwrap();

    let listener = LoopbackListener::bind(&any_port()).unwrap();
    let browser = browser_get(port_of(&listener), "/?error=server_error%3Cb%3E&state=st8");
    assert_eq!(
        wait(&listener).unwrap_err(),
        SignInError::Refused("server_errorb".into())
    );
    browser.join().unwrap();
}

#[test]
fn times_out_and_honours_cancel() {
    let listener = LoopbackListener::bind(&any_port()).unwrap();
    let never = AtomicBool::new(false);
    let result = listener.wait_for_code("st8", Duration::from_millis(300), &never);
    assert_eq!(result.unwrap_err(), SignInError::TimedOut);
    let cancelled = AtomicBool::new(true);
    let result = listener.wait_for_code("st8", Duration::from_secs(60), &cancelled);
    assert_eq!(result.unwrap_err(), SignInError::Cancelled);
}

#[test]
fn fixed_ports_fall_back_to_the_next_free_one() {
    // Two free ports: the first stays taken by another socket.
    let taken = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let free = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let ports: &'static [u16] = Box::leak(Box::new([
        taken.local_addr().unwrap().port(),
        free.local_addr().unwrap().port(),
    ]));
    drop(free);
    let redirect = LoopbackRedirect {
        host: RedirectHost::Localhost,
        ports: PortRule::Fixed(ports),
    };
    let listener = LoopbackListener::bind(&redirect).unwrap();
    assert_eq!(port_of(&listener), ports[1]);
    assert_eq!(
        listener.redirect_uri(),
        format!("http://localhost:{}/", ports[1])
    );

    let busy: &'static [u16] = Box::leak(Box::new([ports[0]]));
    let redirect = LoopbackRedirect {
        host: RedirectHost::Ipv4Loopback,
        ports: PortRule::Fixed(busy),
    };
    assert!(matches!(
        LoopbackListener::bind(&redirect),
        Err(SignInError::PortsBusy(list)) if list == ports[0].to_string()
    ));
}
