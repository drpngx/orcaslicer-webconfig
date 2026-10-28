fn main() {
  #[cfg(target_arch = "wasm32")]
  wasm_logger::init(wasm_logger::Config::new(log::Level::Trace));

  yew::Renderer::<orcaslicer_webconfig::App>::new().render();
}
