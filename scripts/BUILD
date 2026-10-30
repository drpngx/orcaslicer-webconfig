load("@rules_python//python:defs.bzl", "py_binary")

package(
    default_visibility = ["//visibility:public"],
)

py_binary(
    name = "download_vendor_profiles",
    srcs = ["download_vendor_profiles.py"],
)

py_binary(
    name = "import_vendor_profiles",
    srcs = ["import_vendor_profiles.py"],
)

py_binary(
    name = "capture_doc_screenshots",
    srcs = ["capture_doc_screenshots.py"],
)

py_binary(
    name = "build_tauri_portable",
    srcs = ["build_tauri_portable.py"],
    data = [
        "//:app_wasm",
        "//:app_wasm_opt",
        "//:app_wasm_opt_br",
        "//:compiled_docs.json",
        "//:imported_vendor_catalog.json",
        "//:orcaslicer_generated_catalog.json",
        "//:static_files",
    ],
)
