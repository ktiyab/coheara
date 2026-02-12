//! L4-03: WiFi Transfer — local HTTP server for phone-to-desktop document transfer.
//!
//! Starts a temporary axum server on the local network. The patient scans a QR code
//! with their phone, enters a 6-digit PIN, and uploads documents. Files are staged
//! in the profile directory for later processing through the import pipeline.
//!
//! Security model: PIN authentication + local network only + short-lived server.
//! Uses HTTP (not HTTPS) because self-signed TLS certs cause browser warnings on
//! phones (especially iOS Safari), making the feature unusable. WPA2/WPA3 provides
//! transport-layer encryption on the WiFi segment.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, DefaultBodyLimit, Multipart, State as AxumState};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::{oneshot, Mutex as TokioMutex};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Transfer session metadata — returned to frontend via IPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferSession {
    pub session_id: Uuid,
    pub server_addr: String,
    pub url: String,
    pub pin: String,
    pub started_at: chrono::NaiveDateTime,
    pub upload_count: u32,
    pub max_uploads: u32,
    pub timeout_secs: u64,
}

/// QR code data returned when starting a transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QrCodeData {
    pub url: String,
    pub pin: String,
    pub svg: String,
}

/// Result of a single file upload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadResult {
    pub filename: String,
    pub size_bytes: u64,
    pub mime_type: String,
    pub received_at: chrono::NaiveDateTime,
}

/// Transfer server configuration.
pub struct TransferConfig {
    pub max_file_size: u64,
    pub max_uploads: u32,
    pub timeout_secs: u64,
    pub allowed_mime_types: Vec<String>,
}

impl Default for TransferConfig {
    fn default() -> Self {
        Self {
            max_file_size: 50 * 1024 * 1024, // 50 MB
            max_uploads: 20,
            timeout_secs: 300, // 5 minutes
            allowed_mime_types: vec![
                "image/jpeg".into(),
                "image/png".into(),
                "image/webp".into(),
                "image/heic".into(),
                "application/pdf".into(),
            ],
        }
    }
}

/// Handle to a running transfer server. Stored in AppState.
pub struct TransferServer {
    pub session: TransferSession,
    shutdown_tx: Option<oneshot::Sender<()>>,
    upload_count: Arc<TokioMutex<u32>>,
    last_activity: Arc<TokioMutex<Instant>>,
    received_files: Arc<TokioMutex<Vec<UploadResult>>>,
}

impl TransferServer {
    /// Get current upload count.
    pub async fn upload_count(&self) -> u32 {
        *self.upload_count.lock().await
    }

    /// Get list of received files.
    pub async fn received_files(&self) -> Vec<UploadResult> {
        self.received_files.lock().await.clone()
    }

    /// Build a status snapshot with current upload count and received files.
    pub async fn status(&self) -> TransferStatusResponse {
        TransferStatusResponse {
            session: TransferSession {
                upload_count: *self.upload_count.lock().await,
                ..self.session.clone()
            },
            received_files: self.received_files.lock().await.clone(),
        }
    }

    /// Seconds since last activity on the server.
    pub async fn idle_secs(&self) -> u64 {
        self.last_activity.lock().await.elapsed().as_secs()
    }

    /// Shut down the server gracefully.
    pub fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
            tracing::info!("WiFi transfer server shutdown signal sent");
        }
    }
}

/// Transfer status returned by get_transfer_status command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferStatusResponse {
    pub session: TransferSession,
    pub received_files: Vec<UploadResult>,
}

// ---------------------------------------------------------------------------
// Internal types (axum server state)
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct ServerState {
    pin: String,
    upload_count: Arc<TokioMutex<u32>>,
    max_uploads: u32,
    max_file_size: u64,
    allowed_mime_types: Vec<String>,
    staging_dir: PathBuf,
    last_activity: Arc<TokioMutex<Instant>>,
    failed_attempts: Arc<TokioMutex<HashMap<IpAddr, u32>>>,
    received_files: Arc<TokioMutex<Vec<UploadResult>>>,
}

#[derive(Serialize)]
struct UploadResponse {
    success: bool,
    message: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

/// Max failed PIN attempts before blocking a client IP.
const MAX_PIN_ATTEMPTS: u32 = 5;

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// Generate a random 6-digit PIN.
pub fn generate_pin() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    format!("{:06}", rng.gen_range(0..1_000_000u32))
}

/// Check if an IP address is on a local (private) network.
pub fn is_local_network(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private(),
        IpAddr::V6(_) => false, // IPv4 only for simplicity
    }
}

/// Detect MIME type from file magic bytes (not extension or Content-Type header).
pub fn detect_mime_from_bytes(bytes: &[u8]) -> String {
    if bytes.len() < 4 {
        return "application/octet-stream".into();
    }

    // JPEG: FF D8 FF
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return "image/jpeg".into();
    }
    // PNG: 89 50 4E 47
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        return "image/png".into();
    }
    // PDF: %PDF
    if bytes.starts_with(b"%PDF") {
        return "application/pdf".into();
    }
    // WebP: RIFF....WEBP
    if bytes.len() >= 12 && bytes[..4] == *b"RIFF" && bytes[8..12] == *b"WEBP" {
        return "image/webp".into();
    }
    // HEIF/HEIC: ....ftyp at offset 4
    if bytes.len() >= 12 && bytes[4..8] == *b"ftyp" {
        if let Ok(brand) = std::str::from_utf8(&bytes[8..12]) {
            if brand.starts_with("heic")
                || brand.starts_with("heix")
                || brand.starts_with("mif1")
            {
                return "image/heic".into();
            }
        }
    }

    "application/octet-stream".into()
}

/// Sanitize a filename — removes path traversal and special characters.
pub fn sanitize_filename(name: &str) -> String {
    // Remove path separators and null bytes, replace other special chars
    let sanitized: String = name
        .chars()
        .filter(|&c| c != '/' && c != '\\' && c != '\0')
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();

    // Remove consecutive dots (path traversal prevention)
    let sanitized = sanitized.replace("..", "");

    // Truncate to 100 characters
    let sanitized = if sanitized.len() > 100 {
        sanitized[..100].to_string()
    } else {
        sanitized
    };

    if sanitized.is_empty() {
        "document".into()
    } else {
        sanitized
    }
}

/// Generate a QR code as an SVG string.
pub fn generate_qr_code(url: &str) -> Result<String, String> {
    use qrcode::render::svg;
    use qrcode::QrCode;

    let code =
        QrCode::new(url.as_bytes()).map_err(|e| format!("QR generation failed: {e}"))?;

    let svg_string = code
        .render::<svg::Color>()
        .min_dimensions(200, 200)
        .max_dimensions(300, 300)
        .dark_color(svg::Color("#1c1917"))
        .light_color(svg::Color("#ffffff"))
        .quiet_zone(true)
        .build();

    Ok(svg_string)
}

// ---------------------------------------------------------------------------
// Server lifecycle
// ---------------------------------------------------------------------------

/// Start the WiFi transfer server on the local network.
///
/// Binds to an ephemeral port on the local IP, generates a PIN and QR code,
/// and spawns the axum server in a background task. Returns a handle to
/// control the server.
pub async fn start_transfer_server(
    staging_dir: PathBuf,
    config: TransferConfig,
) -> Result<TransferServer, String> {
    // 1. Detect local IP
    let local_ip = local_ip_address::local_ip()
        .map_err(|e| format!("Cannot detect local IP: {e}"))?;

    if !is_local_network(&local_ip) {
        return Err(
            "Not on a local network. WiFi transfer requires a local network connection.".into(),
        );
    }

    // 2. Bind to an ephemeral port (OS selects available port)
    let listener = tokio::net::TcpListener::bind(SocketAddr::new(local_ip, 0))
        .await
        .map_err(|e| format!("Failed to bind server: {e}"))?;

    let addr = listener
        .local_addr()
        .map_err(|e| format!("Failed to get server address: {e}"))?;

    // 3. Generate 6-digit PIN
    let pin = generate_pin();

    // 4. Build URL
    let url = format!("http://{}:{}/upload", addr.ip(), addr.port());

    // 5. Create session metadata
    let session = TransferSession {
        session_id: Uuid::new_v4(),
        server_addr: addr.to_string(),
        url: url.clone(),
        pin: pin.clone(),
        started_at: chrono::Local::now().naive_local(),
        upload_count: 0,
        max_uploads: config.max_uploads,
        timeout_secs: config.timeout_secs,
    };

    // 6. Create shared state for axum handlers
    let upload_count = Arc::new(TokioMutex::new(0u32));
    let last_activity = Arc::new(TokioMutex::new(Instant::now()));
    let received_files = Arc::new(TokioMutex::new(Vec::<UploadResult>::new()));

    std::fs::create_dir_all(&staging_dir)
        .map_err(|e| format!("Failed to create staging directory: {e}"))?;

    let server_state = Arc::new(ServerState {
        pin,
        upload_count: upload_count.clone(),
        max_uploads: config.max_uploads,
        max_file_size: config.max_file_size,
        allowed_mime_types: config.allowed_mime_types,
        staging_dir,
        last_activity: last_activity.clone(),
        failed_attempts: Arc::new(TokioMutex::new(HashMap::new())),
        received_files: received_files.clone(),
    });

    // 7. Build router
    let app = Router::new()
        .route("/upload", get(serve_upload_page))
        .route("/upload", post(handle_upload))
        .route("/health", get(|| async { "ok" }))
        .layer(DefaultBodyLimit::max(55 * 1024 * 1024)) // 55 MB (multipart overhead)
        .with_state(server_state);

    // 8. Set up shutdown signal
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    let activity_for_timeout = last_activity.clone();
    let timeout_secs = config.timeout_secs;

    // 9. Spawn server in background task
    tokio::spawn(async move {
        let shutdown_signal = async move {
            let manual = async {
                let _ = shutdown_rx.await;
            };
            let timeout = async {
                loop {
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    let elapsed = activity_for_timeout.lock().await.elapsed().as_secs();
                    if elapsed > timeout_secs {
                        tracing::info!("Transfer server auto-shutdown: inactivity timeout");
                        return;
                    }
                }
            };
            tokio::select! {
                () = manual => { tracing::info!("Transfer server shutdown by user"); }
                () = timeout => {}
            }
        };

        if let Err(e) = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(shutdown_signal)
        .await
        {
            tracing::error!("Transfer server error: {e}");
        }
    });

    tracing::info!(addr = %addr, "WiFi transfer server started");

    Ok(TransferServer {
        session,
        shutdown_tx: Some(shutdown_tx),
        upload_count,
        last_activity,
        received_files,
    })
}

// ---------------------------------------------------------------------------
// Axum handlers
// ---------------------------------------------------------------------------

async fn serve_upload_page() -> Html<&'static str> {
    Html(UPLOAD_PAGE_HTML)
}

async fn handle_upload(
    ConnectInfo(client_addr): ConnectInfo<SocketAddr>,
    AxumState(state): AxumState<Arc<ServerState>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    // Update activity timestamp
    *state.last_activity.lock().await = Instant::now();

    let client_ip = client_addr.ip();

    // Brute force protection: block after MAX_PIN_ATTEMPTS wrong PINs
    {
        let attempts = state.failed_attempts.lock().await;
        if attempts.get(&client_ip).copied().unwrap_or(0) >= MAX_PIN_ATTEMPTS {
            return (
                StatusCode::FORBIDDEN,
                Json(ErrorResponse {
                    error: "Too many incorrect PINs. Please restart the transfer on your computer."
                        .into(),
                }),
            )
                .into_response();
        }
    }

    // Check upload limit
    {
        let count = state.upload_count.lock().await;
        if *count >= state.max_uploads {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(ErrorResponse {
                    error: "Upload limit reached for this session.".into(),
                }),
            )
                .into_response();
        }
    }

    // Parse multipart fields
    let mut pin_provided = String::new();
    let mut file_data: Option<(String, Vec<u8>)> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "pin" => {
                pin_provided = field.text().await.unwrap_or_default();
            }
            "file" => {
                let filename = field.file_name().unwrap_or("document").to_string();
                match field.bytes().await {
                    Ok(bytes) => {
                        file_data = Some((filename, bytes.to_vec()));
                    }
                    Err(e) => {
                        tracing::warn!("Failed to read upload bytes: {e}");
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(ErrorResponse {
                                error: "Failed to read file data.".into(),
                            }),
                        )
                            .into_response();
                    }
                }
            }
            _ => {}
        }
    }

    // Validate PIN
    if pin_provided != state.pin {
        let mut attempts = state.failed_attempts.lock().await;
        *attempts.entry(client_ip).or_insert(0) += 1;
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Incorrect PIN. Check the number shown on your computer.".into(),
            }),
        )
            .into_response();
    }

    // Validate file presence
    let (filename, bytes) = match file_data {
        Some(data) => data,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "No file provided.".into(),
                }),
            )
                .into_response();
        }
    };

    // Check file size
    if bytes.len() as u64 > state.max_file_size {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(ErrorResponse {
                error: format!(
                    "File too large. Maximum {}MB.",
                    state.max_file_size / (1024 * 1024)
                ),
            }),
        )
            .into_response();
    }

    // Validate MIME type via magic bytes
    let detected_mime = detect_mime_from_bytes(&bytes);
    if !state.allowed_mime_types.contains(&detected_mime) {
        return (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Json(ErrorResponse {
                error: "File type not supported. Please send an image or PDF.".into(),
            }),
        )
            .into_response();
    }

    // Sanitize filename and stage the file
    let safe_filename = sanitize_filename(&filename);

    if let Err(e) = std::fs::create_dir_all(&state.staging_dir) {
        tracing::error!("Failed to create staging dir: {e}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Failed to save file.".into(),
            }),
        )
            .into_response();
    }

    let staged_path = state
        .staging_dir
        .join(format!("{}_{}", Uuid::new_v4(), safe_filename));

    if let Err(e) = std::fs::write(&staged_path, &bytes) {
        tracing::error!("Failed to stage file: {e}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Failed to save file.".into(),
            }),
        )
            .into_response();
    }

    // Increment upload count
    {
        let mut count = state.upload_count.lock().await;
        *count += 1;
    }

    // Record received file
    let upload_result = UploadResult {
        filename: safe_filename.clone(),
        size_bytes: bytes.len() as u64,
        mime_type: detected_mime.clone(),
        received_at: chrono::Local::now().naive_local(),
    };
    state.received_files.lock().await.push(upload_result);

    tracing::info!(
        filename = %safe_filename,
        size = bytes.len(),
        mime = %detected_mime,
        "File received via WiFi transfer"
    );

    (
        StatusCode::OK,
        Json(UploadResponse {
            success: true,
            message: format!("Document received! {safe_filename}"),
        }),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// Upload page HTML (self-contained, mobile-optimized, no external resources)
// ---------------------------------------------------------------------------

const UPLOAD_PAGE_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1">
  <title>Coheara — Send Documents</title>
  <style>
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif;
      background: #fafaf9; color: #1c1917;
      min-height: 100vh; display: flex; flex-direction: column;
      align-items: center; justify-content: center; padding: 24px;
    }
    h1 { font-size: 24px; margin-bottom: 8px; }
    p { color: #78716c; font-size: 14px; margin-bottom: 24px; text-align: center; }
    .pin-input {
      display: flex; gap: 8px; justify-content: center; margin-bottom: 24px;
    }
    .pin-input input {
      width: 44px; height: 52px; text-align: center; font-size: 24px;
      border: 2px solid #d6d3d1; border-radius: 12px; outline: none;
      font-weight: 600;
    }
    .pin-input input:focus { border-color: #4a7c59; }
    .actions { display: flex; flex-direction: column; gap: 12px; width: 100%; max-width: 320px; }
    .btn {
      display: flex; align-items: center; justify-content: center;
      padding: 16px; border-radius: 12px; font-size: 16px; font-weight: 500;
      cursor: pointer; border: none; min-height: 56px; width: 100%;
    }
    .btn-primary { background: #4a7c59; color: white; }
    .btn-secondary { background: white; color: #44403c; border: 1px solid #d6d3d1; }
    .btn:disabled { opacity: 0.5; cursor: not-allowed; }
    .status { margin-top: 24px; text-align: center; }
    .status.success { color: #16a34a; }
    .status.error { color: #dc2626; }
    .progress { display: none; margin-top: 16px; }
    .progress-bar {
      height: 4px; background: #e7e5e4; border-radius: 2px; overflow: hidden;
    }
    .progress-fill {
      height: 100%; background: #4a7c59; transition: width 0.3s;
    }
    #file-input { display: none; }
    #camera-input { display: none; }
  </style>
</head>
<body>
  <h1>Send to Coheara</h1>
  <p>Enter the PIN shown on your computer, then choose a document to send.</p>

  <div class="pin-input" id="pin-container">
    <input type="tel" maxlength="1" data-index="0" autocomplete="off">
    <input type="tel" maxlength="1" data-index="1" autocomplete="off">
    <input type="tel" maxlength="1" data-index="2" autocomplete="off">
    <input type="tel" maxlength="1" data-index="3" autocomplete="off">
    <input type="tel" maxlength="1" data-index="4" autocomplete="off">
    <input type="tel" maxlength="1" data-index="5" autocomplete="off">
  </div>

  <div class="actions">
    <button class="btn btn-primary" id="btn-photo" disabled>
      Take a photo
    </button>
    <button class="btn btn-secondary" id="btn-gallery" disabled>
      Choose from gallery
    </button>
    <button class="btn btn-secondary" id="btn-file" disabled>
      Choose a file
    </button>
  </div>

  <input type="file" id="file-input" accept="image/*,application/pdf">
  <input type="file" id="camera-input" accept="image/*" capture="environment">

  <div class="progress" id="progress">
    <div class="progress-bar"><div class="progress-fill" id="progress-fill"></div></div>
  </div>

  <div class="status" id="status"></div>

  <script>
    var pinInputs = document.querySelectorAll('.pin-input input');
    var btnPhoto = document.getElementById('btn-photo');
    var btnGallery = document.getElementById('btn-gallery');
    var btnFile = document.getElementById('btn-file');
    var fileInput = document.getElementById('file-input');
    var cameraInput = document.getElementById('camera-input');
    var statusEl = document.getElementById('status');
    var progressEl = document.getElementById('progress');
    var progressFill = document.getElementById('progress-fill');

    var pin = '';

    pinInputs.forEach(function(input, i) {
      input.addEventListener('input', function(e) {
        if (e.target.value && i < 5) pinInputs[i + 1].focus();
        updatePin();
      });
      input.addEventListener('keydown', function(e) {
        if (e.key === 'Backspace' && !e.target.value && i > 0) {
          pinInputs[i - 1].focus();
        }
      });
    });

    function updatePin() {
      pin = Array.from(pinInputs).map(function(i) { return i.value; }).join('');
      var complete = pin.length === 6;
      btnPhoto.disabled = !complete;
      btnGallery.disabled = !complete;
      btnFile.disabled = !complete;
    }

    btnPhoto.addEventListener('click', function() { cameraInput.click(); });
    btnGallery.addEventListener('click', function() { fileInput.removeAttribute('capture'); fileInput.click(); });
    btnFile.addEventListener('click', function() { fileInput.removeAttribute('capture'); fileInput.click(); });

    cameraInput.addEventListener('change', handleFile);
    fileInput.addEventListener('change', handleFile);

    function handleFile(e) {
      var file = e.target.files[0];
      if (!file) return;

      if (file.size > 50 * 1024 * 1024) {
        showStatus('File too large. Maximum 50MB.', 'error');
        return;
      }

      var formData = new FormData();
      formData.append('file', file);
      formData.append('pin', pin);

      progressEl.style.display = 'block';
      progressFill.style.width = '0%';
      showStatus('Sending...', '');

      var xhr = new XMLHttpRequest();
      xhr.open('POST', '/upload');

      xhr.upload.onprogress = function(ev) {
        if (ev.lengthComputable) {
          progressFill.style.width = Math.round((ev.loaded / ev.total) * 100) + '%';
        }
      };

      xhr.onload = function() {
        progressEl.style.display = 'none';
        if (xhr.status === 200) {
          showStatus('Sent! You can send another or close this page.', 'success');
        } else {
          try {
            var resp = JSON.parse(xhr.responseText);
            showStatus(resp.error || 'Upload failed', 'error');
          } catch (_) {
            showStatus('Upload failed', 'error');
          }
        }
      };

      xhr.onerror = function() {
        progressEl.style.display = 'none';
        showStatus('Connection failed. Make sure your phone and computer are on the same WiFi.', 'error');
      };

      xhr.send(formData);
      e.target.value = '';
    }

    function showStatus(text, type) {
      statusEl.textContent = text;
      statusEl.className = 'status ' + type;
    }

    pinInputs[0].focus();
  </script>
</body>
</html>"#;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- PIN generation -------------------------------------------------------

    #[test]
    fn pin_format_is_six_digits() {
        let pin = generate_pin();
        assert_eq!(pin.len(), 6);
        assert!(pin.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn pin_is_random() {
        let pin1 = generate_pin();
        let pin2 = generate_pin();
        // 1-in-1,000,000 chance of collision — acceptable flake risk
        assert_ne!(pin1, pin2);
    }

    // -- Local network validation ---------------------------------------------

    #[test]
    fn private_ipv4_is_local() {
        assert!(is_local_network(&"192.168.1.1".parse().unwrap()));
        assert!(is_local_network(&"192.168.0.100".parse().unwrap()));
        assert!(is_local_network(&"10.0.0.1".parse().unwrap()));
        assert!(is_local_network(&"10.255.255.255".parse().unwrap()));
        assert!(is_local_network(&"172.16.0.1".parse().unwrap()));
        assert!(is_local_network(&"172.31.255.255".parse().unwrap()));
    }

    #[test]
    fn public_ipv4_is_not_local() {
        assert!(!is_local_network(&"8.8.8.8".parse().unwrap()));
        assert!(!is_local_network(&"1.1.1.1".parse().unwrap()));
        assert!(!is_local_network(&"203.0.113.1".parse().unwrap()));
        assert!(!is_local_network(&"172.32.0.1".parse().unwrap()));
    }

    #[test]
    fn ipv6_is_not_supported() {
        assert!(!is_local_network(&"::1".parse().unwrap()));
        assert!(!is_local_network(&"fe80::1".parse().unwrap()));
    }

    // -- MIME detection -------------------------------------------------------

    #[test]
    fn detect_jpeg() {
        assert_eq!(
            detect_mime_from_bytes(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10]),
            "image/jpeg"
        );
    }

    #[test]
    fn detect_png() {
        assert_eq!(
            detect_mime_from_bytes(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A]),
            "image/png"
        );
    }

    #[test]
    fn detect_pdf() {
        assert_eq!(
            detect_mime_from_bytes(b"%PDF-1.4 some content"),
            "application/pdf"
        );
    }

    #[test]
    fn detect_webp() {
        let mut bytes = vec![0u8; 12];
        bytes[..4].copy_from_slice(b"RIFF");
        bytes[8..12].copy_from_slice(b"WEBP");
        assert_eq!(detect_mime_from_bytes(&bytes), "image/webp");
    }

    #[test]
    fn detect_heic() {
        let mut bytes = vec![0u8; 12];
        bytes[4..8].copy_from_slice(b"ftyp");
        bytes[8..12].copy_from_slice(b"heic");
        assert_eq!(detect_mime_from_bytes(&bytes), "image/heic");
    }

    #[test]
    fn detect_heic_mif1_brand() {
        let mut bytes = vec![0u8; 12];
        bytes[4..8].copy_from_slice(b"ftyp");
        bytes[8..12].copy_from_slice(b"mif1");
        assert_eq!(detect_mime_from_bytes(&bytes), "image/heic");
    }

    #[test]
    fn detect_unknown_bytes() {
        assert_eq!(
            detect_mime_from_bytes(&[0x00, 0x01, 0x02, 0x03]),
            "application/octet-stream"
        );
    }

    #[test]
    fn detect_too_short() {
        assert_eq!(detect_mime_from_bytes(&[0xFF]), "application/octet-stream");
        assert_eq!(detect_mime_from_bytes(&[]), "application/octet-stream");
    }

    // -- Filename sanitization ------------------------------------------------

    #[test]
    fn sanitize_path_traversal() {
        let result = sanitize_filename("../../../etc/passwd");
        assert!(!result.contains(".."));
        assert!(!result.contains('/'));
    }

    #[test]
    fn sanitize_special_chars() {
        assert_eq!(sanitize_filename("my file (1).jpg"), "my_file__1_.jpg");
    }

    #[test]
    fn sanitize_long_name() {
        let long_name = "a".repeat(200);
        let result = sanitize_filename(&long_name);
        assert!(result.len() <= 100);
    }

    #[test]
    fn sanitize_empty_name() {
        assert_eq!(sanitize_filename(""), "document");
    }

    #[test]
    fn sanitize_preserves_valid_name() {
        assert_eq!(sanitize_filename("photo.jpg"), "photo.jpg");
        assert_eq!(sanitize_filename("scan-2024.pdf"), "scan-2024.pdf");
        assert_eq!(sanitize_filename("doc_v2.png"), "doc_v2.png");
    }

    #[test]
    fn sanitize_null_bytes() {
        let result = sanitize_filename("file\0name.jpg");
        assert!(!result.contains('\0'));
        assert_eq!(result, "filename.jpg");
    }

    #[test]
    fn sanitize_backslash_path() {
        let result = sanitize_filename("C:\\Users\\test\\file.jpg");
        assert!(!result.contains('\\'));
    }

    // -- QR code generation ---------------------------------------------------

    #[test]
    fn qr_code_generates_svg() {
        let svg = generate_qr_code("http://192.168.1.100:8080/upload").unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
    }

    #[test]
    fn qr_code_handles_empty_url() {
        // Empty URL should still produce a valid QR code
        let result = generate_qr_code("");
        assert!(result.is_ok());
    }

    // -- Config defaults ------------------------------------------------------

    #[test]
    fn default_config_values() {
        let config = TransferConfig::default();
        assert_eq!(config.max_file_size, 50 * 1024 * 1024);
        assert_eq!(config.max_uploads, 20);
        assert_eq!(config.timeout_secs, 300);
        assert_eq!(config.allowed_mime_types.len(), 5);
        assert!(config.allowed_mime_types.contains(&"image/jpeg".to_string()));
        assert!(config.allowed_mime_types.contains(&"application/pdf".to_string()));
    }

    // -- TransferServer -------------------------------------------------------

    fn test_session() -> TransferSession {
        TransferSession {
            session_id: Uuid::new_v4(),
            server_addr: "192.168.1.1:8080".into(),
            url: "http://192.168.1.1:8080/upload".into(),
            pin: "123456".into(),
            started_at: chrono::Local::now().naive_local(),
            upload_count: 0,
            max_uploads: 20,
            timeout_secs: 300,
        }
    }

    #[tokio::test]
    async fn server_shutdown_sends_signal() {
        let (tx, rx) = oneshot::channel();
        let mut server = TransferServer {
            session: test_session(),
            shutdown_tx: Some(tx),
            upload_count: Arc::new(TokioMutex::new(0)),
            last_activity: Arc::new(TokioMutex::new(Instant::now())),
            received_files: Arc::new(TokioMutex::new(Vec::new())),
        };

        server.shutdown();
        assert!(rx.await.is_ok());

        // Second shutdown is safe (no-op)
        server.shutdown();
    }

    #[tokio::test]
    async fn server_tracks_upload_count() {
        let count = Arc::new(TokioMutex::new(5u32));
        let server = TransferServer {
            session: test_session(),
            shutdown_tx: None,
            upload_count: count,
            last_activity: Arc::new(TokioMutex::new(Instant::now())),
            received_files: Arc::new(TokioMutex::new(Vec::new())),
        };

        assert_eq!(server.upload_count().await, 5);
    }

    #[tokio::test]
    async fn server_tracks_received_files() {
        let files = Arc::new(TokioMutex::new(vec![UploadResult {
            filename: "test.jpg".into(),
            size_bytes: 1024,
            mime_type: "image/jpeg".into(),
            received_at: chrono::Local::now().naive_local(),
        }]));

        let server = TransferServer {
            session: test_session(),
            shutdown_tx: None,
            upload_count: Arc::new(TokioMutex::new(1)),
            last_activity: Arc::new(TokioMutex::new(Instant::now())),
            received_files: files,
        };

        let received = server.received_files().await;
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].filename, "test.jpg");
    }

    #[tokio::test]
    async fn server_status_reflects_live_count() {
        let count = Arc::new(TokioMutex::new(3u32));
        let server = TransferServer {
            session: test_session(),
            shutdown_tx: None,
            upload_count: count.clone(),
            last_activity: Arc::new(TokioMutex::new(Instant::now())),
            received_files: Arc::new(TokioMutex::new(Vec::new())),
        };

        let status = server.status().await;
        assert_eq!(status.session.upload_count, 3);

        // Simulate another upload
        *count.lock().await = 4;
        let status = server.status().await;
        assert_eq!(status.session.upload_count, 4);
    }
}
