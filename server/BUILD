load("@crate_index//:defs.bzl", "aliases", "all_crate_deps")
load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_library")

rust_library(
    name = "server_lib",
    srcs = ["src/lib.rs"],
    aliases = aliases(),
    compile_data = [
        "//:local_slicer_config.json",
        "//:static/index.html",
    ],
    crate_name = "server",
    edition = "2021",
    proc_macro_deps = all_crate_deps(
        proc_macro = True,
    ),
    visibility = ["//visibility:public"],
    deps = all_crate_deps(
        normal = True,
    ) + ["//:orcaslicer_webconfig"],
)

rust_binary(
    name = "server",
    srcs = ["src/main.rs"],
    aliases = aliases(),
    compile_data = [
        "//:local_slicer_config.json",
        "//:static/index.html",
    ],
    data = [
        "//:app_wasm",
        "//:local_slicer_config.json",
        "//:static_files",
    ],
    edition = "2021",
    proc_macro_deps = all_crate_deps(
        proc_macro = True,
    ),
    deps = all_crate_deps(
        normal = True,
    ) + [
        ":server_lib",
        "//:orcaslicer_webconfig",
    ],
)
