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

rust_binary(
    name = "generate_orca_catalog",
    srcs = ["src/bin/generate_orca_catalog.rs"],
    aliases = aliases(),
    edition = "2021",
    proc_macro_deps = all_crate_deps(
        proc_macro = True,
    ),
    deps = all_crate_deps(
        normal = True,
    ),
)

genrule(
    name = "orcaslicer_catalog_gen",
    srcs = [
        "@orcaslicer_repo//:profiles",
        "@orcaslicer_repo//:resources/profiles/OrcaFilamentLibrary.json",
    ],
    outs = ["orcaslicer_generated_catalog.json"],
    cmd = "$(location :generate_orca_catalog) $@ $$(dirname $(location @orcaslicer_repo//:resources/profiles/OrcaFilamentLibrary.json))",
    tools = [":generate_orca_catalog"],
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
    compile_data = glob(["transformations/**/*.lua"]) + [
        "additional_catalog.json",
        ":orcaslicer_generated_catalog.json",
    ],
    data = [
        ":orcaslicer_generated_catalog.json",
    ],
    edition = "2021",
    proc_macro_deps = all_crate_deps(
        proc_macro = True,
    ),
    rustc_env = {
        "ORCASLICER_GENERATED_CATALOG": "$(execpath :orcaslicer_generated_catalog.json)",
    },
    deps = all_crate_deps(
        normal = True,
    ),
)

rust_test(
    name = "orcaslicer_webconfig_test",
    compile_data = glob(["transformations/**/*.lua"]) + [
        "additional_catalog.json",
        ":orcaslicer_generated_catalog.json",
    ],
    crate = ":orcaslicer_webconfig",
    data = [
        ":orcaslicer_generated_catalog.json",
    ],
    proc_macro_deps = all_crate_deps(
        proc_macro = True,
    ),
    rustc_env = {
        "ORCASLICER_GENERATED_CATALOG": "$(execpath :orcaslicer_generated_catalog.json)",
    },
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
    srcs = glob(["static/**"]) + ["local_slicer_config.json"],
    visibility = ["//visibility:public"],
)

exports_files([
    "local_slicer_config.json",
    "static/index.html",
])
