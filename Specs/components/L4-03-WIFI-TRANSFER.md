# L4-03 — WiFi Transfer

<!--
=============================================================================
COMPONENT SPEC — The phone-to-desktop bridge.
Engineer review: E-RS (Rust, lead), E-SC (Security), E-UX (UI/UX), E-QA (QA)
Marie takes a photo of her prescription with her phone. Coheara receives it
via local WiFi without touching the internet. QR code makes it effortless.
This component runs a temporary HTTP server — security is non-negotiable.
=============================================================================
-->

## Table of Contents

| Section | Offset |
|---------|--------|
| [1] Identity | `offset=20 limit=30` |
| [2] Dependencies | `offset=50 limit=22` |
| [3] Interfaces | `offset=72 limit=70` |
| [4] Server Lifecycle | `offset=142 limit=60` |
| [5] QR Code Generation | `offset=202 limit=40` |
| [6] Mobile Upload Page | `offset=242 limit=80` |
| [7] File Reception & Validation | `offset=322 limit=55` |
| [8] Security | `offset=377 limit=70` |
| [9] Tauri Commands (IPC) | `offset=447 limit=65` |
| [10] Svelte Components | `offset=512 limit=90` |
| [11] Error Handling | `offset=602 limit=30` |
| [12] Testing | `offset=632 limit=50` |
| [13] Performance | `offset=682 limit=15` |
| [14] Open Questions | `offset=697 limit=10` |

---

## [1] Identity

**What:** Local WiFi transfer — a temporary HTTP server on the desktop that lets patients upload documents from their phone by scanning a QR code. The phone never installs anything — it opens a mobile-optimized upload page in the browser. Files transfer over local network only. Server starts on demand, auto-shuts down after inactivity.

**After this session:**
- Patient taps "Receive from phone" on desktop
- Desktop starts local HTTPS server on random port
- Desktop generates QR code + displays URL + 6-digit PIN
- Patient scans QR code with phone camera
- Phone browser opens upload page (responsive HTML)
- Patient takes photo or selects file, enters PIN, uploads
- Desktop receives file, validates, and enters document pipeline
- Server auto-shuts down after 5 minutes of inactivity or patient clicks "Done"
- Maximum 20 uploads per session
- Maximum 50MB per file

**Estimated complexity:** Medium
**Source:** Tech Spec v1.1 Section 10 (Local WiFi Transfer), Section 11.3 (Transfer Security)

---

## [2] Dependencies

**Incoming:**
- L0-03 (encryption — ProfileSession for encrypting received files)
- L1-01 (document import — files enter the import pipeline after reception)
- L3-01 (profile management — active session required)

**Outgoing:**
- L1-01 → L1-02 → L1-03 → L1-04 (received files enter full document pipeline)
- L3-02 (home screen — shows "Document received!" notification)

**New Cargo.toml dependencies:**
```toml
# HTTP server
axum = { version = "0.7", features = ["multipart"] }
axum-server = { version = "0.6", features = ["tls-rustls"] }

# TLS
rcgen = "0.13"             # Self-signed certificate generation
rustls = "0.23"

# QR code
qrcode = "0.14"

# Network
local-ip-address = "0.6"  # Detect local network IP
```

---

## [3] Interfaces

### Backend Types

```rust
// src-tauri/src/wifi_transfer.rs

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, oneshot};
use uuid::Uuid;

/// Transfer session state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferSession {
    pub session_id: Uuid,
    pub server_addr: SocketAddr,
    pub url: String,            // Full URL for QR code
    pub pin: String,            // 6-digit PIN
    pub started_at: chrono::NaiveDateTime,
    pub upload_count: u32,
    pub max_uploads: u32,       // Default: 20
    pub timeout_secs: u64,      // Default: 300 (5 min)
}

/// Upload result for a single file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadResult {
    pub filename: String,
    pub size_bytes: u64,
    pub mime_type: String,
    pub received_at: chrono::NaiveDateTime,
    pub staged_path: PathBuf,  // Where file is staged for pipeline
}

/// Server control handle
pub struct TransferServer {
    pub session: TransferSession,
    pub shutdown_tx: oneshot::Sender<()>,
    pub last_activity: Arc<Mutex<std::time::Instant>>,
}

/// Configuration
pub struct TransferConfig {
    pub max_file_size: u64,     // 50 MB default
    pub max_uploads: u32,       // 20 default
    pub timeout_secs: u64,      // 300 (5 min) default
    pub allowed_mime_types: Vec<String>,
}

impl Default for TransferConfig {
    fn default() -> Self {
        Self {
            max_file_size: 50 * 1024 * 1024,  // 50 MB
            max_uploads: 20,
            timeout_secs: 300,
            allowed_mime_types: vec![
                "image/jpeg".into(),
                "image/png".into(),
                "image/webp".into(),
                "image/heic".into(),
                "image/heif".into(),
                "application/pdf".into(),
            ],
        }
    }
}

/// QR code data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QrCodeData {
    pub url: String,
    pub pin: String,
    pub svg: String,  // QR code as SVG string
}
```

### Frontend Types

```typescript
// src/lib/types/transfer.ts

export interface TransferSession {
  session_id: string;
  server_addr: string;
  url: string;
  pin: string;
  started_at: string;
  upload_count: number;
  max_uploads: number;
  timeout_secs: number;
}

export interface QrCodeData {
  url: string;
  pin: string;
  svg: string;
}

export interface UploadResult {
  filename: string;
  size_bytes: number;
  mime_type: string;
  received_at: string;
}

export type TransferStatus = 'idle' | 'starting' | 'active' | 'stopping' | 'error';
```

---

## [4] Server Lifecycle

### Startup Sequence

```rust
use axum::{
    Router, extract::{Multipart, State as AxumState},
    response::{Html, IntoResponse},
    routing::{get, post},
};
use axum_server::tls_rustls::RustlsConfig;
use local_ip_address::local_ip;

/// Starts the transfer server
pub async fn start_transfer_server(
    profile_session: &ProfileSession,
    config: TransferConfig,
) -> Result<TransferServer, CohearaError> {
    // 1. Detect local IP
    let local_ip = local_ip()
        .map_err(|e| CohearaError::Network(format!("Cannot detect local IP: {e}")))?;

    // Validate it's a local network address
    if !is_local_network(&local_ip) {
        return Err(CohearaError::Network(
            "Not on a local network. WiFi transfer requires a local network.".into()
        ));
    }

    // 2. Generate random port (49152-65535 ephemeral range)
    let port = {
        let listener = std::net::TcpListener::bind(format!("{}:0", local_ip))?;
        listener.local_addr()?.port()
    };

    let addr = SocketAddr::new(local_ip, port);

    // 3. Generate 6-digit PIN
    let pin = generate_pin();

    // 4. Generate self-signed TLS certificate
    let (cert_pem, key_pem) = generate_self_signed_cert(&local_ip.to_string())?;
    let tls_config = RustlsConfig::from_pem(cert_pem.into(), key_pem.into()).await?;

    // 5. Build URL
    let url = format!("https://{}:{}/upload", local_ip, port);

    // 6. Create session
    let session_id = Uuid::new_v4();
    let session = TransferSession {
        session_id,
        server_addr: addr,
        url: url.clone(),
        pin: pin.clone(),
        started_at: chrono::Local::now().naive_local(),
        upload_count: 0,
        max_uploads: config.max_uploads,
        timeout_secs: config.timeout_secs,
    };

    // 7. Shared state for the server
    let shared_state = Arc::new(ServerState {
        pin: pin.clone(),
        upload_count: Mutex::new(0),
        max_uploads: config.max_uploads,
        max_file_size: config.max_file_size,
        allowed_mime_types: config.allowed_mime_types.clone(),
        staging_dir: profile_session.profile_data_dir().join("staging"),
        last_activity: Arc::new(Mutex::new(std::time::Instant::now())),
    });

    // 8. Build router
    let app = Router::new()
        .route("/upload", get(serve_upload_page))
        .route("/upload", post(handle_upload))
        .route("/health", get(|| async { "ok" }))
        .with_state(shared_state.clone());

    // 9. Start server with shutdown signal
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    let last_activity = shared_state.last_activity.clone();
    let timeout = config.timeout_secs;

    // Spawn timeout watcher
    let activity_clone = last_activity.clone();
    let shutdown_signal = async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            let elapsed = activity_clone.lock().await.elapsed().as_secs();
            if elapsed > timeout {
                tracing::info!("Transfer server auto-shutdown: inactivity timeout");
                break;
            }
        }
    };

    // Spawn server
    tokio::spawn(async move {
        let server = axum_server::bind_rustls(addr, tls_config)
            .serve(app.into_make_service());

        tokio::select! {
            result = server => {
                if let Err(e) = result {
                    tracing::error!("Transfer server error: {e}");
                }
            }
            _ = shutdown_rx => {
                tracing::info!("Transfer server shutdown by user");
            }
            _ = shutdown_signal => {}
        }
    });

    Ok(TransferServer {
        session,
        shutdown_tx,
        last_activity,
    })
}

/// Validates IP is on local network
fn is_local_network(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            v4.is_private()  // 10.x.x.x, 172.16-31.x.x, 192.168.x.x
        }
        std::net::IpAddr::V6(_) => false,  // Only support IPv4 for simplicity
    }
}

/// Generates a random 6-digit PIN
fn generate_pin() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    format!("{:06}", rng.gen_range(0..999999))
}
```

### Shutdown

Three ways to shut down:
1. User clicks "Done receiving" → `shutdown_tx.send(())`
2. Inactivity timeout (5 minutes) → auto-shutdown
3. Profile lock → server stopped as part of session cleanup

---

## [5] QR Code Generation

```rust
use qrcode::QrCode;
use qrcode::render::svg;

/// Generates QR code as SVG
pub fn generate_qr_code(url: &str) -> Result<String, CohearaError> {
    let code = QrCode::new(url.as_bytes())
        .map_err(|e| CohearaError::Internal(format!("QR generation failed: {e}")))?;

    let svg_string = code.render::<svg::Color>()
        .min_dimensions(200, 200)
        .max_dimensions(300, 300)
        .dark_color(svg::Color("#1c1917"))  // stone-900
        .light_color(svg::Color("#ffffff"))
        .quiet_zone(true)
        .build();

    Ok(svg_string)
}
```

### Desktop Display

QR code is displayed as inline SVG in the Svelte component. Below it: the URL text (for manual entry) and the 6-digit PIN in large font.

---

## [6] Mobile Upload Page

### Served HTML

The server serves a self-contained, mobile-optimized HTML page. No external resources — everything inline (CSS, JS). Works on any modern phone browser.

```rust
/// Server state shared across handlers
#[derive(Clone)]
struct ServerState {
    pin: String,
    upload_count: Mutex<u32>,
    max_uploads: u32,
    max_file_size: u64,
    allowed_mime_types: Vec<String>,
    staging_dir: PathBuf,
    last_activity: Arc<Mutex<std::time::Instant>>,
}

/// Serves the mobile upload page
async fn serve_upload_page() -> Html<String> {
    Html(UPLOAD_PAGE_HTML.to_string())
}

const UPLOAD_PAGE_HTML: &str = r#"
<!DOCTYPE html>
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
    const pinInputs = document.querySelectorAll('.pin-input input');
    const btnPhoto = document.getElementById('btn-photo');
    const btnGallery = document.getElementById('btn-gallery');
    const btnFile = document.getElementById('btn-file');
    const fileInput = document.getElementById('file-input');
    const cameraInput = document.getElementById('camera-input');
    const statusEl = document.getElementById('status');
    const progressEl = document.getElementById('progress');
    const progressFill = document.getElementById('progress-fill');

    let pin = '';

    // PIN input handling
    pinInputs.forEach((input, i) => {
      input.addEventListener('input', (e) => {
        const val = e.target.value;
        if (val && i < 5) pinInputs[i + 1].focus();
        updatePin();
      });
      input.addEventListener('keydown', (e) => {
        if (e.key === 'Backspace' && !e.target.value && i > 0) {
          pinInputs[i - 1].focus();
        }
      });
    });

    function updatePin() {
      pin = Array.from(pinInputs).map(i => i.value).join('');
      const complete = pin.length === 6;
      btnPhoto.disabled = !complete;
      btnGallery.disabled = !complete;
      btnFile.disabled = !complete;
    }

    btnPhoto.addEventListener('click', () => cameraInput.click());
    btnGallery.addEventListener('click', () => { fileInput.removeAttribute('capture'); fileInput.click(); });
    btnFile.addEventListener('click', () => { fileInput.removeAttribute('capture'); fileInput.click(); });

    cameraInput.addEventListener('change', handleFile);
    fileInput.addEventListener('change', handleFile);

    async function handleFile(e) {
      const file = e.target.files[0];
      if (!file) return;

      // Size check
      if (file.size > 50 * 1024 * 1024) {
        showStatus('File too large. Maximum 50MB.', 'error');
        return;
      }

      const formData = new FormData();
      formData.append('file', file);
      formData.append('pin', pin);

      progressEl.style.display = 'block';
      progressFill.style.width = '0%';
      showStatus('Sending...', '');

      try {
        const xhr = new XMLHttpRequest();
        xhr.open('POST', '/upload');

        xhr.upload.onprogress = (e) => {
          if (e.lengthComputable) {
            progressFill.style.width = Math.round((e.loaded / e.total) * 100) + '%';
          }
        };

        xhr.onload = () => {
          progressEl.style.display = 'none';
          if (xhr.status === 200) {
            showStatus('Sent! You can send another or close this page.', 'success');
          } else {
            const resp = JSON.parse(xhr.responseText);
            showStatus(resp.error || 'Upload failed', 'error');
          }
        };

        xhr.onerror = () => {
          progressEl.style.display = 'none';
          showStatus('Connection failed. Make sure your phone and computer are on the same WiFi.', 'error');
        };

        xhr.send(formData);
      } catch (err) {
        progressEl.style.display = 'none';
        showStatus('Error: ' + err.message, 'error');
      }

      // Reset file input
      e.target.value = '';
    }

    function showStatus(text, type) {
      statusEl.textContent = text;
      statusEl.className = 'status ' + type;
    }

    // Focus first PIN input
    pinInputs[0].focus();
  </script>
</body>
</html>
"#;
```

---

## [7] File Reception & Validation

### Upload Handler

```rust
use axum::{
    extract::{Multipart, State as AxumState},
    http::StatusCode,
    Json,
};

#[derive(Serialize)]
struct UploadResponse {
    success: bool,
    message: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

/// Handles file upload from mobile device
async fn handle_upload(
    AxumState(state): AxumState<Arc<ServerState>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    // Update activity timestamp
    *state.last_activity.lock().await = std::time::Instant::now();

    // Check upload limit
    {
        let count = state.upload_count.lock().await;
        if *count >= state.max_uploads {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(ErrorResponse {
                    error: "Upload limit reached for this session.".into(),
                }),
            ).into_response();
        }
    }

    let mut pin_provided = String::new();
    let mut file_data: Option<(String, Vec<u8>, String)> = None;  // (filename, bytes, content_type)

    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "pin" => {
                pin_provided = field.text().await.unwrap_or_default();
            }
            "file" => {
                let filename = field.file_name()
                    .unwrap_or("document")
                    .to_string();
                let content_type = field.content_type()
                    .unwrap_or("application/octet-stream")
                    .to_string();
                let bytes = field.bytes().await.unwrap_or_default();
                file_data = Some((filename, bytes.to_vec(), content_type));
            }
            _ => {}
        }
    }

    // Validate PIN
    if pin_provided != state.pin {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Incorrect PIN. Check the number shown on your computer.".into(),
            }),
        ).into_response();
    }

    // Validate file
    let (filename, bytes, content_type) = match file_data {
        Some(data) => data,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse { error: "No file provided.".into() }),
            ).into_response();
        }
    };

    // Check file size
    if bytes.len() as u64 > state.max_file_size {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(ErrorResponse {
                error: format!("File too large. Maximum {}MB.",
                    state.max_file_size / (1024 * 1024)),
            }),
        ).into_response();
    }

    // Validate MIME type via magic bytes (not Content-Type header)
    let detected_mime = detect_mime_from_bytes(&bytes);
    if !state.allowed_mime_types.contains(&detected_mime) {
        return (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Json(ErrorResponse {
                error: "File type not supported. Please send an image or PDF.".into(),
            }),
        ).into_response();
    }

    // Sanitize filename
    let safe_filename = sanitize_filename(&filename);

    // Stage the file
    std::fs::create_dir_all(&state.staging_dir).ok();
    let staged_path = state.staging_dir
        .join(format!("{}_{}", Uuid::new_v4(), safe_filename));

    if let Err(e) = std::fs::write(&staged_path, &bytes) {
        tracing::error!("Failed to stage file: {e}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: "Failed to save file.".into() }),
        ).into_response();
    }

    // Increment upload count
    {
        let mut count = state.upload_count.lock().await;
        *count += 1;
    }

    tracing::info!(
        "File received: {} ({} bytes, {})",
        safe_filename, bytes.len(), detected_mime
    );

    (
        StatusCode::OK,
        Json(UploadResponse {
            success: true,
            message: format!("Document received! {}", safe_filename),
        }),
    ).into_response()
}

/// Detects MIME type from file magic bytes (not extension)
fn detect_mime_from_bytes(bytes: &[u8]) -> String {
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
    // PDF: 25 50 44 46 (%PDF)
    if bytes.starts_with(b"%PDF") {
        return "application/pdf".into();
    }
    // WebP: RIFF....WEBP
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return "image/webp".into();
    }
    // HEIF/HEIC: ....ftyp
    if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
        let brand = std::str::from_utf8(&bytes[8..12]).unwrap_or("");
        if brand.starts_with("heic") || brand.starts_with("heix") || brand.starts_with("mif1") {
            return "image/heic".into();
        }
    }

    "application/octet-stream".into()
}

/// Sanitizes a filename — removes path traversal, special chars
fn sanitize_filename(name: &str) -> String {
    let name = name
        .replace(['/', '\\', '..', '\0'], "")
        .replace(|c: char| !c.is_alphanumeric() && c != '.' && c != '-' && c != '_', "_");

    // Ensure reasonable length
    if name.len() > 100 {
        name[..100].to_string()
    } else if name.is_empty() {
        "document".into()
    } else {
        name
    }
}
```

---

## [8] Security

### Mandatory Security Measures

| Measure | Implementation | Rationale |
|---------|---------------|-----------|
| **Mandatory PIN** | 6-digit random PIN displayed on desktop, required in upload form | Prevents unauthorized uploads from devices on same network |
| **Local network only** | `is_local_network()` check on IP — rejects non-private IPs | No internet-routable access |
| **Random port** | Ephemeral port per session (49152-65535) | Not predictable |
| **Self-signed TLS** | `rcgen` generates cert per session; HTTPS enforced | Encrypts transfer on local network |
| **File type validation** | Magic bytes detection, not Content-Type header or extension | Prevents spoofed file types |
| **Max file size** | 50MB per file, enforced server-side | Prevents resource exhaustion |
| **Upload limit** | 20 files per session | Rate limiting |
| **Auto-timeout** | 5 minutes inactivity → server shuts down | Minimizes attack surface |
| **On-demand only** | Server only runs when user explicitly starts it | No persistent listener |
| **No CORS bypass** | Same-origin only (no Access-Control-Allow-Origin: *) | Prevents cross-site requests |
| **No persistent data on phone** | Phone uses browser — no local storage, no service workers | Nothing remains on phone |
| **Staging directory** | Files staged in encrypted profile directory | Files encrypted at rest |

### TLS Certificate Generation

```rust
use rcgen::{Certificate, CertificateParams, DnType, SanType};

fn generate_self_signed_cert(
    ip: &str,
) -> Result<(String, String), CohearaError> {
    let mut params = CertificateParams::default();
    params.distinguished_name.push(DnType::CommonName, "Coheara Transfer");
    params.subject_alt_names = vec![
        SanType::IpAddress(ip.parse().unwrap()),
    ];

    // Short validity — only needs to last the session
    params.not_before = rcgen::date_time_ymd(2025, 1, 1);
    params.not_after = rcgen::date_time_ymd(2027, 1, 1);

    let cert = Certificate::from_params(params)
        .map_err(|e| CohearaError::Internal(format!("Cert generation failed: {e}")))?;

    Ok((cert.serialize_pem()?, cert.serialize_private_key_pem()))
}
```

### PIN Brute Force Protection

After 5 incorrect PIN attempts from the same IP, block that IP for the remainder of the session:

```rust
// In ServerState
pub failed_attempts: Mutex<HashMap<std::net::IpAddr, u32>>,

// In handle_upload, before PIN check
let client_ip = /* extract from request */;
{
    let attempts = state.failed_attempts.lock().await;
    if attempts.get(&client_ip).copied().unwrap_or(0) >= 5 {
        return (StatusCode::FORBIDDEN, Json(ErrorResponse {
            error: "Too many incorrect PINs. Please restart the transfer on your computer.".into()
        })).into_response();
    }
}

// On PIN failure
{
    let mut attempts = state.failed_attempts.lock().await;
    *attempts.entry(client_ip).or_insert(0) += 1;
}
```

---

## [9] Tauri Commands (IPC)

```rust
// src-tauri/src/commands/transfer.rs

use tauri::State;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

/// Starts the WiFi transfer server
#[tauri::command]
pub async fn start_wifi_transfer(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<QrCodeData, String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;

    // Check if server already running
    if state.transfer_server.lock().await.is_some() {
        return Err("Transfer server already running".into());
    }

    let config = TransferConfig::default();
    let server = start_transfer_server(session, config).await
        .map_err(|e| format!("Failed to start server: {e}"))?;

    // Generate QR code
    let qr_svg = generate_qr_code(&server.session.url)
        .map_err(|e| format!("QR code error: {e}"))?;

    let qr_data = QrCodeData {
        url: server.session.url.clone(),
        pin: server.session.pin.clone(),
        svg: qr_svg,
    };

    // Store server handle
    *state.transfer_server.lock().await = Some(server);

    // Listen for uploads and emit events to frontend
    let app_clone = app.clone();
    tokio::spawn(async move {
        // Watch staging directory for new files
        // Emit "file-received" event to frontend
        // Each received file triggers: app_clone.emit("file-received", upload_info)
    });

    state.update_activity();
    Ok(qr_data)
}

/// Stops the WiFi transfer server
#[tauri::command]
pub async fn stop_wifi_transfer(
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut server_opt = state.transfer_server.lock().await;
    if let Some(server) = server_opt.take() {
        let _ = server.shutdown_tx.send(());
        tracing::info!("WiFi transfer stopped by user");
    }
    state.update_activity();
    Ok(())
}

/// Gets current transfer status
#[tauri::command]
pub async fn get_transfer_status(
    state: State<'_, AppState>,
) -> Result<Option<TransferSession>, String> {
    let server_opt = state.transfer_server.lock().await;
    Ok(server_opt.as_ref().map(|s| s.session.clone()))
}

/// Processes all staged files through the document pipeline
#[tauri::command]
pub async fn process_staged_files(
    state: State<'_, AppState>,
) -> Result<u32, String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;

    let staging_dir = session.profile_data_dir().join("staging");
    if !staging_dir.exists() {
        return Ok(0);
    }

    let mut count = 0;
    for entry in std::fs::read_dir(&staging_dir)
        .map_err(|e| format!("Failed to read staging dir: {e}"))? {
        let entry = entry.map_err(|e| format!("Dir entry error: {e}"))?;
        let path = entry.path();

        // Import through L1-01 document import pipeline
        import_document_from_path(session, &path)
            .map_err(|e| format!("Import error for {:?}: {e}", path))?;

        // Remove staged file after successful import
        std::fs::remove_file(&path).ok();
        count += 1;
    }

    state.update_activity();
    Ok(count)
}
```

### AppState Extension

```rust
// Add to AppState struct (L3-01)
pub transfer_server: tokio::sync::Mutex<Option<TransferServer>>,
```

### Frontend API

```typescript
// src/lib/api/transfer.ts
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { QrCodeData, TransferSession, UploadResult } from '$lib/types/transfer';

export async function startWifiTransfer(): Promise<QrCodeData> {
  return invoke<QrCodeData>('start_wifi_transfer');
}

export async function stopWifiTransfer(): Promise<void> {
  return invoke('stop_wifi_transfer');
}

export async function getTransferStatus(): Promise<TransferSession | null> {
  return invoke<TransferSession | null>('get_transfer_status');
}

export async function processStagedFiles(): Promise<number> {
  return invoke<number>('process_staged_files');
}

export function onFileReceived(callback: (result: UploadResult) => void) {
  return listen<UploadResult>('file-received', (event) => {
    callback(event.payload);
  });
}
```

---

## [10] Svelte Components

### Transfer Screen

```svelte
<!-- src/lib/components/transfer/TransferScreen.svelte -->
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    startWifiTransfer, stopWifiTransfer,
    processStagedFiles, onFileReceived
  } from '$lib/api/transfer';
  import type { QrCodeData, UploadResult } from '$lib/types/transfer';

  interface Props {
    onComplete: () => void;
    onCancel: () => void;
  }
  let { onComplete, onCancel }: Props = $props();

  type Status = 'starting' | 'active' | 'error' | 'stopping';

  let status: Status = $state('starting');
  let qrData: QrCodeData | null = $state(null);
  let receivedFiles: UploadResult[] = $state([]);
  let error: string | null = $state(null);
  let unlisten: (() => void) | null = $state(null);

  onMount(async () => {
    try {
      qrData = await startWifiTransfer();
      status = 'active';

      // Listen for received files
      const unlistenFn = await onFileReceived((result) => {
        receivedFiles = [...receivedFiles, result];
      });
      unlisten = unlistenFn;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      status = 'error';
    }
  });

  onDestroy(async () => {
    if (unlisten) unlisten();
    await stopWifiTransfer().catch(() => {});
  });

  async function handleDone() {
    status = 'stopping';
    try {
      await stopWifiTransfer();
      // Process all received files through the pipeline
      const count = await processStagedFiles();
      if (count > 0) {
        // Emit event for home screen refresh
      }
      onComplete();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      status = 'error';
    }
  }
</script>

<div class="flex flex-col items-center min-h-screen pb-20 bg-stone-50 px-6 py-8">
  {#if status === 'starting'}
    <div class="flex flex-col items-center justify-center flex-1">
      <div class="animate-spin w-8 h-8 border-2 border-[var(--color-primary)]
                  border-t-transparent rounded-full mb-4"></div>
      <p class="text-stone-500">Starting transfer server...</p>
    </div>

  {:else if status === 'error'}
    <div class="flex flex-col items-center justify-center flex-1">
      <p class="text-red-600 mb-4">{error}</p>
      <button
        class="px-6 py-3 bg-stone-200 rounded-xl text-stone-700 min-h-[44px]"
        onclick={onCancel}
      >
        Go back
      </button>
    </div>

  {:else if status === 'active' && qrData}
    <h2 class="text-xl font-semibold text-stone-800 mb-2">Receive from phone</h2>
    <p class="text-sm text-stone-500 mb-6 text-center">
      Scan this code with your phone camera to send documents.
    </p>

    <!-- QR Code -->
    <div class="bg-white p-6 rounded-2xl shadow-sm border border-stone-100 mb-6">
      {@html qrData.svg}
    </div>

    <!-- PIN display -->
    <div class="mb-4 text-center">
      <p class="text-xs text-stone-500 mb-1">Enter this PIN on your phone:</p>
      <p class="text-4xl font-mono font-bold tracking-[0.3em] text-stone-800">
        {qrData.pin}
      </p>
    </div>

    <!-- URL fallback -->
    <p class="text-xs text-stone-400 mb-8 text-center">
      Or type this in your phone's browser:<br>
      <span class="font-mono text-stone-500">{qrData.url}</span>
    </p>

    <!-- Received files -->
    {#if receivedFiles.length > 0}
      <div class="w-full max-w-sm mb-6">
        <h3 class="text-sm font-medium text-stone-600 mb-2">
          {receivedFiles.length} file{receivedFiles.length === 1 ? '' : 's'} received
        </h3>
        {#each receivedFiles as file}
          <div class="flex items-center gap-3 py-2 px-3 bg-green-50 rounded-lg mb-1">
            <span class="text-green-600">&#x2713;</span>
            <span class="text-sm text-stone-700 truncate">{file.filename}</span>
            <span class="text-xs text-stone-400 ml-auto">
              {(file.size_bytes / 1024).toFixed(0)}KB
            </span>
          </div>
        {/each}
      </div>
    {/if}

    <!-- Done button -->
    <button
      class="w-full max-w-sm px-6 py-4 bg-[var(--color-primary)] text-white rounded-xl
             text-base font-medium min-h-[44px]"
      onclick={handleDone}
    >
      Done receiving
    </button>
    <button
      class="mt-2 text-stone-500 text-sm min-h-[44px]"
      onclick={onCancel}
    >
      Cancel
    </button>

  {:else if status === 'stopping'}
    <div class="flex flex-col items-center justify-center flex-1">
      <div class="animate-pulse text-stone-400">Processing received files...</div>
    </div>
  {/if}
</div>
```

---

## [11] Error Handling

| Error | User Message | Recovery |
|-------|-------------|----------|
| No local network detected | "WiFi transfer requires your computer to be on a local network." | Ask user to check WiFi connection |
| Port bind fails | "Couldn't start the transfer server. Please try again." | Retry (new random port) |
| TLS cert generation fails | "Couldn't secure the connection. Please try again." | Retry |
| Upload too large | "File too large. Maximum 50MB." (shown on phone) | User selects smaller file |
| Wrong PIN | "Incorrect PIN. Check the number shown on your computer." (shown on phone) | User re-enters PIN |
| Upload limit reached | "Upload limit reached for this session." (shown on phone) | User clicks "Done" and starts new session |
| File type unsupported | "File type not supported. Please send an image or PDF." (shown on phone) | User selects correct file type |
| Too many failed PINs | "Too many incorrect PINs. Please restart the transfer." (shown on phone) | Restart transfer session |
| Server auto-shutdown | Server silently stops; desktop shows "Transfer session ended" | User restarts if needed |

---

## [12] Testing

### Unit Tests (Rust)

| Test | What |
|------|------|
| `test_generate_pin_format` | PIN is exactly 6 digits |
| `test_generate_pin_random` | Two PINs are different (probabilistic) |
| `test_is_local_network_private` | 192.168.1.1, 10.0.0.1, 172.16.0.1 all pass |
| `test_is_local_network_public` | 8.8.8.8, 1.1.1.1 fail |
| `test_detect_mime_jpeg` | JPEG magic bytes → "image/jpeg" |
| `test_detect_mime_png` | PNG magic bytes → "image/png" |
| `test_detect_mime_pdf` | %PDF magic bytes → "application/pdf" |
| `test_detect_mime_webp` | RIFF+WEBP → "image/webp" |
| `test_detect_mime_unknown` | Random bytes → "application/octet-stream" |
| `test_sanitize_filename_traversal` | "../../../etc/passwd" → "etcpasswd" |
| `test_sanitize_filename_special` | "my file (1).jpg" → "my_file__1_.jpg" |
| `test_sanitize_filename_long` | 200-char name → truncated to 100 |
| `test_sanitize_filename_empty` | "" → "document" |
| `test_qr_code_generation` | QR code SVG generated without error |
| `test_self_signed_cert` | Certificate and key PEM generated |
| `test_upload_page_served` | GET /upload returns HTML with 200 |
| `test_upload_correct_pin` | POST with correct PIN → 200 |
| `test_upload_wrong_pin` | POST with wrong PIN → 401 |
| `test_upload_too_large` | POST with >50MB → 413 |
| `test_upload_bad_type` | POST with .exe → 415 |
| `test_upload_limit` | 21st upload → 429 |
| `test_brute_force_block` | 6th wrong PIN from same IP → 403 |

### Integration Tests

| Test | What |
|------|------|
| `test_full_transfer_flow` | Start server → upload via HTTP client → file staged → stop server |
| `test_auto_timeout` | Server stops after configured inactivity |
| `test_staged_files_processed` | `process_staged_files` imports through L1-01 pipeline |

### Frontend Tests

| Test | What |
|------|------|
| `test_qr_code_displayed` | QR SVG rendered on screen |
| `test_pin_displayed` | PIN shown in large font |
| `test_file_received_count` | Count updates on file-received event |
| `test_done_button_stops_server` | "Done receiving" triggers stop |

---

## [13] Performance

- Server starts in < 2 seconds (cert generation is the bottleneck)
- QR code generation: < 100ms
- File reception: limited by WiFi speed (typically 10-50 Mbps local)
- Staging write: limited by disk I/O (typically < 500ms for 50MB)
- No CPU overhead when idle (async event loop)

---

## [14] Open Questions

- **Q1:** Should we support receiving multiple files in a single upload form submission? Current answer: one file at a time — simpler, clearer progress feedback.
- **Q2:** Should the mobile page support drag-and-drop on tablets? Current answer: no — camera/gallery/file buttons cover all use cases for mobile.
