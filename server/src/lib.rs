use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use axum::body::Body;
use axum::extract::Json;
use axum::http::{header, HeaderValue, Method, Response, StatusCode};
use axum::response::{Html, IntoResponse};
use axum::routing::{get, post};
use axum::Router;
use log::info;
use orcaslicer_webconfig::fs_access::{
  LocalFsSaveRequest, LocalFsSaveResponse, LocalFsScanPayload, LocalFsScanProgress,
  ScannedFileEntry,
};
use serde::{Deserialize, Serialize};

struct ServerScanState {
  progress: LocalFsScanProgress,
  result: Option<LocalFsScanPayload>,
}

fn scan_state() -> &'static Arc<Mutex<ServerScanState>> {
  static STATE: OnceLock<Arc<Mutex<ServerScanState>>> = OnceLock::new();
  STATE.get_or_init(|| {
    Arc::new(Mutex::new(ServerScanState {
      progress: LocalFsScanProgress {
        running: false,
        done: false,
        phase: "Idle".to_string(),
        files_read: 0,
        total_files: 0,
        current_file: String::new(),
      },
      result: None,
    }))
  })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocalSlicerConfigFile {
  pub local_slicer_directory: String,
  #[serde(default)]
  pub windows_equivalent_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExternalFileRequest {
  pub app_folder: String, // e.g. "AnycubicSlicerNext" or "OrcaSlicer" inside AppData/Roaming
  pub file_name: String,  // e.g. "AnycubicSlicerNext.conf"
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileResponse {
  pub full_path: String,
  pub content: String,
}

pub fn find_asset(relative_path: &str) -> Option<Vec<u8>> {
  let candidates = [
    relative_path.to_string(),
    format!("_main/{}", relative_path),
    format!("orcaslicer_webconfig/{}", relative_path),
    format!("server/server.runfiles/_main/{}", relative_path),
    format!(
      "server/server.runfiles/orcaslicer_webconfig/{}",
      relative_path
    ),
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
    let p2 = Path::new(&runfiles)
      .join("orcaslicer_webconfig")
      .join(relative_path);
    if let Ok(b) = std::fs::read(&p2) {
      return Some(b);
    }
  }
  None
}

/// Resolves the OS AppData/Roaming (Windows), Application Support (macOS), or ~/.config (Linux) directory.
pub fn resolve_os_roaming_dir() -> Option<PathBuf> {
  if let Ok(appdata) = std::env::var("APPDATA") {
    let p = PathBuf::from(appdata);
    if p.exists() {
      return Some(p);
    }
  }
  if let Ok(home) = std::env::var("HOME") {
    let mac_sup = PathBuf::from(&home)
      .join("Library")
      .join("Application Support");
    if mac_sup.exists() {
      return Some(mac_sup);
    }
    let wsl_roaming = PathBuf::from("/mnt/c/Users/drpng/AppData/Roaming");
    if wsl_roaming.exists() {
      return Some(wsl_roaming);
    }
    let linux_cfg = PathBuf::from(&home).join(".config");
    return Some(linux_cfg);
  }
  None
}

pub fn load_local_slicer_config() -> LocalSlicerConfigFile {
  let raw_str = find_asset("local_slicer_config.json")
    .and_then(|b| String::from_utf8(b).ok())
    .unwrap_or_else(|| include_str!("../../local_slicer_config.json").to_string());

  let mut cfg =
    serde_json::from_str::<LocalSlicerConfigFile>(&raw_str).unwrap_or(LocalSlicerConfigFile {
      local_slicer_directory: "/mnt/c/Users/drpng/AppData/Roaming/AnycubicSlicerNext".to_string(),
      windows_equivalent_path: r"C:\Users\drpng\AppData\Roaming\AnycubicSlicerNext".to_string(),
    });

  // If running as a portable Tauri binary on Windows or macOS where the WSL path doesn't exist,
  // automatically fall back to the native OS AppData/Roaming/AnycubicSlicerNext directory!
  if !Path::new(&cfg.local_slicer_directory).exists() {
    if let Some(roaming) = resolve_os_roaming_dir() {
      let candidate = roaming.join("AnycubicSlicerNext");
      if candidate.exists() {
        cfg.local_slicer_directory = candidate.to_string_lossy().to_string();
      }
    }
  }

  cfg
}

fn with_cors(mut res: Response<Body>) -> Response<Body> {
  let headers = res.headers_mut();
  headers.insert(
    header::ACCESS_CONTROL_ALLOW_ORIGIN,
    HeaderValue::from_static("*"),
  );
  headers.insert(
    header::ACCESS_CONTROL_ALLOW_METHODS,
    HeaderValue::from_static("GET, POST, OPTIONS"),
  );
  headers.insert(
    header::ACCESS_CONTROL_ALLOW_HEADERS,
    HeaderValue::from_static("Content-Type, Authorization"),
  );
  res
}

async fn cors_preflight() -> impl IntoResponse {
  with_cors(Response::new(Body::empty()))
}

fn discover_local_fs_files(
  root: &Path,
  current: &Path,
  depth: usize,
  discovered: &mut Vec<(PathBuf, String, String)>,
  state: &Arc<Mutex<ServerScanState>>,
) {
  if depth > 6 {
    return;
  }
  let Ok(read_dir) = std::fs::read_dir(current) else {
    return;
  };
  let mut dir_entries: Vec<_> = read_dir.filter_map(|e| e.ok()).collect();
  dir_entries.sort_by_key(|e| e.file_name());

  for entry in dir_entries {
    let path = entry.path();
    let Ok(ft) = entry.file_type() else {
      continue;
    };
    let fname = entry.file_name().to_string_lossy().to_string();

    if ft.is_dir() {
      if matches!(
        fname.as_str(),
        "log" | "cache" | "plugins" | "web" | "models" | "ota" | "Creality" | "BBL"
      ) {
        continue;
      }
      discover_local_fs_files(root, &path, depth + 1, discovered, state);
    } else if ft.is_file() {
      if fname.ends_with(".conf") || fname.ends_with(".json") || fname.ends_with(".info") {
        let rel = path
          .strip_prefix(root)
          .map(|p| p.to_string_lossy().replace('\\', "/"))
          .unwrap_or_else(|_| fname.clone());
        discovered.push((path, rel.clone(), fname));
        if let Ok(mut guard) = state.lock() {
          guard.progress.total_files = discovered.len();
          guard.progress.current_file = rel;
        }
      }
    }
  }
}

fn perform_full_local_scan(state: &Arc<Mutex<ServerScanState>>) -> LocalFsScanPayload {
  let cfg = load_local_slicer_config();
  let root_path = Path::new(&cfg.local_slicer_directory);

  if let Ok(mut guard) = state.lock() {
    guard.progress.running = true;
    guard.progress.done = false;
    guard.progress.phase = "Discovering configuration files in directory...".to_string();
    guard.progress.files_read = 0;
    guard.progress.total_files = 0;
    guard.progress.current_file = cfg.local_slicer_directory.clone();
  }

  let mut discovered = Vec::new();
  if root_path.exists() {
    discover_local_fs_files(root_path, root_path, 0, &mut discovered, state);
  }

  let total = discovered.len();
  if let Ok(mut guard) = state.lock() {
    guard.progress.phase = "Reading configuration files from disk...".to_string();
    guard.progress.total_files = total;
  }

  let mut entries = Vec::with_capacity(total);
  for (idx, (path, rel, fname)) in discovered.into_iter().enumerate() {
    if let Ok(mut guard) = state.lock() {
      guard.progress.files_read = idx + 1;
      guard.progress.current_file = rel.clone();
    }
    if let Ok(bytes) = std::fs::read(&path) {
      entries.push(ScannedFileEntry {
        relative_path: rel,
        filename: fname,
        bytes,
      });
    }
  }

  info!(
    "Scanned local filesystem directory '{}' -> {} files read",
    cfg.local_slicer_directory,
    entries.len()
  );

  let payload = LocalFsScanPayload {
    configured_path: cfg.local_slicer_directory,
    windows_equivalent_path: cfg.windows_equivalent_path,
    entries,
  };

  if let Ok(mut guard) = state.lock() {
    guard.progress.running = false;
    guard.progress.done = true;
    guard.progress.phase = "Scan complete".to_string();
    guard.result = Some(payload.clone());
  }

  payload
}

async fn api_local_slicer_scan_start() -> impl IntoResponse {
  let state = scan_state().clone();
  let should_spawn = {
    let mut guard = state.lock().unwrap();
    if !guard.progress.running {
      guard.progress = LocalFsScanProgress {
        running: true,
        done: false,
        phase: "Discovering configuration files in directory...".to_string(),
        files_read: 0,
        total_files: 0,
        current_file: String::new(),
      };
      guard.result = None;
      true
    } else {
      false
    }
  };

  if should_spawn {
    let bg_state = state.clone();
    std::thread::spawn(move || {
      perform_full_local_scan(&bg_state);
    });
  }

  let prog = state.lock().unwrap().progress.clone();
  let body = serde_json::to_string(&prog).unwrap_or_else(|_| "{}".to_string());
  let mut res = Response::new(Body::from(body));
  res.headers_mut().insert(
    header::CONTENT_TYPE,
    HeaderValue::from_static("application/json"),
  );
  with_cors(res)
}

async fn api_local_slicer_scan_status() -> impl IntoResponse {
  let state = scan_state();
  let prog = state.lock().unwrap().progress.clone();
  let body = serde_json::to_string(&prog).unwrap_or_else(|_| "{}".to_string());
  let mut res = Response::new(Body::from(body));
  res.headers_mut().insert(
    header::CONTENT_TYPE,
    HeaderValue::from_static("application/json"),
  );
  with_cors(res)
}

async fn api_local_slicer_scan() -> impl IntoResponse {
  let state = scan_state().clone();
  let cached = state.lock().unwrap().result.clone();
  let payload = match cached {
    Some(p) => p,
    None => perform_full_local_scan(&state),
  };

  let body = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());
  let mut res = Response::new(Body::from(body));
  res.headers_mut().insert(
    header::CONTENT_TYPE,
    HeaderValue::from_static("application/json"),
  );
  with_cors(res)
}

async fn api_local_slicer_save(Json(req): Json<LocalFsSaveRequest>) -> impl IntoResponse {
  let cfg = load_local_slicer_config();
  let root_path = Path::new(&cfg.local_slicer_directory);
  let state = scan_state().clone();

  let mut files_saved = 0usize;
  for file_item in &req.files {
    if file_item.relative_path.contains("..") || !file_item.relative_path.starts_with("user/") {
      continue;
    }
    let full_path = root_path.join(&file_item.relative_path);
    if full_path.exists() {
      let _ = std::fs::write(&full_path, file_item.content.as_bytes());
    }
    if let Ok(mut guard) = state.lock() {
      if let Some(ref mut payload) = guard.result {
        if let Some(entry) = payload
          .entries
          .iter_mut()
          .find(|e| e.relative_path == file_item.relative_path)
        {
          entry.bytes = file_item.content.as_bytes().to_vec();
        }
      }
    }
    files_saved += 1;
  }

  info!(
    "Saved {} modified user configuration files to '{}'",
    files_saved, cfg.local_slicer_directory
  );

  let resp = LocalFsSaveResponse {
    ok: true,
    files_saved,
    message: format!(
      "Saved {} file(s) to {}",
      files_saved, cfg.local_slicer_directory
    ),
  };
  let body = serde_json::to_string(&resp).unwrap_or_else(|_| "{}".to_string());
  let mut res = Response::new(Body::from(body));
  res.headers_mut().insert(
    header::CONTENT_TYPE,
    HeaderValue::from_static("application/json"),
  );
  with_cors(res)
}

async fn read_roaming_file(Json(req): Json<ExternalFileRequest>) -> impl IntoResponse {
  let Some(mut path) = resolve_os_roaming_dir() else {
    let mut err_res = Response::new(Body::from("Could not find OS AppData/Roaming directory"));
    *err_res.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
    return with_cors(err_res);
  };
  if req.app_folder.contains("..") || req.file_name.contains("..") {
    let mut err_res = Response::new(Body::from("Invalid relative path"));
    *err_res.status_mut() = StatusCode::BAD_REQUEST;
    return with_cors(err_res);
  }
  path.push(&req.app_folder);
  path.push(&req.file_name);

  match tokio::fs::read_to_string(&path).await {
    Ok(content) => {
      let payload = FileResponse {
        full_path: path.display().to_string(),
        content,
      };
      let body = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());
      let mut res = Response::new(Body::from(body));
      res.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
      );
      with_cors(res)
    }
    Err(e) => {
      let mut err_res = Response::new(Body::from(format!("Failed to read {:?}: {}", path, e)));
      *err_res.status_mut() = StatusCode::NOT_FOUND;
      with_cors(err_res)
    }
  }
}

async fn index() -> impl IntoResponse {
  if let Some(bytes) = find_asset("static/index.html") {
    if let Ok(html) = String::from_utf8(bytes) {
      let mut res = Html(html).into_response();
      res.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-cache, must-revalidate"),
      );
      return res;
    }
  }
  Html("<!DOCTYPE html><html><head><title>OrcaSlicer WebConfig</title></head><body><h1>Loading OrcaSlicer WebConfig...</h1><script type='module'>import init from '/app_wasm.js'; init();</script></body></html>".to_string()).into_response()
}

async fn serve_wasm_js() -> impl IntoResponse {
  if let Some(bytes) = find_asset("app_wasm/app_wasm.js") {
    let mut res = Response::new(Body::from(bytes));
    res.headers_mut().insert(
      header::CONTENT_TYPE,
      HeaderValue::from_static("application/javascript"),
    );
    res.headers_mut().insert(
      header::CACHE_CONTROL,
      HeaderValue::from_static("no-store, no-cache, must-revalidate"),
    );
    return res.into_response();
  }
  (StatusCode::NOT_FOUND, "app_wasm.js not found").into_response()
}

async fn serve_wasm_bg() -> impl IntoResponse {
  if let Some(bytes) = find_asset("app_wasm/app_wasm_bg.wasm") {
    let mut res = Response::new(Body::from(bytes));
    res.headers_mut().insert(
      header::CONTENT_TYPE,
      HeaderValue::from_static("application/wasm"),
    );
    res.headers_mut().insert(
      header::CACHE_CONTROL,
      HeaderValue::from_static("no-store, no-cache, must-revalidate"),
    );
    return res.into_response();
  }
  (StatusCode::NOT_FOUND, "app_wasm_bg.wasm not found").into_response()
}

/// Shared router builder used by both the standalone web dev server (`server/src/main.rs`)
/// and the embedded Tauri v2 desktop wrapper (`src-tauri/src/main.rs`).
pub fn create_router() -> Router {
  let _ = Method::OPTIONS;
  Router::new()
    .route("/", get(index))
    .route("/index.html", get(index))
    .route("/app_wasm.js", get(serve_wasm_js))
    .route("/app_wasm_bg.wasm", get(serve_wasm_bg))
    .route(
      "/api/local-slicer-scan/start",
      get(api_local_slicer_scan_start).options(cors_preflight),
    )
    .route(
      "/api/local-slicer-scan/status",
      get(api_local_slicer_scan_status).options(cors_preflight),
    )
    .route(
      "/api/local-slicer-scan",
      get(api_local_slicer_scan).options(cors_preflight),
    )
    .route(
      "/api/local-slicer-save",
      post(api_local_slicer_save).options(cors_preflight),
    )
    .route(
      "/api/roaming/read",
      post(read_roaming_file).options(cors_preflight),
    )
    .fallback(get(index))
}
