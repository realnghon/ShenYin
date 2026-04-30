use crate::crypto::{self, CryptoResult, EncryptRequest};
use crate::result_store::{ResultStore, ResultStoreError};
use crate::transport::{self, TextPayload};
use axum::body::Body;
use axum::extract::{Multipart, Path as AxumPath, Query, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, Notify};

const BODY_LIMIT: usize = 1024 * 1024 * 1024;
const INDEX_TEMPLATE: &str = include_str!("../templates/index.html");
const APP_JS: &str = include_str!("../static/app.js");
const STYLE_CSS: &str = include_str!("../static/style.css");

#[derive(Debug, Clone)]
pub struct ServerOptions {
    pub host: String,
    pub port: u16,
    pub no_browser: bool,
    pub port_was_explicit: bool,
}

impl ServerOptions {
    pub fn url(&self) -> String {
        format!("http://{}:{}/", self.host, self.port)
    }

    pub fn can_reuse_existing_instance(&self) -> bool {
        !self.no_browser && !self.port_was_explicit && self.host == "127.0.0.1"
    }
}

#[derive(Clone)]
struct AppState {
    result_store: Arc<ResultStore>,
    shutdown: ShutdownSignal,
    app_session: Option<Arc<AppSession>>,
}

#[derive(Clone)]
struct ShutdownSignal {
    requested: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

impl ShutdownSignal {
    fn new() -> Self {
        Self {
            requested: Arc::new(AtomicBool::new(false)),
            notify: Arc::new(Notify::new()),
        }
    }

    fn request_shutdown(&self) {
        self.requested.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    async fn wait(&self) {
        if self.requested.load(Ordering::SeqCst) {
            return;
        }
        self.notify.notified().await;
    }
}

struct AppSession {
    token: String,
    last_seen: Mutex<SystemTime>,
    heartbeat_seen: AtomicBool,
    closed: AtomicBool,
}

impl AppSession {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            token: uuid::Uuid::new_v4().simple().to_string(),
            last_seen: Mutex::new(SystemTime::now()),
            heartbeat_seen: AtomicBool::new(false),
            closed: AtomicBool::new(false),
        })
    }

    async fn heartbeat(&self) {
        *self.last_seen.lock().await = SystemTime::now();
        self.heartbeat_seen.store(true, Ordering::SeqCst);
    }

    fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
    }

    fn token(&self) -> &str {
        &self.token
    }

    fn heartbeat_seen(&self) -> bool {
        self.heartbeat_seen.load(Ordering::SeqCst)
    }
}

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("{0}")]
    Message(String),
}

pub async fn run(options: ServerOptions) -> Result<(), ServerError> {
    let address = format!("{}:{}", options.host, options.port);
    let listener = match TcpListener::bind(&address).await {
        Ok(listener) => listener,
        Err(error) => {
            if options.can_reuse_existing_instance()
                && shenyin_instance_is_available(&options.host, options.port).await
            {
                open_browser_now(&options.url());
                return Ok(());
            }
            return Err(ServerError::Message(format!(
                "failed to bind {}:{}: {error}",
                options.host, options.port
            )));
        }
    };

    let shutdown = ShutdownSignal::new();
    let app_session = if options.no_browser {
        None
    } else {
        Some(AppSession::new())
    };

    let result_store = Arc::new(
        ResultStore::new(result_store_root(), 24)
            .map_err(|error| ServerError::Message(error.to_string()))?,
    );
    let state = AppState {
        result_store,
        shutdown: shutdown.clone(),
        app_session: app_session.clone(),
    };

    if let Some(app_session) = &app_session {
        let url = format!("{}?session={}", options.url(), app_session.token());
        open_browser_later(url);

        let shutdown_for_task = shutdown.clone();
        let session_for_task = app_session.clone();
        tokio::spawn(async move {
            watch_app_session(session_for_task, shutdown_for_task).await;
        });
    }

    let app = router(state);
    let graceful_shutdown = shutdown.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move { graceful_shutdown.wait().await })
        .await
        .map_err(|error| ServerError::Message(error.to_string()))
}

fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/static/app.js", get(app_js))
        .route("/static/style.css", get(style_css))
        .route("/api/encrypt", post(encrypt))
        .route("/api/decrypt", post(decrypt))
        .route("/api/results/{token}", get(download_result))
        .route("/api/app-session/heartbeat", post(app_session_heartbeat))
        .route("/api/app-session/close", post(app_session_close))
        .layer(axum::extract::DefaultBodyLimit::max(BODY_LIMIT))
        .with_state(state)
}

async fn index(
    State(state): State<AppState>,
    Query(_query): Query<HashMap<String, String>>,
) -> Html<String> {
    let boot = if let Some(session) = &state.app_session {
        format!(r#"{{"session":"{}"}}"#, session.token())
    } else {
        "{}".to_owned()
    };

    let html = INDEX_TEMPLATE.replace("__SHENYIN_BOOT__", &boot);
    Html(html)
}

async fn app_js() -> Response {
    (
        [(
            CONTENT_TYPE,
            HeaderValue::from_static("application/javascript; charset=utf-8"),
        )],
        APP_JS,
    )
        .into_response()
}

async fn style_css() -> Response {
    (
        [(
            CONTENT_TYPE,
            HeaderValue::from_static("text/css; charset=utf-8"),
        )],
        STYLE_CSS,
    )
        .into_response()
}

async fn encrypt(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Json<ApiEnvelope>, ApiError> {
    let form = parse_multipart(multipart).await?;
    let input_type = form.text("input_type").unwrap_or("text").to_owned();
    let compression = form.text("compression").unwrap_or("zlib").to_owned();
    let output_format = match form.text("output_format") {
        Some("binary") => "binary",
        Some("armor") => "armor",
        _ if input_type == "text" => "armor",
        _ => "binary",
    };

    let text_value = if let Some(text) = form.text("text_input") {
        text.to_owned()
    } else if let Some(upload) = form.file("text_input_file") {
        String::from_utf8(upload.bytes.clone())
            .map_err(|_| ApiError::bad_request("文本模式只支持 UTF-8 文本内容。"))?
    } else {
        String::new()
    };

    let file_upload = form.file("file_input");
    if input_type == "file" && file_upload.is_none() {
        return Err(ApiError::bad_request("请选择一个要加密的文件。"));
    }

    let request = EncryptRequest {
        input_type,
        armor: output_format == "armor",
        compression_name: compression,
        passphrase: form.text("passphrase").unwrap_or("").to_owned(),
        text_value,
        file_name: file_upload.map(|upload| upload.file_name.clone()),
        file_bytes: file_upload.map(|upload| upload.bytes.clone()),
    };

    let result = crypto::encrypt_content(request).map_err(ApiError::from_crypto)?;
    let payload = serialize_result(&state, result)?;
    Ok(Json(ApiEnvelope::success("加密完成。", payload)))
}

async fn decrypt(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Json<ApiEnvelope>, ApiError> {
    let form = parse_multipart(multipart).await?;
    let input_type = form.text("input_type").unwrap_or("text").to_owned();
    let encrypted_blob = if input_type == "text" {
        if let Some(text) = form.text("ciphertext_text") {
            text.as_bytes().to_vec()
        } else if let Some(upload) = form.file("ciphertext_text_file") {
            upload.bytes.clone()
        } else {
            Vec::new()
        }
    } else if let Some(upload) = form.file("ciphertext_file") {
        upload.bytes.clone()
    } else {
        return Err(ApiError::bad_request("请选择一个要解密的文件。"));
    };

    let result = crypto::decrypt_content(&encrypted_blob, form.text("passphrase").unwrap_or(""))
        .map_err(ApiError::from_crypto)?;
    let payload = serialize_result(&state, result)?;
    Ok(Json(ApiEnvelope::success("解密完成。", payload)))
}

async fn download_result(
    State(state): State<AppState>,
    AxumPath(token): AxumPath<String>,
) -> Result<Response, ApiError> {
    let stored = state
        .result_store
        .get(&token)
        .map_err(ApiError::from_result_store)?;
    let bytes = fs::read(&stored.path).map_err(|error| ApiError::internal(error.to_string()))?;

    let disposition = format!("attachment; filename=\"{}\"", stored.filename);
    let disposition = HeaderValue::from_str(&disposition)
        .map_err(|error| ApiError::internal(error.to_string()))?;
    let content_type = HeaderValue::from_str(&stored.mime_type)
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));

    Ok((
        [
            (CONTENT_TYPE, content_type),
            (CONTENT_DISPOSITION, disposition),
        ],
        Body::from(bytes),
    )
        .into_response())
}

async fn app_session_heartbeat(
    State(state): State<AppState>,
    Json(_payload): Json<AppSessionPayload>,
) -> StatusCode {
    if let Some(app_session) = &state.app_session {
        app_session.heartbeat().await;
        return StatusCode::NO_CONTENT;
    }
    StatusCode::NOT_FOUND
}

async fn app_session_close(
    State(state): State<AppState>,
    Json(_payload): Json<AppSessionPayload>,
) -> StatusCode {
    if let Some(app_session) = &state.app_session {
        app_session.close();
        state.shutdown.request_shutdown();
        return StatusCode::NO_CONTENT;
    }
    StatusCode::NOT_FOUND
}

fn serialize_result(state: &AppState, result: CryptoResult) -> Result<ApiResultPayload, ApiError> {
    let stored = state
        .result_store
        .save(&result.content, &result.filename, &result.mime_type)
        .map_err(ApiError::from_result_store)?;

    let mut payload = ApiResultPayload {
        kind: result.kind,
        filename: stored.filename,
        size: stored.size,
        download_url: format!("/api/results/{}", stored.token),
        text_available: None,
        text_too_large: None,
        text_length: None,
        text: None,
    };

    if let Some(text) = result.inline_text {
        let text_payload = transport::extract_text_payload(&text);
        payload.apply_text_payload(text_payload);
    }

    Ok(payload)
}

async fn parse_multipart(mut multipart: Multipart) -> Result<ParsedMultipart, ApiError> {
    let mut parsed = ParsedMultipart::default();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| ApiError::bad_request(error.to_string()))?
    {
        let name = field.name().unwrap_or_default().to_owned();
        if name.is_empty() {
            continue;
        }

        let file_name = field.file_name().map(str::to_owned);
        let bytes = field
            .bytes()
            .await
            .map_err(|error| ApiError::bad_request(error.to_string()))?
            .to_vec();

        if let Some(file_name) = file_name {
            parsed.files.insert(name, UploadedFile { file_name, bytes });
        } else {
            let value = String::from_utf8(bytes)
                .map_err(|_| ApiError::bad_request("表单文本必须是 UTF-8。"))?;
            parsed.text.insert(name, value);
        }
    }

    Ok(parsed)
}

async fn watch_app_session(app_session: Arc<AppSession>, shutdown: ShutdownSignal) {
    const FIRST_HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(45);
    const HEARTBEAT_STALE_TIMEOUT: Duration = Duration::from_secs(8);

    loop {
        if app_session.closed.load(Ordering::SeqCst) {
            shutdown.request_shutdown();
            break;
        }

        let last_seen = *app_session.last_seen.lock().await;
        let elapsed = last_seen.elapsed().unwrap_or(Duration::ZERO);
        let timeout = if app_session.heartbeat_seen() {
            HEARTBEAT_STALE_TIMEOUT
        } else {
            FIRST_HEARTBEAT_TIMEOUT
        };

        if elapsed.gt(&timeout) {
            shutdown.request_shutdown();
            break;
        }

        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

#[derive(Debug, Serialize)]
struct ApiEnvelope {
    ok: bool,
    message: String,
    result: ApiResultPayload,
}

impl ApiEnvelope {
    fn success(message: &str, result: ApiResultPayload) -> Self {
        Self {
            ok: true,
            message: message.to_owned(),
            result,
        }
    }
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope {
    ok: bool,
    message: String,
}

#[derive(Debug, Serialize)]
pub struct ApiResultPayload {
    kind: String,
    filename: String,
    size: usize,
    download_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    text_available: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text_too_large: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text_length: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
}

impl ApiResultPayload {
    fn apply_text_payload(&mut self, text_payload: TextPayload) {
        self.text_available = Some(text_payload.text_available);
        self.text_too_large = Some(text_payload.text_too_large);
        self.text_length = Some(text_payload.text_length);
        self.text = text_payload.text;
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AppSessionPayload {
    session_id: String,
}

#[derive(Default)]
struct ParsedMultipart {
    text: HashMap<String, String>,
    files: HashMap<String, UploadedFile>,
}

impl ParsedMultipart {
    fn text(&self, name: &str) -> Option<&str> {
        self.text.get(name).map(String::as_str)
    }

    fn file(&self, name: &str) -> Option<&UploadedFile> {
        self.files.get(name)
    }
}

struct UploadedFile {
    file_name: String,
    bytes: Vec<u8>,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("服务端处理失败：{}", message.into()),
        }
    }

    fn from_crypto(error: crypto::CryptoError) -> Self {
        Self::bad_request(error.to_string())
    }

    fn from_result_store(error: ResultStoreError) -> Self {
        match error {
            ResultStoreError::NotFound => Self {
                status: StatusCode::NOT_FOUND,
                message: error.to_string(),
            },
            _ => Self::internal(error.to_string()),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorEnvelope {
                ok: false,
                message: self.message,
            }),
        )
            .into_response()
    }
}

pub fn runtime_root() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(PathBuf::from))
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn result_store_root() -> PathBuf {
    if cfg!(target_os = "macos") {
        if let Some(home_dir) = std::env::var_os("HOME") {
            return PathBuf::from(home_dir)
                .join("Library")
                .join("Application Support")
                .join("ShenYin")
                .join("results");
        }
    } else if cfg!(target_os = "windows") {
        if let Some(app_data) = std::env::var_os("LOCALAPPDATA").or_else(|| std::env::var_os("APPDATA")) {
            return PathBuf::from(app_data).join("ShenYin").join("results");
        }
    } else if let Some(state_home) = std::env::var_os("XDG_STATE_HOME") {
        return PathBuf::from(state_home).join("ShenYin").join("results");
    } else if let Some(home_dir) = std::env::var_os("HOME") {
        return PathBuf::from(home_dir)
            .join(".local")
            .join("state")
            .join("ShenYin")
            .join("results");
    }

    runtime_root().join("data").join("results")
}

fn open_browser_later(url: String) {
    if browser_launch_disabled() {
        return;
    }

    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(500));
        open_browser_now(&url);
    });
}

pub fn open_browser_now(url: &str) {
    if browser_launch_disabled() {
        return;
    }

    let mut command = browser_command(url);
    let _ = command.spawn();
}

fn browser_launch_disabled() -> bool {
    std::env::var_os("SHENYIN_DISABLE_BROWSER").is_some()
}

fn browser_command(url: &str) -> std::process::Command {
    if cfg!(target_os = "windows") {
        let mut command = std::process::Command::new("cmd");
        command.args(["/C", "start", "", url]);
        command
    } else if cfg!(target_os = "macos") {
        let mut command = std::process::Command::new("/usr/bin/open");
        command.arg(url);
        command
    } else {
        let mut command = std::process::Command::new("xdg-open");
        command.arg(url);
        command
    }
}

async fn shenyin_instance_is_available(host: &str, port: u16) -> bool {
    let address = format!("{host}:{port}");
    let mut stream = match tokio::net::TcpStream::connect(address).await {
        Ok(stream) => stream,
        Err(_) => return false,
    };

    if tokio::io::AsyncWriteExt::write_all(
        &mut stream,
        format!("GET / HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n").as_bytes(),
    )
    .await
    .is_err()
    {
        return false;
    }

    let mut response = Vec::new();
    if tokio::io::AsyncReadExt::read_to_end(&mut stream, &mut response)
        .await
        .is_err()
    {
        return false;
    }

    let response = String::from_utf8_lossy(&response);
    response.starts_with("HTTP/1.1 200") && response.contains("ShenYin")
}
