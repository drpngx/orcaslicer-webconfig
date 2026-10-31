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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpoolmanSyncRequest {
  pub spoolman_url: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpoolmanSyncResponse {
  pub ok: bool,
  pub raw_spools_json: String,
  pub message: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpoolmanPushRequest {
  pub spoolman_url: String,
  pub spool_id: u32,
  pub filament_id: u32,
  pub extruder_temp_c: i32,
  pub bed_temp_c: i32,
  pub flow_ratio: f32,
  pub pressure_advance: f32,
  pub max_volumetric_speed: f32,
  pub remaining_weight_g: i32,
  pub location: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpoolmanPushResponse {
  pub ok: bool,
  pub message: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MoonrakerSetActiveSpoolRequest {
  pub moonraker_url: String,
  pub spool_id: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MoonrakerSetActiveSpoolResponse {
  pub ok: bool,
  pub message: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnycubicAceSlotInfo {
  pub slot_index: usize,
  pub profile_name: String,
  pub material_type: String,
  pub color_hex: String,
  pub status: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct AnycubicSniffedPrinterState {
  pub discovery_tier: String,
  pub device_id: String,
  pub printer_model: String,
  pub active_machine_preset: String,
  pub active_process_preset: String,
  pub ams_filament_ids: String,
  pub cloud_session_detected: bool,
  pub mqtt_broker_url: String,
  pub mqtt_connected_in_logs: bool,
  pub lan_rinkhals_reachable: bool,
  pub lan_endpoint: String,
  pub dryer_status: String,
  pub slots: Vec<AnycubicAceSlotInfo>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnycubicAutoDiscoverRequest {
  pub lan_printer_url: String,
  pub cloud_token: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnycubicAutoDiscoverResponse {
  pub ok: bool,
  pub message: String,
  pub sniffed_state: AnycubicSniffedPrinterState,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnycubicPushSlotsRequest {
  pub lan_printer_url: String,
  pub slot_profile_names: Vec<String>,
  pub slot_colors_hex: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnycubicPushSlotsResponse {
  pub ok: bool,
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

fn normalize_http_base_url(raw: &str, default_url: &str) -> String {
  let trimmed = raw.trim().trim_end_matches('/');
  if trimmed.is_empty() {
    default_url.to_string()
  } else if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
    trimmed.to_string()
  } else {
    format!("http://{}", trimmed)
  }
}

async fn hyper_request_json(
  method: Method,
  url: &str,
  json_body: Option<String>,
) -> Result<(StatusCode, String), String> {
  let uri: hyper::Uri = url
    .parse()
    .map_err(|e| format!("Invalid URL '{}': {}", url, e))?;
  let client = hyper::Client::new();
  let mut req_builder = hyper::Request::builder().method(method).uri(uri);
  let body = if let Some(b) = json_body {
    req_builder = req_builder.header(header::CONTENT_TYPE, "application/json");
    Body::from(b)
  } else {
    Body::empty()
  };
  let req = req_builder
    .body(body)
    .map_err(|e| format!("Request build error: {}", e))?;
  let resp = tokio::time::timeout(std::time::Duration::from_secs(8), client.request(req))
    .await
    .map_err(|_| format!("Timed out connecting to {}", url))?
    .map_err(|e| format!("HTTP connection error to {}: {}", url, e))?;
  let status = resp.status();
  let bytes = hyper::body::to_bytes(resp.into_body())
    .await
    .map_err(|e| format!("Failed reading response from {}: {}", url, e))?;
  let text = String::from_utf8_lossy(&bytes).to_string();
  Ok((status, text))
}

async fn api_spoolman_sync(Json(req): Json<SpoolmanSyncRequest>) -> impl IntoResponse {
  let base_url = normalize_http_base_url(&req.spoolman_url, "http://127.0.0.1:7912");
  let target_url = format!("{}/api/v1/spool?allow_archived=false", base_url);
  let payload = match hyper_request_json(Method::GET, &target_url, None).await {
    Ok((status, body_text)) if status.is_success() => SpoolmanSyncResponse {
      ok: true,
      raw_spools_json: body_text,
      message: format!("Connected to Spoolman at {}", base_url),
    },
    Ok((status, body_text)) => SpoolmanSyncResponse {
      ok: false,
      raw_spools_json: "[]".to_string(),
      message: format!(
        "Spoolman returned HTTP {} from {}: {}",
        status,
        target_url,
        body_text.chars().take(160).collect::<String>()
      ),
    },
    Err(err) => SpoolmanSyncResponse {
      ok: false,
      raw_spools_json: "[]".to_string(),
      message: err,
    },
  };
  let body = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());
  let mut res = Response::new(Body::from(body));
  res.headers_mut().insert(
    header::CONTENT_TYPE,
    HeaderValue::from_static("application/json"),
  );
  with_cors(res)
}

async fn api_spoolman_push(Json(req): Json<SpoolmanPushRequest>) -> impl IntoResponse {
  let base_url = normalize_http_base_url(&req.spoolman_url, "http://127.0.0.1:7912");
  // Best-effort provision custom extra fields on Spoolman so PATCH succeeds even on a fresh Spoolman instance
  for (key, name) in [
    ("flow_ratio", "Flow Ratio"),
    ("pressure_advance", "Pressure Advance"),
    ("max_volumetric_speed", "Max Volumetric Speed"),
  ] {
    let field_url = format!("{}/api/v1/field/filament/{}", base_url, key);
    let field_body = serde_json::json!({
      "name": name,
      "field_type": "text"
    })
    .to_string();
    let _ = hyper_request_json(Method::POST, &field_url, Some(field_body)).await;
  }

  let fil_url = format!("{}/api/v1/filament/{}", base_url, req.filament_id);
  let fil_patch = serde_json::json!({
    "settings_extruder_temp": req.extruder_temp_c,
    "settings_bed_temp": req.bed_temp_c,
    "extra": {
      "flow_ratio": format!("\"{:.3}\"", req.flow_ratio),
      "pressure_advance": format!("\"{:.3}\"", req.pressure_advance),
      "max_volumetric_speed": format!("\"{:.1}\"", req.max_volumetric_speed),
    }
  })
  .to_string();
  let fil_res = hyper_request_json(Method::PATCH, &fil_url, Some(fil_patch)).await;

  let spool_url = format!("{}/api/v1/spool/{}", base_url, req.spool_id);
  let spool_patch = serde_json::json!({
    "remaining_weight": req.remaining_weight_g,
    "location": req.location,
  })
  .to_string();
  let spool_res = hyper_request_json(Method::PATCH, &spool_url, Some(spool_patch)).await;

  let payload = match (fil_res, spool_res) {
    (Ok((s1, _)), Ok((s2, _))) if s1.is_success() || s2.is_success() => SpoolmanPushResponse {
      ok: true,
      message: format!(
        "Synced calibrations (Flow {:.3}, PA {:.3}, MVS {:.1} mm³/s, {}°C/{}°C) & weight ({}g) to Spoolman Spool #{}!",
        req.flow_ratio,
        req.pressure_advance,
        req.max_volumetric_speed,
        req.extruder_temp_c,
        req.bed_temp_c,
        req.remaining_weight_g,
        req.spool_id
      ),
    },
    (Err(e), _) | (_, Err(e)) => SpoolmanPushResponse {
      ok: false,
      message: format!("Failed pushing to Spoolman: {}", e),
    },
    (Ok((s1, b1)), Ok((s2, _))) => SpoolmanPushResponse {
      ok: false,
      message: format!(
        "Spoolman returned HTTP {}/{} when updating Spool #{}: {}",
        s1,
        s2,
        req.spool_id,
        b1.chars().take(140).collect::<String>()
      ),
    },
  };

  let body = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());
  let mut res = Response::new(Body::from(body));
  res.headers_mut().insert(
    header::CONTENT_TYPE,
    HeaderValue::from_static("application/json"),
  );
  with_cors(res)
}

async fn api_moonraker_active_spool(
  Json(req): Json<MoonrakerSetActiveSpoolRequest>,
) -> impl IntoResponse {
  let base_url = normalize_http_base_url(&req.moonraker_url, "http://192.168.1.100:7125");
  let target_url = format!("{}/server/spoolman/spool_id", base_url);
  let req_body = serde_json::json!({
    "spool_id": req.spool_id
  })
  .to_string();
  let payload = match hyper_request_json(Method::POST, &target_url, Some(req_body)).await {
    Ok((status, _)) if status.is_success() => MoonrakerSetActiveSpoolResponse {
      ok: true,
      message: match req.spool_id {
        Some(sid) => format!(
          "Set active Spoolman Spool #{} in Moonraker ({})!",
          sid, base_url
        ),
        None => format!("Cleared active Spoolman spool in Moonraker ({})!", base_url),
      },
    },
    Ok((status, body_text)) => MoonrakerSetActiveSpoolResponse {
      ok: false,
      message: format!(
        "Moonraker returned HTTP {} from {}: {}",
        status,
        target_url,
        body_text.chars().take(140).collect::<String>()
      ),
    },
    Err(err) => MoonrakerSetActiveSpoolResponse {
      ok: false,
      message: err,
    },
  };
  let body = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());
  let mut res = Response::new(Body::from(body));
  res.headers_mut().insert(
    header::CONTENT_TYPE,
    HeaderValue::from_static("application/json"),
  );
  with_cors(res)
}

fn server_base64_decode(input: &str) -> Option<Vec<u8>> {
  const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  let mut lut = [255u8; 256];
  for (i, &b) in TABLE.iter().enumerate() {
    lut[b as usize] = i as u8;
  }
  let clean: Vec<u8> = input
    .bytes()
    .filter(|b| !b.is_ascii_whitespace())
    .collect();
  if clean.is_empty() || clean.len() % 4 != 0 {
    return None;
  }
  let mut out = Vec::with_capacity((clean.len() / 4) * 3);
  for chunk in clean.chunks(4) {
    let a = lut[chunk[0] as usize];
    let b = lut[chunk[1] as usize];
    if a == 255 || b == 255 {
      return None;
    }
    let c = if chunk[2] == b'=' {
      0
    } else {
      let v = lut[chunk[2] as usize];
      if v == 255 {
        return None;
      }
      v
    };
    let d = if chunk[3] == b'=' {
      0
    } else {
      let v = lut[chunk[3] as usize];
      if v == 255 {
        return None;
      }
      v
    };
    out.push((a << 2) | (b >> 4));
    if chunk[2] != b'=' {
      out.push((b << 4) | (c >> 2));
    }
    if chunk[3] != b'=' {
      out.push((c << 6) | d);
    }
  }
  Some(out)
}

fn server_decode_anycubic_obfuscated_str(encoded: &str) -> Option<String> {
  let pass1_raw = server_base64_decode(encoded.trim())?;
  let pass1_shifted: Vec<u8> = pass1_raw.into_iter().map(|b| b.wrapping_sub(5)).collect();
  let pass1_str = std::str::from_utf8(&pass1_shifted).ok()?;
  let pass2_raw = server_base64_decode(pass1_str)?;
  let pass2_shifted: Vec<u8> = pass2_raw.into_iter().map(|b| b.wrapping_sub(5)).collect();
  let plain = String::from_utf8(pass2_shifted).ok()?;
  Some(plain.trim().trim_matches('"').trim_matches('\'').to_string())
}

fn server_infer_material(profile_name: &str) -> String {
  let up = profile_name.to_uppercase();
  for mat in [
    "PETG-CF", "PLA-CF", "ABS-CF", "PA-CF", "PET-CF", "PCTG", "PETG", "TPU", "ASA", "ABS", "PC",
    "PHA", "PVA", "PLA+", "PLA",
  ] {
    if up.contains(mat) {
      return mat.to_string();
    }
  }
  "PLA".to_string()
}

fn server_sniff_conf_text(conf_text: &str) -> Result<AnycubicSniffedPrinterState, String> {
  let cleaned: String = conf_text
    .lines()
    .filter(|l| !l.trim_start().starts_with('#'))
    .collect::<Vec<_>>()
    .join("\n");
  let val: serde_json::Value = serde_json::from_str(&cleaned)
    .map_err(|e| format!("Failed to parse AnycubicSlicerNext.conf: {}", e))?;

  let encoded_dev_id = val
    .get("anycubic_remote_printing")
    .and_then(|rp| rp.get("current_device_id"))
    .and_then(|v| v.as_str())
    .unwrap_or("");
  let device_id = server_decode_anycubic_obfuscated_str(encoded_dev_id).unwrap_or_default();

  let mut ams_filament_ids = String::new();
  if let Some(ams_obj) = val.get("ams_filament_ids").and_then(|v| v.as_object()) {
    for (k, v) in ams_obj {
      let clean_k = k.trim_matches('"');
      if (!device_id.is_empty() && clean_k == device_id) || ams_filament_ids.is_empty() {
        if let Some(s) = v.as_str() {
          ams_filament_ids = s.to_string();
        }
      }
    }
  }

  let mut cloud_session_detected = false;
  if let Some(cloud_obj) = val.get("anycubic_cloud").and_then(|v| v.as_object()) {
    for (_k, v) in cloud_obj {
      if let Some(s) = v.as_str() {
        if s.len() > 64 {
          cloud_session_detected = true;
        }
      }
    }
  }

  let printer_model = val
    .get("firstguide")
    .and_then(|fg| fg.get("model"))
    .and_then(|v| v.as_str())
    .unwrap_or("Anycubic Kobra S1")
    .to_string();

  let mut active_machine_preset = val
    .get("presets")
    .and_then(|p| p.get("machine"))
    .and_then(|v| v.as_str())
    .unwrap_or("")
    .to_string();

  let mut active_process_preset = String::new();
  let mut slot_names: Vec<String> = Vec::new();
  let mut slot_colors: Vec<String> = Vec::new();

  if let Some(arr) = val.get("anycubic_presets").and_then(|a| a.as_array()) {
    let mut best_priority = 0usize;
    for preset in arr {
      let mach = preset.get("machine").and_then(|m| m.as_str()).unwrap_or("");
      if mach.contains(".3mf)") {
        continue;
      }
      let mut f_names = Vec::new();
      for key in [
        "filament",
        "filament_01",
        "filament_02",
        "filament_03",
        "filament_04",
        "filament_05",
        "filament_06",
        "filament_07",
      ] {
        if let Some(fnm) = preset.get(key).and_then(|v| v.as_str()) {
          let trimmed = fnm.trim();
          if !trimmed.is_empty() && !trimmed.starts_with('(') {
            let clean_fnm = trimmed
              .split_once(".3mf)")
              .map(|(head, _)| head.rsplit_once('(').map(|(h, _)| h).unwrap_or(head))
              .unwrap_or(trimmed);
            f_names.push(clean_fnm.trim().to_string());
          }
        }
      }
      let oc_fil_count = f_names
        .iter()
        .filter(|n| n.starts_with("OC -") || n.starts_with("OC:"))
        .count();
      let is_exact_active = !active_machine_preset.is_empty() && mach == active_machine_preset;
      let priority =
        if oc_fil_count >= 2 && (mach.contains("Kobra S1") || mach.starts_with("OC")) {
          40 + oc_fil_count
        } else if is_exact_active && oc_fil_count > 0 {
          30 + oc_fil_count
        } else if is_exact_active {
          20
        } else if (mach.contains("Kobra S1") || mach.starts_with("OC")) && oc_fil_count > 0 {
          15 + oc_fil_count
        } else if mach.contains("Kobra S1") {
          10
        } else {
          0
        };
      if priority > best_priority && !f_names.is_empty() {
        best_priority = priority;
        if active_machine_preset.is_empty() {
          active_machine_preset = mach.to_string();
        }
        if let Some(proc_str) = preset.get("process").and_then(|p| p.as_str()) {
          active_process_preset = proc_str.to_string();
        }
        slot_names = f_names;
        if let Some(cols) = preset.get("filament_colors").and_then(|fc| fc.as_str()) {
          slot_colors = cols
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        }
      }
    }
  }

  let default_colors = [
    "#BC4033", "#658946", "#E10600", "#4B5563", "#8E9299", "#1A1A1A", "#8E9299", "#D4AF37",
  ];
  let mut slots = Vec::new();
  for (idx, prof_name) in slot_names.iter().enumerate() {
    let color_hex = slot_colors
      .get(idx)
      .cloned()
      .unwrap_or_else(|| default_colors.get(idx).copied().unwrap_or("#3B82F6").to_string());
    slots.push(AnycubicAceSlotInfo {
      slot_index: idx + 1,
      profile_name: prof_name.clone(),
      material_type: server_infer_material(prof_name),
      color_hex,
      status: "Sniffed (.conf)".to_string(),
    });
  }

  Ok(AnycubicSniffedPrinterState {
    discovery_tier: "Tier 1: Local AnycubicSlicerNext.conf".to_string(),
    device_id,
    printer_model,
    active_machine_preset,
    active_process_preset,
    ams_filament_ids,
    cloud_session_detected,
    mqtt_broker_url: "mqtts://mqtt-universe.anycubic.com:8883".to_string(),
    mqtt_connected_in_logs: false,
    lan_rinkhals_reachable: false,
    lan_endpoint: String::new(),
    dryer_status: "Idle / Not Queried".to_string(),
    slots,
  })
}

async fn api_anycubic_auto_discover(
  Json(req): Json<AnycubicAutoDiscoverRequest>,
) -> impl IntoResponse {
  let cfg = load_local_slicer_config();
  let mut sniffed = AnycubicSniffedPrinterState::default();
  let mut tier_messages: Vec<String> = Vec::new();

  // Tier 1: Sniff local AnycubicSlicerNext.conf and MainApp_*.log via capability-sandboxed Dir
  if let Ok(root_dir) = Dir::open_ambient_dir(&cfg.local_slicer_directory, ambient_authority()) {
    let conf_name = if root_dir.exists("AnycubicSlicerNext.conf") {
      "AnycubicSlicerNext.conf"
    } else if root_dir.exists("OrcaSlicer.conf") {
      "OrcaSlicer.conf"
    } else {
      ""
    };
    if !conf_name.is_empty() {
      if let Ok(conf_text) = root_dir.read_to_string(conf_name) {
        if let Ok(parsed) = server_sniff_conf_text(&conf_text) {
          sniffed = parsed;
          tier_messages.push(format!(
            "Tier 1 ({}): decoded device_id='{}', {} ACE Pro slot(s), ams_filament_ids='{}'",
            conf_name,
            sniffed.device_id,
            sniffed.slots.len(),
            sniffed.ams_filament_ids
          ));
        }
      }
    }
    // Check log/MainApp_*.log for mqtt-universe.anycubic.com:8883 connection status
    if let Ok(log_dir) = root_dir.open_dir("log") {
      if let Ok(entries) = log_dir.entries() {
        let mut log_names: Vec<String> = entries
          .filter_map(|e| e.ok())
          .map(|e| e.file_name().to_string_lossy().to_string())
          .filter(|n| n.starts_with("MainApp_") && n.ends_with(".log"))
          .collect();
        log_names.sort();
        for lname in log_names.iter().rev().take(3) {
          if let Ok(ltxt) = log_dir.read_to_string(lname) {
            if ltxt.contains("success on CONNECTED") {
              sniffed.mqtt_connected_in_logs = true;
              break;
            }
          }
        }
      }
    }
  }

  // Tier 2: Optional Local LAN Rinkhals / Moonraker (:7125) Auto-Probe
  let lan_trimmed = req.lan_printer_url.trim();
  if !lan_trimmed.is_empty() && lan_trimmed != "http://192.168.1.100:7125" {
    let base_lan = normalize_http_base_url(lan_trimmed, "http://192.168.1.100:7125");
    let probe_url = format!("{}/printer/objects/query?ace&extruder&heater_bed&print_stats", base_lan);
    if let Ok((status, body_text)) = hyper_request_json(Method::GET, &probe_url, None).await {
      if status.is_success() {
        sniffed.lan_rinkhals_reachable = true;
        sniffed.lan_endpoint = base_lan.clone();
        sniffed.discovery_tier =
          "Tier 1 + Tier 2: Local AnycubicSlicerNext.conf + Rinkhals LAN (:7125)".to_string();
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body_text) {
          if let Some(dryer) = v
            .pointer("/result/status/ace/dryer_status")
            .and_then(|d| d.as_str())
          {
            sniffed.dryer_status = dryer.to_string();
          } else {
            sniffed.dryer_status = "Connected via Rinkhals LAN (:7125)".to_string();
          }
        }
        for s in &mut sniffed.slots {
          s.status = "Synced (.conf + LAN :7125)".to_string();
        }
        tier_messages.push(format!("Tier 2 (Rinkhals LAN {}): reachable!", base_lan));
      } else {
        tier_messages.push(format!("Tier 2 (LAN {}): HTTP {}", base_lan, status));
      }
    } else {
      tier_messages.push(format!(
        "Tier 2 (LAN {}): unreachable, using Tier 1 local .conf",
        base_lan
      ));
    }
  }

  // Tier 3: Optional Anycubic Cloud token status
  if !req.cloud_token.trim().is_empty() {
    sniffed.cloud_session_detected = true;
    sniffed.discovery_tier = format!("{} + Tier 3: Anycubic Cloud Token", sniffed.discovery_tier);
    tier_messages.push("Tier 3 (Anycubic Cloud): bearer token configured".to_string());
  } else if sniffed.cloud_session_detected {
    tier_messages.push(format!(
      "Tier 3 (Anycubic Cloud): encrypted session detected in AnycubicSlicerNext.conf (MQTT log connected={})",
      sniffed.mqtt_connected_in_logs
    ));
  }

  let ok = !sniffed.slots.is_empty() || !sniffed.device_id.is_empty();
  let message = if tier_messages.is_empty() {
    format!(
      "No AnycubicSlicerNext.conf found in {}",
      cfg.local_slicer_directory
    )
  } else {
    tier_messages.join(" • ")
  };

  let payload = AnycubicAutoDiscoverResponse {
    ok,
    message,
    sniffed_state: sniffed,
  };
  let body = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());
  let mut res = Response::new(Body::from(body));
  res.headers_mut().insert(
    header::CONTENT_TYPE,
    HeaderValue::from_static("application/json"),
  );
  with_cors(res)
}

async fn api_anycubic_push_slots(
  Json(req): Json<AnycubicPushSlotsRequest>,
) -> impl IntoResponse {
  let cfg = load_local_slicer_config();
  let mut status_parts: Vec<String> = Vec::new();
  let mut ok = false;

  if let Ok(root_dir) = Dir::open_ambient_dir(&cfg.local_slicer_directory, ambient_authority()) {
    let conf_name = if root_dir.exists("AnycubicSlicerNext.conf") {
      "AnycubicSlicerNext.conf"
    } else if root_dir.exists("OrcaSlicer.conf") {
      "OrcaSlicer.conf"
    } else {
      ""
    };
    if !conf_name.is_empty() {
      if let Ok(conf_text) = root_dir.read_to_string(conf_name) {
        let _ = root_dir.write(format!("{}.bak", conf_name), conf_text.as_bytes());
        let mut checksum_line = String::new();
        let mut body_lines = Vec::new();
        for line in conf_text.lines() {
          if line.trim_start().starts_with('#') {
            checksum_line = line.to_string();
          } else {
            body_lines.push(line);
          }
        }
        let cleaned = body_lines.join("\n");
        if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&cleaned) {
          let colors_csv = req
            .slot_colors_hex
            .iter()
            .map(|c| {
              if c.starts_with('#') {
                c.to_uppercase()
              } else {
                format!("#{}", c.to_uppercase())
              }
            })
            .collect::<Vec<_>>()
            .join(",");
          let keys = [
            "filament",
            "filament_01",
            "filament_02",
            "filament_03",
            "filament_04",
            "filament_05",
            "filament_06",
            "filament_07",
          ];
          if let Some(arr) = val.get_mut("anycubic_presets").and_then(|a| a.as_array_mut()) {
            for preset in arr.iter_mut() {
              let mach = preset
                .get("machine")
                .and_then(|m| m.as_str())
                .unwrap_or("")
                .to_string();
              if mach.contains(".3mf)") {
                continue;
              }
              if mach.contains("Kobra S1") || mach.starts_with("OC") {
                if let Some(obj) = preset.as_object_mut() {
                  for (i, key) in keys.iter().enumerate() {
                    if let Some(p_name) = req.slot_profile_names.get(i) {
                      obj.insert((*key).to_string(), serde_json::Value::String(p_name.clone()));
                    } else {
                      obj.remove(*key);
                    }
                  }
                  if !colors_csv.is_empty() {
                    obj.insert(
                      "filament_colors".to_string(),
                      serde_json::Value::String(colors_csv.clone()),
                    );
                  }
                }
              }
            }
          }
          if let Ok(mut out) = serde_json::to_string_pretty(&val) {
            if !checksum_line.is_empty() {
              out.push('\n');
              out.push_str(&checksum_line);
            }
            out.push('\n');
            if root_dir.write(conf_name, out.as_bytes()).is_ok() {
              ok = true;
              status_parts.push(format!(
                "Updated {} ACE Pro slot(s) & RGB colors in {}",
                req.slot_profile_names.len(),
                conf_name
              ));
            }
          }
        }
      }
    }
  }

  let payload = AnycubicPushSlotsResponse {
    ok,
    message: if status_parts.is_empty() {
      "Failed to update AnycubicSlicerNext.conf".to_string()
    } else {
      status_parts.join(" • ")
    },
  };
  let body = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());
  let mut res = Response::new(Body::from(body));
  res.headers_mut().insert(
    header::CONTENT_TYPE,
    HeaderValue::from_static("application/json"),
  );
  with_cors(res)
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
    .route(
      "/api/spoolman/sync",
      post(api_spoolman_sync).options(cors_preflight),
    )
    .route(
      "/api/spoolman/push",
      post(api_spoolman_push).options(cors_preflight),
    )
    .route(
      "/api/moonraker/active-spool",
      post(api_moonraker_active_spool).options(cors_preflight),
    )
    .route(
      "/api/anycubic/auto-discover",
      post(api_anycubic_auto_discover).options(cors_preflight),
    )
    .route(
      "/api/anycubic/push-slots",
      post(api_anycubic_push_slots).options(cors_preflight),
    )
    .fallback(get(index))
}
