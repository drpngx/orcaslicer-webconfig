use crate::fs_access::{LocalFsSaveFileEntry, ScannedFileEntry};
use crate::models::{FilamentDeltaEntry, FilamentSpool, MachineRig, ProcessProfile};
use crate::orca_profile::OrcaProfile;
use crate::threemf::build_zip_archive;
use crate::transformations::{ExternalConflict, PendingDiskUpdate};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub struct DirectoryImportSummary {
  pub user_folder_id: String,
  pub machines: Vec<MachineRig>,
  pub active_machine_id: String,
  pub filaments: Vec<FilamentSpool>,
  pub processes: Vec<ProcessProfile>,
  pub active_process_id: String,
  pub base_profile: OrcaProfile,
  pub conflicts: Vec<ExternalConflict>,
  pub total_files_scanned: usize,
  pub disk_flush_matrix: Vec<String>,
  pub disk_flush_slot_count: usize,
  pub disk_flush_multiplier: f64,
  pub conf_relative_path: String,
}

/// Create a complete `.zip` backup of all scanned directory files
pub fn build_directory_backup_zip(entries: &[ScannedFileEntry]) -> Vec<u8> {
  let refs: Vec<(&str, &[u8])> = entries
    .iter()
    .map(|e| (e.relative_path.as_str(), e.bytes.as_slice()))
    .collect();
  build_zip_archive(&refs)
}

/// Extract all `ScannedFileEntry` items from a `.zip` backup archive
pub fn extract_directory_backup_zip(zip_bytes: &[u8]) -> Result<Vec<ScannedFileEntry>, String> {
  let mut entries = Vec::new();
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

    if method == 0 {
      if let Ok(rel_path) = std::str::from_utf8(&zip_bytes[name_start..name_start + name_len]) {
        let normalized = rel_path.replace('\\', "/");
        if !normalized.ends_with('/') {
          let filename = normalized
            .rsplit('/')
            .next()
            .unwrap_or(&normalized)
            .to_string();
          entries.push(ScannedFileEntry {
            relative_path: normalized,
            filename,
            bytes: zip_bytes[data_start..data_end].to_vec(),
          });
        }
      }
    }

    pos = data_end;
  }

  if entries.is_empty() {
    Err("No valid uncompressed configuration entries found in .zip archive.".to_string())
  } else {
    Ok(entries)
  }
}

/// Strip `# MD5 checksum ...` trailing comments that AnycubicSlicerNext appends to `.conf` files
pub fn clean_conf_json(raw: &str) -> String {
  let mut lines = Vec::new();
  for line in raw.lines() {
    if line.trim_start().starts_with('#') {
      continue;
    }
    lines.push(line);
  }
  lines.join("\n")
}

fn extract_str(val: &serde_json::Value, key: &str) -> Option<String> {
  val.get(key).and_then(|v| {
    if let Some(s) = v.as_str() {
      Some(s.to_string())
    } else if let Some(arr) = v.as_array() {
      arr.first().and_then(|x| x.as_str()).map(|s| s.to_string())
    } else {
      None
    }
  })
}

/// Parse a scanned `AnycubicSlicerNext` or `OrcaSlicer` directory into our internal data structures,
/// understanding user filament and machine configs as transformations from system bases,
/// and flagging any remaining discrepancies as `ExternalConflict` items!
pub fn import_scanned_directory(
  entries: &[ScannedFileEntry],
) -> Result<DirectoryImportSummary, String> {
  if entries.is_empty() {
    return Err(
      "Selected directory contained no readable .json or .conf configuration files.".to_string(),
    );
  }

  let mut system_filaments: HashMap<String, serde_json::Value> = HashMap::new();
  let mut system_machines: HashMap<String, serde_json::Value> = HashMap::new();
  let mut user_filament_files: Vec<(String, serde_json::Value, String)> = Vec::new();
  let mut user_machine_files: Vec<(String, serde_json::Value)> = Vec::new();
  let mut user_process_files: Vec<(String, serde_json::Value)> = Vec::new();
  let mut conf_json: Option<serde_json::Value> = None;
  let mut conf_relative_path = "AnycubicSlicerNext.conf".to_string();
  let mut user_folder_id = "default".to_string();

  for entry in entries {
    let text = match std::str::from_utf8(&entry.bytes) {
      Ok(t) => t,
      Err(_) => continue,
    };

    if entry.filename.ends_with(".conf")
      && (entry.filename.contains("AnycubicSlicer") || entry.filename.contains("OrcaSlicer"))
    {
      conf_relative_path = entry.relative_path.clone();
      let cleaned = clean_conf_json(text);
      if let Ok(v) = serde_json::from_str::<serde_json::Value>(&cleaned) {
        if let Some(pf) = v
          .get("app")
          .and_then(|a| a.get("preset_folder"))
          .and_then(|p| p.as_str())
        {
          if !pf.is_empty() {
            user_folder_id = pf.to_string();
          }
        }
        conf_json = Some(v);
      }
      continue;
    }

    if !entry.filename.ends_with(".json") {
      continue;
    }

    // Skip old version backup folders (`user_backup-*`) so we only import live configs
    if entry.relative_path.contains("user_backup") {
      continue;
    }

    let Ok(val) = serde_json::from_str::<serde_json::Value>(text) else {
      continue;
    };

    let name = extract_str(&val, "name").unwrap_or_else(|| entry.filename.clone());

    if entry.relative_path.starts_with("system/") {
      if entry.relative_path.contains("/filament/") {
        system_filaments.insert(name, val);
      } else if entry.relative_path.contains("/machine/") {
        system_machines.insert(name, val);
      }
    } else if entry.relative_path.starts_with("user/") {
      // Detect user folder ID from path `user/<id>/...`
      let parts: Vec<&str> = entry.relative_path.split('/').collect();
      if parts.len() >= 2 && parts[1] != "default" && !parts[1].ends_with(".cereal") {
        user_folder_id = parts[1].to_string();
      }

      if entry.relative_path.contains("/filament/") {
        if entry.relative_path.contains("/filament/base/") {
          system_filaments
            .entry(name.clone())
            .or_insert_with(|| val.clone());
        }
        user_filament_files.push((entry.relative_path.clone(), val, text.to_string()));
      } else if entry.relative_path.contains("/machine/") {
        user_machine_files.push((entry.relative_path.clone(), val));
      } else if entry.relative_path.contains("/process/") {
        user_process_files.push((entry.relative_path.clone(), val));
      }
    }
  }

  // 1. Reconstruct Machine Rigs (detecting Anycubic Kobra S1 with Hardened Steel Nozzle!)
  let mut machines = Vec::new();
  let mut conflicts = Vec::new();
  let mut active_machine_name_from_conf = String::new();
  let mut active_process_name_from_conf = String::new();
  let mut slot_filament_names_from_conf: Vec<String> = Vec::new();
  let mut disk_flush_matrix: Vec<String> = Vec::new();
  let mut disk_flush_slot_count: usize = 0;
  let mut disk_flush_multiplier: f64 = 1.0;
  let mut slot_colors_from_conf: Vec<String> = vec![
    "#658946".to_string(),
    "#E10600".to_string(),
    "#AFAFAF".to_string(),
    "#75787B".to_string(),
    "#CDA077".to_string(),
    "#7841CE".to_string(),
    "#06B6D4".to_string(),
    "#FACC15".to_string(),
  ];

  if let Some(ref c) = conf_json {
    if let Some(fm) = c
      .get("app")
      .and_then(|a| a.get("flush_multiplier"))
      .and_then(|x| x.as_str())
      .and_then(|s| s.parse::<f64>().ok())
    {
      disk_flush_multiplier = fm;
    }
    if let Some(m) = c
      .get("presets")
      .and_then(|p| p.get("machine"))
      .and_then(|x| x.as_str())
    {
      active_machine_name_from_conf = m.to_string();
    }
    // Extract active slot filaments, colors, process, and `flush_volumes_matrix` from `anycubic_presets`
    if let Some(arr) = c.get("anycubic_presets").and_then(|a| a.as_array()) {
      for preset in arr {
        let mach = preset.get("machine").and_then(|m| m.as_str()).unwrap_or("");
        let is_exact_active =
          !active_machine_name_from_conf.is_empty() && mach == active_machine_name_from_conf;
        if is_exact_active
          || (mach.contains("Kobra S1") && slot_filament_names_from_conf.is_empty())
        {
          if let Some(proc_str) = preset.get("process").and_then(|p| p.as_str()) {
            active_process_name_from_conf = proc_str.to_string();
          }
          let mut f_names = Vec::new();
          for key in [
            "filament",
            "filament_01",
            "filament_02",
            "filament_03",
            "filament_04",
            "filament_05",
            "filament_06",
            "filament_07",
          ] {
            if let Some(fnm) = preset.get(key).and_then(|v| v.as_str()) {
              if !fnm.trim().is_empty() {
                f_names.push(fnm.trim().to_string());
              }
            }
          }
          if !f_names.is_empty() {
            slot_filament_names_from_conf = f_names;
          }
          if let Some(cols) = preset.get("filament_colors").and_then(|fc| fc.as_str()) {
            let parsed_cols: Vec<String> = cols
              .split(',')
              .filter(|s| !s.trim().is_empty())
              .map(|s| s.trim().to_string())
              .collect();
            if !parsed_cols.is_empty() {
              slot_colors_from_conf = parsed_cols;
            }
          }
          if let Some(fmat) = preset
            .get("flush_volumes_matrix")
            .and_then(|fm| fm.as_str())
          {
            let parsed_cells: Vec<String> = fmat
              .split('|')
              .filter(|s| !s.trim().is_empty())
              .map(|s| {
                s.trim()
                  .parse::<f64>()
                  .map(|v| format!("{:.0}", v.round()))
                  .unwrap_or_else(|_| "0".to_string())
              })
              .collect();
            if parsed_cells.len() > 1 {
              let n = (parsed_cells.len() as f64).sqrt().round() as usize;
              if n * n == parsed_cells.len() {
                disk_flush_slot_count = n;
              }
              disk_flush_matrix = parsed_cells;
            }
          }
        }
      }
    }
  }

  for (rel_path, m_val) in &user_machine_files {
    let name = extract_str(m_val, "name").unwrap_or_else(|| "User Machine".to_string());
    let inherits =
      extract_str(m_val, "inherits").unwrap_or_else(|| "Anycubic Kobra S1 0.4 nozzle".to_string());
    let nozzle_type =
      extract_str(m_val, "nozzle_type").unwrap_or_else(|| "hardened_steel".to_string());
    let nozzle_material = match nozzle_type.as_str() {
      "hardened_steel" => "Hardened Steel".to_string(),
      "diamondback" => "Diamondback PCD".to_string(),
      "cht" => "Bi-Metal CHT".to_string(),
      _ => "Brass".to_string(),
    };

    let base_sys = system_machines.get(&inherits);
    // System base (`Anycubic Kobra S1 0.4 nozzle`) uses 0.8mm retract @ 40mm/s, 0mm cut retract.
    // Nozzle material (Brass vs. Hardened Steel) does NOT change retraction speed or length.
    let base_retract_speed = base_sys
      .and_then(|b| extract_str(b, "retraction_speed"))
      .unwrap_or_else(|| "40".to_string());
    let base_retract_len = base_sys
      .and_then(|b| extract_str(b, "retraction_length"))
      .unwrap_or_else(|| "0.8".to_string());
    let base_cut_retract = base_sys
      .and_then(|b| extract_str(b, "retraction_distances_when_cut"))
      .unwrap_or_else(|| "0".to_string());

    let expected_retract_speed = base_retract_speed.clone();
    let expected_retract_len = base_retract_len.clone();
    let expected_cut_retract = "18".to_string();

    let disk_retract_speed = extract_str(m_val, "retraction_speed");
    let disk_retract_len = extract_str(m_val, "retraction_length");
    let disk_cut_retract = extract_str(m_val, "retraction_distances_when_cut");

    let effective_disk_speed = disk_retract_speed
      .clone()
      .unwrap_or_else(|| base_retract_speed.clone());
    let effective_disk_len = disk_retract_len
      .clone()
      .unwrap_or_else(|| base_retract_len.clone());
    let effective_disk_cut = disk_cut_retract
      .clone()
      .unwrap_or_else(|| base_cut_retract.clone());

    if effective_disk_speed != expected_retract_speed {
      conflicts.push(ExternalConflict {
        param_key: format!("machine:retraction_speed ({})", name),
        param_label: format!("Machine Retraction Speed [{}]", name),
        base_value: format!("{} mm/s (System Machine Base)", base_retract_speed),
        expected_derived_value: format!(
          "{} mm/s (Stock Direct-Drive Standard)",
          expected_retract_speed
        ),
        actual_external_value: format!("{} mm/s", effective_disk_speed),
        modifying_rule_id: "Machine Retraction Override on Disk".to_string(),
        source_relative_path: rel_path.clone(),
        json_field_key: "retraction_speed".to_string(),
        clean_raw_value: expected_retract_speed.clone(),
        disk_raw_value: effective_disk_speed,
      });
    }

    if effective_disk_len != expected_retract_len {
      conflicts.push(ExternalConflict {
        param_key: format!("machine:retraction_length ({})", name),
        param_label: format!("Machine Retraction Length [{}]", name),
        base_value: format!("{} mm (System Machine Base)", base_retract_len),
        expected_derived_value: format!(
          "{} mm (Stock Direct-Drive Standard)",
          expected_retract_len
        ),
        actual_external_value: format!("{} mm", effective_disk_len),
        modifying_rule_id: "Machine Retraction Override on Disk".to_string(),
        source_relative_path: rel_path.clone(),
        json_field_key: "retraction_length".to_string(),
        clean_raw_value: expected_retract_len.clone(),
        disk_raw_value: effective_disk_len.clone(),
      });
    }

    if effective_disk_cut != expected_cut_retract {
      let actual_desc = if disk_cut_retract.is_some() {
        format!("{} mm", effective_disk_cut)
      } else {
        format!("{} mm (Inherited System Base)", effective_disk_cut)
      };
      conflicts.push(ExternalConflict {
        param_key: format!("machine:retraction_distances_when_cut ({})", name),
        param_label: format!("Cut Retraction Distance [{}]", name),
        base_value: format!("{} mm (System Machine Base)", base_cut_retract),
        expected_derived_value: format!("{} mm (ACE Pro Cutter Rule)", expected_cut_retract),
        actual_external_value: actual_desc,
        modifying_rule_id: "Multi-Material Tip Forming & Anti-Clog Rule (AMS / ACE Pro)"
          .to_string(),
        source_relative_path: rel_path.clone(),
        json_field_key: "retraction_distances_when_cut".to_string(),
        clean_raw_value: expected_cut_retract,
        disk_raw_value: effective_disk_cut,
      });
    }

    let is_active = if !active_machine_name_from_conf.is_empty() {
      name == active_machine_name_from_conf
    } else {
      machines.is_empty()
    };

    let mut addons = Vec::new();
    if name.contains("NewStartcode V4") {
      addons.push("hollow_front_back_covers".to_string());
    }

    let mut rig = MachineRig {
      id: name.to_lowercase().replace([' ', '-', '.'], "_"),
      name: name.clone(),
      printer_model: "Anycubic Kobra S1".to_string(),
      bed_size: "250 × 250 × 250 mm".to_string(),
      kinematics: "CoreXY Enclosed S-Series".to_string(),
      firmware: "Klipper (AnycubicSlicerNext 2.0.0.3)".to_string(),
      hotend: "Kobra S1 All-Metal Hotend".to_string(),
      nozzle_type,
      nozzle_material: nozzle_material.clone(),
      nozzle_name: format!("{} 0.40mm", nozzle_material),
      nozzle_diameter: "0.4".to_string(),
      nozzle_hrc: 55,
      max_volumetric_flow_limit: 16.0,
      max_volumetric_flow: "16.0 mm³/s".to_string(),
      base_retraction_length: disk_retract_len
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.80),
      melt_zone_multiplier: 1.07,
      ace_pro_units: 2,
      addons,
      max_speed: "600 mm/s".to_string(),
      max_accel: "20,000 mm/s²".to_string(),
      is_active,
      applied_transformations: vec![
        "Thermal Conductivity & Nozzle Material Rule".to_string(),
        "Volumetric Speed Ceiling Rule".to_string(),
        "Upgraded Flushing Volume Matrix (Lua)".to_string(),
      ],
      notes: String::new(),
      source_disk_profile: Some(rel_path.clone()),
    };
    let (_, hrc) = rig.orca_machine_nozzle_type_and_hrc();
    rig.nozzle_hrc = hrc;
    let disk_notes = extract_str(m_val, "printer_notes")
      .or_else(|| extract_str(m_val, "notes"))
      .unwrap_or_default();
    if disk_notes.trim().is_empty() {
      rig.notes = rig.generate_descriptive_notes();
    } else {
      rig.notes = format!(
        "{}\n{}",
        disk_notes.trim(),
        rig.generate_descriptive_notes()
      );
    }
    machines.push(rig);
  }

  if machines.is_empty() {
    machines.push(MachineRig::kobra_s1_hardened_steel_png());
  }

  let active_machine_id = machines
    .iter()
    .find(|m| m.is_active)
    .or_else(|| machines.first())
    .map(|m| m.id.clone())
    .unwrap_or_else(|| "kobra_s1_hs_png".to_string());

  // 2. Reconstruct Filaments as Transformations from System Base
  // Note:
  // - Profiles inside `filament/base/` (or with `"inherits": ""`) are standalone third-party base filaments
  //   (such as `CHCKX PETG PCTG @Anycubic Kobra S1 0.4 nozzle` where PCTG prints at 260°C nozzle / 85°C bed,
  //   or `FLASHFORGE PLA Carbon @Anycubic Kobra S1 0.4 nozzle` which is a 205°C/55°C base profile without a `_HS` field).
  //   Standalone base profiles are their own baseline and must NOT be diffed against Anycubic PETG (235°C) or Anycubic PLA-CF (220°C/65°C)!
  let mut filaments = Vec::new();
  for (idx, (rel_path, f_val, raw_text)) in user_filament_files.iter().enumerate() {
    let name = extract_str(f_val, "name").unwrap_or_else(|| format!("Filament #{}", idx + 1));
    let raw_inherits = extract_str(f_val, "inherits").unwrap_or_default();
    let is_standalone_base_file =
      rel_path.contains("/filament/base/") || raw_inherits.trim().is_empty();

    // Determine actual material type from profile name or `filament_type` field
    let upper_name = name.to_uppercase();
    let raw_mat_type = if upper_name.contains("PCTG") {
      "PCTG".to_string()
    } else if upper_name.contains("PLA CARBON") || upper_name.contains("PLA-CF") {
      "PLA-CF".to_string()
    } else if upper_name.contains("PETG-CF") {
      "PETG-CF".to_string()
    } else {
      extract_str(f_val, "filament_type")
        .or_else(|| {
          if upper_name.contains("PETG") {
            Some("PETG".to_string())
          } else if upper_name.contains("TPU") {
            Some("TPU".to_string())
          } else if upper_name.contains("ABS") {
            Some("ABS".to_string())
          } else if upper_name.contains("ASA") {
            Some("ASA".to_string())
          } else if upper_name.contains("PA12")
            || upper_name.contains("PA6")
            || upper_name.contains("NYLON")
            || upper_name.contains(" PA ")
          {
            Some("PA".to_string())
          } else if upper_name.contains("PLA") {
            Some("PLA".to_string())
          } else {
            None
          }
        })
        .or_else(|| {
          system_filaments
            .get(&raw_inherits)
            .and_then(|b| extract_str(b, "filament_type"))
        })
        .unwrap_or_else(|| "PLA".to_string())
    };
    let mat_type = match raw_mat_type.as_str() {
      "PETG Translucent" => "PETG".to_string(),
      "PLA Matte" | "PLA Silk" => "PLA".to_string(),
      other => other.to_string(),
    };

    // Resolve true system base matching the actual `mat_type`
    let canonical_base_name = match mat_type.as_str() {
      "PETG" | "PCTG" => "Anycubic PETG @Anycubic Kobra S1 0.4 nozzle",
      "PETG-CF" => "Anycubic PETG-CF @Anycubic Kobra S1 0.4 nozzle",
      "PLA-CF" => "Anycubic PLA-CF @Anycubic Kobra S1 0.4 nozzle",
      "TPU" => "Anycubic TPU @Anycubic Kobra S1 0.4 nozzle",
      "ABS" => "Anycubic ABS @Anycubic Kobra S1 0.4 nozzle",
      "ASA" => "Anycubic ASA @Anycubic Kobra S1 0.4 nozzle",
      "PA" | "PA12" | "PA6-CF" => {
        if system_filaments.contains_key("yxpoler PA @Anycubic Kobra S1 0.4 nozzle") {
          "yxpoler PA @Anycubic Kobra S1 0.4 nozzle"
        } else {
          "Anycubic PA @Anycubic Kobra S1 Max 0.4 nozzle"
        }
      }
      "PC" => "Anycubic PC @Anycubic Kobra S1 0.4 nozzle",
      _ => {
        if name.contains("PLA+") {
          "Anycubic PLA+ @Anycubic Kobra S1 0.4 nozzle"
        } else {
          "Anycubic PLA @Anycubic Kobra S1 0.4 nozzle"
        }
      }
    };

    let inherited_sys = if !raw_inherits.is_empty() {
      system_filaments.get(&raw_inherits)
    } else {
      None
    };
    let inherited_mat_matches = inherited_sys
      .and_then(|b| extract_str(b, "filament_type"))
      .map(|m| m.eq_ignore_ascii_case(&mat_type) || (mat_type == "PCTG" && m == "PETG"))
      .unwrap_or(false);

    let (resolved_inherits_name, base_sys) = if is_standalone_base_file {
      (format!("Standalone Base ({})", mat_type), None)
    } else if inherited_mat_matches {
      (raw_inherits.clone(), inherited_sys)
    } else {
      (
        canonical_base_name.to_string(),
        system_filaments.get(canonical_base_name),
      )
    };

    // Determine system Brass and system Hardened Steel (_HS) baselines separately
    // so we NEVER diff Brass values (`nozzle_temperature` / `nozzle_temperature_BRASS`)
    // against Hardened Steel values (`nozzle_temperature_HS`).
    let default_mat_brass_temp = match mat_type.as_str() {
      "PCTG" => 255,
      "PETG" | "PETG-CF" => 230,
      "TPU" => 230,
      "ABS" => 255,
      "ASA" => 260,
      "PA" | "PA12" | "PA6-CF" => 275,
      "PC" => 260,
      "PLA-CF" => 205,
      _ => 210,
    };

    let explicit_hs_opt: Option<i32> =
      extract_str(f_val, "nozzle_temperature_HS").and_then(|s| s.parse().ok());
    let explicit_brass_opt: Option<i32> =
      extract_str(f_val, "nozzle_temperature_BRASS").and_then(|s| s.parse().ok());
    let explicit_generic_opt: Option<i32> =
      extract_str(f_val, "nozzle_temperature").and_then(|s| s.parse().ok());

    let sys_brass_temp: i32 = if is_standalone_base_file {
      explicit_brass_opt
        .or(explicit_generic_opt)
        .unwrap_or(default_mat_brass_temp)
    } else {
      base_sys
        .and_then(|b| {
          extract_str(b, "nozzle_temperature_BRASS")
            .or_else(|| extract_str(b, "nozzle_temperature"))
        })
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(default_mat_brass_temp)
    };

    let sys_hs_temp_opt: Option<i32> = if is_standalone_base_file {
      explicit_hs_opt.or_else(|| explicit_generic_opt.map(|t| t + 5))
    } else {
      base_sys
        .and_then(|b| extract_str(b, "nozzle_temperature_HS"))
        .and_then(|s| s.parse::<i32>().ok())
    };
    let expected_hs_temp: i32 = sys_hs_temp_opt.unwrap_or(sys_brass_temp + 5);

    // Like-for-like baseline and disk temperature for the active Hardened Steel rig:
    // If a standalone base file (like FLASHFORGE PLA Carbon) only defines `nozzle_temperature: 205` (no `_HS`),
    // its Brass base is 205°C and its Hardened Steel (+5°C thermal rule) effective temp is 210°C!
    let base_nozzle_temp: i32 = if is_standalone_base_file {
      explicit_hs_opt.unwrap_or(sys_brass_temp)
    } else {
      sys_hs_temp_opt.unwrap_or(sys_brass_temp)
    };
    let disk_nozzle_temp: i32 = if let Some(hs_val) = explicit_hs_opt {
      hs_val
    } else if is_standalone_base_file {
      // Standalone base file without `_HS`: apply +5°C Hardened Steel thermal rule cleanly without a false conflict
      sys_brass_temp + 5
    } else {
      explicit_generic_opt.unwrap_or(expected_hs_temp)
    };

    let default_mat_bed_temp = match mat_type.as_str() {
      "PCTG" => 85,
      "PETG" | "PETG-CF" => 75,
      "PLA-CF" => 55,
      "TPU" => 45,
      "ABS" | "ASA" => 90,
      "PA" | "PA12" | "PA6-CF" | "PC" => 100,
      _ => 55,
    };

    let disk_bed_opt: Option<i32> = extract_str(f_val, "textured_plate_temp")
      .or_else(|| extract_str(f_val, "hot_plate_temp"))
      .and_then(|s| s.parse().ok());

    let base_bed_temp: i32 = if is_standalone_base_file {
      disk_bed_opt.unwrap_or(default_mat_bed_temp)
    } else {
      base_sys
        .and_then(|b| {
          extract_str(b, "textured_plate_temp").or_else(|| extract_str(b, "hot_plate_temp"))
        })
        .and_then(|s| s.parse().ok())
        .unwrap_or(default_mat_bed_temp)
    };

    let disk_bed_temp: i32 = disk_bed_opt.unwrap_or(base_bed_temp);

    let flow_ratio: f32 = extract_str(f_val, "filament_flow_ratio")
      .or_else(|| base_sys.and_then(|b| extract_str(b, "filament_flow_ratio")))
      .and_then(|s| s.parse().ok())
      .unwrap_or(0.96);

    let pressure_advance: f32 = extract_str(f_val, "pressure_advance")
      .or_else(|| base_sys.and_then(|b| extract_str(b, "pressure_advance")))
      .and_then(|s| s.parse().ok())
      .unwrap_or(0.040);

    let max_vol: f32 = extract_str(f_val, "filament_max_volumetric_speed")
      .or_else(|| base_sys.and_then(|b| extract_str(b, "filament_max_volumetric_speed")))
      .and_then(|s| s.parse().ok())
      .unwrap_or(if mat_type == "PETG" || mat_type == "PCTG" {
        12.0
      } else {
        16.0
      });

    // Analyze `nozzle_temperature_HS` like-for-like (System HS vs. User HS, or System single-temp +5°C HS rule)
    let mut deltas = Vec::new();
    let matches_hs_rule =
      is_standalone_base_file || (disk_nozzle_temp - expected_hs_temp).abs() <= 2;
    let base_hs_label = if is_standalone_base_file {
      format!("{}°C (Standalone Base)", base_nozzle_temp)
    } else if let Some(sys_hs) = sys_hs_temp_opt {
      format!("{}°C (System HS)", sys_hs)
    } else {
      format!("{}°C (System Base)", sys_brass_temp)
    };
    let expected_hs_label = if is_standalone_base_file {
      format!("{}°C ({} HS Baseline)", disk_nozzle_temp, mat_type)
    } else if let Some(sys_hs) = sys_hs_temp_opt {
      format!("{}°C (System HS Baseline)", sys_hs)
    } else {
      format!("{}°C (Hardened Steel +5°C Rule)", expected_hs_temp)
    };

    if disk_nozzle_temp != base_nozzle_temp {
      deltas.push(FilamentDeltaEntry {
        param_key: "nozzle_temperature_HS".to_string(),
        base_value: base_hs_label.clone(),
        disk_value: format!("{}°C", disk_nozzle_temp),
        reconstructed_by_lua: matches_hs_rule,
        rule_explanation: if is_standalone_base_file {
          if explicit_hs_opt.is_some() {
            format!(
              "Standalone {} base profile ({}°C HS / {}°C Brass)",
              mat_type, disk_nozzle_temp, sys_brass_temp
            )
          } else {
            format!(
              "Standalone {} base ({}°C Brass + 5°C Hardened Steel Rule → {}°C)",
              mat_type, sys_brass_temp, disk_nozzle_temp
            )
          }
        } else if matches_hs_rule {
          if sys_hs_temp_opt.is_some() {
            format!(
              "Matches system Hardened Steel baseline ({}°C)",
              disk_nozzle_temp
            )
          } else {
            format!(
              "Reconstructed by Hardened Steel Thermal Rule (+{}°C over {}°C System Base)",
              disk_nozzle_temp - sys_brass_temp,
              sys_brass_temp
            )
          }
        } else {
          format!(
            "User HS override ({}°C on disk vs {})",
            disk_nozzle_temp, expected_hs_label
          )
        },
      });
    }

    if !matches_hs_rule && !is_standalone_base_file {
      let actual_label = if explicit_hs_opt.is_none() {
        format!("{}°C (Missing _HS field on disk)", disk_nozzle_temp)
      } else {
        format!("{}°C (User HS Override)", disk_nozzle_temp)
      };

      conflicts.push(ExternalConflict {
        param_key: format!("filament:{}:nozzle_temperature_HS", name),
        param_label: format!("{} — Nozzle Temp (HS)", name),
        base_value: format!("{} • {}", base_hs_label, resolved_inherits_name),
        expected_derived_value: expected_hs_label,
        actual_external_value: actual_label,
        modifying_rule_id: "Thermal Conductivity & Nozzle Material Rule".to_string(),
        source_relative_path: rel_path.clone(),
        json_field_key: "nozzle_temperature_HS".to_string(),
        clean_raw_value: expected_hs_temp.to_string(),
        disk_raw_value: disk_nozzle_temp.to_string(),
      });
    }

    // Bed adhesion rule (+5°C over stock Textured PEI bed temp for PLA/PETG user overrides)
    let expected_bed_adhesion =
      if !is_standalone_base_file && (mat_type == "PLA" || mat_type == "PETG") {
        base_bed_temp + 5
      } else {
        base_bed_temp
      };
    if disk_bed_temp != base_bed_temp {
      let matches_bed_rule = (disk_bed_temp - expected_bed_adhesion).abs() <= 2;
      deltas.push(FilamentDeltaEntry {
        param_key: "textured_plate_temp".to_string(),
        base_value: format!("{}°C", base_bed_temp),
        disk_value: format!("{}°C", disk_bed_temp),
        reconstructed_by_lua: matches_bed_rule,
        rule_explanation: if matches_bed_rule {
          format!(
            "Reconstructed by Bed Adhesion / Hotbed Rule (+{}°C over base {}°C)",
            disk_bed_temp - base_bed_temp,
            base_bed_temp
          )
        } else {
          format!(
            "Custom bed temp delta ({}°C vs base {}°C; Rule expects {}°C)",
            disk_bed_temp, base_bed_temp, expected_bed_adhesion
          )
        },
      });

      if !is_standalone_base_file
        && (disk_bed_temp - expected_bed_adhesion).abs() > 0
        && (mat_type == "PLA" || mat_type == "PETG")
      {
        conflicts.push(ExternalConflict {
          param_key: format!("filament:{}:textured_plate_temp", name),
          param_label: format!("{} — Textured Plate Temp", name),
          base_value: format!("{}°C ({} Base)", base_bed_temp, mat_type),
          expected_derived_value: format!("{}°C (Bed Adhesion +5°C Rule)", expected_bed_adhesion),
          actual_external_value: format!("{}°C", disk_bed_temp),
          modifying_rule_id: "First-Layer Squish & Bed Adhesion Rule".to_string(),
          source_relative_path: rel_path.clone(),
          json_field_key: "textured_plate_temp".to_string(),
          clean_raw_value: expected_bed_adhesion.to_string(),
          disk_raw_value: disk_bed_temp.to_string(),
        });
      }
    }

    // Flag cross-polymer inheritance mismatch (e.g. `yxpoler Nylon blue PA6` (`PA`) inheriting from `yxpoler ABS blue` (`ABS`))
    if !is_standalone_base_file && !raw_inherits.is_empty() && !inherited_mat_matches {
      let wrong_parent_mat = inherited_sys
        .and_then(|b| extract_str(b, "filament_type"))
        .unwrap_or_else(|| "Mismatched Base".to_string());
      deltas.push(FilamentDeltaEntry {
        param_key: "inherits".to_string(),
        base_value: format!("{} ({})", canonical_base_name, mat_type),
        disk_value: format!("{} ({})", raw_inherits, wrong_parent_mat),
        reconstructed_by_lua: false,
        rule_explanation: format!(
          "Unsafe cross-polymer inheritance on disk: {} filament inherits from {} ({}) profile!",
          mat_type, raw_inherits, wrong_parent_mat
        ),
      });
      conflicts.push(ExternalConflict {
        param_key: format!("filament:{}:inherits", name),
        param_label: format!("{} — Inherited Base Profile (Polymer Mismatch)", name),
        base_value: format!("{} ({} Base)", canonical_base_name, mat_type),
        expected_derived_value: canonical_base_name.to_string(),
        actual_external_value: format!("{} ({})", raw_inherits, wrong_parent_mat),
        modifying_rule_id: "Polymer Compatibility & Inheritance Guard".to_string(),
        source_relative_path: rel_path.clone(),
        json_field_key: "inherits".to_string(),
        clean_raw_value: canonical_base_name.to_string(),
        disk_raw_value: raw_inherits.clone(),
      });
    }

    // Flag excessive part cooling fan on warp-prone engineering polymers (ABS / ASA / PA / PC)
    if matches!(
      mat_type.as_str(),
      "ABS" | "ASA" | "PA" | "PA12" | "PA6-CF" | "PC"
    ) {
      if let Some(fan_hs) =
        extract_str(f_val, "fan_max_speed_HS").and_then(|s| s.parse::<i32>().ok())
      {
        let fan_brass = extract_str(f_val, "fan_max_speed_BRASS")
          .and_then(|s| s.parse::<i32>().ok())
          .unwrap_or(20);
        if fan_hs > 35 && fan_brass <= 35 {
          deltas.push(FilamentDeltaEntry {
            param_key: "fan_max_speed_HS".to_string(),
            base_value: format!("{}% (Brass Base)", fan_brass),
            disk_value: format!("{}% (Unsafe HS Fan)", fan_hs),
            reconstructed_by_lua: false,
            rule_explanation: format!(
              "Unsafe {}% max cooling fan on Hardened Steel for warp-prone {} (causes severe warping & layer delamination; Brass is {}%)",
              fan_hs, mat_type, fan_brass
            ),
          });
          conflicts.push(ExternalConflict {
            param_key: format!("filament:{}:fan_max_speed_HS", name),
            param_label: format!("{} — Max Cooling Fan (HS)", name),
            base_value: format!("{}% ({} Safe Max)", fan_brass, mat_type),
            expected_derived_value: format!("{}% (Matches Brass Cooling Schedule)", fan_brass),
            actual_external_value: format!("{}% (Warping Hazard)", fan_hs),
            modifying_rule_id: "Rig Hardware Add-Ons & Cooling Schedule Rule".to_string(),
            source_relative_path: rel_path.clone(),
            json_field_key: "fan_max_speed_HS".to_string(),
            clean_raw_value: fan_brass.to_string(),
            disk_raw_value: fan_hs.to_string(),
          });
        }
      }
    }

    let color_hex = slot_colors_from_conf
      .get(idx % slot_colors_from_conf.len())
      .cloned()
      .unwrap_or_else(|| "#06B6D4".to_string());

    let lower_name = name.to_lowercase();
    let lower_inherits = raw_inherits.to_lowercase();
    let lower_raw_mat = raw_mat_type.to_lowercase();
    let finish_type = if lower_name.contains("translucent")
      || lower_name.contains("clear")
      || lower_name.contains("transparent")
      || lower_inherits.contains("translucent")
      || lower_raw_mat.contains("translucent")
    {
      "Translucent".to_string()
    } else if lower_name.contains("matt")
      || lower_inherits.contains("matt")
      || lower_raw_mat.contains("matt")
    {
      "Matte".to_string()
    } else if lower_name.contains("silk")
      || lower_inherits.contains("silk")
      || lower_raw_mat.contains("silk")
    {
      "Silk".to_string()
    } else if lower_name.contains("carbon") || mat_type.contains("-CF") {
      "Carbon Fiber".to_string()
    } else if lower_name.contains("glass") || mat_type.contains("-GF") {
      "Glass Fiber".to_string()
    } else {
      "Basic".to_string()
    };

    let is_abrasive = finish_type.contains("Carbon")
      || finish_type.contains("Glass")
      || mat_type.contains("-CF")
      || mat_type.contains("-GF");
    let required_nozzle_hrc: u32 = if is_abrasive {
      55
    } else {
      extract_str(f_val, "required_nozzle_HRC")
        .and_then(|s| s.parse().ok())
        .unwrap_or(3)
    };

    let opacity = if finish_type == "Translucent" {
      0.30
    } else {
      0.95
    };

    let pigment_stickiness = if finish_type.contains("Carbon") || mat_type.contains("-CF") {
      1.15
    } else if color_hex == "#E10600" {
      1.25
    } else {
      1.00
    };

    let vendor = extract_str(f_val, "filament_vendor")
      .or_else(|| base_sys.and_then(|b| extract_str(b, "filament_vendor")))
      .unwrap_or_else(|| "Anycubic".to_string());

    let tuned_for_nozzle_type = if explicit_hs_opt.is_some() {
      "hardened_steel".to_string()
    } else if is_standalone_base_file {
      "brass".to_string()
    } else {
      "hardened_steel".to_string()
    };

    let (dry_c, dry_hr) = match mat_type.as_str() {
      "PETG" | "PETG-CF" | "PETG-GF" | "PCTG" => (55, 6),
      "TPU" | "TPU 95A" | "PEBA" => (50, 4),
      "ABS" | "ASA" | "PA" | "PA12" | "PA6-CF" | "PAHT-CF" | "PC" => (55, 8),
      _ => (45, 4), // PLA, PLA+, PLA-CF, PLA Matte, Silk -> 45°C for 4h (below 45°C Tg vitrification!)
    };

    filaments.push(FilamentSpool {
      id: format!("disk_spool_{}", idx + 1),
      name,
      brand: vendor,
      inherits: resolved_inherits_name,
      material_type: mat_type.clone(),
      nozzle_type: "hardened_steel".to_string(),
      tuned_for_nozzle_type,
      required_nozzle_hrc,
      color_hex,
      color_name: format!("Slot #{}", idx + 1),
      brass_base_temp: sys_brass_temp,
      hs_base_temp: expected_hs_temp,
      base_print_temp: base_nozzle_temp,
      print_temp: disk_nozzle_temp,
      base_bed_temp,
      bed_temp: disk_bed_temp,
      flow_ratio,
      pressure_advance,
      max_volumetric_speed: max_vol,
      fan_speed_percent: if mat_type == "PETG" || mat_type == "PCTG" {
        40
      } else {
        100
      },
      remaining_grams: 850,
      is_loaded: idx < 8,
      finish_type,
      opacity,
      pigment_stickiness,
      drying_temp_c: dry_c,
      drying_time_hr: dry_hr,
      is_rfid_synced: true,
      deltas_from_base: deltas,
      raw_disk_json: Some(raw_text.clone()),
    });
  }

  // Include any system filaments referenced in `slot_filament_names_from_conf` (e.g. `Anycubic TPU @Anycubic Kobra S1 0.4 nozzle`
  // or `Anycubic PETG-CF @Anycubic Kobra S1 0.4 nozzle`) that don't have a separate user override file,
  // and order the leading slots to match `AnycubicSlicerNext.conf` 1-to-1 so `flush_volumes_matrix` aligns with Slots #1..#N!
  for (slot_idx, slot_fname) in slot_filament_names_from_conf.iter().enumerate() {
    if !filaments.iter().any(|f| &f.name == slot_fname) {
      let sys_val = system_filaments.get(slot_fname);
      let mat_type = sys_val
        .and_then(|b| extract_str(b, "filament_type"))
        .unwrap_or_else(|| {
          let upper = slot_fname.to_uppercase();
          if upper.contains("TPU") {
            "TPU".to_string()
          } else if upper.contains("PETG") {
            "PETG".to_string()
          } else if upper.contains("ABS") {
            "ABS".to_string()
          } else if upper.contains("ASA") {
            "ASA".to_string()
          } else {
            "PLA".to_string()
          }
        });
      let sys_brass = sys_val
        .and_then(|b| {
          extract_str(b, "nozzle_temperature_BRASS")
            .or_else(|| extract_str(b, "nozzle_temperature"))
        })
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(205);
      let base_temp = sys_val
        .and_then(|b| {
          extract_str(b, "nozzle_temperature_HS").or_else(|| extract_str(b, "nozzle_temperature"))
        })
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(220);
      let base_bed = sys_val
        .and_then(|b| {
          extract_str(b, "textured_plate_temp").or_else(|| extract_str(b, "hot_plate_temp"))
        })
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(if mat_type == "TPU" { 45 } else { 75 });
      let color_hex = slot_colors_from_conf
        .get(slot_idx % slot_colors_from_conf.len())
        .cloned()
        .unwrap_or_else(|| "#75787B".to_string());
      let (dry_c, dry_hr) = match mat_type.as_str() {
        "PETG" | "PETG-CF" | "PETG-GF" | "PCTG" => (55, 6),
        "TPU" | "TPU 95A" | "PEBA" => (50, 4),
        "ABS" | "ASA" | "PA" | "PA12" | "PA6-CF" | "PAHT-CF" | "PC" => (55, 8),
        _ => (45, 4), // PLA, PLA+, PLA-CF -> 45°C for 4h
      };
      filaments.push(FilamentSpool {
        id: format!("disk_sys_spool_{}", slot_idx + 1),
        name: slot_fname.clone(),
        brand: "Anycubic".to_string(),
        inherits: slot_fname.clone(),
        material_type: mat_type.clone(),
        nozzle_type: "hardened_steel".to_string(),
        tuned_for_nozzle_type: "any".to_string(),
        required_nozzle_hrc: if slot_fname.contains("-CF") || slot_fname.contains("-GF") {
          55
        } else {
          3
        },
        color_hex: color_hex.clone(),
        color_name: format!("Slot #{}", slot_idx + 1),
        brass_base_temp: sys_brass,
        hs_base_temp: base_temp,
        base_print_temp: base_temp,
        print_temp: base_temp,
        base_bed_temp: base_bed,
        bed_temp: base_bed,
        flow_ratio: 0.96,
        pressure_advance: 0.040,
        max_volumetric_speed: if mat_type == "TPU" { 6.0 } else { 12.0 },
        fan_speed_percent: if mat_type == "PETG" { 40 } else { 100 },
        remaining_grams: 850,
        is_loaded: true,
        finish_type: if slot_fname.contains("-CF") {
          "Carbon Fiber".to_string()
        } else {
          "Basic".to_string()
        },
        opacity: 0.95,
        pigment_stickiness: if slot_fname.contains("-CF") {
          1.15
        } else if color_hex == "#E10600" {
          1.25
        } else {
          1.00
        },
        drying_temp_c: dry_c,
        drying_time_hr: dry_hr,
        is_rfid_synced: true,
        deltas_from_base: Vec::new(),
        raw_disk_json: sys_val.and_then(|v| serde_json::to_string_pretty(v).ok()),
      });
    }
  }

  // Sort filaments so that slots listed in `AnycubicSlicerNext.conf` appear in their exact slot order first,
  // followed by custom `- png` / `hotbed` profiles
  filaments.sort_by_key(|s| {
    if let Some(pos) = slot_filament_names_from_conf
      .iter()
      .position(|n| n == &s.name)
    {
      pos
    } else if s.name.contains("png") || s.name.contains("hotbed") {
      100
    } else if s.name.contains("CHCKX") || s.name.contains("FLASHFORGE") {
      101
    } else {
      102
    }
  });

  // Assign exact slot colors and slot labels for the leading slots from `AnycubicSlicerNext.conf`
  for (i, sp) in filaments.iter_mut().enumerate() {
    if let Some(col) = slot_colors_from_conf.get(i) {
      sp.color_hex = col.clone();
      sp.pigment_stickiness =
        if sp.finish_type.contains("Carbon") || sp.material_type.contains("-CF") {
          1.15
        } else if col == "#E10600" {
          1.25
        } else {
          1.00
        };
    }
    sp.color_name = format!("Slot #{}", i + 1);
    sp.is_loaded = i < 8;
  }

  // 3. Reconstruct Process Profiles & Ensure High Quality, Standard, and High Speed Tiers Exist
  let mut processes = Vec::new();
  for (idx, (rel_path, p_val)) in user_process_files.iter().enumerate() {
    let name = extract_str(p_val, "name").unwrap_or_else(|| format!("Process #{}", idx + 1));
    let inherits = extract_str(p_val, "inherits").unwrap_or_default();
    let print_seq = extract_str(p_val, "print_sequence").unwrap_or_else(|| "by layer".to_string());
    let wall_gen = extract_str(p_val, "wall_generator").unwrap_or_else(|| "classic".to_string());

    if name.contains("png") && (print_seq != "by layer" || wall_gen != "classic") {
      conflicts.push(ExternalConflict {
        param_key: format!("process:{}:wall_and_sequence", name),
        param_label: format!("Process '{}' (wall_generator & print_sequence)", name),
        base_value: "classic / by layer (System Base)".to_string(),
        expected_derived_value: "classic / by layer".to_string(),
        actual_external_value: format!("{} / {}", wall_gen, print_seq),
        modifying_rule_id: "Process Profile Override on Disk".to_string(),
        source_relative_path: rel_path.clone(),
        json_field_key: "wall_generator,print_sequence".to_string(),
        clean_raw_value: "classic,by layer".to_string(),
        disk_raw_value: format!("{},{}", wall_gen, print_seq),
      });
    }

    let is_hq = name.to_lowercase().contains("high quality")
      || inherits.to_lowercase().contains("high quality");
    let is_hs = name.to_lowercase().contains("high speed")
      || name.to_lowercase().contains("draft")
      || name.to_lowercase().contains("fast");

    let mut proc_prof = ProcessProfile {
      id: format!("disk_proc_{}", idx + 1),
      name,
      inherits,
      layer_height: extract_str(p_val, "layer_height")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.20),
      line_width: extract_str(p_val, "line_width")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.42),
      outer_wall_speed: extract_str(p_val, "outer_wall_speed")
        .and_then(|s| s.parse().ok())
        .unwrap_or(if is_hq {
          60.0
        } else if is_hs {
          300.0
        } else {
          200.0
        }),
      inner_wall_speed: extract_str(p_val, "inner_wall_speed")
        .and_then(|s| s.parse().ok())
        .unwrap_or(if is_hq {
          150.0
        } else if is_hs {
          350.0
        } else {
          300.0
        }),
      sparse_infill_speed: extract_str(p_val, "sparse_infill_speed")
        .and_then(|s| s.parse().ok())
        .unwrap_or(if is_hq {
          250.0
        } else if is_hs {
          350.0
        } else {
          270.0
        }),
      initial_layer_speed: extract_str(p_val, "initial_layer_speed")
        .and_then(|s| s.parse().ok())
        .unwrap_or(if is_hq {
          50.0
        } else if is_hs {
          60.0
        } else {
          50.0
        }),
      default_acceleration: extract_str(p_val, "default_acceleration")
        .and_then(|s| s.parse().ok())
        .unwrap_or(if is_hq {
          5000.0
        } else if is_hs {
          15000.0
        } else {
          10000.0
        }),
      print_sequence: print_seq,
      wall_generator: wall_gen,
      notes: String::new(),
    };
    let disk_proc_notes = extract_str(p_val, "notes").unwrap_or_default();
    if disk_proc_notes.trim().is_empty()
      || disk_proc_notes.contains("[OrcaSlicer WebConfig Process")
    {
      proc_prof.notes = proc_prof.generate_descriptive_notes();
    } else {
      proc_prof.notes = format!(
        "{}\n{}",
        disk_proc_notes.trim(),
        proc_prof.generate_descriptive_notes()
      );
    }
    processes.push(proc_prof);
  }

  // Ensure all 5 default OC process presets (0.20mm Standard, 0.20mm Quality, 0.20mm Speed, 0.12mm Quality, 0.28mm Speed) exist
  for def_proc in ProcessProfile::get_default_processes() {
    if !processes.iter().any(|p| p.name == def_proc.name) {
      processes.push(def_proc);
    }
  }

  // Order processes cleanly: High Quality -> Standard -> High Speed
  processes.sort_by_key(|p| match p.quality_tier() {
    "high_quality" => 0,
    "standard" => 1,
    "high_speed" => 2,
    _ => 3,
  });

  let active_process_id = processes
    .iter()
    .find(|p| !active_process_name_from_conf.is_empty() && p.name == active_process_name_from_conf)
    .or_else(|| processes.iter().find(|p| p.quality_tier() == "standard"))
    .or_else(|| processes.first())
    .map(|p| p.id.clone())
    .unwrap_or_else(|| "proc_020_std_png".to_string());

  // System Base Profile represents the shipped Anycubic Kobra S1 0.4mm Brass Nozzle baseline
  // BEFORE our Hardened Steel + ACE Pro transformation rules are applied.
  let mut base_profile = OrcaProfile::get_stock_kobra_s1();
  base_profile.name = "Anycubic Kobra S1 0.4 nozzle (Shipped Brass System Base)".to_string();
  base_profile.set_str("nozzle_type", "brass");
  base_profile.set_array_str("nozzle_temperature", "230");
  base_profile.set_array_str("nozzle_temperature_initial_layer", "230");
  base_profile.set_array_str("nozzle_temperature_HS", "230");
  base_profile.set_array_str("nozzle_temperature_initial_layer_HS", "230");
  base_profile.set_array_str("hot_plate_temp", "75");
  base_profile.set_array_str("hot_plate_temp_initial_layer", "75");
  base_profile.set_array_str("textured_plate_temp", "75");
  base_profile.set_array_str("textured_plate_temp_initial_layer", "75");
  base_profile.set_array_str("filament_retraction_length", "0.8");
  base_profile.set_array_str("filament_retraction_speed", "40");
  base_profile.set_str("cut_retraction_length", "0");
  base_profile.set_str("flush_multiplier", &format!("{:.2}", disk_flush_multiplier));

  Ok(DirectoryImportSummary {
    user_folder_id,
    machines,
    active_machine_id,
    filaments,
    processes,
    active_process_id,
    base_profile,
    conflicts,
    total_files_scanned: entries.len(),
    disk_flush_matrix,
    disk_flush_slot_count,
    disk_flush_multiplier,
    conf_relative_path,
  })
}

/// Build a JSON summary of the actual raw `user/<id>/{machine,filament,process}/*.json` files on disk.
/// This reflects what is currently saved on the user's filesystem (and does NOT mirror our derived config until saved!).
pub fn build_user_disk_config_summary_json(entries: &[ScannedFileEntry]) -> String {
  let mut machine_files = Vec::new();
  let mut filament_files = Vec::new();
  let mut process_files = Vec::new();

  for entry in entries {
    if !entry.filename.ends_with(".json")
      || !entry.relative_path.starts_with("user/")
      || entry.relative_path.contains("user_backup")
    {
      continue;
    }
    let Ok(text) = std::str::from_utf8(&entry.bytes) else {
      continue;
    };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(text) else {
      continue;
    };

    let item = serde_json::json!({
      "disk_path": entry.relative_path,
      "contents": val,
    });

    if entry.relative_path.contains("/machine/") {
      machine_files.push(item);
    } else if entry.relative_path.contains("/filament/") {
      filament_files.push(item);
    } else if entry.relative_path.contains("/process/") {
      process_files.push(item);
    }
  }

  serde_json::to_string_pretty(&serde_json::json!({
    "source": "Actual Files on Disk (Does NOT auto-mirror until you click Save)",
    "user_machine_profiles_on_disk": machine_files,
    "user_filament_profiles_on_disk": filament_files,
    "user_process_profiles_on_disk": process_files,
  }))
  .unwrap_or_else(|_| "{}".to_string())
}

fn set_json_field_preserving_shape(
  map: &mut serde_json::Map<String, serde_json::Value>,
  key: &str,
  new_val: &str,
) {
  let is_arr = match map.get(key) {
    Some(serde_json::Value::Array(_)) => true,
    Some(_) => false,
    None => {
      key.starts_with("nozzle_temperature")
        || key.contains("_plate_temp")
        || key.starts_with("retraction_")
        || key.starts_with("filament_")
    }
  };
  if is_arr {
    map.insert(key.to_string(), serde_json::json!([new_val]));
  } else {
    map.insert(
      key.to_string(),
      serde_json::Value::String(new_val.to_string()),
    );
  }
}

/// Apply staged `PendingDiskUpdate` items (from "Keep Clean Rule Value" or Start-From-Scratch Wizard) to `entries`
/// and return the modified/created files to write back to disk.
pub fn apply_pending_updates_to_entries(
  entries: &mut Vec<ScannedFileEntry>,
  updates: &[PendingDiskUpdate],
) -> Vec<LocalFsSaveFileEntry> {
  let mut modified_paths: HashMap<String, String> = HashMap::new();

  for upd in updates {
    if upd.source_relative_path.is_empty() {
      continue;
    }
    let target_rel_path = crate::models::sanitize_user_relative_path(&upd.source_relative_path);

    if !entries.iter().any(|e| e.relative_path == target_rel_path) {
      let fname = target_rel_path
        .rsplit('/')
        .next()
        .unwrap_or("profile.json")
        .to_string();
      let initial_json = serde_json::to_string_pretty(&serde_json::json!({
        "from": "User",
        "name": fname.trim_end_matches(".json"),
        upd.json_field_key.clone(): [upd.new_clean_value.clone()],
      }))
      .unwrap_or_else(|_| "{}".to_string());
      entries.push(ScannedFileEntry {
        relative_path: target_rel_path.clone(),
        filename: fname,
        bytes: initial_json.as_bytes().to_vec(),
      });
    }

    if let Some(entry) = entries
      .iter_mut()
      .find(|e| e.relative_path == target_rel_path)
    {
      let Ok(raw_text) = std::str::from_utf8(&entry.bytes) else {
        continue;
      };
      let is_conf = entry.relative_path.ends_with(".conf");
      let cleaned_text = if is_conf {
        clean_conf_json(raw_text)
      } else {
        raw_text.to_string()
      };
      let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&cleaned_text) else {
        continue;
      };

      if is_conf {
        if upd.json_field_key == "flush_multiplier" {
          if let Some(app_map) = val.get_mut("app").and_then(|a| a.as_object_mut()) {
            let formatted = upd
              .new_clean_value
              .parse::<f64>()
              .map(|v| format!("{:.6}", v))
              .unwrap_or_else(|_| upd.new_clean_value.clone());
            app_map.insert(
              "flush_multiplier".to_string(),
              serde_json::Value::String(formatted),
            );
          }
        } else if upd.json_field_key == "flush_volumes_matrix" {
          if let Some(arr) = val
            .get_mut("anycubic_presets")
            .and_then(|a| a.as_array_mut())
          {
            for preset in arr.iter_mut() {
              let mach = preset
                .get("machine")
                .and_then(|m| m.as_str())
                .unwrap_or("")
                .to_string();
              if mach.contains("Kobra S1") {
                if let Some(pmap) = preset.as_object_mut() {
                  pmap.insert(
                    "flush_volumes_matrix".to_string(),
                    serde_json::Value::String(upd.new_clean_value.clone()),
                  );
                }
              }
            }
          }
        }
      } else if let Some(map) = val.as_object_mut() {
        if upd.json_field_key == "wall_generator,print_sequence" {
          let parts: Vec<&str> = upd.new_clean_value.split(',').collect();
          if parts.len() == 2 {
            set_json_field_preserving_shape(map, "wall_generator", parts[0].trim());
            set_json_field_preserving_shape(map, "print_sequence", parts[1].trim());
          }
        } else {
          set_json_field_preserving_shape(map, &upd.json_field_key, &upd.new_clean_value);
          if upd.json_field_key == "nozzle_temperature_HS"
            && map.contains_key("nozzle_temperature_initial_layer_HS")
          {
            set_json_field_preserving_shape(
              map,
              "nozzle_temperature_initial_layer_HS",
              &upd.new_clean_value,
            );
          }
          if upd.json_field_key == "textured_plate_temp" && map.contains_key("hot_plate_temp") {
            set_json_field_preserving_shape(map, "hot_plate_temp", &upd.new_clean_value);
          }
        }
      }

      if let Ok(pretty) = serde_json::to_string_pretty(&val) {
        entry.bytes = pretty.as_bytes().to_vec();
        modified_paths.insert(entry.relative_path.clone(), pretty);
      }
    }
  }

  let mut out: Vec<LocalFsSaveFileEntry> = modified_paths
    .into_iter()
    .map(|(relative_path, content)| LocalFsSaveFileEntry {
      relative_path,
      content,
    })
    .collect();
  out.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
  out
}

/// Synchronizes all live `filaments` (including newly added `OC:` spools), `machines`, `processes`,
/// `AnycubicSlicerNext.conf` active slot bindings, and staged `PendingDiskUpdate`s into `entries`,
/// returning all new or modified `LocalFsSaveFileEntry` items for direct disk save or `.zip` export.
pub fn sync_live_state_to_entries(
  entries: &mut Vec<ScannedFileEntry>,
  machines: &[MachineRig],
  active_machine_id: &str,
  filaments: &[FilamentSpool],
  processes: &[ProcessProfile],
  active_process_id: &str,
  flush_matrix: &[String],
  flush_multiplier: f64,
  conf_relative_path: &str,
  pending_updates: &[PendingDiskUpdate],
) -> Vec<LocalFsSaveFileEntry> {
  let mut modified_map: HashMap<String, String> = HashMap::new();

  // 1. Detect user_folder_id from existing `user/<id>/...` entries, defaulting to "default"
  let mut user_folder_id = "default".to_string();
  for entry in entries.iter() {
    let rel = entry.relative_path.replace('\\', "/");
    if let Some(after_user) = rel.strip_prefix("user/") {
      if let Some(seg) = after_user.split('/').next() {
        if !seg.is_empty() {
          user_folder_id = seg.to_string();
          break;
        }
      }
    }
  }

  let active_rig = machines
    .iter()
    .find(|m| m.id == active_machine_id)
    .or_else(|| machines.first())
    .cloned()
    .unwrap_or_else(MachineRig::kobra_s1_hardened_steel_png);

  // Heal any existing `user/` `.json` entry paths that contain `:` or extra `/` inside the profile name
  for entry in entries.iter_mut() {
    if entry.relative_path.starts_with("user/") && entry.relative_path.ends_with(".json") {
      let healed_rel = crate::models::sanitize_user_relative_path(&entry.relative_path);
      if healed_rel != entry.relative_path {
        if let Some((_, clean_fname)) = healed_rel.rsplit_once('/') {
          entry.filename = clean_fname.to_string();
        }
        entry.relative_path = healed_rel;
      }
    }
  }

  // 2. Ensure every MachineRig in `machines` has an up-to-date file in `user/<user_folder_id>/machine/`
  for rig in machines {
    let fname = crate::models::sanitize_profile_filename(&rig.name);
    let rel_path = format!("user/{}/machine/{}", user_folder_id, fname);
    let (orca_noz, _) = rig.orca_machine_nozzle_type_and_hrc();
    let noz_diam_str = rig.formatted_nozzle_diameter();
    let machine_json = serde_json::to_string_pretty(&serde_json::json!({
      "type": "machine",
      "from": "User",
      "is_custom_defined": "0",
      "inherits": format!("{} {} nozzle", rig.printer_model, noz_diam_str),
      "name": rig.name,
      "printer_settings_id": rig.name,
      "printer_model": rig.printer_model,
      "printer_variant": noz_diam_str,
      "nozzle_type": orca_noz,
      "nozzle_diameter": [noz_diam_str],
      "retraction_length": [format!("{:.2}", rig.base_retraction_length)],
      "retraction_speed": ["40"],
      "printer_notes": rig.notes,
      "version": "2.0.0.0"
    }))
    .unwrap_or_else(|_| "{}".to_string());

    if let Some(existing) = entries.iter_mut().find(|e| {
      e.relative_path == rel_path
        || e.filename == fname
        || rig
          .source_disk_profile
          .as_deref()
          .map(|sp| sp == e.relative_path)
          .unwrap_or(false)
    }) {
      if !rig.id.starts_with("disk_")
        || rig.name.starts_with("OC -")
        || existing.bytes != machine_json.as_bytes()
      {
        if rig.source_disk_profile.is_none() || rig.name.starts_with("OC -") {
          existing.bytes = machine_json.as_bytes().to_vec();
          modified_map.insert(existing.relative_path.clone(), machine_json);
        }
      }
    } else {
      entries.push(ScannedFileEntry {
        relative_path: rel_path.clone(),
        filename: fname,
        bytes: machine_json.as_bytes().to_vec(),
      });
      modified_map.insert(rel_path, machine_json);
    }
  }

  // 3. Ensure every ProcessProfile in `processes` has a file in `user/<user_folder_id>/process/`
  // and prune any stale `OC` process files from earlier naming conventions
  let valid_proc_fnames: std::collections::HashSet<String> = processes
    .iter()
    .map(|p| crate::models::sanitize_profile_filename(&p.name))
    .collect();
  entries.retain(|e| {
    if e.relative_path.contains("/process/") && e.filename.starts_with("OC") {
      return valid_proc_fnames.contains(&e.filename);
    }
    true
  });

  let compat_printer = format!(
    "{} {} nozzle",
    active_rig.printer_model,
    active_rig.formatted_nozzle_diameter()
  );
  for proc in processes {
    let fname = crate::models::sanitize_profile_filename(&proc.name);
    let rel_path = format!("user/{}/process/{}", user_folder_id, fname);
    let proc_json = serde_json::to_string_pretty(&serde_json::json!({
      "type": "process",
      "from": "User",
      "is_custom_defined": "0",
      "inherits": proc.inherits,
      "compatible_printers": [compat_printer],
      "name": proc.name,
      "print_settings_id": proc.name,
      "layer_height": format!("{:.2}", proc.layer_height),
      "line_width": format!("{:.2}", proc.line_width),
      "outer_wall_line_width": format!("{:.2}", proc.line_width),
      "outer_wall_speed": format!("{:.0}", proc.outer_wall_speed),
      "inner_wall_speed": format!("{:.0}", proc.inner_wall_speed),
      "sparse_infill_speed": format!("{:.0}", proc.sparse_infill_speed),
      "initial_layer_speed": format!("{:.0}", proc.initial_layer_speed),
      "default_acceleration": format!("{:.0}", proc.default_acceleration),
      "wall_generator": proc.wall_generator,
      "wall_transition_angle": "10",
      "wall_transition_length": "100%",
      "min_feature_size": "25%",
      "min_bead_width": "85%",
      "seam_slope_type": "all",
      "seam_slope_conditional": "1",
      "seam_slope_start_height": "10%",
      "seam_slope_entire_loop": "0",
      "seam_slope_inner_walls": "1",
      "seam_slope_min_length": "15",
      "seam_slope_steps": "15",
      "seam_gap": "10%",
      "scarf_angle_threshold": "155",
      "wipe_on_loops": "1",
      "sparse_infill_pattern": "gyroid",
      "reduce_crossing_wall": "1",
      "print_sequence": proc.print_sequence,
      "notes": proc.notes,
      "version": "2.0.0.0"
    }))
    .unwrap_or_else(|_| "{}".to_string());

    if let Some(existing) = entries
      .iter_mut()
      .find(|e| e.relative_path == rel_path || e.filename == fname)
    {
      if !proc.id.starts_with("disk_") || existing.bytes != proc_json.as_bytes() {
        existing.bytes = proc_json.as_bytes().to_vec();
        modified_map.insert(existing.relative_path.clone(), proc_json);
      }
    } else {
      entries.push(ScannedFileEntry {
        relative_path: rel_path.clone(),
        filename: fname,
        bytes: proc_json.as_bytes().to_vec(),
      });
      modified_map.insert(rel_path, proc_json);
    }
  }

  // 4. Synchronize all `filaments` (including newly added `OC:` spools and edited spools)
  // into `user/<user_folder_id>/filament/<sanitized_name>.json`
  for spool in filaments {
    let fname = crate::models::sanitize_profile_filename(&spool.name);
    let raw_fname = format!("{}.json", spool.name);
    let default_rel_path = format!("user/{}/filament/{}", user_folder_id, fname);
    let fil_json = crate::threemf::export_standalone_filament_json(spool, &active_rig);

    if let Some(existing) = entries.iter_mut().find(|e| {
      e.relative_path == default_rel_path
        || (e.relative_path.contains("/filament/")
          && (e.filename == fname || e.filename == raw_fname))
    }) {
      // If this is a newly added/custom OC: spool or its bytes differ, update it
      if !spool.id.starts_with("disk_") || existing.bytes != fil_json.as_bytes() {
        existing.bytes = fil_json.as_bytes().to_vec();
        modified_map.insert(existing.relative_path.clone(), fil_json);
      }
    } else {
      entries.push(ScannedFileEntry {
        relative_path: default_rel_path.clone(),
        filename: fname,
        bytes: fil_json.as_bytes().to_vec(),
      });
      modified_map.insert(default_rel_path, fil_json);
    }
  }

  // 5. Ensure `AnycubicSlicerNext.conf` exists and reflects active machine, process, AMS slots, colors, and flush matrix
  let conf_path = if conf_relative_path.trim().is_empty() {
    "AnycubicSlicerNext.conf".to_string()
  } else {
    conf_relative_path.to_string()
  };
  let ams_slots = (active_rig.ace_pro_units * 4).max(1).min(filaments.len());
  let active_slots: Vec<&FilamentSpool> = filaments.iter().take(ams_slots).collect();
  let active_proc_name = processes
    .iter()
    .find(|p| p.id == active_process_id)
    .or_else(|| processes.first())
    .map(|p| p.name.clone())
    .unwrap_or_else(|| "0.20mm Standard @Anycubic Kobra S1 0.4 nozzle".to_string());
  let matrix_pipe = flush_matrix
    .iter()
    .map(|v| {
      v.parse::<f64>()
        .map(|f| format!("{:.6}", f))
        .unwrap_or_else(|_| format!("{}.000000", v))
    })
    .collect::<Vec<_>>()
    .join("|");
  let colors_csv = active_slots
    .iter()
    .map(|s| s.color_hex.clone())
    .collect::<Vec<_>>()
    .join(",");

  if !entries.iter().any(|e| e.relative_path == conf_path) {
    let mut preset_obj = serde_json::Map::new();
    preset_obj.insert(
      "machine".to_string(),
      serde_json::Value::String(active_rig.name.clone()),
    );
    preset_obj.insert(
      "process".to_string(),
      serde_json::Value::String(active_proc_name),
    );
    for (idx, sp) in active_slots.iter().enumerate() {
      let key = if idx == 0 {
        "filament".to_string()
      } else {
        format!("filament_{:02}", idx)
      };
      preset_obj.insert(key, serde_json::Value::String(sp.name.clone()));
    }
    preset_obj.insert(
      "filament_colors".to_string(),
      serde_json::Value::String(colors_csv),
    );
    if !matrix_pipe.is_empty() {
      preset_obj.insert(
        "flush_volumes_matrix".to_string(),
        serde_json::Value::String(matrix_pipe),
      );
    }

    let conf_json = serde_json::to_string_pretty(&serde_json::json!({
      "app": {
        "preset_folder": user_folder_id,
        "version": "2.0.0.3",
        "flush_multiplier": format!("{:.6}", flush_multiplier)
      },
      "models": [{
        "model": active_rig.printer_model,
        "nozzle_diameter": active_rig.formatted_nozzle_diameter(),
        "vendor": "Anycubic"
      }],
      "presets": {
        "machine": active_rig.name
      },
      "anycubic_presets": [serde_json::Value::Object(preset_obj)]
    }))
    .unwrap_or_else(|_| "{}".to_string());

    let fname = conf_path
      .rsplit('/')
      .next()
      .unwrap_or("AnycubicSlicerNext.conf")
      .to_string();
    entries.push(ScannedFileEntry {
      relative_path: conf_path.clone(),
      filename: fname,
      bytes: conf_json.as_bytes().to_vec(),
    });
    modified_map.insert(conf_path, conf_json);
  }

  // 6. Apply any staged `PendingDiskUpdate` items on top (ignoring stale OC process filenames
  // and skipping `notes` / `printer_notes` since Steps 2-4 already write the exact scalar string `notes` / `printer_notes`!)
  let filtered_updates: Vec<PendingDiskUpdate> = pending_updates
    .iter()
    .filter(|upd| {
      if upd.json_field_key == "notes" || upd.json_field_key == "printer_notes" {
        return false;
      }
      let clean_rel = crate::models::sanitize_user_relative_path(&upd.source_relative_path);
      if clean_rel.contains("/process/OC") {
        let fname = clean_rel.rsplit('/').next().unwrap_or_default();
        return valid_proc_fnames.contains(fname);
      }
      true
    })
    .cloned()
    .collect();
  for upd_file in apply_pending_updates_to_entries(entries, &filtered_updates) {
    modified_map.insert(upd_file.relative_path, upd_file.content);
  }

  let mut out: Vec<LocalFsSaveFileEntry> = modified_map
    .into_iter()
    .map(|(relative_path, content)| LocalFsSaveFileEntry {
      relative_path,
      content,
    })
    .collect();
  out.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
  out
}

/// Build a faithful in-memory snapshot of `/mnt/c/Users/drpng/AppData/Roaming/AnycubicSlicerNext`
/// for unit testing and quick verification
pub fn build_reference_anycubic_entries() -> Vec<ScannedFileEntry> {
  vec![
    ScannedFileEntry {
      relative_path: "AnycubicSlicerNext.conf".to_string(),
      filename: "AnycubicSlicerNext.conf".to_string(),
      bytes: br##"{
        "app": { "preset_folder": "434139", "version": "2.0.0.3", "flush_multiplier": "1.000000" },
        "models": [{ "model": "Anycubic Kobra S1", "nozzle_diameter": "0.4", "vendor": "Anycubic" }],
        "presets": { "machine": "Anycubic Kobra S1 0.4 nozzle - png" },
        "anycubic_presets": [{
          "machine": "Anycubic Kobra S1 0.4 nozzle - png",
          "process": "0.20mm Standard @AC KS1 - png",
          "filament": "CHCKX PETG PCTG @Anycubic Kobra S1 0.4 nozzle",
          "filament_01": "Anycubic TPU @Anycubic Kobra S1 0.4 nozzle",
          "filament_02": "FLASHFORGE PLA Carbon @Anycubic Kobra S1 0.4 nozzle",
          "filament_03": "Anycubic PETG @Anycubic Kobra S1 0.4 nozzle - png",
          "filament_04": "Anycubic PETG - hotbed",
          "filament_05": "Anycubic PLA+ hotbed",
          "filament_colors": "#658946,#E10600,#AFAFAF,#75787B,#CDA077,#7841CE",
          "flush_volumes_matrix": "0.000000|255.000000|301.000000|138.000000|296.000000|185.000000|405.000000|0.000000|508.000000|402.000000|446.000000|311.000000|141.000000|277.000000|0.000000|131.000000|149.000000|183.000000|134.000000|251.000000|266.000000|0.000000|301.000000|177.000000|154.000000|227.000000|175.000000|162.000000|0.000000|167.000000|280.000000|256.000000|402.000000|275.000000|367.000000|0.000000"
        }]
      }
      # MD5 checksum ECE7CFFD5492599DE99B68BD041585FF"##
        .to_vec(),
    },
    ScannedFileEntry {
      relative_path: "system/Anycubic/machine/Anycubic Kobra S1 0.4 nozzle.json".to_string(),
      filename: "Anycubic Kobra S1 0.4 nozzle.json".to_string(),
      bytes: br#"{
        "name": "Anycubic Kobra S1 0.4 nozzle",
        "nozzle_type": "brass",
        "retraction_length": ["0.8"],
        "retraction_speed": ["40"]
      }"#
        .to_vec(),
    },
    ScannedFileEntry {
      relative_path: "system/Anycubic/filament/Anycubic PETG @Anycubic Kobra S1 0.4 nozzle.json"
        .to_string(),
      filename: "Anycubic PETG @Anycubic Kobra S1 0.4 nozzle.json".to_string(),
      bytes: br#"{
        "name": "Anycubic PETG @Anycubic Kobra S1 0.4 nozzle",
        "filament_type": ["PETG"],
        "nozzle_temperature": ["230"],
        "textured_plate_temp": ["75"],
        "filament_flow_ratio": ["0.96"],
        "filament_max_volumetric_speed": ["12"],
        "pressure_advance": ["0.04"]
      }"#
        .to_vec(),
    },
    ScannedFileEntry {
      relative_path: "system/Anycubic/filament/Anycubic PLA+ @Anycubic Kobra S1 0.4 nozzle.json"
        .to_string(),
      filename: "Anycubic PLA+ @Anycubic Kobra S1 0.4 nozzle.json".to_string(),
      bytes: br#"{
        "name": "Anycubic PLA+ @Anycubic Kobra S1 0.4 nozzle",
        "filament_type": ["PLA"],
        "nozzle_temperature": ["205"],
        "nozzle_temperature_HS": ["220"],
        "textured_plate_temp": ["55"],
        "filament_flow_ratio": ["0.96"],
        "filament_max_volumetric_speed": ["15"],
        "pressure_advance": ["0.04"]
      }"#
        .to_vec(),
    },
    ScannedFileEntry {
      relative_path: "user/434139/machine/Anycubic Kobra S1 0.4 nozzle - png.json".to_string(),
      filename: "Anycubic Kobra S1 0.4 nozzle - png.json".to_string(),
      bytes: br#"{
        "from": "User",
        "inherits": "Anycubic Kobra S1 0.4 nozzle",
        "name": "Anycubic Kobra S1 0.4 nozzle - png",
        "nozzle_type": "hardened_steel",
        "retraction_speed": ["35"]
      }"#
        .to_vec(),
    },
    ScannedFileEntry {
      relative_path: "user/434139/machine/Anycubic Kobra S1 -NewStartcode V4 -0.4n -Next.json"
        .to_string(),
      filename: "Anycubic Kobra S1 -NewStartcode V4 -0.4n -Next.json".to_string(),
      bytes: br#"{
        "from": "User",
        "inherits": "Anycubic Kobra S1 0.4 nozzle",
        "name": "Anycubic Kobra S1 -NewStartcode V4 -0.4n -Next",
        "nozzle_type": "hardened_steel",
        "retraction_length": ["0.65"],
        "retraction_speed": ["70"],
        "retraction_distances_when_cut": ["18"]
      }"#
        .to_vec(),
    },
    ScannedFileEntry {
      relative_path: "user/434139/filament/Anycubic PETG - hotbed.json".to_string(),
      filename: "Anycubic PETG - hotbed.json".to_string(),
      bytes: br#"{
        "from": "User",
        "inherits": "Anycubic PETG @Anycubic Kobra S1 0.4 nozzle",
        "name": "Anycubic PETG - hotbed",
        "nozzle_temperature_HS": ["235"],
        "nozzle_temperature_initial_layer_HS": ["235"],
        "textured_plate_temp": ["82"],
        "textured_plate_temp_initial_layer": ["82"]
      }"#
        .to_vec(),
    },
    ScannedFileEntry {
      relative_path: "user/434139/filament/Anycubic PETG @Anycubic Kobra S1 0.4 nozzle - png.json"
        .to_string(),
      filename: "Anycubic PETG @Anycubic Kobra S1 0.4 nozzle - png.json".to_string(),
      bytes: br#"{
        "from": "User",
        "inherits": "Anycubic PETG @Anycubic Kobra S1 0.4 nozzle",
        "name": "Anycubic PETG @Anycubic Kobra S1 0.4 nozzle - png",
        "textured_plate_temp": ["80"],
        "textured_plate_temp_initial_layer": ["80"]
      }"#
        .to_vec(),
    },
    ScannedFileEntry {
      relative_path: "user/434139/filament/Anycubic PLA+ hotbed.json".to_string(),
      filename: "Anycubic PLA+ hotbed.json".to_string(),
      bytes: br#"{
        "from": "User",
        "inherits": "Anycubic PLA+ @Anycubic Kobra S1 0.4 nozzle",
        "name": "Anycubic PLA+ hotbed",
        "nozzle_temperature_HS": ["230"],
        "nozzle_temperature_initial_layer_HS": ["230"],
        "textured_plate_temp": ["60"],
        "textured_plate_temp_initial_layer": ["60"]
      }"#
        .to_vec(),
    },
    ScannedFileEntry {
      relative_path: "user/434139/filament/base/CHCKX PETG PCTG @Anycubic Kobra S1 0.4 nozzle.json"
        .to_string(),
      filename: "CHCKX PETG PCTG @Anycubic Kobra S1 0.4 nozzle.json".to_string(),
      bytes: br#"{
        "from": "User",
        "inherits": "",
        "name": "CHCKX PETG PCTG @Anycubic Kobra S1 0.4 nozzle",
        "filament_type": ["PETG"],
        "filament_vendor": ["CHCKX"],
        "nozzle_temperature": ["230"],
        "nozzle_temperature_BRASS": ["230"],
        "nozzle_temperature_HS": ["260"],
        "nozzle_temperature_range_low": ["250"],
        "textured_plate_temp": ["85"],
        "textured_plate_temp_initial_layer": ["80"],
        "filament_flow_ratio": ["0.96"],
        "pressure_advance": ["0.08"]
      }"#
        .to_vec(),
    },
    ScannedFileEntry {
      relative_path:
        "user/434139/filament/base/FLASHFORGE PLA Carbon @Anycubic Kobra S1 0.4 nozzle.json"
          .to_string(),
      filename: "FLASHFORGE PLA Carbon @Anycubic Kobra S1 0.4 nozzle.json".to_string(),
      bytes: br#"{
        "from": "User",
        "inherits": "",
        "name": "FLASHFORGE PLA Carbon @Anycubic Kobra S1 0.4 nozzle",
        "filament_type": ["PLA"],
        "filament_vendor": ["FLASHFORGE"],
        "nozzle_temperature": ["205"],
        "textured_plate_temp": ["55"],
        "filament_flow_ratio": ["0.98"],
        "pressure_advance": ["0.035"],
        "required_nozzle_HRC": ["3"]
      }"#
        .to_vec(),
    },
    ScannedFileEntry {
      relative_path:
        "user/434139/filament/base/yxpoler ABS blue @Anycubic Kobra S1 0.4 nozzle.json"
          .to_string(),
      filename: "yxpoler ABS blue @Anycubic Kobra S1 0.4 nozzle.json".to_string(),
      bytes: br#"{
        "from": "User",
        "inherits": "",
        "name": "yxpoler ABS blue @Anycubic Kobra S1 0.4 nozzle",
        "filament_type": ["ABS"],
        "filament_vendor": ["yxpoler"],
        "nozzle_temperature": ["250"],
        "nozzle_temperature_BRASS": ["260"],
        "nozzle_temperature_HS": ["260"],
        "nozzle_temperature_initial_layer_HS": ["265"],
        "textured_plate_temp": ["88"],
        "textured_plate_temp_initial_layer": ["90"],
        "fan_max_speed_BRASS": ["20"],
        "fan_max_speed_HS": ["80"],
        "filament_flow_ratio": ["0.93"],
        "pressure_advance": ["0.04"]
      }"#
        .to_vec(),
    },
    ScannedFileEntry {
      relative_path: "user/434139/filament/base/yxpoler PA @Anycubic Kobra S1 0.4 nozzle.json"
        .to_string(),
      filename: "yxpoler PA @Anycubic Kobra S1 0.4 nozzle.json".to_string(),
      bytes: br#"{
        "from": "User",
        "inherits": "",
        "name": "yxpoler PA @Anycubic Kobra S1 0.4 nozzle",
        "filament_type": ["PA"],
        "filament_vendor": ["yxpoler"],
        "nozzle_temperature": ["250"],
        "nozzle_temperature_BRASS": ["260"],
        "nozzle_temperature_HS": ["275"],
        "nozzle_temperature_initial_layer_HS": ["270"],
        "textured_plate_temp": ["95"],
        "textured_plate_temp_initial_layer": ["95"],
        "fan_max_speed_BRASS": ["20"],
        "fan_max_speed_HS": ["80"],
        "filament_flow_ratio": ["0.93"],
        "pressure_advance": ["0.04"]
      }"#
        .to_vec(),
    },
    ScannedFileEntry {
      relative_path: "user/434139/filament/yxpoler PA @Anycubic Kobra S1 0.4 nozzle.json"
        .to_string(),
      filename: "yxpoler PA @Anycubic Kobra S1 0.4 nozzle.json".to_string(),
      bytes: br#"{
        "from": "User",
        "inherits": "yxpoler ABS blue @Anycubic Kobra S1 0.4 nozzle",
        "name": "yxpoler Nylon blue PA6 @Anycubic Kobra S1 0.4 nozzle",
        "filament_type": ["PA"]
      }"#
        .to_vec(),
    },
    ScannedFileEntry {
      relative_path: "user/434139/process/0.20mm Standard @AC KS1 - png.json".to_string(),
      filename: "0.20mm Standard @AC KS1 - png.json".to_string(),
      bytes: br#"{
        "from": "User",
        "inherits": "0.20mm Standard @Anycubic Kobra S1 0.4 nozzle",
        "name": "0.20mm Standard @AC KS1 - png",
        "print_sequence": "by object",
        "wall_generator": "arachne"
      }"#
        .to_vec(),
    },
  ]
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_import_scanned_directory_detects_kobra_s1_hardened_nozzle_and_deltas() {
    let entries = build_reference_anycubic_entries();
    let summary = import_scanned_directory(&entries).unwrap();

    assert_eq!(summary.user_folder_id, "434139");
    assert_eq!(summary.machines.len(), 2);

    let active_rig = summary
      .machines
      .iter()
      .find(|m| m.id == summary.active_machine_id)
      .unwrap();
    assert_eq!(active_rig.name, "Anycubic Kobra S1 0.4 nozzle - png");
    assert_eq!(active_rig.nozzle_type, "hardened_steel");
    assert_eq!(active_rig.nozzle_material, "Hardened Steel");

    // Verify Anycubic PETG - hotbed was understood as +5°C HS nozzle rule (230 -> 235) and +7°C bed delta (75 -> 82)
    let petg_hotbed = summary
      .filaments
      .iter()
      .find(|f| f.name == "Anycubic PETG - hotbed")
      .unwrap();
    assert_eq!(petg_hotbed.base_print_temp, 230);
    assert_eq!(petg_hotbed.print_temp, 235);
    assert!(petg_hotbed
      .deltas_from_base
      .iter()
      .any(|d| d.param_key == "nozzle_temperature_HS" && d.reconstructed_by_lua));

    // Verify Anycubic PLA+ hotbed compares HS-to-HS (220°C System HS -> 230°C User HS), NOT against 205°C Brass
    let pla_plus_hotbed = summary
      .filaments
      .iter()
      .find(|f| f.name == "Anycubic PLA+ hotbed")
      .unwrap();
    assert_eq!(pla_plus_hotbed.base_print_temp, 220);
    assert_eq!(pla_plus_hotbed.print_temp, 230);

    // Verify machine retraction speed is compared against 40 mm/s (stock direct-drive standard), not 70 mm/s
    let v4_retract_conflict = summary
      .conflicts
      .iter()
      .find(|c| {
        c.param_key == "machine:retraction_speed (Anycubic Kobra S1 -NewStartcode V4 -0.4n -Next)"
      })
      .unwrap();
    assert_eq!(v4_retract_conflict.clean_raw_value, "40");
    assert_eq!(v4_retract_conflict.disk_raw_value, "70");

    // Verify all 3 Quality/Speed Toggle presets ("high_quality", "standard", "high_speed") exist
    assert!(summary
      .processes
      .iter()
      .any(|p| p.quality_tier() == "high_quality"));
    assert!(summary
      .processes
      .iter()
      .any(|p| p.quality_tier() == "standard"));
    assert!(summary
      .processes
      .iter()
      .any(|p| p.quality_tier() == "high_speed"));

    // Verify descriptive notes and concise titles for Machine and Process profiles (prefixed with OC - and without ACE8 or Windows-forbidden ':')
    assert_eq!(
      active_rig.format_concise_title(),
      "OC - Kobra S1 • 0.4 Hardened"
    );
    assert_eq!(
      active_rig.orca_machine_nozzle_type_and_hrc(),
      ("hardened_steel", 55)
    );
    assert!(active_rig.notes.contains("[OrcaSlicer WebConfig Machine]"));
    assert!(summary
      .processes
      .iter()
      .all(|p| p.notes.contains("[OrcaSlicer WebConfig Process")));

    // Verify standalone base profiles PCTG and PLA-CF have accurate computed values and zero false conflicts
    let pctg = summary
      .filaments
      .iter()
      .find(|f| f.name.contains("CHCKX PETG PCTG"))
      .unwrap();
    assert_eq!(pctg.material_type, "PCTG");
    assert_eq!(pctg.base_print_temp, 260);
    assert_eq!(pctg.print_temp, 260);
    assert_eq!(pctg.bed_temp, 85);
    assert_eq!(pctg.nozzle_type, "hardened_steel");
    assert!(pctg.deltas_from_base.is_empty());
    assert!(!summary
      .conflicts
      .iter()
      .any(|c| c.param_key.contains("CHCKX PETG PCTG")));

    let pla_cf = summary
      .filaments
      .iter()
      .find(|f| f.name.contains("FLASHFORGE PLA Carbon"))
      .unwrap();
    assert_eq!(pla_cf.material_type, "PLA-CF");
    assert_eq!(pla_cf.base_print_temp, 205);
    assert_eq!(pla_cf.print_temp, 210);
    assert_eq!(pla_cf.bed_temp, 55);
    assert_eq!(pla_cf.required_nozzle_hrc, 55);
    assert_eq!(pla_cf.nozzle_type, "hardened_steel");
    assert_eq!(pla_cf.deltas_from_base.len(), 1);
    assert_eq!(
      pla_cf.deltas_from_base[0].rule_badge_label(),
      "Rule: Thermal Conductivity & Nozzle"
    );
    assert!((pla_cf.pigment_stickiness - 1.15).abs() < 1e-4);
    assert_eq!(pla_cf.drying_temp_c, 45);
    assert!(!summary
      .conflicts
      .iter()
      .any(|c| c.param_key.contains("FLASHFORGE PLA Carbon")));

    // Verify PLA+ drying temp is 45°C (not 55°C) and recompute_for_nozzle never uses Hardened Steel values on Brass/Diamondback
    let mut pla_plus_clone = pla_plus_hotbed.clone();
    assert_eq!(pla_plus_clone.drying_temp_c, 45);
    assert_eq!(pla_plus_clone.brass_base_temp, 205);
    assert_eq!(pla_plus_clone.hs_base_temp, 220);
    pla_plus_clone.recompute_for_nozzle("brass");
    assert_eq!(pla_plus_clone.print_temp, 205);
    assert!(pla_plus_clone.compatibility_warning("brass", 0).is_some());
    pla_plus_clone.recompute_for_nozzle("diamondback");
    assert_eq!(pla_plus_clone.print_temp, 195);

    // Verify imported disk flushing volume matrix (6x6 = 36 cells) and flush_multiplier (1.0)
    assert_eq!(summary.disk_flush_slot_count, 6);
    assert_eq!(summary.disk_flush_matrix.len(), 36);
    assert_eq!(summary.disk_flush_matrix[1], "255");
    assert_eq!(summary.disk_flush_multiplier, 1.0);

    // Verify conflicts were flagged (e.g. retraction_speed = 35 vs 40, textured_plate_temp = 82 vs 80)
    assert!(!summary.conflicts.is_empty());

    // Verify Safety & Weird Value Validator catches all unsafe (Danger) and weird (Warning) items
    let active_rig = summary.machines.first().unwrap();
    let safety_warnings = crate::transformations::TransformationEngine::evaluate_safety_warnings(
      active_rig,
      &summary.filaments,
      &summary.base_profile,
      &summary.conflicts,
      &summary.disk_flush_matrix,
    );
    assert!(safety_warnings.iter().any(|w| {
      w.severity == crate::transformations::SafetySeverity::Danger
        && w
          .param_key
          .contains("Inherited Base Profile (Polymer Mismatch)")
    }));
    assert!(safety_warnings.iter().any(|w| {
      w.severity == crate::transformations::SafetySeverity::Danger
        && w.param_key.contains("Max Cooling Fan (HS)")
    }));
    assert!(safety_warnings.iter().any(|w| {
      w.severity == crate::transformations::SafetySeverity::Danger
        && w
          .param_key
          .contains("nozzle_temperature_BRASS vs range_low")
    }));
    assert!(safety_warnings.iter().any(|w| {
      w.severity == crate::transformations::SafetySeverity::Warning
        && w.param_key.contains("pressure_advance")
    }));
    assert!(safety_warnings.iter().any(|w| {
      w.severity == crate::transformations::SafetySeverity::Warning
        && w.param_key.contains("required_nozzle_HRC (On Disk)")
    }));

    // Verify ZIP backup contains all scanned files and round-trips through extract_directory_backup_zip
    let backup_zip = build_directory_backup_zip(&entries);
    assert_eq!(&backup_zip[0..4], b"PK\x03\x04");
    let extracted = extract_directory_backup_zip(&backup_zip).unwrap();
    assert_eq!(extracted.len(), entries.len());
    let reimported = import_scanned_directory(&extracted).unwrap();
    assert_eq!(reimported.user_folder_id, "434139");
  }

  #[test]
  fn test_zip_export_sanitizes_filenames_and_sets_utf8_flag() {
    let catalog = crate::catalog::load_merged_catalog();
    let petg_cf = catalog
      .filaments
      .iter()
      .find(|f| f.code == "PETG-CF")
      .expect("PETG-CF should exist in catalog");
    let spool = FilamentSpool::from_catalog_entry(1, petg_cf, "hardened_steel");
    assert_eq!(spool.name, "OC - PETG-CF • 0.4 Hardened");
    assert_eq!(
      crate::models::sanitize_profile_filename("OC: PETG-CF • 0.4 Hardened"),
      "OC - PETG-CF • 0.4 Hardened.json"
    );
    assert_eq!(
      crate::models::sanitize_user_relative_path(
        "user/default/process/OC: 0.20mm Std • 200/300 • Arachne • By-Obj @Kobra S1.json"
      ),
      "user/default/process/OC - 0.20mm Std • 200-300 • Arachne • By-Obj @Kobra S1.json"
    );

    let rig = MachineRig::kobra_s1_hardened_steel_png();
    let procs = ProcessProfile::get_default_processes();
    let mut entries = Vec::new();
    let legacy_upd = PendingDiskUpdate {
      param_key: "process.notes".to_string(),
      param_label: "Process Profile Notes".to_string(),
      source_relative_path:
        "user/default/process/OC: 0.20mm Std • 200/300 • Arachne • By-Obj @KS1.json".to_string(),
      json_field_key: "notes".to_string(),
      old_disk_value: "(none)".to_string(),
      new_clean_value: procs[2].notes.clone(),
    };
    let _ = sync_live_state_to_entries(
      &mut entries,
      std::slice::from_ref(&rig),
      &rig.id,
      std::slice::from_ref(&spool),
      &procs,
      &procs[0].id,
      &["0".to_string()],
      1.0,
      "AnycubicSlicerNext.conf",
      &[legacy_upd],
    );

    assert!(entries
      .iter()
      .any(|e| e.relative_path == "user/default/filament/OC - PETG-CF • 0.4 Hardened.json"));
    // Verify all 5 process profiles exist right under `user/default/process/` with concise names
    // (0.20mm Standard, 0.20mm Quality, 0.20mm Speed, 0.12mm Quality, 0.28mm Speed) and no clutter
    let proc_entries: Vec<&ScannedFileEntry> = entries
      .iter()
      .filter(|e| e.relative_path.starts_with("user/default/process/"))
      .collect();
    assert_eq!(proc_entries.len(), 5);
    assert!(proc_entries
      .iter()
      .any(|e| e.filename == "OC - 0.20mm Standard @Kobra S1.json"));
    assert!(proc_entries
      .iter()
      .any(|e| e.filename == "OC - 0.20mm Quality @Kobra S1.json"));
    assert!(proc_entries
      .iter()
      .any(|e| e.filename == "OC - 0.20mm Speed @Kobra S1.json"));
    assert!(proc_entries
      .iter()
      .any(|e| e.filename == "OC - 0.12mm Quality @Kobra S1.json"));
    assert!(proc_entries
      .iter()
      .any(|e| e.filename == "OC - 0.28mm Speed @Kobra S1.json"));
    for pe in &proc_entries {
      assert_eq!(
        pe.relative_path.matches('/').count(),
        3,
        "Process path must be directly inside user/default/process/ with no subfolder: {}",
        pe.relative_path
      );
      assert!(
        !pe.filename.contains("Arachne")
          && !pe.filename.contains("By-Obj")
          && !pe.filename.contains("HQ")
          && !pe.filename.contains("Std")
          && !pe.filename.contains("High"),
        "Process filename should use Quality/Speed/Standard without High/Arachne/By-Obj/speed clutter: {}",
        pe.filename
      );
    }
    for entry in &entries {
      assert!(
        !entry.filename.contains(':'),
        "ZIP filename must not contain ':': {}",
        entry.filename
      );
      if entry.relative_path.starts_with("user/") && entry.relative_path.ends_with(".json") {
        let val: serde_json::Value =
          serde_json::from_slice(&entry.bytes).expect("Valid JSON preset in ZIP");
        let name_str = val["name"].as_str().expect("name must be string");
        assert!(
          !name_str.contains(':'),
          "Internal JSON 'name' in {} must NEVER contain ':' (Windows NTFS forbidden char): {}",
          entry.relative_path,
          name_str
        );
        if let Some(nd) = val.get("nozzle_diameter") {
          assert_eq!(nd, &serde_json::json!(["0.4"]));
        }
        if let Some(notes) = val.get("notes") {
          assert!(
            notes.is_string(),
            "Process/machine 'notes' in {} must be a scalar string, not an array: {:?}",
            entry.relative_path,
            notes
          );
        }
      }
    }

    let zip_bytes = build_directory_backup_zip(&entries);
    // Verify Bit 11 (0x0800 UTF-8 Language Encoding Flag) is set in the Local File Header
    let local_flags = u16::from_le_bytes([zip_bytes[6], zip_bytes[7]]);
    assert_eq!(local_flags & 0x0800, 0x0800);
  }
}
