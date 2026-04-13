use std::env;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, ExitCode};
use std::thread;
use std::time::Duration;

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 8765;
const INDEX_BODY: &str = r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>ShenYin</title>
  <style>
    :root {
      color-scheme: light;
      font-family: "Segoe UI", "PingFang SC", "Microsoft YaHei", sans-serif;
    }

    body {
      margin: 0;
      min-height: 100vh;
      display: grid;
      place-items: center;
      background: linear-gradient(135deg, #f4f1e8 0%, #e4ecf7 100%);
      color: #1f2a37;
    }

    main {
      width: min(640px, calc(100vw - 32px));
      padding: 32px;
      border-radius: 24px;
      background: rgba(255, 255, 255, 0.86);
      box-shadow: 0 22px 60px rgba(15, 23, 42, 0.12);
      backdrop-filter: blur(12px);
    }

    h1 {
      margin: 0 0 12px;
      font-size: clamp(2rem, 5vw, 3rem);
    }

    p {
      margin: 0;
      line-height: 1.7;
      font-size: 1rem;
    }
  </style>
</head>
<body>
  <main>
    <h1>ShenYin</h1>
    <p>Rust baseline is running. GitHub Actions release builds can now verify the packaged app by checking this page returns HTTP 200.</p>
  </main>
</body>
</html>
"#;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let options = Options::parse(env::args().skip(1))?;
    let listener = TcpListener::bind((options.host.as_str(), options.port))
        .map_err(|error| format!("failed to bind {}:{}: {error}", options.host, options.port))?;

    if !options.no_browser {
        open_browser_later(options.host.clone(), options.port);
    }

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(|| {
                    let _ = respond(stream);
                });
            }
            Err(error) => {
                eprintln!("connection error: {error}");
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct Options {
    host: String,
    port: u16,
    no_browser: bool,
}

impl Options {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut host = DEFAULT_HOST.to_owned();
        let mut port = DEFAULT_PORT;
        let mut no_browser = false;
        let mut args = args.into_iter();

        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--host" => {
                    host = args.next().ok_or_else(|| missing_value("--host"))?;
                }
                "--port" => {
                    let value = args.next().ok_or_else(|| missing_value("--port"))?;
                    port = value
                        .parse::<u16>()
                        .map_err(|_| format!("invalid value for --port: {value}"))?;
                }
                "--no-browser" => {
                    no_browser = true;
                }
                "--help" | "-h" => {
                    println!("{}", Self::usage());
                    std::process::exit(0);
                }
                other => {
                    return Err(format!("unknown argument: {other}\n\n{}", Self::usage()));
                }
            }
        }

        Ok(Self {
            host,
            port,
            no_browser,
        })
    }

    fn usage() -> &'static str {
        "Usage: shenyin [--host HOST] [--port PORT] [--no-browser]"
    }
}

fn missing_value(flag: &str) -> String {
    format!("missing value for {flag}")
}

fn respond(mut stream: TcpStream) -> std::io::Result<()> {
    let mut buffer = [0_u8; 2048];
    let bytes_read = stream.read(&mut buffer)?;
    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let request_line = request.lines().next().unwrap_or_default();

    if request_line.starts_with("GET / ") || request_line == "GET / HTTP/1.1" {
        write_response(&mut stream, "200 OK", INDEX_BODY)
    } else {
        write_response(&mut stream, "404 Not Found", "Not Found")
    }
}

fn write_response(stream: &mut TcpStream, status: &str, body: &str) -> std::io::Result<()> {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()
}

fn open_browser_later(host: String, port: u16) {
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(500));
        let url = format!("http://{host}:{port}/");
        let mut command = browser_command(&url);
        let _ = command.spawn();
    });
}

fn browser_command(url: &str) -> Command {
    if cfg!(target_os = "windows") {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", "", url]);
        command
    } else if cfg!(target_os = "macos") {
        let mut command = Command::new("open");
        command.arg(url);
        command
    } else {
        let mut command = Command::new("xdg-open");
        command.arg(url);
        command
    }
}
