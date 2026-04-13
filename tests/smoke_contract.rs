use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

const HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 8765;

#[test]
fn serves_root_endpoint_with_requested_host_port() {
    let _guard = test_lock()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let port = unused_port();
    let mut child = start_app(port);
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut status = None;

    while Instant::now() < deadline {
        if let Some(code) = root_status(port) {
            status = Some(code);
            break;
        }

        if let Some(exit_status) = child.try_wait().expect("failed to poll child process") {
            panic!("server exited before becoming ready: {exit_status}");
        }

        thread::sleep(Duration::from_millis(100));
    }

    stop_child(&mut child);

    assert_eq!(status, Some(200), "expected HTTP 200 from GET /");
}

#[test]
fn exits_with_error_when_port_is_in_use() {
    let _guard = test_lock()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let listener = TcpListener::bind((HOST, 0)).expect("failed to reserve a free port");
    let port = listener
        .local_addr()
        .expect("failed to inspect reserved port")
        .port();

    let output = Command::new(binary_path())
        .args(["--host", HOST, "--port", &port.to_string(), "--no-browser"])
        .output()
        .expect("failed to run binary");

    assert!(
        !output.status.success(),
        "expected a non-zero exit status when the port is already in use"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("port") || stderr.contains("bind") || stderr.contains("address"),
        "expected stderr to mention the bind failure, got: {stderr}"
    );
}

#[test]
fn launching_without_args_reuses_existing_shenyin_instance() {
    let _guard = test_lock()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let server = FakeServer::spawn(
        DEFAULT_PORT,
        "HTTP/1.1 200 OK\r\nContent-Length: 39\r\nConnection: close\r\n\r\n<html><title>ShenYin</title></html>",
    );

    let output = Command::new(binary_path())
        .env("SHENYIN_DISABLE_BROWSER", "1")
        .output()
        .expect("failed to run binary");

    server.stop();

    assert!(
        output.status.success(),
        "expected the app to reuse an existing ShenYin instance, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn launching_without_args_still_fails_when_other_service_owns_default_port() {
    let _guard = test_lock()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let server = FakeServer::spawn(
        DEFAULT_PORT,
        "HTTP/1.1 200 OK\r\nContent-Length: 32\r\nConnection: close\r\n\r\n<html><title>Other</title></html>",
    );

    let output = Command::new(binary_path())
        .env("SHENYIN_DISABLE_BROWSER", "1")
        .output()
        .expect("failed to run binary");

    server.stop();

    assert!(
        !output.status.success(),
        "expected the app to fail when an unrelated service owns the default port"
    );
}

fn start_app(port: u16) -> Child {
    Command::new(binary_path())
        .args(["--host", HOST, "--port", &port.to_string(), "--no-browser"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start ShenYin")
}

fn root_status(port: u16) -> Option<u16> {
    let mut stream = TcpStream::connect((HOST, port)).ok()?;
    stream
        .write_all(
            format!("GET / HTTP/1.1\r\nHost: {HOST}:{port}\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .ok()?;

    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    let status_line = response.lines().next()?;
    status_line.split_whitespace().nth(1)?.parse().ok()
}

fn unused_port() -> u16 {
    let listener = TcpListener::bind((HOST, 0)).expect("failed to bind to an ephemeral port");
    let port = listener
        .local_addr()
        .expect("failed to inspect bound socket")
        .port();
    drop(listener);
    port
}

fn stop_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn test_lock() -> &'static Mutex<()> {
    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    TEST_LOCK.get_or_init(|| Mutex::new(()))
}

fn binary_path() -> &'static str {
    env!("CARGO_BIN_EXE_shenyin")
}

struct FakeServer {
    stop_port: u16,
    thread: Option<thread::JoinHandle<()>>,
}

impl FakeServer {
    fn spawn(port: u16, response: &'static str) -> Self {
        let listener = TcpListener::bind((HOST, port)).expect("failed to bind fake server");
        listener
            .set_nonblocking(true)
            .expect("failed to configure fake server");

        let stop_listener =
            TcpListener::bind((HOST, 0)).expect("failed to allocate fake server stop port");
        let stop_port = stop_listener
            .local_addr()
            .expect("failed to inspect stop port")
            .port();
        stop_listener
            .set_nonblocking(true)
            .expect("failed to configure stop listener");

        let thread = thread::spawn(move || {
            loop {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let mut buffer = [0_u8; 1024];
                        let _ = stream.read(&mut buffer);
                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.flush();
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(_) => break,
                }

                match stop_listener.accept() {
                    Ok(_) => break,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(_) => break,
                }

                thread::sleep(Duration::from_millis(25));
            }
        });

        Self {
            stop_port,
            thread: Some(thread),
        }
    }

    fn stop(mut self) {
        let _ = TcpStream::connect((HOST, self.stop_port));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
