// Hides the black console window on Windows release builds!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::net::TcpListener;
use tauri::{WebviewUrl, WebviewWindowBuilder};

fn main() {
  tauri::Builder::default()
    .setup(|app| {
      // 1. Bind to port 0 so the OS assigns an open port (zero port conflicts)
      let std_listener =
        TcpListener::bind("127.0.0.1:0").expect("Failed to bind local ephemeral port");
      std_listener
        .set_nonblocking(true)
        .expect("Failed to set non-blocking listener");
      let port = std_listener.local_addr().unwrap().port();

      // 2. Spawn our existing Rust backend router (`server::create_router()`) inside Tauri's Tokio runtime
      tauri::async_runtime::spawn(async move {
        axum::Server::from_tcp(std_listener)
          .expect("Failed to create Axum server from TCP listener")
          .serve(server::create_router().into_make_service())
          .await
          .expect("Embedded backend server error");
      });

      // 3. Create the main window and inject the dynamic port into `window.__API_BASE__`
      let init_script = format!("window.__API_BASE__ = 'http://127.0.0.1:{}';", port);
      WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("Orca Studio Configurator")
        .inner_size(1280.0, 860.0)
        .initialization_script(&init_script)
        .build()?;

      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
