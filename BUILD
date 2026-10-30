load("@bazel_skylib//rules:common_settings.bzl", "bool_flag")
load("@crate_index//:defs.bzl", "aliases", "all_crate_deps")
load("@rules_python//python:pip.bzl", "compile_pip_requirements")
load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_library", "rust_test")
load("@rules_rust_wasm_bindgen//:defs.bzl", "rust_wasm_bindgen")
load("//:version.bzl", "WEBCONFIG_VERSION")
load("//emsdk:emsdk.bzl", "wasmopt")

package(
    default_visibility = ["//:__subpackages__"],
)

compile_pip_requirements(
    name = "requirements",
    requirements_in = "requirements.txt",
    requirements_txt = "requirements_lock.txt",
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
    version = WEBCONFIG_VERSION,
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
    version = WEBCONFIG_VERSION,
    proc_macro_deps = all_crate_deps(
        proc_macro = True,
    ),
    deps = all_crate_deps(
        normal = True,
    ),
)

rust_binary(
    name = "compile_docs",
    srcs = ["src/bin/compile_docs.rs"],
    aliases = aliases(),
    edition = "2021",
    version = WEBCONFIG_VERSION,
    proc_macro_deps = all_crate_deps(
        proc_macro = True,
    ),
    deps = all_crate_deps(
        normal = True,
    ),
)

genrule(
    name = "compiled_docs_gen",
    srcs = glob(["doc/*.md"]),
    outs = ["compiled_docs.json"],
    cmd = "$(location :compile_docs) $@ $(SRCS)",
    tools = [":compile_docs"],
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

genrule(
    name = "vendor_catalog_gen",
    srcs = glob(
        include = [
            "vendor_profiles/siraya_tech/**",
            "vendor_profiles/sunlu/**",
            "vendor_profiles/polymaker/**",
        ],
    ),
    outs = ["imported_vendor_catalog.json"],
    cmd = """
        MANIFEST_FILE="$(location vendor_profiles/siraya_tech/manifest.json)"
        VENDOR_DIR="$$(dirname "$$(dirname "$$MANIFEST_FILE")")"
        $(location //scripts:import_vendor_profiles) --vendor-dir "$$VENDOR_DIR" --output $@
    """,
    tools = ["//scripts:import_vendor_profiles"],
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
        ":compiled_docs.json",
        ":imported_vendor_catalog.json",
        ":orcaslicer_generated_catalog.json",
    ],
    data = [
        ":compiled_docs.json",
        ":imported_vendor_catalog.json",
        ":orcaslicer_generated_catalog.json",
    ],
    edition = "2021",
    version = WEBCONFIG_VERSION,
    proc_macro_deps = all_crate_deps(
        proc_macro = True,
    ),
    rustc_env = {
        "COMPILED_DOCS_JSON": "$(execpath :compiled_docs.json)",
        "IMPORTED_VENDOR_CATALOG": "$(execpath :imported_vendor_catalog.json)",
        "ORCASLICER_GENERATED_CATALOG": "$(execpath :orcaslicer_generated_catalog.json)",
    },
    deps = all_crate_deps(
        normal = True,
    ),
)

rust_test(
    name = "orcaslicer_webconfig_test",
    compile_data = glob(["transformations/**/*.lua"]) + [
        "Cargo.toml",
        "additional_catalog.json",
        "src-tauri/Cargo.toml",
        "src-tauri/tauri.conf.json",
        ":compiled_docs.json",
        ":imported_vendor_catalog.json",
        ":orcaslicer_generated_catalog.json",
        "//server:Cargo.toml",
    ],
    crate = ":orcaslicer_webconfig",
    data = [
        ":compiled_docs.json",
        ":imported_vendor_catalog.json",
        ":orcaslicer_generated_catalog.json",
    ],
    proc_macro_deps = all_crate_deps(
        proc_macro = True,
    ),
    rustc_env = {
        "COMPILED_DOCS_JSON": "$(execpath :compiled_docs.json)",
        "IMPORTED_VENDOR_CATALOG": "$(execpath :imported_vendor_catalog.json)",
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

wasmopt(
    name = "app_wasm_opt",
    src = ":app_wasm",
    out = "app_wasm/app_wasm_bg_opt.wasm",
)

genrule(
    name = "app_wasm_opt_br",
    srcs = [":app_wasm_opt"],
    outs = ["app_wasm/app_wasm_bg_opt.wasm.br"],
    cmd = "$(execpath @brotli) -9 $< -o $@",
    tools = ["@brotli"],
)

filegroup(
    name = "static_files",
    srcs = glob([
        "static/**",
        "doc/**",
    ]) + ["local_slicer_config.json"],
    visibility = ["//visibility:public"],
)

exports_files([
    "local_slicer_config.json",
    "static/index.html",
])
