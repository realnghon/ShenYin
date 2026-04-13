use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const HOST: &str = "127.0.0.1";

#[test]
fn serves_root_endpoint_with_requested_host_port() {
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

fn binary_path() -> &'static str {
    env!("CARGO_BIN_EXE_shenyin")
}
