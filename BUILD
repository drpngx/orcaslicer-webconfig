load("@bazel_skylib//rules:common_settings.bzl", "bool_flag")
load("@crate_index//:defs.bzl", "aliases", "all_crate_deps")
load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_library", "rust_test")
load("@rules_rust_wasm_bindgen//:defs.bzl", "rust_wasm_bindgen")

package(
    default_visibility = ["//:__subpackages__"],
)

config_setting(
    name = "debug",
    values = {
        "compilation_mode": "dbg",
    },
)

config_setting(
    name = "fastbuild",
    values = {
        "compilation_mode": "fastbuild",
    },
)

config_setting(
    name = "opt",
    values = {
        "compilation_mode": "opt",
    },
)

rust_binary(
    name = "app",
    srcs = ["src/bin/app.rs"],
    aliases = aliases(),
    edition = "2021",
    proc_macro_deps = all_crate_deps(
        proc_macro = True,
    ),
    rustc_flags = select({
        ":debug": [
            "-Copt-level=0",
        ],
        ":fastbuild": [],
        "//conditions:default": [
            "-Ccodegen-units=1",
            "-Cpanic=abort",
            "-Copt-level=z",
        ],
    }),
    deps = all_crate_deps(
        normal = True,
    ) + [
        ":orcaslicer_webconfig",
    ],
)

rust_library(
    name = "orcaslicer_webconfig",
    srcs = glob(
        include = [
            "src/**/*.rs",
        ],
        exclude = [
            "src/bin/**",
        ],
    ),
    aliases = aliases(),
    edition = "2021",
    proc_macro_deps = all_crate_deps(
        proc_macro = True,
    ),
    deps = all_crate_deps(
        normal = True,
    ),
)

rust_test(
    name = "orcaslicer_webconfig_test",
    crate = ":orcaslicer_webconfig",
    proc_macro_deps = all_crate_deps(
        proc_macro = True,
    ),
    deps = all_crate_deps(
        normal = True,
    ),
)

rust_wasm_bindgen(
    name = "app_wasm",
    target = "web",
    wasm_file = ":app",
)

filegroup(
    name = "static_files",
    srcs = glob(["static/**"]),
    visibility = ["//visibility:public"],
)
