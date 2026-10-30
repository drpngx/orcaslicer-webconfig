// Hides the black console window on Windows release builds!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use cap_std::{ambient_authority, fs::Dir};
use std::net::TcpListener;
use std::path::Path;
use tauri::{WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_fs::FsExt;

fn main() {
  tauri::Builder::default()
    .plugin(tauri_plugin_fs::init())
    .setup(|app| {
      // 1. Restrict Tauri filesystem scope (`tauri-plugin-fs`) + verify capability directory (`cap-std`)
      let cfg = server::load_local_slicer_config();
      let slicer_root = Path::new(&cfg.local_slicer_directory);
      if slicer_root.exists() {
        if let Ok(_cap_dir) = Dir::open_ambient_dir(slicer_root, ambient_authority()) {
          let fs_scope = app.fs_scope();
          let _ = fs_scope.allow_directory(slicer_root, true);
        }
      }

      // 2. Bind to port 0 so the OS assigns an open port (zero port conflicts)
      let std_listener =
        TcpListener::bind("127.0.0.1:0").expect("Failed to bind local ephemeral port");
      std_listener
        .set_nonblocking(true)
        .expect("Failed to set non-blocking listener");
      let port = std_listener.local_addr().unwrap().port();

      // 3. Spawn our existing Rust backend router (`server::create_router()`) inside Tauri's Tokio runtime
      tauri::async_runtime::spawn(async move {
        axum::Server::from_tcp(std_listener)
          .expect("Failed to create Axum server from TCP listener")
          .serve(server::create_router().into_make_service())
          .await
          .expect("Embedded backend server error");
      });

      // 4. Create the main window and inject the dynamic port + Tauri desktop mode flag
      let init_script = format!(
        "window.__API_BASE__ = 'http://127.0.0.1:{}'; window.__ORCA_WEBCONFIG_TAURI__ = true;",
        port
      );
      WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("OrcaSlicer WebConfig (for v2.2)")
        .inner_size(1280.0, 860.0)
        .initialization_script(&init_script)
        .build()?;

      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
