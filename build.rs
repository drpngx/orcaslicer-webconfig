use std::env;
use std::path::PathBuf;

fn main() {
  let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into()));
  let bazel_bin = manifest_dir.join("bazel-bin");

  for (env_key, filename) in [
    (
      "ORCASLICER_GENERATED_CATALOG",
      "orcaslicer_generated_catalog.json",
    ),
    ("IMPORTED_VENDOR_CATALOG", "imported_vendor_catalog.json"),
    ("COMPILED_DOCS_JSON", "compiled_docs.json"),
  ] {
    if env::var_os(env_key).is_none() {
      let candidate = bazel_bin.join(filename);
      println!("cargo:rustc-env={}={}", env_key, candidate.display());
    }
  }
}
