use hyper::{Body, Client, Method, Request, Response, Server, StatusCode, Uri};
use hyper::service::{make_service_fn, service_fn};
use hyper::client::HttpConnector;
use hyper::upgrade::Upgraded;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::Notify;

const ENCRYPTED_B64: &str = include_str!("../../encrypted_blob.b64");

fn is_allowed_domain(domain: &Uri) -> bool {
    let host = match domain.host() {
        Some(host) => host,
        None => return false,
    };

    let port = domain.port_u16();
    for allowed in &crate::config::CONFIG.allowed_domains {
        if let Some((a_host, a_port)) = allowed.split_once(":") {
            if host == a_host {
                if let Ok(a_port) = a_port.parse::<u16>() {
                    if port == Some(a_port) {
                        return true;
                    }
                }
            }
        } else if host == *allowed {
            return true;
        }
    }
    false
}

fn is_logout_path(uri: &Uri) -> bool {
    let path = uri.path();
    crate::config::CONFIG
        .logout_paths
        .iter()
        .any(|p| path == p.as_str())
}

fn is_allowed_connect(host: &str, port: Option<u16>) -> bool {
    for allowed in &crate::config::CONFIG.allowed_domains {
        if let Some((a_host, a_port)) = allowed.split_once(":") {
            if host == a_host {
                if let Ok(a_port) = a_port.parse::<u16>() {
                    if port == Some(a_port) {
                        return true;
                    }
                }
            }
        } else if host == *allowed {
            // No port specified in config — allow default HTTPS port (443)
            return port.is_none() || port == Some(443);
        }
    }
    false
}

async fn handle_connect(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    let host = req.uri().host().unwrap_or("").to_string();
    let port = req.uri().port_u16();

    if !is_allowed_connect(&host, port) {
        return Ok(Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body(Body::from("Forbidden"))
            .unwrap());
    }

    let addr = format!("{}:{}", host, port.unwrap_or(443));

    tokio::task::spawn(async move {
        match hyper::upgrade::on(req).await {
            Ok(upgraded) => {
                if let Err(e) = tunnel(upgraded, &addr).await {
                    eprintln!("Tunnel error to {}: {}", addr, e);
                }
            }
            Err(e) => eprintln!("Upgrade error: {}", e),
        }
    });

    Ok(Response::new(Body::empty()))
}

async fn tunnel(upgraded: Upgraded, addr: &str) -> std::io::Result<()> {
    let mut server = TcpStream::connect(addr).await?;
    let mut client = upgraded;
    let (from_client, from_server) =
        tokio::io::copy_bidirectional(&mut client, &mut server).await?;
    println!(
        "Tunnel closed ({}): client sent {} bytes, server sent {} bytes",
        addr, from_client, from_server
    );
    Ok(())
}

async fn proxy_handler(
    mut req: Request<Body>,
    client: Client<HttpConnector>,
    logout_signal: Arc<Notify>,
) -> Result<Response<Body>, hyper::Error> {
    // Handle HTTPS tunneling via CONNECT
    if req.method() == Method::CONNECT {
        return handle_connect(req).await;
    }

    let uri = req.uri().clone();

    if !is_allowed_domain(&uri) {
        return Ok(Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body(Body::from("Forbidden"))
            .unwrap());
    }

    let logout = is_logout_path(&uri);

    req.headers_mut().insert(
        "X-Kioski-Encrypted",
        hyper::header::HeaderValue::from_str(ENCRYPTED_B64.trim())
            .expect("Invalid header value"),
    );

    let response = client.request(req).await?;

    if logout {
        logout_signal.notify_waiters();
    }

    Ok(response)
}

pub async fn run(
    listener: std::net::TcpListener,
    logout_signal: Arc<Notify>,
    proxy_shutdown: Arc<Notify>,
) {
    let client = Client::new();
    let logout_svc = logout_signal.clone();

    let make_svc = make_service_fn(move |_| {
        let client = client.clone();
        let logout_signal = logout_svc.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                let client = client.clone();
                let logout_signal = logout_signal.clone();
                async move {
                    proxy_handler(req, client, logout_signal).await
                }
            }))
        }
    });

    let server = Server::from_tcp(listener)
        .expect("Failed to create server from listener")
        .serve(make_svc)
        .with_graceful_shutdown(proxy_shutdown.notified());

    if let Err(e) = server.await {
        eprintln!("Server error: {}", e);
    }
}