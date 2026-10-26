use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, Url};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = showOpenFilePicker)]
    pub fn show_open_file_picker(options: &JsValue) -> js_sys::Promise;

    #[wasm_bindgen(method, js_name = getFile)]
    pub fn get_file(this: &FileSystemFileHandle) -> js_sys::Promise;

    #[wasm_bindgen(method, js_name = createWritable)]
    pub fn create_writable(this: &FileSystemFileHandle) -> js_sys::Promise;

    #[derive(Clone, PartialEq)]
    #[wasm_bindgen]
    pub type FileSystemFileHandle;

    #[wasm_bindgen(method, getter)]
    pub fn name(this: &FileSystemFileHandle) -> String;

    #[wasm_bindgen(method)]
    pub fn write(this: &FileSystemWritableFileStream, data: &JsValue) -> js_sys::Promise;

    #[wasm_bindgen(method)]
    pub fn close(this: &FileSystemWritableFileStream) -> js_sys::Promise;

    #[derive(Clone, PartialEq)]
    #[wasm_bindgen]
    pub type FileSystemWritableFileStream;
}

#[derive(Clone, Debug, PartialEq)]
pub enum FileBindingStatus {
    Unbound,
    BoundDirectly {
        filename: String,
        path_hint: String,
        last_saved: String,
    },
    FallbackLoaded {
        filename: String,
    },
}

pub struct FsManager;

impl FsManager {
    /// Check if the File System Access API is supported by the current browser
    pub fn is_fs_access_supported() -> bool {
        if let Some(window) = web_sys::window() {
            let val = js_sys::Reflect::get(&window, &"showOpenFilePicker".into()).unwrap_or(JsValue::UNDEFINED);
            !val.is_undefined()
        } else {
            false
        }
    }

    /// Read file content from a FileSystemFileHandle
    pub async fn read_file(handle: &FileSystemFileHandle) -> Result<(String, String), String> {
        let file_promise = handle.get_file();
        let file_val = JsFuture::from(file_promise)
            .await
            .map_err(|e| format!("Failed to get file from handle: {:?}", e))?;
        
        let file: web_sys::File = file_val.unchecked_into();
        let name = file.name();

        let text_promise = file.text();
        let text_val = JsFuture::from(text_promise)
            .await
            .map_err(|e| format!("Failed to read file text: {:?}", e))?;

        let content = text_val.as_string().unwrap_or_default();
        Ok((name, content))
    }

    /// Write content back to a FileSystemFileHandle, closing the stream to avoid locking
    pub async fn write_file(handle: &FileSystemFileHandle, content: &str) -> Result<(), String> {
        let writable_promise = handle.create_writable();
        let writable_val = JsFuture::from(writable_promise)
            .await
            .map_err(|e| format!("Failed to create writable stream: {:?}", e))?;

        let writable: FileSystemWritableFileStream = writable_val.unchecked_into();

        let content_js = JsValue::from_str(content);
        let write_promise = writable.write(&content_js);
        JsFuture::from(write_promise)
            .await
            .map_err(|e| format!("Failed to write content: {:?}", e))?;

        let close_promise = writable.close();
        JsFuture::from(close_promise)
            .await
            .map_err(|e| format!("Failed to close stream: {:?}", e))?;

        Ok(())
    }

    /// Fallback file download for browsers or when disconnected
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
}
