use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use zeroize::Zeroize;

use std::{convert::Infallible, sync::Arc};
use tokio::sync::RwLock;
use tokio::process::Command;

use warp::Filter;
use reqwest::Client;
use warp::hyper::header::{HeaderValue};

const ENCRYPTED_B64: &str = include_str!("../encrypted_blob.b64");

// Browser → Local proxy authentication key
const EMBEDDED_LOCAL_AUTH_KEY: &str =
    "35873c16f62dbf573b8381f6c4243508ce9ade4965b4a268f39a8b4c56e09f5f";


// ---------------------------------------------------------------
// Chromium launcher
// ---------------------------------------------------------------
async fn launch_chromium() -> Result<()> {
    let url = "https://evalify.amritanet.edu";

    let args = vec![
        "--kiosk",
        "--fullscreen",
        "--incognito",
        "--no-first-run",
        "--no-default-browser-check",
        "--disable-extensions",
        "--disable-popup-blocking",
        "--disable-print-preview",
        "--disable-sync",
        "--disable-webgl",
        "--disable-webrtc",
        "--app=https://evalify.amritanet.edu",
    ];

    let mut cmd = Command::new("chromium");
    cmd.args(&args);
    cmd.spawn().expect("failed to launch chromium");

    Ok(())
}


// ---------------------------------------------------------------
// MAIN
// ---------------------------------------------------------------
#[tokio::main]
async fn main() -> Result<()> {
    let listen_addr = ([127, 0, 0, 1], 8473);
    let allowed_origin = "https://evalify.amritanet.edu";

    let backend_base = std::env::var("BACKEND_BASE_URL")
        .unwrap_or_else(|_| "https://evalify.amritanet.edu".to_string());

    // Load the encrypted kiosk secret (do NOT decrypt)
    let encrypted_blob = ENCRYPTED_B64.trim().to_string();

    let enc_arc = Arc::new(RwLock::new(encrypted_blob));
    let client = Arc::new(Client::builder().build()?);

    let client_filter = warp::any().map(move || client.clone());
    let encrypted_filter = warp::any().map(move || enc_arc.clone());

    let allowed_origin_str = allowed_origin.to_string();
    let backend_base_str = backend_base.clone();


    // ---------------------------------------------------------------
    // Proxy logic
    // ---------------------------------------------------------------
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
                    // Origin check
                    if origin.as_deref() != Some(&allowed_origin) {
                        return Ok::<_, warp::Rejection>(
                            warp::reply::with_status("Forbidden origin",
                                warp::http::StatusCode::FORBIDDEN)
                        );
                    }

                    // Local proxy auth check
                    match x_local_auth {
                        Some(ref v) if v == EMBEDDED_LOCAL_AUTH_KEY => {}
                        _ => {
                            return Ok::<_, warp::Rejection>(
                                warp::reply::with_status("Unauthorized",
                                    warp::http::StatusCode::UNAUTHORIZED)
                            );
                        }
                    }

                    // Build backend URL
                    let mut url = format!(
                        "{}/{}",
                        backend_base.trim_end_matches('/'),
                        tail.as_str().trim_start_matches('/')
                    );
                    if !query.is_empty() {
                        url.push('?');
                        url.push_str(&query);
                    }

                    let mut req_builder = client.request(method.clone(), &url);

                    headers.remove("host");
                    headers.remove("x-local-auth");
                    headers.remove("origin");
                    headers.remove("content-length");

                    for (name, value) in headers.iter() {
                        req_builder = req_builder.header(name, value.clone());
                    }

                    // SEND encrypted kiosk secret (unchanged)
                    let encrypted_blob = encrypted_arc.read().await;
                    req_builder = req_builder.header(
                        "X-Kiosk-Encrypted",
                        HeaderValue::from_str(&encrypted_blob)
                            .map_err(|_| warp::reject::custom(()))?
                    );

                    if !body.is_empty() {
                        req_builder = req_builder.body(body.to_vec());
                    }

                    // Send to backend
                    let resp = req_builder.send().await
                        .map_err(|_| warp::reject::custom(()))?;

                    let status = resp.status();
                    let resp_bytes = resp.bytes().await
                        .map_err(|_| warp::reject::custom(()))?;

                    let mut response = warp::reply::Response::new(
                        warp::hyper::Body::from(resp_bytes)
                    );
                    *response.status_mut() = status;

                    for (name, value) in resp.headers().iter() {
                        if name.as_str() != "transfer-encoding" {
                            response.headers_mut().insert(name.clone(), value.clone());
                        }
                    }

                    Ok::<_, warp::Rejection>(response)
                }
            }
        );


    // Launch chromium
    tokio::spawn(async {
        if let Err(e) = launch_chromium().await {
            eprintln!("Chromium launch error: {:?}", e);
        }
    });

    println!("Local authenticated proxy running at http://127.0.0.1:8473");

    warp::serve(proxy_route).run(listen_addr).await;

    Ok(())
}
