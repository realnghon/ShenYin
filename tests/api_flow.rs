use reqwest::multipart::{Form, Part};
use serde_json::Value;
use shenyin::server::{self, ServerOptions};
use std::net::TcpListener;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const HOST: &str = "127.0.0.1";

#[tokio::test]
async fn text_encrypt_decrypt_flow_matches_original_contract() {
    let _guard = test_lock()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let port = unused_port();
    let server = tokio::spawn(server::run(ServerOptions {
        host: HOST.into(),
        port,
        no_browser: true,
        port_was_explicit: true,
    }));

    wait_until_ready(port).await;
    let client = reqwest::Client::new();

    let encrypt = client
        .post(format!("http://{HOST}:{port}/api/encrypt"))
        .multipart(
            Form::new()
                .text("input_type", "text")
                .text("compression", "zlib")
                .text("passphrase", "pw")
                .text("text_input", "hello flow"),
        )
        .send()
        .await
        .unwrap();
    let encrypt_json: Value = encrypt.json().await.unwrap();
    assert_eq!(encrypt_json["ok"], true);
    let encrypted_text = encrypt_json["result"]["text"].as_str().unwrap().to_owned();

    let decrypt = client
        .post(format!("http://{HOST}:{port}/api/decrypt"))
        .multipart(
            Form::new()
                .text("input_type", "text")
                .text("passphrase", "pw")
                .text("ciphertext_text", encrypted_text),
        )
        .send()
        .await
        .unwrap();
    let decrypt_json: Value = decrypt.json().await.unwrap();
    assert_eq!(decrypt_json["ok"], true);
    assert_eq!(decrypt_json["result"]["text"], "hello flow");

    server.abort();
}

#[tokio::test]
async fn file_encrypt_to_text_and_download_roundtrip() {
    let _guard = test_lock()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let port = unused_port();
    let server = tokio::spawn(server::run(ServerOptions {
        host: HOST.into(),
        port,
        no_browser: true,
        port_was_explicit: true,
    }));

    wait_until_ready(port).await;
    let client = reqwest::Client::new();

    let encrypt = client
        .post(format!("http://{HOST}:{port}/api/encrypt"))
        .multipart(
            Form::new()
                .text("input_type", "file")
                .text("output_format", "armor")
                .text("compression", "zlib")
                .text("passphrase", "pw")
                .part("file_input", Part::bytes(b"hello file".to_vec()).file_name("demo.txt")),
        )
        .send()
        .await
        .unwrap();
    let encrypt_json: Value = encrypt.json().await.unwrap();
    assert_eq!(encrypt_json["ok"], true);
    let encoded_text = encrypt_json["result"]["text"].as_str().unwrap().to_owned();

    let decrypt = client
        .post(format!("http://{HOST}:{port}/api/decrypt"))
        .multipart(
            Form::new()
                .text("input_type", "file")
                .text("passphrase", "pw")
                .part(
                    "ciphertext_file",
                    Part::bytes(encoded_text.into_bytes()).file_name("demo.txt.txt"),
                ),
        )
        .send()
        .await
        .unwrap();
    let decrypt_json: Value = decrypt.json().await.unwrap();
    assert_eq!(decrypt_json["ok"], true);
    assert_eq!(decrypt_json["result"]["filename"], "demo.txt");

    server.abort();
}

#[tokio::test]
async fn closing_browser_session_shuts_down_server() {
    let _guard = test_lock()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    unsafe {
        std::env::set_var("SHENYIN_DISABLE_BROWSER", "1");
    }

    let port = unused_port();
    let server = tokio::spawn(server::run(ServerOptions {
        host: HOST.into(),
        port,
        no_browser: false,
        port_was_explicit: true,
    }));

    wait_until_ready(port).await;
    let client = reqwest::Client::new();
    let html = client
        .get(format!("http://{HOST}:{port}/"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let session_id = extract_session(&html);

    let close = client
        .post(format!("http://{HOST}:{port}/api/app-session/close"))
        .json(&serde_json::json!({ "session_id": session_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(close.status(), reqwest::StatusCode::NO_CONTENT);

    let result = tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .expect("server did not stop after close signal")
        .expect("server task panicked");
    assert!(result.is_ok());

    unsafe {
        std::env::remove_var("SHENYIN_DISABLE_BROWSER");
    }
}

#[tokio::test]
async fn large_file_encrypt_download_and_text_decrypt() {
    let _guard = test_lock()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let port = unused_port();
    let server = tokio::spawn(server::run(ServerOptions {
        host: HOST.into(),
        port,
        no_browser: true,
        port_was_explicit: true,
    }));

    wait_until_ready(port).await;
    let client = reqwest::Client::new();
    let original: Vec<u8> = (0..300_000).map(|index| ((index * 37 + 17) % 256) as u8).collect();

    let encrypt = client
        .post(format!("http://{HOST}:{port}/api/encrypt"))
        .multipart(
            Form::new()
                .text("input_type", "file")
                .text("output_format", "armor")
                .text("compression", "none")
                .text("passphrase", "pw")
                .part("file_input", Part::bytes(original.clone()).file_name("large.bin")),
        )
        .send()
        .await
        .unwrap();
    let encrypt_json: Value = encrypt.json().await.unwrap();
    assert_eq!(encrypt_json["result"]["text_too_large"], true);
    assert_eq!(encrypt_json["result"]["text_available"], false);
    let download_url = encrypt_json["result"]["download_url"].as_str().unwrap();

    let encoded_text = client
        .get(format!("http://{HOST}:{port}{download_url}"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    let decrypt = client
        .post(format!("http://{HOST}:{port}/api/decrypt"))
        .multipart(
            Form::new()
                .text("input_type", "text")
                .text("passphrase", "pw")
                .text("ciphertext_text", encoded_text),
        )
        .send()
        .await
        .unwrap();
    let decrypt_json: Value = decrypt.json().await.unwrap();
    assert_eq!(decrypt_json["result"]["filename"], "large.bin");

    server.abort();
}

#[tokio::test]
async fn large_text_post_is_not_rejected_by_form_limit() {
    let _guard = test_lock()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let port = unused_port();
    let server = tokio::spawn(server::run(ServerOptions {
        host: HOST.into(),
        port,
        no_browser: true,
        port_was_explicit: true,
    }));

    wait_until_ready(port).await;
    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://{HOST}:{port}/api/decrypt"))
        .multipart(
            Form::new()
                .text("input_type", "text")
                .text("passphrase", "pw")
                .text("ciphertext_text", "A".repeat(1_200_000)),
        )
        .send()
        .await
        .unwrap();

    assert_ne!(response.status(), reqwest::StatusCode::PAYLOAD_TOO_LARGE);

    server.abort();
}

fn extract_session(html: &str) -> String {
    let marker = r#""session":"#;
    let start = html.find(marker).expect("missing session boot marker") + marker.len();
    let remainder = &html[start..];
    let end = remainder.find('"').expect("missing session closing quote");
    remainder[..end].to_owned()
}

async fn wait_until_ready(port: u16) {
    let client = reqwest::Client::new();
    for _ in 0..40 {
        if let Ok(response) = client
            .get(format!("http://{HOST}:{port}/"))
            .send()
            .await
        {
            if response.status() == reqwest::StatusCode::OK {
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("server did not become ready on port {port}");
}

fn unused_port() -> u16 {
    let listener = TcpListener::bind((HOST, 0)).expect("failed to reserve a free port");
    let port = listener
        .local_addr()
        .expect("failed to inspect reserved port")
        .port();
    drop(listener);
    port
}

fn test_lock() -> &'static Mutex<()> {
    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    TEST_LOCK.get_or_init(|| Mutex::new(()))
}
