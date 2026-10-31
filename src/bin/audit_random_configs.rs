use orcaslicer_webconfig::catalog::{load_merged_catalog, FILAMENT_BRAND_OPTIONS, FINISH_OPTIONS};
use orcaslicer_webconfig::dir_import::{import_scanned_directory, sync_live_state_to_entries};
use orcaslicer_webconfig::fs_access::ScannedFileEntry;
use orcaslicer_webconfig::models::{
  FilamentSpool, MachineRig, ProcessProfile, WebConfigStateSnapshot, WEBCONFIG_APP_VERSION,
};
use orcaslicer_webconfig::orca_profile::OrcaProfile;
use orcaslicer_webconfig::threemf::{
  build_3mf_project_container, build_project_settings_config_json,
  parse_ace_loadout_from_bytes_or_json, resolve_effective_orca_profile_values,
};
use orcaslicer_webconfig::transformations::{SafetySeverity, TransformationEngine};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

struct SimpleRng {
  state: u64,
}

impl SimpleRng {
  fn new(seed: u64) -> Self {
    Self {
      state: if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed },
    }
  }

  fn next_u64(&mut self) -> u64 {
    let mut x = self.state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    self.state = x;
    x
  }

  fn next_usize(&mut self, len: usize) -> usize {
    if len <= 1 {
      0
    } else {
      (self.next_u64() as usize) % len
    }
  }

  fn choose<'a, T>(&mut self, slice: &'a [T]) -> &'a T {
    let idx = self.next_usize(slice.len());
    &slice[idx]
  }
}

fn first_str_val(val: &Value, key: &str) -> Option<String> {
  let v = val.get(key)?;
  if let Some(s) = v.as_str() {
    Some(s.to_string())
  } else if let Some(arr) = v.as_array() {
    arr.first()?.as_str().map(|s| s.to_string())
  } else {
    None
  }
}

fn safe_temp_bounds_for_polymer(mat: &str, finish: &str) -> (i32, i32, i32, i32) {
  let canon = FilamentSpool::canonical_polymer_key(mat);
  let up = mat.to_uppercase();
  if finish.eq_ignore_ascii_case("Foaming")
    && (canon.starts_with("TPU") || canon == "PEBA" || up.contains("AIR"))
  {
    (185, 285, 35, 85)
  } else {
    let (min_noz, max_noz) = FilamentSpool::polymer_safe_nozzle_bounds(mat);
    let (min_bed, max_bed) = FilamentSpool::polymer_safe_bed_bounds(mat);
    (min_noz, max_noz, min_bed, max_bed)
  }
}

fn apply_nozzle_metadata(rig: &mut MachineRig, nozzle_type: &str) {
  let noz_cfg = orcaslicer_webconfig::constants::constants().lookup_nozzle(nozzle_type);
  rig.nozzle_type = nozzle_type.to_string();
  rig.nozzle_material = noz_cfg.material_label.clone();
  rig.nozzle_name = noz_cfg.default_name.clone();
  let (_, hrc) = rig.orca_machine_nozzle_type_and_hrc();
  rig.nozzle_hrc = hrc;
  rig.refresh_title_and_notes(true);
}

fn run_audit_suite(out_root: &Path, num_configs: usize, seed: u64) {
  let out_root = out_root.to_path_buf();
  if out_root.exists() {
    let _ = fs::remove_dir_all(&out_root);
  }
  fs::create_dir_all(&out_root).expect("Failed to create audit root directory");

  let catalog = load_merged_catalog();
  let mut rng = SimpleRng::new(seed);

  let brands: Vec<&str> = FILAMENT_BRAND_OPTIONS
    .iter()
    .copied()
    .filter(|&b| b != "All")
    .collect();
  let core_materials = [
    "PLA",
    "PLA+",
    "PLA-CF",
    "PHA",
    "APLA",
    "PETG",
    "PETG-CF",
    "PET-CF",
    "PCTG",
    "TPU for AMS",
    "TPU 95A",
    "TPU 90A",
    "TPU 85A",
    "PEBA",
    "ASA",
    "ASA-CF",
    "ABS",
    "ABS-CF",
    "PC",
    "PC-CF",
    "PA",
    "PA6-CF",
    "PA12-CF",
    "PAHT-CF",
    "PPA-CF",
    "PVA",
    "BVOH",
    "HIPS",
  ];
  let non_abrasive_materials = [
    "PLA",
    "PLA+",
    "PHA",
    "APLA",
    "PETG",
    "PCTG",
    "TPU for AMS",
    "TPU 95A",
    "TPU 90A",
    "TPU 85A",
    "PEBA",
    "ASA",
    "ABS",
    "PC",
    "PA",
    "PVA",
    "BVOH",
    "HIPS",
  ];
  let non_abrasive_finishes = ["Basic", "Matte", "Silk", "Translucent"];
  let hardened_finishes: Vec<&str> = FINISH_OPTIONS.iter().map(|(v, _)| *v).collect();
  let nozzle_choices = [
    "orca_hardened_steel",
    "diamondback",
    "ruby_brass",
    "orca_brass",
    "cm2",
  ];
  let climate_choices = [
    ("temperate", 22, 45),
    ("tropical", 30, 78),
    ("cold_dry", 15, 25),
    ("cold_humid", 15, 72),
    ("hot_dry", 29, 25),
  ];

  println!(
    "=== Phase 1: Generating {} randomized configurations in {} ===",
    num_configs,
    out_root.display()
  );

  let mut config_dirs = Vec::new();
  for cfg_idx in 1..=num_configs {
    let cfg_dir = out_root.join(format!("config_{:03}", cfg_idx));
    fs::create_dir_all(&cfg_dir).expect("Failed to create config subdir");

    let mut rig = MachineRig::kobra_s1_hardened_steel_png();
    // Start on a random nozzle, then switch nozzles during setup to test recompute_for_nozzle
    let initial_nozzle = *rng.choose(&nozzle_choices);
    let final_nozzle = *rng.choose(&nozzle_choices);
    apply_nozzle_metadata(&mut rig, initial_nozzle);

    let (climate_preset, amb_c, rh_pct) = *rng.choose(&climate_choices);
    rig.climate_preset = climate_preset.to_string();
    rig.ambient_temp_c = amb_c;
    rig.humidity_pct = rh_pct;
    rig.ace_pro_units = if cfg_idx % 5 == 0 { 2 } else { 1 };

    let (_, final_hrc) = {
      let mut tmp = rig.clone();
      apply_nozzle_metadata(&mut tmp, final_nozzle);
      tmp.orca_machine_nozzle_type_and_hrc()
    };
    let allow_abrasive = final_hrc >= 55;

    let slot_count = match cfg_idx % 4 {
      0 => 9, // 2x ACE Pro + 1 External Spool Holder (9-slot)
      1 => 8, // 2x ACE Pro (8-slot)
      2 => 6, // 1x ACE Pro + 2 Reserve (6-slot)
      _ => 4, // 1x ACE Pro (4-slot)
    };
    rig.ace_pro_units = if slot_count >= 8 { 2 } else { 1 };
    let ams_capacity = rig.ace_pro_units * 4;
    if cfg_idx % 3 == 0 {
      rig.addons = vec![
        "flowtech_hotend".to_string(),
        "microswiss_duct".to_string(),
        "hollow_front_back_covers".to_string(),
      ];
    } else if cfg_idx % 3 == 1 {
      rig.addons = vec!["hollow_front_back_covers".to_string()];
    }
    let mut spools = Vec::new();

    for slot_i in 0..slot_count {
      // Intentionally start from a random catalog entry (often high-temp like ABS-CF / PA6-CF / PCTG)
      // and then perform multiple random brand -> material -> finish -> brand transitions!
      let start_entry = rng.choose(&catalog.filaments);
      let mut sp = FilamentSpool::from_catalog_entry(slot_i + 1, start_entry, &rig.nozzle_type);

      // Step A: switch to a high-temp or conductive/foaming state first to try to provoke stale temp bugs
      let chaos_mat = *rng.choose(&core_materials);
      sp.apply_material_change(chaos_mat, &rig.nozzle_type);
      let chaos_brand = *rng.choose(&brands);
      sp.apply_brand_change(chaos_brand, &rig.nozzle_type);
      if rng.next_usize(3) == 0 {
        sp.apply_finish_change("Conductive", &rig.nozzle_type);
      } else if rng.next_usize(3) == 0 {
        sp.apply_finish_change("Foaming", &rig.nozzle_type);
      }

      // Step B: settle on the target material, finish, and brand for this slot
      let in_active_ams = slot_i < ams_capacity;
      let mut target_mat = if allow_abrasive {
        *rng.choose(&core_materials)
      } else {
        *rng.choose(&non_abrasive_materials)
      };
      // Soft TPU (TPU 95A / TPU 90A / TPU 85A / PEBA) belongs in Reserve slots outside active AMS; active AMS slots use TPU for AMS
      if in_active_ams
        && matches!(
          target_mat,
          "TPU 95A" | "TPU 90A" | "TPU 85A" | "PEBA"
        )
      {
        target_mat = "TPU for AMS";
      }
      // Guarantee that config_001..config_010 explicitly test PHA, TPU for AMS, and PETG combinations
      if cfg_idx <= 10 {
        match slot_i {
          0 => target_mat = "TPU for AMS",
          1 => target_mat = "PETG",
          2 => target_mat = "PHA",
          3 => target_mat = "PLA+",
          _ => {}
        }
      }

      sp.apply_material_change(target_mat, &rig.nozzle_type);

      let target_brand = *rng.choose(&brands);
      sp.apply_brand_change(target_brand, &rig.nozzle_type);

      let mat_up = sp.material_type.to_uppercase();
      let target_finish = if mat_up.contains("-CF") || mat_up.contains("CARBON") {
        "Carbon Fiber"
      } else if mat_up.contains("-GF") || mat_up.contains("GLASS") {
        "Glass Fiber"
      } else if !allow_abrasive {
        *rng.choose(&non_abrasive_finishes)
      } else {
        let cand = *rng.choose(&hardened_finishes);
        // Avoid leaving soft Foaming TPU in an active AMS slot without high-shore setting
        if in_active_ams && cand == "Foaming" && sp.material_type.contains("TPU") {
          "Basic"
        } else {
          cand
        }
      };
      sp.apply_finish_change(target_finish, &rig.nozzle_type);

      // Switch brand one more time AFTER selecting material & finish to verify brand switching preserves polymer
      let final_brand = *rng.choose(&brands);
      let expected_canon_before_brand = FilamentSpool::canonical_polymer_key(&sp.material_type);
      sp.apply_brand_change(final_brand, &rig.nozzle_type);
      let after_up = sp.material_type.to_uppercase();
      if after_up.contains("-CF") || after_up.contains("-RCF") || after_up.contains("CARBON") {
        sp.apply_finish_change("Carbon Fiber", &rig.nozzle_type);
      } else if after_up.contains("-GF") || after_up.contains("-RGF") || after_up.contains("GLASS") {
        sp.apply_finish_change("Glass Fiber", &rig.nozzle_type);
      } else if !orcaslicer_webconfig::constants::constants()
        .is_finish_compatible_with_polymer(&sp.finish_type, &sp.material_type)
      {
        sp.apply_finish_change("Basic", &rig.nozzle_type);
      }
      assert_eq!(
        FilamentSpool::canonical_polymer_key(&sp.material_type),
        expected_canon_before_brand,
        "Config #{}: Brand change to '{}' mutated polymer from '{}' to '{}'",
        cfg_idx,
        final_brand,
        expected_canon_before_brand,
        sp.material_type
      );
      if !allow_abrasive && sp.required_nozzle_hrc > final_hrc {
        sp.apply_finish_change("Basic", &rig.nozzle_type);
      }

      // Exercise active-foaming Shore A hardness curves (60A..85A) on reserve-slot Foaming TPU/PEBA
      if !in_active_ams && sp.is_foaming_tpu() {
        let shore_choices = [60u32, 65, 70, 75, 85];
        let shore = *rng.choose(&shore_choices);
        sp.apply_foaming_shore_hardness(shore, &rig.nozzle_type);
      }

      if !catalog.standard_colors.is_empty() {
        let swatch = rng.choose(&catalog.standard_colors);
        sp.color_hex = swatch.hex.clone();
        sp.color_name = swatch.name.clone();
      }
      sp.is_loaded = slot_i < ams_capacity;
      spools.push(sp);
    }

    // Switch the rig nozzle to `final_nozzle` and recompute all spools
    apply_nozzle_metadata(&mut rig, final_nozzle);
    for sp in &mut spools {
      sp.recompute_for_nozzle(&rig.nozzle_type);
      sp.rebuild_synthetic_disk_json(&rig.nozzle_type);
    }

    let processes = ProcessProfile::get_default_processes();
    let active_proc_idx = rng.next_usize(processes.len());
    let active_proc = &processes[active_proc_idx];

    let base_profile = OrcaProfile::get_stock_kobra_s1();
    let mut rules = TransformationEngine::get_default_rules();
    TransformationEngine::sync_rules_for_rig(
      &mut rules,
      &rig.nozzle_type,
      &rig.nozzle_material,
      rig.ace_pro_units,
    );
    // Toggle combinations of optional Lua symptom/filament/process rules across configs
    for r in &mut rules {
      if matches!(
        r.id.as_str(),
        "symptom_bed_adhesion_petg"
          | "symptom_spaghetti_pla"
          | "symptom_stringing_diamondback"
          | "symptom_clog_ace_pro"
          | "filament_pctg_antistringing"
          | "process_scarf_seams_arachne"
      ) && rng.next_usize(3) == 0
      {
        r.enabled = true;
      }
    }
    let derived_overrides =
      resolve_effective_orca_profile_values(&rig, active_proc, &serde_json::Map::new());

    // Evaluate the real Lua transformation stack (including `flush_volumes_matrix.lua`)
    let n = spools.len();
    let slots_json: Vec<Value> = spools
      .iter()
      .map(|s| {
        serde_json::json!({
          "name": s.name,
          "material_type": s.material_type,
          "color_hex": s.color_hex,
          "opacity": s.opacity,
          "pigment_stickiness": s.pigment_stickiness,
        })
      })
      .collect();
    let eval_ctx = serde_json::json!({
      "printer_model": rig.printer_model,
      "hotend": rig.hotend,
      "nozzle_type": rig.nozzle_type,
      "nozzle_material": rig.nozzle_material,
      "nozzle_diameter": rig.nozzle_diameter,
      "hotend_max_flow": rig.max_volumetric_flow_limit,
      "melt_zone_multiplier": rig.melt_zone_multiplier,
      "flush_multiplier": 1.0,
      "layer_height": active_proc.layer_height,
      "line_width": active_proc.line_width,
      "filament_type": spools[0].material_type,
      "addons": rig.addons,
      "climate_preset": rig.climate_preset,
      "ambient_temp_c": rig.ambient_temp_c,
      "humidity_pct": rig.humidity_pct,
      "slots": slots_json,
    });
    let stack_eval = TransformationEngine::evaluate_stack(&base_profile, &rules, &eval_ctx);
    let flush_matrix = stack_eval.flush_matrix;

    // Seed half of the configs with a pre-existing stale `AnycubicSlicerNext.conf` referencing stock Anycubic PLA-CF
    // so we verify `sync_live_state_to_entries` overwrites pre-existing `AnycubicSlicerNext.conf` with `OC -` spools!
    let mut entries: Vec<ScannedFileEntry> = Vec::new();
    if cfg_idx % 2 == 0 {
      let stale_conf = br##"{
        "app": { "preset_folder": "default", "version": "2.0.0.3", "flush_multiplier": "1.000000" },
        "models": [{ "model": "Anycubic Kobra S1", "nozzle_diameter": "0.4", "vendor": "Anycubic" }],
        "presets": { "machine": "Anycubic Kobra S1 0.4 nozzle" },
        "anycubic_presets": [{
          "machine": "Anycubic Kobra S1 0.4 nozzle",
          "process": "0.20mm Standard @Anycubic Kobra S1 0.4 nozzle",
          "filament": "Anycubic PLA-CF @Anycubic Kobra S1 0.4 nozzle",
          "filament_01": "Anycubic PETG @Anycubic Kobra S1 0.4 nozzle",
          "filament_colors": "#658946,#E10600"
        }]
      }"##;
      entries.push(ScannedFileEntry {
        relative_path: "AnycubicSlicerNext.conf".to_string(),
        filename: "AnycubicSlicerNext.conf".to_string(),
        bytes: stale_conf.to_vec(),
      });
    }
    let save_files = sync_live_state_to_entries(
      &mut entries,
      std::slice::from_ref(&rig),
      &rig.id,
      &spools,
      &processes,
      &active_proc.id,
      &flush_matrix,
      1.0,
      "AnycubicSlicerNext.conf",
      &[],
      &derived_overrides,
    );

    for sf in &save_files {
      let full_path = cfg_dir.join(&sf.relative_path);
      if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent).expect("Failed to create parent dir");
      }
      fs::write(&full_path, &sf.content).expect("Failed to write save file");
    }

    // Also write `project_settings.config` (3MF config), `project_export.3mf`, and `orcaslicer_webconfig.json`
    let proj_cfg = build_project_settings_config_json(
      &rig,
      active_proc,
      &spools,
      &flush_matrix,
      &derived_overrides,
    );
    fs::write(cfg_dir.join("project_settings.config"), proj_cfg)
      .expect("Failed to write project_settings.config");
    let threemf_bytes = build_3mf_project_container(
      &rig,
      active_proc,
      &spools,
      &flush_matrix,
      &derived_overrides,
    );
    fs::write(cfg_dir.join("project_export.3mf"), &threemf_bytes)
      .expect("Failed to write project_export.3mf");

    let snapshot = WebConfigStateSnapshot {
      schema_version: WEBCONFIG_APP_VERSION.to_string(),
      config_version: cfg_idx as u32,
      orca_slicer_version: "2.2".to_string(),
      webconfig_app_version: WEBCONFIG_APP_VERSION.to_string(),
      bound_filename: format!("Audit Config #{:03}", cfg_idx),
      bound_path_hint: cfg_dir.display().to_string(),
      detected_hardware_summary: rig.format_concise_title(),
      machines: vec![rig.clone()],
      active_machine_id: rig.id.clone(),
      filaments: spools.clone(),
      active_filament_type: spools[0].material_type.clone(),
      processes: processes.clone(),
      active_process_id: active_proc.id.clone(),
      base_profile,
      rules,
      conflicts: Vec::new(),
      pending_updates: Vec::new(),
      flush_multiplier: 1.0,
      disk_flush_multiplier: 1.0,
      disk_flush_matrix: flush_matrix,
      disk_flush_slot_count: n,
      conf_relative_path: "AnycubicSlicerNext.conf".to_string(),
    };
    fs::write(
      cfg_dir.join("orcaslicer_webconfig.json"),
      serde_json::to_string_pretty(&snapshot).unwrap(),
    )
    .expect("Failed to write orcaslicer_webconfig.json");

    config_dirs.push(cfg_dir);
  }

  println!(
    "=== Phase 2: Auditing {} generated configuration directories one by one ===",
    config_dirs.len()
  );

  let mut report_lines = Vec::new();
  report_lines.push("# Random Configuration Hardware & Thermal Audit Report".to_string());
  report_lines.push(String::new());
  report_lines.push(format!(
    "- **Total Configurations Audited**: `{}`",
    config_dirs.len()
  ));
  report_lines.push(format!("- **Output Directory**: `{}`", out_root.display()));
  report_lines.push(String::new());

  let mut total_filaments_audited = 0usize;
  let mut total_processes_audited = 0usize;
  let mut total_failures = Vec::new();

  for (idx, cfg_dir) in config_dirs.iter().enumerate() {
    let cfg_name = cfg_dir
      .file_name()
      .and_then(|s| s.to_str())
      .unwrap_or("config");
    let mut cfg_failures = Vec::new();

    // 1. Audit WebConfig snapshot round-trip + sanitize()
    let snap_txt = fs::read_to_string(cfg_dir.join("orcaslicer_webconfig.json"))
      .expect("Missing orcaslicer_webconfig.json");
    let mut snap: WebConfigStateSnapshot =
      serde_json::from_str(&snap_txt).expect("Invalid orcaslicer_webconfig.json");
    snap.sanitize();
    let rig = snap.machines.first().expect("Missing machine rig in snapshot");

    // 2. Audit every filament JSON file in user/default/filament/
    let fil_dir = cfg_dir.join("user/default/filament");
    let mut fil_files: Vec<PathBuf> = fs::read_dir(&fil_dir)
      .expect("Missing filament dir")
      .filter_map(|e| e.ok().map(|x| x.path()))
      .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
      .collect();
    fil_files.sort();

    let mut slot_summaries = Vec::new();
    let mut standard_ams_temps_and_ranges: Vec<(String, i32, i32, i32)> = Vec::new();

    for ff in &fil_files {
      total_filaments_audited += 1;
      let txt = fs::read_to_string(ff).expect("Failed to read filament json");
      let val: Value = serde_json::from_str(&txt).expect("Invalid filament json");
      let name = first_str_val(&val, "name").unwrap_or_default();
      let ftype = first_str_val(&val, "filament_type").unwrap_or_default();
      let notes = first_str_val(&val, "filament_notes").unwrap_or_default();
      let noz_t: i32 = first_str_val(&val, "nozzle_temperature")
        .and_then(|s| s.parse().ok())
        .unwrap_or(-1);
      let init_t: i32 = first_str_val(&val, "nozzle_temperature_initial_layer")
        .and_then(|s| s.parse().ok())
        .unwrap_or(-1);
      let range_low: i32 = first_str_val(&val, "nozzle_temperature_range_low")
        .and_then(|s| s.parse().ok())
        .unwrap_or(-1);
      let range_high: i32 = first_str_val(&val, "nozzle_temperature_range_high")
        .and_then(|s| s.parse().ok())
        .unwrap_or(-1);
      let bed_t: i32 = first_str_val(&val, "textured_plate_temp")
        .and_then(|s| s.parse().ok())
        .unwrap_or(-1);
      let flow: f32 = first_str_val(&val, "filament_flow_ratio")
        .and_then(|s| s.parse().ok())
        .unwrap_or(-1.0);
      let mvs: f32 = first_str_val(&val, "filament_max_volumetric_speed")
        .and_then(|s| s.parse().ok())
        .unwrap_or(-1.0);
      let ret_spd: f32 = first_str_val(&val, "filament_retraction_speed")
        .and_then(|s| s.parse().ok())
        .unwrap_or(-1.0);
      if val.get("compatible_printers")
        != Some(&serde_json::json!(["Anycubic Kobra S1 0.4 nozzle"]))
      {
        cfg_failures.push(format!(
          "[{}] Filament '{}' must have compatible_printers = ['Anycubic Kobra S1 0.4 nozzle'], got {:?}",
          cfg_name,
          name,
          val.get("compatible_printers")
        ));
      }
      if val.get("filament_vendor").is_some() {
        cfg_failures.push(format!(
          "[{}] Filament '{}' must NOT emit 'filament_vendor' (causes AnycubicSlicerNext 3MF vs user preset mismatch)!",
          cfg_name, name
        ));
      }

      let matching_spool = snap.filaments.iter().find(|s| s.name == name);
      let mat_for_bounds = matching_spool
        .map(|s| s.material_type.as_str())
        .unwrap_or(&ftype);
      let fin_for_bounds = matching_spool
        .map(|s| s.finish_type.as_str())
        .unwrap_or("Basic");
      if !orcaslicer_webconfig::constants::constants()
        .is_finish_compatible_with_polymer(fin_for_bounds, mat_for_bounds)
      {
        cfg_failures.push(format!(
          "[{}] Filament '{}' has physically incompatible finish '{}' for polymer '{}'",
          cfg_name, name, fin_for_bounds, mat_for_bounds
        ));
      }

      let (min_noz, max_noz, min_bed, max_bed) =
        safe_temp_bounds_for_polymer(mat_for_bounds, fin_for_bounds);

      let init_bed: i32 = first_str_val(&val, "hot_plate_temp_initial_layer")
        .and_then(|s| s.parse().ok())
        .unwrap_or(-1);
      let fan_min: i32 = first_str_val(&val, "fan_min_speed")
        .and_then(|s| s.parse().ok())
        .unwrap_or(-1);
      let fan_max: i32 = first_str_val(&val, "fan_max_speed")
        .and_then(|s| s.parse().ok())
        .unwrap_or(-1);
      if !(min_noz..=max_noz).contains(&noz_t) || noz_t > 295 || init_t > 295 {
        cfg_failures.push(format!(
          "[{}] Filament '{}' ({} / {}) has unsafe nozzle_temperature={}°C / init={}°C (expected {}..={}°C and <=295°C Kobra S1 ceiling; notes={})",
          cfg_name, name, mat_for_bounds, fin_for_bounds, noz_t, init_t, min_noz, max_noz, notes
        ));
      }
      if init_t < noz_t || init_t > max_noz + 10 {
        cfg_failures.push(format!(
          "[{}] Filament '{}' has invalid initial layer nozzle temp={}°C vs continuous={}°C",
          cfg_name, name, init_t, noz_t
        ));
      }
      if range_low < 175
        || range_high > 300
        || noz_t < range_low
        || init_t > range_high
      {
        cfg_failures.push(format!(
          "[{}] Filament '{}' nozzle temp ({}°C / init {}°C) or range [{}..={}°C] violates Kobra S1 [175..=300°C] limits",
          cfg_name, name, noz_t, init_t, range_low, range_high
        ));
      }
      let exp_noz_note = format!("Nozzle: {}°C", noz_t);
      let exp_bed_note = format!("Bed: {}°C", bed_t);
      if !notes.contains(&exp_noz_note) || !notes.contains(&exp_bed_note) {
        cfg_failures.push(format!(
          "[{}] Filament '{}' filament_notes ('{}') does not match exported '{}' / '{}'",
          cfg_name, name, notes, exp_noz_note, exp_bed_note
        ));
      }
      if !(min_bed..=max_bed).contains(&bed_t) || bed_t > 110 || init_bed > 110 {
        cfg_failures.push(format!(
          "[{}] Filament '{}' ({}) has out-of-bounds bed_temp={}°C / init_bed={}°C (expected {}..={}°C and <=110°C Kobra S1 bed ceiling)",
          cfg_name, name, mat_for_bounds, bed_t, init_bed, min_bed, max_bed
        ));
      }
      let is_hot_climate = rig.ambient_temp_c >= 26
        || rig.climate_preset == "tropical"
        || rig.climate_preset == "hot_dry";
      let expected_min_bed = if is_hot_climate {
        min_bed.max((rig.ambient_temp_c + 15).clamp(45, 50))
      } else {
        min_bed
      };
      for (plate_key, exp_plate_t) in [
        ("hot_plate_temp", bed_t),
        ("hot_plate_temp_initial_layer", init_bed),
        ("textured_plate_temp", bed_t),
        ("textured_plate_temp_initial_layer", init_bed),
        ("cool_plate_temp", bed_t),
        ("cool_plate_temp_initial_layer", init_bed),
        ("eng_plate_temp", bed_t),
        ("eng_plate_temp_initial_layer", init_bed),
        ("textured_cool_plate_temp", bed_t),
        ("textured_cool_plate_temp_initial_layer", init_bed),
        ("supertack_plate_temp", bed_t),
        ("supertack_plate_temp_initial_layer", init_bed),
      ] {
        let p_val = first_str_val(&val, plate_key)
          .and_then(|s| s.parse::<i32>().ok())
          .unwrap_or(0);
        if p_val != exp_plate_t || p_val < expected_min_bed {
          cfg_failures.push(format!(
            "[{}] Filament '{}' key '{}' is {}°C (expected {}°C >= {}°C across all plates for climate={})",
            cfg_name, name, plate_key, p_val, exp_plate_t, expected_min_bed, rig.climate_preset
          ));
        }
      }
      if fan_min < 0 || fan_max < 0 || fan_min > fan_max || fan_max > 100 {
        cfg_failures.push(format!(
          "[{}] Filament '{}' has invalid or inverted cooling fan speeds: fan_min_speed={}% vs fan_max_speed={}%",
          cfg_name, name, fan_min, fan_max
        ));
      }
      let is_foaming = fin_for_bounds.eq_ignore_ascii_case("Foaming");
      let max_allowed_flow = if is_foaming { 1.02 } else { 1.05 };
      let min_allowed_flow = if is_foaming { 0.45 } else { 0.90 };
      if !(min_allowed_flow..=max_allowed_flow).contains(&flow) {
        cfg_failures.push(format!(
          "[{}] Filament '{}' has invalid flow_ratio={} (expected {}..={})",
          cfg_name, name, flow, min_allowed_flow, max_allowed_flow
        ));
      }
      if !(1.5..=22.0).contains(&mvs) {
        cfg_failures.push(format!(
          "[{}] Filament '{}' has invalid max_volumetric_speed={} (expected 1.5..=22.0 mm³/s)",
          cfg_name, name, mvs
        ));
      }
      let is_tpu_family = ftype == "TPU" || ftype == "PEBA" || mat_for_bounds.contains("TPU") || mat_for_bounds.contains("PEBA");
      let max_ret_spd = if is_tpu_family { 35.0 } else { 55.0 };
      if !(15.0..=max_ret_spd).contains(&ret_spd) {
        cfg_failures.push(format!(
          "[{}] Filament '{}' ({}) has invalid retraction_speed={} (expected 15.0..={} mm/s)",
          cfg_name, name, ftype, ret_spd, max_ret_spd
        ));
      }
      if name.to_uppercase().contains("-CF-CF")
        || name.to_uppercase().contains("-GF-GF")
        || name.to_uppercase().contains("RCF08-CF")
        || name.to_uppercase().contains("CF10-CF")
        || name.to_uppercase().contains("CF20-CF")
        || name.to_uppercase().contains("CF17-CF")
      {
        cfg_failures.push(format!(
          "[{}] Filament '{}' has redundant fiber suffix in preset title",
          cfg_name, name
        ));
      }

      let canon = FilamentSpool::canonical_polymer_key(mat_for_bounds);
      if name.contains("PETG-CF") || name.contains("PETG-GF") || name.contains("PETG-rCF") {
        if noz_t < 235 || ftype != "PETG-CF" {
          cfg_failures.push(format!(
            "[{}] Filament '{}' must have nozzle_temperature >= 235°C and filament_type='PETG-CF' (got {}°C, type='{}')",
            cfg_name, name, noz_t, ftype
          ));
        }
      }
      if matches!(
        canon,
        "PLA" | "PLA+" | "PLA-CF" | "PHA" | "PVA" | "TPU" | "TPU for AMS" | "TPU 95A" | "PETG"
          | "PETG-CF" | "PCTG"
      ) && noz_t <= 265
      {
        standard_ams_temps_and_ranges.push((name.clone(), noz_t, range_low, range_high));
      }

      slot_summaries.push(format!(
        "`{}` (`{}`: `{}°C` nozzle `[{}..{}°C]`, `{}°C` bed, `{:.1} mm³/s`)",
        name, mat_for_bounds, noz_t, range_low, range_high, bed_t, mvs
      ));
    }

    // Check cross-filament multi-material range compatibility across standard AMS polymers in this config
    for (name_a, temp_a, _, _) in &standard_ams_temps_and_ranges {
      for (name_b, _, low_b, high_b) in &standard_ams_temps_and_ranges {
        if *temp_a < *low_b || *temp_a > *high_b {
          cfg_failures.push(format!(
            "[{}] Multi-material range incompatibility: '{}' ({}°C) is outside '{}' range [{}..={}°C]",
            cfg_name, name_a, temp_a, name_b, low_b, high_b
          ));
        }
      }
    }

    // 3. Audit all 5 process JSON files in user/default/process/
    let expected_procs = [
      ("OC - 0.12mm Quality @Kobra S1.json", "0.12", "60", "150", "200", "50", "5000"),
      ("OC - 0.20mm Quality @Kobra S1.json", "0.20", "60", "150", "250", "50", "5000"),
      ("OC - 0.20mm Speed @Kobra S1.json", "0.20", "300", "350", "350", "60", "15000"),
      ("OC - 0.20mm Standard @Kobra S1.json", "0.20", "200", "300", "270", "50", "10000"),
      ("OC - 0.28mm Speed @Kobra S1.json", "0.28", "250", "320", "320", "60", "12000"),
    ];
    for (pfile, exp_lh, exp_ow, exp_iw, exp_si, exp_il, exp_acc) in expected_procs {
      total_processes_audited += 1;
      let ppath = cfg_dir.join("user/default/process").join(pfile);
      let ptxt = fs::read_to_string(&ppath).expect("Missing process JSON file");
      let pval: Value = serde_json::from_str(&ptxt).expect("Invalid process JSON");
      if first_str_val(&pval, "layer_height").as_deref() != Some(exp_lh)
        || first_str_val(&pval, "outer_wall_speed").as_deref() != Some(exp_ow)
        || first_str_val(&pval, "inner_wall_speed").as_deref() != Some(exp_iw)
        || first_str_val(&pval, "sparse_infill_speed").as_deref() != Some(exp_si)
        || first_str_val(&pval, "initial_layer_speed").as_deref() != Some(exp_il)
        || first_str_val(&pval, "default_acceleration").as_deref() != Some(exp_acc)
      {
        cfg_failures.push(format!(
          "[{}] Process '{}' speeds/heights corrupted: got lh={:?} ow={:?} iw={:?} si={:?} il={:?} acc={:?}",
          cfg_name,
          pfile,
          first_str_val(&pval, "layer_height"),
          first_str_val(&pval, "outer_wall_speed"),
          first_str_val(&pval, "inner_wall_speed"),
          first_str_val(&pval, "sparse_infill_speed"),
          first_str_val(&pval, "initial_layer_speed"),
          first_str_val(&pval, "default_acceleration"),
        ));
      }
      if pval.get("internal_solid_infill_speed").is_some() {
        cfg_failures.push(format!(
          "[{}] Process '{}' leaked internal_solid_infill_speed",
          cfg_name, pfile
        ));
      }
    }

    // 4. Audit `project_settings.config` (3MF config) vs standalone filament JSONs and `AnycubicSlicerNext.conf`
    let proj_txt = fs::read_to_string(cfg_dir.join("project_settings.config"))
      .expect("Missing project_settings.config");
    let proj_val: Value =
      serde_json::from_str(&proj_txt).expect("Invalid project_settings.config JSON");
    let conf_txt = fs::read_to_string(cfg_dir.join("AnycubicSlicerNext.conf"))
      .expect("Missing AnycubicSlicerNext.conf");
    let conf_val: Value =
      serde_json::from_str(&conf_txt).expect("Invalid AnycubicSlicerNext.conf JSON");

    if let Some(proj_ids) = proj_val
      .get("filament_settings_id")
      .and_then(|v| v.as_array())
    {
      if proj_ids.len() != snap.filaments.len() {
        cfg_failures.push(format!(
          "[{}] project_settings.config has {} slots, expected {}",
          cfg_name,
          proj_ids.len(),
          snap.filaments.len()
        ));
      }
      let diff_arr = proj_val
        .get("different_settings_to_system")
        .and_then(|v| v.as_array());
      for (s_idx, id_val) in proj_ids.iter().enumerate() {
        let slot_name = id_val.as_str().unwrap_or_default();
        let disk_path = fil_dir.join(format!("{}.json", slot_name));
        if !disk_path.exists() {
          cfg_failures.push(format!(
            "[{}] project_settings.config slot #{} '{}' has no matching file in user/default/filament/",
            cfg_name,
            s_idx + 1,
            slot_name
          ));
        } else {
          let d_txt = fs::read_to_string(&disk_path).unwrap();
          let d_val: Value = serde_json::from_str(&d_txt).unwrap();
          let slot_diff_set: std::collections::HashSet<&str> = diff_arr
            .and_then(|a| a.get(s_idx + 1))
            .and_then(|v| v.as_str())
            .map(|s| s.split(';').filter(|k| !k.is_empty()).collect())
            .unwrap_or_default();
          if let Some(d_obj) = d_val.as_object() {
            for (k, _) in d_obj {
              if matches!(
                k.as_str(),
                "type"
                  | "from"
                  | "is_custom_defined"
                  | "inherits"
                  | "name"
                  | "filament_settings_id"
                  | "compatible_printers"
                  | "version"
              ) {
                continue;
              }
              if !slot_diff_set.contains(k.as_str()) {
                cfg_failures.push(format!(
                  "[{}] Slot #{} '{}' disk key '{}' is MISSING from different_settings_to_system (will cause update_non_diff_values_to_base_config to reset it and mark preset as is_project_embedded)!",
                  cfg_name,
                  s_idx + 1,
                  slot_name,
                  k
                ));
              }
              let proj_cell = proj_val
                .get(k)
                .and_then(|v| v.as_array())
                .and_then(|a| a.get(s_idx))
                .and_then(|x| x.as_str());
              let disk_cell = first_str_val(&d_val, k);
              if proj_cell != disk_cell.as_deref() {
                cfg_failures.push(format!(
                  "[{}] 3MF vs Disk JSON mismatch on slot #{} '{}' key '{}': 3MF={:?} vs Disk={:?}",
                  cfg_name,
                  s_idx + 1,
                  slot_name,
                  k,
                  proj_cell,
                  disk_cell
                ));
              }
            }
          }
          for &diff_k in &slot_diff_set {
            let proj_cell = proj_val
              .get(diff_k)
              .and_then(|v| v.as_array())
              .and_then(|a| a.get(s_idx))
              .and_then(|x| x.as_str());
            let disk_cell = first_str_val(&d_val, diff_k);
            if proj_cell != disk_cell.as_deref() {
              cfg_failures.push(format!(
                "[{}] different_settings_to_system key '{}' mismatch on slot #{} '{}': 3MF={:?} vs Disk={:?}",
                cfg_name,
                diff_k,
                s_idx + 1,
                slot_name,
                proj_cell,
                disk_cell
              ));
            }
          }
        }
      }

      // Also verify Process (index 0) and Machine (index N+1) 3MF vs Disk JSON + different_settings_to_system
      let proc_id = proj_val
        .get("print_settings_id")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
      let proc_disk_path = cfg_dir
        .join("user/default/process")
        .join(format!("{}.json", proc_id));
      if !proc_disk_path.exists() {
        cfg_failures.push(format!(
          "[{}] project_settings.config print_settings_id '{}' has no matching file in user/default/process/",
          cfg_name, proc_id
        ));
      } else {
        let p_txt = fs::read_to_string(&proc_disk_path).unwrap();
        let p_val: Value = serde_json::from_str(&p_txt).unwrap();
        let proc_diff_set: std::collections::HashSet<&str> = diff_arr
          .and_then(|a| a.first())
          .and_then(|v| v.as_str())
          .map(|s| s.split(';').filter(|k| !k.is_empty()).collect())
          .unwrap_or_default();
        if let Some(p_obj) = p_val.as_object() {
          for (k, v) in p_obj {
            if matches!(
              k.as_str(),
              "type"
                | "from"
                | "is_custom_defined"
                | "inherits"
                | "name"
                | "print_settings_id"
                | "compatible_printers"
                | "version"
            ) {
              continue;
            }
            if !proc_diff_set.contains(k.as_str()) {
              cfg_failures.push(format!(
                "[{}] Process '{}' disk key '{}' is MISSING from different_settings_to_system[0]!",
                cfg_name, proc_id, k
              ));
            }
            if proj_val.get(k) != Some(v) {
              cfg_failures.push(format!(
                "[{}] Process '{}' key '{}' mismatch: 3MF={:?} vs Disk={:?}",
                cfg_name,
                proc_id,
                k,
                proj_val.get(k),
                v
              ));
            }
          }
        }
      }

      let mach_id = proj_val
        .get("printer_settings_id")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
      let mach_disk_path = cfg_dir
        .join("user/default/machine")
        .join(format!("{}.json", mach_id));
      if !mach_disk_path.exists() {
        cfg_failures.push(format!(
          "[{}] project_settings.config printer_settings_id '{}' has no matching file in user/default/machine/",
          cfg_name, mach_id
        ));
      } else {
        let m_txt = fs::read_to_string(&mach_disk_path).unwrap();
        let m_val: Value = serde_json::from_str(&m_txt).unwrap();
        let mach_diff_set: std::collections::HashSet<&str> = diff_arr
          .and_then(|a| a.last())
          .and_then(|v| v.as_str())
          .map(|s| s.split(';').filter(|k| !k.is_empty()).collect())
          .unwrap_or_default();
        if let Some(m_obj) = m_val.as_object() {
          for (k, v) in m_obj {
            if matches!(
              k.as_str(),
              "type"
                | "from"
                | "is_custom_defined"
                | "inherits"
                | "name"
                | "printer_settings_id"
                | "printer_model"
                | "printer_variant"
                | "compatible_printers"
                | "version"
            ) {
              continue;
            }
            if !mach_diff_set.contains(k.as_str()) {
              cfg_failures.push(format!(
                "[{}] Machine '{}' disk key '{}' is MISSING from different_settings_to_system[last]!",
                cfg_name, mach_id, k
              ));
            }
            if proj_val.get(k) != Some(v) {
              cfg_failures.push(format!(
                "[{}] Machine '{}' key '{}' mismatch: 3MF={:?} vs Disk={:?}",
                cfg_name,
                mach_id,
                k,
                proj_val.get(k),
                v
              ));
            }
          }
        }
      }
    }

    if let Some(ac_presets) = conf_val
      .get("anycubic_presets")
      .and_then(|v| v.as_array())
      .and_then(|a| a.first())
    {
      for s_idx in 0..snap.filaments.len() {
        let key = if s_idx == 0 {
          "filament".to_string()
        } else {
          format!("filament_{:02}", s_idx)
        };
        let conf_slot = ac_presets.get(&key).and_then(|v| v.as_str()).unwrap_or("");
        if conf_slot != snap.filaments[s_idx].name {
          cfg_failures.push(format!(
            "[{}] AnycubicSlicerNext.conf '{}' is '{}', expected '{}'",
            cfg_name, key, conf_slot, snap.filaments[s_idx].name
          ));
        }
      }
      let n_slots = snap.filaments.len();
      let fmat_str = ac_presets
        .get("flush_volumes_matrix")
        .and_then(|v| v.as_str())
        .unwrap_or("");
      let fmat_cells: Vec<f64> = fmat_str
        .split('|')
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<f64>().ok())
        .collect();
      if fmat_cells.len() != n_slots * n_slots {
        cfg_failures.push(format!(
          "[{}] AnycubicSlicerNext.conf flush_volumes_matrix has {} cells, expected {} ({}x{})",
          cfg_name,
          fmat_cells.len(),
          n_slots * n_slots,
          n_slots,
          n_slots
        ));
      } else {
        for r in 0..n_slots {
          for c in 0..n_slots {
            let v = fmat_cells[r * n_slots + c];
            if r == c && v.abs() > 1e-6 {
              cfg_failures.push(format!(
                "[{}] flush_volumes_matrix diagonal [{},{}] is {}, expected 0",
                cfg_name, r, c, v
              ));
            } else if r != c && !(70.0..=900.0).contains(&v) {
              cfg_failures.push(format!(
                "[{}] flush_volumes_matrix off-diagonal [{},{}] is {}, expected 70..=900",
                cfg_name, r, c, v
              ));
            }
          }
        }
      }
    }

    // 5. Re-import the directory via `import_scanned_directory` and run `evaluate_safety_warnings`
    let mut scanned = Vec::new();
    collect_scanned_entries(cfg_dir, cfg_dir, &mut scanned);
    let imported = import_scanned_directory(&scanned).expect("import_scanned_directory failed");
    if imported.filaments.len() != snap.filaments.len() {
      cfg_failures.push(format!(
        "[{}] import_scanned_directory returned {} filaments, expected {}",
        cfg_name,
        imported.filaments.len(),
        snap.filaments.len()
      ));
    } else {
      for (s_idx, (orig_sp, imp_sp)) in snap
        .filaments
        .iter()
        .zip(imported.filaments.iter())
        .enumerate()
      {
        if imp_sp.name != orig_sp.name
          || imp_sp.brand != orig_sp.brand
          || imp_sp.material_type != orig_sp.material_type
          || imp_sp.finish_type != orig_sp.finish_type
        {
          cfg_failures.push(format!(
            "[{}] Directory round-trip slot #{} mismatch: expected ('{}' / brand='{}' / mat='{}' / finish='{}'), got ('{}' / brand='{}' / mat='{}' / finish='{}')",
            cfg_name,
            s_idx + 1,
            orig_sp.name,
            orig_sp.brand,
            orig_sp.material_type,
            orig_sp.finish_type,
            imp_sp.name,
            imp_sp.brand,
            imp_sp.material_type,
            imp_sp.finish_type
          ));
        }
      }
    }

    // 6. Re-import `project_export.3mf` via `parse_ace_loadout_from_bytes_or_json` and verify round-trip integrity
    let threemf_raw =
      fs::read(cfg_dir.join("project_export.3mf")).expect("Missing project_export.3mf");
    match parse_ace_loadout_from_bytes_or_json(&threemf_raw) {
      Err(err) => {
        cfg_failures.push(format!(
          "[{}] parse_ace_loadout_from_bytes_or_json failed on project_export.3mf: {}",
          cfg_name, err
        ));
      }
      Ok(tmf) => {
        if tmf.spools.len() != snap.filaments.len() {
          cfg_failures.push(format!(
            "[{}] 3MF re-import returned {} spools, expected {}",
            cfg_name,
            tmf.spools.len(),
            snap.filaments.len()
          ));
        } else {
          for (s_idx, (orig_sp, tmf_sp)) in
            snap.filaments.iter().zip(tmf.spools.iter()).enumerate()
          {
            if tmf_sp.brand == "OC"
              || tmf_sp.name != orig_sp.name
              || tmf_sp.brand != orig_sp.brand
              || FilamentSpool::canonical_polymer_key(&tmf_sp.material_type)
                != FilamentSpool::canonical_polymer_key(&orig_sp.material_type)
              || tmf_sp.required_nozzle_hrc != orig_sp.required_nozzle_hrc
              || !tmf_sp.inherits.contains("@Anycubic Kobra S1 0.4 nozzle")
            {
              cfg_failures.push(format!(
                "[{}] 3MF round-trip slot #{} mismatch: expected (name='{}', brand='{}', mat='{}', hrc={}), got (name='{}', brand='{}', mat='{}', hrc={}, inherits='{}')",
                cfg_name,
                s_idx + 1,
                orig_sp.name,
                orig_sp.brand,
                orig_sp.material_type,
                orig_sp.required_nozzle_hrc,
                tmf_sp.name,
                tmf_sp.brand,
                tmf_sp.material_type,
                tmf_sp.required_nozzle_hrc,
                tmf_sp.inherits
              ));
            }
          }
        }
      }
    }

    let warnings = TransformationEngine::evaluate_safety_warnings(
      rig,
      &imported.filaments,
      &imported.base_profile,
      &imported.conflicts,
      &imported.disk_flush_matrix,
    );
    let danger_warnings: Vec<_> = warnings
      .iter()
      .filter(|w| w.severity == SafetySeverity::Danger)
      .collect();
    if !danger_warnings.is_empty() || !imported.conflicts.is_empty() {
      cfg_failures.push(format!(
        "[{}] Re-imported directory triggered {} Danger warnings ({:?}) and {} conflicts ({:?})",
        cfg_name,
        danger_warnings.len(),
        danger_warnings,
        imported.conflicts.len(),
        imported.conflicts
      ));
    }

    // Also evaluate safety warnings directly on the sanitized snapshot state
    let snap_warnings = TransformationEngine::evaluate_safety_warnings(
      rig,
      &snap.filaments,
      &snap.base_profile,
      &snap.conflicts,
      &snap.disk_flush_matrix,
    );
    if !snap_warnings.is_empty() {
      cfg_failures.push(format!(
        "[{}] Snapshot state triggered unexpected safety warnings: {:?}",
        cfg_name, snap_warnings
      ));
    }

    let status_badge = if cfg_failures.is_empty() {
      "PASS"
    } else {
      "FAIL"
    };
    println!(
      "  [{:02}/{}] {} | Rig: {} ({}, climate={}) | {} filaments | {}",
      idx + 1,
      config_dirs.len(),
      cfg_name,
      rig.name,
      rig.nozzle_type,
      rig.climate_preset,
      fil_files.len(),
      status_badge
    );

    report_lines.push(format!(
      "## `{}` — {} (`{}` • `{}` • Climate: `{}`)",
      cfg_name, status_badge, rig.name, rig.nozzle_type, rig.climate_preset
    ));
    for s_line in slot_summaries {
      report_lines.push(format!("- {}", s_line));
    }
    if !cfg_failures.is_empty() {
      for fail in &cfg_failures {
        report_lines.push(format!("- **FAILURE**: {}", fail));
      }
      total_failures.extend(cfg_failures);
    }
    report_lines.push(String::new());
  }

  let report_path = out_root.join("AUDIT_REPORT.md");
  fs::write(&report_path, report_lines.join("\n")).expect("Failed to write AUDIT_REPORT.md");

  println!("==================================================================");
  println!(
    "Audit Complete: {} configs, {} filaments, {} processes audited.",
    config_dirs.len(),
    total_filaments_audited,
    total_processes_audited
  );
  println!("Detailed report written to: {}", report_path.display());

  if !total_failures.is_empty() {
    eprintln!("AUDIT FAILED with {} issue(s):", total_failures.len());
    for f in &total_failures {
      eprintln!("  - {}", f);
    }
    std::process::exit(1);
  }
  println!("ALL {} RANDOM CONFIGURATIONS PASSED 100% OF HARDWARE, TEMPERATURE & SPEED CHECKS!", config_dirs.len());
}

fn main() {
  let args: Vec<String> = std::env::args().collect();
  let mut out_root = std::env::temp_dir().join("orcaslicer_webconfig_random_audit");
  let mut num_configs: usize = 30;
  let mut seed: u64 = 0x51C3_2026_CAFE_BEEF;

  let mut i = 1;
  while i < args.len() {
    match args[i].as_str() {
      "--dir" | "-d" => {
        if i + 1 < args.len() {
          out_root = PathBuf::from(&args[i + 1]);
          i += 1;
        }
      }
      "--count" | "-n" => {
        if i + 1 < args.len() {
          if let Ok(n) = args[i + 1].parse::<usize>() {
            num_configs = n;
          }
          i += 1;
        }
      }
      "--seed" | "-s" => {
        if i + 1 < args.len() {
          if let Ok(s) = args[i + 1].parse::<u64>() {
            seed = s;
          }
          i += 1;
        }
      }
      "--repair-from-snapshot" => {
        if i + 4 < args.len() {
          let snap_path = PathBuf::from(&args[i + 1]);
          let slicer_dir = PathBuf::from(&args[i + 2]);
          let out_3mf = PathBuf::from(&args[i + 3]);
          let out_zip = PathBuf::from(&args[i + 4]);
          repair_live_slicer_dir(&snap_path, &slicer_dir, &out_3mf, &out_zip);
          return;
        }
      }
      other if !other.starts_with('-') => {
        out_root = PathBuf::from(other);
      }
      _ => {}
    }
    i += 1;
  }

  run_audit_suite(&out_root, num_configs, seed);
}

fn repair_live_slicer_dir(
  snap_path: &Path,
  slicer_dir: &Path,
  out_3mf: &Path,
  out_zip: &Path,
) {
  let snap_txt = fs::read_to_string(snap_path).expect("Failed to read snapshot json");
  let mut snap: WebConfigStateSnapshot =
    serde_json::from_str(&snap_txt).expect("Failed to parse snapshot json");
  snap.sanitize();

  let mut entries = Vec::new();
  collect_scanned_entries(slicer_dir, slicer_dir, &mut entries);
  let active_rig = snap
    .machines
    .iter()
    .find(|m| m.id == snap.active_machine_id)
    .or_else(|| snap.machines.first())
    .expect("No machine in snapshot");
  let active_proc = snap
    .processes
    .iter()
    .find(|p| p.id == snap.active_process_id)
    .or_else(|| snap.processes.first())
    .expect("No process in snapshot");
  let derived_overrides =
    resolve_effective_orca_profile_values(active_rig, active_proc, &serde_json::Map::new());

  let save_files = sync_live_state_to_entries(
    &mut entries,
    &snap.machines,
    &snap.active_machine_id,
    &snap.filaments,
    &snap.processes,
    &snap.active_process_id,
    &snap.disk_flush_matrix,
    snap.flush_multiplier,
    &snap.conf_relative_path,
    &snap.pending_updates,
    &derived_overrides,
  );
  for sf in &save_files {
    let dest = slicer_dir.join(&sf.relative_path);
    if let Some(parent) = dest.parent() {
      fs::create_dir_all(parent).expect("Failed to create parent dir");
    }
    fs::write(&dest, &sf.content).expect("Failed to write repaired file");
    println!("Updated live file: {}", dest.display());
  }

  let threemf_bytes = build_3mf_project_container(
    active_rig,
    active_proc,
    &snap.filaments,
    &snap.disk_flush_matrix,
    &derived_overrides,
  );
  fs::write(out_3mf, &threemf_bytes).expect("Failed to write out_3mf");
  println!(
    "Wrote repaired 3MF ({} bytes): {}",
    threemf_bytes.len(),
    out_3mf.display()
  );

  let zip_bytes = orcaslicer_webconfig::dir_import::build_directory_backup_zip(&entries);
  fs::write(out_zip, &zip_bytes).expect("Failed to write out_zip");
  println!(
    "Wrote repaired backup ZIP ({} bytes): {}",
    zip_bytes.len(),
    out_zip.display()
  );
}

fn collect_scanned_entries(root: &Path, dir: &Path, out: &mut Vec<ScannedFileEntry>) {
  if let Ok(entries) = fs::read_dir(dir) {
    let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|x| x.path())).collect();
    paths.sort();
    for p in paths {
      if p.is_dir() {
        collect_scanned_entries(root, &p, out);
      } else if let Ok(rel) = p.strip_prefix(root) {
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if rel_str == "orcaslicer_webconfig.json"
          || rel_str == "project_settings.config"
          || rel_str == "project_export.3mf"
          || rel_str == "AUDIT_REPORT.md"
        {
          continue;
        }
        let fname = p
          .file_name()
          .and_then(|s| s.to_str())
          .unwrap_or_default()
          .to_string();
        if let Ok(bytes) = fs::read(&p) {
          out.push(ScannedFileEntry {
            relative_path: rel_str,
            filename: fname,
            bytes,
          });
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_random_configs_audit_suite() {
    let out_root = std::env::temp_dir().join("orcaslicer_webconfig_bazel_test_audit");
    run_audit_suite(&out_root, 20, 0x51C3_2026_CAFE_BEEF);
  }
}
