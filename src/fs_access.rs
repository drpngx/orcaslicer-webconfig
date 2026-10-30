use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, HtmlInputElement, Url};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScannedFileEntry {
  pub relative_path: String,
  pub filename: String,
  pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LocalFsScanPayload {
  pub configured_path: String,
  pub windows_equivalent_path: String,
  pub entries: Vec<ScannedFileEntry>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LocalFsScanProgress {
  pub running: bool,
  pub done: bool,
  pub phase: String,
  pub files_read: usize,
  pub total_files: usize,
  pub current_file: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LocalFsSaveFileEntry {
  pub relative_path: String,
  pub content: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LocalFsSaveRequest {
  pub files: Vec<LocalFsSaveFileEntry>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LocalFsSaveResponse {
  pub ok: bool,
  pub files_saved: usize,
  pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DetectedClientOs {
  Windows,
  MacOs,
  Linux,
}

impl DetectedClientOs {
  pub fn label(&self) -> &'static str {
    match self {
      DetectedClientOs::Windows => "Windows",
      DetectedClientOs::MacOs => "macOS",
      DetectedClientOs::Linux => "Linux / WSL",
    }
  }

  pub fn anycubic_slicer_path(&self) -> &'static str {
    match self {
      DetectedClientOs::Windows => r"C:\Users\drpng\AppData\Roaming\AnycubicSlicerNext",
      DetectedClientOs::MacOs => "~/Library/Application Support/AnycubicSlicerNext",
      DetectedClientOs::Linux => "/mnt/c/Users/drpng/AppData/Roaming/AnycubicSlicerNext",
    }
  }

  pub fn anycubic_path(&self) -> &'static str {
    self.anycubic_slicer_path()
  }

  pub fn anycubic_env_shortcut(&self) -> &'static str {
    r"%APPDATA%\AnycubicSlicerNext"
  }

  pub fn orca_slicer_path(&self) -> &'static str {
    match self {
      DetectedClientOs::Windows => r"%APPDATA%\OrcaSlicer",
      DetectedClientOs::MacOs => "~/Library/Application Support/OrcaSlicer",
      DetectedClientOs::Linux => "~/.config/OrcaSlicer",
    }
  }

  pub fn orca_path(&self) -> &'static str {
    self.orca_slicer_path()
  }
}

pub struct FsManager;

const WEBCONFIG_STORAGE_KEY: &str = "orcaslicer_webconfig_state_v1";

impl FsManager {
  /// Persist the entire application state to browser `localStorage` so it survives page reloads.
  pub fn save_state_to_storage(state: &crate::models::WebConfigStateSnapshot) {
    if let Some(window) = web_sys::window() {
      if let Ok(Some(storage)) = window.local_storage() {
        if let Ok(json_str) = serde_json::to_string(state) {
          let _ = storage.set_item(WEBCONFIG_STORAGE_KEY, &json_str);
        }
      }
    }
  }

  /// Load persisted application state from browser `localStorage`, if present.
  pub fn load_state_from_storage() -> Option<crate::models::WebConfigStateSnapshot> {
    let window = web_sys::window()?;
    let storage = window.local_storage().ok()??;
    let json_str = storage.get_item(WEBCONFIG_STORAGE_KEY).ok()??;
    let mut snap = serde_json::from_str::<crate::models::WebConfigStateSnapshot>(&json_str).ok()?;
    snap.sanitize();
    Some(snap)
  }

  /// Clear persisted application state from browser `localStorage` when clicking Reset All.
  pub fn clear_state_from_storage() {
    if let Some(window) = web_sys::window() {
      if let Ok(Some(storage)) = window.local_storage() {
        let _ = storage.remove_item(WEBCONFIG_STORAGE_KEY);
      }
    }
  }

  /// Returns true when running inside the Tauri v2 desktop container (`window.__ORCA_WEBCONFIG_TAURI__`
  /// or `window.__TAURI_INTERNALS__` / `window.__TAURI__`), and false in standard Website mode.
  pub fn is_tauri() -> bool {
    let Some(window) = web_sys::window() else {
      return false;
    };
    for key in [
      "__ORCA_WEBCONFIG_TAURI__",
      "__TAURI_INTERNALS__",
      "__TAURI__",
    ] {
      if let Ok(val) = js_sys::Reflect::get(&window, &JsValue::from_str(key)) {
        if !val.is_undefined() && !val.is_null() && val != JsValue::FALSE {
          return true;
        }
      }
    }
    false
  }

  /// Auto-switching API URL helper for standard Web Dev (`""` relative path)
  /// and Tauri v2 Desktop App (`window.__API_BASE__`, e.g. `"http://127.0.0.1:<ephemeral_port>"`).
  pub fn api_url(path: &str) -> String {
    let base = web_sys::window()
      .and_then(|w| js_sys::Reflect::get(&w, &JsValue::from_str("__API_BASE__")).ok())
      .and_then(|v| v.as_string())
      .unwrap_or_default();
    format!("{}{}", base.trim_end_matches('/'), path)
  }

  /// Asynchronously sleep for `ms` milliseconds using browser `setTimeout`
  pub async fn sleep_ms(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
      if let Some(window) = web_sys::window() {
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
      } else {
        let _ = resolve.call0(&JsValue::NULL);
      }
    });
    let _ = JsFuture::from(promise).await;
  }

  /// Detect the HTTP client OS from `window.navigator.userAgent`
  pub fn detect_client_os() -> DetectedClientOs {
    if let Some(window) = web_sys::window() {
      if let Ok(ua) = window.navigator().user_agent() {
        let ua_lower = ua.to_lowercase();
        if ua_lower.contains("win") {
          return DetectedClientOs::Windows;
        } else if ua_lower.contains("mac") {
          return DetectedClientOs::MacOs;
        }
      }
    }
    DetectedClientOs::Linux
  }

  /// Copy text to the system clipboard via `navigator.clipboard.writeText`
  pub fn copy_to_clipboard(text: &str) {
    if let Some(window) = web_sys::window() {
      let nav = window.navigator();
      if let Ok(clip) = js_sys::Reflect::get(&nav, &"clipboard".into()) {
        if !clip.is_undefined() {
          if let Ok(write_fn_val) = js_sys::Reflect::get(&clip, &"writeText".into()) {
            if let Ok(write_fn) = write_fn_val.dyn_into::<js_sys::Function>() {
              let _ = write_fn.call1(&clip, &JsValue::from_str(text));
            }
          }
        }
      }
    }
  }

  /// Read raw binary bytes from a standard `<input type="file">` element (e.g., for `.zip` backup import)
  pub async fn read_input_file_bytes(
    input: &HtmlInputElement,
  ) -> Result<(String, Vec<u8>), String> {
    let files = input
      .files()
      .ok_or_else(|| "No files selected".to_string())?;
    let file = files
      .get(0)
      .ok_or_else(|| "Selected file list is empty".to_string())?;
    let name = file.name();

    let buf_promise = file.array_buffer();
    let buf_val = JsFuture::from(buf_promise)
      .await
      .map_err(|e| format!("Failed to read file arrayBuffer: {:?}", e))?;

    let uint8_arr = js_sys::Uint8Array::new(&buf_val);
    let bytes = uint8_arr.to_vec();
    input.set_value("");
    Ok((name, bytes))
  }

  /// Trigger a browser download for a text/JSON file
  pub fn trigger_browser_download(filename: &str, content: &str) -> Result<(), String> {
    if let Some(window) = web_sys::window() {
      if let Some(document) = window.document() {
        let parts = js_sys::Array::new();
        parts.push(&JsValue::from_str(content));

        let bag = BlobPropertyBag::new();
        bag.set_type("application/json");

        let blob = Blob::new_with_str_sequence_and_options(&parts, &bag)
          .map_err(|e| format!("Failed to create blob: {:?}", e))?;

        let url = Url::create_object_url_with_blob(&blob)
          .map_err(|e| format!("Failed to create blob URL: {:?}", e))?;

        let anchor_el = document
          .create_element("a")
          .map_err(|e| format!("Failed to create anchor: {:?}", e))?;
        let anchor: HtmlAnchorElement = anchor_el.unchecked_into();
        anchor.set_href(&url);
        anchor.set_download(filename);
        anchor.click();
        let _ = Url::revoke_object_url(&url);
        return Ok(());
      }
    }
    Err("Document not available for download".to_string())
  }

  /// Trigger a browser download for binary bytes (such as a `.zip` backup or `.3mf` container)
  pub fn trigger_binary_download(
    filename: &str,
    bytes: &[u8],
    mime_type: &str,
  ) -> Result<(), String> {
    if let Some(window) = web_sys::window() {
      if let Some(document) = window.document() {
        let uint8_arr = js_sys::Uint8Array::from(bytes);
        let parts = js_sys::Array::new();
        parts.push(&uint8_arr);

        let bag = BlobPropertyBag::new();
        bag.set_type(mime_type);

        let blob = Blob::new_with_u8_array_sequence_and_options(&parts, &bag)
          .map_err(|e| format!("Failed to create binary blob: {:?}", e))?;

        let url = Url::create_object_url_with_blob(&blob)
          .map_err(|e| format!("Failed to create blob URL: {:?}", e))?;

        let anchor_el = document
          .create_element("a")
          .map_err(|e| format!("Failed to create anchor: {:?}", e))?;
        let anchor: HtmlAnchorElement = anchor_el.unchecked_into();
        anchor.set_href(&url);
        anchor.set_download(filename);
        anchor.click();
        let _ = Url::revoke_object_url(&url);
        return Ok(());
      }
    }
    Err("Document not available for binary download".to_string())
  }
}
