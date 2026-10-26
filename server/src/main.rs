use std::net::SocketAddr;
use std::path::Path;

use anyhow::Result;
use axum::body::Body;
use axum::http::{header, HeaderValue, Response, StatusCode};
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use axum::Router;
use clap::Parser;
use log::info;

#[derive(Parser, Debug)]
#[clap(name = "orcaslicer-webconfig-server")]
struct Args {
    #[clap(short, long)]
    port: Option<u16>,

    #[clap(short = 'H', long)]
    host: Option<String>,
}

fn find_asset(relative_path: &str) -> Option<Vec<u8>> {
    let candidates = [
        format!("{}", relative_path),
        format!("_main/{}", relative_path),
        format!("orcaslicer_webconfig/{}", relative_path),
        format!("server/server.runfiles/_main/{}", relative_path),
        format!("server/server.runfiles/orcaslicer_webconfig/{}", relative_path),
        format!("bazel-bin/{}", relative_path),
    ];
    for c in &candidates {
        if let Ok(bytes) = std::fs::read(c) {
            return Some(bytes);
        }
    }
    if let Ok(runfiles) = std::env::var("RUNFILES_DIR") {
        let p1 = Path::new(&runfiles).join("_main").join(relative_path);
        if let Ok(b) = std::fs::read(&p1) {
            return Some(b);
        }
        let p2 = Path::new(&runfiles).join("orcaslicer_webconfig").join(relative_path);
        if let Ok(b) = std::fs::read(&p2) {
            return Some(b);
        }
    }
    None
}

async fn index() -> impl IntoResponse {
    if let Some(bytes) = find_asset("static/index.html") {
        if let Ok(html) = String::from_utf8(bytes) {
            return Html(html).into_response();
        }
    }
    Html("<!DOCTYPE html><html><head><title>OrcaSlicer WebConfig</title></head><body><h1>Loading OrcaSlicer WebConfig...</h1><script type='module'>import init from '/app_wasm.js'; init();</script></body></html>".to_string()).into_response()
}

async fn serve_wasm_js() -> impl IntoResponse {
    if let Some(bytes) = find_asset("app_wasm/app_wasm.js") {
        let mut res = Response::new(Body::from(bytes));
        res.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("application/javascript"));
        return res.into_response();
    }
    (StatusCode::NOT_FOUND, "app_wasm.js not found").into_response()
}

async fn serve_wasm_bg() -> impl IntoResponse {
    if let Some(bytes) = find_asset("app_wasm/app_wasm_bg.wasm") {
        let mut res = Response::new(Body::from(bytes));
        res.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("application/wasm"));
        return res.into_response();
    }
    (StatusCode::NOT_FOUND, "app_wasm_bg.wasm not found").into_response()
}

fn app() -> Router {
    Router::new()
        .route("/", get(index))
        .route("/index.html", get(index))
        .route("/app_wasm.js", get(serve_wasm_js))
        .route("/app_wasm_bg.wasm", get(serve_wasm_bg))
        .fallback(get(index))
}

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::init();

    let args = Args::parse();

    let (env_host, env_port) = if let Ok(addr_str) = std::env::var("HTTP_LISTEN_ADDR") {
        if let Ok(sa) = addr_str.parse::<SocketAddr>() {
            (sa.ip().to_string(), sa.port())
        } else if let Some((h, p)) = addr_str.split_once(':') {
            (h.to_string(), p.parse().unwrap_or(8080))
        } else {
            ("0.0.0.0".to_string(), 8080)
        }
    } else {
        ("0.0.0.0".to_string(), 8080)
    };

    let host = args.host.unwrap_or(env_host);
    let port = args.port.unwrap_or(env_port);
    let socket_addr: SocketAddr = format!("{}:{}", host, port).parse()?;

    info!("OrcaSlicer WebConfig server listening on http://{}", socket_addr);
    info!("Open http://127.0.0.1:{} in your browser", port);

    let server = axum::Server::bind(&socket_addr).serve(app().into_make_service());
    server.await?;

    Ok(())
}
