#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use shenyin::server::{self, ServerOptions};
use std::env;
use std::process::ExitCode;

const DEFAULT_HOST: &str = "127.0.0.1";
#[tokio::main]
async fn main() -> ExitCode {
    let options = match Options::parse(env::args().skip(1)) {
        Ok(options) => options,
        Err(error) => {
            report_startup_error(&error, true);
            return ExitCode::FAILURE;
        }
    };

    let server_options = ServerOptions {
        host: options.host.clone(),
        port: options.port,
        no_browser: options.no_browser,
        port_was_explicit: options.port_was_explicit,
    };

    match server::run(server_options).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            report_startup_error(&error.to_string(), !options.no_browser);
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug, Clone)]
struct Options {
    host: String,
    port: u16,
    no_browser: bool,
    port_was_explicit: bool,
}

impl Options {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut host = DEFAULT_HOST.to_owned();
        let mut port = default_port();
        let mut no_browser = false;
        let mut port_was_explicit = false;
        let mut args = args.into_iter();

        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--host" => {
                    host = args
                        .next()
                        .ok_or_else(|| "missing value for --host".to_owned())?;
                }
                "--port" => {
                    let value = args
                        .next()
                        .ok_or_else(|| "missing value for --port".to_owned())?;
                    port = value
                        .parse::<u16>()
                        .map_err(|_| format!("invalid value for --port: {value}"))?;
                    port_was_explicit = true;
                }
                "--no-browser" => no_browser = true,
                "--help" | "-h" => {
                    println!("Usage: shenyin [--host HOST] [--port PORT] [--no-browser]");
                    std::process::exit(0);
                }
                other => {
                    return Err(format!(
                        "unknown argument: {other}\n\nUsage: shenyin [--host HOST] [--port PORT] [--no-browser]"
                    ));
                }
            }
        }

        Ok(Self {
            host,
            port,
            no_browser,
            port_was_explicit,
        })
    }
}

fn default_port() -> u16 {
    std::env::var("SHENYIN_DEFAULT_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8765)
}

fn report_startup_error(message: &str, show_ui_error: bool) {
    eprintln!("{message}");

    #[cfg(not(target_os = "windows"))]
    let _ = show_ui_error;

    #[cfg(target_os = "windows")]
    if show_ui_error
        && std::env::var_os("SHENYIN_DISABLE_BROWSER").is_none()
        && std::env::var_os("SHENYIN_DISABLE_UI_ERROR").is_none()
    {
        show_windows_error_dialog(message);
    }
}

#[cfg(target_os = "windows")]
fn show_windows_error_dialog(message: &str) {
    let title = to_wide("ShenYin launch failed");
    let body = to_wide(message);

    unsafe {
        let _ = MessageBoxW(
            std::ptr::null_mut(),
            body.as_ptr(),
            title.as_ptr(),
            MB_ICONERROR | MB_OK,
        );
    }
}

#[cfg(target_os = "windows")]
fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(target_os = "windows")]
const MB_OK: u32 = 0x0000;
#[cfg(target_os = "windows")]
const MB_ICONERROR: u32 = 0x0010;

#[cfg(target_os = "windows")]
#[link(name = "user32")]
unsafe extern "system" {
    fn MessageBoxW(
        hwnd: *mut core::ffi::c_void,
        text: *const u16,
        caption: *const u16,
        kind: u32,
    ) -> i32;
}
