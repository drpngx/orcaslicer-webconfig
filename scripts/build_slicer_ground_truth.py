#!/usr/bin/env python3
"""Builds ground-truth OrcaSlicer C++ + AnycubicSlicerNext schema and resolved parent presets."""

import json
import os
import re
import sys
import zipfile


def strip_cpp_comments(src: str) -> str:
    src = re.sub(r"/\*.*?\*/", "", src, flags=re.S)
    src = re.sub(r"//[^\n]*", "", src)
    return src


def extract_brace_strings(src: str, var_name: str) -> set[str]:
    m = re.search(re.escape(var_name) + r"\s*(?:=\s*)?\{([^}]*)\}", src, re.S)
    if not m:
        return set()
    body = strip_cpp_comments(m.group(1))
    return set(re.findall(r'"([^"]+)"', body))


def parse_cpp_schema(print_config_cpp_path: str, preset_cpp_path: str) -> dict:
    pc = open(print_config_cpp_path, encoding="utf-8", errors="ignore").read()
    pr = open(preset_cpp_path, encoding="utf-8", errors="ignore").read()

    # 1. Static enum value maps: s_keys_map_<EnumName> = { {"val", ...}, ... };
    enum_type_maps: dict[str, list[str]] = {}
    for m in re.finditer(
        r"s_keys_map_([A-Za-z0-9_]+)\s*=\s*\{(.*?)\};", pc, flags=re.S
    ):
        ename = m.group(1)
        body = strip_cpp_comments(m.group(2))
        vals = re.findall(r'\{\s*"([^"]*)"', body)
        if vals:
            enum_type_maps[ename] = vals

    # 2. Parse every `this->add("key", coType)` block
    vector_types = {
        "coBools",
        "coEnums",
        "coFloats",
        "coFloatsOrPercents",
        "coInts",
        "coPercents",
        "coPoints",
        "coPointsGroups",
        "coStrings",
    }
    options: dict[str, dict] = {}
    blocks = pc.split("this->add(")[1:]
    for b in blocks:
        m = re.match(r'\s*"([^"]+)"\s*,\s*(co[A-Za-z0-9_]+)\s*\)', b)
        if not m:
            continue
        key, co_type = m.group(1), m.group(2)
        nullable = "def->nullable = true" in b
        # Tooltip extraction (handles multi-line concatenated C++ string literals)
        tooltip = ""
        tm = re.search(
            r"def->tooltip\s*=\s*L\(((?:\s*\"(?:\\.|[^\"\\])*\"\s*)+)\)", b
        )
        if tm:
            lits = re.findall(r'"((?:\\.|[^"\\])*)"', tm.group(1))
            tooltip = (
                "".join(lits)
                .replace("\\n", " ")
                .replace('\\"', '"')
                .strip()
            )
            tooltip = re.sub(r"\s+", " ", tooltip)

        enum_vals: list[str] = []
        if co_type in ("coEnum", "coEnums"):
            pushes = re.findall(r'enum_values\.push_back\(\s*"([^"]*)"\s*\)', b)
            if pushes:
                enum_vals = pushes
            else:
                em = re.search(
                    r"ConfigOptionEnum<([A-Za-z0-9_]+)>::get_enum_values", b
                )
                if em and em.group(1) in enum_type_maps:
                    enum_vals = list(enum_type_maps[em.group(1)])

        options[key] = {
            "co_type": co_type,
            "is_vector": co_type in vector_types,
            "nullable": nullable,
            "enum_values": enum_vals,
            "tooltip": tooltip,
        }

    # 3. Preset scope sets from Preset.cpp + PrintConfig.cpp extruder keys
    print_keys = extract_brace_strings(pr, "s_Preset_print_options")
    filament_keys = extract_brace_strings(pr, "s_Preset_filament_options")
    machine_keys = (
        extract_brace_strings(pr, "s_Preset_printer_options")
        | extract_brace_strings(pr, "s_Preset_machine_limits_options")
        | extract_brace_strings(pc, "m_extruder_option_keys")
    )
    return {
        "options": options,
        "print_keys": print_keys,
        "filament_keys": filament_keys,
        "machine_keys": machine_keys,
    }


def resolve_preset_map(by_name: dict[str, dict]) -> dict[str, dict]:
    cache: dict[str, dict] = {}

    def resolve_one(name: str, seen: set[str]) -> dict:
        if name in cache:
            return cache[name]
        raw = by_name.get(name)
        if not raw or name in seen:
            return dict(raw) if raw else {}
        inh = raw.get("inherits")
        merged: dict = {}
        if isinstance(inh, str) and inh and inh in by_name:
            merged.update(resolve_one(inh, seen | {name}))
        merged.update(raw)
        cache[name] = merged
        return merged

    for n in by_name:
        resolve_one(n, set())
    return cache


def main() -> int:
    if len(sys.argv) != 6:
        print(
            "Usage: build_slicer_ground_truth.py <out.json> <PrintConfig.cpp> <Preset.cpp> <orca_profiles_dir> <anycubic_profiles.zip>",
            file=sys.stderr,
        )
        return 1

    out_path, pc_path, pr_path, orca_profiles_dir, ac_zip_path = sys.argv[1:]
    cpp = parse_cpp_schema(pc_path, pr_path)
    options = cpp["options"]
    print_keys: set[str] = set(cpp["print_keys"]) | {
        "type",
        "name",
        "from",
        "inherits",
        "version",
        "is_custom_defined",
        "instantiation",
        "setting_id",
        "print_settings_id",
        "compatible_printers",
        "compatible_printers_condition",
        "print_timestamp",
    }
    filament_keys: set[str] = set(cpp["filament_keys"]) | {
        "type",
        "name",
        "from",
        "inherits",
        "version",
        "is_custom_defined",
        "instantiation",
        "setting_id",
        "filament_settings_id",
        "filament_id",
        "filament_vendor",
        "compatible_printers",
        "compatible_printers_condition",
        "compatible_prints",
        "compatible_prints_condition",
        "filament_timestamp",
    }
    machine_keys: set[str] = set(cpp["machine_keys"]) | {
        "type",
        "name",
        "from",
        "inherits",
        "version",
        "is_custom_defined",
        "instantiation",
        "setting_id",
        "printer_settings_id",
        "printer_model",
        "printer_variant",
        "default_print_profile",
        "default_filament_profile",
        "printer_timestamp",
        "upward_compatible_machine",
    }

    def record_vendor_preset_keys(d: dict) -> None:
        ptype = d.get("type", "")
        target_set = None
        if ptype == "process":
            target_set = print_keys
        elif ptype == "filament":
            target_set = filament_keys
        elif ptype == "machine":
            target_set = machine_keys
        if target_set is None:
            return
        for k, v in d.items():
            target_set.add(k)
            # Fork-specific sibling keys like `nozzle_temperature_BRASS` / `_HS`
            for suf in ("_BRASS", "_HS"):
                if k.endswith(suf) and k not in options:
                    base_k = k[: -len(suf)]
                    if base_k in options:
                        options[k] = dict(options[base_k])
            if k in options and options[k]["enum_values"]:
                evs = set(options[k]["enum_values"])
                cand_vals = (
                    [v]
                    if isinstance(v, str)
                    else ([x for x in v if isinstance(x, str)] if isinstance(v, list) else [])
                )
                for cv in cand_vals:
                    if cv and cv != "nil" and cv not in evs:
                        options[k]["enum_values"].append(cv)
                        evs.add(cv)

    # Load OrcaFilamentLibrary shared base
    lib_by_type: dict[str, dict[str, dict]] = {
        "machine": {},
        "process": {},
        "filament": {},
    }
    lib_dir = os.path.join(orca_profiles_dir, "OrcaFilamentLibrary")
    if os.path.isdir(lib_dir):
        for root, _, files in os.walk(lib_dir):
            for fn in sorted(files):
                if not fn.endswith(".json"):
                    continue
                try:
                    d = json.load(open(os.path.join(root, fn), encoding="utf-8-sig"))
                except Exception:
                    continue
                if isinstance(d, dict) and d.get("type") in lib_by_type and "name" in d:
                    record_vendor_preset_keys(d)
                    lib_by_type[d["type"]][d["name"]] = d

    resolved_presets: dict[str, dict[str, dict]] = {
        "machine": {},
        "process": {},
        "filament": {},
    }

    # Resolve non-Anycubic vendors from @orcaslicer_repo
    for vd in sorted(os.listdir(orca_profiles_dir)):
        vpath = os.path.join(orca_profiles_dir, vd)
        if not os.path.isdir(vpath) or vd in ("OrcaFilamentLibrary", "Anycubic"):
            continue
        for ptype in ("machine", "process", "filament"):
            sub = os.path.join(vpath, ptype)
            if not os.path.isdir(sub):
                continue
            vmap = dict(lib_by_type[ptype])
            for root, _, files in os.walk(sub):
                for fn in sorted(files):
                    if not fn.endswith(".json"):
                        continue
                    try:
                        d = json.load(open(os.path.join(root, fn), encoding="utf-8-sig"))
                    except Exception:
                        continue
                    if isinstance(d, dict) and d.get("type") == ptype and "name" in d:
                        record_vendor_preset_keys(d)
                        vmap[d["name"]] = d
            res = resolve_preset_map(vmap)
            for pname, pdict in res.items():
                if pdict.get("instantiation") == "true" or pname.endswith("@System"):
                    resolved_presets[ptype][pname] = pdict

    upstream_all_keys = set(print_keys) | set(filament_keys) | set(machine_keys)

    # Resolve Anycubic from AnycubicSlicerNext zip archive (authoritative override for all Anycubic models)
    ac_by_type: dict[str, dict[str, dict]] = {
        "machine": dict(lib_by_type["machine"]),
        "process": dict(lib_by_type["process"]),
        "filament": dict(lib_by_type["filament"]),
    }
    with zipfile.ZipFile(ac_zip_path, "r") as zf:
        for zname in sorted(zf.namelist()):
            if not zname.endswith(".json"):
                continue
            try:
                d = json.loads(zf.read(zname).decode("utf-8-sig"))
            except Exception:
                continue
            if isinstance(d, dict) and d.get("type") in ac_by_type and "name" in d:
                record_vendor_preset_keys(d)
                ac_by_type[d["type"]][d["name"]] = d

    for ptype in ("machine", "process", "filament"):
        res = resolve_preset_map(ac_by_type[ptype])
        for pname, pdict in res.items():
            if pdict.get("instantiation") == "true" or pname in ac_by_type[ptype]:
                resolved_presets[ptype][pname] = pdict

    # Also keep resolved OrcaFilamentLibrary `@System` presets
    for ptype in ("machine", "process", "filament"):
        res = resolve_preset_map(lib_by_type[ptype])
        for pname, pdict in res.items():
            if pdict.get("instantiation") == "true" or pname.endswith("@System"):
                resolved_presets[ptype].setdefault(pname, pdict)

    anycubic_fork_only_keys = sorted(
        (print_keys | filament_keys | machine_keys) - upstream_all_keys
    )

    out_obj = {
        "print_keys": sorted(print_keys),
        "filament_keys": sorted(filament_keys),
        "machine_keys": sorted(machine_keys),
        "anycubic_fork_only_keys": anycubic_fork_only_keys,
        "options": options,
        "resolved_presets": resolved_presets,
    }
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(out_obj, f, sort_keys=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
