use orcaslicer_webconfig::catalog::{load_merged_catalog, FILAMENT_BRAND_OPTIONS, FINISH_OPTIONS};
use orcaslicer_webconfig::dir_import::{
  import_scanned_directory, sync_live_state_to_entries,
  sync_live_state_to_entries_with_process_overrides,
};
use orcaslicer_webconfig::fs_access::ScannedFileEntry;
use orcaslicer_webconfig::models::{
  FilamentSpool, MachineRig, ProcessProfile, WebConfigStateSnapshot, WEBCONFIG_APP_VERSION,
};
use orcaslicer_webconfig::orca_profile::OrcaProfile;
use orcaslicer_webconfig::threemf::{
  build_3mf_project_container, build_project_settings_config_json,
  parse_ace_loadout_from_bytes_or_json,
};
use orcaslicer_webconfig::transformations::{
  RuleScopeFilter, SafetySeverity, TransformationEngine,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Clone, Debug, Deserialize)]
struct CppOptionSpec {
  co_type: String,
  is_vector: bool,
  nullable: bool,
  #[serde(default)]
  enum_values: Vec<String>,
  #[serde(default)]
  tooltip: String,
}

#[derive(Debug, Deserialize)]
struct RawGroundTruthBundle {
  print_keys: Vec<String>,
  filament_keys: Vec<String>,
  machine_keys: Vec<String>,
  #[serde(default)]
  anycubic_fork_only_keys: Vec<String>,
  options: HashMap<String, CppOptionSpec>,
  resolved_presets: HashMap<String, HashMap<String, BTreeMap<String, Value>>>,
}

struct GroundTruthSlicerSchema {
  print_keys: HashSet<String>,
  filament_keys: HashSet<String>,
  machine_keys: HashSet<String>,
  anycubic_fork_only_keys: HashSet<String>,
  options: HashMap<String, CppOptionSpec>,
  resolved_presets: HashMap<String, HashMap<String, BTreeMap<String, Value>>>,
}

impl GroundTruthSlicerSchema {
  fn get() -> &'static Self {
    static INSTANCE: OnceLock<GroundTruthSlicerSchema> = OnceLock::new();
    INSTANCE.get_or_init(|| {
      let mut candidates = Vec::new();
      if let Ok(env_p) = std::env::var("SLICER_GROUND_TRUTH_JSON") {
        candidates.push(PathBuf::from(&env_p));
        if let Ok(rf_dir) = std::env::var("RUNFILES_DIR") {
          candidates.push(PathBuf::from(&rf_dir).join("_main").join(&env_p));
        }
        if let Ok(ts_dir) = std::env::var("TEST_SRCDIR") {
          candidates.push(PathBuf::from(&ts_dir).join("_main").join(&env_p));
        }
      }
      candidates.push(PathBuf::from("bazel-bin/slicer_ground_truth.json"));
      let path = candidates
        .into_iter()
        .find(|p| p.exists())
        .expect("Missing slicer_ground_truth.json (ensure SLICER_GROUND_TRUTH_JSON is set by Bazel)");
      let txt = fs::read_to_string(&path).expect("Failed to read slicer_ground_truth.json");
      let raw: RawGroundTruthBundle =
        serde_json::from_str(&txt).expect("Failed to parse slicer_ground_truth.json");
      GroundTruthSlicerSchema {
        print_keys: raw.print_keys.into_iter().collect(),
        filament_keys: raw.filament_keys.into_iter().collect(),
        machine_keys: raw.machine_keys.into_iter().collect(),
        anycubic_fork_only_keys: raw.anycubic_fork_only_keys.into_iter().collect(),
        options: raw.options,
        resolved_presets: raw.resolved_presets,
      }
    })
  }

  /// Validates an exported standalone preset JSON (`machine`, `process`, or `filament`) directly
  /// against OrcaSlicer `Preset.cpp` + `PrintConfig.cpp` + `AnycubicSlicerNext` ground truth.
  fn validate_preset_json(&self, ptype: &str, label: &str, val: &Value, failures: &mut Vec<String>) {
    let Some(obj) = val.as_object() else {
      failures.push(format!("[{}] {} preset is not a JSON object", label, ptype));
      return;
    };
    let allowed = match ptype {
      "process" => &self.print_keys,
      "filament" => &self.filament_keys,
      "machine" => &self.machine_keys,
      _ => return,
    };
    if let Some(inh) = first_str_val(val, "inherits").filter(|s| !s.trim().is_empty()) {
      if let Some(pmap) = self.resolved_presets.get(ptype) {
        if !pmap.contains_key(&inh) {
          failures.push(format!(
            "[{}] {} preset '{}' has unresolved inherits='{}' (missing from OrcaSlicer + AnycubicSlicerNext system presets)",
            label,
            ptype,
            first_str_val(val, "name").unwrap_or_default(),
            inh
          ));
        }
      }
    }
    let is_anycubic_preset = first_str_val(val, "compatible_printers")
      .map(|s| s.to_uppercase().contains("ANYCUBIC"))
      .unwrap_or(false)
      || first_str_val(val, "printer_model")
        .map(|s| s.to_uppercase().contains("ANYCUBIC"))
        .unwrap_or(false)
      || first_str_val(val, "inherits")
        .map(|s| s.to_uppercase().contains("ANYCUBIC"))
        .unwrap_or(false);
    for (k, v) in obj {
      if !is_anycubic_preset && self.anycubic_fork_only_keys.contains(k) {
        failures.push(format!(
          "[{}] non-Anycubic {} preset '{}' illegally emitted AnycubicSlicerNext fork-only key '{}': value={:?}",
          label,
          ptype,
          first_str_val(val, "name").unwrap_or_default(),
          k,
          v
        ));
        continue;
      }
      if !allowed.contains(k) {
        let wrong_scope = if self.filament_keys.contains(k) {
          " (belongs to FILAMENT scope)"
        } else if self.print_keys.contains(k) {
          " (belongs to PROCESS scope)"
        } else if self.machine_keys.contains(k) {
          " (belongs to MACHINE scope)"
        } else {
          " (unknown to OrcaSlicer Preset.cpp & AnycubicSlicerNext)"
        };
        failures.push(format!(
          "[{}] {} preset '{}' contains illegal key '{}'{}: value={:?}",
          label,
          ptype,
          first_str_val(val, "name").unwrap_or_default(),
          k,
          wrong_scope,
          v
        ));
        continue;
      }
      if let Some(opt) = self.options.get(k) {
        if ptype == "filament" && opt.is_vector && !matches!(k.as_str(), "filament_id") {
          if !v.is_array() {
            failures.push(format!(
              "[{}] filament '{}' key '{}' has C++ vector type {} in PrintConfig.cpp but was serialized as non-array {:?}",
              label,
              first_str_val(val, "name").unwrap_or_default(),
              k,
              opt.co_type,
              v
            ));
          }
        }
        if ptype == "process" && !opt.is_vector && k != "compatible_printers" {
          if !v.is_string() {
            failures.push(format!(
              "[{}] process '{}' key '{}' has C++ scalar type {} in PrintConfig.cpp but was serialized as non-string {:?}",
              label,
              first_str_val(val, "name").unwrap_or_default(),
              k,
              opt.co_type,
              v
            ));
          }
        }
        if matches!(opt.co_type.as_str(), "coEnum" | "coEnums") && !opt.enum_values.is_empty() {
          let strs: Vec<&str> = if let Some(s) = v.as_str() {
            vec![s]
          } else if let Some(arr) = v.as_array() {
            arr.iter().filter_map(|x| x.as_str()).collect()
          } else {
            Vec::new()
          };
          for s in strs {
            if s == "nil" && opt.nullable {
              continue;
            }
            if !opt.enum_values.iter().any(|ev| ev == s) {
              failures.push(format!(
                "[{}] {} '{}' key '{}' has invalid PrintConfig.cpp enum value '{}' (allowed: {:?})",
                label,
                ptype,
                first_str_val(val, "name").unwrap_or_default(),
                k,
                s,
                opt.enum_values
              ));
            }
          }
        }
      }
    }
  }
}

struct SimpleRng {
  state: u64,
}

impl SimpleRng {
  fn new(seed: u64) -> Self {
    Self {
      state: if seed == 0 {
        0x9E37_79B9_7F4A_7C15
      } else {
        seed
      },
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
  if MachineRig::is_microswiss_nozzle_type(&rig.nozzle_type) {
    if !rig.addons.iter().any(|a| a == "flowtech_hotend") {
      rig.addons.push("flowtech_hotend".to_string());
    }
  } else {
    rig.addons.retain(|a| a != "flowtech_hotend");
  }
  rig.refresh_title_and_notes(true);
}

/// Base rig for a printer, built the way the scratch wizard builds rigs: identity and
/// kinematics from the generated Orca catalog machine entry (so Bed-Slinger labeling flows
/// through), hardware limits from constants.json `machines[]`. Rotating the random audit
/// across these keeps the multi-printer paths (motion-class travel-accel policy, compat
/// printers, inherits templates) under randomized coverage.
fn base_rig_for_printer(printer_model: &str) -> MachineRig {
  let cat = load_merged_catalog();
  let machine = cat
    .machines
    .iter()
    .find(|m| m.name == printer_model)
    .unwrap_or_else(|| panic!("catalog machine {:?} present", printer_model));
  let mc = orcaslicer_webconfig::catalog::machine_constants(printer_model);
  let mut rig = MachineRig::kobra_s1_hardened_steel_png();
  rig.printer_model = machine.name.clone();
  rig.kinematics = machine.kinematics.clone();
  rig.bed_size = machine.build_volume.clone();
  rig.hotend = format!("{} All-Metal Hotend", machine.name);
  rig.firmware = "OrcaSlicer v2.5.1".to_string();
  rig.ace_pro_units = if machine.name.to_uppercase().contains("ANYCUBIC") {
    1
  } else {
    0
  };
  if machine.default_retraction_mm > 0.0 {
    rig.base_retraction_length = machine.default_retraction_mm as f64;
  }
  rig.max_volumetric_flow_limit = mc.max_volumetric_speed_mm3s as f64;
  rig.max_volumetric_flow = format!("{:.1} mm³/s", mc.max_volumetric_speed_mm3s);
  rig.refresh_title_and_notes(true);
  rig
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
    "=== Phase 0: Negative Hardware Safety & Static Lua Hygiene Checks ==="
  );
  {
    // 0a. Verify incompatible high-temp polymer (`PPA-CF`, min 280°C) on a 260°C PTFE Ender-3
    // triggers a `SafetySeverity::Danger` warning.
    let ender_rig = base_rig_for_printer("Creality Ender-3");
    let ppa_entry = catalog
      .filaments
      .iter()
      .find(|f| f.code == "PPA-CF")
      .expect("PPA-CF in catalog");
    let ppa_spool = FilamentSpool::from_catalog_entry(1, ppa_entry, "orca_hardened_steel");
    let neg_warnings = TransformationEngine::evaluate_safety_warnings(
      &ender_rig,
      &[ppa_spool],
      &OrcaProfile::get_stock_for_printer_model("Creality Ender-3"),
      &[],
      &[],
    );
    assert!(
      neg_warnings.iter().any(|w| {
        w.severity == SafetySeverity::Danger
          && w.id.starts_with("spool_mach_hotend_incompat:")
      }),
      "PPA-CF on 260°C Creality Ender-3 MUST raise spool_mach_hotend_incompat Danger warning, got {:?}",
      neg_warnings
    );

    // 0a-2. Verify Bambu Lab CoreXY kinematics & real X1C MVS values (never watered-down @System 12 mm³/s)
    for bbl_model in ["Bambu Lab X1 Carbon", "Bambu Lab P1S"] {
      let bbl_m = catalog
        .find_machine(bbl_model)
        .unwrap_or_else(|| panic!("{} in catalog", bbl_model));
      assert!(
        bbl_m.kinematics.contains("CoreXY"),
        "{} kinematics must declare CoreXY, got '{}'",
        bbl_model,
        bbl_m.kinematics
      );
    }
    for (bbl_code, min_mvs) in [("PLA", 18.0_f32), ("PETG", 14.0_f32), ("ABS", 15.0_f32)] {
      let bbl_f = catalog
        .filaments
        .iter()
        .find(|f| f.brand == "Bambu Lab" && f.code == bbl_code)
        .unwrap_or_else(|| panic!("Bambu Lab {} in catalog", bbl_code));
      let bbl_spool = FilamentSpool::from_catalog_entry(1, bbl_f, "orca_hardened_steel");
      assert!(
        bbl_f.max_vol_mm3s >= min_mvs && bbl_spool.max_volumetric_speed >= min_mvs,
        "Bambu Lab {} must preserve X1C CoreXY MVS >= {:.1} mm³/s on both CatalogFilament ({:.1}) and FilamentSpool ({:.1})",
        bbl_code,
        min_mvs,
        bbl_f.max_vol_mm3s,
        bbl_spool.max_volumetric_speed
      );
    }

    // 0b. Static scan of `transformations/` to forbid bare `tonumber(config[...])` on potentially
    // array-wrapped or `%`-suffixed OrcaSlicer keys (must use `scalar_num`).
    let mut lua_files = Vec::new();
    fn walk_lua(dir: &Path, out: &mut Vec<PathBuf>) {
      if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
          let p = e.path();
          if p.is_dir() {
            walk_lua(&p, out);
          } else if p.extension().and_then(|s| s.to_str()) == Some("lua") {
            out.push(p);
          }
        }
      }
    }
    walk_lua(Path::new("transformations"), &mut lua_files);
    lua_files.sort();
    for lf in &lua_files {
      if let Ok(src) = fs::read_to_string(lf) {
        for (line_no, line) in src.lines().enumerate() {
          let trimmed = line.trim();
          if trimmed.starts_with("--") {
            continue;
          }
          if let Some(pos) = trimmed.find("tonumber(config[") {
            let after = &trimmed[pos + "tonumber(config[".len()..];
            if let Some(close_bracket) = after.find(']') {
              let tail = &after[close_bracket + 1..];
              if tail.starts_with(')') {
                panic!(
                  "Static Lua Hygiene Violation at {}:{}: bare `tonumber(config[...])` breaks on array-wrapped `[\"val\"]` or `%`-suffixed keys — use `scalar_num(config[...], default)` instead:\n  {}",
                  lf.display(),
                  line_no + 1,
                  trimmed
                );
              }
            }
          }
        }
      }
    }
  }

  println!(
    "=== Phase 1: Generating {} randomized configurations in {} ===",
    num_configs,
    out_root.display()
  );

  let printer_roster = [
    "Anycubic Kobra S1",
    "Bambu Lab X1 Carbon",
    "Creality Ender-3",
    "Anycubic Kobra 3",
    "Anycubic Kobra 4",
    "Anycubic Kobra S1 Max",
    "Anycubic Kobra 2 Pro",
    "Bambu Lab P1S",
    "Voron 2.4 350",
    "Prusa MK4S",
    "Sovol SV06",
  ];

  let mut config_dirs = Vec::new();
  // (kind, param_key, clobbered_rule_id, clobbering_rule_id) across all configs
  let mut shadow_census: Vec<(&'static str, String, String, String)> = Vec::new();
  for cfg_idx in 1..=num_configs {
    let cfg_dir = out_root.join(format!("config_{:03}", cfg_idx));
    fs::create_dir_all(&cfg_dir).expect("Failed to create config subdir");

    // Rotate across 11 diverse printers (Anycubic S1 / S1 Max / Kobra 3 / Kobra 4 / Kobra 2 Pro,
    // Bambu X1C / P1S, Creality Ender-3 Bowden Marlin, Voron 2.4, Prusa MK4S, Sovol SV06).
    let chosen_printer = printer_roster[(cfg_idx - 1) % printer_roster.len()];
    let mut rig = if chosen_printer == "Anycubic Kobra S1" {
      MachineRig::kobra_s1_hardened_steel_png()
    } else {
      base_rig_for_printer(chosen_printer)
    };
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
    // Mirror the app's hardware invariant (MachineRig::on_change_nozzle): MicroSwiss nozzles
    // fit ONLY the FlowTech hotend and are auto-installed with it; the stock hotend takes no
    // MicroSwiss nozzle. Synthetic rigs must not combine them any other way.
    if MachineRig::is_microswiss_nozzle_type(&rig.nozzle_type) {
      if !rig.addons.iter().any(|a| a == "flowtech_hotend") {
        rig.addons.push("flowtech_hotend".to_string());
      }
    } else {
      rig.addons.retain(|a| a != "flowtech_hotend");
    }
    let mut spools = Vec::new();

    let rig_mc = orcaslicer_webconfig::catalog::machine_constants(&rig.printer_model);
    let is_mach_compat_mat = |m: &str| -> bool {
      let poly = orcaslicer_webconfig::constants::constants().lookup_polymer(m);
      let (mn_noz, _) = FilamentSpool::polymer_safe_nozzle_bounds(m);
      let (mn_bed, _) = FilamentSpool::polymer_safe_bed_bounds(m);
      mn_noz <= rig_mc.max_hotend_temp_c
        && mn_bed <= rig_mc.max_bed_temp_c
        && (poly.default_brass_nozzle_c - 10) <= rig_mc.max_hotend_temp_c
    };
    let compat_core_materials: Vec<&str> = core_materials
      .iter()
      .copied()
      .filter(|&m| is_mach_compat_mat(m))
      .collect();
    let compat_non_abrasive_materials: Vec<&str> = non_abrasive_materials
      .iter()
      .copied()
      .filter(|&m| is_mach_compat_mat(m))
      .collect();

    for slot_i in 0..slot_count {
      // Intentionally start from a random catalog entry (often high-temp like ABS-CF / PA6-CF / PCTG)
      // and then perform multiple random brand -> material -> finish -> brand transitions!
      let start_entry = rng.choose(&catalog.filaments);
      let mut sp = FilamentSpool::from_catalog_entry(slot_i + 1, start_entry, &rig.nozzle_type);

      // Step A: switch to a high-temp or conductive/foaming state first to try to provoke stale temp bugs
      let chaos_mat = *rng.choose(&compat_core_materials);
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
        *rng.choose(&compat_core_materials)
      } else {
        *rng.choose(&compat_non_abrasive_materials)
      };
      // Soft TPU (TPU 95A / TPU 90A / TPU 85A / PEBA) belongs in Reserve slots outside active AMS; active AMS slots use TPU for AMS
      if in_active_ams && matches!(target_mat, "TPU 95A" | "TPU 90A" | "TPU 85A" | "PEBA") {
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
      } else if after_up.contains("-GF") || after_up.contains("-RGF") || after_up.contains("GLASS")
      {
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
      sp.recompute_for_rig(&rig);
      sp.clamp_to_machine_limits(&rig.printer_model);
      sp.refresh_thermal_deltas(&rig.nozzle_type);
    }

    // Build rig-tailored process presets (real system `inherits` + machine-scaled speeds/accels)
    // and rig-specific stock base profile.
    let processes = ProcessProfile::get_default_processes_for_rig(&rig);
    let active_proc_idx = rng.next_usize(processes.len());
    let active_proc = &processes[active_proc_idx];

    let base_profile = OrcaProfile::get_stock_for_printer_model(&rig.printer_model);
    let mut rules = TransformationEngine::get_default_rules();
    TransformationEngine::sync_rules_for_rig(
      &mut rules,
      &rig.nozzle_type,
      &rig.nozzle_material,
      rig.ace_pro_units,
    );
    // Toggle combinations of optional Lua symptom/filament/process rules across configs
    let templates = TransformationEngine::get_parameterized_calibration_templates();
    for r in &mut rules {
      if r.id == "sym_wet_filament" && rng.next_usize(2) == 0 {
        r.enabled = true;
        let rh_choice = *rng.choose(&[30, 40, 50, 65]);
        let mut params = std::collections::HashMap::new();
        params.insert("filament_humidity_pct".to_string(), rh_choice.to_string());
        r.lua_code =
          orcaslicer_webconfig::lua_engine::embed_param_defaults_in_lua(&r.lua_code, &params);
      } else if matches!(
        r.id.as_str(),
        "line_width_geometry" | "outer_wall_consistency"
      ) && rng.next_usize(2) == 0
      {
        if let Some(tpl) = templates.iter().find(|t| t.id == r.id) {
          let mut ans = tpl.default_answers();
          if r.id == "line_width_geometry" {
            let preset = *rng.choose(&["balanced", "strength", "fine_detail"]);
            tpl.apply_answer_change(&mut ans, "geometry_preset", preset.to_string());
          } else if r.id == "outer_wall_consistency" {
            let preset = *rng.choose(&["auto_guard", "sub_resonance_gloss", "spire_safe"]);
            tpl.apply_answer_change(&mut ans, "consistency_mode", preset.to_string());
          }
          let (title, desc, _badge, lua_src) = tpl.generate_from_answers(&ans, &r.condition_expr);
          r.name = title;
          r.description = desc;
          r.lua_code = lua_src;
        }
      } else if r.id == "weather_climate_adjustment"
        && rig.climate_preset != "temperate"
        && rng.next_usize(2) == 0
      {
        r.enabled = true;
        let mut params = std::collections::HashMap::new();
        params.insert("climate_preset".to_string(), rig.climate_preset.clone());
        params.insert("ambient_temp_c".to_string(), rig.ambient_temp_c.to_string());
        params.insert("humidity_pct".to_string(), rig.humidity_pct.to_string());
        let brim_choice = *rng.choose(&["outer_only", "brim_ears", "keep"]);
        params.insert("cold_brim_mode".to_string(), brim_choice.to_string());
        r.lua_code =
          orcaslicer_webconfig::lua_engine::embed_param_defaults_in_lua(&r.lua_code, &params);
      } else if matches!(
        r.id.as_str(),
        "symptom_bed_adhesion_petg"
          | "symptom_spaghetti_pla"
          | "symptom_clog_ace_pro"
          | "filament_pctg_antistringing"
          | "process_scarf_seams_arachne"
          | "support_tuning"
          | "ironing_tuning"
          | "dimensional_precision"
          | "bridge_overhang_tuning"
          | "structural_strength"
      ) && rng.next_usize(3) == 0
      {
        r.enabled = true;
        if let Some(tpl) = templates.iter().find(|t| t.id == r.id) {
          let mut ans = tpl.default_answers();
          match r.id.as_str() {
            "support_tuning" => {
              let preset = *rng.choose(&["auto_polymer", "snug_mechanical", "zero_gap_dissimilar"]);
              tpl.apply_answer_change(&mut ans, "support_preset", preset.to_string());
            }
            "ironing_tuning" => {
              let preset = *rng.choose(&[
                "auto_polymer",
                "pla_matte_satin",
                "petg_gloss_seal",
                "asa_abs_dense",
                "custom",
              ]);
              tpl.apply_answer_change(&mut ans, "ironing_preset", preset.to_string());
              if preset == "custom" {
                let itype = *rng.choose(&["top", "topmost", "solid"]);
                tpl.apply_answer_change(&mut ans, "ironing_type", itype.to_string());
              }
            }
            "dimensional_precision" => {
              let preset = *rng.choose(&[
                "surface_and_overhang",
                "mechanical_exact",
                "clearance_slip_fit",
                "press_fit_tight",
              ]);
              tpl.apply_answer_change(&mut ans, "fit_preset", preset.to_string());
            }
            "bridge_overhang_tuning" => {
              let preset = *rng.choose(&[
                "tensioned_anti_sag",
                "steep_overhang_max",
                "high_speed_functional",
              ]);
              tpl.apply_answer_change(&mut ans, "bridge_preset", preset.to_string());
            }
            "structural_strength" => {
              let preset = *rng.choose(&["functional", "load_bearing", "stiff_crosshatch"]);
              tpl.apply_answer_change(&mut ans, "strength_preset", preset.to_string());
            }
            _ => {}
          }
          let (title, desc, _badge, lua_src) = tpl.generate_from_answers(&ans, &r.condition_expr);
          r.name = title;
          r.description = desc;
          r.lua_code = lua_src;
        }
        // Exercise declarative RuleScopeFilters (Item #2) on some enabled core packs
        if r.id == "ironing_tuning" && rng.next_usize(2) == 0 {
          r.scope = RuleScopeFilter {
            polymers: vec!["PLA".to_string(), "PETG".to_string()],
            nozzle_diameters: Vec::new(),
            nozzle_materials: Vec::new(),
            process_tiers: vec!["fine".to_string(), "quality".to_string()],
          };
        } else if r.id == "structural_strength" && rng.next_usize(2) == 0 {
          r.scope = RuleScopeFilter {
            polymers: Vec::new(),
            nozzle_diameters: vec!["0.4".to_string(), "0.6".to_string()],
            nozzle_materials: Vec::new(),
            process_tiers: vec![
              "standard".to_string(),
              "speed".to_string(),
              "draft".to_string(),
            ],
          };
        }
      }
    }

    // Also instantiate 1–2 parameterized Symptom (`sym_*`) or Calibration (`cal_*`) wizard rules
    // (exercising all 25+ templates in `transformations/parametric/` under real stack & export!).
    let wizard_ids = [
      "sym_warping_adhesion",
      "sym_not_sticking",
      "sym_elephants_foot",
      "sym_stringing",
      "sym_boogers",
      "sym_clogging",
      "sym_overhang_curl",
      "sym_weak_layers",
      "sym_zits_blobs",
      "sym_layer_shifting",
      "sym_corner_bulge",
      "sym_under_extrusion",
      "sym_over_extrusion",
      "sym_infill_wall_gaps",
      "sym_pillowing",
      "sym_ringing_vfa",
      "sym_visible_seam",
      "cal_yolo_flow",
      "cal_pressure_advance",
      "cal_retraction_tower",
      "cal_max_volumetric_speed",
    ];
    if rng.next_usize(2) == 0 {
      let wid = *rng.choose(&wizard_ids);
      if let Some(tpl) = templates.iter().find(|t| t.id == wid) {
        let ans = tpl.default_answers();
        let target_poly = spools[0].material_type.clone();
        let (title, desc, _badge, lua_src) = tpl.generate_from_answers(&ans, "all");
        rules.push(orcaslicer_webconfig::transformations::TransformationRule {
          id: format!("wiz_{}_{}", wid, cfg_idx),
          name: title,
          description: desc,
          category: "Wizard Rule".to_string(),
          icon: "🧙".to_string(),
          enabled: true,
          condition_expr: "all".to_string(),
          lua_code: lua_src,
          priority: 52,
          scope: RuleScopeFilter {
            polymers: vec![target_poly],
            nozzle_diameters: Vec::new(),
            nozzle_materials: Vec::new(),
            process_tiers: Vec::new(),
          },
        });
      }
    }

    // Evaluate the real Lua transformation stack (including `flush_volumes_matrix.lua`, per-process evaluation, and new TODO packs)
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
    let all_filament_types: Vec<String> = spools.iter().map(|s| s.material_type.clone()).collect();
    let eval_ctx = serde_json::json!({
      "printer_model": rig.printer_model,
      "hotend": rig.hotend,
      "nozzle_type": rig.nozzle_type,
      "nozzle_material": rig.nozzle_material,
      "nozzle_diameter": rig.nozzle_diameter,
      "hotend_max_flow": rig.max_volumetric_flow_limit,
      "hotend_max_temp_c": orcaslicer_webconfig::catalog::machine_constants(&rig.printer_model).max_hotend_temp_c,
      "printer_kinematics_class": rig.kinematics_class(),
      "travel_accel_target_mm2": rig.travel_accel_policy().travel_accel_target_mm2,
      "travel_accel_floor_mm2": rig.travel_accel_policy().travel_accel_floor_mm2,
      "filament_max_flow": spools[0].max_volumetric_speed as f64,
      "melt_zone_multiplier": rig.melt_zone_multiplier,
      "flush_multiplier": 1.0,
      "layer_height": active_proc.layer_height,
      "line_width": active_proc.line_width,
      "filament_type": spools[0].material_type,
      "filament_finish": spools[0].finish_type,
      "filament_is_elastomer": orcaslicer_webconfig::constants::constants()
        .is_elastomer_family_material(&spools[0].material_type),
      "filament_nozzle_max_c": FilamentSpool::polymer_safe_nozzle_bounds(&spools[0].material_type).1,
      "all_filament_types": all_filament_types,
      "addons": rig.addons,
      "climate_preset": rig.climate_preset,
      "ambient_temp_c": rig.ambient_temp_c,
      "humidity_pct": rig.humidity_pct,
      "slots": slots_json,
    });
    let (seeded_base, seeded_ctx) =
      TransformationEngine::seed_process_inputs(&base_profile, &eval_ctx, active_proc);
    let stack_eval = TransformationEngine::evaluate_stack(&seeded_base, &rules, &seeded_ctx);
    // Census of "⚡ Overwritten by later rule" (intent lost) vs "♻ Recomputed" (later rule
    // re-derived the value from this step's other outputs — intent preserved) steps.
    for prov in &stack_eval.provenance_items {
      for step in &prov.steps {
        if let Some(kind) = step.shadow_kind {
          shadow_census.push((
            match kind {
              orcaslicer_webconfig::transformations::RuleShadowKind::Overwritten => "OVERWRITTEN",
              orcaslicer_webconfig::transformations::RuleShadowKind::Recomputed => "recomputed",
              orcaslicer_webconfig::transformations::RuleShadowKind::Accepted => "accepted",
              orcaslicer_webconfig::transformations::RuleShadowKind::Clamped => "clamped",
            },
            step.param_key.clone(),
            step.rule_id.clone(),
            step
              .shadowed_by_rule_id
              .clone()
              .unwrap_or_else(|| "?".to_string()),
          ));
        }
      }
    }
    let flush_matrix = stack_eval.flush_matrix;
    let per_proc_overrides = TransformationEngine::evaluate_stack_per_process(
      &base_profile,
      &rules,
      &eval_ctx,
      &processes,
    );
    let derived_overrides = per_proc_overrides
      .get(&active_proc.id)
      .cloned()
      .unwrap_or(stack_eval.resulting_profile.values);

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
    let save_files = sync_live_state_to_entries_with_process_overrides(
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
      &per_proc_overrides,
    );

    for sf in &save_files.files {
      let full_path = cfg_dir.join(&sf.relative_path);
      if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent).expect("Failed to create parent dir");
      }
      fs::write(&full_path, &sf.content).expect("Failed to write save file");
    }
    for del_rel in &save_files.deletions {
      let full_path = cfg_dir.join(del_rel);
      let _ = fs::remove_file(&full_path);
      let _ = fs::remove_file(full_path.with_extension("info"));
    }

    // Also write `project_settings.config` (3MF config), `project_export.3mf`, and `orcaslicer_webconfig.json`
    let proj_cfg = build_project_settings_config_json(
      &rig,
      active_proc,
      &spools,
      &flush_matrix,
      &derived_overrides,
    );
    let slot_filament_prov: Vec<Vec<orcaslicer_webconfig::transformations::ParameterProvenance>> =
      spools
        .iter()
        .map(|sp| {
          orcaslicer_webconfig::threemf::evaluate_spool_filament_provenance(
            sp,
            &rig,
            Some(&derived_overrides),
          )
          .provenance_items
        })
        .collect();
    let mut mach_json = String::new();
    let mut proc_json = String::new();
    let mut fil_map = std::collections::HashMap::new();
    for sf in &save_files.files {
      if sf.relative_path.starts_with("user/default/machine/") && sf.relative_path.ends_with(".json")
      {
        mach_json = sf.content.clone();
      } else if sf.relative_path.starts_with("user/default/process/")
        && sf.relative_path.ends_with(".json")
        && sf.relative_path.contains(&active_proc.name)
      {
        proc_json = sf.content.clone();
      } else if sf.relative_path.starts_with("user/default/filament/")
        && sf.relative_path.ends_with(".json")
      {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&sf.content) {
          if let Some(name) = v.get("name").and_then(|n| n.as_str()) {
            fil_map.insert(name.to_string(), sf.content.clone());
          }
        }
      }
    }
    if mach_json.is_empty() {
      mach_json = orcaslicer_webconfig::threemf::build_machine_preset_json_with_overrides(
        &rig,
        Some(&derived_overrides),
      );
    }
    if proc_json.is_empty() {
      proc_json = orcaslicer_webconfig::threemf::build_process_preset_json(
        active_proc,
        &rig,
        &derived_overrides,
      );
    }
    for sp in &spools {
      let title = sp.format_concise_title_for_rig(&rig);
      fil_map.entry(title).or_insert_with(|| {
        orcaslicer_webconfig::threemf::export_standalone_filament_json_with_overrides(
          sp,
          &rig,
          &derived_overrides,
        )
      });
    }
    if let Err(errs) =
      orcaslicer_webconfig::threemf::verify_provenance_matches_standalone_and_3mf_exports(
        &stack_eval.provenance_items,
        &slot_filament_prov,
        &proj_cfg,
        &mach_json,
        &proc_json,
        &fil_map,
      )
    {
      panic!(
        "[config_{:03}] Provenance ledger chain / 3-way export parity failed: {:?}",
        cfg_idx, errs
      );
    }
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
      spoolman_url: "http://127.0.0.1:7912".to_string(),
      spoolman_inventory: Vec::new(),
      moonraker_enabled: false,
      moonraker_url: "http://192.168.1.100:7125".to_string(),
      active_moonraker_spool_id: None,
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
    let rig = snap
      .machines
      .first()
      .expect("Missing machine rig in snapshot");

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
      GroundTruthSlicerSchema::get().validate_preset_json(
        "filament",
        cfg_name,
        &val,
        &mut cfg_failures,
      );
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
      let expected_compat_printer = format!(
        "{} {} nozzle",
        rig.printer_model,
        rig.formatted_nozzle_diameter()
      );
      if val.get("compatible_printers") != Some(&serde_json::json!([expected_compat_printer])) {
        cfg_failures.push(format!(
          "[{}] Filament '{}' must have compatible_printers = ['{}'], got {:?}",
          cfg_name,
          name,
          expected_compat_printer,
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
      // Flexible-family speed cap, retraction-speed band, and the nozzle-temp floor
      // relaxation all come from constants.json `hardware_limits` (the same single source
      // the resolver clamp and the Lua `ctx.filament_is_elastomer` flag use).
      let hw_limits = &orcaslicer_webconfig::constants::constants().hardware_limits;

      let init_bed: i32 = first_str_val(&val, "textured_plate_temp_initial_layer")
        .and_then(|s| s.parse().ok())
        .unwrap_or(-1);
      let hot_bed: i32 = first_str_val(&val, "hot_plate_temp")
        .and_then(|s| s.parse().ok())
        .unwrap_or(-1);
      let cool_bed: i32 = first_str_val(&val, "cool_plate_temp")
        .and_then(|s| s.parse().ok())
        .unwrap_or(-1);
      let tex_cool_bed: i32 = first_str_val(&val, "textured_cool_plate_temp")
        .and_then(|s| s.parse().ok())
        .unwrap_or(-1);
      let eng_bed: i32 = first_str_val(&val, "eng_plate_temp")
        .and_then(|s| s.parse().ok())
        .unwrap_or(-1);
      let supertack_bed: i32 = first_str_val(&val, "supertack_plate_temp")
        .and_then(|s| s.parse().ok())
        .unwrap_or(-1);
      let fan_min: i32 = first_str_val(&val, "fan_min_speed")
        .and_then(|s| s.parse().ok())
        .unwrap_or(-1);
      let fan_max: i32 = first_str_val(&val, "fan_max_speed")
        .and_then(|s| s.parse().ok())
        .unwrap_or(-1);
      let rig_mc = orcaslicer_webconfig::catalog::machine_constants(&rig.printer_model);
      let machine_max_c = rig_mc.max_hotend_temp_c;
      let machine_max_bed_c = rig_mc.max_bed_temp_c;
      let rig_cm_opt = catalog.find_machine(&rig.printer_model);

      // Verify `filament.inherits` is a REAL instantiated OrcaSlicer / AnycubicSlicerNext preset
      // (never a fabricated cross-vendor or non-existent `@Creality ...` string!).
      let fil_inh = first_str_val(&val, "inherits").unwrap_or_default();
      let fil_inh_valid = catalog.instantiated_system_filaments.contains(&fil_inh)
        || rig_cm_opt
          .map(|cm| cm.filament_presets.values().any(|p| p == &fil_inh))
          .unwrap_or(false);
      if !fil_inh_valid
        || (rig.printer_model != "Anycubic Kobra S1"
          && !rig.printer_model.starts_with("Anycubic")
          && fil_inh.contains("Anycubic"))
      {
        cfg_failures.push(format!(
          "[{}] Filament '{}' on '{}' exported non-instantiated or cross-vendor inherits='{}'",
          cfg_name, name, rig.printer_model, fil_inh
        ));
      }

      if !((min_noz - hw_limits.nozzle_temp_floor_relax_c - 6)..=(max_noz + 10)).contains(&noz_t)
        || noz_t > 300
        || init_t > 300
        || noz_t > machine_max_c
        || init_t > machine_max_c
      {
        cfg_failures.push(format!(
          "[{}] Filament '{}' ({} / {}) has unsafe nozzle_temperature={}°C / init={}°C (expected {}..={}°C and <={}°C hotend ceiling; notes={})",
          cfg_name, name, mat_for_bounds, fin_for_bounds, noz_t, init_t, min_noz, max_noz, machine_max_c, notes
        ));
      }
      if init_t < noz_t || init_t > max_noz + 10 {
        cfg_failures.push(format!(
          "[{}] Filament '{}' has invalid initial layer nozzle temp={}°C vs continuous={}°C",
          cfg_name, name, init_t, noz_t
        ));
      }
      if range_low < 175
        || range_high > machine_max_c
        || range_high > 300
        || noz_t < range_low
        || init_t > range_high
      {
        cfg_failures.push(format!(
          "[{}] Filament '{}' nozzle temp ({}°C / init {}°C) or range [{}..={}°C] violates machine [175..={}°C] limits",
          cfg_name, name, noz_t, init_t, range_low, range_high, machine_max_c
        ));
      }
      let exp_noz_note = format!("Nozzle: {}°C", noz_t);
      let exp_bed_note = format!("Bed: {}°C", bed_t);
      let exp_flow_note = format!("Flow: {:.2}", flow);
      let exp_mvs_note = format!("Max Vol: {:.1} mm³/s", mvs);
      let exp_fan_note = format!("Fan: {}%", fan_min);
      if !notes.contains(&exp_noz_note)
        || !notes.contains(&exp_bed_note)
        || !notes.contains(&exp_flow_note)
        || !notes.contains(&exp_mvs_note)
        || !notes.contains(&exp_fan_note)
      {
        cfg_failures.push(format!(
          "[{}] Filament '{}' filament_notes ('{}') does not match exported '{}' / '{}' / '{}' / '{}' / '{}'",
          cfg_name, name, notes, exp_noz_note, exp_bed_note, exp_flow_note, exp_mvs_note, exp_fan_note
        ));
      }
      let eff_max_bed = max_bed.min(machine_max_bed_c);
      let eff_min_bed = min_bed.min(eff_max_bed);
      if !(eff_min_bed..=eff_max_bed).contains(&bed_t)
        || bed_t > machine_max_bed_c
        || init_bed > machine_max_bed_c
        || init_bed < bed_t
      {
        cfg_failures.push(format!(
          "[{}] Filament '{}' ({}) on '{}' has out-of-bounds bed_temp={}°C / init_bed={}°C (expected {}..={}°C and <={}°C machine bed ceiling)",
          cfg_name, name, mat_for_bounds, rig.printer_model, bed_t, init_bed, eff_min_bed, eff_max_bed, machine_max_bed_c
        ));
      }

      let poly_for_fan = orcaslicer_webconfig::constants::constants().lookup_polymer(mat_for_bounds);
      let is_warp_prone_mat = poly_for_fan.is_warp_prone()
        || mat_for_bounds.to_uppercase().contains("HIPS")
        || mat_for_bounds.to_uppercase().starts_with("PET-CF")
        || mat_for_bounds.to_uppercase().starts_with("PET-GF");
      let is_tpu_family_early =
        orcaslicer_webconfig::constants::constants().is_elastomer_family_material(mat_for_bounds);
      let is_high_temp_plate_mat = is_warp_prone_mat
        || mat_for_bounds.to_uppercase().contains("PCTG")
        || bed_t >= 80;

      let parent_plates = catalog
        .instantiated_filament_plates
        .get(&fil_inh)
        .copied()
        .unwrap_or([-1; 6]);
      let p_cool = parent_plates[0] as i32;
      let p_tex_cool = parent_plates[1] as i32;
      let p_eng = parent_plates[3] as i32;

      // Ground-truth 6-plate surface chemistry & parent-preset invariants:
      // 1. Low-temp coated plates NEVER exceed their coating thermal survival limits (`cool <= 60°C`,
      //    `textured_cool <= 65°C`, `supertack <= 70°C` — NEVER stamped with 100–110°C PEI bed temps!).
      if cool_bed < 0 || cool_bed > 60 || tex_cool_bed < 0 || tex_cool_bed > 65 || supertack_bed < 0 || supertack_bed > 70 {
        cfg_failures.push(format!(
          "[{}] Filament '{}' ({}) exported coated plate temp exceeding coating survival limits: cool={}°C, tex_cool={}°C, supertack={}°C",
          cfg_name, name, mat_for_bounds, cool_bed, tex_cool_bed, supertack_bed
        ));
      }
      // 2. High-temp warp-prone polymers (`ABS`, `ASA`, `PC`, `PA*`, `PET-CF`, `HIPS`) and `PCTG`
      //    MUST mark Cool Plate / Textured Cool Plate as unsupported (`0°C`) — even when AnycubicSlicerNext's
      //    parent inherited `35°C` / `40°C` from `fdm_filament_common` — so selecting a Cool Plate never
      //    prints ABS/PC/PA at 35°C! Conversely, low-temp polymers preserve non-zero parent Cool Plate support.
      let is_pctg_mat = mat_for_bounds.to_uppercase().contains("PCTG");
      if is_warp_prone_mat || is_pctg_mat {
        if cool_bed != 0 || tex_cool_bed != 0 || (is_warp_prone_mat && supertack_bed != 0) {
          cfg_failures.push(format!(
            "[{}] High-temp/warp filament '{}' ({}) must disable low-temp coated plates (cool={}, tex_cool={}, supertack={})!",
            cfg_name, name, mat_for_bounds, cool_bed, tex_cool_bed, supertack_bed
          ));
        }
      } else {
        if p_cool == 0 && cool_bed != 0 {
          cfg_failures.push(format!(
            "[{}] Filament '{}' inherits '{}' (parent cool_plate_temp=0) but exported cool_plate_temp={}°C",
            cfg_name, name, fil_inh, cool_bed
          ));
        } else if p_cool > 0 && p_cool <= 55 && cool_bed == 0 {
          cfg_failures.push(format!(
            "[{}] Filament '{}' inherits '{}' (parent cool_plate_temp={}°C) but clobbered cool_plate_temp to 0°C!",
            cfg_name, name, fil_inh, p_cool
          ));
        }
        if p_tex_cool > 0 && p_tex_cool <= 60 && tex_cool_bed == 0 {
          cfg_failures.push(format!(
            "[{}] Filament '{}' inherits '{}' (parent textured_cool_plate_temp={}°C) but clobbered textured_cool_plate_temp to 0°C!",
            cfg_name, name, fil_inh, p_tex_cool
          ));
        }
      }
      if p_eng == 0 && eng_bed != 0 {
        cfg_failures.push(format!(
          "[{}] Filament '{}' inherits '{}' (parent eng_plate_temp=0) but exported eng_plate_temp={}°C",
          cfg_name, name, fil_inh, eng_bed
        ));
      }

      // 3. Universal Layer-1 -> Layer-2 Bed Contraction Guard across ALL 6 plate pairs (`0 <= init - cont <= 5°C`):
      let max_plate_step = if is_tpu_family_early { 3 } else { 5 };
      for (p_cont_k, p_init_k) in [
        ("textured_plate_temp", "textured_plate_temp_initial_layer"),
        ("hot_plate_temp", "hot_plate_temp_initial_layer"),
        ("eng_plate_temp", "eng_plate_temp_initial_layer"),
        ("cool_plate_temp", "cool_plate_temp_initial_layer"),
        ("textured_cool_plate_temp", "textured_cool_plate_temp_initial_layer"),
        ("supertack_plate_temp", "supertack_plate_temp_initial_layer"),
      ] {
        if let (Some(c_v), Some(i_v)) = (
          first_str_val(&val, p_cont_k).and_then(|s| s.parse::<i32>().ok()),
          first_str_val(&val, p_init_k).and_then(|s| s.parse::<i32>().ok()),
        ) {
          if i_v < c_v || i_v - c_v > max_plate_step {
            cfg_failures.push(format!(
              "[{}] Filament '{}' ({}) has unsafe Layer-1 -> Layer-2 plate step {}={}°C -> {}={}°C (max step {}°C)",
              cfg_name, name, mat_for_bounds, p_init_k, i_v, p_cont_k, c_v, max_plate_step
            ));
          }
        }
      }

      // 4. Zero 24V PWM Fan Stall Zone (`1..=9%`), Klipper PA Activation, & Chamber Exhaust Activation:
      for pwm_k in [
        "fan_min_speed",
        "fan_max_speed",
        "overhang_fan_speed",
        "additional_cooling_fan_speed",
        "during_print_exhaust_fan_speed",
      ] {
        if let Some(pwm_v) = first_str_val(&val, pwm_k)
          .and_then(|s| s.trim().trim_end_matches('%').parse::<i32>().ok())
        {
          if (1..=9).contains(&pwm_v) {
            cfg_failures.push(format!(
              "[{}] Filament '{}' exported {}={}% inside the 1–9% 24V PWM blower stall zone!",
              cfg_name, name, pwm_k, pwm_v
            ));
          }
        }
      }
      if rig.base_retraction_length < 2.0
        && first_str_val(&val, "enable_pressure_advance").as_deref() != Some("1")
      {
        cfg_failures.push(format!(
          "[{}] Direct-drive filament '{}' must export enable_pressure_advance='1' so pressure_advance is not dead!",
          cfg_name, name
        ));
      } else if rig.base_retraction_length >= 2.0
        && first_str_val(&val, "enable_pressure_advance").as_deref() != Some("0")
      {
        cfg_failures.push(format!(
          "[{}] Bowden Marlin filament '{}' on '{}' must export enable_pressure_advance='0' (stock Marlin LIN_ADVANCE disabled)!",
          cfg_name, name, rig.printer_model
        ));
      }
      let has_glass_or_struct_fan_override = snap.rules.iter().any(|r| {
        r.enabled
          && (r.id.contains("transparent_glass")
            || r.id.contains("structural_strength")
            || r.lua_code.contains("_structural_fan_cut_pct"))
      });
      if rig.kinematics.contains("Aux Cooling")
        && rig.ambient_temp_c >= 26
        && !has_glass_or_struct_fan_override
        && matches!(FilamentSpool::canonical_polymer_key(mat_for_bounds), "PLA" | "PLA+" | "PHA")
        && !mat_for_bounds.to_uppercase().contains("-CF")
        && !mat_for_bounds.to_uppercase().contains("-GF")
        && !fin_for_bounds.to_uppercase().contains("CARBON")
        && !fin_for_bounds.to_uppercase().contains("GLASS")
      {
        let aux_pct = first_str_val(&val, "additional_cooling_fan_speed")
          .and_then(|s| s.trim().trim_end_matches('%').parse::<i32>().ok())
          .unwrap_or(0);
        if aux_pct <= 0 {
          cfg_failures.push(format!(
            "[{}] Hot/Tropical Aux-Cooling rig '{}' ({}°C) wiped neat PLA/PHA '{}' additional_cooling_fan_speed to {}%!",
            cfg_name, rig.printer_model, rig.ambient_temp_c, name, aux_pct
          ));
        }
      }
      if rig.is_bedslinger() && val.get("during_print_exhaust_fan_speed").is_some() {
        cfg_failures.push(format!(
          "[{}] Open-frame bed-slinger '{}' filament '{}' must NOT export chamber during_print_exhaust_fan_speed!",
          cfg_name, rig.printer_model, name
        ));
      } else if !rig.is_bedslinger() {
        if let Some(ex_v) = first_str_val(&val, "during_print_exhaust_fan_speed")
          .and_then(|s| s.trim().trim_end_matches('%').parse::<i32>().ok())
        {
          if ex_v > 0 && first_str_val(&val, "activate_air_filtration").as_deref() != Some("1") {
            cfg_failures.push(format!(
              "[{}] Enclosed filament '{}' set during_print_exhaust_fan_speed={}% without activate_air_filtration='1' (dead M106 P3 in GCode.cpp)!",
              cfg_name, name, ex_v
            ));
          }
        }
      }
      if is_warp_prone_mat {
        if let Some(vit_v) =
          first_str_val(&val, "temperature_vitrification").and_then(|s| s.parse::<i32>().ok())
        {
          if vit_v <= init_bed {
            cfg_failures.push(format!(
              "[{}] Warp-prone filament '{}' ({}) has temperature_vitrification={}°C <= init_bed={}°C (would falsely warn user to open chamber door!)",
              cfg_name, name, mat_for_bounds, vit_v, init_bed
            ));
          }
        }
      }
      if is_high_temp_plate_mat {
        let mat_up_abs = mat_for_bounds.to_uppercase();
        if (mat_up_abs.contains("ABS") && !mat_up_abs.contains("PC"))
          || mat_up_abs.contains("ASA")
        {
          if bed_t > 100 && hot_bed >= bed_t {
            cfg_failures.push(format!(
              "[{}] ABS/ASA filament '{}' at textured={}°C (>100°C) must step down Smooth PEI hot_plate_temp (<{}°C), got {}°C",
              cfg_name, name, bed_t, bed_t, hot_bed
            ));
          }
        }
      }
      if is_tpu_family_early && supertack_bed != 0 {
        cfg_failures.push(format!(
          "[{}] Elastomer filament '{}' ({}) MUST disable supertack_plate_temp ('0') to prevent tearing SuperTack coating, got {}°C",
          cfg_name, name, mat_for_bounds, supertack_bed
        ));
      }
      if poly_for_fan.is_copolyester() && fan_min > 65 {
        cfg_failures.push(format!(
          "[{}] Copolyester filament '{}' ({}) exported excessive continuous fan_min_speed={}% (>65% ruins PETG/PETG-CF/PCTG layer bonding)",
          cfg_name, name, mat_for_bounds, fan_min
        ));
      }

      // Warp-prone thermal-step guard (`init_bed - bed_t <= 5°C`), `close_fan_the_first_x_layers >= 3`,
      // and `fan_min < fan_max` on warp-prone polymers.
      let close_fan_layers = first_str_val(&val, "close_fan_the_first_x_layers")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(-1);
      if close_fan_layers < 1 {
        cfg_failures.push(format!(
          "[{}] Filament '{}' missing or zero close_fan_the_first_x_layers={}",
          cfg_name, name, close_fan_layers
        ));
      }
      if is_warp_prone_mat {
        if init_bed - bed_t > 5 {
          cfg_failures.push(format!(
            "[{}] Warp-prone filament '{}' ({}) has excessive Layer 1 -> Layer 2 bed drop ({}°C -> {}°C > 5°C across Tg)",
            cfg_name, name, mat_for_bounds, init_bed, bed_t
          ));
        }
        if close_fan_layers < 3 {
          cfg_failures.push(format!(
            "[{}] Warp-prone filament '{}' ({}) must keep close_fan_the_first_x_layers >= 3, got {}",
            cfg_name, name, mat_for_bounds, close_fan_layers
          ));
        }
        if fan_max > 0 && fan_min >= fan_max {
          cfg_failures.push(format!(
            "[{}] Warp-prone filament '{}' ({}) must keep continuous fan_min_speed ({}%) below fan_max_speed ({}%)",
            cfg_name, name, mat_for_bounds, fan_min, fan_max
          ));
        }
      }
      if is_tpu_family_early {
        let zhop_v = first_str_val(&val, "filament_z_hop")
          .and_then(|s| s.parse::<f64>().ok())
          .unwrap_or(-1.0);
        if zhop_v.abs() > 1e-6 {
          cfg_failures.push(format!(
            "[{}] Elastomer filament '{}' ({}) must export filament_z_hop = 0 (no ooze lift on TPU/PEBA), got {}",
            cfg_name, name, mat_for_bounds, zhop_v
          ));
        }
      }

      if fan_min < 0 || fan_max < 0 || fan_min > fan_max || fan_max > 100 {
        cfg_failures.push(format!(
          "[{}] Filament '{}' has invalid or inverted cooling fan speeds: fan_min_speed={}% vs fan_max_speed={}%",
          cfg_name, name, fan_min, fan_max
        ));
      }
      if is_warp_prone_mat {
        if let Some(ovh_s) = first_str_val(&val, "overhang_fan_speed") {
          if let Ok(ovh_n) = ovh_s.trim().trim_end_matches('%').parse::<i32>() {
            if ovh_n > 35 {
              cfg_failures.push(format!(
                "[{}] Warp-prone filament '{}' ({}) exported unsafe overhang_fan_speed={}% (> 35% delamination/warp threshold)",
                cfg_name, name, mat_for_bounds, ovh_n
              ));
            }
          }
        }
      } else if let Some(ovh_s) = first_str_val(&val, "overhang_fan_speed") {
        if let Ok(ovh_n) = ovh_s.trim().trim_end_matches('%').parse::<i32>() {
          if ovh_n < fan_min {
            cfg_failures.push(format!(
              "[{}] Filament '{}' ({}) exported overhang_fan_speed={}% slower than continuous fan_min_speed={}%",
              cfg_name, name, mat_for_bounds, ovh_n, fan_min
            ));
          }
        }
      }
      let is_foaming = fin_for_bounds.eq_ignore_ascii_case("Foaming");
      let max_allowed_flow = if is_foaming { 1.02 } else { 1.05 };
      let min_allowed_flow = if is_foaming { 0.45 } else { 0.88 };
      if !(min_allowed_flow..=max_allowed_flow).contains(&flow) {
        cfg_failures.push(format!(
          "[{}] Filament '{}' has invalid flow_ratio={} (expected {}..={})",
          cfg_name, name, flow, min_allowed_flow, max_allowed_flow
        ));
      }
      if !(1.5..=26.0).contains(&mvs) {
        cfg_failures.push(format!(
          "[{}] Filament '{}' has invalid max_volumetric_speed={} (expected 1.5..=26.0 mm³/s)",
          cfg_name, name, mvs
        ));
      }
      // Flexible-family speed cap and the nozzle-temp floor relaxation both come from
      // constants.json `hardware_limits` (same single source the resolver and the Lua
      // `ctx.filament_is_elastomer` flag use).
      let is_tpu_family =
        orcaslicer_webconfig::constants::constants().is_elastomer_family_material(mat_for_bounds);
      let max_ret_spd = if is_tpu_family {
        hw_limits.elastomer_max_retraction_speed_mms as f32
      } else {
        hw_limits.max_retraction_speed_mms
      };
      if !((hw_limits.min_retraction_speed_mms)..=max_ret_spd).contains(&ret_spd) {
        cfg_failures.push(format!(
          "[{}] Filament '{}' ({}) has invalid retraction_speed={} (expected {}..={} mm/s)",
          cfg_name, name, ftype, ret_spd, hw_limits.min_retraction_speed_mms, max_ret_spd
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
      if canon == "PLA-CF"
        || name.to_uppercase().contains("PLA-CF")
        || name.to_uppercase().contains("PLA-GF")
      {
        if let Some(aux_v) = first_str_val(&val, "additional_cooling_fan_speed")
          .and_then(|s| s.trim_end_matches('%').parse::<f64>().ok())
        {
          if aux_v > 0.0 {
            cfg_failures.push(format!(
              "[{}] Fiber-reinforced PLA '{}' has additional_cooling_fan_speed={}% > 0% (M106 P2 side blower chills 65°C PEI plate and causes spaghetti detachment)",
              cfg_name, name, aux_v
            ));
          }
        }
        if let Some(nfl_v) = first_str_val(&val, "close_fan_the_first_x_layers")
          .and_then(|s| s.parse::<i32>().ok())
        {
          if nfl_v < 2 {
            cfg_failures.push(format!(
              "[{}] Fiber-reinforced PLA '{}' has close_fan_the_first_x_layers={} < 2",
              cfg_name, name, nfl_v
            ));
          }
        }
      }
      if matches!(canon, "ABS" | "ASA" | "ASA-CF" | "PC" | "PA" | "PA-CF" | "PPA-CF" | "PET-CF" | "HIPS") {
        if let Some(fv) = first_str_val(&val, "additional_cooling_fan_speed")
          .and_then(|s| s.parse::<f64>().ok())
        {
          if fv > 0.0 {
            cfg_failures.push(format!(
              "[{}] Warp-prone filament '{}' ({}) has additional_cooling_fan_speed={}% > 0% (auxiliary cooling warps ABS/PC/PA)",
              cfg_name, name, canon, fv
            ));
          }
        }
        if let Some(ovh) = first_str_val(&val, "overhang_fan_speed")
          .and_then(|s| s.trim_end_matches('%').parse::<f64>().ok())
        {
          if ovh > 35.0 {
            cfg_failures.push(format!(
              "[{}] Warp-prone filament '{}' ({}) has overhang_fan_speed={}% > 35%",
              cfg_name, name, canon, ovh
            ));
          }
        }
        if let Some(ex_dur) = first_str_val(&val, "during_print_exhaust_fan_speed")
          .and_then(|s| s.trim_end_matches('%').parse::<f64>().ok())
        {
          if ex_dur > 15.0 {
            cfg_failures.push(format!(
              "[{}] Warp-prone filament '{}' ({}) has during_print_exhaust_fan_speed={}% > 15% (evacuates chamber heat during ABS/ASA/PC/PA print)",
              cfg_name, name, canon, ex_dur
            ));
          }
        }
      }
      if rig.base_retraction_length >= 2.0
        && !name.to_lowercase().contains("air")
        && !snap
          .rules
          .iter()
          .any(|r| r.enabled && r.id.contains("cal_retraction_tower"))
      {
        if let Some(fret) =
          first_str_val(&val, "filament_retraction_length").and_then(|s| s.parse::<f64>().ok())
        {
          if fret < rig.base_retraction_length * 0.6 {
            cfg_failures.push(format!(
              "[{}] Filament '{}' on Bowden '{}' (machine retraction {:.2}mm) was clobbered to direct-drive retraction_length={:.2}mm",
              cfg_name, name, rig.printer_model, rig.base_retraction_length, fret
            ));
          }
        }
      }
      if !matches!(canon, "PETG" | "PETG-CF" | "PCTG") {
        if let Some(ze) = first_str_val(&val, "filament_retract_lift_enforce") {
          if ze == "Top Only" {
            cfg_failures.push(format!(
              "[{}] Non-PETG filament '{}' ({}) has leaked filament_retract_lift_enforce='Top Only'",
              cfg_name, name, canon
            ));
          }
        }
      }
      if name.contains("PETG-CF") || name.contains("PETG-GF") || name.contains("PETG-rCF") {
        // 235°C fiber-flow floor minus the approved anti-stringing band trim (same
        // `nozzle_temp_floor_relax_c` policy as the nozzle-temp floor above).
        let petg_cf_floor = 235 - hw_limits.nozzle_temp_floor_relax_c;
        if noz_t < petg_cf_floor || ftype != "PETG-CF" {
          cfg_failures.push(format!(
            "[{}] Filament '{}' must have nozzle_temperature >= {}°C and filament_type='PETG-CF' (got {}°C, type='{}')",
            cfg_name, name, petg_cf_floor, noz_t, ftype
          ));
        }
      }
      if matches!(canon, "PVA" | "BVOH") {
        if noz_t > 220 || range_high > 235 {
          cfg_failures.push(format!(
            "[{}] Water-soluble support '{}' ({}) risks thermal charring: nozzle_temperature={}°C (expected <= 220°C), range_high={}°C (expected <= 235°C)",
            cfg_name, name, canon, noz_t, range_high
          ));
        }
        if first_str_val(&val, "dont_slow_down_outer_wall").as_deref() == Some("1") {
          cfg_failures.push(format!(
            "[{}] Water-soluble support '{}' ({}) locked dont_slow_down_outer_wall='1'",
            cfg_name, name, canon
          ));
        }
      }
      if canon == "PPA-CF"
        && machine_max_c >= 300
        && rig.nozzle_type.contains("hardened")
        && !snap.rules.iter().any(|r| {
          r.enabled
            && (r.id.contains("cal_temp_tower")
              || r.id.contains("transparent_glass")
              || r.id.contains("wet_filament"))
        })
      {
        if noz_t < 292 {
          cfg_failures.push(format!(
            "[{}] Exotic high-temp '{}' (PPA-CF) on >=300°C Hardened Steel '{}' was under-heated at {}°C (expected >= 292°C)",
            cfg_name, name, rig.printer_model, noz_t
          ));
        }
      }
      if fin_for_bounds.eq_ignore_ascii_case("Foaming")
        && !snap
          .rules
          .iter()
          .any(|r| r.enabled && r.id.contains("rig_addons_cooling"))
      {
        if let Some(fv) = first_str_val(&val, "additional_cooling_fan_speed")
          .and_then(|s| s.trim_end_matches('%').parse::<f64>().ok())
        {
          if fv > 0.0 {
            cfg_failures.push(format!(
              "[{}] Active-foaming filament '{}' has additional_cooling_fan_speed={}% > 0% (quenches Varioshore/LW cell expansion)",
              cfg_name, name, fv
            ));
          }
        }
      }
      if matches!(
        canon,
        "PLA"
          | "PLA+"
          | "PLA-CF"
          | "PHA"
          | "TPU"
          | "TPU for AMS"
          | "TPU 95A"
          | "PETG"
          | "PETG-CF"
          | "PCTG"
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

    // 3. Audit all 5 process JSON files in user/default/process/ against rig-tailored presets
    // and verify `process.inherits` is a real instantiated preset for `rig.printer_model`!
    let expected_rig_procs = ProcessProfile::get_default_processes_for_rig(rig);
    let rig_cm_opt = catalog.find_machine(&rig.printer_model);
    for exp_proc in &expected_rig_procs {
      total_processes_audited += 1;
      let pfile = format!("{}.json", exp_proc.name);
      let ppath = cfg_dir.join("user/default/process").join(&pfile);
      let ptxt = fs::read_to_string(&ppath)
        .unwrap_or_else(|e| panic!("Missing process JSON file {}: {}", ppath.display(), e));
      let pval: Value = serde_json::from_str(&ptxt).expect("Invalid process JSON");
      GroundTruthSlicerSchema::get().validate_preset_json(
        "process",
        cfg_name,
        &pval,
        &mut cfg_failures,
      );
      let proc_inh = first_str_val(&pval, "inherits").unwrap_or_default();
      if rig.printer_model != "Anycubic Kobra S1"
        && !proc_inh.contains(&rig.printer_model)
        && proc_inh.contains("@Anycubic Kobra S1")
      {
        cfg_failures.push(format!(
          "[{}] Process '{}' on '{}' leaked Kobra S1 inherits='{}'",
          cfg_name, pfile, rig.printer_model, proc_inh
        ));
      }
      if let Some(cm) = rig_cm_opt {
        if !cm.process_presets.is_empty()
          && !cm.process_presets.contains(&proc_inh)
          && proc_inh != cm.default_print_profile
        {
          cfg_failures.push(format!(
            "[{}] Process '{}' on '{}' has non-instantiated inherits='{}' (expected one of {:?})",
            cfg_name, pfile, rig.printer_model, proc_inh, cm.process_presets
          ));
        }
      }
      let exp_lh = format!("{:.2}", exp_proc.layer_height);
      let ow_num = first_str_val(&pval, "outer_wall_speed")
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(-1.0);
      let iw_num = first_str_val(&pval, "inner_wall_speed")
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(-1.0);
      let si_num = first_str_val(&pval, "sparse_infill_speed")
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(-1.0);
      let il_num = first_str_val(&pval, "initial_layer_speed")
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(-1.0);
      let acc_num = first_str_val(&pval, "default_acceleration")
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(-1.0);
      if first_str_val(&pval, "layer_height").as_deref() != Some(exp_lh.as_str())
        || !(15.0..=400.0).contains(&ow_num)
        || !(20.0..=450.0).contains(&iw_num)
        || !(20.0..=450.0).contains(&si_num)
        || !(10.0..=80.0).contains(&il_num)
        || !(300.0..=20000.0).contains(&acc_num)
        || (rig.printer_model == "Creality Ender-3" && (ow_num > 120.0 || acc_num > 2500.0))
      {
        cfg_failures.push(format!(
          "[{}] Process '{}' on '{}' has invalid speeds/accels: lh={:?} ow={} iw={} si={} il={} acc={}",
          cfg_name,
          pfile,
          rig.printer_model,
          first_str_val(&pval, "layer_height"),
          ow_num,
          iw_num,
          si_num,
          il_num,
          acc_num,
        ));
      }
      if pval.get("internal_solid_infill_speed").is_some() {
        cfg_failures.push(format!(
          "[{}] Process '{}' leaked internal_solid_infill_speed",
          cfg_name, pfile
        ));
      }
      // Process `notes` must describe the exact exported speeds / accel / strategy
      let pnotes = pval.get("notes").and_then(|v| v.as_str()).unwrap_or("");
      let num_of = |k: &str| -> f64 {
        first_str_val(&pval, k)
          .and_then(|s| s.parse::<f64>().ok())
          .unwrap_or(-1.0)
      };
      let mut exp_note_parts = vec![
        format!("Layer Height: {:.2}mm", num_of("layer_height")),
        format!("{:.0} mm/s Outer Wall", num_of("outer_wall_speed")),
        format!("{:.0} mm/s Inner Wall", num_of("inner_wall_speed")),
        format!("{:.0} mm/s Sparse Infill", num_of("sparse_infill_speed")),
        format!("{:.0} mm/s Initial Layer", num_of("initial_layer_speed")),
        format!("Default Accel: {:.0} mm/s²", num_of("default_acceleration")),
      ];
      if let Some(lw) = first_str_val(&pval, "line_width").and_then(|s| s.parse::<f64>().ok()) {
        exp_note_parts.push(format!("Line Width: {:.2}mm", lw));
      }
      for k in ["wall_generator", "print_sequence"] {
        if let Some(v) = first_str_val(&pval, k) {
          exp_note_parts.push(format!("'{}'", v));
        }
      }
      for part in &exp_note_parts {
        if !pnotes.contains(part.as_str()) {
          cfg_failures.push(format!(
            "[{}] Process '{}' notes do not match exported value '{}' (notes='{}')",
            cfg_name, pfile, part, pnotes
          ));
        }
      }
      // Verify Item #5 (line_width_geometry default-on) and Item #6 (outer_wall_consistency default-on)
      for lw_key in [
        "outer_wall_line_width",
        "inner_wall_line_width",
        "sparse_infill_line_width",
        "top_surface_line_width",
        "initial_layer_line_width",
      ] {
        let lw_v = first_str_val(&pval, lw_key).and_then(|s| s.parse::<f64>().ok());
        if !lw_v.map(|x| (0.15..=1.35).contains(&x)).unwrap_or(false) {
          cfg_failures.push(format!(
            "[{}] Process '{}' has missing or out-of-range {}={:?}",
            cfg_name,
            pfile,
            lw_key,
            first_str_val(&pval, lw_key)
          ));
        }
      }
      if pval.get("dont_slow_down_outer_wall").is_some() {
        cfg_failures.push(format!(
          "[{}] Process '{}' illegally contains Filament-scoped key dont_slow_down_outer_wall",
          cfg_name, pfile
        ));
      }
      for ov_k in [
        "overhang_2_4_speed",
        "overhang_3_4_speed",
        "overhang_4_4_speed",
      ] {
        if first_str_val(&pval, ov_k).as_deref() == Some("0") {
          cfg_failures.push(format!(
            "[{}] Process '{}' wiped {} to '0' (disabling overhang slowdown in OrcaSlicer C++)",
            cfg_name, pfile, ov_k
          ));
        }
      }
      let sm_raw = first_str_val(&pval, "small_perimeter_speed").unwrap_or_default();
      let sm_valid = if let Some(pct_s) = sm_raw.trim().strip_suffix('%') {
        pct_s
          .trim()
          .parse::<f64>()
          .map(|p| (30.0..=85.0).contains(&p))
          .unwrap_or(false)
      } else {
        sm_raw
          .trim()
          .parse::<f64>()
          .map(|x| (15.0..=75.0).contains(&x))
          .unwrap_or(false)
      };
      if !sm_valid {
        cfg_failures.push(format!(
          "[{}] Process '{}' has invalid small_perimeter_speed={:?} (expected 15..=75 mm/s or 30..=85%)",
          cfg_name, pfile, sm_raw
        ));
      }
      let rig_noz_d = rig
        .formatted_nozzle_diameter()
        .parse::<f64>()
        .unwrap_or(0.4);
      if let Some(ts_lw) =
        first_str_val(&pval, "top_surface_line_width").and_then(|s| s.parse::<f64>().ok())
      {
        if ts_lw + 1e-4 < rig_noz_d {
          cfg_failures.push(format!(
            "[{}] Process '{}' has under-width top_surface_line_width={} < nozzle_diameter={}mm",
            cfg_name, pfile, ts_lw, rig_noz_d
          ));
        }
      }
      let has_layer_shift_rule = snap
        .rules
        .iter()
        .any(|r| r.enabled && r.id.contains("sym_layer_shifting"));
      if let Some(trav_acc) =
        first_str_val(&pval, "travel_acceleration").and_then(|s| s.parse::<f64>().ok())
      {
        let min_trav_acc = if rig.is_bedslinger() || has_layer_shift_rule {
          500.0
        } else {
          acc_num.min(9000.0)
        };
        if trav_acc + 1e-4 < min_trav_acc {
          cfg_failures.push(format!(
            "[{}] Process '{}' has travel_acceleration={} < minimum {} (default_acceleration={})",
            cfg_name, pfile, trav_acc, min_trav_acc, acc_num
          ));
        }
      }
      if first_str_val(&pval, "sparse_infill_density").as_deref() == Some("100%")
        && first_str_val(&pval, "sparse_infill_pattern").as_deref() == Some("alignedrectilinear")
        && first_str_val(&pval, "internal_solid_infill_pattern").as_deref()
          != Some("alignedrectilinear")
      {
        cfg_failures.push(format!(
          "[{}] Optical Glass 100% infill process '{}' MUST set internal_solid_infill_pattern='alignedrectilinear' (OrcaSlicer converts 100% sparse infill to internal_solid_infill_pattern!)",
          cfg_name, pfile
        ));
      }
      if let Some(ov1) = first_str_val(&pval, "overhang_1_4_speed")
        .filter(|s| !s.trim().ends_with('%'))
        .and_then(|s| s.parse::<f64>().ok())
      {
        if ov1 > ow_num + 1e-6 {
          cfg_failures.push(format!(
            "[{}] Process '{}' has overhang_1_4_speed={} > outer_wall_speed={}",
            cfg_name, pfile, ov1, ow_num
          ));
        }
      }
      if let Some(br_spd) = first_str_val(&pval, "bridge_speed")
        .filter(|s| !s.trim().ends_with('%'))
        .and_then(|s| s.parse::<f64>().ok())
      {
        if br_spd > ow_num * 1.25 + 1e-6 {
          cfg_failures.push(format!(
            "[{}] Process '{}' has bridge_speed={} > 1.25 * outer_wall_speed={}",
            cfg_name, pfile, br_spd, ow_num
          ));
        }
      }
      if let Some(ws) = first_str_val(&pval, "wall_sequence") {
        if !matches!(
          ws.as_str(),
          "inner wall/outer wall" | "outer wall/inner wall" | "inner-outer-inner wall"
        ) {
          cfg_failures.push(format!(
            "[{}] Process '{}' has invalid OrcaSlicer wall_sequence='{}' (expected 'inner wall/outer wall', 'outer wall/inner wall', or 'inner-outer-inner wall')",
            cfg_name, pfile, ws
          ));
        }
      }
      let ironing_rule = snap
        .rules
        .iter()
        .find(|r| r.id == "ironing_tuning" && r.enabled);
      if ironing_rule.is_none() && pval.get("ironing_type").is_some() {
        cfg_failures.push(format!(
          "[{}] Process '{}' leaked untouched ironing_type when ironing_tuning is disabled",
          cfg_name, pfile
        ));
      }
      if let Some(ir) = ironing_rule {
        if !ir.scope.process_tiers.is_empty()
          && pfile.contains("0.28mm")
          && !ir
            .scope
            .process_tiers
            .iter()
            .any(|t| t.contains("draft") || t.contains("0.28"))
          && pval.get("ironing_type").is_some()
        {
          cfg_failures.push(format!(
            "[{}] Process '{}' leaked ironing_type despite process_tiers scope {:?}",
            cfg_name, pfile, ir.scope.process_tiers
          ));
        }
      }
      let support_enabled = snap
        .rules
        .iter()
        .any(|r| r.id == "support_tuning" && r.enabled);
      if !support_enabled && pval.get("support_style").is_some() {
        cfg_failures.push(format!(
          "[{}] Process '{}' leaked untouched support_style when support_tuning is disabled",
          cfg_name, pfile
        ));
      }
      let dim_enabled = snap
        .rules
        .iter()
        .any(|r| r.id == "dimensional_precision" && r.enabled);
      if !dim_enabled && pval.get("xy_hole_compensation").is_some() {
        cfg_failures.push(format!(
          "[{}] Process '{}' leaked untouched xy_hole_compensation when dimensional_precision is disabled",
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

    if proj_val.get("curr_bed_type").and_then(|v| v.as_str()) != Some("Textured PEI Plate") {
      cfg_failures.push(format!(
        "[{}] project_settings.config must set curr_bed_type='Textured PEI Plate', got {:?}",
        cfg_name,
        proj_val.get("curr_bed_type")
      ));
    }
    if let Some(cm) = catalog.find_machine(&rig.printer_model) {
      if !cm.printable_area.is_empty() {
        let proj_area: Vec<String> = proj_val
          .get("printable_area")
          .and_then(|v| v.as_array())
          .map(|arr| {
            arr
              .iter()
              .filter_map(|x| x.as_str().map(|s| s.to_string()))
              .collect()
          })
          .unwrap_or_default();
        if proj_area != cm.printable_area {
          cfg_failures.push(format!(
            "[{}] project_settings.config printable_area on '{}' is {:?}, expected {:?}",
            cfg_name, rig.printer_model, proj_area, cm.printable_area
          ));
        }
      }
      if !cm.printable_height.is_empty()
        && proj_val.get("printable_height").and_then(|v| v.as_str())
          != Some(cm.printable_height.as_str())
      {
        cfg_failures.push(format!(
          "[{}] project_settings.config printable_height on '{}' is {:?}, expected {:?}",
          cfg_name,
          rig.printer_model,
          proj_val.get("printable_height"),
          cm.printable_height
        ));
      }
    }

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
            if orcaslicer_webconfig::threemf::is_filament_scoped_key(k) {
              cfg_failures.push(format!(
                "[{}] Process '{}' leaked filament-scoped key '{}'={:?}!",
                cfg_name, proc_id, k, v
              ));
            }
            if orcaslicer_webconfig::threemf::is_machine_scoped_key(k) {
              cfg_failures.push(format!(
                "[{}] Process '{}' leaked machine-scoped key '{}'={:?}!",
                cfg_name, proc_id, k, v
              ));
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
        GroundTruthSlicerSchema::get().validate_preset_json(
          "machine",
          cfg_name,
          &m_val,
          &mut cfg_failures,
        );
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
                | "printer_timestamp"
                | "compatible_printers"
                | "version"
            ) {
              continue;
            }
            if orcaslicer_webconfig::threemf::is_filament_scoped_key(k) {
              cfg_failures.push(format!(
                "[{}] Machine '{}' leaked filament-scoped key '{}'={:?}!",
                cfg_name, mach_id, k, v
              ));
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

    // 4b. Exhaustive AnycubicSlicerNext C++ Enum Validator (verified against AnycubicSlicer.dll):
    let check_enum_value = |failures: &mut Vec<String>, ctx_label: &str, key: &str, val: &str| {
      let valid_opts: Option<&[&str]> = match key {
        "z_hop_types" | "filament_z_hop_types" => Some(&[
          "Auto Lift",
          "Normal Lift",
          "Slope Lift",
          "Spiral Lift",
          "nil",
        ]),
        "retract_lift_enforce" | "filament_retract_lift_enforce" => Some(&[
          "All Surfaces",
          "Top Only",
          "Bottom Only",
          "Top and Bottom",
          "nil",
        ]),
        "wall_generator" => Some(&["classic", "arachne"]),
        "wall_sequence" => Some(&[
          "inner wall/outer wall",
          "outer wall/inner wall",
          "inner-outer-inner wall",
          "inner wall/outer wall/infill",
          "outer wall/inner wall/infill",
          "inner-outer-inner wall/infill",
          "infill/inner wall/outer wall",
          "infill/outer wall/inner wall",
        ]),
        "ironing_type" => Some(&["no ironing", "top", "topmost", "solid"]),
        "brim_type" => Some(&[
          "no_brim",
          "outer_only",
          "inner_only",
          "outer_and_inner",
          "auto_brim",
          "brim_ears",
          "painted",
        ]),
        "support_type" => Some(&[
          "normal(auto)",
          "tree(auto)",
          "normal(manual)",
          "tree(manual)",
          "hybrid(auto)",
        ]),
        "support_style" => Some(&[
          "default",
          "grid",
          "snug",
          "organic",
          "tree_slim",
          "tree_strong",
          "tree_hybrid",
        ]),
        "seam_position" => Some(&["nearest", "aligned", "aligned_back", "back", "random"]),
        "seam_slope_type" => Some(&["none", "external", "all"]),
        "print_sequence" => Some(&["by layer", "by object"]),
        "sparse_infill_pattern" => Some(&[
          "rectilinear",
          "alignedrectilinear",
          "grid",
          "triangles",
          "tri-hexagon",
          "cubic",
          "adaptivecubic",
          "quartercubic",
          "line",
          "concentric",
          "zig-zag",
          "zigzag",
          "crosszag",
          "lockedzag",
          "honeycomb",
          "3dhoneycomb",
          "gyroid",
          "hilbertcurve",
          "archimedeanchords",
          "octagramspiral",
          "supportcubic",
          "lightning",
          "crosshatch",
          "tpmsd",
          "tpmsfk",
          "lateral-lattice",
          "lateral-honeycomb",
        ]),
        "top_surface_pattern" | "bottom_surface_pattern" => Some(&[
          "monotonic",
          "monotonicline",
          "rectilinear",
          "alignedrectilinear",
          "concentric",
          "zig-zag",
          "zigzag",
          "hilbertcurve",
          "archimedeanchords",
          "octagramspiral",
        ]),
        _ => None,
      };
      if let Some(allowed) = valid_opts {
        if !allowed.contains(&val) {
          failures.push(format!(
            "[{}] {} has invalid AnycubicSlicerNext C++ enum value for '{}': {:?} (allowed: {:?})",
            cfg_name, ctx_label, key, val, allowed
          ));
        }
      }
    };
    if let Some(proj_obj) = proj_val.as_object() {
      for (k, v) in proj_obj {
        match v {
          Value::String(s) => check_enum_value(&mut cfg_failures, "project_settings.config", k, s),
          Value::Array(arr) => {
            for elem in arr {
              if let Some(s) = elem.as_str() {
                check_enum_value(&mut cfg_failures, "project_settings.config", k, s);
              }
            }
          }
          _ => {}
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

    // 5. Re-import the directory via `import_scanned_directory` and verify printer_model, bed_size, and filaments
    let mut scanned = Vec::new();
    collect_scanned_entries(cfg_dir, cfg_dir, &mut scanned);
    let imported = import_scanned_directory(&scanned).expect("import_scanned_directory failed");
    let imp_active_mach = imported
      .machines
      .iter()
      .find(|m| m.is_active)
      .or_else(|| imported.machines.first());
    if imp_active_mach.map(|m| m.printer_model.as_str()) != Some(rig.printer_model.as_str()) {
      cfg_failures.push(format!(
        "[{}] import_scanned_directory corrupted active printer_model: expected '{}', got {:?}",
        cfg_name,
        rig.printer_model,
        imp_active_mach.map(|m| &m.printer_model)
      ));
    }
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
          for (s_idx, (orig_sp, tmf_sp)) in snap.filaments.iter().zip(tmf.spools.iter()).enumerate()
          {
            let tmf_inh_valid = catalog.instantiated_system_filaments.contains(&tmf_sp.inherits)
              || catalog
                .find_machine(&rig.printer_model)
                .map(|cm| cm.filament_presets.values().any(|p| p == &tmf_sp.inherits))
                .unwrap_or(false);
            if tmf_sp.brand == "OC"
              || tmf_sp.name != orig_sp.name
              || tmf_sp.brand != orig_sp.brand
              || FilamentSpool::canonical_polymer_key(&tmf_sp.material_type)
                != FilamentSpool::canonical_polymer_key(&orig_sp.material_type)
              || tmf_sp.required_nozzle_hrc != orig_sp.required_nozzle_hrc
              || !tmf_inh_valid
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

  // Overwrite census summary: how often later rules clobbered earlier rules' values, split by
  // intent-lost (OVERWRITTEN) vs re-derived (recomputed).
  let mut census_counts: std::collections::BTreeMap<(String, String, String, String), usize> =
    std::collections::BTreeMap::new();
  for (kind, param, clobbered, clobberer) in &shadow_census {
    *census_counts
      .entry((
        kind.to_string(),
        param.clone(),
        clobbered.clone(),
        clobberer.clone(),
      ))
      .or_insert(0) += 1;
  }
  let overwritten_total = shadow_census
    .iter()
    .filter(|(k, ..)| *k == "OVERWRITTEN")
    .count();
  let recomputed_total = shadow_census
    .iter()
    .filter(|(k, ..)| *k == "recomputed")
    .count();
  let accepted_total = shadow_census
    .iter()
    .filter(|(k, ..)| *k == "accepted")
    .count();
  let clamped_total = shadow_census
    .iter()
    .filter(|(k, ..)| *k == "clamped")
    .count();
  println!("------------------------------------------------------------------");
  println!(
    "Overwrite census: {} intent-lost OVERWRITTEN + {} recomputed + {} accepted + {} clamped step(s) across {} configs",
    overwritten_total,
    recomputed_total,
    accepted_total,
    clamped_total,
    config_dirs.len()
  );
  let mut census_rows: Vec<(&(String, String, String, String), &usize)> =
    census_counts.iter().collect();
  census_rows.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(&b.0)));
  for ((kind, param, clobbered, clobberer), count) in census_rows.iter().take(40) {
    let verb = match kind.as_str() {
      "accepted" => "accepted by",
      "clamped" => "clamped by",
      _ => "clobbered by",
    };
    println!(
      "  {:>3}x  [{}] {} : {} {} {}",
      count, kind, param, clobbered, verb, clobberer
    );
  }

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
    if cfg!(test) {
      panic!(
        "AUDIT FAILED with {} issue(s):\n{}",
        total_failures.len(),
        total_failures.join("\n")
      );
    } else {
      std::process::exit(1);
    }
  }
  println!(
    "ALL {} RANDOM CONFIGURATIONS PASSED 100% OF HARDWARE, TEMPERATURE & SPEED CHECKS!",
    config_dirs.len()
  );
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
      "--dump-inspection-pack" => {
        if i + 1 < args.len() {
          let pack_dir = PathBuf::from(&args[i + 1]);
          dump_inspection_pack(&pack_dir);
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

fn fmt_json_compact(v: Option<&Value>) -> String {
  match v {
    None => "*(unset)*".to_string(),
    Some(Value::String(s)) => format!("`\"{}\"`", s.replace('\n', "\\n")),
    Some(Value::Array(arr)) => {
      if arr.len() == 1 {
        if let Some(s) = arr[0].as_str() {
          return format!("`[\"{}\"]`", s.replace('\n', "\\n"));
        }
      }
      let joined: Vec<String> = arr
        .iter()
        .take(4)
        .map(|x| x.to_string().replace('\n', "\\n"))
        .collect();
      if arr.len() > 4 {
        format!("`[{}, … ({})]`", joined.join(", "), arr.len())
      } else {
        format!("`[{}]`", joined.join(", "))
      }
    }
    Some(other) => format!("`{}`", other),
  }
}

fn write_side_by_side_diff_section(
  md: &mut Vec<String>,
  gt: &GroundTruthSlicerSchema,
  ptype: &str,
  rel_path: &str,
  exported: &Value,
) {
  let Some(exp_obj) = exported.as_object() else {
    return;
  };
  let name = first_str_val(exported, "name").unwrap_or_else(|| rel_path.to_string());
  let inh = first_str_val(exported, "inherits").unwrap_or_default();
  let parent_map = gt
    .resolved_presets
    .get(ptype)
    .and_then(|m| m.get(&inh));

  md.push(format!("## `{}` (`{}`)", name, rel_path));
  md.push(format!("- **Preset Type**: `{}` | **Inherits**: `{}`", ptype, inh));
  if let Some(notes) = first_str_val(exported, "notes")
    .or_else(|| first_str_val(exported, "filament_notes"))
    .or_else(|| first_str_val(exported, "printer_notes"))
  {
    md.push(format!("- **Notes**: {}", notes));
  }
  md.push(String::new());
  md.push("| Key | Stock Parent (`inherits`) | Exported Child JSON | Effective Merged | C++ Type & Tooltip |".to_string());
  md.push("| :--- | :--- | :--- | :--- | :--- |".to_string());

  let important_unoverridden_keys: &[&str] = &[
    "nozzle_temperature",
    "nozzle_temperature_initial_layer",
    "nozzle_temperature_range_low",
    "nozzle_temperature_range_high",
    "nozzle_temperature_range_low_HS",
    "nozzle_temperature_range_high_HS",
    "textured_plate_temp",
    "textured_plate_temp_initial_layer",
    "hot_plate_temp",
    "hot_plate_temp_initial_layer",
    "cool_plate_temp",
    "textured_cool_plate_temp",
    "eng_plate_temp",
    "supertack_plate_temp",
    "temperature_vitrification",
    "fan_min_speed",
    "fan_max_speed",
    "overhang_fan_speed",
    "overhang_fan_threshold",
    "additional_cooling_fan_speed",
    "activate_air_filtration",
    "during_print_exhaust_fan_speed",
    "complete_print_exhaust_fan_speed",
    "close_fan_the_first_x_layers",
    "dont_slow_down_outer_wall",
    "slow_down_layer_time",
    "filament_flow_ratio",
    "filament_max_volumetric_speed",
    "enable_pressure_advance",
    "pressure_advance",
    "filament_retraction_length",
    "filament_retraction_speed",
    "filament_deretraction_speed",
    "filament_retract_before_wipe",
    "filament_wipe",
    "filament_wipe_distance",
    "filament_z_hop",
    "filament_z_hop_types",
    "filament_retract_lift_enforce",
    "outer_wall_speed",
    "inner_wall_speed",
    "sparse_infill_speed",
    "initial_layer_speed",
    "initial_layer_infill_speed",
    "small_perimeter_speed",
    "bridge_speed",
    "bridge_flow",
    "overhang_1_4_speed",
    "overhang_2_4_speed",
    "overhang_3_4_speed",
    "overhang_4_4_speed",
    "seam_slope_type",
    "seam_slope_inner_walls",
    "seam_slope_start_height",
    "seam_slope_min_length",
    "wall_generator",
    "wall_sequence",
    "top_surface_pattern",
    "bottom_surface_pattern",
    "internal_solid_infill_pattern",
    "brim_type",
    "brim_width",
    "initial_layer_print_height",
    "initial_layer_line_width",
    "elefant_foot_compensation",
  ];

  let mut all_keys: BTreeSet<String> = exp_obj.keys().cloned().collect();
  if let Some(pm) = parent_map {
    for &ik in important_unoverridden_keys {
      if pm.contains_key(ik) {
        all_keys.insert(ik.to_string());
      }
    }
  }

  for k in all_keys {
    if matches!(
      k.as_str(),
      "name"
        | "from"
        | "inherits"
        | "version"
        | "type"
        | "is_custom_defined"
        | "filament_settings_id"
        | "print_settings_id"
        | "printer_settings_id"
        | "filament_notes"
        | "notes"
        | "printer_notes"
        | "filament_timestamp"
        | "print_timestamp"
        | "printer_timestamp"
    ) {
      continue;
    }
    let p_val = parent_map.and_then(|pm| pm.get(&k));
    let e_val = exp_obj.get(&k);
    let eff_val = e_val.or(p_val);
    let opt_desc = if let Some(opt) = gt.options.get(&k) {
      let short_tip: String = opt.tooltip.chars().take(110).collect();
      if short_tip.is_empty() {
        format!("`{}`", opt.co_type)
      } else {
        format!("`{}` — {}", opt.co_type, short_tip)
      }
    } else {
      "*(fork/meta)*".to_string()
    };
    md.push(format!(
      "| `{}` | {} | {} | {} | {} |",
      k,
      fmt_json_compact(p_val),
      fmt_json_compact(e_val),
      fmt_json_compact(eff_val),
      opt_desc
    ));
  }
  md.push(String::new());
}

fn dump_inspection_pack(pack_root: &Path) {
  let _ = fs::remove_dir_all(pack_root);
  fs::create_dir_all(pack_root).expect("Failed to create inspection pack dir");
  let gt = GroundTruthSlicerSchema::get();
  let catalog = load_merged_catalog();
  let templates = TransformationEngine::get_parameterized_calibration_templates();

  struct PackScenario {
    slug: &'static str,
    title: &'static str,
    printer_model: &'static str,
    nozzle_type: &'static str,
    nozzle_diameter: f32,
    addons: Vec<&'static str>,
    climate_preset: &'static str,
    ambient_temp_c: i32,
    humidity_pct: i32,
    wet_rh: Option<i32>,
    extra_template_ids: Vec<(&'static str, Vec<(&'static str, &'static str)>)>,
    spool_specs: Vec<(&'static str, &'static str)>,
  }

  let scenarios = vec![
    PackScenario {
      slug: "01_kobra_s1_diamondback_tropical_wet",
      title: "Anycubic Kobra S1 • 0.4mm Diamondback + FlowTech • Tropical (30°C, 80% RH) + Wet Filament (65% RH)",
      printer_model: "Anycubic Kobra S1",
      nozzle_type: "diamondback",
      nozzle_diameter: 0.4,
      addons: vec!["flowtech_hotend"],
      climate_preset: "tropical",
      ambient_temp_c: 30,
      humidity_pct: 80,
      wet_rh: Some(65),
      extra_template_ids: vec![("anycubic_pctg_as_petg", vec![])],
      spool_specs: vec![
        ("Anycubic", "PLA"),
        ("Anycubic", "PLA+"),
        ("Anycubic", "PLA Silk"),
        ("Anycubic", "PLA-CF"),
        ("Generic", "PHA"),
        ("Anycubic", "PETG"),
        ("Generic", "PETG Translucent"),
        ("Anycubic", "PETG-CF"),
        ("SUNLU", "PETG-CF"),
        ("Generic", "PCTG"),
        ("Anycubic", "TPU"),
        ("Anycubic", "TPU 95A"),
        ("Anycubic", "ABS"),
        ("Anycubic", "ASA"),
        ("Generic", "PC-CF"),
        ("Anycubic", "PA6-CF"),
      ],
    },
    PackScenario {
      slug: "02_kobra_s1_hardened_cold_humid_structural",
      title: "Anycubic Kobra S1 • 0.4mm Hardened Steel • Cold & Humid (15°C, 80% RH) + Structural Strength Pack",
      printer_model: "Anycubic Kobra S1",
      nozzle_type: "orca_hardened_steel",
      nozzle_diameter: 0.4,
      addons: vec![],
      climate_preset: "cold_humid",
      ambient_temp_c: 15,
      humidity_pct: 80,
      wet_rh: None,
      extra_template_ids: vec![
        ("structural_strength", vec![("strength_preset", "load_bearing")]),
        ("anycubic_pctg_as_petg", vec![]),
      ],
      spool_specs: vec![
        ("Anycubic", "PLA"),
        ("Anycubic", "PLA-CF"),
        ("Anycubic", "PETG"),
        ("Anycubic", "ABS"),
        ("Anycubic", "PA6-CF"),
        ("Anycubic", "TPU 95A"),
      ],
    },
    PackScenario {
      slug: "03_bambu_x1c_brass_temperate_glass",
      title: "Bambu Lab X1 Carbon • 0.4mm Brass • Temperate (22°C, 45% RH) + Optical Glass Mode",
      printer_model: "Bambu Lab X1 Carbon",
      nozzle_type: "orca_brass",
      nozzle_diameter: 0.4,
      addons: vec![],
      climate_preset: "temperate",
      ambient_temp_c: 22,
      humidity_pct: 45,
      wet_rh: None,
      extra_template_ids: vec![("sym_transparent_glass", vec![])],
      spool_specs: vec![
        ("Generic", "PETG Translucent"),
        ("Bambu Lab", "PLA"),
        ("Generic", "PC"),
        ("Generic", "ASA"),
      ],
    },
    PackScenario {
      slug: "04_ender3_bowden_cht_06_hot_dry",
      title: "Creality Ender-3 (Bowden 4.0mm, 260°C/100°C Ceiling) • 0.6mm CHT • Hot & Dry (30°C, 25% RH)",
      printer_model: "Creality Ender-3",
      nozzle_type: "cht",
      nozzle_diameter: 0.6,
      addons: vec![],
      climate_preset: "hot_dry",
      ambient_temp_c: 30,
      humidity_pct: 25,
      wet_rh: None,
      extra_template_ids: vec![],
      spool_specs: vec![
        ("Generic", "PLA"),
        ("Generic", "PETG"),
        ("Generic", "PCTG"),
        ("Generic", "TPU 95A"),
      ],
    },
    PackScenario {
      slug: "05_bambu_x1c_hardened_major_and_exotics",
      title: "Bambu Lab X1 Carbon • 0.4mm Hardened Steel • Temperate Standard Mode (Major + Exotic Polymers)",
      printer_model: "Bambu Lab X1 Carbon",
      nozzle_type: "orca_hardened_steel",
      nozzle_diameter: 0.4,
      addons: vec![],
      climate_preset: "temperate",
      ambient_temp_c: 22,
      humidity_pct: 45,
      wet_rh: None,
      extra_template_ids: vec![],
      spool_specs: vec![
        ("Bambu Lab", "PLA"),
        ("Generic", "PLA-CF"),
        ("Bambu Lab", "PETG"),
        ("Bambu Lab", "ABS"),
        ("Generic", "TPU for AMS"),
        ("Generic", "PEBA"),
        ("Generic", "TPU Foaming"),
        ("Generic", "PET-CF"),
        ("Generic", "PAHT-CF"),
        ("Generic", "PPA-CF"),
        ("Generic", "PVA"),
      ],
    },
    PackScenario {
      slug: "06_anycubic_kobra_s1_temperate_exotics",
      title: "Anycubic Kobra S1 • 0.4mm Hardened Steel • Temperate Baseline (Major + Exotic Polymers)",
      printer_model: "Anycubic Kobra S1",
      nozzle_type: "orca_hardened_steel",
      nozzle_diameter: 0.4,
      addons: vec![],
      climate_preset: "temperate",
      ambient_temp_c: 22,
      humidity_pct: 45,
      wet_rh: None,
      extra_template_ids: vec![("anycubic_pctg_as_petg", vec![])],
      spool_specs: vec![
        ("Anycubic", "PLA"),
        ("Anycubic", "PETG"),
        ("Anycubic", "PLA-CF"),
        ("Generic", "PEBA"),
        ("Generic", "TPU Foaming"),
        ("Generic", "TPU 85A"),
        ("Generic", "PET-CF"),
        ("Generic", "PPA-CF"),
        ("Generic", "PVA"),
        ("Generic", "PLA Wood"),
      ],
    },
  ];

  let mut schema_failures = Vec::new();
  for sc in &scenarios {
    let sc_dir = pack_root.join(sc.slug);
    fs::create_dir_all(&sc_dir).unwrap();

    let mut rig = base_rig_for_printer(sc.printer_model);
    if sc.spool_specs.len() >= 8 && rig.ace_pro_units == 1 {
      rig.ace_pro_units = 2;
    }
    rig.nozzle_diameter = format!("{:.1}", sc.nozzle_diameter);
    apply_nozzle_metadata(&mut rig, sc.nozzle_type);
    rig.addons = sc.addons.iter().map(|s| s.to_string()).collect();
    rig.climate_preset = sc.climate_preset.to_string();
    rig.ambient_temp_c = sc.ambient_temp_c;
    rig.humidity_pct = sc.humidity_pct;
    rig.refresh_title_and_notes(true);

    let mut spools = Vec::new();
    for (idx, (brand, code)) in sc.spool_specs.iter().enumerate() {
      let entry = catalog
        .filaments
        .iter()
        .find(|f| f.brand.eq_ignore_ascii_case(brand) && f.code.eq_ignore_ascii_case(code))
        .or_else(|| catalog.filaments.iter().find(|f| f.code.eq_ignore_ascii_case(code)))
        .unwrap_or_else(|| panic!("Missing catalog entry {} {}", brand, code));
      let mut sp = FilamentSpool::from_catalog_entry(idx + 1, entry, &rig.nozzle_type);
      sp.recompute_for_rig(&rig);
      sp.clamp_to_machine_limits(&rig.printer_model);
      sp.refresh_thermal_deltas(&rig.nozzle_type);
      spools.push(sp);
    }

    let processes = ProcessProfile::get_default_processes_for_rig(&rig);
    let active_proc = &processes[0];
    let base_profile = OrcaProfile::get_stock_for_printer_model(&rig.printer_model);

    let mut rules = TransformationEngine::get_default_rules();
    TransformationEngine::sync_rules_for_rig(
      &mut rules,
      &rig.nozzle_type,
      &rig.nozzle_material,
      rig.ace_pro_units,
    );
    for r in &mut rules {
      if r.id == "weather_climate_adjustment" {
        r.enabled = true;
        let mut params = HashMap::new();
        params.insert("climate_preset".to_string(), rig.climate_preset.clone());
        params.insert("ambient_temp_c".to_string(), rig.ambient_temp_c.to_string());
        params.insert("humidity_pct".to_string(), rig.humidity_pct.to_string());
        r.lua_code =
          orcaslicer_webconfig::lua_engine::embed_param_defaults_in_lua(&r.lua_code, &params);
      }
    }
    if let Some(wrh) = sc.wet_rh {
      if let Some(tpl) = templates.iter().find(|t| t.id == "sym_wet_filament") {
        let mut ans = tpl.default_answers();
        tpl.apply_answer_change(&mut ans, "humidity_pct", wrh.to_string());
        let (title, desc, _, lua) = tpl.generate_from_answers(&ans, "all");
        rules.push(orcaslicer_webconfig::transformations::TransformationRule {
          id: "sym_wet_filament_inspect".to_string(),
          name: title,
          description: desc,
          category: "Symptom Wizard".to_string(),
          icon: "💧".to_string(),
          enabled: true,
          condition_expr: "all".to_string(),
          lua_code: lua,
          priority: 52,
          scope: RuleScopeFilter::default(),
        });
      }
    }
    for (tid, overrides_kv) in &sc.extra_template_ids {
      if let Some(r_existing) = rules.iter_mut().find(|r| r.id == *tid) {
        r_existing.enabled = true;
        if let Some(tpl) = templates.iter().find(|t| t.id == *tid) {
          let mut ans = tpl.default_answers();
          for (k, v) in overrides_kv {
            tpl.apply_answer_change(&mut ans, k, (*v).to_string());
          }
          let (_, _, _, lua) = tpl.generate_from_answers(&ans, "all");
          r_existing.lua_code = lua;
        }
      } else if let Some(tpl) = templates.iter().find(|t| t.id == *tid) {
        let mut ans = tpl.default_answers();
        for (k, v) in overrides_kv {
          tpl.apply_answer_change(&mut ans, k, (*v).to_string());
        }
        let (title, desc, _, lua) = tpl.generate_from_answers(&ans, "all");
        rules.push(orcaslicer_webconfig::transformations::TransformationRule {
          id: (*tid).to_string(),
          name: title,
          description: desc,
          category: "Inspection Rule".to_string(),
          icon: "🔬".to_string(),
          enabled: true,
          condition_expr: "all".to_string(),
          lua_code: lua,
          priority: 54,
          scope: RuleScopeFilter::default(),
        });
      }
    }

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
    let all_filament_types: Vec<String> = spools.iter().map(|s| s.material_type.clone()).collect();
    let eval_ctx = serde_json::json!({
      "printer_model": rig.printer_model,
      "hotend": rig.hotend,
      "nozzle_type": rig.nozzle_type,
      "nozzle_material": rig.nozzle_material,
      "nozzle_diameter": rig.nozzle_diameter,
      "hotend_max_flow": rig.max_volumetric_flow_limit,
      "hotend_max_temp_c": orcaslicer_webconfig::catalog::machine_constants(&rig.printer_model).max_hotend_temp_c,
      "printer_kinematics_class": rig.kinematics_class(),
      "travel_accel_target_mm2": rig.travel_accel_policy().travel_accel_target_mm2,
      "travel_accel_floor_mm2": rig.travel_accel_policy().travel_accel_floor_mm2,
      "filament_max_flow": spools[0].max_volumetric_speed as f64,
      "melt_zone_multiplier": rig.melt_zone_multiplier,
      "flush_multiplier": 1.0,
      "layer_height": active_proc.layer_height,
      "line_width": active_proc.line_width,
      "filament_type": spools[0].material_type,
      "filament_finish": spools[0].finish_type,
      "filament_is_elastomer": orcaslicer_webconfig::constants::constants()
        .is_elastomer_family_material(&spools[0].material_type),
      "filament_nozzle_max_c": FilamentSpool::polymer_safe_nozzle_bounds(&spools[0].material_type).1,
      "all_filament_types": all_filament_types,
      "addons": rig.addons,
      "climate_preset": rig.climate_preset,
      "ambient_temp_c": rig.ambient_temp_c,
      "humidity_pct": rig.humidity_pct,
      "slots": slots_json,
    });
    let (seeded_base, seeded_ctx) =
      TransformationEngine::seed_process_inputs(&base_profile, &eval_ctx, active_proc);
    let stack_eval = TransformationEngine::evaluate_stack(&seeded_base, &rules, &seeded_ctx);
    let per_proc_overrides = TransformationEngine::evaluate_stack_per_process(
      &base_profile,
      &rules,
      &eval_ctx,
      &processes,
    );
    let derived_overrides = per_proc_overrides
      .get(&active_proc.id)
      .cloned()
      .unwrap_or(stack_eval.resulting_profile.values);

    let mut entries = Vec::new();
    let save_files = sync_live_state_to_entries_with_process_overrides(
      &mut entries,
      std::slice::from_ref(&rig),
      &rig.id,
      &spools,
      &processes,
      &active_proc.id,
      &stack_eval.flush_matrix,
      1.0,
      "",
      &[],
      &derived_overrides,
      &per_proc_overrides,
    );

    let mut md = Vec::new();
    md.push(format!("# Side-by-Side Ground-Truth Diff — {}", sc.title));
    md.push(String::new());

    let mut sorted_files = save_files.files.clone();
    sorted_files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    for sf in &sorted_files {
      let dest = sc_dir.join(&sf.relative_path);
      if let Some(p) = dest.parent() {
        fs::create_dir_all(p).unwrap();
      }
      fs::write(&dest, &sf.content).unwrap();
      if sf.relative_path.ends_with(".json") {
        let ptype = if sf.relative_path.contains("/machine/") {
          "machine"
        } else if sf.relative_path.contains("/process/") {
          "process"
        } else if sf.relative_path.contains("/filament/") {
          "filament"
        } else {
          ""
        };
        if !ptype.is_empty() {
          let v: Value = serde_json::from_str(&sf.content).unwrap();
          gt.validate_preset_json(ptype, sc.slug, &v, &mut schema_failures);
          write_side_by_side_diff_section(&mut md, gt, ptype, &sf.relative_path, &v);
        }
      }
    }
    fs::write(sc_dir.join("SIDE_BY_SIDE_DIFFS.md"), md.join("\n")).unwrap();
    println!(
      "Wrote inspection scenario '{}' ({} files) -> {}",
      sc.slug,
      sorted_files.len(),
      sc_dir.display()
    );
  }

  if !schema_failures.is_empty() {
    for f in &schema_failures {
      eprintln!("SCHEMA FAILURE: {}", f);
    }
    panic!(
      "Inspection pack failed {} ground-truth C++ schema checks!",
      schema_failures.len()
    );
  }
}

fn repair_live_slicer_dir(snap_path: &Path, slicer_dir: &Path, out_3mf: &Path, out_zip: &Path) {
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
  let slots_json: Vec<Value> = snap
    .filaments
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
    "printer_model": active_rig.printer_model,
    "hotend": active_rig.hotend,
    "nozzle_type": active_rig.nozzle_type,
    "nozzle_material": active_rig.nozzle_material,
    "nozzle_diameter": active_rig.nozzle_diameter,
    "hotend_max_flow": active_rig.max_volumetric_flow_limit,
    "hotend_max_temp_c": orcaslicer_webconfig::catalog::machine_constants(&active_rig.printer_model).max_hotend_temp_c,
    "printer_kinematics_class": active_rig.kinematics_class(),
    "travel_accel_target_mm2": active_rig.travel_accel_policy().travel_accel_target_mm2,
    "travel_accel_floor_mm2": active_rig.travel_accel_policy().travel_accel_floor_mm2,
    "melt_zone_multiplier": active_rig.melt_zone_multiplier,
    "flush_multiplier": snap.flush_multiplier,
    "layer_height": active_proc.layer_height,
    "line_width": active_proc.line_width,
    "filament_type": snap.filaments.first().map(|s| s.material_type.as_str()).unwrap_or("PLA"),
    "addons": active_rig.addons,
    "climate_preset": active_rig.climate_preset,
    "ambient_temp_c": active_rig.ambient_temp_c,
    "humidity_pct": active_rig.humidity_pct,
    "slots": slots_json,
  });
  let stack_eval = TransformationEngine::evaluate_stack(&snap.base_profile, &snap.rules, &eval_ctx);
  let derived_overrides = stack_eval.resulting_profile.values;

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
  for sf in &save_files.files {
    let dest = slicer_dir.join(&sf.relative_path);
    if let Some(parent) = dest.parent() {
      fs::create_dir_all(parent).expect("Failed to create parent dir");
    }
    fs::write(&dest, &sf.content).expect("Failed to write repaired file");
    println!("Updated live file: {}", dest.display());
  }
  for del_rel in &save_files.deletions {
    let dest = slicer_dir.join(del_rel);
    let _ = fs::remove_file(&dest);
    let _ = fs::remove_file(dest.with_extension("info"));
    println!("Pruned stale OC preset: {}", dest.display());
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
