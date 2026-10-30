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

/// Resolve a guaranteed-valid system filament preset name for Anycubic Kobra S1 0.4 nozzle
pub fn resolve_kobra_s1_filament_inherits(spool: &FilamentSpool) -> String {
  let is_oc = spool.name.starts_with("OC -") || spool.name.starts_with("OC:");
  if spool.id.starts_with("disk_")
    && !is_oc
    && !spool.inherits.trim().is_empty()
    && !spool.inherits.contains("TPU for ACE @Anycubic Kobra S1 ")
    && !spool.inherits.contains("TPU 95A @Anycubic Kobra S1 ")
    && !spool.inherits.contains("PCTG @Anycubic Kobra S1 ")
  {
    return spool.inherits.clone();
  }
  match spool.material_type.trim() {
    "PLA+" => return "Anycubic PLA+ @Anycubic Kobra S1 0.4 nozzle".to_string(),
    "PLA Matte" => return "Anycubic PLA Matte @Anycubic Kobra S1 0.4 nozzle".to_string(),
    "PLA Silk" => return "Anycubic PLA Silk @Anycubic Kobra S1 0.4 nozzle".to_string(),
    "PLA Glow" => return "Anycubic PLA Glow @Anycubic Kobra S1 0.4 nozzle".to_string(),
    "PLA High Speed" => return "Anycubic PLA High Speed @Anycubic Kobra S1 0.4 nozzle".to_string(),
    _ => {}
  }
  match FilamentSpool::canonical_polymer_key(&spool.material_type) {
    "PLA+" => "Anycubic PLA+ @Anycubic Kobra S1 0.4 nozzle".to_string(),
    "PLA-CF" => "Anycubic PLA-CF @Anycubic Kobra S1 0.4 nozzle".to_string(),
    "PETG-CF" | "PCTG-CF" => "Anycubic PETG-CF @Anycubic Kobra S1 0.4 nozzle".to_string(),
    "PET-CF" => "Anycubic PET-CF @Anycubic Kobra S1 0.4 nozzle".to_string(),
    "PETG" | "PCTG" => "Anycubic PETG @Anycubic Kobra S1 0.4 nozzle".to_string(),
    "TPU" | "TPU for AMS" | "TPU 95A" | "TPU 90A" | "TPU 85A" => {
      "Anycubic TPU @Anycubic Kobra S1 0.4 nozzle".to_string()
    }
    "PEBA" => "Anycubic PEBA @Anycubic Kobra S1 0.4 nozzle".to_string(),
    "ASA" | "ASA-CF" => "Anycubic ASA @Anycubic Kobra S1 0.4 nozzle".to_string(),
    "ABS" | "ABS-CF" => "Anycubic ABS @Anycubic Kobra S1 0.4 nozzle".to_string(),
    "PC" | "PC-CF" => "Anycubic PC @Anycubic Kobra S1 0.4 nozzle".to_string(),
    "PA" | "PA6-CF" | "PA12-CF" | "PAHT-CF" | "PPA-CF" => {
      "Anycubic PA6-CF @Anycubic Kobra S1 0.4 nozzle".to_string()
    }
    _ => "Anycubic PLA @Anycubic Kobra S1 0.4 nozzle".to_string(),
  }
}

/// Map internal material codes to OrcaSlicer / AnycubicSlicerNext valid `filament_type` enum strings
pub fn resolve_orca_filament_type(material_type: &str) -> String {
  match FilamentSpool::canonical_polymer_key(material_type) {
    "PLA+" => "PLA".to_string(),
    "PLA-CF" => "PLA-CF".to_string(),
    "PCTG" | "PETG" => "PETG".to_string(),
    "PCTG-CF" | "PETG-CF" => "PETG-CF".to_string(),
    "PET-CF" => "PET-CF".to_string(),
    "TPU" | "TPU for AMS" | "TPU 95A" | "TPU 90A" | "TPU 85A" => "TPU".to_string(),
    "PEBA" => "PEBA".to_string(),
    "ASA" | "ASA-CF" => "ASA".to_string(),
    "ABS" | "ABS-CF" => "ABS".to_string(),
    "PC" | "PC-CF" => "PC".to_string(),
    "PA6-CF" | "PA12-CF" => "PA6-CF".to_string(),
    "PAHT-CF" | "PPA-CF" => "PAHT-CF".to_string(),
    "PA" => "PA".to_string(),
    "PVA" => "PVA".to_string(),
    "BVOH" => "BVOH".to_string(),
    "HIPS" => "HIPS".to_string(),
    "PHA" => "PHA".to_string(),
    "PCL" => "PCL".to_string(),
    other => other.to_string(),
  }
}

const FILAMENT_DIFF_KEYS: &str = "default_filament_colour;fan_max_speed;fan_max_speed_BRASS;fan_max_speed_HS;fan_min_speed;fan_min_speed_BRASS;fan_min_speed_HS;filament_deretraction_speed;filament_diameter;filament_flow_ratio;filament_max_volumetric_speed;filament_notes;filament_retract_before_wipe;filament_retraction_length;filament_retraction_speed;filament_type;filament_wipe;filament_wipe_distance;filament_z_hop;filament_z_hop_types;hot_plate_temp;hot_plate_temp_initial_layer;nozzle_temperature;nozzle_temperature_BRASS;nozzle_temperature_HS;nozzle_temperature_initial_layer;nozzle_temperature_initial_layer_BRASS;nozzle_temperature_initial_layer_HS;nozzle_temperature_range_high;nozzle_temperature_range_low;pressure_advance;required_nozzle_HRC;textured_plate_temp;textured_plate_temp_initial_layer";

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
  resolve_spool_orca_filament_map_with_rig(spool, None)
}

pub fn resolve_spool_orca_filament_map_with_rig(
  spool: &FilamentSpool,
  rig_opt: Option<&MachineRig>,
) -> serde_json::Map<String, serde_json::Value> {
  let cat = crate::catalog::load_merged_catalog();
  let orca_fil_type = resolve_orca_filament_type(&spool.material_type);
  let is_pctg = spool.material_type.contains("PCTG") || spool.name.to_uppercase().contains("PCTG");
  let is_pctg_or_petg = is_pctg || orca_fil_type.contains("PETG") || orca_fil_type.contains("PET");
  let is_tpu = orca_fil_type.contains("TPU") || orca_fil_type.contains("PEBA");
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
  let mut climate_noz_delta: i32 = 0;
  let mut climate_bed_delta: i32 = 0;
  let mut climate_init_bed_extra: i32 = 0;
  let mut climate_retract_delta: f64 = 0.0;
  if let Some(rig) = rig_opt {
    let temp_c = rig.ambient_temp_c;
    let rh_pct = rig.humidity_pct;
    let is_hot =
      temp_c >= 26 || rig.climate_preset == "tropical" || rig.climate_preset == "hot_dry";
    let is_cold =
      temp_c <= 18 || rig.climate_preset == "cold_dry" || rig.climate_preset == "cold_humid";
    let is_humid =
      rh_pct >= 60 || rig.climate_preset == "tropical" || rig.climate_preset == "cold_humid";
    let is_ms = rig.is_heat_creep_resistant_nozzle();

    if is_humid {
      climate_retract_delta = if is_pctg_or_petg || is_tpu {
        0.20
      } else {
        0.15
      };
    }
    if is_hot {
      climate_noz_delta = if is_ms {
        -5
      } else if is_pctg_or_petg || orca_fil_type == "PLA" {
        -10
      } else if is_tpu {
        -6
      } else {
        -4
      };
      climate_bed_delta = if is_pctg_or_petg {
        -3
      } else if orca_fil_type == "PLA" || is_tpu {
        -5
      } else {
        0
      };
    } else if is_cold {
      climate_noz_delta = if is_ms { 4 } else { 6 } - if is_humid { 3 } else { 0 };
      climate_bed_delta = 5;
      climate_init_bed_extra = 5;
    } else if is_humid {
      climate_noz_delta = -3;
    }
  }

  let base_noz = first_str_from_map(&base_map, "nozzle_temperature")
    .and_then(|s| s.parse::<i32>().ok())
    .unwrap_or(spool.print_temp);
  let base_init = first_str_from_map(&base_map, "nozzle_temperature_initial_layer")
    .and_then(|s| s.parse::<i32>().ok())
    .unwrap_or(base_noz);
  let init_delta = if is_pctg_or_petg || is_tpu {
    0
  } else {
    (base_init - base_noz).clamp(0, 10)
  };
  let effective_temp = (spool.print_temp + climate_noz_delta).clamp(175, 350);
  let initial_temp = (effective_temp + init_delta).clamp(175, 350);
  let effective_bed = (spool.bed_temp + climate_bed_delta).clamp(35, 120);
  let initial_bed = (effective_bed + climate_init_bed_extra).clamp(35, 120);
  let export_name = spool.format_concise_title();
  let inherits = resolve_kobra_s1_filament_inherits(spool);

  // Ensure retraction, wipe, z-hop, and fan_min_speed / fan_max_speed are always populated cleanly
  let base_ret_len = first_str_from_map(&base_map, "filament_retraction_length")
    .and_then(|s| s.parse::<f64>().ok())
    .unwrap_or(if is_pctg {
      1.00
    } else if is_pctg_or_petg {
      0.80
    } else {
      0.60
    });
  let effective_ret_len = if is_pctg && base_ret_len < 1.00 {
    (1.00 + climate_retract_delta).clamp(0.30, 1.45)
  } else {
    (base_ret_len + climate_retract_delta).clamp(0.30, 1.45)
  };
  let ret_len_str = if (effective_ret_len - effective_ret_len.round()).abs() < 1e-6 {
    format!("{:.1}", effective_ret_len)
  } else {
    format!("{:.2}", effective_ret_len)
      .trim_end_matches('0')
      .trim_end_matches('.')
      .to_string()
  };
  let ret_spd_str = if is_pctg {
    "45".to_string()
  } else {
    first_str_from_map(&base_map, "filament_retraction_speed")
      .unwrap_or_else(|| if is_pctg_or_petg { "40" } else { "40" }.to_string())
  };
  let fan_min_pct = spool.fan_speed_percent.clamp(0, 100);
  let fan_max_pct = if is_pctg_or_petg && fan_min_pct >= 30 {
    90
  } else {
    fan_min_pct
  };

  base_map.insert("inherits".to_string(), serde_json::json!(inherits));
  base_map.insert("name".to_string(), serde_json::json!(export_name));
  base_map.insert(
    "filament_settings_id".to_string(),
    serde_json::json!([export_name]),
  );
  base_map.insert(
    "filament_type".to_string(),
    serde_json::json!([orca_fil_type]),
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
  base_map.insert(
    "hot_plate_temp".to_string(),
    serde_json::json!([effective_bed.to_string()]),
  );
  base_map.insert(
    "hot_plate_temp_initial_layer".to_string(),
    serde_json::json!([initial_bed.to_string()]),
  );
  base_map.insert(
    "textured_plate_temp".to_string(),
    serde_json::json!([effective_bed.to_string()]),
  );
  base_map.insert(
    "textured_plate_temp_initial_layer".to_string(),
    serde_json::json!([initial_bed.to_string()]),
  );
  base_map.insert(
    "filament_flow_ratio".to_string(),
    serde_json::json!([format!("{:.2}", spool.flow_ratio)]),
  );
  base_map.insert(
    "pressure_advance".to_string(),
    serde_json::json!([format!("{:.3}", spool.pressure_advance)]),
  );
  base_map.insert(
    "filament_max_volumetric_speed".to_string(),
    serde_json::json!([format!("{:.1}", spool.max_volumetric_speed)]),
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
  if is_pctg_or_petg {
    base_map.insert(
      "filament_retract_before_wipe".to_string(),
      serde_json::json!(["0%"]),
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
    if !base_map.contains_key("filament_z_hop") {
      base_map.insert("filament_z_hop".to_string(), serde_json::json!(["0.15"]));
    }
    if !base_map.contains_key("filament_z_hop_types") {
      base_map.insert(
        "filament_z_hop_types".to_string(),
        serde_json::json!(["Spiral Lift"]),
      );
    }
  }
  base_map.insert(
    "filament_notes".to_string(),
    serde_json::json!([spool.generate_descriptive_notes()]),
  );
  base_map.insert(
    "version".to_string(),
    serde_json::json!(WEBCONFIG_ORCA_PRESET_VERSION),
  );
  base_map
}

fn build_filament_preset_json_with_source(
  spool: &FilamentSpool,
  rig: &MachineRig,
  from_src: &str,
) -> String {
  let resolved = resolve_spool_orca_filament_map_with_rig(spool, Some(rig));
  let mut out = serde_json::Map::new();
  out.insert("type".to_string(), serde_json::json!("filament"));
  out.insert("from".to_string(), serde_json::json!(from_src));
  out.insert("is_custom_defined".to_string(), serde_json::json!("0"));

  for key in [
    "inherits",
    "name",
    "filament_settings_id",
    "compatible_printers",
    "filament_type",
    "filament_diameter",
    "required_nozzle_HRC",
    "default_filament_colour",
    "nozzle_temperature",
    "nozzle_temperature_BRASS",
    "nozzle_temperature_HS",
    "nozzle_temperature_initial_layer",
    "nozzle_temperature_initial_layer_BRASS",
    "nozzle_temperature_initial_layer_HS",
    "nozzle_temperature_range_low",
    "nozzle_temperature_range_high",
    "hot_plate_temp",
    "hot_plate_temp_initial_layer",
    "textured_plate_temp",
    "textured_plate_temp_initial_layer",
    "filament_flow_ratio",
    "pressure_advance",
    "filament_max_volumetric_speed",
    "fan_min_speed",
    "fan_min_speed_BRASS",
    "fan_min_speed_HS",
    "fan_max_speed",
    "fan_max_speed_BRASS",
    "fan_max_speed_HS",
    "filament_retraction_length",
    "filament_retraction_speed",
    "filament_z_hop",
    "filament_z_hop_types",
    "filament_retract_before_wipe",
    "filament_deretraction_speed",
    "filament_wipe",
    "filament_wipe_distance",
    "filament_notes",
    "version",
  ] {
    if let Some(v) = resolved.get(key) {
      out.insert(key.to_string(), v.clone());
    }
  }

  serde_json::to_string_pretty(&serde_json::Value::Object(out)).unwrap_or_else(|_| "{}".to_string())
}

/// Export a standalone OrcaSlicer Filament `.json` delta preset (Section 4.A)
pub fn export_standalone_filament_json(spool: &FilamentSpool, rig: &MachineRig) -> String {
  build_filament_preset_json_with_source(spool, rig, "User")
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
      "melt_zone_multiplier": rig.melt_zone_multiplier,
      "layer_height": process.layer_height,
      "line_width": process.line_width,
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

  for (k, v) in derived_overrides {
    // Do not let single-process/single-filament speed caps from `derived_overrides` overwrite
    // each distinct ProcessProfile's own speeds (`outer_wall_speed`, `inner_wall_speed`, `sparse_infill_speed`,
    // `internal_solid_infill_speed`, `initial_layer_speed`, `default_acceleration`), unless Optical Glass mode is active.
    // Also prevent low travel_acceleration (< 8000 mm/s²) from causing stringing during travel departures.
    if !is_optical_glass_mode
      && matches!(
        k.as_str(),
        "layer_height"
          | "line_width"
          | "outer_wall_line_width"
          | "inner_wall_line_width"
          | "sparse_infill_line_width"
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
    if k == "travel_acceleration" {
      if let Some(ta) = v.as_str().and_then(|s| s.parse::<f64>().ok()) {
        if ta < 8000.0 {
          effective.insert(k.clone(), serde_json::json!("10000"));
          continue;
        }
      }
    }
    effective.insert(k.clone(), v.clone());
  }
  if !is_optical_glass_mode {
    // Preserve each ProcessProfile's exact configured speeds and acceleration without artificial hotend clamping
    // (OrcaSlicer enforces each filament's `filament_max_volumetric_speed` dynamically at slice time).
    for (key, raw_spd) in [
      ("outer_wall_speed", process.outer_wall_speed),
      ("inner_wall_speed", process.inner_wall_speed),
      ("sparse_infill_speed", process.sparse_infill_speed),
      ("initial_layer_speed", process.initial_layer_speed),
      ("default_acceleration", process.default_acceleration),
    ] {
      effective.insert(key.to_string(), serde_json::json!(format!("{:.0}", raw_spd)));
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

  let mut out = serde_json::Map::new();
  out.insert("type".to_string(), serde_json::json!("process"));
  out.insert("from".to_string(), serde_json::json!("User"));
  out.insert("is_custom_defined".to_string(), serde_json::json!("0"));
  out.insert("inherits".to_string(), serde_json::json!(process.inherits));
  out.insert(
    "compatible_printers".to_string(),
    serde_json::json!([compat_printer]),
  );
  out.insert("name".to_string(), serde_json::json!(process.name));
  out.insert(
    "print_settings_id".to_string(),
    serde_json::json!(process.name),
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
    ) {
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
  for extra_key in ["seam_slope_inner_walls", "avoid_crossing_perimeters"] {
    if !out.contains_key(extra_key) {
      if let Some(v) = effective.get(extra_key).filter(|v| v.is_string()) {
        out.insert(extra_key.to_string(), v.clone());
      }
    }
  }
  let proc_notes = if process.notes.trim().is_empty() {
    process.generate_descriptive_notes()
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

/// Builds a user Machine `.json` preset from the imported OrcaSlicer machine base + rig settings
pub fn build_machine_preset_json(rig: &MachineRig) -> String {
  let cat = crate::catalog::load_merged_catalog();
  let (orca_noz, hrc) = rig.orca_machine_nozzle_type_and_hrc();
  let noz_diam_str = rig.formatted_nozzle_diameter();
  let notes_str = if rig.notes.trim().is_empty() {
    rig.generate_descriptive_notes()
  } else {
    rig.notes.clone()
  };
  let mut out = serde_json::Map::new();
  out.insert("type".to_string(), serde_json::json!("machine"));
  out.insert("from".to_string(), serde_json::json!("User"));
  out.insert("is_custom_defined".to_string(), serde_json::json!("0"));
  out.insert(
    "inherits".to_string(),
    serde_json::json!(format!("{} {} nozzle", rig.printer_model, noz_diam_str)),
  );
  out.insert("name".to_string(), serde_json::json!(rig.name));
  out.insert(
    "printer_settings_id".to_string(),
    serde_json::json!(rig.name),
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
  let ret_len_str = if (rig.base_retraction_length - rig.base_retraction_length.round()).abs() < 1e-6 {
    format!("{:.1}", rig.base_retraction_length)
  } else {
    format!("{:.2}", rig.base_retraction_length)
      .trim_end_matches('0')
      .trim_end_matches('.')
      .to_string()
  };
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
  let proc_notes = if process.notes.trim().is_empty() {
    process.generate_descriptive_notes()
  } else {
    process.notes.clone()
  };

  let slot_maps: Vec<serde_json::Map<String, serde_json::Value>> = active_slots
    .iter()
    .map(|sp| resolve_spool_orca_filament_map_with_rig(sp, Some(rig)))
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
    serde_json::json!(rig.format_concise_title()),
  );
  out.insert("printer_notes".to_string(), serde_json::json!(rig_notes));
  out.insert(
    "print_settings_id".to_string(),
    serde_json::json!(process.format_concise_title(&rig.short_model_tag())),
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
    serde_json::json!([format!("{:.2}", rig.base_retraction_length)]),
  );
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

  let fil_array_keys = [
    "filament_settings_id",
    "filament_type",
    "filament_diameter",
    "required_nozzle_HRC",
    "default_filament_colour",
    "nozzle_temperature",
    "nozzle_temperature_BRASS",
    "nozzle_temperature_HS",
    "nozzle_temperature_initial_layer",
    "nozzle_temperature_initial_layer_BRASS",
    "nozzle_temperature_initial_layer_HS",
    "nozzle_temperature_range_low",
    "nozzle_temperature_range_high",
    "hot_plate_temp",
    "hot_plate_temp_initial_layer",
    "textured_plate_temp",
    "textured_plate_temp_initial_layer",
    "filament_flow_ratio",
    "pressure_advance",
    "filament_max_volumetric_speed",
    "fan_min_speed",
    "fan_min_speed_BRASS",
    "fan_min_speed_HS",
    "fan_max_speed",
    "fan_max_speed_BRASS",
    "fan_max_speed_HS",
    "filament_retraction_length",
    "filament_retraction_speed",
    "filament_z_hop",
    "filament_z_hop_types",
    "filament_retract_before_wipe",
    "filament_deretraction_speed",
    "filament_wipe",
    "filament_wipe_distance",
    "filament_notes",
  ];
  for fk in fil_array_keys {
    let vals = collect_slot_vec(fk);
    if fk == "default_filament_colour" {
      out.insert(
        "filament_colour".to_string(),
        serde_json::json!(vals.clone()),
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
    ) {
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
  for extra_key in ["seam_slope_inner_walls", "avoid_crossing_perimeters"] {
    if !out.contains_key(extra_key) {
      if let Some(v) = effective.get(extra_key).filter(|v| v.is_string()) {
        out.insert(extra_key.to_string(), v.clone());
        proc_diff_keys.insert(extra_key.to_string());
      }
    }
  }

  let mut different_settings_to_system: Vec<String> = Vec::with_capacity(active_slots.len() + 2);
  different_settings_to_system.push(proc_diff_keys.into_iter().collect::<Vec<_>>().join(";"));
  for _ in active_slots {
    different_settings_to_system.push(FILAMENT_DIFF_KEYS.to_string());
  }
  different_settings_to_system.push(
    "nozzle_diameter;nozzle_hrc;nozzle_type;printer_notes;retraction_length;retraction_speed"
      .to_string(),
  );

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

  serde_json::to_string_pretty(&serde_json::Value::Object(out)).unwrap_or_else(|_| "{}".to_string())
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

    let brand = name
      .split(&['-', ' '][..])
      .find(|part| !part.is_empty() && *part != "Kobra")
      .unwrap_or("Anycubic")
      .to_string();

    let brass_base = (print_temp - 5).max(190);
    spools.push(FilamentSpool {
      id: format!("imported_slot_{}", idx + 1),
      name,
      brand,
      inherits: format!("Anycubic {} @Anycubic Kobra S1 0.4 nozzle", mat),
      material_type: mat.clone(),
      nozzle_type: "hardened_steel".to_string(),
      tuned_for_nozzle_type: "hardened_steel".to_string(),
      required_nozzle_hrc: 3,
      color_hex: color_hex.clone(),
      color_name: format!("Slot #{} ({})", idx + 1, color_hex),
      brass_base_temp: brass_base,
      hs_base_temp: print_temp,
      base_print_temp: brass_base,
      print_temp,
      base_bed_temp: (bed_temp - 5).max(45),
      bed_temp,
      flow_ratio,
      pressure_advance,
      max_volumetric_speed: max_vol,
      fan_speed_percent: if mat == "PETG" { 40 } else { 100 },
      remaining_grams: 850,
      is_loaded: true,
      finish_type,
      opacity,
      pigment_stickiness,
      drying_temp_c: if mat == "PETG" { 55 } else { 45 },
      drying_time_hr: if mat == "PETG" { 6 } else { 4 },
      is_rfid_synced: true,
      deltas_from_base: Vec::new(),
      raw_disk_json: None,
    });
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
    assert_eq!(FilamentSpool::canonical_polymer_key(&spool.material_type), "PETG");

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
}
