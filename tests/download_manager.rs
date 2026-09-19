use std::{path::PathBuf, sync::Arc, time::Duration};

use andromeda_client::download::{
    Checksum, DownloadConfig, DownloadEvent, DownloadJob, DownloadManager, DownloadPlan,
};
use sha2::Digest as _;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::{Mutex, mpsc},
};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
struct Reply {
    status: &'static str,
    headers: &'static str,
    body: &'static [u8],
}

async fn server(
    replies: Vec<Reply>,
) -> (
    reqwest::Url,
    Arc<Mutex<Vec<String>>>,
    tokio::task::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&requests);
    let task = tokio::spawn(async move {
        for reply in replies {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = vec![0_u8; 4096];
            let read = socket.read(&mut bytes).await.unwrap();
            captured
                .lock()
                .await
                .push(String::from_utf8_lossy(&bytes[..read]).into_owned());
            let response = format!(
                "HTTP/1.1 {}\r\nContent-Length: {}\r\n{}Connection: close\r\n\r\n",
                reply.status,
                reply.body.len(),
                reply.headers
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            socket.write_all(reply.body).await.unwrap();
        }
    });
    (
        format!("http://{address}/artifact").parse().unwrap(),
        requests,
        task,
    )
}

fn job(url: reqwest::Url, body: &[u8]) -> DownloadJob {
    DownloadJob {
        id: "artifact".into(),
        url,
        destination: PathBuf::from("nested/artifact.bin"),
        expected_size: Some(body.len().try_into().unwrap()),
        checksum: Some(Checksum::Sha256(hex::encode(sha2::Sha256::digest(body)))),
    }
}

fn test_config() -> DownloadConfig {
    DownloadConfig {
        initial_backoff: Duration::from_millis(1),
        idle_timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(3),
        ..DownloadConfig::default()
    }
}

#[tokio::test]
async fn verifies_and_atomically_publishes_a_download() {
    let body = b"verified artifact";
    let (url, _, server) = server(vec![Reply {
        status: "200 OK",
        headers: "ETag: \"fixture\"\r\n",
        body,
    }])
    .await;
    let temp = tempfile::tempdir().unwrap();
    let manager = DownloadManager::new(temp.path(), test_config()).unwrap();
    let (events, mut receiver) = mpsc::channel(32);
    let result = manager
        .execute(
            DownloadPlan {
                jobs: vec![job(url, body)],
            },
            CancellationToken::new(),
            events,
        )
        .await
        .unwrap();
    server.await.unwrap();

    assert_eq!(result.completed_jobs, 1);
    assert_eq!(
        std::fs::read(temp.path().join("nested/artifact.bin")).unwrap(),
        body
    );
    assert!(
        !temp
            .path()
            .join("nested/.artifact.bin.andromeda.part")
            .exists()
    );
    assert!(
        std::iter::from_fn(|| receiver.try_recv().ok())
            .any(|event| matches!(event, DownloadEvent::Completed { .. }))
    );
}

#[tokio::test]
async fn resumes_only_with_a_validator_and_confirmed_range() {
    let whole = b"resume-safe";
    let existing = &whole[..6];
    let remainder = &whole[6..];
    let (url, requests, server) = server(vec![Reply {
        status: "206 Partial Content",
        headers: "ETag: \"stable\"\r\nContent-Range: bytes 6-10/11\r\n",
        body: remainder,
    }])
    .await;
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().join("nested");
    std::fs::create_dir(&directory).unwrap();
    std::fs::write(directory.join(".artifact.bin.andromeda.part"), existing).unwrap();
    std::fs::write(
        directory.join(".artifact.bin.andromeda.resume.json"),
        format!(r#"{{"url":"{url}","validator":"\"stable\""}}"#),
    )
    .unwrap();

    let manager = DownloadManager::new(temp.path(), test_config()).unwrap();
    let (events, _) = mpsc::channel(16);
    let result = manager
        .execute(
            DownloadPlan {
                jobs: vec![job(url, whole)],
            },
            CancellationToken::new(),
            events,
        )
        .await
        .unwrap();
    server.await.unwrap();

    assert_eq!(result.network_bytes, remainder.len() as u64);
    assert_eq!(
        std::fs::read(directory.join("artifact.bin")).unwrap(),
        whole
    );
    let request = &requests.lock().await[0];
    assert!(request.contains("range: bytes=6-") || request.contains("Range: bytes=6-"));
    assert!(request.contains("if-range: \"stable\"") || request.contains("If-Range: \"stable\""));
}

#[tokio::test]
async fn retries_transient_http_failures() {
    let body = b"eventually available";
    let (url, requests, server) = server(vec![
        Reply {
            status: "503 Service Unavailable",
            headers: "",
            body: b"later",
        },
        Reply {
            status: "200 OK",
            headers: "ETag: \"stable\"\r\n",
            body,
        },
    ])
    .await;
    let temp = tempfile::tempdir().unwrap();
    let manager = DownloadManager::new(temp.path(), test_config()).unwrap();
    let (events, mut receiver) = mpsc::channel(32);
    manager
        .execute(
            DownloadPlan {
                jobs: vec![job(url, body)],
            },
            CancellationToken::new(),
            events,
        )
        .await
        .unwrap();
    server.await.unwrap();

    assert_eq!(requests.lock().await.len(), 2);
    assert!(
        std::iter::from_fn(|| receiver.try_recv().ok())
            .any(|event| matches!(event, DownloadEvent::Retrying { .. }))
    );
}

#[tokio::test]
async fn observes_cancellation_before_scheduling_network_work() {
    let temp = tempfile::tempdir().unwrap();
    let manager = DownloadManager::new(temp.path(), test_config()).unwrap();
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let (events, _) = mpsc::channel(8);
    let error = manager
        .execute(
            DownloadPlan {
                jobs: vec![job(
                    "http://127.0.0.1:9/unreachable".parse().unwrap(),
                    b"never",
                )],
            },
            cancellation,
            events,
        )
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        andromeda_client::download::DownloadError::Cancelled
    ));
}
