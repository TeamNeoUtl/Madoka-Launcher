use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread::{self, JoinHandle};

use launcher_providers::{Dns, ProviderError, SelfUpdater};

/// Serves one response on a loopback port and returns the base URL.
fn serve(status: u16, body: String) -> (String, JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 4096];
        let length = stream.read(&mut request).unwrap();
        write!(
            stream,
            "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        String::from_utf8_lossy(&request[..length]).into_owned()
    });
    (base, server)
}

fn release(tag: &str) -> String {
    format!(
        r#"{{"tag_name":"{tag}","name":"{tag}","created_at":"2026-01-01T00:00:00Z","body":"","assets":[]}}"#
    )
}

fn updater(base: &str, current: &str) -> SelfUpdater {
    SelfUpdater::new("test", Dns::default(), None, current).with_api_base_url(base)
}

#[test]
fn check_returns_the_version_of_a_newer_release() {
    let (base, server) = serve(200, release("v0.2.0"));
    let found = updater(&base, "0.1.0").check().unwrap();
    assert_eq!(found.as_deref(), Some("0.2.0"));
    let request = server.join().unwrap();
    assert!(request.starts_with("GET /repos/TeamNeoUtl/Madoka-launcher/releases/latest "));
}

#[test]
fn check_returns_none_for_the_running_version() {
    let (base, server) = serve(200, release("v0.1.0"));
    assert_eq!(updater(&base, "0.1.0").check().unwrap(), None);
    server.join().unwrap();
}

#[test]
fn check_returns_none_for_an_older_release() {
    let (base, server) = serve(200, release("v0.0.9"));
    assert_eq!(updater(&base, "0.1.0").check().unwrap(), None);
    server.join().unwrap();
}

#[test]
fn check_fails_on_an_error_status() {
    let (base, server) = serve(404, "{}".to_owned());
    let error = updater(&base, "0.1.0").check().unwrap_err();
    assert!(matches!(error, ProviderError::SelfUpdate(_)));
    server.join().unwrap();
}
