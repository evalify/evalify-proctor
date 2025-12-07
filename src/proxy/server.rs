use hyper::{Body, Client, Request, Response, Server, StatusCode, Uri};
use hyper::service::{make_service_fn, service_fn};
use hyper::client::HttpConnector;
use std::net::SocketAddr;
use std::convert::Infallible;

const ENCRYPTED_B64: &str = include_str!("../encrypted_blob.b64");

static ALLOWED_DOMAINS: &[&str] = &[
    "evalify.amritanet.edu",
    "localhost:3000"
];

fn is_allowed_domain(domain : &Uri) -> bool {
    let host = match domain.host(){
        Some(host) => host,
        None => return false,
    };

    let port = domain.port_u16();
    for allowed in ALLOWED_DOMAINS{
        if let Some((a_host,a_port)) = allowed.split_once(":"){
            if host == a_host {
                if let Ok(a_port) = a_port.parse::<u16>(){
                    if port == Some(a_port){
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

async fn proxy_handler(
    mut req: Request<Body>,
    client: Client<HttpConnector>,
) -> Result<Response<Body>, hyper::Error> {

    let uri = req.uri().clone();

    if !is_allowed_domain(&uri) {
        return Ok(Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body(Body::from("Forbidden"))
            .unwrap());
    }

    if is_allowed_domain(&uri){
        req.headers_mut().insert(
            "X-Kioski-Encrypted",
            hyper::header::HeaderValue::from_str(ENCRYPTED_B64.trim())
                .expect("Invalid ENCRYPTED_B64 header value"),
        );
    }
    let response = client.request(req).await?;
    Ok(response)
}

pub async fn run(port: u16) {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let client = std::sync::Arc::new(Client::new());

    let make_svc = make_service_fn(move |_| {
        let client = client.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                let client = client.clone();
                async move {
                    proxy_handler(req, client)
                }
            }))
        }
    });
    println!("Listening on http://{}", addr);
    let server = Server::bind(&addr).serve(make_svc).await;
    if let Err(e) = server {
        eprintln!("Server error: {}", e);
    }
}