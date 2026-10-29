use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Mutex;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use launcher_core::{Error, Fetcher, Progress, ProgressSink};
use launcher_providers::ReqwestFetcher;
use tempfile::tempdir;
use url::Url;

#[derive(Default)]
struct Recorder(Mutex<Vec<Progress>>);

impl ProgressSink for Recorder {
    fn report(&self, progress: Progress) {
        self.0.lock().unwrap().push(progress);
    }
}

/// Serves one response in 10 kB writes on a loopback port.
fn serve(status: u16, body: Vec<u8>) -> (Url, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 1024];
        let _ = stream.read(&mut request);
        // The client may close the connection early (error status); write errors are ignored.
        let _ = write!(
            stream,
            "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        for chunk in body.chunks(10_000) {
            let _ = stream.write_all(chunk);
            let _ = stream.flush();
            thread::sleep(Duration::from_millis(5));
        }
    });
    (
        Url::parse(&format!("http://127.0.0.1:{port}/file")).unwrap(),
        server,
    )
}

#[tokio::test]
async fn progress_is_monotonic_and_ends_at_the_content_length() {
    let body: Vec<u8> = (0..100_000_u32).map(|i| (i % 251) as u8).collect();
    let (url, server) = serve(200, body.clone());
    let dir = tempdir().unwrap();
    let dest = dir.path().join("out.bin");
    let recorder = Recorder::default();

    ReqwestFetcher::new("test")
        .unwrap()
        .fetch(&url, &dest, &recorder)
        .await
        .unwrap();
    server.join().unwrap();

    assert_eq!(fs::read(&dest).unwrap(), body);
    let seen = recorder.0.lock().unwrap();
    assert_eq!(
        seen.first(),
        Some(&Progress {
            done: 0,
            total: Some(100_000)
        })
    );
    assert_eq!(
        seen.last(),
        Some(&Progress {
            done: 100_000,
            total: Some(100_000)
        })
    );
    assert!(seen.len() > 2);
    assert!(seen.windows(2).all(|pair| pair[0].done <= pair[1].done));
    assert!(seen.iter().all(|progress| progress.total == Some(100_000)));
}

#[tokio::test]
async fn error_status_fails_without_progress() {
    let (url, server) = serve(500, b"error".to_vec());
    let dir = tempdir().unwrap();
    let recorder = Recorder::default();

    let result = ReqwestFetcher::new("test")
        .unwrap()
        .fetch(&url, &dir.path().join("out.bin"), &recorder)
        .await;
    server.join().unwrap();

    assert!(matches!(result, Err(Error::Fetch { ref message, .. }) if message.contains("500")));
    assert!(recorder.0.lock().unwrap().is_empty());
}
