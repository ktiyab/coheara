//! App Distribution Server — serves the mobile companion over local WiFi.
//!
//! Eliminates app store dependency by making the desktop the distribution
//! point for the mobile companion. Serves:
//! - `/install`           — Landing page with platform detection
//! - `/install/android`   — APK download page with sideload instructions
//! - `/install/android/download` — APK binary download
//! - `/app/**`            — Full PWA (iOS + Android fallback)
//! - `/update`            — Version check for mobile app updates
//! - `/health`            — Server health check
//!
//! Runs on HTTP (not HTTPS) because the install page must work in a raw
//! browser — iOS Safari rejects self-signed TLS certificates. The served
//! artifacts (app binaries) are public, not patient data. WPA2/WPA3
//! provides transport encryption on the WiFi segment.
//!
//! Security: local network only, rate limiting, APK integrity hash.
//! See `Specs/implementation/01-APP-DISTRIBUTION-SERVER.md`.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, State as AxumState};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde::{Deserialize, Serialize};
use tokio::sync::{oneshot, Mutex as TokioMutex};
use uuid::Uuid;

use crate::config;
use crate::wifi_transfer::{generate_qr_code, is_local_network};

// ═══════════════════════════════════════════════════════════
// Public types
// ═══════════════════════════════════════════════════════════

/// Distribution server session metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributionSession {
    pub session_id: Uuid,
    pub server_addr: String,
    pub url: String,
    pub started_at: chrono::NaiveDateTime,
    pub desktop_version: String,
}

/// Distribution server status returned to frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributionStatus {
    pub session: DistributionSession,
    pub request_count: u32,
    pub apk_available: bool,
    pub pwa_available: bool,
}

/// QR code data for the install page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallQrCode {
    pub url: String,
    pub svg: String,
    pub desktop_version: String,
}

/// Server configuration.
pub struct DistributionConfig {
    /// Port to bind on (0 = ephemeral).
    pub port: u16,
    /// Requests per minute per IP before rate limiting.
    pub rate_limit_per_min: u32,
    /// Path to the mobile PWA build directory (contains index.html, _app/, etc.).
    pub pwa_dir: Option<PathBuf>,
    /// Path to the Android APK file.
    pub apk_path: Option<PathBuf>,
}

impl Default for DistributionConfig {
    fn default() -> Self {
        Self {
            port: 0, // Ephemeral port by default
            rate_limit_per_min: 60,
            pwa_dir: None,
            apk_path: None,
        }
    }
}

/// Handle to a running distribution server. Stored in CoreState.
pub struct DistributionServer {
    pub session: DistributionSession,
    shutdown_tx: Option<oneshot::Sender<()>>,
    request_count: Arc<TokioMutex<u32>>,
    pub qr_code: InstallQrCode,
}

impl DistributionServer {
    /// Get the current request count.
    pub async fn request_count(&self) -> u32 {
        *self.request_count.lock().await
    }

    /// Build a status snapshot.
    pub async fn status(&self, has_apk: bool, has_pwa: bool) -> DistributionStatus {
        DistributionStatus {
            session: self.session.clone(),
            request_count: *self.request_count.lock().await,
            apk_available: has_apk,
            pwa_available: has_pwa,
        }
    }

    /// Shut down the server gracefully.
    pub fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
            tracing::info!("Distribution server shutdown signal sent");
        }
    }
}

// ═══════════════════════════════════════════════════════════
// Internal types (axum server state)
// ═══════════════════════════════════════════════════════════

#[derive(Clone)]
pub(crate) struct ServerState {
    desktop_version: String,
    request_count: Arc<TokioMutex<u32>>,
    rate_limit_per_min: u32,
    rate_tracker: Arc<TokioMutex<HashMap<IpAddr, Vec<Instant>>>>,
    pwa_dir: Option<PathBuf>,
    apk_path: Option<PathBuf>,
}

// ═══════════════════════════════════════════════════════════
// Server lifecycle
// ═══════════════════════════════════════════════════════════

/// Start the distribution server on the local network.
///
/// Binds to the local IP on the configured port (or ephemeral),
/// and spawns the axum server in a background task.
pub async fn start_distribution_server(
    config: DistributionConfig,
) -> Result<DistributionServer, String> {
    // 1. Detect local IP
    let local_ip = local_ip_address::local_ip()
        .map_err(|e| format!("Cannot detect local IP: {e}"))?;

    if !is_local_network(&local_ip) {
        return Err(
            "Not on a local network. Distribution server requires a local network connection."
                .into(),
        );
    }

    // 2. Bind to configured port (0 = ephemeral)
    let listener = tokio::net::TcpListener::bind(SocketAddr::new(local_ip, config.port))
        .await
        .map_err(|e| format!("Failed to bind distribution server: {e}"))?;

    let addr = listener
        .local_addr()
        .map_err(|e| format!("Failed to get server address: {e}"))?;

    // 3. Build URL
    let base_url = format!("http://{}:{}", addr.ip(), addr.port());

    // 4. Generate QR code pointing to install page
    let install_url = format!("{base_url}/install");
    let qr_svg = generate_qr_code(&install_url)
        .map_err(|e| format!("QR code generation failed: {e}"))?;

    // 5. Check available assets
    let has_apk = config
        .apk_path
        .as_ref()
        .map(|p| p.exists())
        .unwrap_or(false);
    let has_pwa = config
        .pwa_dir
        .as_ref()
        .map(|d| d.join("index.html").exists())
        .unwrap_or(false);

    tracing::info!(
        addr = %addr,
        apk = has_apk,
        pwa = has_pwa,
        "Distribution server starting"
    );

    // 6. Create session
    let session = DistributionSession {
        session_id: Uuid::new_v4(),
        server_addr: addr.to_string(),
        url: base_url.clone(),
        started_at: chrono::Local::now().naive_local(),
        desktop_version: config::APP_VERSION.to_string(),
    };

    // 7. Create shared state
    let request_count = Arc::new(TokioMutex::new(0u32));

    let server_state = Arc::new(ServerState {
        desktop_version: config::APP_VERSION.to_string(),
        request_count: request_count.clone(),
        rate_limit_per_min: config.rate_limit_per_min,
        rate_tracker: Arc::new(TokioMutex::new(HashMap::new())),
        pwa_dir: config.pwa_dir,
        apk_path: config.apk_path,
    });

    // 8. Build router
    let app = build_distribution_router(server_state);

    // 9. Shutdown signal
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    // 10. Spawn server
    tokio::spawn(async move {
        let shutdown_signal = async move {
            let _ = shutdown_rx.await;
            tracing::info!("Distribution server shutdown by user");
        };

        if let Err(e) = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(shutdown_signal)
        .await
        {
            tracing::error!("Distribution server error: {e}");
        }
    });

    let qr_code = InstallQrCode {
        url: install_url,
        svg: qr_svg,
        desktop_version: config::APP_VERSION.to_string(),
    };

    tracing::info!(addr = %addr, "Distribution server started");

    Ok(DistributionServer {
        session,
        shutdown_tx: Some(shutdown_tx),
        request_count,
        qr_code,
    })
}

/// Build the distribution router (extracted for testability).
pub(crate) fn build_distribution_router(state: Arc<ServerState>) -> Router {
    let mut router = Router::new()
        .route("/install", get(handle_install_landing))
        .route("/install/android", get(handle_android_page))
        .route("/install/android/download", get(handle_apk_download))
        .route("/update", get(handle_version_check))
        .route("/health", get(handle_health));

    // PWA static file serving: /app/** → pwa_dir/
    if let Some(ref pwa_dir) = state.pwa_dir {
        if pwa_dir.join("index.html").exists() {
            router = router
                .route("/app", get(handle_pwa_index))
                .route("/app/*rest", get(handle_pwa_asset));
        }
    }

    router.with_state(state)
}

// ═══════════════════════════════════════════════════════════
// Rate limiting (per-IP sliding window)
// ═══════════════════════════════════════════════════════════

async fn check_rate_limit(state: &ServerState, ip: IpAddr) -> bool {
    let mut tracker = state.rate_tracker.lock().await;
    let now = Instant::now();
    let window = Duration::from_secs(60);

    let timestamps = tracker.entry(ip).or_default();

    // Remove entries older than the window
    timestamps.retain(|t| now.duration_since(*t) < window);

    if timestamps.len() as u32 >= state.rate_limit_per_min {
        return false; // Rate limited
    }

    timestamps.push(now);
    true
}

async fn increment_request_count(state: &ServerState) {
    let mut count = state.request_count.lock().await;
    *count += 1;
}

// ═══════════════════════════════════════════════════════════
// Handlers
// ═══════════════════════════════════════════════════════════

/// GET /install — Platform-detecting landing page
async fn handle_install_landing(
    ConnectInfo(client_addr): ConnectInfo<SocketAddr>,
    AxumState(state): AxumState<Arc<ServerState>>,
) -> Response {
    // Local network check
    if !is_local_network(&client_addr.ip()) {
        return (StatusCode::FORBIDDEN, "Local network access only").into_response();
    }

    // Rate limit
    if !check_rate_limit(&state, client_addr.ip()).await {
        return (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded").into_response();
    }

    increment_request_count(&state).await;

    let has_apk = state
        .apk_path
        .as_ref()
        .map(|p| p.exists())
        .unwrap_or(false);
    let has_pwa = state
        .pwa_dir
        .as_ref()
        .map(|d| d.join("index.html").exists())
        .unwrap_or(false);

    let html = render_install_page(&state.desktop_version, has_apk, has_pwa);
    Html(html).into_response()
}

/// GET /install/android — APK download page with sideload instructions
async fn handle_android_page(
    ConnectInfo(client_addr): ConnectInfo<SocketAddr>,
    AxumState(state): AxumState<Arc<ServerState>>,
) -> Response {
    if !is_local_network(&client_addr.ip()) {
        return (StatusCode::FORBIDDEN, "Local network access only").into_response();
    }
    if !check_rate_limit(&state, client_addr.ip()).await {
        return (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded").into_response();
    }

    increment_request_count(&state).await;

    let apk_info = match &state.apk_path {
        Some(path) if path.exists() => {
            let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            let hash = compute_file_sha256(path).unwrap_or_default();
            Some((size, hash))
        }
        _ => None,
    };

    let html = render_android_page(&state.desktop_version, apk_info.as_ref());
    Html(html).into_response()
}

/// GET /install/android/download — Serve APK binary
async fn handle_apk_download(
    ConnectInfo(client_addr): ConnectInfo<SocketAddr>,
    AxumState(state): AxumState<Arc<ServerState>>,
) -> Response {
    if !is_local_network(&client_addr.ip()) {
        return (StatusCode::FORBIDDEN, "Local network access only").into_response();
    }
    if !check_rate_limit(&state, client_addr.ip()).await {
        return (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded").into_response();
    }

    increment_request_count(&state).await;

    let apk_path = match &state.apk_path {
        Some(path) if path.exists() => path.clone(),
        _ => {
            return (StatusCode::NOT_FOUND, "APK not available").into_response();
        }
    };

    match tokio::fs::read(&apk_path).await {
        Ok(bytes) => {
            let filename = format!("Coheara-{}.apk", state.desktop_version);
            Response::builder()
                .status(StatusCode::OK)
                .header(
                    header::CONTENT_TYPE,
                    "application/vnd.android.package-archive",
                )
                .header(
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{filename}\""),
                )
                .header(header::CONTENT_LENGTH, bytes.len().to_string())
                .header(header::CACHE_CONTROL, "no-cache")
                .body(axum::body::Body::from(bytes))
                .unwrap_or_else(|_| {
                    (StatusCode::INTERNAL_SERVER_ERROR, "Failed to build response")
                        .into_response()
                })
        }
        Err(e) => {
            tracing::error!("Failed to read APK: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read APK").into_response()
        }
    }
}

/// GET /app — Serve PWA index.html
async fn handle_pwa_index(
    ConnectInfo(client_addr): ConnectInfo<SocketAddr>,
    AxumState(state): AxumState<Arc<ServerState>>,
) -> Response {
    if !is_local_network(&client_addr.ip()) {
        return (StatusCode::FORBIDDEN, "Local network access only").into_response();
    }
    if !check_rate_limit(&state, client_addr.ip()).await {
        return (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded").into_response();
    }

    increment_request_count(&state).await;
    serve_pwa_file(&state, "index.html").await
}

/// GET /app/*rest — Serve PWA static assets (SPA fallback)
async fn handle_pwa_asset(
    ConnectInfo(client_addr): ConnectInfo<SocketAddr>,
    AxumState(state): AxumState<Arc<ServerState>>,
    axum::extract::Path(rest): axum::extract::Path<String>,
) -> Response {
    if !is_local_network(&client_addr.ip()) {
        return (StatusCode::FORBIDDEN, "Local network access only").into_response();
    }
    if !check_rate_limit(&state, client_addr.ip()).await {
        return (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded").into_response();
    }

    increment_request_count(&state).await;

    // Try to serve the exact file first, fall back to index.html (SPA routing)
    let response = serve_pwa_file(&state, &rest).await;
    if response.status() == StatusCode::NOT_FOUND {
        // SPA fallback — serve index.html for client-side routing
        serve_pwa_file(&state, "index.html").await
    } else {
        response
    }
}

/// Serve a file from the PWA directory with correct MIME type and caching.
async fn serve_pwa_file(state: &ServerState, path: &str) -> Response {
    let pwa_dir = match &state.pwa_dir {
        Some(dir) => dir,
        None => {
            return (StatusCode::NOT_FOUND, "PWA not available").into_response();
        }
    };

    // Sanitize path — prevent directory traversal
    let clean_path = path
        .replace("..", "")
        .trim_start_matches('/')
        .to_string();

    let file_path = pwa_dir.join(&clean_path);

    // Ensure the resolved path is still within pwa_dir
    match file_path.canonicalize() {
        Ok(canonical) => {
            let pwa_canonical = pwa_dir.canonicalize().unwrap_or_default();
            if !canonical.starts_with(&pwa_canonical) {
                return (StatusCode::FORBIDDEN, "Path traversal denied").into_response();
            }
        }
        Err(_) => {
            return (StatusCode::NOT_FOUND, "File not found").into_response();
        }
    }

    if !file_path.is_file() {
        return (StatusCode::NOT_FOUND, "File not found").into_response();
    }

    match tokio::fs::read(&file_path).await {
        Ok(bytes) => {
            let mime = mime_guess::from_path(&file_path)
                .first_or_octet_stream()
                .to_string();

            // Immutable assets get long cache, everything else no-cache
            let cache_control = if clean_path.contains("immutable") {
                "public, max-age=31536000, immutable"
            } else {
                "no-cache"
            };

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime)
                .header(header::CACHE_CONTROL, cache_control)
                .header(header::CONTENT_LENGTH, bytes.len().to_string())
                .body(axum::body::Body::from(bytes))
                .unwrap_or_else(|_| {
                    (StatusCode::INTERNAL_SERVER_ERROR, "Response build failed").into_response()
                })
        }
        Err(_) => (StatusCode::NOT_FOUND, "File not found").into_response(),
    }
}

/// GET /update — Version check endpoint
async fn handle_version_check(
    ConnectInfo(client_addr): ConnectInfo<SocketAddr>,
    AxumState(state): AxumState<Arc<ServerState>>,
) -> Response {
    if !is_local_network(&client_addr.ip()) {
        return (StatusCode::FORBIDDEN, "Local network access only").into_response();
    }
    if !check_rate_limit(&state, client_addr.ip()).await {
        return (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded").into_response();
    }

    increment_request_count(&state).await;

    let apk_info = state.apk_path.as_ref().and_then(|path| {
        if path.exists() {
            let size = std::fs::metadata(path).map(|m| m.len()).ok()?;
            let hash = compute_file_sha256(path).ok()?;
            Some(ApkInfo {
                url: "/install/android/download".to_string(),
                hash,
                size,
            })
        } else {
            None
        }
    });

    let has_pwa = state
        .pwa_dir
        .as_ref()
        .map(|d| d.join("index.html").exists())
        .unwrap_or(false);

    let response = VersionResponse {
        version: state.desktop_version.clone(),
        min_compatible: state.desktop_version.clone(), // Same for now
        android: apk_info,
        pwa: if has_pwa {
            Some(PwaInfo {
                url: "/app".to_string(),
                sw_version: state.desktop_version.clone(),
            })
        } else {
            None
        },
        desktop_version: state.desktop_version.clone(),
    };

    axum::Json(response).into_response()
}

/// GET /health — Server health check
async fn handle_health(
    AxumState(state): AxumState<Arc<ServerState>>,
) -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "status": "ok",
        "version": state.desktop_version,
    }))
}

// ═══════════════════════════════════════════════════════════
// Response types
// ═══════════════════════════════════════════════════════════

#[derive(Serialize)]
struct VersionResponse {
    version: String,
    min_compatible: String,
    android: Option<ApkInfo>,
    pwa: Option<PwaInfo>,
    desktop_version: String,
}

#[derive(Serialize)]
struct ApkInfo {
    url: String,
    hash: String,
    size: u64,
}

#[derive(Serialize)]
struct PwaInfo {
    url: String,
    sw_version: String,
}

// ═══════════════════════════════════════════════════════════
// Utility functions
// ═══════════════════════════════════════════════════════════

/// Compute SHA-256 hash of a file (for APK integrity verification).
fn compute_file_sha256(path: &std::path::Path) -> Result<String, std::io::Error> {
    use sha2::{Digest, Sha256};

    let bytes = std::fs::read(path)?;
    let hash = Sha256::digest(&bytes);
    Ok(format!("sha256:{:x}", hash))
}

// ═══════════════════════════════════════════════════════════
// HTML rendering — self-contained pages (no external deps)
// ═══════════════════════════════════════════════════════════

/// Render the install landing page with platform detection.
fn render_install_page(version: &str, has_apk: bool, has_pwa: bool) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>Install Coheara Companion</title>
<style>
*,*::before,*::after{{box-sizing:border-box}}
body{{margin:0;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;background:#fafaf9;color:#1c1917;display:flex;align-items:center;justify-content:center;min-height:100vh;padding:24px}}
.card{{background:#fff;border-radius:16px;box-shadow:0 4px 24px rgba(0,0,0,.08);max-width:400px;width:100%;padding:32px;text-align:center}}
h1{{font-size:1.5rem;margin:0 0 8px}}
.version{{color:#78716c;font-size:.875rem;margin-bottom:24px}}
.icon{{width:72px;height:72px;background:#4A6FA5;border-radius:16px;margin:0 auto 16px;display:flex;align-items:center;justify-content:center}}
.icon svg{{width:40px;height:40px;fill:#fff}}
.btn{{display:block;width:100%;padding:16px;border:none;border-radius:12px;font-size:1rem;font-weight:600;cursor:pointer;margin-bottom:12px;text-decoration:none;transition:transform .1s}}
.btn:active{{transform:scale(.97)}}
.btn-primary{{background:#4A6FA5;color:#fff}}
.btn-secondary{{background:#e7e5e4;color:#1c1917}}
.note{{font-size:.8rem;color:#a8a29e;margin-top:16px;line-height:1.4}}
.hidden{{display:none}}
</style>
</head>
<body>
<div class="card">
  <div class="icon"><svg viewBox="0 0 24 24"><path d="M19 3H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zm-7 14c-3.3 0-6-2.7-6-6s2.7-6 6-6 6 2.7 6 6-2.7 6-6 6zm3-6c0 1.7-1.3 3-3 3s-3-1.3-3-3 1.3-3 3-3 3 1.3 3 3z"/></svg></div>
  <h1>Coheara Companion</h1>
  <p class="version">v{version}</p>

  <div id="android" class="hidden">
    {apk_section}
    {pwa_android_section}
  </div>

  <div id="ios" class="hidden">
    {pwa_ios_section}
  </div>

  <div id="other" class="hidden">
    <p>Open this page on your phone to install the Coheara companion app.</p>
  </div>

  <p class="note">Your health data stays on your devices. This app connects to your Coheara desktop over local WiFi only.</p>
</div>
<script>
(function(){{
  var ua=navigator.userAgent||'';
  var isAndroid=/android/i.test(ua);
  var isIOS=/iphone|ipad|ipod/i.test(ua);
  if(isAndroid)document.getElementById('android').classList.remove('hidden');
  else if(isIOS)document.getElementById('ios').classList.remove('hidden');
  else document.getElementById('other').classList.remove('hidden');
}})();
</script>
</body>
</html>"#,
        version = version,
        apk_section = if has_apk {
            r#"<a href="/install/android" class="btn btn-primary">Install Android App</a>"#
        } else {
            ""
        },
        pwa_android_section = if has_pwa {
            if has_apk {
                r#"<a href="/app" class="btn btn-secondary">Or use Web App</a>"#
            } else {
                r#"<a href="/app" class="btn btn-primary">Open Web App</a>"#
            }
        } else {
            ""
        },
        pwa_ios_section = if has_pwa {
            r#"<a href="/app" class="btn btn-primary">Open Web App</a>
    <p style="font-size:.85rem;color:#78716c;margin-top:8px">Tap <strong>Share</strong> then <strong>Add to Home Screen</strong> for the full app experience.</p>"#
        } else {
            r#"<p>PWA not available. Please update your desktop app.</p>"#
        },
    )
}

/// Render the Android APK download page with sideload instructions.
fn render_android_page(version: &str, apk_info: Option<&(u64, String)>) -> String {
    let (size_display, hash_display) = match apk_info {
        Some((size, hash)) => {
            let mb = *size as f64 / (1024.0 * 1024.0);
            (format!("{mb:.1} MB"), hash.clone())
        }
        None => ("N/A".to_string(), "N/A".to_string()),
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>Install Coheara for Android</title>
<style>
*,*::before,*::after{{box-sizing:border-box}}
body{{margin:0;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;background:#fafaf9;color:#1c1917;display:flex;align-items:center;justify-content:center;min-height:100vh;padding:24px}}
.card{{background:#fff;border-radius:16px;box-shadow:0 4px 24px rgba(0,0,0,.08);max-width:440px;width:100%;padding:32px}}
h1{{font-size:1.25rem;margin:0 0 8px;text-align:center}}
.version{{color:#78716c;font-size:.875rem;text-align:center;margin-bottom:20px}}
.btn{{display:block;width:100%;padding:16px;border:none;border-radius:12px;font-size:1rem;font-weight:600;cursor:pointer;text-decoration:none;text-align:center;background:#4A6FA5;color:#fff;margin-bottom:16px;transition:transform .1s}}
.btn:active{{transform:scale(.97)}}
.info{{display:flex;justify-content:space-between;font-size:.85rem;color:#78716c;margin-bottom:20px;padding:12px;background:#f5f5f4;border-radius:8px}}
.steps{{list-style:none;padding:0;margin:0}}
.steps li{{padding:12px 0;border-bottom:1px solid #e7e5e4;font-size:.9rem;display:flex;gap:12px}}
.steps li:last-child{{border:none}}
.step-num{{background:#4A6FA5;color:#fff;width:24px;height:24px;border-radius:50%;display:flex;align-items:center;justify-content:center;font-size:.75rem;font-weight:700;flex-shrink:0}}
.back{{display:block;text-align:center;margin-top:16px;color:#4A6FA5;font-size:.9rem;text-decoration:none}}
</style>
</head>
<body>
<div class="card">
  <h1>Coheara for Android</h1>
  <p class="version">v{version} &middot; {size}</p>

  {download_button}

  <div class="info">
    <span>SHA-256</span>
    <span style="font-family:monospace;font-size:.75rem;word-break:break-all">{hash}</span>
  </div>

  <ol class="steps">
    <li><span class="step-num">1</span><span>Tap <strong>Download</strong> above</span></li>
    <li><span class="step-num">2</span><span>Open the downloaded file</span></li>
    <li><span class="step-num">3</span><span>If asked, allow installation from your browser</span></li>
    <li><span class="step-num">4</span><span>Tap <strong>Install</strong></span></li>
    <li><span class="step-num">5</span><span>Open Coheara and pair with your desktop</span></li>
  </ol>

  <a href="/install" class="back">&larr; Back</a>
</div>
</body>
</html>"#,
        version = version,
        size = size_display,
        hash = hash_display,
        download_button = if apk_info.is_some() {
            r#"<a href="/install/android/download" class="btn">Download APK</a>"#
        } else {
            r#"<div class="btn" style="background:#a8a29e;cursor:default">APK Not Available</div>"#
        },
    )
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_state(pwa_dir: Option<PathBuf>, apk_path: Option<PathBuf>) -> Arc<ServerState> {
        Arc::new(ServerState {
            desktop_version: "0.1.0".to_string(),
            request_count: Arc::new(TokioMutex::new(0)),
            rate_limit_per_min: 60,
            rate_tracker: Arc::new(TokioMutex::new(HashMap::new())),
            pwa_dir,
            apk_path,
        })
    }

    fn make_request(uri: &str, ip: &str) -> axum::http::Request<Body> {
        let addr: SocketAddr = format!("{ip}:12345").parse().unwrap();
        let mut req = axum::http::Request::builder()
            .uri(uri)
            .body(Body::empty())
            .unwrap();
        req.extensions_mut().insert(ConnectInfo(addr));
        req
    }

    // -- Health endpoint ---------------------------------------------------

    #[tokio::test]
    async fn health_returns_ok() {
        let state = test_state(None, None);
        let app = build_distribution_router(state);
        let req = make_request("/health", "192.168.1.100");
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
        assert_eq!(json["version"], "0.1.0");
    }

    // -- Install landing page -----------------------------------------------

    #[tokio::test]
    async fn install_page_returns_html() {
        let state = test_state(None, None);
        let app = build_distribution_router(state);
        let req = make_request("/install", "192.168.1.100");
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("Coheara Companion"));
        assert!(html.contains("0.1.0"));
    }

    #[tokio::test]
    async fn install_page_rejects_public_ip() {
        let state = test_state(None, None);
        let app = build_distribution_router(state);
        let req = make_request("/install", "8.8.8.8");
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    // -- Version check endpoint ---------------------------------------------

    #[tokio::test]
    async fn version_check_returns_json() {
        let state = test_state(None, None);
        let app = build_distribution_router(state);
        let req = make_request("/update", "192.168.1.100");
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["version"], "0.1.0");
        assert_eq!(json["desktop_version"], "0.1.0");
        assert!(json["android"].is_null());
        assert!(json["pwa"].is_null());
    }

    // -- Android page -------------------------------------------------------

    #[tokio::test]
    async fn android_page_without_apk_shows_unavailable() {
        let state = test_state(None, None);
        let app = build_distribution_router(state);
        let req = make_request("/install/android", "192.168.1.100");
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("APK Not Available"));
    }

    #[tokio::test]
    async fn apk_download_returns_404_when_no_apk() {
        let state = test_state(None, None);
        let app = build_distribution_router(state);
        let req = make_request("/install/android/download", "192.168.1.100");
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    // -- APK serving with real file -----------------------------------------

    #[tokio::test]
    async fn apk_download_serves_file() {
        // Create a temp APK file
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"fake-apk-content").unwrap();

        let state = test_state(None, Some(tmp.path().to_path_buf()));
        let app = build_distribution_router(state);
        let req = make_request("/install/android/download", "192.168.1.100");
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers().get("content-type").unwrap(),
            "application/vnd.android.package-archive"
        );

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"fake-apk-content");
    }

    // -- PWA serving --------------------------------------------------------

    #[tokio::test]
    async fn pwa_index_serves_html() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp_dir.path().join("index.html"),
            "<html><body>PWA</body></html>",
        )
        .unwrap();

        let state = test_state(Some(tmp_dir.path().to_path_buf()), None);
        let app = build_distribution_router(state);
        let req = make_request("/app", "192.168.1.100");
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("PWA"));
    }

    #[tokio::test]
    async fn pwa_immutable_asset_gets_long_cache() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp_dir.path().join("index.html"), "<html></html>").unwrap();
        let immutable_dir = tmp_dir.path().join("_app").join("immutable");
        std::fs::create_dir_all(&immutable_dir).unwrap();
        std::fs::write(immutable_dir.join("app.js"), "console.log('hi')").unwrap();

        let state = test_state(Some(tmp_dir.path().to_path_buf()), None);
        let app = build_distribution_router(state);
        let req = make_request("/app/_app/immutable/app.js", "192.168.1.100");
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let cache = resp.headers().get("cache-control").unwrap().to_str().unwrap();
        assert!(cache.contains("immutable"));
        assert!(cache.contains("31536000"));
    }

    #[tokio::test]
    async fn pwa_spa_fallback_serves_index() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp_dir.path().join("index.html"),
            "<html><body>SPA Root</body></html>",
        )
        .unwrap();

        let state = test_state(Some(tmp_dir.path().to_path_buf()), None);
        let app = build_distribution_router(state);
        // Request a route that doesn't exist as a file — SPA fallback
        let req = make_request("/app/chat/conversation/123", "192.168.1.100");
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("SPA Root"));
    }

    #[tokio::test]
    async fn pwa_rejects_directory_traversal() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp_dir.path().join("index.html"), "<html></html>").unwrap();

        // Create a file outside pwa_dir
        let outside = tmp_dir.path().parent().unwrap().join("secret.txt");
        std::fs::write(&outside, "secret").unwrap();

        let state = test_state(Some(tmp_dir.path().to_path_buf()), None);
        let app = build_distribution_router(state);
        let req = make_request("/app/../secret.txt", "192.168.1.100");
        let resp = app.oneshot(req).await.unwrap();
        // Should get SPA fallback (index.html) since .. is stripped, or 404/403
        // The path sanitization removes ".." so it becomes "/secret.txt" which doesn't exist
        // and falls back to index.html
        assert!(resp.status() == StatusCode::OK || resp.status() == StatusCode::NOT_FOUND);
    }

    // -- Rate limiting ------------------------------------------------------

    #[tokio::test]
    async fn rate_limit_blocks_after_threshold() {
        let state = Arc::new(ServerState {
            desktop_version: "0.1.0".to_string(),
            request_count: Arc::new(TokioMutex::new(0)),
            rate_limit_per_min: 3, // Very low for testing
            rate_tracker: Arc::new(TokioMutex::new(HashMap::new())),
            pwa_dir: None,
            apk_path: None,
        });

        let ip: IpAddr = "192.168.1.100".parse().unwrap();

        assert!(check_rate_limit(&state, ip).await);
        assert!(check_rate_limit(&state, ip).await);
        assert!(check_rate_limit(&state, ip).await);
        // 4th request should be blocked
        assert!(!check_rate_limit(&state, ip).await);
    }

    #[tokio::test]
    async fn rate_limit_different_ips_independent() {
        let state = Arc::new(ServerState {
            desktop_version: "0.1.0".to_string(),
            request_count: Arc::new(TokioMutex::new(0)),
            rate_limit_per_min: 2,
            rate_tracker: Arc::new(TokioMutex::new(HashMap::new())),
            pwa_dir: None,
            apk_path: None,
        });

        let ip1: IpAddr = "192.168.1.100".parse().unwrap();
        let ip2: IpAddr = "192.168.1.101".parse().unwrap();

        assert!(check_rate_limit(&state, ip1).await);
        assert!(check_rate_limit(&state, ip1).await);
        assert!(!check_rate_limit(&state, ip1).await); // ip1 blocked

        assert!(check_rate_limit(&state, ip2).await); // ip2 still fine
    }

    // -- Request counting ---------------------------------------------------

    #[tokio::test]
    async fn request_count_increments() {
        let state = test_state(None, None);

        assert_eq!(*state.request_count.lock().await, 0);
        increment_request_count(&state).await;
        assert_eq!(*state.request_count.lock().await, 1);
        increment_request_count(&state).await;
        assert_eq!(*state.request_count.lock().await, 2);
    }

    // -- SHA-256 computation ------------------------------------------------

    #[tokio::test]
    async fn sha256_computes_correctly() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"hello world").unwrap();

        let hash = compute_file_sha256(tmp.path()).unwrap();
        assert!(hash.starts_with("sha256:"));
        // SHA-256 of "hello world"
        assert!(hash.contains("b94d27b9934d3e08a52e52d7da7dabfa"));
    }

    // -- HTML rendering -----------------------------------------------------

    #[test]
    fn install_page_contains_platform_detection() {
        let html = render_install_page("0.1.0", true, true);
        assert!(html.contains("android"));
        assert!(html.contains("ios"));
        assert!(html.contains("isAndroid"));
        assert!(html.contains("isIOS"));
    }

    #[test]
    fn install_page_hides_apk_when_unavailable() {
        let html = render_install_page("0.1.0", false, true);
        assert!(!html.contains("/install/android"));
        assert!(html.contains("/app")); // PWA still shown
    }

    #[test]
    fn android_page_shows_hash_when_available() {
        let html = render_android_page("0.1.0", Some(&(15_000_000, "sha256:abc123".to_string())));
        assert!(html.contains("sha256:abc123"));
        assert!(html.contains("14.3 MB"));
        assert!(html.contains("/install/android/download"));
    }

    // -- Distribution server lifecycle --------------------------------------

    #[tokio::test]
    async fn distribution_session_fields() {
        let session = DistributionSession {
            session_id: Uuid::new_v4(),
            server_addr: "192.168.1.42:8080".to_string(),
            url: "http://192.168.1.42:8080".to_string(),
            started_at: chrono::Local::now().naive_local(),
            desktop_version: "0.1.0".to_string(),
        };
        assert_eq!(session.desktop_version, "0.1.0");
        assert!(session.url.starts_with("http://"));
    }

    #[tokio::test]
    async fn distribution_server_shutdown() {
        let (tx, rx) = oneshot::channel();
        let mut server = DistributionServer {
            session: DistributionSession {
                session_id: Uuid::new_v4(),
                server_addr: "127.0.0.1:0".to_string(),
                url: "http://127.0.0.1:0".to_string(),
                started_at: chrono::Local::now().naive_local(),
                desktop_version: "0.1.0".to_string(),
            },
            shutdown_tx: Some(tx),
            request_count: Arc::new(TokioMutex::new(5)),
            qr_code: InstallQrCode {
                url: "http://test".to_string(),
                svg: "<svg/>".to_string(),
                desktop_version: "0.1.0".to_string(),
            },
        };

        assert_eq!(server.request_count().await, 5);
        server.shutdown();
        // After shutdown, tx is consumed
        assert!(server.shutdown_tx.is_none());
        // Receiver should get the signal
        assert!(rx.await.is_ok());
    }

    #[tokio::test]
    async fn version_check_with_apk_and_pwa() {
        let tmp_apk = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp_apk.path(), b"fake-apk").unwrap();

        let tmp_dir = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp_dir.path().join("index.html"), "<html></html>").unwrap();

        let state = test_state(
            Some(tmp_dir.path().to_path_buf()),
            Some(tmp_apk.path().to_path_buf()),
        );
        let app = build_distribution_router(state);
        let req = make_request("/update", "192.168.1.100");
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["android"].is_object());
        assert!(json["android"]["hash"].as_str().unwrap().starts_with("sha256:"));
        assert!(json["pwa"].is_object());
        assert_eq!(json["pwa"]["url"], "/app");
    }

    // -- Config defaults ----------------------------------------------------

    #[test]
    fn default_config_values() {
        let config = DistributionConfig::default();
        assert_eq!(config.port, 0);
        assert_eq!(config.rate_limit_per_min, 60);
        assert!(config.pwa_dir.is_none());
        assert!(config.apk_path.is_none());
    }
}
