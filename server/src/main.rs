use std::net::SocketAddr;

use anyhow::Result;
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
      (h.to_string(), p.parse().unwrap_or(8003))
    } else {
      ("0.0.0.0".to_string(), 8003)
    }
  } else {
    ("0.0.0.0".to_string(), 8003)
  };

  let host = args.host.unwrap_or(env_host);
  let port = args.port.unwrap_or(env_port);

  let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
  info!("OrcaSlicer WebConfig server listening on http://{}", addr);
  info!("Open http://127.0.0.1:{} in your browser", port);

  let app = server::create_router();
  axum::Server::bind(&addr)
    .serve(app.into_make_service())
    .await?;

  Ok(())
}
