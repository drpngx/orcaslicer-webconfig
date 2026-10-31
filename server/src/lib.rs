use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use axum::body::Body;
use axum::extract::{Json, Path as AxumPath};
use axum::http::{header, HeaderMap, HeaderValue, Method, Response, StatusCode};
use axum::response::{Html, IntoResponse};
use axum::routing::{get, post};
use axum::Router;
use cap_std::{ambient_authority, fs::Dir};
use log::info;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScannedFileEntry {
  pub relative_path: String,
  pub filename: String,
  pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalFsScanPayload {
  pub configured_path: String,
  pub windows_equivalent_path: String,
  pub entries: Vec<ScannedFileEntry>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalFsScanProgress {
  pub running: bool,
  pub done: bool,
  pub phase: String,
  pub files_read: usize,
  pub total_files: usize,
  pub current_file: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalFsSaveFileEntry {
  pub relative_path: String,
  pub content: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalFsSaveRequest {
  pub files: Vec<LocalFsSaveFileEntry>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalFsSaveResponse {
  pub ok: bool,
  pub files_saved: usize,
  pub message: String,
}

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

/// Resolves the OS AppData/Roaming (Windows / WSL), Application Support (macOS), or ~/.config (Linux) directory
/// dynamically using the current user / home directory without hardcoding any username.
pub fn resolve_os_roaming_dir() -> Option<PathBuf> {
  if let Ok(appdata) = std::env::var("APPDATA") {
    let p = PathBuf::from(appdata);
    if p.exists() {
      return Some(p);
    }
  }
  if let Ok(userprofile) = std::env::var("USERPROFILE") {
    let p = PathBuf::from(userprofile).join("AppData").join("Roaming");
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
  }
  let wsl_users = Path::new("/mnt/c/Users");
  if wsl_users.is_dir() {
    for env_key in ["WINUSER", "USERNAME", "USER", "LOGNAME"] {
      if let Ok(u) = std::env::var(env_key) {
        if !u.trim().is_empty() {
          let candidate = wsl_users.join(u.trim()).join("AppData").join("Roaming");
          if candidate.is_dir() {
            return Some(candidate);
          }
        }
      }
    }
    if let Ok(entries) = std::fs::read_dir(wsl_users) {
      let mut fallback_roaming: Option<PathBuf> = None;
      for entry in entries.flatten() {
        let uname = entry.file_name().to_string_lossy().to_string();
        if matches!(
          uname.as_str(),
          "Public" | "Default" | "Default User" | "All Users" | "desktop.ini"
        ) {
          continue;
        }
        let roaming = entry.path().join("AppData").join("Roaming");
        if roaming.is_dir() {
          if roaming.join("AnycubicSlicerNext").is_dir() || roaming.join("OrcaSlicer").is_dir() {
            return Some(roaming);
          }
          if fallback_roaming.is_none() {
            fallback_roaming = Some(roaming);
          }
        }
      }
      if fallback_roaming.is_some() {
        return fallback_roaming;
      }
    }
  }
  if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
    let p = PathBuf::from(xdg);
    if p.exists() {
      return Some(p);
    }
  }
  if let Ok(home) = std::env::var("HOME") {
    return Some(PathBuf::from(home).join(".config"));
  }
  None
}

fn derive_windows_equivalent_path(resolved_dir: &Path, fallback: &str) -> String {
  let s = resolved_dir.to_string_lossy();
  if let Some(rest) = s.strip_prefix("/mnt/c/") {
    return format!(r"C:\{}", rest.replace('/', r"\"));
  }
  if s.contains(r":\") {
    return s.to_string();
  }
  if !fallback.trim().is_empty() && fallback != "auto" {
    return fallback.to_string();
  }
  r"%APPDATA%\AnycubicSlicerNext".to_string()
}

pub fn load_local_slicer_config() -> LocalSlicerConfigFile {
  if let Ok(env_dir) = std::env::var("ORCASLICER_LOCAL_DIR") {
    if !env_dir.trim().is_empty() {
      let p = PathBuf::from(env_dir.trim());
      return LocalSlicerConfigFile {
        windows_equivalent_path: derive_windows_equivalent_path(&p, ""),
        local_slicer_directory: p.to_string_lossy().to_string(),
      };
    }
  }

  let raw_str = find_asset("local_slicer_config.json")
    .and_then(|b| String::from_utf8(b).ok())
    .unwrap_or_else(|| include_str!("../../local_slicer_config.json").to_string());

  let mut cfg =
    serde_json::from_str::<LocalSlicerConfigFile>(&raw_str).unwrap_or(LocalSlicerConfigFile {
      local_slicer_directory: "auto".to_string(),
      windows_equivalent_path: r"%APPDATA%\AnycubicSlicerNext".to_string(),
    });

  let needs_auto = cfg.local_slicer_directory.trim().is_empty()
    || cfg.local_slicer_directory == "auto"
    || cfg.local_slicer_directory.contains('%')
    || cfg.local_slicer_directory.starts_with('~')
    || !Path::new(&cfg.local_slicer_directory).exists();

  if needs_auto {
    if let Some(roaming) = resolve_os_roaming_dir() {
      let anycubic = roaming.join("AnycubicSlicerNext");
      let orca = roaming.join("OrcaSlicer");
      let chosen = if anycubic.exists() || !orca.exists() {
        anycubic
      } else {
        orca
      };
      cfg.windows_equivalent_path =
        derive_windows_equivalent_path(&chosen, &cfg.windows_equivalent_path);
      cfg.local_slicer_directory = chosen.to_string_lossy().to_string();
    }
  } else {
    cfg.windows_equivalent_path = derive_windows_equivalent_path(
      Path::new(&cfg.local_slicer_directory),
      &cfg.windows_equivalent_path,
    );
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
  current_dir: &Dir,
  rel_prefix: &str,
  depth: usize,
  discovered: &mut Vec<(String, String)>,
  state: &Arc<Mutex<ServerScanState>>,
) {
  if depth > 6 {
    return;
  }
  let Ok(read_dir) = current_dir.entries() else {
    return;
  };
  let mut dir_entries: Vec<_> = read_dir.filter_map(|e| e.ok()).collect();
  dir_entries.sort_by_key(|e| e.file_name());

  for entry in dir_entries {
    let Ok(ft) = entry.file_type() else {
      continue;
    };
    let fname = entry.file_name().to_string_lossy().to_string();
    let rel = if rel_prefix.is_empty() {
      fname.clone()
    } else {
      format!("{}/{}", rel_prefix, fname)
    };

    if ft.is_dir() {
      if matches!(
        fname.as_str(),
        "log" | "cache" | "plugins" | "web" | "models" | "ota" | "Creality" | "BBL"
      ) {
        continue;
      }
      if let Ok(sub_dir) = current_dir.open_dir(&fname) {
        discover_local_fs_files(&sub_dir, &rel, depth + 1, discovered, state);
      }
    } else if ft.is_file() {
      if fname.ends_with(".conf") || fname.ends_with(".json") || fname.ends_with(".info") {
        discovered.push((rel.clone(), fname));
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
  let root_cap_dir = Dir::open_ambient_dir(root_path, ambient_authority()).ok();
  if let Some(ref cap_dir) = root_cap_dir {
    discover_local_fs_files(cap_dir, "", 0, &mut discovered, state);
  }

  let total = discovered.len();
  if let Ok(mut guard) = state.lock() {
    guard.progress.phase = "Reading configuration files from disk...".to_string();
    guard.progress.total_files = total;
  }

  let mut entries = Vec::with_capacity(total);
  if let Some(ref cap_dir) = root_cap_dir {
    for (idx, (rel, fname)) in discovered.into_iter().enumerate() {
      if let Ok(mut guard) = state.lock() {
        guard.progress.files_read = idx + 1;
        guard.progress.current_file = rel.clone();
      }
      if let Ok(bytes) = cap_dir.read(&rel) {
        entries.push(ScannedFileEntry {
          relative_path: rel,
          filename: fname,
          bytes,
        });
      }
    }
  }

  info!(
    "Scanned local filesystem directory '{}' via cap-std -> {} files read",
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

  let Ok(cap_dir) = Dir::open_ambient_dir(root_path, ambient_authority()) else {
    let resp = LocalFsSaveResponse {
      ok: false,
      files_saved: 0,
      message: format!(
        "Capability sandbox could not open slicer directory: {}",
        cfg.local_slicer_directory
      ),
    };
    let body = serde_json::to_string(&resp).unwrap_or_else(|_| "{}".to_string());
    let mut res = Response::new(Body::from(body));
    res.headers_mut().insert(
      header::CONTENT_TYPE,
      HeaderValue::from_static("application/json"),
    );
    return with_cors(res);
  };

  // Discover all existing `user/<id>/` subdirectories on disk (e.g. `434139` and `default`)
  // so that even if the client session started from scratch (`user/default/...`), every
  // saved `machine/`, `process/`, and `filament/` `.json` preset is mirrored into all active
  // user account folders on disk.
  let mut existing_user_folders: Vec<String> = Vec::new();
  if let Ok(user_dir) = cap_dir.open_dir("user") {
    if let Ok(entries_iter) = user_dir.entries() {
      for entry_res in entries_iter.flatten() {
        if let Ok(ft) = entry_res.file_type() {
          if ft.is_dir() {
            if let Ok(name) = entry_res.file_name().into_string() {
              if !name.is_empty() && !name.starts_with('.') {
                existing_user_folders.push(name);
              }
            }
          }
        }
      }
    }
  }

  let mut files_saved = 0usize;
  for file_item in &req.files {
    let rel = file_item.relative_path.replace('\\', "/");
    let is_allowed_target =
      (rel.starts_with("user/") && rel.ends_with(".json")) || rel.ends_with(".conf");
    if rel.contains("..") || !is_allowed_target {
      continue;
    }

    // If writing `AnycubicSlicerNext.conf` and the incoming payload was synthesized without
    // an initial directory scan (missing `app.slicer_uuid`), merge its `presets`,
    // `anycubic_presets`, `models`, `filament_presets`, and `app.flush_multiplier` into the
    // existing on-disk `AnycubicSlicerNext.conf` so `app.preset_folder` (e.g. `"434139"`)
    // and cloud/window settings are never clobbered.
    let content_to_write = if rel.ends_with(".conf") {
      if let Ok(existing_text) = cap_dir.read_to_string(&rel) {
        let ex_json_part = existing_text
          .split("# MD5")
          .next()
          .unwrap_or(&existing_text)
          .trim();
        let in_json_part = file_item
          .content
          .split("# MD5")
          .next()
          .unwrap_or(&file_item.content)
          .trim();
        if let (
          Ok(serde_json::Value::Object(mut ex_map)),
          Ok(serde_json::Value::Object(in_map)),
        ) = (
          serde_json::from_str::<serde_json::Value>(ex_json_part),
          serde_json::from_str::<serde_json::Value>(in_json_part),
        ) {
          let incoming_is_synthetic = in_map
            .get("app")
            .and_then(|a| a.get("slicer_uuid"))
            .is_none();
          if incoming_is_synthetic {
            for copy_key in ["presets", "anycubic_presets", "models", "filament_presets"] {
              if let Some(v) = in_map.get(copy_key) {
                ex_map.insert(copy_key.to_string(), v.clone());
              }
            }
            if let Some(fm) = in_map
              .get("app")
              .and_then(|a| a.get("flush_multiplier"))
              .cloned()
            {
              if let Some(serde_json::Value::Object(ex_app)) = ex_map.get_mut("app") {
                ex_app.insert("flush_multiplier".to_string(), fm);
              }
            }
            serde_json::to_string_pretty(&serde_json::Value::Object(ex_map))
              .unwrap_or_else(|_| file_item.content.clone())
          } else {
            file_item.content.clone()
          }
        } else {
          file_item.content.clone()
        }
      } else {
        file_item.content.clone()
      }
    } else {
      file_item.content.clone()
    };

    let mut target_rels = vec![rel.clone()];
    if let Some(after_user) = rel.strip_prefix("user/") {
      if let Some((_src_folder, subpath)) = after_user.split_once('/') {
        if subpath.starts_with("machine/")
          || subpath.starts_with("process/")
          || subpath.starts_with("filament/")
        {
          for folder in &existing_user_folders {
            let mirror_rel = format!("user/{}/{}", folder, subpath);
            if !target_rels.contains(&mirror_rel) {
              target_rels.push(mirror_rel);
            }
          }
        }
      }
    }

    for target_rel in target_rels {
      if let Some(parent) = Path::new(&target_rel).parent() {
        if !parent.as_os_str().is_empty() {
          let _ = cap_dir.create_dir_all(parent);
        }
      }
      if cap_dir
        .write(&target_rel, content_to_write.as_bytes())
        .is_ok()
      {
        if let Ok(mut guard) = state.lock() {
          if let Some(ref mut payload) = guard.result {
            if let Some(entry) = payload
              .entries
              .iter_mut()
              .find(|e| e.relative_path == target_rel)
            {
              entry.bytes = content_to_write.as_bytes().to_vec();
            }
          }
        }
        files_saved += 1;
      }
    }
  }

  info!(
    "Saved {} modified user configuration files to '{}' via cap-std",
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
  let Some(roaming_dir) = resolve_os_roaming_dir() else {
    let mut err_res = Response::new(Body::from("Could not find OS AppData/Roaming directory"));
    *err_res.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
    return with_cors(err_res);
  };
  if !matches!(req.app_folder.as_str(), "AnycubicSlicerNext" | "OrcaSlicer")
    || req.file_name.contains("..")
  {
    let mut err_res = Response::new(Body::from(
      "Access denied by capability sandbox: only AnycubicSlicerNext or OrcaSlicer allowed",
    ));
    *err_res.status_mut() = StatusCode::FORBIDDEN;
    return with_cors(err_res);
  }
  let target_root = roaming_dir.join(&req.app_folder);
  let Ok(cap_dir) = Dir::open_ambient_dir(&target_root, ambient_authority()) else {
    let mut err_res = Response::new(Body::from(format!(
      "Could not open capability directory {:?}",
      target_root
    )));
    *err_res.status_mut() = StatusCode::NOT_FOUND;
    return with_cors(err_res);
  };

  match cap_dir.read_to_string(&req.file_name) {
    Ok(content) => {
      let full_path = target_root.join(&req.file_name).display().to_string();
      let payload = FileResponse { full_path, content };
      let body = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());
      let mut res = Response::new(Body::from(body));
      res.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
      );
      with_cors(res)
    }
    Err(e) => {
      let mut err_res = Response::new(Body::from(format!(
        "Failed to read {:?} in {:?}: {}",
        req.file_name, target_root, e
      )));
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

async fn serve_wasm_bg(headers: HeaderMap) -> impl IntoResponse {
  let accepts_br = headers
    .get(header::ACCEPT_ENCODING)
    .and_then(|v| v.to_str().ok())
    .map(|s| s.split(',').any(|part| part.trim().starts_with("br")))
    .unwrap_or(false);

  if accepts_br {
    if let Some(bytes) = find_asset("app_wasm/app_wasm_bg_opt.wasm.br") {
      let mut res = Response::new(Body::from(bytes));
      res.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/wasm"),
      );
      res
        .headers_mut()
        .insert(header::CONTENT_ENCODING, HeaderValue::from_static("br"));
      res.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-cache, must-revalidate"),
      );
      return res.into_response();
    }
  }

  if let Some(bytes) =
    find_asset("app_wasm/app_wasm_bg_opt.wasm").or_else(|| find_asset("app_wasm/app_wasm_bg.wasm"))
  {
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

async fn serve_doc_image(AxumPath(filename): AxumPath<String>) -> impl IntoResponse {
  if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
    return (StatusCode::BAD_REQUEST, "Invalid image filename").into_response();
  }
  let rel = format!("doc/images/{}", filename);
  if let Some(bytes) = find_asset(&rel) {
    let content_type = if filename.ends_with(".svg") {
      "image/svg+xml"
    } else {
      "image/png"
    };
    let mut res = Response::new(Body::from(bytes));
    res
      .headers_mut()
      .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    return res.into_response();
  }
  (StatusCode::NOT_FOUND, "Doc image not found").into_response()
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
    .route("/doc/images/:filename", get(serve_doc_image))
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
