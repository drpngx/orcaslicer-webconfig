use orcaslicer_webconfig::catalog::{load_merged_catalog, FILAMENT_BRAND_OPTIONS, FINISH_OPTIONS};
use orcaslicer_webconfig::dir_import::{import_scanned_directory, sync_live_state_to_entries};
use orcaslicer_webconfig::fs_access::ScannedFileEntry;
use orcaslicer_webconfig::models::{
  FilamentSpool, MachineRig, ProcessProfile, WebConfigStateSnapshot, WEBCONFIG_APP_VERSION,
};
use orcaslicer_webconfig::orca_profile::OrcaProfile;
use orcaslicer_webconfig::threemf::{
  build_project_settings_config_json, resolve_effective_orca_profile_values,
};
use orcaslicer_webconfig::transformations::{SafetySeverity, TransformationEngine};
use serde_json::Value;
use std::env;
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
  if canon == "PCL" {
    (70, 175, 25, 65)
  } else if finish.eq_ignore_ascii_case("Foaming")
    && (canon.starts_with("TPU") || canon == "PEBA" || up.contains("AIR"))
  {
    (185, 285, 30, 85)
  } else if up.contains("PETG-ESD") || up.contains("PA612-ESD") {
    (240, 305, 60, 110)
  } else {
    match canon {
      "PHA" => (190, 235, 35, 70),
      "PLA" | "PLA+" | "PLA-CF" => (185, 235, 35, 70),
      "PVA" | "BVOH" => (180, 230, 35, 70),
      "PETG" | "PETG-CF" => (210, 260, 55, 85),
      "PCTG" | "PCTG-CF" => (235, 275, 65, 95),
      "TPU" | "TPU for AMS" | "TPU 95A" | "TPU 90A" | "TPU 85A" | "TPU-GF" | "PEBA" => {
        (190, 255, 30, 70)
      }
      "ABS" | "ABS-CF" | "ASA" | "ASA-CF" | "HIPS" => (230, 290, 80, 115),
      "PEEK" => (320, 420, 100, 150),
      "PPS" => (290, 360, 85, 130),
      "PC" | "PC-CF" | "PET-CF" | "PA" | "PA6-CF" | "PA12-CF" | "PAHT-CF" | "PPA-CF" => {
        (240, 320, 70, 120)
      }
      _ => (180, 300, 30, 115),
    }
  }
}

fn apply_nozzle_metadata(rig: &mut MachineRig, nozzle_type: &str) {
  rig.nozzle_type = nozzle_type.to_string();
  match nozzle_type {
    "diamondback" => {
      rig.nozzle_material = "MicroSwiss Diamondback (PCD)".to_string();
      rig.nozzle_name = "MicroSwiss Diamondback PCD 0.40mm".to_string();
    }
    "ruby_brass" => {
      rig.nozzle_material = "Ruby Tip (Brass Body)".to_string();
      rig.nozzle_name = "Brass-Body Ruby Tip 0.40mm".to_string();
    }
    "orca_brass" => {
      rig.nozzle_material = "Brass".to_string();
      rig.nozzle_name = "Standard Brass 0.40mm".to_string();
    }
    "cm2" => {
      rig.nozzle_material = "MicroSwiss CM2 (Copper + M2 HSS)".to_string();
      rig.nozzle_name = "MicroSwiss CM2 Hardened Tip 0.40mm".to_string();
    }
    _ => {
      rig.nozzle_material = "Hardened Steel".to_string();
      rig.nozzle_name = "Hardened Steel Wear-Resistant 0.40mm".to_string();
    }
  }
  let (_, hrc) = rig.orca_machine_nozzle_type_and_hrc();
  rig.nozzle_hrc = hrc;
  rig.refresh_title_and_notes(true);
}

fn main() {
  let args: Vec<String> = env::args().collect();
  let out_root = args
    .get(1)
    .map(PathBuf::from)
    .unwrap_or_else(|| PathBuf::from("/tmp/orcaslicer_random_config_audit"));
  let num_configs: usize = args
    .get(2)
    .and_then(|s| s.parse().ok())
    .unwrap_or(30);
  let seed: u64 = args
    .get(3)
    .and_then(|s| s.parse().ok())
    .unwrap_or(20261030);

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
    "PETG",
    "PETG-CF",
    "PCTG",
    "TPU for AMS",
    "TPU 95A",
    "ASA",
    "ABS",
    "ABS-CF",
    "PC",
    "PA6-CF",
    "PVA",
  ];
  let non_abrasive_materials = [
    "PLA",
    "PLA+",
    "PHA",
    "PETG",
    "PCTG",
    "TPU for AMS",
    "TPU 95A",
    "ASA",
    "ABS",
    "PC",
    "PVA",
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

    let slot_count = if rig.ace_pro_units == 2 { 8 } else { 6 };
    let ams_capacity = rig.ace_pro_units * 4;
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
      // Soft TPU (TPU 95A) belongs in Reserve slots outside active AMS; active AMS slots use TPU for AMS
      if in_active_ams && target_mat == "TPU 95A" {
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

      let target_finish = if !allow_abrasive {
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
    let rules = TransformationEngine::get_default_rules();
    let derived_overrides =
      resolve_effective_orca_profile_values(&rig, active_proc, &serde_json::Map::new());

    // Compute n x n flushing matrix with 0 diagonals
    let n = spools.len();
    let mut flush_matrix = Vec::with_capacity(n * n);
    for r in 0..n {
      for c in 0..n {
        if r == c {
          flush_matrix.push("0".to_string());
        } else {
          flush_matrix.push("240".to_string());
        }
      }
    }

    let mut entries: Vec<ScannedFileEntry> = Vec::new();
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

    // Also write `project_settings.config` (3MF config) and `orcaslicer_webconfig.json`
    let proj_cfg = build_project_settings_config_json(
      &rig,
      active_proc,
      &spools,
      &flush_matrix,
      &derived_overrides,
    );
    fs::write(cfg_dir.join("project_settings.config"), proj_cfg)
      .expect("Failed to write project_settings.config");

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

      let matching_spool = snap.filaments.iter().find(|s| s.name == name);
      let mat_for_bounds = matching_spool
        .map(|s| s.material_type.as_str())
        .unwrap_or(&ftype);
      let fin_for_bounds = matching_spool
        .map(|s| s.finish_type.as_str())
        .unwrap_or("Basic");

      let (min_noz, max_noz, min_bed, max_bed) =
        safe_temp_bounds_for_polymer(mat_for_bounds, fin_for_bounds);

      if !(min_noz..=max_noz).contains(&noz_t) {
        cfg_failures.push(format!(
          "[{}] Filament '{}' ({} / {}) has unsafe nozzle_temperature={}°C (expected {}..={}°C; notes={})",
          cfg_name, name, mat_for_bounds, fin_for_bounds, noz_t, min_noz, max_noz, notes
        ));
      }
      if init_t < noz_t || init_t > max_noz + 10 {
        cfg_failures.push(format!(
          "[{}] Filament '{}' has invalid initial layer nozzle temp={}°C vs continuous={}°C",
          cfg_name, name, init_t, noz_t
        ));
      }
      if range_low <= 0 || range_high <= 0 || noz_t < range_low || init_t > range_high {
        cfg_failures.push(format!(
          "[{}] Filament '{}' nozzle temp ({}°C / init {}°C) outside its own range [{}..={}°C]",
          cfg_name, name, noz_t, init_t, range_low, range_high
        ));
      }
      if !(min_bed..=max_bed).contains(&bed_t) {
        cfg_failures.push(format!(
          "[{}] Filament '{}' ({}) has out-of-bounds bed_temp={}°C (expected {}..={}°C)",
          cfg_name, name, mat_for_bounds, bed_t, min_bed, max_bed
        ));
      }
      if !(0.45..=1.08).contains(&flow) {
        cfg_failures.push(format!(
          "[{}] Filament '{}' has invalid flow_ratio={}",
          cfg_name, name, flow
        ));
      }
      if !(1.5..=40.0).contains(&mvs) {
        cfg_failures.push(format!(
          "[{}] Filament '{}' has invalid max_volumetric_speed={}",
          cfg_name, name, mvs
        ));
      }
      if !(15.0..=55.0).contains(&ret_spd) {
        cfg_failures.push(format!(
          "[{}] Filament '{}' has invalid retraction_speed={}",
          cfg_name, name, ret_spd
        ));
      }

      let canon = FilamentSpool::canonical_polymer_key(mat_for_bounds);
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

    // 4. Re-import the directory via `import_scanned_directory` and run `evaluate_safety_warnings`
    let mut scanned = Vec::new();
    collect_scanned_entries(cfg_dir, cfg_dir, &mut scanned);
    let imported = import_scanned_directory(&scanned).expect("import_scanned_directory failed");
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
