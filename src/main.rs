use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use zeroize::Zeroize;

use std::{convert::Infallible, sync::Arc};
use tokio::sync::RwLock;
use tokio::process::Command;

use warp::Filter;
use reqwest::Client;
use warp::hyper::header::HeaderValue;

const ENCRYPTED_B64: &str = include_str!("../encrypted_blob.b64");

// Browser → Local proxy authentication key (example; replace per-device if you want)
const EMBEDDED_LOCAL_AUTH_KEY: &str =
    "35873c16f62dbf573b8381f6c4243508ce9ade4965b4a268f39a8b4c56e09f5f";


// ---------------------------------------------------------------
// Chromium launcher (full kiosk flags)
// ---------------------------------------------------------------
async fn launch_chromium() -> Result<()> {
    let url = "https://evalify.amritanet.edu";

    // Full list of flags (as requested)
    let args = vec![
        "--kiosk",
        "--fullscreen",
        "--incognito",
        "--no-first-run",
        "--no-default-browser-check",
        "--disable-features=Translate,TranslateUI,PrintBrowser,PrintPreview,Pay,AutofillServerCommunication,SharedArrayBuffer",
        "--disable-extensions",
        "--disable-component-extensions-with-background-pages",
        "--disable-plugins",
        "--disable-pdf-extension",
        "--disable-print-preview",
        "--disable-save-password-bubble",
        "--disable-logging",
        "--disable-notifications",
        "--disable-popup-blocking",
        "--disable-session-crashed-bubble",
        "--disable-infobars",
        "--disable-background-networking",
        "--disable-background-timer-throttling",
        "--disable-client-side-phishing-detection",
        "--disable-default-apps",
        "--disable-hang-monitor",
        "--disable-sync",
        "--disable-translate",
        "--disable-new-tab-first-run",
        "--disable-pinch",
        "--disable-smooth-scrolling",
        "--disable-remote-fonts",
        "--disable-file-system",
        "--disable-reading-from-canvas",
        "--disable-web-security",
        "--disable-renderer-accessibility",
        "--no-referrers",
        "--no-proxy-server",
        &format!("--app={}", url),
        "--window-size=1920,1080",
        "--overscroll-history-navigation=0",
        "--noerrdialogs",
        "--no-sandbox",
        "--disable-gpu",
        "--disable-software-rasterizer",
        "--disable-dev-shm-usage",
        "--disable-setuid-sandbox",
        "--disable-breakpad",
        "--disable-device-discovery-notifications",
        "--disable-print-preview",
        "--disable-user-media-security",
        "--disable-user-media",
        "--disable-webrtc",
        "--autoplay-policy=no-user-gesture-required",
        "--disable-popup-blocking",
        "--disable-webgl",
        "--disable-reading-from-canvas",
        "--remote-debugging-port=0",
        "--enable-logging=stderr",
    ];

    println!("Launching Chromium in kiosk mode…");

    // prefer "chromium" then "google-chrome"
    let chrome_bin = which::which("chromium")
        .or_else(|_| which::which("google-chrome"))
        .unwrap_or_else(|_| std::path::PathBuf::from("chromium"));

    let mut cmd = Command::new(chrome_bin);
    cmd.args(&args);

    // Spawn and do NOT wait — Chromium runs indefinitely
    cmd.spawn().expect("failed to launch chromium");

    Ok(())
}


// ---------------------------------------------------------------
// MAIN: local proxy that forwards encrypted kiosk blob to backend
// ---------------------------------------------------------------
#[tokio::main]
async fn main() -> Result<()> {
    let listen_addr = ([127, 0, 0, 1], 8473);
    let allowed_origin = "https://evalify.amritanet.edu";

    let backend_base = std::env::var("BACKEND_BASE_URL")
        .unwrap_or_else(|_| "https://evalify.amritanet.edu".to_string());

    // Load encrypted kiosk secret (do NOT decrypt client-side)
    let encrypted_blob = ENCRYPTED_B64.trim().to_string();

    let enc_arc = Arc::new(RwLock::new(encrypted_blob));
    let client = Arc::new(Client::builder().build()?);

    let client_filter = warp::any().map(move || client.clone());
    let encrypted_filter = warp::any().map(move || enc_arc.clone());

    let allowed_origin_str = allowed_origin.to_string();
    let backend_base_str = backend_base.clone();

    // Proxy route: /proxy/{tail...}
    let proxy_route = warp::path("proxy")
        .and(warp::path::tail())
        .and(warp::method())
        .and(warp::header::optional::<String>("origin"))
        .and(warp::header::optional::<String>("x-local-auth"))
        .and(warp::header::headers_cloned())
        .and(
            warp::query::raw()
                .or_else(|_| async { Ok::<(String,), Infallible>((String::new(),)) })
        )
        .and(warp::body::bytes())
        .and(client_filter)
        .and(encrypted_filter)
        .and_then(
            move |tail: warp::path::Tail,
                  method: warp::http::Method,
                  origin: Option<String>,
                  x_local_auth: Option<String>,
                  mut headers: warp::http::HeaderMap,
                  query: String,
                  body: bytes::Bytes,
                  client: Arc<Client>,
                  encrypted_arc: Arc<RwLock<String>>| {

                let allowed_origin = allowed_origin_str.clone();
                let backend_base = backend_base_str.clone();

                async move {
                    // 1) Origin check
                    if origin.as_deref() != Some(&allowed_origin) {
                        return Ok::<_, warp::Rejection>(
                            warp::reply::with_status("Forbidden origin",
                                warp::http::StatusCode::FORBIDDEN)
                        );
                    }

                    // 2) Local auth header check
                    match x_local_auth {
                        Some(ref v) if v == EMBEDDED_LOCAL_AUTH_KEY => {}
                        _ => {
                            return Ok::<_, warp::Rejection>(
                                warp::reply::with_status("Unauthorized",
                                    warp::http::StatusCode::UNAUTHORIZED)
                            );
                        }
                    }

                    // 3) Build backend URL
                    let mut url = format!(
                        "{}/{}",
                        backend_base.trim_end_matches('/'),
                        tail.as_str().trim_start_matches('/')
                    );
                    if !query.is_empty() {
                        url.push('?');
                        url.push_str(&query);
                    }

                    // 4) Prepare outbound request and copy headers (strip local-auth etc.)
                    let mut req_builder = client.request(method.clone(), &url);

                    headers.remove("host");
                    headers.remove("x-local-auth");
                    headers.remove("origin");
                    headers.remove("content-length");

                    for (name, value) in headers.iter() {
                        req_builder = req_builder.header(name, value.clone());
                    }

                    // 5) Inject encrypted kiosk blob header (backend will decrypt/validate)
                    let encrypted_blob = encrypted_arc.read().await;
                    req_builder = req_builder.header(
                        "X-Kiosk-Encrypted",
                        HeaderValue::from_str(&encrypted_blob)
                            .map_err(|_| warp::reject::custom(()))?
                    );

                    // 6) Set body if present
                    if !body.is_empty() {
                        req_builder = req_builder.body(body.to_vec());
                    }

                    // 7) Forward to backend
                    let resp = req_builder.send().await
                        .map_err(|_| warp::reject::custom(()))?;

                    let status = resp.status();
                    let resp_bytes = resp.bytes().await
                        .map_err(|_| warp::reject::custom(()))?;

                    // 8) Build reply copying headers and body
                    let mut response = warp::reply::Response::new(warp::hyper::Body::from(resp_bytes));
                    *response.status_mut() = status;

                    for (name, value) in resp.headers().iter() {
                        if !name.as_str().eq_ignore_ascii_case("transfer-encoding") {
                            response.headers_mut().insert(name.clone(), value.clone());
                        }
                    }

                    Ok::<_, warp::Rejection>(response)
                }
            }
        );

    // Launch Chromium (background)
    tokio::spawn(async {
        if let Err(e) = launch_chromium().await {
            eprintln!("Chromium launch error: {:?}", e);
        }
    });

    println!("Local authenticated proxy running at http://127.0.0.1:8473");

    // Run warp server
    warp::serve(proxy_route).run(listen_addr).await;

    Ok(())
}
