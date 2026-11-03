use crate::models::{FilamentSpool, MachineRig, ProcessProfile, WEBCONFIG_ORCA_PRESET_VERSION};
use serde::{Deserialize, Serialize};

/// Compute standard IEEE 802.3 CRC32 for ZIP archive entries
pub fn crc32_ieee(data: &[u8]) -> u32 {
  let mut crc: u32 = 0xFFFF_FFFF;
  for &byte in data {
    crc ^= byte as u32;
    for _ in 0..8 {
      if (crc & 1) != 0 {
        crc = (crc >> 1) ^ 0xEDB8_8320;
      } else {
        crc >>= 1;
      }
    }
  }
  !crc
}

/// Build an uncompressed (STORE method = 0) ZIP archive in memory from `(path, bytes)` entries.
pub fn build_zip_archive(files: &[(&str, &[u8])]) -> Vec<u8> {
  let mut out = Vec::new();
  let mut central_dir = Vec::new();

  for &(name, data) in files {
    let name_bytes = name.as_bytes();
    let crc = crc32_ieee(data);
    let size = data.len() as u32;
    let local_header_offset = out.len() as u32;

    // Local File Header (30 bytes + filename)
    // Bit 11 (0x0800) = Language encoding flag (EFS): filename and comment are UTF-8 encoded
    let utf8_flag: u16 = 0x0800;
    out.extend_from_slice(&0x0403_4B50u32.to_le_bytes()); // signature
    out.extend_from_slice(&20u16.to_le_bytes()); // version needed (2.0)
    out.extend_from_slice(&utf8_flag.to_le_bytes()); // general purpose bit flag (UTF-8)
    out.extend_from_slice(&0u16.to_le_bytes()); // compression method: 0 = STORE
    out.extend_from_slice(&0u16.to_le_bytes()); // last mod file time
    out.extend_from_slice(&0u16.to_le_bytes()); // last mod file date
    out.extend_from_slice(&crc.to_le_bytes()); // crc-32
    out.extend_from_slice(&size.to_le_bytes()); // compressed size
    out.extend_from_slice(&size.to_le_bytes()); // uncompressed size
    out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes()); // file name length
    out.extend_from_slice(&0u16.to_le_bytes()); // extra field length
    out.extend_from_slice(name_bytes);
    out.extend_from_slice(data);

    // Central Directory File Header (46 bytes + filename)
    central_dir.extend_from_slice(&0x0201_4B50u32.to_le_bytes()); // signature
    central_dir.extend_from_slice(&20u16.to_le_bytes()); // version made by
    central_dir.extend_from_slice(&20u16.to_le_bytes()); // version needed
    central_dir.extend_from_slice(&utf8_flag.to_le_bytes()); // flags (UTF-8)
    central_dir.extend_from_slice(&0u16.to_le_bytes()); // compression method: 0 = STORE
    central_dir.extend_from_slice(&0u16.to_le_bytes()); // mod time
    central_dir.extend_from_slice(&0u16.to_le_bytes()); // mod date
    central_dir.extend_from_slice(&crc.to_le_bytes()); // crc-32
    central_dir.extend_from_slice(&size.to_le_bytes()); // compressed size
    central_dir.extend_from_slice(&size.to_le_bytes()); // uncompressed size
    central_dir.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes()); // filename len
    central_dir.extend_from_slice(&0u16.to_le_bytes()); // extra len
    central_dir.extend_from_slice(&0u16.to_le_bytes()); // comment len
    central_dir.extend_from_slice(&0u16.to_le_bytes()); // disk number start
    central_dir.extend_from_slice(&0u16.to_le_bytes()); // internal file attrs
    central_dir.extend_from_slice(&0u32.to_le_bytes()); // external file attrs
    central_dir.extend_from_slice(&local_header_offset.to_le_bytes()); // local header offset
    central_dir.extend_from_slice(name_bytes);
  }

  let cd_offset = out.len() as u32;
  let cd_size = central_dir.len() as u32;
  let num_entries = files.len() as u16;

  out.extend_from_slice(&central_dir);

  // End of Central Directory Record (22 bytes)
  out.extend_from_slice(&0x0605_4B50u32.to_le_bytes()); // EOCD signature
  out.extend_from_slice(&0u16.to_le_bytes()); // disk number
  out.extend_from_slice(&0u16.to_le_bytes()); // disk with central dir
  out.extend_from_slice(&num_entries.to_le_bytes()); // entries on this disk
  out.extend_from_slice(&num_entries.to_le_bytes()); // total entries
  out.extend_from_slice(&cd_size.to_le_bytes()); // central dir size
  out.extend_from_slice(&cd_offset.to_le_bytes()); // central dir offset
  out.extend_from_slice(&0u16.to_le_bytes()); // comment length

  out
}

/// Extract a file by path from an uncompressed (STORE) ZIP archive in memory.
pub fn extract_file_from_zip(zip_bytes: &[u8], target_name: &str) -> Option<String> {
  let mut pos = 0usize;
  while pos + 30 <= zip_bytes.len() {
    let sig = u32::from_le_bytes([
      zip_bytes[pos],
      zip_bytes[pos + 1],
      zip_bytes[pos + 2],
      zip_bytes[pos + 3],
    ]);
    if sig != 0x0403_4B50 {
      break;
    }
    let method = u16::from_le_bytes([zip_bytes[pos + 8], zip_bytes[pos + 9]]);
    let comp_size = u32::from_le_bytes([
      zip_bytes[pos + 18],
      zip_bytes[pos + 19],
      zip_bytes[pos + 20],
      zip_bytes[pos + 21],
    ]) as usize;
    let name_len = u16::from_le_bytes([zip_bytes[pos + 26], zip_bytes[pos + 27]]) as usize;
    let extra_len = u16::from_le_bytes([zip_bytes[pos + 28], zip_bytes[pos + 29]]) as usize;

    let name_start = pos + 30;
    let data_start = name_start + name_len + extra_len;
    let data_end = data_start + comp_size;

    if data_end > zip_bytes.len() {
      break;
    }

    if let Ok(entry_name) = std::str::from_utf8(&zip_bytes[name_start..name_start + name_len]) {
      if entry_name == target_name && method == 0 {
        if let Ok(text) = std::str::from_utf8(&zip_bytes[data_start..data_end]) {
          return Some(text.to_string());
        }
      }
    }

    pos = data_end;
  }
  None
}

/// Resolve a guaranteed-valid system filament preset name for the active MachineRig (or default machine)
pub fn resolve_filament_inherits_for_rig(
  spool: &FilamentSpool,
  rig_opt: Option<&MachineRig>,
) -> String {
  let is_oc = spool.name.starts_with("OC -") || spool.name.starts_with("OC:");
  if spool.id.starts_with("disk_")
    && !is_oc
    && !spool.inherits.trim().is_empty()
    && !spool.inherits.contains("TPU for ACE @")
    && !spool.inherits.contains("TPU 95A @")
    && !spool.inherits.contains("PCTG @")
  {
    return spool.inherits.clone();
  }
  let (model, noz) = if let Some(r) = rig_opt {
    (r.printer_model.clone(), r.formatted_nozzle_diameter())
  } else {
    let mc = &crate::constants::constants().machines[0];
    (mc.printer_model.clone(), mc.default_nozzle_diameter.clone())
  };
  // `machine_constants` prefers curated `machines[]` overrides and otherwise derives from
  // the Orca catalog, so a Prusa/Sovol/… rig inherits its own vendor's preset family
  // instead of the Kobra fallback.
  let mc = crate::catalog::machine_constants(&model);
  let noz = if noz.trim().is_empty() {
    mc.default_nozzle_diameter.clone()
  } else {
    noz.trim().to_string()
  };
  let c = crate::constants::constants();
  let trimmed_mat = spool.material_type.trim();
  let base_code = if c
    .direct_inherits_preset_codes
    .iter()
    .any(|code| code.eq_ignore_ascii_case(trimmed_mat))
  {
    trimmed_mat
  } else {
    &c.lookup_polymer(&spool.material_type).base_preset_code
  };
  mc.inherits_template
    .replace("{vendor_prefix}", &mc.vendor_prefix)
    .replace("{base_preset_code}", base_code)
    .replace("{printer_model}", &mc.printer_model)
    .replace("{nozzle_diameter}", &noz)
}

pub fn resolve_kobra_s1_filament_inherits(spool: &FilamentSpool) -> String {
  resolve_filament_inherits_for_rig(spool, None)
}

/// Map internal material codes to OrcaSlicer / AnycubicSlicerNext valid `filament_type` enum strings
pub fn resolve_orca_filament_type(material_type: &str) -> String {
  crate::constants::constants()
    .lookup_polymer(material_type)
    .slicer_filament_type
    .clone()
}

/// Side-channel key set by the `anycubic_pctg_as_petg` rule (Anycubic rigs only by default).
pub const PCTG_AS_PETG_SIDE_CHANNEL: &str = "_anycubic_pctg_as_petg";

/// Marker appended to `filament_notes` when a PCTG-family spool is declared as `PETG`, so a later
/// directory import restores `material_type = PCTG` (and its 240–275°C brackets) from disk.
pub const PCTG_AS_PETG_NOTE_MARKER: &str = "[PCTG declared as PETG for AnycubicSlicerNext]";

/// AnycubicSlicerNext's hard-coded `filament_type` enum lacks `PCTG`: maps a PCTG-family slicer
/// type (`PCTG`, `PCTG-CF`, ...) to its PETG equivalent. Any other type is returned unchanged.
pub fn declare_pctg_family_as_petg(slicer_filament_type: &str) -> String {
  let trimmed = slicer_filament_type.trim();
  match trimmed.to_uppercase().strip_prefix("PCTG") {
    Some(rest) => format!("PETG{}", rest),
    None => trimmed.to_string(),
  }
}

pub const FILAMENT_SETTING_KEYS: &[&str] = &[
  "cool_plate_temp",
  "cool_plate_temp_initial_layer",
  "default_filament_colour",
  "eng_plate_temp",
  "eng_plate_temp_initial_layer",
  "fan_max_speed",
  "fan_max_speed_BRASS",
  "fan_max_speed_HS",
  "fan_min_speed",
  "fan_min_speed_BRASS",
  "fan_min_speed_HS",
  "filament_deretraction_speed",
  "filament_diameter",
  "filament_flow_ratio",
  "filament_max_volumetric_speed",
  "filament_notes",
  "filament_retract_before_wipe",
  "filament_retraction_length",
  "filament_retraction_speed",
  "filament_type",
  "filament_wipe",
  "filament_wipe_distance",
  "filament_z_hop",
  "filament_z_hop_types",
  "hot_plate_temp",
  "hot_plate_temp_initial_layer",
  "nozzle_temperature",
  "nozzle_temperature_BRASS",
  "nozzle_temperature_HS",
  "nozzle_temperature_initial_layer",
  "nozzle_temperature_initial_layer_BRASS",
  "nozzle_temperature_initial_layer_HS",
  "nozzle_temperature_range_high",
  "nozzle_temperature_range_low",
  "pressure_advance",
  "required_nozzle_HRC",
  "supertack_plate_temp",
  "supertack_plate_temp_initial_layer",
  "textured_cool_plate_temp",
  "textured_cool_plate_temp_initial_layer",
  "textured_plate_temp",
  "textured_plate_temp_initial_layer",
];

/// Returns `true` if `key` belongs to OrcaSlicer's **Filament** scope (per-extruder / per-slot
/// vector in `project_settings.config` and standalone `filament/*.json` presets).
/// Filament-scoped keys must NEVER be emitted into Process (`process/*.json`,
/// `different_settings_to_system[0]`) or Machine (`machine/*.json`,
/// `different_settings_to_system[N+1]`) presets.
pub fn is_filament_scoped_key(key: &str) -> bool {
  if FILAMENT_SETTING_KEYS.contains(&key) {
    return true;
  }
  key.starts_with("filament_")
    || key.starts_with("nozzle_temperature")
    || key.ends_with("_plate_temp")
    || key.ends_with("_plate_temp_initial_layer")
    || key.starts_with("fan_min_speed")
    || key.starts_with("fan_max_speed")
    || matches!(
      key,
      "overhang_fan_speed"
        | "overhang_fan_threshold"
        | "enable_overhang_bridge_fan"
        | "close_fan_the_first_x_layers"
        | "slow_down_layer_time"
        | "slow_down_min_speed"
        | "slow_down_for_layer_cooling"
        | "additional_cooling_fan_speed"
        | "during_print_exhaust_fan_speed"
        | "complete_print_exhaust_fan_speed"
        | "chamber_fan_speed"
        | "chamber_temperature"
        | "reduce_fan_stop_start_freq"
        | "fan_cooling_layer_time"
        | "pressure_advance"
        | "enable_pressure_advance"
        | "temperature_vitrification"
        | "required_nozzle_HRC"
        | "default_filament_colour"
    )
}

/// Returns `true` if `key` belongs to OrcaSlicer's **Machine** scope (`machine/*.json` and
/// `different_settings_to_system[N+1]`). Machine-scoped keys must NEVER be emitted into Process
/// (`process/*.json`, `different_settings_to_system[0]`) or Filament (`filament/*.json`) presets.
pub fn is_machine_scoped_key(key: &str) -> bool {
  matches!(
    key,
    "nozzle_type"
      | "nozzle_hrc"
      | "nozzle_diameter"
      | "retraction_length"
      | "retraction_speed"
      | "retraction_distances_when_cut"
      | "deretraction_speed"
      | "retract_before_wipe"
      | "retract_restart_extra"
      | "retraction_minimum_travel"
      | "z_hop"
      | "z_hop_types"
      | "wipe"
      | "wipe_distance"
      | "printer_notes"
      | "printable_area"
      | "printable_height"
      | "before_layer_change_gcode"
      | "change_filament_gcode"
      | "machine_end_gcode"
      | "machine_start_gcode"
      | "layer_change_gcode"
      | "time_lapse_gcode"
      | "machine_pause_gcode"
  ) || key.starts_with("machine_")
    || key.starts_with("extruder_")
}

fn first_str_from_map(
  map: &serde_json::Map<String, serde_json::Value>,
  key: &str,
) -> Option<String> {
  match map.get(key)? {
    serde_json::Value::String(s) => Some(s.clone()),
    serde_json::Value::Array(arr) => arr.first()?.as_str().map(|s| s.to_string()),
    serde_json::Value::Number(n) => Some(n.to_string()),
    _ => None,
  }
}

/// Resolves a filament spool's OrcaSlicer JSON parameter map by importing the resolved OrcaSlicer
/// base filament profile (`default_filament_bases` from `@orcaslicer_repo`), merging any imported
/// `spool.raw_disk_json` values, and applying the spool's active parameters (plus any active
/// weather/climate and anti-stringing offsets when `rig` is provided).
pub fn resolve_spool_orca_filament_map(
  spool: &FilamentSpool,
) -> serde_json::Map<String, serde_json::Value> {
  resolve_spool_orca_filament_map_with_rig_and_overrides(spool, None, None)
}

pub fn resolve_spool_orca_filament_map_with_rig(
  spool: &FilamentSpool,
  rig_opt: Option<&MachineRig>,
) -> serde_json::Map<String, serde_json::Value> {
  resolve_spool_orca_filament_map_with_rig_and_overrides(spool, rig_opt, None)
}

/// Filament keys the Lua nozzle-material stack owns after the Step-1 pipeline unification
/// (`doc/pwl-nozzle-compensation-math.md` §8). `None` means the rule stack left the seeded
/// pipeline-A value unchanged, so the pipeline-A value ships as-is.
pub struct LuaFilamentOverrides {
  pub nozzle_temperature: Option<String>,
  pub nozzle_temperature_initial_layer: Option<String>,
  pub filament_flow_ratio: Option<String>,
  pub filament_retraction_length: Option<String>,
  pub filament_retraction_speed: Option<String>,
  pub filament_z_hop: Option<String>,
  pub filament_z_hop_types: Option<String>,
  pub pressure_advance: Option<String>,
  pub filament_max_volumetric_speed: Option<String>,
  pub fan_min_speed: Option<String>,
  pub fan_max_speed: Option<String>,
}

/// Step-1 pipeline unification: evaluate the default Lua transformation stack once per spool,
/// seeded with that spool's brass baseline (x0) for the filament keys the nozzle rules read,
/// and return the stack's final values for the keys the rules own. Both export paths
/// (standalone filament `.json` and 3MF per-slot columns) go through
/// `resolve_spool_orca_filament_map_with_rig_and_overrides`, so they get parity for free.
fn evaluate_spool_lua_filament_overrides(
  spool: &FilamentSpool,
  rig: &MachineRig,
  seed_temp: i32,
  seed_init: i32,
  seed_ret_len: &str,
  seed_ret_speed: &str,
  seed_flow: f32,
  seed_z_hop: &str,
  seed_z_hop_type: &str,
  seed_pa: f32,
  seed_mvs: f32,
  seed_fan_min: i32,
  seed_fan_max: i32,
  structural_fan_cut_pct: i32,
  pre_temp_delta_c: i32,
) -> LuaFilamentOverrides {
  let mut seed_map = serde_json::Map::new();
  let seed_pa_str = format!("{:.3}", seed_pa);
  let seed_mvs_str = format!("{:.1}", seed_mvs);
  let seed_fan_min_str = seed_fan_min.to_string();
  let seed_fan_max_str = seed_fan_max.to_string();
  for (key, val) in [
    ("nozzle_temperature", seed_temp.to_string()),
    ("nozzle_temperature_initial_layer", seed_init.to_string()),
    ("nozzle_temperature_HS", seed_temp.to_string()),
    ("nozzle_temperature_initial_layer_HS", seed_init.to_string()),
    ("filament_flow_ratio", format!("{:.2}", seed_flow)),
    ("filament_retraction_length", seed_ret_len.to_string()),
    ("filament_retraction_speed", seed_ret_speed.to_string()),
    ("filament_z_hop", seed_z_hop.to_string()),
    ("filament_z_hop_types", seed_z_hop_type.to_string()),
    ("pressure_advance", seed_pa_str.clone()),
    ("filament_max_volumetric_speed", seed_mvs_str.clone()),
    ("fan_min_speed", seed_fan_min_str.clone()),
    ("fan_max_speed", seed_fan_max_str.clone()),
  ] {
    seed_map.insert(key.to_string(), serde_json::json!([val]));
  }
  if structural_fan_cut_pct > 0 {
    seed_map.insert(
      "_structural_fan_cut_pct".to_string(),
      serde_json::Value::String(structural_fan_cut_pct.to_string()),
    );
  }
  let ctx = serde_json::json!({
    "printer_model": rig.printer_model,
    "hotend": rig.hotend,
    "nozzle_type": rig.nozzle_type,
    "nozzle_material": rig.nozzle_material,
    "nozzle_diameter": rig.nozzle_diameter,
    "hotend_max_flow": rig.max_volumetric_flow_limit,
    "hotend_max_temp_c": crate::catalog::machine_constants(&rig.printer_model).max_hotend_temp_c,
    "melt_zone_multiplier": rig.melt_zone_multiplier,
    "printer_kinematics_class": rig.kinematics_class(),
    "travel_accel_target_mm2": rig.travel_accel_policy().travel_accel_target_mm2,
    "travel_accel_floor_mm2": rig.travel_accel_policy().travel_accel_floor_mm2,
    "filament_type": spool.material_type,
    "filament_finish": spool.finish_type,
    "filament_nozzle_max_c": FilamentSpool::polymer_safe_nozzle_bounds(&spool.material_type).1,
    "filament_is_elastomer": crate::constants::constants()
      .is_elastomer_family_material(&spool.material_type),
    "addons": rig.addons,
    "climate_preset": rig.climate_preset,
    "ambient_temp_c": rig.ambient_temp_c,
    "humidity_pct": rig.humidity_pct,
    "pre_temp_delta_c": pre_temp_delta_c,
  });
  let mut rules = crate::transformations::TransformationEngine::get_default_rules();
  crate::transformations::TransformationEngine::sync_rules_for_rig(
    &mut rules,
    &rig.nozzle_type,
    &rig.nozzle_material,
    rig.ace_pro_units,
  );
  let seed_profile = crate::orca_profile::OrcaProfile {
    name: spool.name.clone(),
    profile_type: "filament".to_string(),
    inherits: None,
    from: None,
    values: seed_map,
  };
  let eval =
    crate::transformations::TransformationEngine::evaluate_stack(&seed_profile, &rules, &ctx);
  let vals = eval.resulting_profile.values;
  let take = |key: &str, seed: &str| -> Option<String> {
    let cur = vals
      .get(key)
      .and_then(|v| {
        v.as_array()
          .and_then(|a| a.first())
          .and_then(|x| x.as_str())
          .map(|s| s.to_string())
          .or_else(|| v.as_str().map(|s| s.to_string()))
      })
      .unwrap_or_else(|| seed.to_string());
    if cur != seed {
      Some(cur)
    } else {
      None
    }
  };
  LuaFilamentOverrides {
    nozzle_temperature: take("nozzle_temperature", &seed_temp.to_string()),
    nozzle_temperature_initial_layer: take(
      "nozzle_temperature_initial_layer",
      &seed_init.to_string(),
    ),
    filament_flow_ratio: take("filament_flow_ratio", &format!("{:.2}", seed_flow)),
    filament_retraction_length: take("filament_retraction_length", seed_ret_len),
    filament_retraction_speed: take("filament_retraction_speed", seed_ret_speed),
    filament_z_hop: take("filament_z_hop", seed_z_hop),
    filament_z_hop_types: take("filament_z_hop_types", seed_z_hop_type),
    pressure_advance: take("pressure_advance", &seed_pa_str),
    filament_max_volumetric_speed: take("filament_max_volumetric_speed", &seed_mvs_str),
    fan_min_speed: take("fan_min_speed", &seed_fan_min_str),
    fan_max_speed: take("fan_max_speed", &seed_fan_max_str),
  }
}

pub fn resolve_spool_orca_filament_map_with_rig_and_overrides(
  spool: &FilamentSpool,
  rig_opt: Option<&MachineRig>,
  derived_overrides: Option<&serde_json::Map<String, serde_json::Value>>,
) -> serde_json::Map<String, serde_json::Value> {
  let c = crate::constants::constants();
  let cat = crate::catalog::load_merged_catalog();
  let orca_fil_type = resolve_orca_filament_type(&spool.material_type);
  let poly_cfg = c.lookup_polymer(&spool.material_type);
  let is_foaming_tpu = spool.is_foaming_tpu();
  let mut base_map = serde_json::Map::new();
  if let Some(fil_base) = cat
    .default_filament_bases
    .get(&orca_fil_type)
    .or_else(|| cat.default_filament_bases.get("PLA"))
  {
    for (k, v) in fil_base {
      base_map.insert(k.clone(), v.clone());
    }
  }
  if let Some(ref raw_txt) = spool.raw_disk_json {
    if let Ok(serde_json::Value::Object(disk_obj)) =
      serde_json::from_str::<serde_json::Value>(raw_txt)
    {
      for (k, v) in disk_obj {
        if v.is_string()
          || (v.is_array()
            && v
              .as_array()
              .map(|a| !a.is_empty() && a.iter().all(|x| x.is_string()))
              .unwrap_or(false))
        {
          base_map.insert(k, v);
        }
      }
    }
  }

  // Compute relative weather/climate deltas when a non-temperate rig climate is active
  let (climate_noz_delta, climate_bed_delta, climate_init_bed_extra, climate_retract_delta) =
    if let Some(rig) = rig_opt {
      poly_cfg.compute_climate_deltas(
        rig.ambient_temp_c,
        rig.humidity_pct,
        &rig.climate_preset,
        rig.is_heat_creep_resistant_nozzle(),
      )
    } else {
      (0, 0, 0, 0.0)
    };

  let wet_rh = derived_overrides
    .and_then(|m| m.get("_wet_filament_rh"))
    .and_then(|v| v.as_str())
    .and_then(|s| s.parse::<i32>().ok())
    .unwrap_or(0);
  let wet_active = wet_rh > 20 && !is_foaming_tpu;
  let wet_temp_drop = if wet_active {
    derived_overrides
      .and_then(|m| m.get("_wet_filament_temp_drop_c"))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<i32>().ok())
      .unwrap_or(0)
  } else {
    0
  };
  let wet_init_bed_boost = if wet_active {
    derived_overrides
      .and_then(|m| m.get("_wet_filament_init_bed_boost_c"))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<i32>().ok())
      .unwrap_or(0)
  } else {
    0
  };
  let wet_flow_trim = if wet_active {
    derived_overrides
      .and_then(|m| m.get("_wet_filament_flow_trim"))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<f64>().ok())
      .unwrap_or(0.0)
  } else {
    0.0
  };
  let wet_retract_add = if wet_active {
    derived_overrides
      .and_then(|m| m.get("_wet_filament_retract_add_mm"))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<f64>().ok())
      .unwrap_or(0.0)
  } else {
    0.0
  };
  let wet_retract_before_wipe = if wet_active {
    derived_overrides
      .and_then(|m| m.get("_wet_filament_retract_before_wipe_pct"))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<i32>().ok())
  } else {
    None
  };

  let base_noz = first_str_from_map(&base_map, "nozzle_temperature")
    .and_then(|s| s.parse::<i32>().ok())
    .unwrap_or(spool.print_temp);
  let base_init = first_str_from_map(&base_map, "nozzle_temperature_initial_layer")
    .and_then(|s| s.parse::<i32>().ok())
    .unwrap_or(base_noz);
  let init_delta = if poly_cfg.initial_layer_temp_boost_c(is_foaming_tpu) == 0 {
    0
  } else {
    (base_init - base_noz).clamp(0, 10)
  };
  let (min_safe_noz, mut max_safe_noz) =
    FilamentSpool::polymer_safe_nozzle_bounds(&spool.material_type);
  if is_foaming_tpu {
    max_safe_noz = max_safe_noz.max(280);
  }
  // The active machine's hotend rating caps the polymer band from above (curated
  // `machines[].max_hotend_temp_c` override, else the global limit via
  // `catalog::machine_constants`): a PA spool (290 °C base) must never export above a
  // 260 °C-rated Ender-3 hotend just because the polymer allows 295.
  if let Some(rig) = rig_opt {
    let machine_max = crate::catalog::machine_constants(&rig.printer_model)
      .max_hotend_temp_c
      .min(c.hardware_limits.max_hotend_temp_c);
    max_safe_noz = max_safe_noz.min(machine_max);
    if max_safe_noz < min_safe_noz {
      max_safe_noz = min_safe_noz;
    }
  }
  let (min_safe_bed, max_safe_bed) = FilamentSpool::polymer_safe_bed_bounds(&spool.material_type);
  let climate_min_bed = if let Some(rig) = rig_opt {
    if rig.ambient_temp_c >= 26
      || rig.climate_preset == "tropical"
      || rig.climate_preset == "hot_dry"
    {
      min_safe_bed.max((rig.ambient_temp_c + 15).clamp(45, 50))
    } else {
      min_safe_bed
    }
  } else {
    min_safe_bed
  };
  let pre_wet_temp = (spool.print_temp + climate_noz_delta).clamp(min_safe_noz, max_safe_noz);
  // Structural Strength pack side-channel: per-spool nozzle boost (clamped to THIS polymer's safe max)
  // and max-fan reduction. Skipped for active-foaming TPU, where temperature controls foam density.
  let structural_boost = if is_foaming_tpu {
    0
  } else {
    derived_overrides
      .and_then(|m| m.get("_structural_temp_boost_c"))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<i32>().ok())
      .unwrap_or(0)
      .clamp(0, 10)
  };
  let structural_fan_cut_pct = derived_overrides
    .and_then(|m| m.get("_structural_fan_cut_pct"))
    .and_then(|v| v.as_str())
    .and_then(|s| s.parse::<i32>().ok())
    .unwrap_or(0)
    .clamp(0, 60);
  let pre_wet_temp = (pre_wet_temp + structural_boost).clamp(min_safe_noz, max_safe_noz);
  let mut effective_temp = (pre_wet_temp + wet_temp_drop).clamp(min_safe_noz, max_safe_noz);
  let mut initial_temp = (pre_wet_temp + init_delta).clamp(min_safe_noz, max_safe_noz);
  let effective_bed = (spool.bed_temp + climate_bed_delta).clamp(climate_min_bed, max_safe_bed);
  let initial_bed = (effective_bed + climate_init_bed_extra + wet_init_bed_boost)
    .clamp(climate_min_bed, max_safe_bed);
  let mut effective_flow_ratio = if wet_flow_trim.abs() > 1e-6 {
    (spool.flow_ratio as f64 + wet_flow_trim).clamp(0.85, 1.05) as f32
  } else {
    spool.flow_ratio
  };
  let export_name = spool.format_concise_title();
  let inherits = resolve_filament_inherits_for_rig(spool, rig_opt);

  let kobra_unbottleneck_petg_abs = derived_overrides
    .and_then(|m| m.get("_kobra_unbottleneck_petg_abs"))
    .and_then(|v| v.as_str())
    == Some("1");
  // `anycubic_pctg_as_petg`: only the *declared* `filament_type` string changes. `orca_fil_type`
  // (base preset lookup, PETG-only Kobra tweaks) and `spool.material_type` (safe temperature
  // bounds, `nozzle_temperature_range_low/high`) intentionally stay on PCTG.
  let declare_pctg_as_petg = derived_overrides
    .and_then(|m| m.get(PCTG_AS_PETG_SIDE_CHANNEL))
    .and_then(|v| v.as_str())
    == Some("1");
  let declared_fil_type = if declare_pctg_as_petg {
    declare_pctg_family_as_petg(&orca_fil_type)
  } else {
    orca_fil_type.clone()
  };
  let pctg_declared_as_petg = declared_fil_type != orca_fil_type;
  let is_translucent_finish = spool.finish_type.eq_ignore_ascii_case("Translucent")
    || spool.material_type.to_lowercase().contains("translucent");

  // Ensure retraction, wipe, z-hop, and fan_min_speed / fan_max_speed are always populated cleanly
  let is_external_disk_spool = spool.id.starts_with("disk_")
    && !spool.name.starts_with("OC -")
    && !spool.name.starts_with("OC:");
  let default_ret = poly_cfg.default_retraction_mm as f64;
  let mut base_ret_len = if is_external_disk_spool {
    first_str_from_map(&base_map, "filament_retraction_length")
      .and_then(|s| s.parse::<f64>().ok())
      .unwrap_or(default_ret)
      .max(default_ret)
  } else if spool.is_siraya_foaming_air() {
    0.0
  } else if is_foaming_tpu {
    0.35
  } else {
    default_ret
  };
  if kobra_unbottleneck_petg_abs
    && matches!(orca_fil_type.as_str(), "PETG" | "ABS" | "ASA")
    && base_ret_len > 0.70
  {
    base_ret_len = 0.70;
  }
  let effective_ret_len = if spool.is_siraya_foaming_air() {
    0.0
  } else {
    (base_ret_len + climate_retract_delta + wet_retract_add).clamp(0.30, 1.45)
  };
  let ret_len_str = crate::constants::PolymerConstants::format_retraction_mm(effective_ret_len);
  let mut ret_spd_str = if is_external_disk_spool {
    first_str_from_map(&base_map, "filament_retraction_speed")
      .unwrap_or_else(|| poly_cfg.default_retraction_speed_mms.to_string())
  } else if is_foaming_tpu {
    "25".to_string()
  } else {
    poly_cfg.default_retraction_speed_mms.to_string()
  };
  // App-wide elastomer policy (constants.json `hardware_limits.elastomer_*`, mirrored by the
  // random-config audit): flexible families never retract faster than the configured cap,
  // regardless of vendor disk defaults — faster pulls slip at the direct-drive gears and
  // cavitate the melt.
  if c.is_elastomer_family_material(&spool.material_type) {
    if let Ok(s) = ret_spd_str.trim().parse::<f64>() {
      let cap = c.hardware_limits.elastomer_max_retraction_speed_mms as f64;
      if s > cap {
        ret_spd_str = format!("{}", cap as i32);
      }
    }
  }
  let mut fan_min_pct = spool.fan_speed_percent.clamp(0, 100);
  let mut fan_max_pct = poly_cfg.resolve_fan_max_pct(fan_min_pct);
  let mut effective_pa = spool.pressure_advance;
  let mut effective_mvs = spool.max_volumetric_speed;
  if kobra_unbottleneck_petg_abs && !is_translucent_finish {
    if orca_fil_type == "PETG" && effective_mvs < 19.0 {
      effective_mvs = 19.0;
    } else if (orca_fil_type == "ABS" || orca_fil_type == "ASA") && effective_mvs < 23.0 {
      effective_mvs = 23.0;
    }
  }
  let (z_hop_str, z_hop_type_str, mut retract_before_wipe_str) =
    poly_cfg.resolve_z_hop_and_retract_before_wipe(&spool.finish_type, is_foaming_tpu);
  if let Some(rbw_pct) = wet_retract_before_wipe {
    retract_before_wipe_str = format!("{}%", rbw_pct);
  }
  let mut z_hop_str = z_hop_str;
  let mut z_hop_type_str = z_hop_type_str.to_string();
  let mut ret_len_str = ret_len_str;
  let mut flow_ratio_str_override: Option<String> = None;

  // ===== Step-1 pipeline unification (doc/pwl-nozzle-compensation-math.md §8) =====
  // The nozzle-material / hotend / anti-stringing Lua rules now OWN these filament keys in the
  // exported presets. Seed = the spool's brass baseline (x0) composed with every pipeline-A
  // side channel except nozzle physics (climate, structural boost, wet filament, Kobra
  // unbottleneck, brand/disk retraction), so the stack's output is the FINAL value. Skipped
  // for foaming/Siraya spools whose zero/foam-specific retraction is material physics the
  // curves must not touch. Both export paths share this function → JSON/3MF parity for free.
  if let Some(rig) = rig_opt {
    if !spool.is_siraya_foaming_air() && !is_foaming_tpu {
      let noz_cfg = c.lookup_nozzle(&rig.nozzle_type);
      // Brass baseline x0: the catalog `brass_base_temp` when `print_temp` is exactly the
      // recomputed (brass + nozzle offset / HS base) product; otherwise `print_temp` was
      // hand-tuned (user edit or disk override) and is honored as the effective baseline by
      // inverting the nozzle offset — a 268°C PCTG spool must never snap back to the
      // catalog's 240°C brass value (test_anycubic_pctg_declared_as_petg...).
      let noz_delta = if noz_cfg.uses_hs_base_temp {
        c.hardware_limits.hs_base_temp_delta_c
      } else {
        noz_cfg.temp_offset_from_brass_c
      };
      let brass_base = if spool.brass_base_temp > 0 {
        if (spool.print_temp - (spool.brass_base_temp + noz_delta)).abs() <= 1 {
          spool.brass_base_temp
        } else {
          (spool.print_temp - noz_delta).max(190)
        }
      } else {
        (spool.print_temp - noz_delta).max(190)
      };
      let seed_temp = (brass_base + climate_noz_delta + structural_boost + wet_temp_drop)
        .clamp(min_safe_noz, max_safe_noz);
      let seed_init = (seed_temp + init_delta).clamp(min_safe_noz, max_safe_noz);
      let pre_temp_delta_c = seed_temp - brass_base;
      let overrides = evaluate_spool_lua_filament_overrides(
        spool,
        rig,
        seed_temp,
        seed_init,
        &ret_len_str,
        &ret_spd_str,
        effective_flow_ratio,
        &z_hop_str,
        &z_hop_type_str,
        effective_pa,
        effective_mvs,
        fan_min_pct,
        fan_max_pct,
        structural_fan_cut_pct,
        pre_temp_delta_c,
      );
      // The P25 anti-stringing band trim is a bounded, deliberate excursion below the
      // polymer's conservative floor (`hardware_limits.nozzle_temp_floor_relax_c`, approved in
      // doc/pwl-nozzle-compensation-math.md §5.1), so the final nozzle temp clamps to
      // floor−relax instead of undoing it. The initial-layer temp keeps the hard floor (P25
      // deliberately does not trim it).
      let trim_floor = (min_safe_noz - c.hardware_limits.nozzle_temp_floor_relax_c)
        .max(c.hardware_limits.min_extrusion_temp_c)
        .min(max_safe_noz);
      if let Some(t) = overrides.nozzle_temperature {
        if let Ok(parsed) = t.parse::<i32>() {
          effective_temp = parsed.clamp(trim_floor, max_safe_noz);
        }
      }
      if let Some(t) = overrides.nozzle_temperature_initial_layer {
        if let Ok(parsed) = t.parse::<i32>() {
          initial_temp = parsed.clamp(min_safe_noz, max_safe_noz);
        }
      }
      if let Some(r) = overrides.filament_retraction_length {
        ret_len_str = r;
      }
      if let Some(s) = overrides.filament_retraction_speed {
        ret_spd_str = s;
      }
      if let Some(f) = overrides.filament_flow_ratio {
        if let Ok(parsed) = f.parse::<f32>() {
          effective_flow_ratio = parsed;
          flow_ratio_str_override = Some(f);
        }
      }
      if let Some(pa) = overrides.pressure_advance {
        if let Ok(parsed) = pa.parse::<f32>() {
          effective_pa = parsed.clamp(0.005, 0.120);
        }
      }
      if let Some(mvs) = overrides.filament_max_volumetric_speed {
        if let Ok(parsed) = mvs.parse::<f32>() {
          let mvs_ceil = if orca_fil_type == "ABS" || orca_fil_type == "ASA" {
            24.0
          } else {
            22.0
          };
          effective_mvs = parsed.clamp(1.5, mvs_ceil);
        }
      }
      if let Some(fmin) = overrides.fan_min_speed {
        if let Ok(parsed) = fmin.parse::<i32>() {
          fan_min_pct = parsed.clamp(0, 100);
        }
      }
      if let Some(fmax) = overrides.fan_max_speed {
        if let Ok(parsed) = fmax.parse::<i32>() {
          fan_max_pct = parsed.clamp(0, 100).max(fan_min_pct);
        }
      }
      // Never invent a hop for polymers that deliberately ship 0.00 z-hop (e.g. flexible).
      let base_hop_positive = z_hop_str
        .trim()
        .parse::<f64>()
        .map(|v| v > 0.0)
        .unwrap_or(false);
      if base_hop_positive {
        if let Some(z) = overrides.filament_z_hop {
          z_hop_str = z;
        }
        if let Some(zt) = overrides.filament_z_hop_types {
          z_hop_type_str = zt;
        }
      }
    }
  }
  if structural_fan_cut_pct > 0 {
    let cut_max =
      ((fan_max_pct as f64) * (100 - structural_fan_cut_pct) as f64 / 100.0).round() as i32;
    fan_min_pct = fan_min_pct.min(cut_max);
    fan_max_pct = cut_max;
  }

  let compat_printer = rig_opt
    .map(|r| {
      format!(
        "{} {} nozzle",
        r.printer_model,
        r.formatted_nozzle_diameter()
      )
    })
    .unwrap_or_else(|| {
      let mc = &crate::constants::constants().machines[0];
      format!("{} {} nozzle", mc.printer_model, mc.default_nozzle_diameter)
    });
  base_map.remove("filament_vendor");
  base_map.insert(
    "compatible_printers".to_string(),
    serde_json::json!([compat_printer]),
  );
  base_map.insert("inherits".to_string(), serde_json::json!(inherits));
  base_map.insert("name".to_string(), serde_json::json!(export_name));
  base_map.insert(
    "filament_settings_id".to_string(),
    serde_json::json!([export_name]),
  );
  base_map.insert(
    "filament_type".to_string(),
    serde_json::json!([declared_fil_type]),
  );
  base_map.insert(
    "required_nozzle_HRC".to_string(),
    serde_json::json!([spool.required_nozzle_hrc.to_string()]),
  );
  base_map.insert(
    "default_filament_colour".to_string(),
    serde_json::json!([spool.color_hex]),
  );
  base_map.insert(
    "nozzle_temperature".to_string(),
    serde_json::json!([effective_temp.to_string()]),
  );
  base_map.insert(
    "nozzle_temperature_BRASS".to_string(),
    serde_json::json!([effective_temp.to_string()]),
  );
  base_map.insert(
    "nozzle_temperature_HS".to_string(),
    serde_json::json!([effective_temp.to_string()]),
  );
  base_map.insert(
    "nozzle_temperature_initial_layer".to_string(),
    serde_json::json!([initial_temp.to_string()]),
  );
  base_map.insert(
    "nozzle_temperature_initial_layer_BRASS".to_string(),
    serde_json::json!([initial_temp.to_string()]),
  );
  base_map.insert(
    "nozzle_temperature_initial_layer_HS".to_string(),
    serde_json::json!([initial_temp.to_string()]),
  );
  let (range_low, range_high) =
    FilamentSpool::compute_nozzle_temp_range(&spool.material_type, effective_temp, initial_temp);
  base_map.insert(
    "nozzle_temperature_range_low".to_string(),
    serde_json::json!([range_low.to_string()]),
  );
  base_map.insert(
    "nozzle_temperature_range_high".to_string(),
    serde_json::json!([range_high.to_string()]),
  );
  for plate_key in [
    "hot_plate_temp",
    "textured_plate_temp",
    "cool_plate_temp",
    "eng_plate_temp",
    "textured_cool_plate_temp",
    "supertack_plate_temp",
  ] {
    base_map.insert(
      plate_key.to_string(),
      serde_json::json!([effective_bed.to_string()]),
    );
    base_map.insert(
      format!("{}_initial_layer", plate_key),
      serde_json::json!([initial_bed.to_string()]),
    );
  }
  base_map.insert(
    "filament_flow_ratio".to_string(),
    serde_json::json!([
      flow_ratio_str_override.unwrap_or_else(|| format!("{:.2}", effective_flow_ratio))
    ]),
  );
  base_map.insert(
    "enable_pressure_advance".to_string(),
    serde_json::json!(["1"]),
  );
  base_map.insert(
    "pressure_advance".to_string(),
    serde_json::json!([format!("{:.3}", effective_pa)]),
  );
  base_map.insert(
    "temperature_vitrification".to_string(),
    serde_json::json!([(effective_bed + 10).max(55).to_string()]),
  );
  base_map.insert(
    "filament_max_volumetric_speed".to_string(),
    serde_json::json!([format!("{:.1}", effective_mvs)]),
  );
  base_map.insert(
    "fan_min_speed".to_string(),
    serde_json::json!([fan_min_pct.to_string()]),
  );
  base_map.insert(
    "fan_min_speed_BRASS".to_string(),
    serde_json::json!([fan_min_pct.to_string()]),
  );
  base_map.insert(
    "fan_min_speed_HS".to_string(),
    serde_json::json!([fan_min_pct.to_string()]),
  );
  base_map.insert(
    "fan_max_speed".to_string(),
    serde_json::json!([fan_max_pct.to_string()]),
  );
  base_map.insert(
    "fan_max_speed_BRASS".to_string(),
    serde_json::json!([fan_max_pct.to_string()]),
  );
  base_map.insert(
    "fan_max_speed_HS".to_string(),
    serde_json::json!([fan_max_pct.to_string()]),
  );
  base_map.insert(
    "filament_retraction_length".to_string(),
    serde_json::json!([ret_len_str]),
  );
  base_map.insert(
    "filament_retraction_speed".to_string(),
    serde_json::json!([ret_spd_str]),
  );

  if !is_external_disk_spool {
    base_map.insert(
      "filament_retract_before_wipe".to_string(),
      serde_json::json!([retract_before_wipe_str]),
    );
    base_map.insert(
      "filament_deretraction_speed".to_string(),
      serde_json::json!(["0"]),
    );
    base_map.insert("filament_wipe".to_string(), serde_json::json!(["1"]));
    base_map.insert(
      "filament_wipe_distance".to_string(),
      serde_json::json!(["2"]),
    );
    base_map.insert("filament_z_hop".to_string(), serde_json::json!([z_hop_str]));
    base_map.insert(
      "filament_z_hop_types".to_string(),
      serde_json::json!([z_hop_type_str]),
    );
  } else {
    for (fallback_key, fallback_val) in [
      ("filament_diameter", "1.75"),
      (
        "filament_retract_before_wipe",
        retract_before_wipe_str.as_str(),
      ),
      ("filament_deretraction_speed", "0"),
      ("filament_wipe", "1"),
      ("filament_wipe_distance", "2"),
      ("filament_z_hop", z_hop_str.as_str()),
      ("filament_z_hop_types", z_hop_type_str.as_str()),
    ] {
      if !base_map.contains_key(fallback_key) {
        base_map.insert(fallback_key.to_string(), serde_json::json!([fallback_val]));
      }
    }
    if wet_retract_before_wipe.is_some() {
      base_map.insert(
        "filament_retract_before_wipe".to_string(),
        serde_json::json!([retract_before_wipe_str]),
      );
    }
  }
  if !base_map.contains_key("filament_diameter") {
    base_map.insert("filament_diameter".to_string(), serde_json::json!(["1.75"]));
  }
  let mut notes = spool.generate_descriptive_notes_with_effective_values_and_pa(
    effective_temp,
    effective_bed,
    effective_flow_ratio,
    effective_pa,
    effective_mvs,
    fan_min_pct,
  );
  if pctg_declared_as_petg {
    notes.push_str(&format!(
      " {} filament_type={} (slicer lacks {}); temps follow {} limits {}–{}°C nozzle, {}–{}°C bed.",
      PCTG_AS_PETG_NOTE_MARKER,
      declared_fil_type,
      orca_fil_type,
      spool.material_type,
      min_safe_noz,
      max_safe_noz,
      min_safe_bed,
      max_safe_bed
    ));
  }
  base_map.insert("filament_notes".to_string(), serde_json::json!([notes]));
  base_map.insert(
    "version".to_string(),
    serde_json::json!(WEBCONFIG_ORCA_PRESET_VERSION),
  );
  base_map
}

fn build_filament_preset_json_with_source(
  spool: &FilamentSpool,
  rig: &MachineRig,
  derived_overrides: Option<&serde_json::Map<String, serde_json::Value>>,
  from_src: &str,
) -> String {
  let resolved =
    resolve_spool_orca_filament_map_with_rig_and_overrides(spool, Some(rig), derived_overrides);
  let mut out = serde_json::Map::new();
  out.insert("type".to_string(), serde_json::json!("filament"));
  out.insert("from".to_string(), serde_json::json!(from_src));
  out.insert("is_custom_defined".to_string(), serde_json::json!("0"));

  for meta_key in [
    "inherits",
    "name",
    "filament_settings_id",
    "compatible_printers",
  ] {
    if let Some(v) = resolved.get(meta_key) {
      out.insert(meta_key.to_string(), v.clone());
    }
  }
  for &key in FILAMENT_SETTING_KEYS {
    if let Some(v) = resolved.get(key) {
      out.insert(key.to_string(), v.clone());
    }
  }
  if let Some(v) = resolved.get("version") {
    out.insert("version".to_string(), v.clone());
  }

  serde_json::to_string_pretty(&serde_json::Value::Object(out)).unwrap_or_else(|_| "{}".to_string())
}

/// Export a standalone OrcaSlicer Filament `.json` delta preset (Section 4.A)
pub fn export_standalone_filament_json(spool: &FilamentSpool, rig: &MachineRig) -> String {
  build_filament_preset_json_with_source(spool, rig, None, "User")
}

/// Export a standalone OrcaSlicer Filament `.json` delta preset with active Lua `derived_overrides`
/// (such as `_wet_filament_*` humidity adjustments), guaranteeing 100% key-for-key parity with
/// `.3mf` `Metadata/project_settings.config`.
pub fn export_standalone_filament_json_with_overrides(
  spool: &FilamentSpool,
  rig: &MachineRig,
  derived_overrides: &serde_json::Map<String, serde_json::Value>,
) -> String {
  build_filament_preset_json_with_source(spool, rig, Some(derived_overrides), "User")
}

/// Evaluates the effective OrcaSlicer profile parameters by starting from the imported OrcaSlicer
/// base profile (`@orcaslicer_repo` `default_machine_base` + `default_process_base`), applying the
/// active `ProcessProfile` speeds/heights, running the default Lua transformation stack if
/// `derived_overrides` has not yet been evaluated, and merging `derived_overrides` on top.
pub fn resolve_effective_orca_profile_values(
  rig: &MachineRig,
  process: &ProcessProfile,
  derived_overrides: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Map<String, serde_json::Value> {
  let mut base = crate::orca_profile::OrcaProfile::get_stock_kobra_s1();
  base.set_str("layer_height", &format!("{:.2}", process.layer_height));
  base.set_str("line_width", &format!("{:.2}", process.line_width));
  base.set_str(
    "outer_wall_line_width",
    &format!("{:.2}", process.line_width),
  );
  base.set_str(
    "outer_wall_speed",
    &format!("{:.0}", process.outer_wall_speed),
  );
  base.set_str(
    "inner_wall_speed",
    &format!("{:.0}", process.inner_wall_speed),
  );
  base.set_str(
    "sparse_infill_speed",
    &format!("{:.0}", process.sparse_infill_speed),
  );
  base.set_str(
    "initial_layer_speed",
    &format!("{:.0}", process.initial_layer_speed),
  );
  base.set_str(
    "default_acceleration",
    &format!("{:.0}", process.default_acceleration),
  );
  base.set_str("wall_generator", &process.wall_generator);
  base.set_str("print_sequence", &process.print_sequence);
  base.set_str("notes", &process.notes);

  let mut effective = if derived_overrides.contains_key("seam_slope_type") {
    base.values
  } else {
    let mut rules = crate::transformations::TransformationEngine::get_default_rules();
    crate::transformations::TransformationEngine::sync_rules_for_rig(
      &mut rules,
      &rig.nozzle_type,
      &rig.nozzle_material,
      rig.ace_pro_units,
    );
    let ctx = serde_json::json!({
      "printer_model": rig.printer_model,
      "nozzle_type": rig.nozzle_type,
      "nozzle_material": rig.nozzle_material,
      "nozzle_diameter": rig.nozzle_diameter,
      "hotend_max_flow": rig.max_volumetric_flow_limit,
      "hotend_max_temp_c": crate::catalog::machine_constants(&rig.printer_model).max_hotend_temp_c,
      "melt_zone_multiplier": rig.melt_zone_multiplier,
      "printer_kinematics_class": rig.kinematics_class(),
      "travel_accel_target_mm2": rig.travel_accel_policy().travel_accel_target_mm2,
      "travel_accel_floor_mm2": rig.travel_accel_policy().travel_accel_floor_mm2,
      "layer_height": process.layer_height,
      "line_width": process.line_width,
      "process_tier": crate::transformations::classify_process_scope_tier(process),
      "addons": rig.addons,
    });
    crate::transformations::TransformationEngine::evaluate_stack(&base, &rules, &ctx)
      .resulting_profile
      .values
  };

  let is_optical_glass_mode = derived_overrides
    .get("sparse_infill_pattern")
    .and_then(|v| v.as_str())
    == Some("alignedrectilinear")
    && derived_overrides
      .get("sparse_infill_density")
      .and_then(|v| v.as_str())
      == Some("100%");
  let has_wet_filament_first_layer = derived_overrides
    .get("_wet_filament_init_bed_boost_c")
    .and_then(|v| v.as_str())
    .and_then(|s| s.parse::<i32>().ok())
    .unwrap_or(0)
    > 0;

  let eval_lh = derived_overrides
    .get("_evaluated_for_layer_height")
    .and_then(|v| v.as_str())
    .and_then(|s| s.parse::<f64>().ok())
    .unwrap_or(process.layer_height);
  let get_user_scale = |key: &str| -> f64 {
    derived_overrides
      .get(&format!("_user_rule_scale_{}", key))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<f64>().ok())
      .unwrap_or(1.0)
  };

  for (k, v) in derived_overrides {
    if k.starts_with('_') || is_filament_scoped_key(k) {
      continue;
    }
    // Do not let single-process/single-filament speed caps from `derived_overrides` overwrite
    // each distinct ProcessProfile's own speeds (`outer_wall_speed`, `inner_wall_speed`, `sparse_infill_speed`,
    // `internal_solid_infill_speed`, `initial_layer_speed`, `default_acceleration`), unless Optical Glass mode
    // or Wet Filament first-layer recovery is active.
    if !is_optical_glass_mode
      && !(has_wet_filament_first_layer
        && matches!(
          k.as_str(),
          "initial_layer_speed" | "initial_layer_infill_speed"
        ))
      && !(matches!(k.as_str(), "line_width" | "outer_wall_line_width")
        && v.as_str() != Some("0.42"))
      && matches!(
        k.as_str(),
        "layer_height"
          | "line_width"
          | "outer_wall_line_width"
          | "outer_wall_speed"
          | "inner_wall_speed"
          | "sparse_infill_speed"
          | "internal_solid_infill_speed"
          | "top_surface_speed"
          | "gap_infill_speed"
          | "support_speed"
          | "initial_layer_speed"
          | "initial_layer_infill_speed"
          | "default_acceleration"
      )
    {
      continue;
    }
    if k == "travel_acceleration"
      && !derived_overrides.contains_key("_bounds")
      && !derived_overrides.contains_key("_lua_touched_keys")
    {
      // Fallback for synthetic override maps that bypassed `TransformationEngine::evaluate_stack`
      // (stacks evaluated through `evaluate_stack` enforce `travel_acceleration` bounds via
      // `constrain("travel_acceleration", ...)` and the Priority = ∞ Epilogue Projection pass).
      let accel_policy = rig.travel_accel_policy();
      if let Some(ta) = v.as_str().and_then(|s| s.parse::<f64>().ok()) {
        if ta < accel_policy.travel_accel_floor_mm2 as f64 {
          effective.insert(
            k.clone(),
            serde_json::json!(accel_policy.travel_accel_stock_mm2.to_string()),
          );
          continue;
        }
      }
    }
    // Scale layer-height-dependent overrides (`support_top_z_distance`, `support_bottom_z_distance`,
    // `top_shell_thickness`, `bottom_shell_thickness`) accurately for this specific `process.layer_height`.
    if (process.layer_height - eval_lh).abs() > 1e-4 && eval_lh > 0.01 {
      if matches!(
        k.as_str(),
        "support_top_z_distance" | "support_bottom_z_distance"
      ) {
        if let Some(zd_val) = v.as_str().and_then(|s| s.parse::<f64>().ok()) {
          if zd_val > 0.01 {
            let zd_offset = zd_val - eval_lh;
            let adjusted = (process.layer_height + zd_offset).max(0.04);
            effective.insert(k.clone(), serde_json::json!(format!("{:.2}", adjusted)));
            continue;
          }
        }
      } else if matches!(k.as_str(), "top_shell_thickness" | "bottom_shell_thickness") {
        if let Some(th_val) = v.as_str().and_then(|s| s.parse::<f64>().ok()) {
          if th_val > 0.01 {
            let adjusted = th_val * (process.layer_height / eval_lh);
            effective.insert(k.clone(), serde_json::json!(format!("{:.2}", adjusted)));
            continue;
          }
        }
      }
    }
    effective.insert(k.clone(), v.clone());
  }
  if !is_optical_glass_mode {
    // Preserve each ProcessProfile's exact configured speeds and acceleration without artificial hotend clamping,
    // while applying any explicit user-enabled speed/acceleration/line-width Lua rule scale factors!
    for (key, raw_spd) in [
      ("outer_wall_speed", process.outer_wall_speed),
      ("inner_wall_speed", process.inner_wall_speed),
      ("sparse_infill_speed", process.sparse_infill_speed),
      ("default_acceleration", process.default_acceleration),
    ] {
      let scale = get_user_scale(key);
      let mut scaled_spd = (raw_spd * scale).round().max(15.0);
      if key == "outer_wall_speed" {
        if let Some(vfa_mode) = derived_overrides
          .get("_outer_wall_vfa_mode")
          .and_then(|v| v.as_str())
        {
          let sub_spd = derived_overrides
            .get("_outer_wall_vfa_sub_speed")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(55.0);
          let clear_spd = derived_overrides
            .get("_outer_wall_vfa_clear_speed")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(145.0);
          let eff_mvs = derived_overrides
            .get("_outer_wall_vfa_effective_mvs")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(18.0);
          let is_silk = derived_overrides
            .get("_outer_wall_vfa_is_silk")
            .and_then(|v| v.as_str())
            == Some("1");
          let lock_pref = derived_overrides
            .get("_outer_wall_vfa_lock_pref")
            .and_then(|v| v.as_str())
            .unwrap_or("auto");
          let ow_width = effective
            .get("outer_wall_line_width")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(process.line_width);
          let cross_sec = (process.layer_height * ow_width).max(0.01);
          let mvs_ceiling = (eff_mvs / cross_sec).floor();
          let actual_outer_spd = scaled_spd.min(mvs_ceiling);
          let mut stepped_down = false;
          if vfa_mode == "sub_resonance_gloss"
            || (is_silk && scaled_spd > sub_spd + 5.0)
            || (actual_outer_spd > sub_spd + 5.0 && actual_outer_spd < clear_spd)
          {
            scaled_spd = (sub_spd * scale).round().max(15.0);
            stepped_down = true;
          }
          if lock_pref == "auto" {
            let lock_ow = if vfa_mode == "spire_safe"
              || (!stepped_down
                && !is_silk
                && scaled_spd >= clear_spd
                && process.layer_height <= 0.145)
            {
              "0"
            } else {
              "1"
            };
            effective.insert(
              "dont_slow_down_outer_wall".to_string(),
              serde_json::json!(lock_ow),
            );
          }
        }
      }
      effective.insert(
        key.to_string(),
        serde_json::json!(format!("{:.0}", scaled_spd)),
      );
    }
    // `inner_wall_line_width` / `sparse_infill_line_width` now flow straight through from the
    // Lua stack (nozzle-proportional Line Width Geometry pack + relative symptom wizards).
    if !has_wet_filament_first_layer {
      effective.insert(
        "initial_layer_speed".to_string(),
        serde_json::json!(format!("{:.0}", process.initial_layer_speed)),
      );
    }
  }
  if effective.get("seam_slope_type").and_then(|v| v.as_str()) == Some("contour") {
    effective.insert(
      "seam_slope_type".to_string(),
      serde_json::Value::String("external".to_string()),
    );
  }
  effective
}

pub const EXTRA_PROCESS_OVERRIDE_KEYS: &[&str] = &[
  "initial_layer_line_width",
  "inner_wall_line_width",
  "sparse_infill_line_width",
  "top_surface_line_width",
  "internal_solid_infill_line_width",
  "support_line_width",
  "seam_slope_inner_walls",
  "avoid_crossing_perimeters",
  "enable_support",
  "support_type",
  "support_style",
  "support_top_z_distance",
  "support_bottom_z_distance",
  "support_interface_top_layers",
  "support_interface_bottom_layers",
  "support_interface_spacing",
  "support_interface_speed",
  "ironing_type",
  "ironing_flow",
  "ironing_speed",
  "ironing_spacing",
  "top_surface_pattern",
  "only_one_wall_top",
  "wall_sequence",
  "precise_outer_wall",
  "xy_hole_compensation",
  "xy_contour_compensation",
  "elefant_foot_compensation",
  "bridge_flow",
  "bridge_speed",
  "enable_overhang_speed",
  "detect_overhang_wall",
  "overhang_1_4_speed",
  "overhang_2_4_speed",
  "overhang_3_4_speed",
  "overhang_4_4_speed",
  "wall_loops",
  "alternate_extra_wall",
  "top_shell_layers",
  "bottom_shell_layers",
  "infill_wall_overlap",
  "sparse_infill_pattern",
  "dont_slow_down_outer_wall",
  "small_perimeter_speed",
  "small_perimeter_threshold",
];

/// Builds a user Process `.json` preset from the imported OrcaSlicer process base + Lua-evaluated overrides
pub fn build_process_preset_json(
  process: &ProcessProfile,
  rig: &MachineRig,
  derived_overrides: &serde_json::Map<String, serde_json::Value>,
) -> String {
  let cat = crate::catalog::load_merged_catalog();
  let effective = resolve_effective_orca_profile_values(rig, process, derived_overrides);
  let compat_printer = format!(
    "{} {} nozzle",
    rig.printer_model,
    rig.formatted_nozzle_diameter()
  );
  let proc_name = if process.name.trim().is_empty() {
    process.format_concise_title(&rig.short_model_tag())
  } else {
    process.name.clone()
  };

  let mut out = serde_json::Map::new();
  out.insert("type".to_string(), serde_json::json!("process"));
  out.insert("from".to_string(), serde_json::json!("User"));
  out.insert("is_custom_defined".to_string(), serde_json::json!("0"));
  out.insert("inherits".to_string(), serde_json::json!(process.inherits));
  out.insert(
    "compatible_printers".to_string(),
    serde_json::json!([compat_printer]),
  );
  out.insert("name".to_string(), serde_json::json!(proc_name));
  out.insert(
    "print_settings_id".to_string(),
    serde_json::json!(proc_name),
  );

  for (k, base_val) in &cat.default_process_base {
    if matches!(
      k.as_str(),
      "type"
        | "from"
        | "is_custom_defined"
        | "inherits"
        | "compatible_printers"
        | "name"
        | "print_settings_id"
        | "notes"
        | "version"
        | "filename_format"
    ) || is_filament_scoped_key(k)
      || is_machine_scoped_key(k)
    {
      continue;
    }
    if let Some(eff_val) = effective.get(k) {
      let is_core_or_changed = eff_val != base_val
        || matches!(
          k.as_str(),
          "layer_height"
            | "line_width"
            | "outer_wall_line_width"
            | "outer_wall_speed"
            | "inner_wall_speed"
            | "sparse_infill_speed"
            | "initial_layer_speed"
            | "default_acceleration"
            | "wall_generator"
            | "wall_transition_angle"
            | "wall_transition_length"
            | "min_feature_size"
            | "min_bead_width"
            | "seam_slope_type"
            | "seam_slope_conditional"
            | "seam_slope_start_height"
            | "seam_slope_entire_loop"
            | "seam_slope_inner_walls"
            | "seam_slope_min_length"
            | "seam_slope_steps"
            | "seam_gap"
            | "scarf_angle_threshold"
            | "wipe_on_loops"
            | "sparse_infill_pattern"
            | "reduce_crossing_wall"
            | "print_sequence"
        );
      if is_core_or_changed && eff_val.is_string() {
        out.insert(k.clone(), eff_val.clone());
      }
    }
  }
  let lua_touched_set: Option<std::collections::HashSet<&str>> = derived_overrides
    .get("_lua_touched_keys")
    .and_then(|v| v.as_str())
    .map(|s| s.split(';').filter(|k| !k.is_empty()).collect());
  for &extra_key in EXTRA_PROCESS_OVERRIDE_KEYS {
    if is_filament_scoped_key(extra_key) || is_machine_scoped_key(extra_key) {
      continue;
    }
    let is_touched = match &lua_touched_set {
      Some(set) => set.contains(extra_key),
      None => derived_overrides.contains_key(extra_key),
    };
    if !out.contains_key(extra_key)
      && (is_touched
        || matches!(
          extra_key,
          "seam_slope_inner_walls" | "avoid_crossing_perimeters"
        ))
    {
      if let Some(v) = effective.get(extra_key).filter(|v| v.is_string()) {
        out.insert(extra_key.to_string(), v.clone());
      }
    }
  }
  let proc_notes = if process.has_auto_generated_notes() {
    process.generate_descriptive_notes_with_effective_values(&effective)
  } else {
    process.notes.clone()
  };
  out.insert("notes".to_string(), serde_json::json!(proc_notes));
  out.insert(
    "version".to_string(),
    serde_json::json!(WEBCONFIG_ORCA_PRESET_VERSION),
  );

  serde_json::to_string_pretty(&serde_json::Value::Object(out)).unwrap_or_else(|_| "{}".to_string())
}

fn format_retraction_length_str(ret_len: f64) -> String {
  if (ret_len - ret_len.round()).abs() < 1e-6 {
    format!("{:.1}", ret_len)
  } else {
    format!("{:.2}", ret_len)
      .trim_end_matches('0')
      .trim_end_matches('.')
      .to_string()
  }
}

/// Builds a user Machine `.json` preset from the imported OrcaSlicer machine base + rig settings
pub fn build_machine_preset_json(rig: &MachineRig) -> String {
  build_machine_preset_json_with_overrides(rig, None)
}

/// Builds a user Machine `.json` preset from the imported OrcaSlicer machine base + rig settings
/// and any machine-level Lua overrides (such as `machine_start_gcode` and `machine_end_gcode`).
pub fn build_machine_preset_json_with_overrides(
  rig: &MachineRig,
  derived_overrides: Option<&serde_json::Map<String, serde_json::Value>>,
) -> String {
  let cat = crate::catalog::load_merged_catalog();
  let (orca_noz, hrc) = rig.orca_machine_nozzle_type_and_hrc();
  let noz_diam_str = rig.formatted_nozzle_diameter();
  let notes_str = if rig.notes.trim().is_empty() {
    rig.generate_descriptive_notes()
  } else {
    rig.notes.clone()
  };
  let mach_name = if rig.name.trim().is_empty() {
    rig.format_concise_title()
  } else {
    rig.name.clone()
  };
  let mut out = serde_json::Map::new();
  out.insert("type".to_string(), serde_json::json!("machine"));
  out.insert("from".to_string(), serde_json::json!("User"));
  out.insert("is_custom_defined".to_string(), serde_json::json!("0"));
  out.insert(
    "inherits".to_string(),
    serde_json::json!(format!("{} {} nozzle", rig.printer_model, noz_diam_str)),
  );
  out.insert("name".to_string(), serde_json::json!(mach_name));
  out.insert(
    "printer_settings_id".to_string(),
    serde_json::json!(mach_name),
  );
  // Stamp every save (matching AnycubicSlicerNext's convention) so a later directory import can
  // order `OC -` incarnations of the same rig (nozzle swaps rename the preset) newest-first.
  out.insert(
    "printer_timestamp".to_string(),
    serde_json::json!(crate::models::now_printer_timestamp()),
  );
  out.insert(
    "printer_model".to_string(),
    serde_json::json!(rig.printer_model),
  );
  out.insert(
    "printer_variant".to_string(),
    serde_json::json!(noz_diam_str),
  );
  out.insert("nozzle_type".to_string(), serde_json::json!(orca_noz));
  out.insert("nozzle_hrc".to_string(), serde_json::json!(hrc.to_string()));
  out.insert(
    "nozzle_diameter".to_string(),
    serde_json::json!([noz_diam_str]),
  );
  let ret_len_str = format_retraction_length_str(rig.base_retraction_length);
  out.insert(
    "retraction_length".to_string(),
    serde_json::json!([ret_len_str]),
  );
  if rig.ace_pro_units > 0 {
    out.insert(
      "retraction_distances_when_cut".to_string(),
      serde_json::json!(["18"]),
    );
  }
  if let Some(ret_spd) = cat.default_machine_base.get("retraction_speed") {
    out.insert("retraction_speed".to_string(), ret_spd.clone());
  }
  if let Some(ov) = derived_overrides {
    for gcode_key in [
      "before_layer_change_gcode",
      "change_filament_gcode",
      "machine_end_gcode",
      "machine_start_gcode",
    ] {
      if let Some(s) = ov
        .get(gcode_key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
      {
        out.insert(gcode_key.to_string(), serde_json::json!(s));
      }
    }
  }
  out.insert("printer_notes".to_string(), serde_json::json!(notes_str));
  out.insert(
    "version".to_string(),
    serde_json::json!(WEBCONFIG_ORCA_PRESET_VERSION),
  );
  serde_json::to_string_pretty(&serde_json::Value::Object(out)).unwrap_or_else(|_| "{}".to_string())
}

/// Build the `Metadata/project_settings.config` JSON string for 4-slot or 8-slot ACE Pro loadouts (Section 4.B)
pub fn build_project_settings_config_json(
  rig: &MachineRig,
  process: &ProcessProfile,
  active_slots: &[FilamentSpool],
  flush_matrix: &[String],
  derived_overrides: &serde_json::Map<String, serde_json::Value>,
) -> String {
  let cat = crate::catalog::load_merged_catalog();
  let effective = resolve_effective_orca_profile_values(rig, process, derived_overrides);
  let (orca_nozzle_type, nozzle_hrc) = rig.orca_machine_nozzle_type_and_hrc();
  let noz_diam_str = rig.formatted_nozzle_diameter();
  let rig_notes = if rig.notes.trim().is_empty() {
    rig.generate_descriptive_notes()
  } else {
    rig.notes.clone()
  };
  let proc_notes = if process.has_auto_generated_notes() {
    process.generate_descriptive_notes_with_effective_values(&effective)
  } else {
    process.notes.clone()
  };
  let mach_name = if rig.name.trim().is_empty() {
    rig.format_concise_title()
  } else {
    rig.name.clone()
  };
  let proc_name = if process.name.trim().is_empty() {
    process.format_concise_title(&rig.short_model_tag())
  } else {
    process.name.clone()
  };

  let raw_slot_maps: Vec<serde_json::Map<String, serde_json::Value>> = active_slots
    .iter()
    .map(|sp| {
      resolve_spool_orca_filament_map_with_rig_and_overrides(sp, Some(rig), Some(derived_overrides))
    })
    .collect();
  // When multiple slots share the same `filament_settings_id` (`OC - *.json`), the file on disk
  // corresponds to the last slot with that `filament_settings_id`. Ensure all preset-level keys
  // (`FILAMENT_SETTING_KEYS`, including `default_filament_colour`) match that canonical disk file
  // so OrcaSlicer's `profile_print_params_same` returns true for every slot, while `filament_colour`
  // preserves each slot's individual spool color.
  let mut canonical_by_id: std::collections::HashMap<
    String,
    serde_json::Map<String, serde_json::Value>,
  > = std::collections::HashMap::new();
  for m in &raw_slot_maps {
    if let Some(fid) = first_str_from_map(m, "filament_settings_id") {
      canonical_by_id.insert(fid, m.clone());
    }
  }
  let slot_maps: Vec<serde_json::Map<String, serde_json::Value>> = raw_slot_maps
    .iter()
    .map(|m| {
      if let Some(fid) = first_str_from_map(m, "filament_settings_id") {
        if let Some(canon) = canonical_by_id.get(&fid) {
          return canon.clone();
        }
      }
      m.clone()
    })
    .collect();
  let slot_extruder_colours: Vec<String> = raw_slot_maps
    .iter()
    .map(|m| first_str_from_map(m, "default_filament_colour").unwrap_or_default())
    .collect();

  let collect_slot_vec = |key: &str| -> Vec<String> {
    slot_maps
      .iter()
      .map(|m| first_str_from_map(m, key).unwrap_or_default())
      .collect()
  };

  let compat_printer = format!("{} {} nozzle", rig.printer_model, noz_diam_str);
  let proc_inherits = if process.inherits.trim().is_empty() {
    cat
      .default_machine_base
      .get("default_print_profile")
      .and_then(|v| v.as_str())
      .unwrap_or_default()
      .to_string()
  } else {
    process.inherits.clone()
  };
  let mut inherits_group: Vec<String> = Vec::with_capacity(active_slots.len() + 2);
  inherits_group.push(proc_inherits);
  for m in &slot_maps {
    inherits_group.push(first_str_from_map(m, "inherits").unwrap_or_default());
  }
  inherits_group.push(compat_printer.clone());

  let mut out = serde_json::Map::new();
  out.insert("from".to_string(), serde_json::json!("project"));
  out.insert(
    "printer_model".to_string(),
    serde_json::json!(rig.printer_model),
  );
  out.insert(
    "printer_variant".to_string(),
    serde_json::json!(noz_diam_str),
  );
  out.insert(
    "printer_settings_id".to_string(),
    serde_json::json!(mach_name),
  );
  out.insert("printer_notes".to_string(), serde_json::json!(rig_notes));
  out.insert(
    "print_settings_id".to_string(),
    serde_json::json!(proc_name),
  );
  out.insert("notes".to_string(), serde_json::json!(proc_notes));
  out.insert(
    "nozzle_diameter".to_string(),
    serde_json::json!([noz_diam_str]),
  );
  out.insert(
    "nozzle_hrc".to_string(),
    serde_json::json!(nozzle_hrc.to_string()),
  );
  out.insert(
    "nozzle_type".to_string(),
    serde_json::json!(orca_nozzle_type),
  );
  out.insert(
    "retraction_length".to_string(),
    serde_json::json!([format_retraction_length_str(rig.base_retraction_length)]),
  );
  if rig.ace_pro_units > 0 {
    out.insert(
      "retraction_distances_when_cut".to_string(),
      serde_json::json!(["18"]),
    );
  }
  for mach_key in ["retraction_speed", "printable_area", "printable_height"] {
    if let Some(v) = cat
      .default_machine_base
      .get(mach_key)
      .or_else(|| effective.get(mach_key))
    {
      out.insert(mach_key.to_string(), v.clone());
    }
  }
  out.insert(
    "compatible_printers".to_string(),
    serde_json::json!([compat_printer]),
  );

  let slot_ids = collect_slot_vec("filament_settings_id");
  out.insert(
    "filament_settings_id".to_string(),
    serde_json::json!(slot_ids),
  );
  for &fk in FILAMENT_SETTING_KEYS {
    let vals = collect_slot_vec(fk);
    if fk == "default_filament_colour" {
      out.insert(
        "filament_colour".to_string(),
        serde_json::json!(slot_extruder_colours.clone()),
      );
    }
    out.insert(fk.to_string(), serde_json::json!(vals));
  }

  let mut proc_diff_keys = std::collections::BTreeSet::new();
  proc_diff_keys.insert("notes".to_string());
  for (k, base_val) in &cat.default_process_base {
    if matches!(
      k.as_str(),
      "type"
        | "from"
        | "is_custom_defined"
        | "inherits"
        | "compatible_printers"
        | "name"
        | "print_settings_id"
        | "notes"
        | "version"
        | "filename_format"
    ) || is_filament_scoped_key(k)
      || is_machine_scoped_key(k)
    {
      continue;
    }
    if let Some(eff_val) = effective.get(k) {
      let is_core_or_changed = eff_val != base_val
        || matches!(
          k.as_str(),
          "layer_height"
            | "line_width"
            | "outer_wall_line_width"
            | "outer_wall_speed"
            | "inner_wall_speed"
            | "sparse_infill_speed"
            | "initial_layer_speed"
            | "default_acceleration"
            | "wall_generator"
            | "wall_transition_angle"
            | "wall_transition_length"
            | "min_feature_size"
            | "min_bead_width"
            | "seam_slope_type"
            | "seam_slope_conditional"
            | "seam_slope_start_height"
            | "seam_slope_entire_loop"
            | "seam_slope_inner_walls"
            | "seam_slope_min_length"
            | "seam_slope_steps"
            | "seam_gap"
            | "scarf_angle_threshold"
            | "wipe_on_loops"
            | "sparse_infill_pattern"
            | "reduce_crossing_wall"
            | "print_sequence"
        );
      if is_core_or_changed && eff_val.is_string() {
        out.insert(k.clone(), eff_val.clone());
        proc_diff_keys.insert(k.clone());
      }
    }
  }
  let lua_touched_set: Option<std::collections::HashSet<&str>> = derived_overrides
    .get("_lua_touched_keys")
    .and_then(|v| v.as_str())
    .map(|s| s.split(';').filter(|k| !k.is_empty()).collect());
  for &extra_key in EXTRA_PROCESS_OVERRIDE_KEYS {
    if is_filament_scoped_key(extra_key) || is_machine_scoped_key(extra_key) {
      continue;
    }
    let is_touched = match &lua_touched_set {
      Some(set) => set.contains(extra_key),
      None => derived_overrides.contains_key(extra_key),
    };
    if !out.contains_key(extra_key)
      && (is_touched
        || matches!(
          extra_key,
          "seam_slope_inner_walls" | "avoid_crossing_perimeters"
        ))
    {
      if let Some(v) = effective.get(extra_key).filter(|v| v.is_string()) {
        out.insert(extra_key.to_string(), v.clone());
        proc_diff_keys.insert(extra_key.to_string());
      }
    }
  }

  let filament_diff_str = FILAMENT_SETTING_KEYS.join(";");
  let mut machine_diff_keys: std::collections::BTreeSet<String> = [
    "nozzle_diameter",
    "nozzle_hrc",
    "nozzle_type",
    "printer_notes",
    "retraction_length",
    "retraction_speed",
  ]
  .into_iter()
  .map(|s| s.to_string())
  .collect();
  if rig.ace_pro_units > 0 {
    machine_diff_keys.insert("retraction_distances_when_cut".to_string());
  }
  for gcode_key in [
    "before_layer_change_gcode",
    "change_filament_gcode",
    "machine_end_gcode",
    "machine_start_gcode",
  ] {
    if let Some(s) = derived_overrides
      .get(gcode_key)
      .and_then(|v| v.as_str())
      .filter(|s| !s.trim().is_empty())
    {
      out.insert(gcode_key.to_string(), serde_json::json!(s));
      machine_diff_keys.insert(gcode_key.to_string());
    }
  }
  let machine_diff_str = machine_diff_keys.into_iter().collect::<Vec<_>>().join(";");

  let mut different_settings_to_system: Vec<String> = Vec::with_capacity(active_slots.len() + 2);
  different_settings_to_system.push(proc_diff_keys.into_iter().collect::<Vec<_>>().join(";"));
  for _ in active_slots {
    different_settings_to_system.push(filament_diff_str.clone());
  }
  different_settings_to_system.push(machine_diff_str);

  out.insert(
    "inherits_group".to_string(),
    serde_json::json!(inherits_group),
  );
  out.insert(
    "different_settings_to_system".to_string(),
    serde_json::json!(different_settings_to_system),
  );

  if let Some(fm) = effective
    .get("flush_multiplier")
    .or_else(|| cat.default_machine_base.get("printer_flush_multiplier"))
  {
    out.insert("flush_multiplier".to_string(), fm.clone());
  }
  let base_wipe_vol = cat
    .default_process_base
    .get("prime_volume")
    .and_then(|v| v.as_str())
    .and_then(|s| s.parse::<i32>().ok())
    .map(|p| (p * 7).to_string())
    .unwrap_or_default();
  out.insert(
    "flush_volumes_vector".to_string(),
    serde_json::json!(vec![base_wipe_vol; active_slots.len() * 2]),
  );
  out.insert(
    "flush_volumes_matrix".to_string(),
    serde_json::json!(flush_matrix),
  );
  if let Some(ver) = cat
    .default_machine_base
    .get("version")
    .or_else(|| effective.get("version"))
  {
    out.insert("version".to_string(), ver.clone());
  } else {
    out.insert("version".to_string(), serde_json::json!("02.03.00.03"));
  }

  let rendered = serde_json::to_string_pretty(&serde_json::Value::Object(out))
    .unwrap_or_else(|_| "{}".to_string());

  #[cfg(debug_assertions)]
  {
    let mach_json = build_machine_preset_json_with_overrides(rig, Some(derived_overrides));
    let proc_json = build_process_preset_json(process, rig, derived_overrides);
    let mut disk_filaments_by_id: std::collections::HashMap<String, String> =
      std::collections::HashMap::new();
    for sp in active_slots {
      let fjson = export_standalone_filament_json_with_overrides(sp, rig, derived_overrides);
      let fid = sp.format_concise_title();
      disk_filaments_by_id.insert(fid, fjson.clone());
      if !sp.name.trim().is_empty() {
        disk_filaments_by_id.insert(sp.name.clone(), fjson);
      }
    }
    if let Err(errs) = verify_project_settings_matches_standalone_presets(
      &rendered,
      &mach_json,
      &proc_json,
      &disk_filaments_by_id,
    ) {
      panic!(
        "3MF project_settings.config diverged from standalone .json presets (would cause AnycubicSlicerNext profile_print_params_same == false and reset slots on New Project!):\n  - {}",
        errs.join("\n  - ")
      );
    }
  }

  rendered
}

/// Verifies that `Metadata/project_settings.config` matches the standalone Machine, Process, and
/// Filament `.json` files key-for-key under OrcaSlicer / AnycubicSlicerNext's
/// `PresetCollection::load_external_preset` (`update_non_diff_values_to_base_config` +
/// `profile_print_params_same`).
///
/// If even a single parameter differs between `project_settings.config` and the disk `.json` files:
/// - Slot 1 (`i == 0`, `LoadAndSelect::Always`) gets marked dirty (`is_dirty = true`) and triggers
///   the "Save / Discard modifications" dialog.
/// - Slots 2..N (`i = 1..N-1`, `LoadAndSelect::OnlyIfModified`) get loaded as temporary
///   `is_project_embedded = true` presets (`OC - ... (Loadout.3mf)`), which are erased by
///   `reset_project_embedded_presets()` when clicking "New Project" -> "Discard", resetting Slots 2..N
///   to `Anycubic PLA @Anycubic Kobra S1 0.4 nozzle`!
pub fn verify_project_settings_matches_standalone_presets(
  project_settings_json: &str,
  machine_json: &str,
  process_json: &str,
  filament_jsons_by_id: &std::collections::HashMap<String, String>,
) -> Result<(), Vec<String>> {
  let mut errors = Vec::new();

  let proj_obj = match serde_json::from_str::<serde_json::Value>(project_settings_json)
    .ok()
    .and_then(|v| v.as_object().cloned())
  {
    Some(o) => o,
    None => {
      errors.push("project_settings_json is not a valid JSON object".to_string());
      return Err(errors);
    }
  };
  let mach_obj = match serde_json::from_str::<serde_json::Value>(machine_json)
    .ok()
    .and_then(|v| v.as_object().cloned())
  {
    Some(o) => o,
    None => {
      errors.push("machine_json is not a valid JSON object".to_string());
      return Err(errors);
    }
  };
  let proc_obj = match serde_json::from_str::<serde_json::Value>(process_json)
    .ok()
    .and_then(|v| v.as_object().cloned())
  {
    Some(o) => o,
    None => {
      errors.push("process_json is not a valid JSON object".to_string());
      return Err(errors);
    }
  };

  let slot_ids: Vec<String> = proj_obj
    .get("filament_settings_id")
    .and_then(|v| v.as_array())
    .map(|arr| {
      arr
        .iter()
        .filter_map(|x| x.as_str().map(|s| s.to_string()))
        .collect()
    })
    .unwrap_or_default();
  if slot_ids.is_empty() {
    errors.push("project_settings.config has empty filament_settings_id".to_string());
    return Err(errors);
  }

  let diff_groups: Vec<std::collections::BTreeSet<String>> = proj_obj
    .get("different_settings_to_system")
    .and_then(|v| v.as_array())
    .map(|arr| {
      arr
        .iter()
        .map(|x| {
          x.as_str()
            .unwrap_or("")
            .split(';')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect()
        })
        .collect()
    })
    .unwrap_or_default();

  if diff_groups.len() != slot_ids.len() + 2 {
    errors.push(format!(
      "different_settings_to_system length ({}) != slot_ids.len() + 2 ({})",
      diff_groups.len(),
      slot_ids.len() + 2
    ));
    return Err(errors);
  }

  let is_ignored_meta_key = |k: &str| -> bool {
    matches!(
      k,
      "type"
        | "from"
        | "is_custom_defined"
        | "name"
        | "version"
        | "inherits"
        | "compatible_prints"
        | "compatible_prints_condition"
        | "compatible_printers"
        | "compatible_printers_condition"
        | "print_settings_id"
        | "filament_settings_id"
        | "sla_print_settings_id"
        | "sla_material_settings_id"
        | "printer_settings_id"
        | "printer_model"
        | "printer_variant"
        | "default_print_profile"
        | "default_filament_profile"
        | "default_sla_print_profile"
        | "default_sla_material_profile"
        | "printer_timestamp"
        | "print_timestamp"
        | "filament_timestamp"
    )
  };

  // 1. Verify Process preset (diff_groups[0])
  let proc_diff = &diff_groups[0];
  for (k, v) in &proc_obj {
    if is_ignored_meta_key(k) {
      continue;
    }
    if is_filament_scoped_key(k) {
      errors.push(format!(
        "Process preset leaked filament-scoped key '{}' ({:?})",
        k, v
      ));
    }
    if is_machine_scoped_key(k) {
      errors.push(format!(
        "Process preset leaked machine-scoped key '{}' ({:?})",
        k, v
      ));
    }
    if !proc_diff.contains(k) {
      errors.push(format!(
        "Process key '{}' is in standalone process .json ({:?}) but missing from different_settings_to_system[0]",
        k, v
      ));
    }
    match proj_obj.get(k) {
      Some(pv) if pv == v => {}
      other => errors.push(format!(
        "Process key '{}' mismatch: standalone .json={:?} vs 3mf project_settings={:?}",
        k, v, other
      )),
    }
  }
  for k in proc_diff {
    if is_ignored_meta_key(k) {
      continue;
    }
    if is_filament_scoped_key(k) {
      errors.push(format!(
        "different_settings_to_system[0] (Process) leaked filament-scoped key '{}'",
        k
      ));
    }
    if is_machine_scoped_key(k) {
      errors.push(format!(
        "different_settings_to_system[0] (Process) leaked machine-scoped key '{}'",
        k
      ));
    }
    match (proc_obj.get(k), proj_obj.get(k)) {
      (Some(dv), Some(pv)) if dv == pv => {}
      (dv, pv) => errors.push(format!(
        "Process diff key '{}' mismatch: standalone .json={:?} vs 3mf project_settings={:?}",
        k, dv, pv
      )),
    }
  }

  // 2. Verify Machine preset (diff_groups[slot_ids.len() + 1])
  let mach_diff = &diff_groups[slot_ids.len() + 1];
  for (k, v) in &mach_obj {
    if is_ignored_meta_key(k) {
      continue;
    }
    if is_filament_scoped_key(k) {
      errors.push(format!(
        "Machine preset leaked filament-scoped key '{}' ({:?})",
        k, v
      ));
    }
    if !mach_diff.contains(k) {
      errors.push(format!(
        "Machine key '{}' is in standalone machine .json ({:?}) but missing from different_settings_to_system[{}]",
        k,
        v,
        slot_ids.len() + 1
      ));
    }
    match proj_obj.get(k) {
      Some(pv) if pv == v => {}
      other => errors.push(format!(
        "Machine key '{}' mismatch: standalone .json={:?} vs 3mf project_settings={:?}",
        k, v, other
      )),
    }
  }
  for k in mach_diff {
    if is_ignored_meta_key(k) {
      continue;
    }
    if is_filament_scoped_key(k) {
      errors.push(format!(
        "different_settings_to_system[{}] (Machine) leaked filament-scoped key '{}'",
        slot_ids.len() + 1,
        k
      ));
    }
    match (mach_obj.get(k), proj_obj.get(k)) {
      (Some(dv), Some(pv)) if dv == pv => {}
      (dv, pv) => errors.push(format!(
        "Machine diff key '{}' mismatch: standalone .json={:?} vs 3mf project_settings={:?}",
        k, dv, pv
      )),
    }
  }

  // 3. Verify every Filament slot (diff_groups[1..=slot_ids.len()])
  for (slot_idx, fid) in slot_ids.iter().enumerate() {
    let fil_str = match filament_jsons_by_id.get(fid) {
      Some(s) => s,
      None => {
        errors.push(format!(
          "Slot {} filament '{}' missing from filament_jsons_by_id",
          slot_idx + 1,
          fid
        ));
        continue;
      }
    };
    let fil_obj = match serde_json::from_str::<serde_json::Value>(fil_str)
      .ok()
      .and_then(|v| v.as_object().cloned())
    {
      Some(o) => o,
      None => {
        errors.push(format!(
          "Slot {} filament '{}' is not a valid JSON object",
          slot_idx + 1,
          fid
        ));
        continue;
      }
    };
    let fil_diff = &diff_groups[slot_idx + 1];
    for (k, v) in &fil_obj {
      if is_ignored_meta_key(k) {
        continue;
      }
      if !is_filament_scoped_key(k) {
        errors.push(format!(
          "Slot {} ({}) filament preset leaked non-filament key '{}' ({:?})",
          slot_idx + 1,
          fid,
          k,
          v
        ));
      }
      if !fil_diff.contains(k) {
        errors.push(format!(
          "Slot {} ({}) filament key '{}' is in standalone .json ({:?}) but missing from different_settings_to_system[{}]",
          slot_idx + 1,
          fid,
          k,
          v,
          slot_idx + 1
        ));
      }
      let disk_scalar = v
        .as_array()
        .and_then(|a| a.first())
        .and_then(|x| x.as_str())
        .or_else(|| v.as_str());
      let proj_scalar = proj_obj
        .get(k)
        .and_then(|pv| pv.as_array())
        .and_then(|a| a.get(slot_idx))
        .and_then(|x| x.as_str());
      if disk_scalar != proj_scalar || disk_scalar.is_none() {
        errors.push(format!(
          "Slot {} ({}) filament key '{}' mismatch: standalone .json={:?} vs 3mf project_settings[{}]={:?}",
          slot_idx + 1,
          fid,
          k,
          disk_scalar,
          slot_idx,
          proj_scalar
        ));
      }
    }
    for k in fil_diff {
      if is_ignored_meta_key(k) {
        continue;
      }
      let disk_scalar = fil_obj
        .get(k)
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|x| x.as_str())
        .or_else(|| fil_obj.get(k).and_then(|v| v.as_str()));
      let proj_scalar = proj_obj
        .get(k)
        .and_then(|pv| pv.as_array())
        .and_then(|a| a.get(slot_idx))
        .and_then(|x| x.as_str());
      if disk_scalar != proj_scalar || disk_scalar.is_none() {
        errors.push(format!(
          "Slot {} ({}) filament diff key '{}' mismatch: standalone .json={:?} vs 3mf project_settings[{}]={:?}",
          slot_idx + 1,
          fid,
          k,
          disk_scalar,
          slot_idx,
          proj_scalar
        ));
      }
    }
  }

  if errors.is_empty() {
    Ok(())
  } else {
    Err(errors)
  }
}

/// Build a complete `.3mf` Project Container (ZIP archive) with `Metadata/project_settings.config`
/// and `Metadata/orcaslicer_webconfig_loadout.json`.
/// Note: Do NOT embed `Metadata/filament_settings_<k>.config`, `Metadata/process_settings_1.config`,
/// or `Metadata/machine_settings_1.config` inside `Metadata/`, because `BBS_3MF` extracts any extra
/// `Metadata/*.config` files and calls `PresetBundle::load_config_file_config`, which sets
/// `is_external = true` on existing user presets of the same name and causes `remove_project_presets()`
/// to purge them when the user clicks "New Project"!
pub fn build_3mf_project_container(
  rig: &MachineRig,
  process: &ProcessProfile,
  active_slots: &[FilamentSpool],
  flush_matrix: &[String],
  derived_overrides: &serde_json::Map<String, serde_json::Value>,
) -> Vec<u8> {
  let content_types_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="model" ContentType="application/vnd.ms-package.3dmanufacturing-3dmodel+xml"/>
  <Default Extension="config" ContentType="application/json"/>
  <Default Extension="json" ContentType="application/json"/>
</Types>"#;

  let rels_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Target="/3D/3dmodel.model" Id="rel-1" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"/>
</Relationships>"#;

  let model_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xml:lang="en-US" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
  <metadata name="Application">BambuStudio-02.03.00.03</metadata>
  <resources/>
  <build/>
</model>"#;

  let project_settings_json =
    build_project_settings_config_json(rig, process, active_slots, flush_matrix, derived_overrides);

  let webconfig_loadout_json = serde_json::to_string_pretty(&serde_json::json!({
    "ace_pro_units": rig.ace_pro_units,
    "filament_material_type": active_slots.iter().map(|s| s.material_type.clone()).collect::<Vec<_>>(),
    "filament_finish_type": active_slots.iter().map(|s| s.finish_type.clone()).collect::<Vec<_>>(),
    "filament_opacity": active_slots.iter().map(|s| s.opacity).collect::<Vec<_>>(),
    "filament_pigment_stickiness": active_slots.iter().map(|s| s.pigment_stickiness).collect::<Vec<_>>(),
    "filament_drying_temp_c": active_slots.iter().map(|s| s.drying_temp_c).collect::<Vec<_>>(),
    "filament_drying_time_hr": active_slots.iter().map(|s| s.drying_time_hr).collect::<Vec<_>>()
  }))
  .unwrap_or_else(|_| "{}".to_string());

  let files: Vec<(&str, &[u8])> = vec![
    ("[Content_Types].xml", content_types_xml.as_bytes()),
    ("_rels/.rels", rels_xml.as_bytes()),
    ("3D/3dmodel.model", model_xml.as_bytes()),
    (
      "Metadata/project_settings.config",
      project_settings_json.as_bytes(),
    ),
    (
      "Metadata/orcaslicer_webconfig_loadout.json",
      webconfig_loadout_json.as_bytes(),
    ),
  ];

  build_zip_archive(&files)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImportedAceLoadout {
  pub ace_pro_units: usize,
  pub spools: Vec<FilamentSpool>,
  pub flush_matrix: Option<Vec<String>>,
}

/// Parse an imported `.3mf` archive, `Metadata/project_settings.config` JSON, or `OrcaSlicer.conf`
/// to populate either a 4-slot Single ACE Pro or 8-slot Double ACE Pro 2 loadout!
pub fn parse_ace_loadout_from_bytes_or_json(data: &[u8]) -> Result<ImportedAceLoadout, String> {
  let (json_text, webconfig_meta_val) = if data.len() >= 4 && &data[0..4] == b"PK\x03\x04" {
    let ps = extract_file_from_zip(data, "Metadata/project_settings.config").ok_or_else(|| {
      "ZIP/.3mf archive does not contain Metadata/project_settings.config".to_string()
    })?;
    let meta = extract_file_from_zip(data, "Metadata/orcaslicer_webconfig_loadout.json")
      .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
    (ps, meta)
  } else {
    (
      std::str::from_utf8(data)
        .map_err(|e| format!("Invalid UTF-8 in config file: {}", e))?
        .to_string(),
      None,
    )
  };

  let val: serde_json::Value =
    serde_json::from_str(&json_text).map_err(|e| format!("Failed to parse JSON loadout: {}", e))?;

  // Check if this is `OrcaSlicer.conf` with a `"presets"` block or a `project_settings.config`
  let ids_arr = val
    .get("filament_settings_id")
    .and_then(|v| v.as_array())
    .or_else(|| {
      val
        .get("presets")
        .and_then(|p| p.get("filaments"))
        .and_then(|v| v.as_array())
    });

  let Some(ids) = ids_arr else {
    return Err("No 'filament_settings_id' or 'presets.filaments' array found in file".to_string());
  };

  let count = ids.len();
  if count == 0 {
    return Err("Filament array in config is empty".to_string());
  }

  let types = webconfig_meta_val
    .as_ref()
    .and_then(|m| m.get("filament_material_type"))
    .and_then(|v| v.as_array())
    .or_else(|| val.get("filament_type").and_then(|v| v.as_array()));
  // Machine identity for per-slot `inherits`: `printer_settings_id` carries the machine
  // preset title ("OC - Bambu Lab X1 Carbon • 0.4 Ruby-Brass" or a stock Orca name); match
  // a known printer model inside it (longest match wins so "Kobra S1 Max" beats "Kobra S1")
  // so imported spools stop claiming Anycubic/Kobra inherits on other vendors' files.
  let printer_settings_id = val
    .get("printer_settings_id")
    .and_then(|v| v.as_str())
    .map(|s| s.to_string())
    .or_else(|| {
      val
        .get("presets")
        .and_then(|p| p.get("machine"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
    })
    .unwrap_or_default();
  let imported_printer_model = {
    let id_lower = printer_settings_id.to_lowercase();
    // Machine titles use the rig's SHORT model tag ("OC - X1C • 0.4 Ruby-Brass"), stock Orca
    // names use the full model — match either, longest needle wins (so "Kobra S1 Max"
    // beats "Kobra S1").
    let short_tag_of = |model: &str| -> String {
      let trimmed = model
        .trim_start_matches("Anycubic ")
        .trim_start_matches("Bambu Lab ")
        .trim_start_matches("Creality ")
        .trim_start_matches("Prusa ")
        .trim();
      if trimmed == "X1 Carbon" {
        "X1C".to_string()
      } else {
        trimmed.to_string()
      }
    };
    let mut needles: Vec<(String, String)> = Vec::new();
    for m in &crate::constants::constants().machines {
      needles.push((m.printer_model.clone(), m.printer_model.clone()));
      needles.push((short_tag_of(&m.printer_model), m.printer_model.clone()));
    }
    for m in &crate::catalog::load_merged_catalog().machines {
      needles.push((m.name.clone(), m.name.clone()));
      needles.push((short_tag_of(&m.name), m.name.clone()));
    }
    needles
      .into_iter()
      .filter(|(needle, _)| !needle.is_empty() && id_lower.contains(&needle.to_lowercase()))
      .max_by_key(|(needle, _)| needle.len())
      .map(|(_, model)| model)
  };
  let imported_nozzle_diam = printer_settings_id
    .rfind("• ")
    .and_then(|idx| {
      printer_settings_id[idx + "• ".len()..]
        .split_whitespace()
        .next()
        .map(|s| s.to_string())
    })
    .unwrap_or_else(|| "0.4".to_string());
  let colors = val.get("filament_colour").and_then(|v| v.as_array());
  let temps = val.get("nozzle_temperature").and_then(|v| v.as_array());
  let bed_temps = val.get("hot_plate_temp").and_then(|v| v.as_array());
  let flows = val.get("filament_flow_ratio").and_then(|v| v.as_array());
  let pas = val.get("pressure_advance").and_then(|v| v.as_array());
  let mvs = val
    .get("filament_max_volumetric_speed")
    .and_then(|v| v.as_array());
  let finishes = webconfig_meta_val
    .as_ref()
    .and_then(|m| m.get("filament_finish_type"))
    .and_then(|v| v.as_array())
    .or_else(|| val.get("filament_finish_type").and_then(|v| v.as_array()));
  let opacities = webconfig_meta_val
    .as_ref()
    .and_then(|m| m.get("filament_opacity"))
    .and_then(|v| v.as_array())
    .or_else(|| val.get("filament_opacity").and_then(|v| v.as_array()));
  let stickiness = webconfig_meta_val
    .as_ref()
    .and_then(|m| m.get("filament_pigment_stickiness"))
    .and_then(|v| v.as_array())
    .or_else(|| {
      val
        .get("filament_pigment_stickiness")
        .and_then(|v| v.as_array())
    });

  let mut spools = Vec::new();
  for idx in 0..count {
    let name = ids
      .get(idx)
      .and_then(|v| v.as_str())
      .unwrap_or("Imported Spool")
      .to_string();
    let mat = types
      .and_then(|a| a.get(idx))
      .and_then(|v| v.as_str())
      .unwrap_or("PLA")
      .to_string();
    let color_hex = colors
      .and_then(|a| a.get(idx))
      .and_then(|v| v.as_str())
      .unwrap_or("#06B6D4")
      .to_string();
    let print_temp = temps
      .and_then(|a| a.get(idx))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<i32>().ok())
      .unwrap_or(215);
    let bed_temp = bed_temps
      .and_then(|a| a.get(idx))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<i32>().ok())
      .unwrap_or(60);
    let flow_ratio = flows
      .and_then(|a| a.get(idx))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<f32>().ok())
      .unwrap_or(0.98);
    let pressure_advance = pas
      .and_then(|a| a.get(idx))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<f32>().ok())
      .unwrap_or(0.032);
    let max_vol = mvs
      .and_then(|a| a.get(idx))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<f32>().ok())
      .unwrap_or(16.0);
    let finish_type = finishes
      .and_then(|a| a.get(idx))
      .and_then(|v| v.as_str())
      .unwrap_or("Basic")
      .to_string();
    let opacity = opacities
      .and_then(|a| a.get(idx))
      .and_then(|v| v.as_f64())
      .map(|f| f as f32)
      .unwrap_or(1.0);
    let pigment_stickiness = stickiness
      .and_then(|a| a.get(idx))
      .and_then(|v| v.as_f64())
      .map(|f| f as f32)
      .unwrap_or(1.0);

    let c = crate::constants::constants();
    let detected_b = c.detect_brand_from_text(&name);
    let brand = if detected_b != "Generic" {
      detected_b.to_string()
    } else if name.starts_with("OC -") || name.starts_with("OC:") {
      "Generic".to_string()
    } else {
      "Anycubic".to_string()
    };

    let noz_cfg = c.lookup_nozzle(&name);
    let detected_noz = noz_cfg.id.clone();
    let poly_cfg = c.lookup_polymer(&mat);

    let hrcs = val.get("required_nozzle_HRC").and_then(|v| v.as_array());
    let req_hrc = hrcs
      .and_then(|a| a.get(idx))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<u32>().ok())
      .unwrap_or_else(|| {
        if FilamentSpool::is_abrasive_material_or_finish(&mat, &finish_type) {
          c.hardware_limits.abrasive_required_hrc
        } else {
          c.hardware_limits.non_abrasive_required_hrc
        }
      });

    let fan_mins = val.get("fan_min_speed").and_then(|v| v.as_array());
    let fan_pct = fan_mins
      .and_then(|a| a.get(idx))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<i32>().ok())
      .unwrap_or(poly_cfg.default_fan_pct);

    let brass_base = if noz_cfg.uses_hs_base_temp {
      (print_temp - 5).max(190)
    } else {
      print_temp - noz_cfg.temp_offset_from_brass_c
    };
    let hs_base = brass_base + 5;
    let mut sp = FilamentSpool {
      id: format!("imported_slot_{}", idx + 1),
      name,
      brand,
      inherits: String::new(),
      material_type: mat.clone(),
      nozzle_type: detected_noz.clone(),
      tuned_for_nozzle_type: detected_noz,
      required_nozzle_hrc: req_hrc,
      color_hex: color_hex.clone(),
      color_name: format!("Slot #{} ({})", idx + 1, color_hex),
      brass_base_temp: brass_base,
      hs_base_temp: hs_base,
      base_print_temp: brass_base,
      print_temp,
      base_bed_temp: bed_temp,
      bed_temp,
      flow_ratio,
      pressure_advance,
      max_volumetric_speed: max_vol,
      fan_speed_percent: fan_pct,
      remaining_grams: 850,
      is_loaded: idx < 8,
      finish_type,
      opacity,
      pigment_stickiness,
      drying_temp_c: poly_cfg.default_dry_temp_c,
      drying_time_hr: poly_cfg.default_dry_time_hr,
      is_rfid_synced: true,
      deltas_from_base: Vec::new(),
      raw_disk_json: None,
    };
    sp.inherits = match &imported_printer_model {
      Some(model) => crate::constants::constants().resolve_filament_inherits(
        model,
        &imported_nozzle_diam,
        &sp.material_type,
      ),
      None => resolve_kobra_s1_filament_inherits(&sp),
    };
    spools.push(sp);
  }

  let flush_matrix = val
    .get("flush_volumes_matrix")
    .and_then(|v| v.as_array())
    .map(|arr| {
      arr
        .iter()
        .map(|x| x.as_str().unwrap_or("0").to_string())
        .collect()
    });

  let ace_pro_units = if count > 4 { 2 } else { 1 };

  Ok(ImportedAceLoadout {
    ace_pro_units,
    spools,
    flush_matrix,
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::dir_import::{build_reference_anycubic_entries, import_scanned_directory};

  #[test]
  fn test_3mf_roundtrip_and_8slot_double_ace_pro2_import() {
    let rig = MachineRig::kobra_s1_hardened_steel_png();
    let proc = ProcessProfile::get_default_processes().remove(0);
    let entries = build_reference_anycubic_entries();
    let mut spools = import_scanned_directory(&entries).unwrap().filaments;
    while spools.len() < 8 {
      let mut dup = spools[0].clone();
      dup.id = format!("extra_slot_{}", spools.len() + 1);
      spools.push(dup);
    }
    let spools: Vec<FilamentSpool> = spools.into_iter().take(8).collect();
    let dummy_matrix: Vec<String> = (0..64)
      .map(|i| {
        if i % 9 == 0 {
          "0".to_string()
        } else {
          "320".to_string()
        }
      })
      .collect();
    let overrides = serde_json::Map::new();

    let zip_bytes = build_3mf_project_container(&rig, &proc, &spools, &dummy_matrix, &overrides);
    assert_eq!(&zip_bytes[0..4], b"PK\x03\x04");

    let imported = parse_ace_loadout_from_bytes_or_json(&zip_bytes).unwrap();
    assert_eq!(imported.ace_pro_units, 2);
    assert_eq!(imported.spools.len(), 8);
    assert!(!imported.spools[0].name.is_empty());
    assert_eq!(imported.flush_matrix.unwrap().len(), 64);

    // Verify that NO extra Metadata/*_settings_*.config files are embedded (which would mark user presets as is_external=true)
    for slot_idx in 1..=8 {
      assert!(
        extract_file_from_zip(
          &zip_bytes,
          &format!("Metadata/filament_settings_{}.config", slot_idx)
        )
        .is_none(),
        "Metadata/filament_settings_{}.config must not be embedded in .3mf",
        slot_idx
      );
      let fil_cfg = export_standalone_filament_json(&spools[slot_idx - 1], &rig);
      let fil_val: serde_json::Value = serde_json::from_str(&fil_cfg).unwrap();
      let fil_obj = fil_val.as_object().unwrap();
      assert!(
        !fil_obj.contains_key("filament_colour"),
        "Standalone filament preset must not contain filament_colour"
      );
      assert!(fil_obj.contains_key("nozzle_temperature_BRASS"));
      assert!(fil_obj.contains_key("nozzle_temperature_HS"));
      assert!(fil_obj.contains_key("fan_max_speed_BRASS"));
      assert!(fil_obj.contains_key("fan_max_speed_HS"));
      for (k, v) in fil_obj {
        assert!(
          v.is_string() || (v.is_array() && v.as_array().unwrap().iter().all(|x| x.is_string())),
          "Non-string/string-array key {} in standalone filament json: {:?}",
          k,
          v
        );
      }
    }
    assert!(extract_file_from_zip(&zip_bytes, "Metadata/machine_settings_1.config").is_none());
    assert!(extract_file_from_zip(&zip_bytes, "Metadata/process_settings_1.config").is_none());

    let mut noisy_overrides = serde_json::Map::new();
    noisy_overrides.insert(
      "fan_cooling_layer_time".to_string(),
      serde_json::json!(["30"]),
    );
    noisy_overrides.insert("z_hop_types".to_string(), serde_json::json!(["Slope"]));
    let ps_json =
      build_project_settings_config_json(&rig, &proc, &spools, &dummy_matrix, &noisy_overrides);
    let ps_val: serde_json::Value = serde_json::from_str(&ps_json).unwrap();
    let ps_map = ps_val.as_object().unwrap();
    assert!(
      !ps_map.contains_key("fan_cooling_layer_time"),
      "1-element filament array from base_profile must never leak into project_settings.config"
    );
    assert_eq!(ps_map["nozzle_diameter"], serde_json::json!(["0.4"]));
    assert_eq!(ps_map["printer_variant"], serde_json::json!("0.4"));
    assert_eq!(ps_map["outer_wall_line_width"], serde_json::json!("0.42"));
    assert_eq!(ps_map["wall_generator"], serde_json::json!("arachne"));
    assert_eq!(ps_map["seam_slope_type"], serde_json::json!("all"));
    assert_eq!(ps_map["seam_slope_steps"], serde_json::json!("15"));
    assert_eq!(ps_map["seam_slope_min_length"], serde_json::json!("15"));
    assert_eq!(ps_map["sparse_infill_pattern"], serde_json::json!("gyroid"));
    assert_eq!(ps_map["filament_diameter"].as_array().unwrap().len(), 8);
    assert_eq!(ps_map["default_filament_colour"], ps_map["filament_colour"]);
    assert_eq!(
      ps_map["nozzle_temperature_BRASS"],
      ps_map["nozzle_temperature"]
    );
    assert_eq!(
      ps_map["nozzle_temperature_HS"],
      ps_map["nozzle_temperature"]
    );
    assert_eq!(ps_map["fan_max_speed_BRASS"], ps_map["fan_max_speed"]);
    assert_eq!(ps_map["fan_max_speed_HS"], ps_map["fan_max_speed"]);
    assert_eq!(ps_map["inherits_group"].as_array().unwrap().len(), 10);

    let model_xml = extract_file_from_zip(&zip_bytes, "3D/3dmodel.model").unwrap();
    assert!(model_xml.contains("BambuStudio-02.03.00.03"));

    assert_eq!(
      ps_map["different_settings_to_system"]
        .as_array()
        .unwrap()
        .len(),
      10
    );
    for (k, v) in ps_map {
      assert!(
        v.is_string() || (v.is_array() && v.as_array().unwrap().iter().all(|x| x.is_string())),
        "Key {} in project_settings.config is not string or Vec<String>: {:?}",
        k,
        v
      );
    }

    let mach_json = build_machine_preset_json(&rig);
    let mach_val: serde_json::Value = serde_json::from_str(&mach_json).unwrap();
    assert_eq!(mach_val["version"], serde_json::json!("0.1.0.0"));
    assert_eq!(mach_val["nozzle_hrc"], serde_json::json!("55"));
    assert!(mach_val["printer_notes"]
      .as_str()
      .unwrap()
      .contains("[OrcaSlicer WebConfig v0.1.0 — Machine]"));

    // Verify all 5 default OC process presets export their exact configured speeds without spurious clamping or leaked internal_solid_infill_speed
    let mut tropical_overrides = serde_json::Map::new();
    tropical_overrides.insert("seam_slope_type".to_string(), serde_json::json!("all"));
    tropical_overrides.insert("outer_wall_speed".to_string(), serde_json::json!("151"));
    tropical_overrides.insert("inner_wall_speed".to_string(), serde_json::json!("151"));
    tropical_overrides.insert("sparse_infill_speed".to_string(), serde_json::json!("151"));
    tropical_overrides.insert(
      "internal_solid_infill_speed".to_string(),
      serde_json::json!("213"),
    );
    tropical_overrides.insert("initial_layer_speed".to_string(), serde_json::json!("18"));
    let expected_procs = [
      (
        "OC - 0.20mm Standard @Kobra S1",
        "0.20",
        "200",
        "300",
        "270",
        "50",
        "10000",
      ),
      (
        "OC - 0.20mm Quality @Kobra S1",
        "0.20",
        "60",
        "150",
        "250",
        "50",
        "5000",
      ),
      (
        "OC - 0.20mm Speed @Kobra S1",
        "0.20",
        "300",
        "350",
        "350",
        "60",
        "15000",
      ),
      (
        "OC - 0.12mm Quality @Kobra S1",
        "0.12",
        "60",
        "150",
        "200",
        "50",
        "5000",
      ),
      (
        "OC - 0.28mm Speed @Kobra S1",
        "0.28",
        "250",
        "320",
        "320",
        "60",
        "12000",
      ),
    ];
    for (p, (exp_name, exp_lh, exp_ow, exp_iw, exp_si, exp_il, exp_acc)) in
      ProcessProfile::get_default_processes()
        .iter()
        .zip(expected_procs.iter())
    {
      assert_eq!(&p.name, exp_name);
      let pj = build_process_preset_json(&p, &rig, &tropical_overrides);
      let pv: serde_json::Value = serde_json::from_str(&pj).unwrap();
      assert_eq!(pv["layer_height"], serde_json::json!(*exp_lh));
      assert_eq!(pv["outer_wall_speed"], serde_json::json!(*exp_ow));
      assert_eq!(pv["inner_wall_speed"], serde_json::json!(*exp_iw));
      assert_eq!(pv["sparse_infill_speed"], serde_json::json!(*exp_si));
      assert_eq!(pv["initial_layer_speed"], serde_json::json!(*exp_il));
      assert_eq!(pv["default_acceleration"], serde_json::json!(*exp_acc));
      assert!(
        pv.get("internal_solid_infill_speed").is_none(),
        "Process {} must not leak internal_solid_infill_speed from derived_overrides",
        exp_name
      );
    }
  }

  #[test]
  fn test_tpu_for_ams_and_petg_temp_range_and_brand_preserving_and_pha_export() {
    let catalog = crate::catalog::load_merged_catalog();
    // 1. Verify PLA is the very first filament in Generic (never PHA)
    let first_generic = catalog
      .filaments
      .iter()
      .find(|f| f.brand == "Generic")
      .expect("Generic brand in catalog");
    assert_eq!(first_generic.code, "PLA");

    // 2. Verify selecting PETG and switching across all brands keeps PETG (never resets to first filament of brand)
    let petg_entry = catalog
      .filaments
      .iter()
      .find(|f| f.brand == "Generic" && f.code == "PETG")
      .expect("Generic PETG in catalog");
    let mut spool = FilamentSpool::from_catalog_entry(1, petg_entry, "hardened_steel");
    assert_eq!(
      FilamentSpool::canonical_polymer_key(&spool.material_type),
      "PETG"
    );

    for brand in [
      "SUNLU",
      "Polymaker",
      "Anycubic",
      "Bambu Lab",
      "eSUN",
      "Overture",
      "Siraya Tech",
      "Generic",
    ] {
      spool.apply_brand_change(brand, "hardened_steel");
      assert_eq!(
        spool.brand, brand,
        "Brand should update to '{}' when switching brand on PETG",
        brand
      );
      assert_eq!(
        FilamentSpool::canonical_polymer_key(&spool.material_type),
        "PETG",
        "Switching brand to '{}' must preserve PETG polymer (got '{}')",
        brand,
        spool.material_type
      );
    }

    // 3. Verify TPU for AMS + PETG exported presets & .3mf project_settings.config have mutually compatible nozzle_temperature_range_low/high
    let tpu_ams_entry = catalog
      .filaments
      .iter()
      .find(|f| f.code == "TPU for AMS")
      .expect("TPU for AMS in catalog");
    let tpu_ams_spool = FilamentSpool::from_catalog_entry(1, tpu_ams_entry, "hardened_steel");
    let petg_spool = FilamentSpool::from_catalog_entry(2, petg_entry, "hardened_steel");
    let rig = MachineRig::kobra_s1_hardened_steel_png();

    let tpu_json: serde_json::Value =
      serde_json::from_str(&export_standalone_filament_json(&tpu_ams_spool, &rig)).unwrap();
    let petg_json: serde_json::Value =
      serde_json::from_str(&export_standalone_filament_json(&petg_spool, &rig)).unwrap();

    let tpu_temp: i32 = tpu_json["nozzle_temperature"][0]
      .as_str()
      .unwrap()
      .parse()
      .unwrap();
    let petg_temp: i32 = petg_json["nozzle_temperature"][0]
      .as_str()
      .unwrap()
      .parse()
      .unwrap();
    let tpu_low: i32 = tpu_json["nozzle_temperature_range_low"][0]
      .as_str()
      .unwrap()
      .parse()
      .unwrap();
    let tpu_high: i32 = tpu_json["nozzle_temperature_range_high"][0]
      .as_str()
      .unwrap()
      .parse()
      .unwrap();
    let petg_low: i32 = petg_json["nozzle_temperature_range_low"][0]
      .as_str()
      .unwrap()
      .parse()
      .unwrap();
    let petg_high: i32 = petg_json["nozzle_temperature_range_high"][0]
      .as_str()
      .unwrap()
      .parse()
      .unwrap();

    assert!(
      tpu_temp >= petg_low && tpu_temp <= petg_high,
      "TPU for AMS temp ({}°C) must be inside PETG range [{}..{}°C]",
      tpu_temp,
      petg_low,
      petg_high
    );
    assert!(
      petg_temp >= tpu_low && petg_temp <= tpu_high,
      "PETG temp ({}°C) must be inside TPU for AMS range [{}..{}°C]",
      petg_temp,
      tpu_low,
      tpu_high
    );

    // 4. Verify importing OC - PHA • 0.4 Hardened.json produces zero conflicts and zero safety warnings
    let pha_entry = catalog
      .filaments
      .iter()
      .find(|f| f.code == "PHA")
      .expect("PHA in catalog");
    let pha_spool = FilamentSpool::from_catalog_entry(1, pha_entry, "hardened_steel");
    let pha_disk_json = export_standalone_filament_json(&pha_spool, &rig);
    let scanned = vec![crate::fs_access::ScannedFileEntry {
      relative_path: "user/434139/filament/OC - PHA • 0.4 Hardened.json".to_string(),
      filename: "OC - PHA • 0.4 Hardened.json".to_string(),
      bytes: pha_disk_json.into_bytes(),
    }];
    let imported = crate::dir_import::import_scanned_directory(&scanned).unwrap();
    assert!(
      imported.conflicts.is_empty(),
      "OC - PHA • 0.4 Hardened must not trigger any external conflicts: {:?}",
      imported.conflicts
    );
    let warnings = crate::transformations::TransformationEngine::evaluate_safety_warnings(
      &rig,
      &imported.filaments,
      &imported.base_profile,
      &imported.conflicts,
      &imported.disk_flush_matrix,
    );
    assert!(
      warnings.is_empty(),
      "OC - PHA • 0.4 Hardened must not trigger any safety warnings at export: {:?}",
      warnings
    );
  }

  #[test]
  fn test_per_process_lua_layer_height_and_speed_scaling_across_all_5_presets() {
    let rig = MachineRig::kobra_s1_hardened_steel_png();
    let procs = ProcessProfile::get_default_processes();
    let active_proc = &procs[0]; // 0.20mm Standard @Kobra S1 (layer_height=0.20, outer_wall_speed=200, sparse_infill_speed=270)

    let mut base = crate::orca_profile::OrcaProfile::get_stock_kobra_s1();
    base.set_str("layer_height", &format!("{:.2}", active_proc.layer_height));
    base.set_str(
      "outer_wall_speed",
      &format!("{:.0}", active_proc.outer_wall_speed),
    );
    base.set_str(
      "inner_wall_speed",
      &format!("{:.0}", active_proc.inner_wall_speed),
    );
    base.set_str(
      "sparse_infill_speed",
      &format!("{:.0}", active_proc.sparse_infill_speed),
    );

    let mut rules = crate::transformations::TransformationEngine::get_default_rules();
    rules.push(crate::transformations::TransformationRule {
      id: "rule_test_support_and_speed".to_string(),
      name: "Test Support Z-Gap & Speed Scaling".to_string(),
      description: "Scales outer_wall_speed by 0.75x, sparse_infill_speed by 0.80x, and sets support_top_z_distance = layer_height + 0.04".to_string(),
      category: "Process".to_string(),
      icon: "tune".to_string(),
      enabled: true,
      priority: 95,
      scope: crate::transformations::RuleScopeFilter::default(),
      condition_expr: "true".to_string(),
      lua_code: r#"
function apply(config, ctx)
  local lh = tonumber(config["layer_height"]) or 0.20
  config["support_top_z_distance"] = string.format("%.2f", lh + 0.04)
  config["top_shell_thickness"] = string.format("%.2f", lh * 6.0)
  local ow = tonumber(config["outer_wall_speed"]) or 200
  config["outer_wall_speed"] = tostring(math.floor(ow * 0.75 + 0.5))
  local si = tonumber(config["sparse_infill_speed"]) or 270
  config["sparse_infill_speed"] = tostring(math.floor(si * 0.80 + 0.5))
end
"#.to_string(),
    });

    let ctx = serde_json::json!({
      "printer_model": rig.printer_model,
      "nozzle_type": rig.nozzle_type,
      "nozzle_material": rig.nozzle_material,
      "nozzle_diameter": rig.nozzle_diameter,
      "hotend_max_flow": rig.max_volumetric_flow_limit,
      "filament_max_flow": 18.0,
      "layer_height": active_proc.layer_height,
      "line_width": active_proc.line_width,
    });

    let eval_res =
      crate::transformations::TransformationEngine::evaluate_stack(&base, &rules, &ctx);
    let derived_overrides = &eval_res.resulting_profile.values;

    // 1. 0.20mm Standard (base outer=200 -> 150, base infill=270 -> 216, lh=0.20 -> zd=0.24, top_thick=1.20)
    let p_020_std: serde_json::Value = serde_json::from_str(&build_process_preset_json(
      &procs[0],
      &rig,
      derived_overrides,
    ))
    .unwrap();
    assert_eq!(
      p_020_std["support_top_z_distance"],
      serde_json::json!("0.24")
    );
    assert_eq!(p_020_std["top_shell_thickness"], serde_json::json!("1.20"));
    assert_eq!(p_020_std["outer_wall_speed"], serde_json::json!("150"));
    assert_eq!(p_020_std["sparse_infill_speed"], serde_json::json!("216"));

    // 2. 0.12mm Quality (base outer=60 -> 45, base infill=200 -> 160, lh=0.12 -> zd=0.16, top_thick=0.72)
    let p_012_hq: serde_json::Value = serde_json::from_str(&build_process_preset_json(
      &procs[3],
      &rig,
      derived_overrides,
    ))
    .unwrap();
    assert_eq!(
      p_012_hq["support_top_z_distance"],
      serde_json::json!("0.16")
    );
    assert_eq!(p_012_hq["top_shell_thickness"], serde_json::json!("0.72"));
    assert_eq!(p_012_hq["outer_wall_speed"], serde_json::json!("45"));
    assert_eq!(p_012_hq["sparse_infill_speed"], serde_json::json!("160"));

    // 3. 0.28mm Speed (base outer=250 -> 188, base infill=320 -> 256, lh=0.28 -> zd=0.32, top_thick=1.68)
    let p_028_spd: serde_json::Value = serde_json::from_str(&build_process_preset_json(
      &procs[4],
      &rig,
      derived_overrides,
    ))
    .unwrap();
    assert_eq!(
      p_028_spd["support_top_z_distance"],
      serde_json::json!("0.32")
    );
    assert_eq!(p_028_spd["top_shell_thickness"], serde_json::json!("1.68"));
    assert_eq!(p_028_spd["outer_wall_speed"], serde_json::json!("188"));
    assert_eq!(p_028_spd["sparse_infill_speed"], serde_json::json!("256"));
  }

  #[test]
  fn test_filament_process_machine_scope_isolation_and_heterogeneous_slots() {
    let catalog = crate::catalog::load_merged_catalog();
    let mut rig = MachineRig::kobra_s1_hardened_steel_png();
    rig.nozzle_type = "diamondback".to_string();
    rig.nozzle_material = "diamond".to_string();
    rig.hotend = "FlowTech (All-Metal)".to_string();
    rig.melt_zone_multiplier = 1.35;

    let proc = ProcessProfile::get_default_processes().remove(0);

    // Build a heterogeneous 4-slot ACE Pro loadout: PLA, PETG, ASA, TPU for AMS
    let mut spools = Vec::new();
    for (idx, code) in ["PLA", "PETG", "ASA", "TPU for AMS"].iter().enumerate() {
      let entry = catalog
        .filaments
        .iter()
        .find(|f| f.code == *code)
        .unwrap_or_else(|| panic!("Missing {} in catalog", code));
      let mut sp = FilamentSpool::from_catalog_entry(idx + 1, entry, "diamondback");
      sp.id = format!("slot_{}", idx + 1);
      spools.push(sp);
    }

    // Simulate a Lua evaluation output containing mixed Process, Machine, and Filament keys
    // (including `overhang_fan_speed` from `bridge_overhang_tuning` and `nozzle_temperature` /
    // `filament_retraction_length` from `thermal_conductivity` + `symptom_stringing_diamondback`).
    let mut mixed_overrides = serde_json::Map::new();
    mixed_overrides.insert("nozzle_temperature".to_string(), serde_json::json!(["202"]));
    mixed_overrides.insert(
      "nozzle_temperature_initial_layer".to_string(),
      serde_json::json!(["212"]),
    );
    mixed_overrides.insert("hot_plate_temp".to_string(), serde_json::json!(["60"]));
    mixed_overrides.insert("chamber_temperature".to_string(), serde_json::json!(["45"]));
    mixed_overrides.insert(
      "filament_retraction_length".to_string(),
      serde_json::json!(["0.50"]),
    );
    mixed_overrides.insert(
      "filament_retraction_speed".to_string(),
      serde_json::json!(["44"]),
    );
    mixed_overrides.insert("filament_z_hop".to_string(), serde_json::json!(["0.15"]));
    mixed_overrides.insert("overhang_fan_speed".to_string(), serde_json::json!("100"));
    mixed_overrides.insert("bridge_flow".to_string(), serde_json::json!("0.92"));
    mixed_overrides.insert("bridge_speed".to_string(), serde_json::json!("28"));
    mixed_overrides.insert("overhang_4_4_speed".to_string(), serde_json::json!("15"));
    mixed_overrides.insert(
      "_lua_touched_keys".to_string(),
      serde_json::json!(
        "nozzle_temperature;filament_retraction_length;overhang_fan_speed;bridge_flow;bridge_speed;overhang_4_4_speed"
      ),
    );

    let proc_json = build_process_preset_json(&proc, &rig, &mixed_overrides);
    let proc_val: serde_json::Value = serde_json::from_str(&proc_json).unwrap();
    let proc_obj = proc_val.as_object().unwrap();

    // 1. Process preset keeps true Process keys (`bridge_flow`, `bridge_speed`, `overhang_4_4_speed`)
    //    and rejects ALL Filament-scoped and Machine-scoped keys (`overhang_fan_speed`, `nozzle_temperature`, etc.)
    assert_eq!(proc_obj.get("bridge_flow"), Some(&serde_json::json!("0.92")));
    assert_eq!(proc_obj.get("bridge_speed"), Some(&serde_json::json!("28")));
    assert_eq!(
      proc_obj.get("overhang_4_4_speed"),
      Some(&serde_json::json!("15"))
    );
    for k in proc_obj.keys() {
      assert!(
        !is_filament_scoped_key(k),
        "Process preset must NEVER contain filament-scoped key '{}'",
        k
      );
      assert!(
        !is_machine_scoped_key(k),
        "Process preset must NEVER contain machine-scoped key '{}'",
        k
      );
    }

    // 2. Machine preset contains zero Filament-scoped keys
    let mach_json = build_machine_preset_json_with_overrides(&rig, Some(&mixed_overrides));
    let mach_val: serde_json::Value = serde_json::from_str(&mach_json).unwrap();
    let mach_obj = mach_val.as_object().unwrap();
    for k in mach_obj.keys() {
      assert!(
        !is_filament_scoped_key(k),
        "Machine preset must NEVER contain filament-scoped key '{}'",
        k
      );
    }

    // 3. 3MF `project_settings.config` evaluates each slot independently via the per-spool Lua pipeline
    //    without Slot 1's PLA `nozzle_temperature` (202°C) overwriting Slot 2 (PETG) or Slot 3 (ASA)!
    let flush_matrix = vec!["0".to_string(); 16];
    let proj_json =
      build_project_settings_config_json(&rig, &proc, &spools, &flush_matrix, &mixed_overrides);
    let proj_val: serde_json::Value = serde_json::from_str(&proj_json).unwrap();
    let noz_temps = proj_val["nozzle_temperature"].as_array().unwrap();
    assert_eq!(noz_temps.len(), 4);
    let pla_t: i32 = noz_temps[0].as_str().unwrap().parse().unwrap();
    let petg_t: i32 = noz_temps[1].as_str().unwrap().parse().unwrap();
    let asa_t: i32 = noz_temps[2].as_str().unwrap().parse().unwrap();
    assert!(
      pla_t < petg_t && petg_t < asa_t,
      "Expected independent per-slot DiamondBack temperatures PLA ({}°C) < PETG ({}°C) < ASA ({}°C)",
      pla_t,
      petg_t,
      asa_t
    );
  }
}
