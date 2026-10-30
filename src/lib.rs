#![recursion_limit = "512"]

pub mod catalog;
pub mod components;
pub mod dir_import;
pub mod fs_access;
pub mod lua_engine;
pub mod models;
pub mod orca_profile;
pub mod threemf;
pub mod transformations;

use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::components::filament_tab::FilamentTab;
use crate::components::home_tab::HomeTab;
use crate::components::machine_tab::MachineTab;
use crate::components::nav::{ActiveTab, NavigationBar};
use crate::components::scratch_wizard::{ScratchWizard, ScratchWizardOutput};
use crate::components::wizard_tab::WizardTab;
use crate::dir_import::{
  build_directory_backup_zip, build_reference_anycubic_entries,
  build_user_disk_config_summary_json, extract_directory_backup_zip, import_scanned_directory,
  sync_live_state_to_entries,
};
use crate::fs_access::{
  FsManager, LocalFsSaveRequest, LocalFsSaveResponse, LocalFsScanPayload, LocalFsScanProgress,
  ScannedFileEntry,
};
use crate::models::{FilamentSpool, MachineRig, ProcessProfile, WebConfigStateSnapshot};
use crate::orca_profile::OrcaProfile;
use crate::threemf::{build_3mf_project_container, export_standalone_filament_json};
use crate::transformations::{
  PendingDiskUpdate, SafetySeverity, SafetyWarning, StackEvaluationResult, StackReorderAction,
  TransformationEngine, TransformationRule,
};

#[derive(Clone, Debug, PartialEq)]
pub enum PendingConfirmedAction {
  SaveToDisk,
  Export3mf,
  ExportWebConfigJson,
  BackupZip,
  ExportStandaloneJson,
  ExportLuaPackage,
}

impl PendingConfirmedAction {
  pub fn label(&self) -> &'static str {
    match self {
      Self::SaveToDisk => "Save Changes to Local Slicer Disk",
      Self::Export3mf => "Export Multi-Material .3mf Project Container",
      Self::ExportWebConfigJson => "Export Orca WebConfig (.json)",
      Self::BackupZip => "Download Full Directory Backup (.zip)",
      Self::ExportStandaloneJson => "Export Standalone OrcaSlicer Preset (.json)",
      Self::ExportLuaPackage => "Export Lua Transformation Stack (.json)",
    }
  }
}

#[function_component]
pub fn App() -> Html {
  let saved_state = use_memo((), |_| FsManager::load_state_from_storage());
  let active_tab = use_state(|| ActiveTab::Home);
  let is_tauri = FsManager::is_tauri();

  // Default to nothing loaded on fresh startup, or restore saved WebConfig state from browser localStorage!
  // On Initialization, Start From Scratch is the default and recommended option.
  let is_config_loaded = use_state(|| saved_state.is_some());
  let show_bind_modal = use_state(|| saved_state.is_none());
  let modal_start_from_scratch = use_state(|| true);
  let show_expert_init = use_state(|| false);
  let show_reset_confirm_modal = use_state(|| false);
  let config_version = use_state(|| {
    (*saved_state)
      .as_ref()
      .map(|s| s.config_version.max(1))
      .unwrap_or(1u32)
  });

  // Pillar 1: Hardware / Hotend Rigs
  let machines = use_state(|| {
    (*saved_state)
      .as_ref()
      .map(|s| s.machines.clone())
      .unwrap_or_default()
  });
  let active_machine_id = use_state(|| {
    (*saved_state)
      .as_ref()
      .map(|s| s.active_machine_id.clone())
      .unwrap_or_else(|| "kobra_s1_hardened_png".to_string())
  });

  // Pillar 2: Filaments & ACE Pro Spools
  let filaments = use_state(|| {
    (*saved_state)
      .as_ref()
      .map(|s| s.filaments.clone())
      .unwrap_or_default()
  });
  let active_filament_type = use_state(|| {
    (*saved_state)
      .as_ref()
      .map(|s| s.active_filament_type.clone())
      .unwrap_or_else(|| "PETG".to_string())
  });

  // Pillar 3: Process / Speed Profiles
  let processes = use_state(|| {
    (*saved_state)
      .as_ref()
      .map(|s| s.processes.clone())
      .unwrap_or_default()
  });
  let active_process_id = use_state(|| {
    (*saved_state)
      .as_ref()
      .map(|s| s.active_process_id.clone())
      .unwrap_or_else(|| "proc_020_std_png".to_string())
  });

  // Scanned directory files for full .zip backup
  let scanned_entries = use_state(Vec::<ScannedFileEntry>::new);
  let detected_hardware_summary = use_state(|| {
    (*saved_state)
      .as_ref()
      .map(|s| s.detected_hardware_summary.clone())
      .unwrap_or_else(|| "Unconnected".to_string())
  });

  // Base Profile & Lua Transformation Stack
  let base_profile = use_state(|| {
    (*saved_state)
      .as_ref()
      .map(|s| s.base_profile.clone())
      .unwrap_or_else(OrcaProfile::get_stock_kobra_s1)
  });
  let rules = use_state(|| {
    (*saved_state)
      .as_ref()
      .map(|s| s.rules.clone())
      .unwrap_or_default()
  });

  // Local Disk & ZIP Source Metadata State
  let bound_filename = use_state(|| {
    (*saved_state)
      .as_ref()
      .map(|s| s.bound_filename.clone())
      .unwrap_or_else(|| "No Config Loaded".to_string())
  });
  let bound_path_hint = use_state(|| {
    (*saved_state)
      .as_ref()
      .map(|s| s.bound_path_hint.clone())
      .unwrap_or_else(|| {
        "Import from Local Filesystem, Load WebConfig JSON, Start From Scratch, or Import a Backup .zip"
          .to_string()
      })
  });
  let last_saved_time = use_state(|| {
    if saved_state.is_some() {
      "Restored from browser site data".to_string()
    } else {
      "Not yet synced".to_string()
    }
  });

  // UI & Flushing Matrix / Multiplier state
  let show_diff_drawer = use_state(|| false);
  let show_export_modal = use_state(|| false);
  let toast_message = use_state(|| None::<String>);
  let conflicts = use_state(|| {
    (*saved_state)
      .as_ref()
      .map(|s| s.conflicts.clone())
      .unwrap_or_default()
  });
  let pending_updates = use_state(|| {
    (*saved_state)
      .as_ref()
      .map(|s| s.pending_updates.clone())
      .unwrap_or_default()
  });
  let execution_logs = use_state(Vec::<String>::new);
  let import_progress = use_state(|| None::<LocalFsScanProgress>);
  let disk_flush_matrix = use_state(|| {
    (*saved_state)
      .as_ref()
      .map(|s| s.disk_flush_matrix.clone())
      .unwrap_or_default()
  });
  let disk_flush_slot_count = use_state(|| {
    (*saved_state)
      .as_ref()
      .map(|s| s.disk_flush_slot_count)
      .unwrap_or(0usize)
  });
  let disk_flush_multiplier = use_state(|| {
    (*saved_state)
      .as_ref()
      .map(|s| s.disk_flush_multiplier)
      .unwrap_or(1.0f64)
  });
  let flush_multiplier = use_state(|| {
    (*saved_state)
      .as_ref()
      .map(|s| s.flush_multiplier)
      .unwrap_or(1.0f64)
  });
  let conf_relative_path = use_state(|| {
    (*saved_state)
      .as_ref()
      .map(|s| s.conf_relative_path.clone())
      .unwrap_or_else(|| "AnycubicSlicerNext.conf".to_string())
  });

  // Automatically persist full site state to browser localStorage whenever a config is loaded
  {
    let is_loaded = *is_config_loaded;
    let snap = WebConfigStateSnapshot {
      schema_version: "2.2.0".to_string(),
      config_version: *config_version,
      orca_slicer_version: "2.2".to_string(),
      webconfig_app_version: "2.2.0".to_string(),
      bound_filename: (*bound_filename).clone(),
      bound_path_hint: (*bound_path_hint).clone(),
      detected_hardware_summary: (*detected_hardware_summary).clone(),
      machines: (*machines).clone(),
      active_machine_id: (*active_machine_id).clone(),
      filaments: (*filaments).clone(),
      active_filament_type: (*active_filament_type).clone(),
      processes: (*processes).clone(),
      active_process_id: (*active_process_id).clone(),
      base_profile: (*base_profile).clone(),
      rules: (*rules).clone(),
      conflicts: (*conflicts).clone(),
      pending_updates: (*pending_updates).clone(),
      flush_multiplier: *flush_multiplier,
      disk_flush_multiplier: *disk_flush_multiplier,
      disk_flush_matrix: (*disk_flush_matrix).clone(),
      disk_flush_slot_count: *disk_flush_slot_count,
      conf_relative_path: (*conf_relative_path).clone(),
    };
    use_effect_with((is_loaded, snap), move |(loaded, snapshot)| {
      if *loaded {
        let _ = FsManager::save_state_to_storage(snapshot);
      }
      || ()
    });
  }

  // Active Rig & Process
  let active_rig = machines
    .iter()
    .find(|m| m.id == *active_machine_id)
    .cloned()
    .or_else(|| machines.first().cloned())
    .unwrap_or_else(MachineRig::kobra_s1_hardened_steel_png);

  let active_proc = processes
    .iter()
    .find(|p| p.id == *active_process_id)
    .cloned()
    .or_else(|| processes.first().cloned())
    .unwrap_or_else(|| ProcessProfile::get_default_processes().remove(1));

  let slot_count = (active_rig.ace_pro_units * 4).min(filaments.len());
  let active_slots: Vec<FilamentSpool> = filaments.iter().take(slot_count).cloned().collect();

  // Minimum filament max volumetric speed across active slots
  let active_filament_max_flow = filaments
    .iter()
    .find(|s| s.material_type.eq_ignore_ascii_case(&active_filament_type))
    .map(|s| s.max_volumetric_speed as f64)
    .unwrap_or(12.0);

  let slots_json: Vec<serde_json::Value> = active_slots
    .iter()
    .map(|s| {
      serde_json::json!({
        "name": s.name,
        "material_type": s.material_type,
        "color_hex": s.color_hex,
        "opacity": s.opacity,
        "pigment_stickiness": s.pigment_stickiness
      })
    })
    .collect();

  let all_filament_types: Vec<String> = filaments.iter().map(|s| s.material_type.clone()).collect();

  // Context for Lua Evaluation (includes rig addons, hotend, weather & flush_multiplier!)
  let eval_context = serde_json::json!({
    "printer_model": active_rig.printer_model,
    "hotend": active_rig.hotend,
    "nozzle_type": active_rig.nozzle_type,
    "nozzle_material": active_rig.nozzle_material,
    "nozzle_diameter": active_rig.nozzle_diameter,
    "hotend_max_flow": active_rig.max_volumetric_flow_limit,
    "filament_max_flow": active_filament_max_flow,
    "melt_zone_multiplier": active_rig.melt_zone_multiplier,
    "flush_multiplier": *flush_multiplier,
    "layer_height": active_proc.layer_height,
    "line_width": active_proc.line_width,
    "filament_type": *active_filament_type,
    "all_filament_types": all_filament_types,
    "addons": active_rig.addons,
    "climate_preset": active_rig.climate_preset,
    "ambient_temp_c": active_rig.ambient_temp_c,
    "humidity_pct": active_rig.humidity_pct,
    "slots": slots_json,
  });

  // Prepare base profile with active process speeds so Lua can cap them
  let mut working_base = (*base_profile).clone();
  working_base.set_str("layer_height", &format!("{:.2}", active_proc.layer_height));
  working_base.set_str(
    "outer_wall_line_width",
    &format!("{:.2}", active_proc.line_width),
  );
  working_base.set_str(
    "outer_wall_speed",
    &format!("{:.0}", active_proc.outer_wall_speed),
  );
  working_base.set_str(
    "inner_wall_speed",
    &format!("{:.0}", active_proc.inner_wall_speed),
  );
  working_base.set_str(
    "sparse_infill_speed",
    &format!("{:.0}", active_proc.sparse_infill_speed),
  );
  working_base.set_str("flush_multiplier", &format!("{:.2}", *flush_multiplier));

  // Evaluate Lua Stack
  let eval_result: StackEvaluationResult =
    TransformationEngine::evaluate_stack(&working_base, &rules, &eval_context);

  // Evaluate Safety & Weird Value Warnings across imported files, conflicts, flushing matrix, and derived profile
  let safety_warnings: Vec<SafetyWarning> = TransformationEngine::evaluate_safety_warnings(
    &active_rig,
    &filaments,
    &eval_result.resulting_profile,
    &conflicts,
    &disk_flush_matrix,
  );
  let pending_confirmed_action = use_state(|| None::<PendingConfirmedAction>);

  // Compute JSON diff preview
  let base_json_str = working_base.to_json_pretty();
  let derived_json_str = eval_result.resulting_profile.to_json_pretty();

  let mut diff_lines = Vec::new();
  let base_lines: Vec<&str> = base_json_str.lines().collect();
  let derived_lines: Vec<&str> = derived_json_str.lines().collect();

  for (b_line, d_line) in base_lines.iter().zip(derived_lines.iter()) {
    if b_line == d_line {
      diff_lines.push(format!("  {}", b_line));
    } else {
      diff_lines.push(format!("- {}", b_line));
      diff_lines.push(format!("+ {}", d_line));
    }
  }
  let json_diff_text = diff_lines.join("\n");

  // Build Our Internal Config Data Structure JSON vs User Disk Config JSON
  let our_internal_config_json = serde_json::to_string_pretty(&serde_json::json!({
    "architecture": "Transformation-Based Config State (Not Absolute Values)",
    "schema_version": "2.2.0",
    "config_version": *config_version,
    "orca_slicer_version": "2.2",
    "webconfig_app_version": "2.2.0",
    "active_rig": {
      "id": active_rig.id,
      "name": active_rig.name,
      "concise_title": active_rig.format_concise_title(),
      "machine_type": active_rig.printer_model,
      "nozzle": {
        "type": active_rig.nozzle_type,
        "material": active_rig.nozzle_material,
        "diameter_mm": active_rig.nozzle_diameter,
      },
      "addons": active_rig.addons,
      "printer_notes": active_rig.notes,
      "source_disk_profile": active_rig.source_disk_profile,
    },
    "active_process": {
      "id": active_proc.id,
      "name": active_proc.name,
      "concise_title": active_proc.format_concise_title(&active_rig.short_model_tag()),
      "quality_tier": active_proc.quality_tier(),
      "notes": active_proc.notes,
    },
    "active_lua_transformations": rules
      .iter()
      .filter(|r| r.enabled)
      .map(|r| serde_json::json!({
        "id": r.id,
        "name": r.name,
        "category": r.category,
        "priority": r.priority,
      }))
      .collect::<Vec<_>>(),
    "staged_pending_disk_saves": (*pending_updates).clone(),
    "imported_filament_deltas": filaments
      .iter()
      .take(4)
      .map(|f| serde_json::json!({
        "name": f.name,
        "inherits_system_base": f.inherits,
        "deltas_from_base": f.deltas_from_base,
      }))
      .collect::<Vec<_>>(),
  }))
  .unwrap_or_default();

  // User Disk Config JSON reflects the ACTUAL files on disk (does NOT mirror our derived state until saved!)
  let user_disk_config_json = build_user_disk_config_summary_json(&scanned_entries);

  // Handlers
  let on_select_tab = {
    let active_tab = active_tab.clone();
    Callback::from(move |tab: ActiveTab| {
      active_tab.set(tab);
    })
  };

  let on_open_bind_modal = {
    let show_bind_modal = show_bind_modal.clone();
    Callback::from(move |_| {
      show_bind_modal.set(true);
    })
  };

  // Fetch and import the real local directory configured in `local_slicer_config.json` via `/api/local-slicer-scan`
  let on_load_reference_snapshot = {
    let is_config_loaded = is_config_loaded.clone();
    let show_bind_modal = show_bind_modal.clone();
    let import_progress = import_progress.clone();
    let machines = machines.clone();
    let active_machine_id = active_machine_id.clone();
    let filaments = filaments.clone();
    let processes = processes.clone();
    let active_process_id = active_process_id.clone();
    let base_profile = base_profile.clone();
    let rules = rules.clone();
    let conflicts = conflicts.clone();
    let pending_updates = pending_updates.clone();
    let scanned_entries = scanned_entries.clone();
    let detected_hardware_summary = detected_hardware_summary.clone();
    let bound_filename = bound_filename.clone();
    let bound_path_hint = bound_path_hint.clone();
    let last_saved_time = last_saved_time.clone();
    let disk_flush_matrix = disk_flush_matrix.clone();
    let disk_flush_slot_count = disk_flush_slot_count.clone();
    let disk_flush_multiplier = disk_flush_multiplier.clone();
    let flush_multiplier = flush_multiplier.clone();
    let conf_relative_path = conf_relative_path.clone();
    let toast = toast_message.clone();

    Callback::from(move |_| {
      let icl = is_config_loaded.clone();
      let sbm = show_bind_modal.clone();
      let ip_state = import_progress.clone();
      let mach_state = machines.clone();
      let amid = active_machine_id.clone();
      let fil_state = filaments.clone();
      let proc_state = processes.clone();
      let apid = active_process_id.clone();
      let bp_state = base_profile.clone();
      let rules_state = rules.clone();
      let conf_state = conflicts.clone();
      let pu_state = pending_updates.clone();
      let se_state = scanned_entries.clone();
      let dhs = detected_hardware_summary.clone();
      let bfn = bound_filename.clone();
      let bph = bound_path_hint.clone();
      let lst = last_saved_time.clone();
      let dfm = disk_flush_matrix.clone();
      let dfsc = disk_flush_slot_count.clone();
      let dfmult = disk_flush_multiplier.clone();
      let fmult = flush_multiplier.clone();
      let crp = conf_relative_path.clone();
      let t = toast.clone();

      ip_state.set(Some(LocalFsScanProgress {
        running: true,
        done: false,
        phase: "Starting local filesystem scan...".to_string(),
        files_read: 0,
        total_files: 0,
        current_file: "/mnt/c/Users/drpng/AppData/Roaming/AnycubicSlicerNext".to_string(),
      }));

      spawn_local(async move {
        let _ = gloo_net::http::Request::get(&FsManager::api_url("/api/local-slicer-scan/start"))
          .send()
          .await;

        let mut final_total = 0usize;
        for _ in 0..1200 {
          FsManager::sleep_ms(50).await;
          if let Ok(resp) =
            gloo_net::http::Request::get(&FsManager::api_url("/api/local-slicer-scan/status"))
              .send()
              .await
          {
            if resp.ok() {
              if let Ok(prog) = resp.json::<LocalFsScanProgress>().await {
                final_total = prog.total_files;
                let is_done = prog.done;
                ip_state.set(Some(prog));
                if is_done {
                  break;
                }
              }
            }
          }
        }

        ip_state.set(Some(LocalFsScanProgress {
          running: true,
          done: false,
          phase: "Parsing files & reconstructing hardware/filament delta transformations..."
            .to_string(),
          files_read: final_total,
          total_files: final_total,
          current_file: "Analyzing Kobra S1 rig, nozzles, and user filament deltas...".to_string(),
        }));
        FsManager::sleep_ms(30).await;

        let mut cfg_path = "/mnt/c/Users/drpng/AppData/Roaming/AnycubicSlicerNext".to_string();
        let mut entries =
          match gloo_net::http::Request::get(&FsManager::api_url("/api/local-slicer-scan"))
            .send()
            .await
          {
            Ok(resp) if resp.ok() => match resp.json::<LocalFsScanPayload>().await {
              Ok(payload) => {
                cfg_path = payload.configured_path;
                payload.entries
              }
              Err(_) => Vec::new(),
            },
            _ => Vec::new(),
          };

        if entries.is_empty() {
          entries = build_reference_anycubic_entries();
        }

        match import_scanned_directory(&entries) {
          Ok(imported) => {
            let active_mach = imported
              .machines
              .iter()
              .find(|m| m.id == imported.active_machine_id)
              .or_else(|| imported.machines.first());
            let hw_summary = active_mach
              .map(|m| {
                format!(
                  "{} ({}) with {} ({}mm) Nozzle — user/{}",
                  m.printer_model,
                  m.name,
                  m.nozzle_material,
                  m.nozzle_diameter,
                  imported.user_folder_id
                )
              })
              .unwrap_or_else(|| "Anycubic Kobra S1 with Hardened Steel Nozzle".to_string());

            se_state.set(entries);
            dhs.set(hw_summary.clone());
            amid.set(imported.active_machine_id.clone());
            apid.set(imported.active_process_id.clone());
            dfm.set(imported.disk_flush_matrix.clone());
            dfsc.set(imported.disk_flush_slot_count);
            dfmult.set(imported.disk_flush_multiplier);
            fmult.set(imported.disk_flush_multiplier);
            crp.set(imported.conf_relative_path.clone());
            bp_state.set(imported.base_profile);
            let mut default_rules = TransformationEngine::get_default_rules();
            if let Some(m) = active_mach {
              TransformationEngine::sync_rules_for_rig(
                &mut default_rules,
                &m.nozzle_type,
                &m.nozzle_material,
                m.ace_pro_units,
              );
            }
            rules_state.set(default_rules);
            mach_state.set(imported.machines);
            fil_state.set(imported.filaments);
            proc_state.set(imported.processes);
            conf_state.set(imported.conflicts.clone());
            pu_state.set(Vec::new());
            bfn.set(format!(
              "AnycubicSlicerNext/user/{}/",
              imported.user_folder_id
            ));
            bph.set(format!(
              "{} (Imported via local_slicer_config.json • {} files scanned)",
              cfg_path, imported.total_files_scanned
            ));
            lst.set("Imported from Local FS".to_string());
            ip_state.set(None);
            icl.set(true);
            sbm.set(false);

            t.set(Some(format!(
              "Read {} files from '{}' (local_slicer_config.json): {}! Flagged {} one-off differences.",
              imported.total_files_scanned,
              cfg_path,
              hw_summary,
              imported.conflicts.len()
            )));
          }
          Err(e) => {
            ip_state.set(None);
            t.set(Some(format!("Import error: {}", e)));
          }
        }
      });
    })
  };

  // Import a `.zip` backup archive via `<input type="file" accept=".zip">`
  let on_import_zip = {
    let is_config_loaded = is_config_loaded.clone();
    let show_bind_modal = show_bind_modal.clone();
    let machines = machines.clone();
    let active_machine_id = active_machine_id.clone();
    let filaments = filaments.clone();
    let processes = processes.clone();
    let active_process_id = active_process_id.clone();
    let base_profile = base_profile.clone();
    let rules = rules.clone();
    let conflicts = conflicts.clone();
    let pending_updates = pending_updates.clone();
    let scanned_entries = scanned_entries.clone();
    let detected_hardware_summary = detected_hardware_summary.clone();
    let bound_filename = bound_filename.clone();
    let bound_path_hint = bound_path_hint.clone();
    let last_saved_time = last_saved_time.clone();
    let disk_flush_matrix = disk_flush_matrix.clone();
    let disk_flush_slot_count = disk_flush_slot_count.clone();
    let disk_flush_multiplier = disk_flush_multiplier.clone();
    let flush_multiplier = flush_multiplier.clone();
    let conf_relative_path = conf_relative_path.clone();
    let toast = toast_message.clone();

    Callback::from(move |e: Event| {
      let input: web_sys::HtmlInputElement = e.target_unchecked_into();
      let icl = is_config_loaded.clone();
      let sbm = show_bind_modal.clone();
      let mach_state = machines.clone();
      let amid = active_machine_id.clone();
      let fil_state = filaments.clone();
      let proc_state = processes.clone();
      let apid = active_process_id.clone();
      let bp_state = base_profile.clone();
      let rules_state = rules.clone();
      let conf_state = conflicts.clone();
      let pu_state = pending_updates.clone();
      let se_state = scanned_entries.clone();
      let dhs = detected_hardware_summary.clone();
      let bfn = bound_filename.clone();
      let bph = bound_path_hint.clone();
      let lst = last_saved_time.clone();
      let dfm = disk_flush_matrix.clone();
      let dfsc = disk_flush_slot_count.clone();
      let dfmult = disk_flush_multiplier.clone();
      let fmult = flush_multiplier.clone();
      let crp = conf_relative_path.clone();
      let t = toast.clone();

      spawn_local(async move {
        match FsManager::read_input_file_bytes(&input).await {
          Ok((zip_name, bytes)) => match extract_directory_backup_zip(&bytes) {
            Ok(entries) => match import_scanned_directory(&entries) {
              Ok(imported) => {
                let active_mach = imported
                  .machines
                  .iter()
                  .find(|m| m.id == imported.active_machine_id)
                  .or_else(|| imported.machines.first());
                let hw_summary = active_mach
                  .map(|m| {
                    format!(
                      "{} ({}) with {} ({}mm) Nozzle — user/{}",
                      m.printer_model,
                      m.name,
                      m.nozzle_material,
                      m.nozzle_diameter,
                      imported.user_folder_id
                    )
                  })
                  .unwrap_or_else(|| "Anycubic Kobra S1 with Hardened Steel Nozzle".to_string());

                se_state.set(entries);
                dhs.set(hw_summary.clone());
                amid.set(imported.active_machine_id.clone());
                apid.set(imported.active_process_id.clone());
                dfm.set(imported.disk_flush_matrix.clone());
                dfsc.set(imported.disk_flush_slot_count);
                dfmult.set(imported.disk_flush_multiplier);
                fmult.set(imported.disk_flush_multiplier);
                crp.set(imported.conf_relative_path.clone());
                bp_state.set(imported.base_profile);
                let mut default_rules = TransformationEngine::get_default_rules();
                if let Some(m) = active_mach {
                  TransformationEngine::sync_rules_for_rig(
                    &mut default_rules,
                    &m.nozzle_type,
                    &m.nozzle_material,
                    m.ace_pro_units,
                  );
                }
                rules_state.set(default_rules);
                mach_state.set(imported.machines);
                fil_state.set(imported.filaments);
                proc_state.set(imported.processes);
                conf_state.set(imported.conflicts.clone());
                pu_state.set(Vec::new());

                bfn.set(format!("{} (user/{})", zip_name, imported.user_folder_id));
                bph.set(format!(
                  "Imported ZIP Archive: {} ({} config files • {})",
                  zip_name, imported.total_files_scanned, hw_summary
                ));
                lst.set("Imported from .zip".to_string());
                icl.set(true);
                sbm.set(false);

                t.set(Some(format!(
                  "Imported '{}' ({} files): {}! Flagged {} one-off differences.",
                  zip_name,
                  imported.total_files_scanned,
                  hw_summary,
                  imported.conflicts.len()
                )));
              }
              Err(err) => {
                t.set(Some(format!("ZIP directory import failed: {}", err)));
              }
            },
            Err(err) => {
              t.set(Some(format!("Failed to extract .zip backup: {}", err)));
            }
          },
          Err(err) => {
            t.set(Some(format!("Failed to read .zip file: {}", err)));
          }
        }
      });
    })
  };

  // Requirement #1: Zip the entire loaded directory (including all added/edited OC: filaments, rigs, processes, and staged updates) and save as a .zip backup!
  let on_backup_zip = {
    let scanned_entries = scanned_entries.clone();
    let machines = machines.clone();
    let active_machine_id = active_machine_id.clone();
    let filaments = filaments.clone();
    let processes = processes.clone();
    let active_process_id = active_process_id.clone();
    let flush_matrix = eval_result.flush_matrix.clone();
    let flush_multiplier = *flush_multiplier;
    let conf_relative_path = conf_relative_path.clone();
    let pending_updates = pending_updates.clone();
    let derived_overrides = eval_result.resulting_profile.values.clone();
    let toast = toast_message.clone();
    Callback::from(move |_| {
      let mut entries = (*scanned_entries).clone();
      let _ = sync_live_state_to_entries(
        &mut entries,
        &machines,
        &active_machine_id,
        &filaments,
        &processes,
        &active_process_id,
        &flush_matrix,
        flush_multiplier,
        &conf_relative_path,
        &pending_updates,
        &derived_overrides,
      );
      let zip_bytes = build_directory_backup_zip(&entries);
      let filename = "AnycubicSlicerNext_Config_Backup.zip";
      match FsManager::trigger_binary_download(filename, &zip_bytes, "application/zip") {
        Ok(()) => {
          toast.set(Some(format!(
            "Created full directory backup '{}' ({} files archived including all OC: filaments/rigs/processes, {} bytes)!",
            filename,
            entries.len(),
            zip_bytes.len()
          )));
        }
        Err(e) => {
          toast.set(Some(format!("Backup ZIP error: {}", e)));
        }
      }
    })
  };

  // Requirement #2: Export full site state as a single Orca WebConfig JSON (`orcaslicer_webconfig.json`)
  let on_export_webconfig_json = {
    let bound_filename = bound_filename.clone();
    let bound_path_hint = bound_path_hint.clone();
    let detected_hardware_summary = detected_hardware_summary.clone();
    let machines = machines.clone();
    let active_machine_id = active_machine_id.clone();
    let filaments = filaments.clone();
    let active_filament_type = active_filament_type.clone();
    let processes = processes.clone();
    let active_process_id = active_process_id.clone();
    let base_profile = base_profile.clone();
    let rules = rules.clone();
    let conflicts = conflicts.clone();
    let pending_updates = pending_updates.clone();
    let flush_multiplier = flush_multiplier.clone();
    let disk_flush_multiplier = disk_flush_multiplier.clone();
    let disk_flush_matrix = disk_flush_matrix.clone();
    let disk_flush_slot_count = disk_flush_slot_count.clone();
    let conf_relative_path = conf_relative_path.clone();
    let config_version = config_version.clone();
    let toast = toast_message.clone();

    Callback::from(move |_| {
      let next_ver = *config_version + 1;
      config_version.set(next_ver);
      let snapshot = WebConfigStateSnapshot {
        schema_version: "2.2.0".to_string(),
        config_version: next_ver,
        orca_slicer_version: "2.2".to_string(),
        webconfig_app_version: "2.2.0".to_string(),
        bound_filename: (*bound_filename).clone(),
        bound_path_hint: (*bound_path_hint).clone(),
        detected_hardware_summary: (*detected_hardware_summary).clone(),
        machines: (*machines).clone(),
        active_machine_id: (*active_machine_id).clone(),
        filaments: (*filaments).clone(),
        active_filament_type: (*active_filament_type).clone(),
        processes: (*processes).clone(),
        active_process_id: (*active_process_id).clone(),
        base_profile: (*base_profile).clone(),
        rules: (*rules).clone(),
        conflicts: (*conflicts).clone(),
        pending_updates: (*pending_updates).clone(),
        flush_multiplier: *flush_multiplier,
        disk_flush_multiplier: *disk_flush_multiplier,
        disk_flush_matrix: (*disk_flush_matrix).clone(),
        disk_flush_slot_count: *disk_flush_slot_count,
        conf_relative_path: (*conf_relative_path).clone(),
      };
      match serde_json::to_string_pretty(&snapshot) {
        Ok(json_str) => {
          let filename = "orcaslicer_webconfig.json";
          match FsManager::trigger_browser_download(filename, &json_str) {
            Ok(()) => {
              toast.set(Some(format!(
                "Exported OrcaSlicer WebConfig v{} (for OrcaSlicer {}) to '{}' ({} machines, {} filaments, {} processes, {} Lua rules)!",
                next_ver,
                snapshot.orca_slicer_version,
                filename,
                snapshot.machines.len(),
                snapshot.filaments.len(),
                snapshot.processes.len(),
                snapshot.rules.len()
              )));
            }
            Err(e) => toast.set(Some(format!("Export JSON error: {}", e))),
          }
        }
        Err(e) => toast.set(Some(format!("Failed to serialize WebConfig JSON: {}", e))),
      }
    })
  };

  // Requirement #2: Load an Orca WebConfig JSON (`orcaslicer_webconfig.json`) to recreate the entire site state
  let on_import_webconfig_json = {
    let is_config_loaded = is_config_loaded.clone();
    let show_bind_modal = show_bind_modal.clone();
    let modal_start_from_scratch = modal_start_from_scratch.clone();
    let config_version = config_version.clone();
    let bound_filename = bound_filename.clone();
    let bound_path_hint = bound_path_hint.clone();
    let detected_hardware_summary = detected_hardware_summary.clone();
    let machines = machines.clone();
    let active_machine_id = active_machine_id.clone();
    let filaments = filaments.clone();
    let active_filament_type = active_filament_type.clone();
    let processes = processes.clone();
    let active_process_id = active_process_id.clone();
    let base_profile = base_profile.clone();
    let rules = rules.clone();
    let conflicts = conflicts.clone();
    let pending_updates = pending_updates.clone();
    let flush_multiplier = flush_multiplier.clone();
    let disk_flush_multiplier = disk_flush_multiplier.clone();
    let disk_flush_matrix = disk_flush_matrix.clone();
    let disk_flush_slot_count = disk_flush_slot_count.clone();
    let conf_relative_path = conf_relative_path.clone();
    let last_saved_time = last_saved_time.clone();
    let toast = toast_message.clone();

    Callback::from(move |e: Event| {
      let input: web_sys::HtmlInputElement = e.target_unchecked_into();
      let is_config_loaded = is_config_loaded.clone();
      let show_bind_modal = show_bind_modal.clone();
      let modal_start_from_scratch = modal_start_from_scratch.clone();
      let config_version = config_version.clone();
      let bound_filename = bound_filename.clone();
      let bound_path_hint = bound_path_hint.clone();
      let detected_hardware_summary = detected_hardware_summary.clone();
      let machines = machines.clone();
      let active_machine_id = active_machine_id.clone();
      let filaments = filaments.clone();
      let active_filament_type = active_filament_type.clone();
      let processes = processes.clone();
      let active_process_id = active_process_id.clone();
      let base_profile = base_profile.clone();
      let rules = rules.clone();
      let conflicts = conflicts.clone();
      let pending_updates = pending_updates.clone();
      let flush_multiplier = flush_multiplier.clone();
      let disk_flush_multiplier = disk_flush_multiplier.clone();
      let disk_flush_matrix = disk_flush_matrix.clone();
      let disk_flush_slot_count = disk_flush_slot_count.clone();
      let conf_relative_path = conf_relative_path.clone();
      let last_saved_time = last_saved_time.clone();
      let toast = toast.clone();

      spawn_local(async move {
        match FsManager::read_input_file_bytes(&input).await {
          Ok((fname, bytes)) => match serde_json::from_slice::<WebConfigStateSnapshot>(&bytes) {
            Ok(mut snap) => {
              snap.sanitize();
              let _ = FsManager::save_state_to_storage(&snap);
              config_version.set(snap.config_version.max(1));
              bound_filename.set(format!("Loaded WebConfig JSON ({})", fname));
              bound_path_hint.set(snap.bound_path_hint.clone());
              detected_hardware_summary.set(snap.detected_hardware_summary.clone());
              machines.set(snap.machines.clone());
              active_machine_id.set(snap.active_machine_id.clone());
              filaments.set(snap.filaments.clone());
              active_filament_type.set(snap.active_filament_type.clone());
              processes.set(snap.processes.clone());
              active_process_id.set(snap.active_process_id.clone());
              base_profile.set(snap.base_profile.clone());
              rules.set(snap.rules.clone());
              conflicts.set(snap.conflicts.clone());
              pending_updates.set(snap.pending_updates.clone());
              flush_multiplier.set(snap.flush_multiplier);
              disk_flush_multiplier.set(snap.disk_flush_multiplier);
              disk_flush_matrix.set(snap.disk_flush_matrix.clone());
              disk_flush_slot_count.set(snap.disk_flush_slot_count);
              conf_relative_path.set(snap.conf_relative_path.clone());
              last_saved_time.set(format!("Loaded from {}", fname));
              is_config_loaded.set(true);
              show_bind_modal.set(false);
              modal_start_from_scratch.set(true);
              toast.set(Some(format!(
                "Loaded OrcaSlicer WebConfig v{} '{}': restored {} machine(s), {} filament(s), {} process(es), and {} Lua rule(s)!",
                snap.config_version,
                fname,
                snap.machines.len(),
                snap.filaments.len(),
                snap.processes.len(),
                snap.rules.len()
              )));
            }
            Err(err) => {
              toast.set(Some(format!(
                "Invalid Orca WebConfig JSON '{}': {}",
                fname, err
              )));
            }
          },
          Err(err) => {
            toast.set(Some(format!("Failed to read JSON file: {}", err)));
          }
        }
      });
    })
  };

  // Requirement #8: Reset All opens an "Are you sure?" confirmation dialog before clearing state
  let on_reset_all = {
    let show_reset_confirm_modal = show_reset_confirm_modal.clone();
    Callback::from(move |_| {
      show_reset_confirm_modal.set(true);
    })
  };

  let on_confirm_reset_all = {
    let is_config_loaded = is_config_loaded.clone();
    let show_bind_modal = show_bind_modal.clone();
    let show_reset_confirm_modal = show_reset_confirm_modal.clone();
    let modal_start_from_scratch = modal_start_from_scratch.clone();
    let show_expert_init = show_expert_init.clone();
    let config_version = config_version.clone();
    let active_tab = active_tab.clone();
    let machines = machines.clone();
    let filaments = filaments.clone();
    let processes = processes.clone();
    let conflicts = conflicts.clone();
    let pending_updates = pending_updates.clone();
    let scanned_entries = scanned_entries.clone();
    let rules = rules.clone();
    let execution_logs = execution_logs.clone();
    let detected_hardware_summary = detected_hardware_summary.clone();
    let bound_filename = bound_filename.clone();
    let bound_path_hint = bound_path_hint.clone();
    let last_saved_time = last_saved_time.clone();
    let disk_flush_matrix = disk_flush_matrix.clone();
    let disk_flush_slot_count = disk_flush_slot_count.clone();
    let toast = toast_message.clone();

    Callback::from(move |_| {
      FsManager::clear_state_from_storage();
      show_reset_confirm_modal.set(false);
      is_config_loaded.set(false);
      show_bind_modal.set(true);
      modal_start_from_scratch.set(true);
      show_expert_init.set(false);
      config_version.set(1);
      active_tab.set(ActiveTab::Home);
      machines.set(Vec::new());
      filaments.set(Vec::new());
      processes.set(Vec::new());
      conflicts.set(Vec::new());
      pending_updates.set(Vec::new());
      scanned_entries.set(Vec::new());
      rules.set(Vec::new());
      execution_logs.set(Vec::new());
      disk_flush_matrix.set(Vec::new());
      disk_flush_slot_count.set(0);
      detected_hardware_summary.set("Unconnected".to_string());
      bound_filename.set("No Config Loaded".to_string());
      bound_path_hint.set(
        "All state cleared — nothing is loaded. Start From Scratch (Recommended), Load Orca WebConfig JSON, or use Expert Mode."
          .to_string(),
      );
      last_saved_time.set("Reset".to_string());
      toast.set(Some(
        "Reset All confirmed: cleared browser site data, rigs, filaments, and transformations and reopened Initialization."
          .to_string(),
      ));
    })
  };

  let on_complete_scratch_wizard = {
    let is_config_loaded = is_config_loaded.clone();
    let show_bind_modal = show_bind_modal.clone();
    let modal_start_from_scratch = modal_start_from_scratch.clone();
    let machines = machines.clone();
    let active_machine_id = active_machine_id.clone();
    let filaments = filaments.clone();
    let active_filament_type = active_filament_type.clone();
    let processes = processes.clone();
    let active_process_id = active_process_id.clone();
    let base_profile = base_profile.clone();
    let rules = rules.clone();
    let conflicts = conflicts.clone();
    let pending_updates = pending_updates.clone();
    let scanned_entries = scanned_entries.clone();
    let detected_hardware_summary = detected_hardware_summary.clone();
    let bound_filename = bound_filename.clone();
    let bound_path_hint = bound_path_hint.clone();
    let last_saved_time = last_saved_time.clone();
    let disk_flush_matrix = disk_flush_matrix.clone();
    let disk_flush_slot_count = disk_flush_slot_count.clone();
    let toast = toast_message.clone();

    Callback::from(move |out: ScratchWizardOutput| {
      let rig_id = out.rig.id.clone();
      if let Some(first_fil) = out.filaments.first() {
        active_filament_type.set(first_fil.material_type.clone());
      }
      if let Some(first_proc) = out.processes.first() {
        active_process_id.set(first_proc.id.clone());
      }
      active_machine_id.set(rig_id);
      machines.set(vec![out.rig]);
      filaments.set(out.filaments);
      processes.set(out.processes);
      base_profile.set(out.base_profile);
      rules.set(out.rules);
      conflicts.set(Vec::new());
      pending_updates.set(out.initial_pending_updates);
      scanned_entries.set(Vec::new());
      disk_flush_matrix.set(Vec::new());
      disk_flush_slot_count.set(0);
      detected_hardware_summary.set(out.summary_label.clone());
      bound_filename.set("Created From Scratch (Unsaved)".to_string());
      bound_path_hint.set(format!(
        "Start-From-Scratch Setup Wizard: {} — Click 'Save to Disk' to write to your local slicer directory",
        out.summary_label
      ));
      last_saved_time.set("Unsaved Wizard Stack".to_string());
      is_config_loaded.set(true);
      show_bind_modal.set(false);
      modal_start_from_scratch.set(false);

      toast.set(Some(format!(
        "Built clean transformation stack from scratch: {}! Click 'Save to Disk' when ready to write files.",
        out.summary_label
      )));
    })
  };

  let on_toggle_rule = {
    let rules = rules.clone();
    Callback::from(move |rule_id: String| {
      let mut list = (*rules).clone();
      if let Some(r) = list.iter_mut().find(|r| r.id == rule_id) {
        r.enabled = !r.enabled;
      }
      rules.set(list);
    })
  };

  let on_update_rule_lua = {
    let rules = rules.clone();
    Callback::from(move |(rule_id, new_lua): (String, String)| {
      let mut list = (*rules).clone();
      if let Some(r) = list.iter_mut().find(|r| r.id == rule_id) {
        r.lua_code = new_lua;
      }
      rules.set(list);
    })
  };

  // Requirement #5: Duplicate the current rig so the user can modify it (e.g. change nozzle or add-ons)
  let on_duplicate_active_rig = {
    let machines = machines.clone();
    let active_machine_id = active_machine_id.clone();
    let toast = toast_message.clone();

    Callback::from(move |_| {
      let mut list = (*machines).clone();
      if let Some(curr) = list
        .iter()
        .find(|m| m.id == *active_machine_id)
        .cloned()
        .or_else(|| list.first().cloned())
      {
        let copy_idx = list.len() + 1;
        let new_id = format!("{}_copy_{}", curr.id, copy_idx);
        for m in &mut list {
          m.is_active = false;
        }
        let mut dup = curr;
        dup.id = new_id.clone();
        dup.source_disk_profile = None;
        dup.is_active = true;
        dup.refresh_title_and_notes(true);
        let new_name = format!("{} #{}", dup.name, copy_idx);
        dup.name = new_name.clone();
        list.push(dup);
        machines.set(list);
        active_machine_id.set(new_id);
        toast.set(Some(format!(
          "Duplicated rig as '{}'! You can now change its nozzle, machine type, or cooling add-ons.",
          new_name
        )));
      }
    })
  };

  let on_change_machine_type = {
    let machines = machines.clone();
    let active_machine_id = active_machine_id.clone();
    let toast = toast_message.clone();

    Callback::from(move |new_model: String| {
      let mut list = (*machines).clone();
      if let Some(m) = list.iter_mut().find(|m| m.id == *active_machine_id) {
        m.printer_model = new_model.clone();
        match new_model.as_str() {
          "Anycubic Kobra 3" => {
            m.kinematics = "Cartesian High-Speed Bedslinger".to_string();
            m.bed_size = "250 × 250 × 260 mm".to_string();
            m.firmware = "Kobra OS (Klipper-derived)".to_string();
          }
          "Voron 2.4 R2 (350mm)" => {
            m.kinematics = "CoreXY Flying Gantry".to_string();
            m.bed_size = "350 × 350 × 330 mm".to_string();
            m.firmware = "Klipper v0.12 Mainline".to_string();
          }
          _ => {
            m.kinematics = "CoreXY Enclosed".to_string();
            m.bed_size = "250 × 250 × 250 mm".to_string();
            m.firmware = "Kobra OS 2.4 (Klipper)".to_string();
          }
        }
        m.refresh_title_and_notes(m.source_disk_profile.is_none());
      }
      machines.set(list);
      toast.set(Some(format!("Updated rig machine type to {}!", new_model)));
    })
  };

  let on_toggle_addon = {
    let machines = machines.clone();
    let active_machine_id = active_machine_id.clone();
    let toast = toast_message.clone();

    Callback::from(move |addon_id: String| {
      let mut list = (*machines).clone();
      if let Some(m) = list.iter_mut().find(|m| m.id == *active_machine_id) {
        if let Some(pos) = m.addons.iter().position(|a| a == &addon_id) {
          m.addons.remove(pos);
          toast.set(Some(format!(
            "Removed '{}' add-on from rig. Cooling/retraction schedule restored.",
            addon_id
          )));
        } else {
          m.addons.push(addon_id.clone());
          toast.set(Some(format!(
            "Installed '{}' add-on on rig! Lua cooling/retraction schedule updated.",
            addon_id
          )));
        }
        m.refresh_title_and_notes(m.source_disk_profile.is_none());
      }
      machines.set(list);
    })
  };

  let on_change_nozzle = {
    let machines = machines.clone();
    let active_machine_id = active_machine_id.clone();
    let filaments = filaments.clone();
    let rules = rules.clone();
    let toast = toast_message.clone();

    Callback::from(move |new_nozzle_type: String| {
      let mut mach_list = (*machines).clone();
      let mut r_list = (*rules).clone();
      let mut fil_list = (*filaments).clone();
      if let Some(m) = mach_list.iter_mut().find(|m| m.id == *active_machine_id) {
        m.nozzle_type = new_nozzle_type.clone();
        match new_nozzle_type.as_str() {
          "diamondback" | "microswiss_diamondback" => {
            m.nozzle_material = "MicroSwiss Diamondback (PCD)".to_string();
            m.nozzle_name = format!("MicroSwiss Diamondback PCD {}0mm", m.nozzle_diameter);
            toast.set(Some(
              "Installed MicroSwiss Diamondback PCD Nozzle (nozzle_type=brass, HRC 68): all default nozzle & thermal rules applied (-10°C, 0.94 Flow, 0.60mm Retract)!"
                .to_string(),
            ));
          }
          "microswiss_brass" | "microswiss_mk8_brass" => {
            m.nozzle_material = "Brass (MicroSwiss TwinClad XT)".to_string();
            m.nozzle_name = format!("MicroSwiss Plated Brass {}0mm", m.nozzle_diameter);
            toast.set(Some(
              "Installed MicroSwiss Plated Brass Nozzle (TwinClad XT™, nozzle_type=brass, HRC 35): default rules applied (0°C offset, 0.99 Flow, 0.70mm Retract)!"
                .to_string(),
            ));
          }
          "brass" | "orca_brass" => {
            m.nozzle_material = "Brass".to_string();
            m.nozzle_name = format!("Standard Brass {}0mm", m.nozzle_diameter);
            toast.set(Some(
              "Installed Standard Brass Nozzle (nozzle_type=brass, HRC 0): default rules applied with 0°C factory thermal baseline.".to_string(),
            ));
          }
          "ruby_brass" => {
            m.nozzle_material = "Ruby Tip (Brass Body)".to_string();
            m.nozzle_name = format!("Brass-Body Ruby Tip {}0mm (CF/GF Ready)", m.nozzle_diameter);
            toast.set(Some(
              "Installed Brass-Body Ruby Tip Nozzle (nozzle_type=brass, HRC 60): 0°C Brass thermal conductivity + HRC 60 Ruby gemstone tip enabled for -CF and -GF composite materials!"
                .to_string(),
            ));
          }
          "hardened_steel" | "orca_hardened_steel" => {
            m.nozzle_material = "Hardened Steel".to_string();
            m.nozzle_name = format!("Hardened Steel Wear-Resistant {}0mm", m.nozzle_diameter);
            toast.set(Some(
              "Installed Hardened Steel Nozzle (nozzle_type=hardened_steel, HRC 55): default rules applied automatically (+5°C thermal compensation)!"
                .to_string(),
            ));
          }
          "cm2" | "microswiss_cm2" => {
            m.nozzle_material = "MicroSwiss CM2 (Copper + M2 HSS)".to_string();
            m.nozzle_name = format!("MicroSwiss CM2 Hardened Tip {}0mm", m.nozzle_diameter);
            toast.set(Some(
              "Installed MicroSwiss CM2™ Nozzle (Copper Core + M2 HSS Tip, HRC 55): 0°C copper conductivity + 0.60mm retract applied!"
                .to_string(),
            ));
          }
          "cht_cm2" | "microswiss_cht_cm2" => {
            m.nozzle_material = "MicroSwiss CHT CM2 (High-Flow Copper + M2 HSS)".to_string();
            m.nozzle_name = format!(
              "MicroSwiss CHT CM2 High-Flow Hardened {}0mm",
              m.nozzle_diameter
            );
            toast.set(Some(
              "Installed MicroSwiss CHT CM2™ High-Flow Hardened Nozzle (HRC 55): +40% high-flow splitter + 0°C copper conductivity applied!"
                .to_string(),
            ));
          }
          _ => {
            m.nozzle_material = "Bi-Metal CHT".to_string();
            m.nozzle_name = format!("Bi-Metal CHT High-Flow {}0mm", m.nozzle_diameter);
            toast.set(Some(
              "Installed High-Flow CHT Nozzle (nozzle_type=brass, HRC 55): default rules applied automatically (+25% volumetric capacity)!"
                .to_string(),
            ));
          }
        }
        if !MachineRig::is_diameter_supported_for_nozzle(&m.nozzle_type, &m.nozzle_diameter) {
          m.nozzle_diameter = "0.4".to_string();
        }
        TransformationEngine::sync_rules_for_rig(
          &mut r_list,
          &m.nozzle_type,
          &m.nozzle_material,
          m.ace_pro_units,
        );
        TransformationEngine::sync_weather_rule_for_rig(&mut r_list, m);
        let (_, mach_hrc) = m.orca_machine_nozzle_type_and_hrc();
        m.nozzle_hrc = mach_hrc;
        m.refresh_title_and_notes(m.source_disk_profile.is_none());
        let short_noz = m.short_nozzle_tag();
        let diam_str = m.nozzle_diameter.clone();
        for sp in fil_list.iter_mut() {
          sp.recompute_for_nozzle(&new_nozzle_type);
          if sp.id.starts_with("spool_scratch_") || sp.id.starts_with("spool_catalog_") {
            if let Some((prefix, _)) = sp
              .name
              .split_once(" • ")
              .or_else(|| sp.name.rsplit_once(" - "))
            {
              sp.name = format!("{} • {} {}", prefix, diam_str, short_noz);
            }
          }
        }
      }
      machines.set(mach_list);
      filaments.set(fil_list);
      rules.set(r_list);
    })
  };

  let on_change_nozzle_diameter = {
    let machines = machines.clone();
    let active_machine_id = active_machine_id.clone();
    let processes = processes.clone();
    let active_process_id = active_process_id.clone();
    let toast = toast_message.clone();

    Callback::from(move |new_diam: String| {
      let mut mach_list = (*machines).clone();
      let mut short_mach = "KS1".to_string();
      if let Some(m) = mach_list.iter_mut().find(|m| m.id == *active_machine_id) {
        m.nozzle_diameter = new_diam.clone();
        m.refresh_title_and_notes(m.source_disk_profile.is_none());
        short_mach = m.short_model_tag();
      }
      machines.set(mach_list);

      let mut proc_list = (*processes).clone();
      if let Some(p) = proc_list.iter_mut().find(|p| p.id == *active_process_id) {
        p.line_width = match new_diam.as_str() {
          "0.2" => 0.22,
          "0.25" => 0.26,
          "0.3" => 0.32,
          "0.6" => 0.62,
          "0.8" => 0.84,
          "1.0" => 1.05,
          _ => 0.42,
        };
        if !p.id.starts_with("disk_proc_") {
          p.name = p.format_concise_title(&short_mach);
        }
        p.notes = p.generate_descriptive_notes();
      }
      processes.set(proc_list);
      toast.set(Some(format!(
        "Nozzle diameter set to {}mm. Lua Volumetric Speed Ceiling dynamically recalculated!",
        new_diam
      )));
    })
  };

  let on_change_ace_units = {
    let machines = machines.clone();
    let active_machine_id = active_machine_id.clone();
    let toast = toast_message.clone();

    Callback::from(move |units: usize| {
      let mut mach_list = (*machines).clone();
      if let Some(m) = mach_list.iter_mut().find(|m| m.id == *active_machine_id) {
        m.ace_pro_units = units;
        m.refresh_title_and_notes(m.source_disk_profile.is_none());
      }
      machines.set(mach_list);

      let label = if units == 2 {
        "Double ACE Pro 2 (8 Slots / up to 8×8 Flushing Matrix)"
      } else {
        "Single ACE Pro (4 Slots / up to 4×4 Flushing Matrix)"
      };
      toast.set(Some(format!("Switched multi-material bank to {}!", label)));
    })
  };

  let on_change_weather = {
    let machines = machines.clone();
    let active_machine_id = active_machine_id.clone();
    let rules = rules.clone();
    let toast = toast_message.clone();

    Callback::from(move |(preset, temp_c, rh_pct): (String, i32, i32)| {
      let mut mach_list = (*machines).clone();
      let mut r_list = (*rules).clone();
      if let Some(m) = mach_list.iter_mut().find(|m| m.id == *active_machine_id) {
        if preset == "custom" {
          m.apply_weather_dials(temp_c, rh_pct);
        } else {
          m.apply_weather_preset(&preset);
        }
        m.refresh_title_and_notes(m.source_disk_profile.is_none());
        TransformationEngine::sync_weather_rule_for_rig(&mut r_list, m);
        let badge = m.climate_summary_badge();
        toast.set(Some(format!(
          "Updated rig weather to {} — relative Weather & Ambient Climate Lua rule synced!",
          badge
        )));
      }
      machines.set(mach_list);
      rules.set(r_list);
    })
  };

  let on_select_active_filament = {
    let active_filament_type = active_filament_type.clone();
    let toast = toast_message.clone();
    Callback::from(move |mat: String| {
      active_filament_type.set(mat.clone());
      toast.set(Some(format!(
        "Active slicing material set to {}. Lua Volumetric Speed Ceiling & rules updated!",
        mat
      )));
    })
  };

  let on_update_spool = {
    let filaments = filaments.clone();
    let toast = toast_message.clone();
    Callback::from(move |updated: FilamentSpool| {
      let mut list = (*filaments).clone();
      let mut changed_mat_msg = None;
      if let Some(pos) = list.iter().position(|s| s.id == updated.id) {
        let is_mat_change = list[pos].material_type != updated.material_type;
        if is_mat_change {
          changed_mat_msg = Some(format!(
            "Updated '{}' material to {} ({}°C nozzle / {}°C bed) & recalculated flushing matrix!",
            updated.format_concise_title(),
            updated.material_type,
            updated.print_temp,
            updated.bed_temp
          ));
        }
        list[pos] = updated;
      }
      filaments.set(list);
      if let Some(msg) = changed_mat_msg {
        toast.set(Some(msg));
      }
    })
  };

  let on_swap_slot_spool = {
    let filaments = filaments.clone();
    let toast = toast_message.clone();
    Callback::from(move |(slot_idx, target_idx): (usize, usize)| {
      let mut list = (*filaments).clone();
      if slot_idx < list.len() && target_idx < list.len() && slot_idx != target_idx {
        list.swap(slot_idx, target_idx);
        let assigned_name = list[slot_idx].format_concise_title();
        filaments.set(list);
        toast.set(Some(format!(
          "Moved '{}' to Slot #{} and recalculated Flushing Volume Matrix!",
          assigned_name,
          slot_idx + 1
        )));
      }
    })
  };

  let on_add_spool = {
    let filaments = filaments.clone();
    let active_filament_type = active_filament_type.clone();
    let toast = toast_message.clone();
    Callback::from(move |new_spool: FilamentSpool| {
      let mut list = (*filaments).clone();
      let title = new_spool.format_concise_title();
      if list.is_empty() {
        active_filament_type.set(new_spool.material_type.clone());
      }
      list.push(new_spool);
      let new_slot_num = list.len();
      filaments.set(list);
      toast.set(Some(format!(
        "Added spool '{}' (#{}) to filament bank!",
        title, new_slot_num
      )));
    })
  };

  let on_remove_spool = {
    let filaments = filaments.clone();
    let active_filament_type = active_filament_type.clone();
    let toast = toast_message.clone();
    Callback::from(move |remove_idx: usize| {
      let mut list = (*filaments).clone();
      if remove_idx < list.len() {
        let removed = list.remove(remove_idx);
        if let Some(first) = list.first() {
          if !list
            .iter()
            .any(|s| s.material_type.eq_ignore_ascii_case(&active_filament_type))
          {
            active_filament_type.set(first.material_type.clone());
          }
        }
        filaments.set(list);
        toast.set(Some(format!(
          "Removed spool '{}' (#{}) from filament bank.",
          removed.format_concise_title(),
          remove_idx + 1
        )));
      }
    })
  };

  let on_clear_spools = {
    let filaments = filaments.clone();
    let toast = toast_message.clone();
    Callback::from(move |_| {
      filaments.set(Vec::new());
      toast.set(Some("Cleared all filament spools.".to_string()));
    })
  };

  let on_set_active_machine = {
    let active_machine_id = active_machine_id.clone();
    let machines = machines.clone();
    let filaments = filaments.clone();
    let rules = rules.clone();
    let detected_hardware_summary = detected_hardware_summary.clone();
    let toast = toast_message.clone();
    Callback::from(move |mid: String| {
      active_machine_id.set(mid.clone());
      let mut list = (*machines).clone();
      let mut fil_list = (*filaments).clone();
      let mut r_list = (*rules).clone();
      for m in &mut list {
        m.is_active = m.id == mid;
        if m.is_active {
          TransformationEngine::sync_rules_for_rig(
            &mut r_list,
            &m.nozzle_type,
            &m.nozzle_material,
            m.ace_pro_units,
          );
          TransformationEngine::sync_weather_rule_for_rig(&mut r_list, m);
          for sp in fil_list.iter_mut() {
            sp.recompute_for_nozzle(&m.nozzle_type);
          }
          detected_hardware_summary.set(format!(
            "{} ({}) with {} ({}mm) Nozzle",
            m.printer_model, m.name, m.nozzle_material, m.nozzle_diameter
          ));
        }
      }
      machines.set(list);
      filaments.set(fil_list);
      rules.set(r_list);
      toast.set(Some(
        "Active machine rig updated, filament temperatures recomputed for active nozzle, and default Lua rules applied!"
          .to_string(),
      ));
    })
  };

  let on_select_process = {
    let active_process_id = active_process_id.clone();
    let toast = toast_message.clone();
    Callback::from(move |pid: String| {
      active_process_id.set(pid);
      toast.set(Some(
        "Process / Speed profile updated. Volumetric speed ceiling recalculated!".to_string(),
      ));
    })
  };

  // Export Standalone OrcaSlicer Filament/Machine .json Preset (Artifact #1)
  let on_export_standalone_json = {
    let active_rig = active_rig.clone();
    let active_slots = active_slots.clone();
    let toast = toast_message.clone();

    Callback::from(move |_| {
      let spool = active_slots.first().cloned().unwrap_or_else(|| {
        let entries = build_reference_anycubic_entries();
        import_scanned_directory(&entries)
          .unwrap()
          .filaments
          .remove(0)
      });
      let json_content = export_standalone_filament_json(&spool, &active_rig);
      let safe_name = crate::models::sanitize_profile_filename(&spool.name);
      if let Err(e) = FsManager::trigger_browser_download(&safe_name, &json_content) {
        toast.set(Some(format!("Download error: {}", e)));
      } else {
        toast.set(Some(format!(
          "Exported standalone OrcaSlicer delta preset '{}'!",
          safe_name
        )));
      }
    })
  };

  let on_export_spool_json = {
    let active_rig = active_rig.clone();
    let toast = toast_message.clone();

    Callback::from(move |spool: FilamentSpool| {
      let json_content = export_standalone_filament_json(&spool, &active_rig);
      let safe_name = crate::models::sanitize_profile_filename(&spool.name);
      let _ = FsManager::trigger_browser_download(&safe_name, &json_content);
      toast.set(Some(format!(
        "Exported standalone OrcaSlicer filament preset '{}'!",
        safe_name
      )));
    })
  };

  // Export .3mf Project Container (Artifact #2)
  let on_export_3mf = {
    let active_rig = active_rig.clone();
    let active_proc = active_proc.clone();
    let active_slots = active_slots.clone();
    let flush_matrix = eval_result.flush_matrix.clone();
    let derived_overrides = eval_result.resulting_profile.values.clone();
    let toast = toast_message.clone();

    Callback::from(move |_| {
      let slots = if active_slots.is_empty() {
        let entries = build_reference_anycubic_entries();
        import_scanned_directory(&entries)
          .unwrap()
          .filaments
          .into_iter()
          .take(4)
          .collect()
      } else {
        active_slots.clone()
      };
      let zip_bytes = build_3mf_project_container(
        &active_rig,
        &active_proc,
        &slots,
        &flush_matrix,
        &derived_overrides,
      );
      let filename = format!("Kobra_ACEPro_{}Slot_Loadout.3mf", slots.len());
      match FsManager::trigger_binary_download(
        &filename,
        &zip_bytes,
        "application/vnd.ms-package.3dmanufacturing-3dmodel+xml",
      ) {
        Ok(()) => {
          toast.set(Some(format!(
            "Exported '{}' with {}-slot ACE Pro arrays & {}×{} flush_volumes_matrix!",
            filename,
            slots.len(),
            slots.len(),
            slots.len()
          )));
        }
        Err(e) => {
          toast.set(Some(format!("3MF export error: {}", e)));
        }
      }
    })
  };

  // Save Changes to Disk: synchronizes all added/edited OC: filaments, rigs, processes, and staged `pending_updates` and writes directly via `/api/local-slicer-save`
  let on_commit_save = {
    let last_saved_time = last_saved_time.clone();
    let scanned_entries = scanned_entries.clone();
    let machines = machines.clone();
    let active_machine_id = active_machine_id.clone();
    let filaments = filaments.clone();
    let processes = processes.clone();
    let active_process_id = active_process_id.clone();
    let flush_matrix = eval_result.flush_matrix.clone();
    let flush_multiplier = *flush_multiplier;
    let conf_relative_path = conf_relative_path.clone();
    let pending_updates = pending_updates.clone();
    let derived_overrides = eval_result.resulting_profile.values.clone();
    let toast = toast_message.clone();

    Callback::from(move |_| {
      let lst = last_saved_time.clone();
      let se_state = scanned_entries.clone();
      let pu_state = pending_updates.clone();
      let t = toast.clone();

      let mut entries_copy = (*scanned_entries).clone();
      let staged_list = (*pending_updates).clone();
      let modified_files = sync_live_state_to_entries(
        &mut entries_copy,
        &machines,
        &active_machine_id,
        &filaments,
        &processes,
        &active_process_id,
        &flush_matrix,
        flush_multiplier,
        &conf_relative_path,
        &staged_list,
        &derived_overrides,
      );

      spawn_local(async move {
        let req_payload = LocalFsSaveRequest {
          files: modified_files.clone(),
        };
        let mut saved_via_api = false;
        if let Ok(req_builder) =
          gloo_net::http::Request::post(&FsManager::api_url("/api/local-slicer-save"))
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(&req_payload).unwrap_or_default())
        {
          if let Ok(resp) = req_builder.send().await {
            if resp.ok() {
              if let Ok(save_res) = resp.json::<LocalFsSaveResponse>().await {
                saved_via_api = save_res.ok;
              }
            }
          }
        }

        se_state.set(entries_copy);
        pu_state.set(Vec::new());
        lst.set("Saved to Local Disk".to_string());

        if saved_via_api {
          t.set(Some(format!(
            "Saved {} profile & configuration file(s) directly to local disk!",
            modified_files.len()
          )));
        } else {
          t.set(Some(format!(
            "Synchronized {} profile & configuration file(s) to configuration state!",
            modified_files.len()
          )));
        }
      });
    })
  };

  // Requirement #4: Resolve flagged difference by either accepting it as a one-off Lua transformation rule
  // or staging the clean rule value to be saved when the user clicks Save to Disk!
  let on_resolve_conflict = {
    let conflicts = conflicts.clone();
    let pending_updates = pending_updates.clone();
    let rules = rules.clone();
    let filaments = filaments.clone();
    let machines = machines.clone();
    let processes = processes.clone();
    let last_saved_time = last_saved_time.clone();
    let toast = toast_message.clone();

    Callback::from(move |(param_key, accept_as_transform): (String, bool)| {
      let mut cur_conflicts = (*conflicts).clone();
      if let Some(pos) = cur_conflicts.iter().position(|c| c.param_key == param_key) {
        let conf = cur_conflicts.remove(pos);
        if accept_as_transform {
          let new_rule = TransformationEngine::create_oneoff_rule_from_conflict(&conf);
          let mut r_list = (*rules).clone();
          TransformationEngine::normalize_stack_order(&mut r_list);
          r_list.push(new_rule.clone());
          TransformationEngine::reindex_priorities(&mut r_list);
          rules.set(r_list);
          toast.set(Some(format!(
            "Created & enabled Lua One-Off Transformation '{}' ({} = {})!",
            new_rule.name, conf.param_key, conf.actual_external_value
          )));
        } else {
          // Update our internal model to the clean rule value and stage a PendingDiskUpdate for Save!
          if conf.param_key.starts_with("filament:") {
            let mut fil_list = (*filaments).clone();
            for sp in &mut fil_list {
              if conf.param_key.contains(&sp.name) {
                if conf.json_field_key == "nozzle_temperature_HS" {
                  if let Ok(clean_t) = conf.clean_raw_value.parse::<i32>() {
                    sp.print_temp = clean_t;
                  }
                } else if conf.json_field_key == "textured_plate_temp" {
                  if let Ok(clean_b) = conf.clean_raw_value.parse::<i32>() {
                    sp.bed_temp = clean_b;
                  }
                }
                for d in &mut sp.deltas_from_base {
                  if d.param_key == conf.json_field_key {
                    d.reconstructed_by_lua = true;
                    d.rule_explanation = format!(
                      "Staged Clean Rule Value ({}) — Pending Save to Disk",
                      conf.clean_raw_value
                    );
                  }
                }
              }
            }
            filaments.set(fil_list);
          } else if conf.param_key.starts_with("machine:") {
            let mut m_list = (*machines).clone();
            for m in &mut m_list {
              if conf.param_key.contains(&m.name) {
                if conf.json_field_key == "retraction_length" {
                  if let Ok(clean_len) = conf.clean_raw_value.parse::<f64>() {
                    m.base_retraction_length = clean_len;
                  }
                }
              }
            }
            machines.set(m_list);
          } else if conf.param_key.starts_with("process:") {
            let mut p_list = (*processes).clone();
            for p in &mut p_list {
              if conf.param_key.contains(&p.name) {
                p.wall_generator = "classic".to_string();
                p.print_sequence = "by layer".to_string();
              }
            }
            processes.set(p_list);
          }

          let candidate_upd = PendingDiskUpdate {
            param_key: conf.param_key.clone(),
            param_label: conf.param_label.clone(),
            source_relative_path: conf.source_relative_path.clone(),
            json_field_key: conf.json_field_key.clone(),
            old_disk_value: conf.actual_external_value.clone(),
            new_clean_value: conf.clean_raw_value.clone(),
          };
          if candidate_upd.has_value_change() {
            let mut pend = (*pending_updates).clone();
            pend.push(candidate_upd);
            pending_updates.set(pend);
            last_saved_time.set("Unsaved Staged Changes".to_string());

            toast.set(Some(format!(
              "Staged Clean Rule Value for {} ({} → {}). Click 'Save to Disk' when ready to write to disk!",
              conf.param_label, conf.actual_external_value, conf.expected_derived_value
            )));
          } else {
            toast.set(Some(format!(
              "Verified Clean Rule Value for {} (matches disk value {}, no disk write required).",
              conf.param_label, conf.actual_external_value
            )));
          }
        }
        conflicts.set(cur_conflicts);
      }
    })
  };

  let on_export_package = {
    let rules = rules.clone();
    let active_machine_id = active_machine_id.clone();
    let toast = toast_message.clone();
    Callback::from(move |_| {
      let pkg = TransformationEngine::export_package(&rules, &active_machine_id);
      if let Err(e) = FsManager::trigger_browser_download("orcaslicer_lua_stack.json", &pkg) {
        toast.set(Some(format!("Export error: {}", e)));
      } else {
        toast.set(Some(
          "1-Click Export: saved 'orcaslicer_lua_stack.json'!".to_string(),
        ));
      }
    })
  };

  let on_create_rule = {
    let rules = rules.clone();
    let toast = toast_message.clone();
    Callback::from(move |new_rule: TransformationRule| {
      let mut list = (*rules).clone();
      TransformationEngine::normalize_stack_order(&mut list);
      let rule_name = new_rule.name.clone();
      let rule_scope = new_rule.condition_expr.clone();
      list.push(new_rule);
      TransformationEngine::reindex_priorities(&mut list);
      let step_pos = list.len();
      rules.set(list);
      toast.set(Some(format!(
        "Added & enabled Lua transformation rule '{}' at Step #{} (bottom of stack, Scope: {})!",
        rule_name, step_pos, rule_scope
      )));
    })
  };

  let on_delete_rule = {
    let rules = rules.clone();
    let toast = toast_message.clone();
    Callback::from(move |rule_id: String| {
      let mut list = (*rules).clone();
      list.retain(|r| r.id != rule_id);
      TransformationEngine::normalize_stack_order(&mut list);
      rules.set(list);
      toast.set(Some(format!(
        "Removed transformation rule '{}' from stack and re-indexed pipeline steps.",
        rule_id
      )));
    })
  };

  let on_reorder_rule = {
    let rules = rules.clone();
    let toast = toast_message.clone();
    Callback::from(move |(rule_id, action): (String, StackReorderAction)| {
      let mut list = (*rules).clone();
      let msg = TransformationEngine::reorder_rule(&mut list, &rule_id, action);
      rules.set(list);
      toast.set(Some(msg));
    })
  };

  let on_save_rig_to_vault = {
    let toast = toast_message.clone();
    Callback::from(move |_| {
      toast.set(Some("Saved active rig configuration to vault!".to_string()));
    })
  };

  let on_clear_logs = {
    let execution_logs = execution_logs.clone();
    Callback::from(move |_| {
      execution_logs.set(Vec::new());
    })
  };

  // Guard Save / Export actions with a confirmation modal whenever unsafe or weird values exist
  let has_safety_warnings = !safety_warnings.is_empty();
  let danger_warning_count = safety_warnings
    .iter()
    .filter(|w| w.severity == SafetySeverity::Danger)
    .count();
  let anomaly_warning_count = safety_warnings
    .iter()
    .filter(|w| w.severity == SafetySeverity::Warning)
    .count();

  let on_commit_save_guarded = {
    let pca = pending_confirmed_action.clone();
    let direct = on_commit_save.clone();
    Callback::from(move |_| {
      if has_safety_warnings {
        pca.set(Some(PendingConfirmedAction::SaveToDisk));
      } else {
        direct.emit(());
      }
    })
  };

  let on_export_3mf_guarded = {
    let pca = pending_confirmed_action.clone();
    let direct = on_export_3mf.clone();
    Callback::from(move |_| {
      if has_safety_warnings {
        pca.set(Some(PendingConfirmedAction::Export3mf));
      } else {
        direct.emit(());
      }
    })
  };

  let on_export_webconfig_json_guarded = {
    let pca = pending_confirmed_action.clone();
    let direct = on_export_webconfig_json.clone();
    Callback::from(move |_| {
      if has_safety_warnings {
        pca.set(Some(PendingConfirmedAction::ExportWebConfigJson));
      } else {
        direct.emit(());
      }
    })
  };

  let on_backup_zip_guarded = {
    let pca = pending_confirmed_action.clone();
    let direct = on_backup_zip.clone();
    Callback::from(move |_| {
      if has_safety_warnings {
        pca.set(Some(PendingConfirmedAction::BackupZip));
      } else {
        direct.emit(());
      }
    })
  };

  let on_export_standalone_json_guarded = {
    let pca = pending_confirmed_action.clone();
    let direct = on_export_standalone_json.clone();
    Callback::from(move |_| {
      if has_safety_warnings {
        pca.set(Some(PendingConfirmedAction::ExportStandaloneJson));
      } else {
        direct.emit(());
      }
    })
  };

  let on_export_package_guarded = {
    let pca = pending_confirmed_action.clone();
    let direct = on_export_package.clone();
    Callback::from(move |_| {
      if has_safety_warnings {
        pca.set(Some(PendingConfirmedAction::ExportLuaPackage));
      } else {
        direct.emit(());
      }
    })
  };

  let on_confirm_pending_action = {
    let pca = pending_confirmed_action.clone();
    let save_cb = on_commit_save.clone();
    let export_3mf_cb = on_export_3mf.clone();
    let export_json_cb = on_export_webconfig_json.clone();
    let backup_zip_cb = on_backup_zip.clone();
    let standalone_cb = on_export_standalone_json.clone();
    let pkg_cb = on_export_package.clone();
    Callback::from(move |_| {
      if let Some(act) = (*pca).clone() {
        pca.set(None);
        match act {
          PendingConfirmedAction::SaveToDisk => save_cb.emit(()),
          PendingConfirmedAction::Export3mf => export_3mf_cb.emit(()),
          PendingConfirmedAction::ExportWebConfigJson => export_json_cb.emit(()),
          PendingConfirmedAction::BackupZip => backup_zip_cb.emit(()),
          PendingConfirmedAction::ExportStandaloneJson => standalone_cb.emit(()),
          PendingConfirmedAction::ExportLuaPackage => pkg_cb.emit(()),
        }
      }
    })
  };

  let on_open_export_modal = {
    let sem = show_export_modal.clone();
    Callback::from(move |_| sem.set(true))
  };

  html! {
    <div class="min-h-screen bg-surface text-on-surface font-sans flex flex-col">
      // Navigation Bar
      <NavigationBar
        active_tab={*active_tab}
        on_select_tab={on_select_tab}
        is_config_loaded={*is_config_loaded}
        is_tauri={is_tauri}
        config_version={*config_version}
        bound_filename={(*bound_filename).clone()}
        active_rig_name={active_rig.name.clone()}
        ace_pro_units={active_rig.ace_pro_units}
        on_open_export_modal={on_open_export_modal.clone()}
        on_reset_all={on_reset_all.clone()}
        has_unapplied_changes={!pending_updates.is_empty()}
        pending_save_count={pending_updates.len()}
      />

      // Export (Website) / Save & Export (Tauri) Popup Modal Menu
      if *show_export_modal {
        <div class="fixed inset-0 z-50 bg-black/80 flex items-center justify-center p-4 overflow-y-auto">
          <div class="bg-surface-container-low border-2 border-primary rounded-2xl max-w-2xl w-full p-space-xl shadow-2xl flex flex-col gap-space-md my-auto">
            <div class="flex items-start justify-between gap-space-md border-b border-surface-container pb-space-sm">
              <div class="flex flex-col gap-0.5">
                <span class="bg-primary text-on-primary text-[10px] font-mono font-bold uppercase px-2 py-0.5 rounded w-fit">
                  {if is_tauri { "Save & Export Menu" } else { "Export Menu" }}
                </span>
                <h2 class="font-headline-lg text-headline-sm text-on-surface font-bold mt-1">
                  {if is_tauri { "Save & Export Configuration" } else { "Export Configuration" }}
                </h2>
                <p class="text-xs text-on-surface-variant">
                  {if is_tauri {
                    "Choose an export format or write your configuration directly to disk."
                  } else {
                    "Choose an export format to download your configuration."
                  }}
                </p>
              </div>
              <button
                class="text-on-surface-variant hover:text-on-surface p-1 cursor-pointer"
                onclick={
                  let sem = show_export_modal.clone();
                  Callback::from(move |_| sem.set(false))
                }
              >
                <span class="material-symbols-outlined">{"close"}</span>
              </button>
            </div>

            <div class="flex flex-col gap-space-sm">
              // Option 1: Export Orca WebConfig (.json)
              <div class="bg-surface-container p-space-md rounded-xl border border-outline-variant flex flex-col sm:flex-row items-start sm:items-center justify-between gap-space-md">
                <div class="flex flex-col gap-1">
                  <div class="flex items-center gap-2">
                    <span class="font-code-md text-code-md text-primary font-bold">
                      {"Export Orca WebConfig (.json)"}
                    </span>
                    <span class="bg-primary/20 text-primary text-[10px] font-mono font-bold px-2 py-0.5 rounded">
                      {format!("Config v{} • OrcaSlicer v2.2", *config_version)}
                    </span>
                  </div>
                  <span class="text-xs text-on-surface">
                    {"Exports your full versioned OrcaSlicer WebConfig state (machine rigs, filaments, processes, Lua rules, and flush matrix)."}
                  </span>
                  <span class="text-[11px] font-mono text-on-surface-variant">
                    {"How to load: Click Reset All → Load Orca WebConfig (.json) in this app."}
                  </span>
                </div>
                <button
                  class="shrink-0 flex items-center gap-space-xs bg-primary-container hover:bg-primary text-on-primary-container px-space-md py-2 rounded-lg text-xs font-mono font-bold transition-colors cursor-pointer"
                  onclick={
                    let cb = on_export_webconfig_json_guarded.clone();
                    Callback::from(move |_| cb.emit(()))
                  }
                >
                  <span class="material-symbols-outlined text-[16px]">{"data_object"}</span>
                  {"Export Orca WebConfig"}
                </button>
              </div>

              // Option 2: Export OrcaSlicer Config (.zip)
              <div class="bg-surface-container p-space-md rounded-xl border border-outline-variant flex flex-col sm:flex-row items-start sm:items-center justify-between gap-space-md">
                <div class="flex flex-col gap-1">
                  <span class="font-code-md text-code-md text-secondary font-bold">
                    {"Export OrcaSlicer Config (.zip)"}
                  </span>
                  <span class="text-xs text-on-surface">
                    {"Exports a ZIP archive containing your custom OC - machine, process, and filament .json presets."}
                  </span>
                  <span class="text-[11px] font-mono text-on-surface-variant">
                    {"How to load: In OrcaSlicer / AnycubicSlicerNext, click File → Import → Import Configs... and select the .zip file."}
                  </span>
                </div>
                <button
                  class="shrink-0 flex items-center gap-space-xs bg-surface-container-highest hover:bg-surface-bright text-secondary border border-secondary/50 px-space-md py-2 rounded-lg text-xs font-mono font-bold transition-colors cursor-pointer"
                  onclick={
                    let cb = on_backup_zip_guarded.clone();
                    Callback::from(move |_| cb.emit(()))
                  }
                >
                  <span class="material-symbols-outlined text-[16px]">{"folder_zip"}</span>
                  {"Export OrcaSlicer Config"}
                </button>
              </div>

              // Option 3: Export OrcaSlicer 3MF Loadout (.3mf)
              <div class="bg-surface-container p-space-md rounded-xl border border-outline-variant flex flex-col sm:flex-row items-start sm:items-center justify-between gap-space-md">
                <div class="flex flex-col gap-1">
                  <span class="font-code-md text-code-md text-tertiary font-bold">
                    {"Export OrcaSlicer 3MF Loadout (.3mf)"}
                  </span>
                  <span class="text-xs text-on-surface">
                    {"Exports a 3MF project pre-loaded with your active machine, process, 8-slot filament loadout, and flushing volumes matrix."}
                  </span>
                  <span class="text-[11px] font-mono text-on-surface-variant">
                    {"How to load: In OrcaSlicer / AnycubicSlicerNext, click File → Open Project... (Ctrl+O) and select the .3mf file."}
                  </span>
                </div>
                <button
                  class="shrink-0 flex items-center gap-space-xs bg-surface-container-highest hover:bg-surface-bright text-tertiary border border-tertiary/50 px-space-md py-2 rounded-lg text-xs font-mono font-bold transition-colors cursor-pointer"
                  onclick={
                    let cb = on_export_3mf_guarded.clone();
                    Callback::from(move |_| cb.emit(()))
                  }
                >
                  <span class="material-symbols-outlined text-[16px]">{"deployed_code"}</span>
                  {"Export OrcaSlicer 3MF Loadout"}
                </button>
              </div>

              // Option 4: Save to Disk (Tauri Desktop Mode ONLY — removed in Website mode per Requirement #2)
              if is_tauri {
                <div class="bg-surface-container p-space-md rounded-xl border border-outline-variant flex flex-col sm:flex-row items-start sm:items-center justify-between gap-space-md">
                  <div class="flex flex-col gap-1">
                    <span class="font-code-md text-code-md text-on-surface font-bold">
                      {"Save to Disk"}
                    </span>
                    <span class="text-xs text-on-surface">
                      {"Writes all custom machine, process, filament presets, and .conf flushing volumes directly into your local slicer folder."}
                    </span>
                    <span class="text-[11px] font-mono text-on-surface-variant">
                      {"How to load: Close the slicer before clicking Save, then launch OrcaSlicer / AnycubicSlicerNext to load automatically."}
                    </span>
                  </div>
                  <button
                    class="shrink-0 flex items-center gap-space-xs bg-primary hover:opacity-90 text-on-primary px-space-md py-2 rounded-lg text-xs font-mono font-bold shadow transition-opacity cursor-pointer"
                    onclick={
                      let cb = on_commit_save_guarded.clone();
                      Callback::from(move |_| cb.emit(()))
                    }
                  >
                    <span class="material-symbols-outlined text-[16px]">{"save"}</span>
                    if !pending_updates.is_empty() {
                      {format!("Save to Disk ({} Staged)", pending_updates.len())}
                    } else {
                      {"Save to Disk"}
                    }
                  </button>
                </div>
              }
            </div>

            <div class="flex items-center justify-end pt-space-sm border-t border-surface-container">
              <button
                class="bg-surface-container-highest hover:bg-surface-bright text-on-surface border border-outline-variant px-space-xl py-2 rounded-lg text-xs font-mono font-bold transition-colors cursor-pointer"
                onclick={
                  let sem = show_export_modal.clone();
                  Callback::from(move |_| sem.set(false))
                }
              >
                {"OK"}
              </button>
            </div>
          </div>
        </div>
      }

      // Safety & Warning Confirmation Modal Dialog at Export / Save
      if let Some(pending_act) = &*pending_confirmed_action {
        <div class="fixed inset-0 z-50 bg-black/85 flex items-center justify-center p-4 overflow-y-auto">
          <div class="bg-surface-container-low border-2 border-red-500 rounded-2xl max-w-3xl w-full p-space-xl shadow-2xl flex flex-col gap-space-md my-auto">
            <div class="flex items-start justify-between gap-space-md">
              <div class="flex flex-col gap-1">
                <div class="flex items-center gap-space-xs">
                  <span class="bg-red-600 text-white text-[10px] font-mono font-bold uppercase px-2 py-0.5 rounded">
                    {"Safety & Warning Confirmation"}
                  </span>
                  if danger_warning_count > 0 {
                    <span class="bg-red-600 text-white border border-red-400 text-[10px] font-mono font-bold uppercase px-2 py-0.5 rounded flex items-center gap-1">
                      <span>{"🛑"}</span>
                      <span>{format!("{} UNSAFE", danger_warning_count)}</span>
                    </span>
                  }
                  if anomaly_warning_count > 0 {
                    <span class="bg-amber-500/20 text-amber-300 border border-amber-400/60 text-[10px] font-mono font-bold uppercase px-2 py-0.5 rounded flex items-center gap-1">
                      <span>{"⚠️"}</span>
                      <span>{format!("{} {}", anomaly_warning_count, if anomaly_warning_count == 1 { "WARNING" } else { "WARNINGS" })}</span>
                    </span>
                  }
                </div>
                <h2 class="font-headline-lg text-headline-sm text-on-surface font-bold mt-1">
                  {format!("Confirm Action: {}", pending_act.label())}
                </h2>
                <p class="text-xs text-on-surface-variant">
                  {"Our safety validator flagged unsafe values or warnings in the current configuration or imported disk profiles. Review the flagged items below before writing to disk or exporting."}
                </p>
              </div>
              <button
                class="text-on-surface-variant hover:text-on-surface p-1 cursor-pointer"
                onclick={
                  let pca = pending_confirmed_action.clone();
                  Callback::from(move |_| pca.set(None))
                }
              >
                <span class="material-symbols-outlined">{"close"}</span>
              </button>
            </div>

            <div class="max-h-80 overflow-y-auto flex flex-col gap-space-xs pr-1">
              {for safety_warnings.iter().map(|w| {
                let is_danger = w.severity == SafetySeverity::Danger;
                html! {
                  <div class={format!(
                    "p-space-sm rounded-lg border flex flex-col gap-1 {}",
                    if is_danger { "bg-red-950/30 border-red-500/80" } else { "bg-surface-container border-amber-500/50" }
                  )}>
                    <div class="flex items-center justify-between gap-2">
                      <div class="flex items-center gap-2">
                        <span class={format!(
                          "text-[10px] font-mono uppercase px-1.5 py-0.5 rounded font-bold flex items-center gap-1 {}",
                          if is_danger { "bg-red-600 text-white border border-red-400" } else { "bg-amber-500/20 text-amber-300 border border-amber-400/50" }
                        )}>
                          <span>{if is_danger { "🛑" } else { "⚠️" }}</span>
                          <span>{if is_danger { "UNSAFE" } else { "WARNING" }}</span>
                        </span>
                        <span class="font-mono text-xs text-on-surface font-bold">
                          {format!("{} — {}", w.scope_label, w.param_key)}
                        </span>
                      </div>
                      <span class="font-mono text-xs text-tertiary">
                        {format!("Safe: {}", w.safe_range)}
                      </span>
                    </div>
                    <div class="flex items-center gap-2 font-mono text-xs">
                      <span class="text-on-surface-variant">{"Current:"}</span>
                      <span class={if is_danger { "text-red-400 font-bold" } else { "text-amber-300 font-bold" }}>
                        {&w.current_value}
                      </span>
                    </div>
                    <p class="text-[11px] text-on-surface-variant">
                      {&w.explanation}
                    </p>
                  </div>
                }
              })}
            </div>

            <div class="flex items-center justify-end gap-space-sm pt-space-sm border-t border-surface-container">
              <button
                class="bg-surface-container-highest hover:bg-surface-bright text-on-surface px-space-lg py-2 rounded-lg text-xs font-mono font-bold transition-colors cursor-pointer"
                onclick={
                  let pca = pending_confirmed_action.clone();
                  Callback::from(move |_| pca.set(None))
                }
              >
                {"Cancel & Review Values"}
              </button>
              <button
                class="bg-error hover:opacity-90 text-on-error px-space-lg py-2 rounded-lg text-xs font-mono font-bold shadow-lg transition-opacity cursor-pointer flex items-center gap-1.5"
                onclick={on_confirm_pending_action}
              >
                <span class="material-symbols-outlined text-[16px]">{"warning"}</span>
                {format!("Proceed with {} Anyway", pending_act.label())}
              </button>
            </div>
          </div>
        </div>
      }

      // Reset All "Are You Sure?" Confirmation Modal Dialog (Requirement #8)
      if *show_reset_confirm_modal {
        <div class="fixed inset-0 z-50 bg-black/85 flex items-center justify-center p-4 overflow-y-auto">
          <div class="bg-surface-container-low border-2 border-error rounded-2xl max-w-lg w-full p-space-xl shadow-2xl flex flex-col gap-space-md my-auto">
            <div class="flex items-start justify-between gap-space-md">
              <div class="flex flex-col gap-1">
                <span class="bg-error text-on-error text-[10px] font-mono font-bold uppercase px-2 py-0.5 rounded w-fit">
                  {"Confirm Reset All"}
                </span>
                <h2 class="font-headline-lg text-headline-sm text-on-surface font-bold mt-1">
                  {"Are you sure you want to Reset All?"}
                </h2>
                <p class="text-xs text-on-surface-variant leading-relaxed">
                  {"This will clear all loaded machine rigs, filament spools, process profiles, and Lua transformation rules from browser storage and return to the Initialization screen. Export your Orca WebConfig (.json) first if you want to save a copy."}
                </p>
              </div>
              <button
                class="text-on-surface-variant hover:text-on-surface p-1 cursor-pointer"
                onclick={
                  let srcm = show_reset_confirm_modal.clone();
                  Callback::from(move |_| srcm.set(false))
                }
              >
                <span class="material-symbols-outlined">{"close"}</span>
              </button>
            </div>

            <div class="flex items-center justify-end gap-space-sm pt-space-sm border-t border-surface-container">
              <button
                class="bg-surface-container-highest hover:bg-surface-bright text-on-surface border border-outline-variant px-space-lg py-2 rounded-lg text-xs font-mono font-bold transition-colors cursor-pointer"
                onclick={
                  let srcm = show_reset_confirm_modal.clone();
                  Callback::from(move |_| srcm.set(false))
                }
              >
                {"Cancel"}
              </button>
              <button
                class="bg-error hover:opacity-90 text-on-error px-space-lg py-2 rounded-lg text-xs font-mono font-bold shadow-lg transition-opacity cursor-pointer flex items-center gap-1.5"
                onclick={on_confirm_reset_all}
              >
                <span class="material-symbols-outlined text-[16px]">{"restart_alt"}</span>
                {"Yes, Reset All"}
              </button>
            </div>
          </div>
        </div>
      }

      // Initialization Modal (Requirements #5 & #6: called "Initialization", no HTTP Client OS,
      // 1. Start From Scratch (Recommended & Default), 2. Load Orca WebConfig, 3. Expert Mode)
      if *show_bind_modal {
        <div class="fixed inset-0 z-50 bg-black/80 flex items-center justify-center p-4 overflow-y-auto">
          <div class="bg-surface-container-low border-2 border-primary rounded-2xl max-w-3xl w-full p-space-xl shadow-2xl flex flex-col gap-space-md my-auto">
            <div class="flex items-start justify-between gap-space-md">
              <div class="flex flex-col gap-1">
                <div class="flex items-center gap-space-xs">
                  <span class="bg-primary text-on-primary text-[10px] font-mono font-bold uppercase px-2 py-0.5 rounded">
                    {"Initialization"}
                  </span>
                  <span class="bg-tertiary/20 text-tertiary border border-tertiary/40 text-[10px] font-mono font-bold uppercase px-2 py-0.5 rounded">
                    {"OrcaSlicer WebConfig for v2.2"}
                  </span>
                </div>
                <h2 class="font-headline-lg text-headline-sm text-on-surface font-bold mt-1">
                  if *modal_start_from_scratch {
                    {"Start From Scratch (Recommended) — 5-Step Transformation Setup Wizard"}
                  } else {
                    {"Load Orca WebConfig or Import Existing Slicer Profiles"}
                  }
                </h2>
                <p class="text-xs text-on-surface-variant">
                  if *modal_start_from_scratch {
                    {"Select your Machine, Nozzle, Bed Plate, AMS / ACE Pro unit, and Filaments to build a clean transformation-based configuration stack from scratch, or load a saved Orca WebConfig (.json)."}
                  } else {
                    {"Load a saved Orca WebConfig (.json) state file, or expand Expert Mode below to import raw slicer configuration archives."}
                  }
                </p>
              </div>
              if *is_config_loaded {
                <button
                  class="text-on-surface-variant hover:text-on-surface p-1 cursor-pointer"
                  onclick={
                    let sbm = show_bind_modal.clone();
                    Callback::from(move |_| sbm.set(false))
                  }
                >
                  <span class="material-symbols-outlined">{"close"}</span>
                </button>
              }
            </div>

            // Primary Initialization Mode Bar:
            // 1. Start From Scratch (Recommended, Default)
            // 2. Load Orca WebConfig (.json)
            // 3. Expert Mode (Deprioritized)
            <div class="grid grid-cols-1 sm:grid-cols-3 gap-space-xs bg-surface-container-lowest p-1.5 rounded-xl border border-outline-variant font-mono text-xs">
              <button
                class={format!(
                  "flex items-center justify-center gap-1.5 py-2 px-2 rounded-lg transition-colors cursor-pointer {}",
                  if *modal_start_from_scratch {
                    "bg-primary text-on-primary font-bold shadow"
                  } else {
                    "text-on-surface-variant hover:text-on-surface"
                  }
                )}
                onclick={
                  let msfs = modal_start_from_scratch.clone();
                  let sei = show_expert_init.clone();
                  Callback::from(move |_| {
                    msfs.set(true);
                    sei.set(false);
                  })
                }
              >
                <span class="material-symbols-outlined text-[16px]">{"auto_fix_high"}</span>
                <span>{"1. Start From Scratch"}</span>
                <span class={format!(
                  "text-[9px] uppercase px-1.5 py-0.5 rounded font-bold {}",
                  if *modal_start_from_scratch {
                    "bg-on-primary text-primary"
                  } else {
                    "bg-tertiary/20 text-tertiary"
                  }
                )}>
                  {"Recommended"}
                </span>
              </button>

              <label
                class={format!(
                  "flex items-center justify-center gap-1.5 py-2 px-2 rounded-lg transition-colors {}",
                  if import_progress.is_some() {
                    "opacity-50 pointer-events-none text-on-surface-variant"
                  } else {
                    "bg-surface-container-high hover:bg-surface-bright text-primary border border-primary/40 font-bold cursor-pointer"
                  }
                )}
                title="Load an exported orcaslicer_webconfig.json state file to recreate the entire site state"
              >
                <span class="material-symbols-outlined text-[16px]">{"data_object"}</span>
                <span>{"2. Load Orca WebConfig"}</span>
                <input
                  type="file"
                  accept=".json,application/json"
                  class="hidden"
                  onchange={on_import_webconfig_json.clone()}
                />
              </label>

              <button
                class={format!(
                  "flex items-center justify-center gap-1.5 py-2 px-2 rounded-lg transition-colors cursor-pointer {}",
                  if !*modal_start_from_scratch {
                    "bg-surface-container-highest text-secondary border border-secondary/50 font-bold shadow"
                  } else {
                    "text-on-surface-variant/80 hover:text-on-surface"
                  }
                )}
                onclick={
                  let msfs = modal_start_from_scratch.clone();
                  let sei = show_expert_init.clone();
                  Callback::from(move |_| {
                    msfs.set(false);
                    sei.set(true);
                  })
                }
              >
                <span class="material-symbols-outlined text-[15px]">{"tune"}</span>
                <span>{"Expert Mode"}</span>
              </button>
            </div>

            if *modal_start_from_scratch {
              <ScratchWizard
                on_complete={on_complete_scratch_wizard.clone()}
                on_back_to_import={
                  let msfs = modal_start_from_scratch.clone();
                  let sei = show_expert_init.clone();
                  Callback::from(move |_| {
                    msfs.set(false);
                    sei.set(true);
                  })
                }
              />
            } else {
              <div class="flex flex-col gap-space-md">
                // Option 1 (Recommended): Start From Scratch Card
                <div class="bg-surface-container p-space-md rounded-xl border-2 border-primary/70 flex flex-col sm:flex-row items-start sm:items-center justify-between gap-space-md">
                  <div class="flex flex-col gap-1">
                    <div class="flex items-center gap-2">
                      <span class="font-code-md text-code-md text-primary font-bold">
                        {"1. Start From Scratch (5-Step Wizard)"}
                      </span>
                      <span class="bg-tertiary/20 text-tertiary border border-tertiary/40 text-[10px] font-mono uppercase px-2 py-0.5 rounded font-bold">
                        {"Recommended"}
                      </span>
                    </div>
                    <span class="text-xs text-on-surface-variant">
                      {"Build a clean transformation stack step-by-step (Machine → Nozzle → Bed → AMS → Filaments)."}
                    </span>
                  </div>
                  <button
                    class="shrink-0 flex items-center gap-space-xs bg-primary hover:opacity-90 text-on-primary px-space-md py-2 rounded-lg text-xs font-mono font-bold shadow transition-opacity cursor-pointer"
                    onclick={
                      let msfs = modal_start_from_scratch.clone();
                      Callback::from(move |_| msfs.set(true))
                    }
                  >
                    <span class="material-symbols-outlined text-[16px]">{"auto_fix_high"}</span>
                    {"Start From Scratch"}
                  </button>
                </div>

                // Option 2: Load Orca WebConfig (.json)
                <div class="bg-surface-container p-space-md rounded-xl border border-primary/40 flex flex-col sm:flex-row items-start sm:items-center justify-between gap-space-md">
                  <div class="flex flex-col gap-1">
                    <span class="font-code-md text-code-md text-on-surface font-bold">
                      {"2. Load Orca WebConfig (.json)"}
                    </span>
                    <span class="text-xs text-on-surface-variant">
                      {"Restore a previously exported orcaslicer_webconfig.json state file with all rigs, spools, and Lua rules."}
                    </span>
                  </div>
                  <label
                    class={format!(
                      "shrink-0 flex items-center gap-space-xs bg-primary-container hover:bg-primary text-on-primary-container px-space-md py-2 rounded-lg text-xs font-mono font-bold transition-colors {}",
                      if import_progress.is_some() {
                        "opacity-50 pointer-events-none"
                      } else {
                        "cursor-pointer"
                      }
                    )}
                  >
                    <span class="material-symbols-outlined text-[16px]">{"data_object"}</span>
                    {"Load Orca WebConfig (.json)"}
                    <input
                      type="file"
                      accept=".json,application/json"
                      class="hidden"
                      onchange={on_import_webconfig_json.clone()}
                    />
                  </label>
                </div>

                // Option 3 (Deprioritized): Collapsible Expert Mode for Raw Slicer Directory / Archive Imports
                <div class="bg-surface-container-lowest p-space-md rounded-xl border border-outline-variant/60 flex flex-col gap-space-sm">
                  <button
                    class="flex items-center justify-between w-full text-left font-mono text-xs text-on-surface-variant hover:text-on-surface cursor-pointer"
                    onclick={
                      let sei = show_expert_init.clone();
                      Callback::from(move |_| sei.set(!*sei))
                    }
                  >
                    <div class="flex items-center gap-2">
                      <span class="material-symbols-outlined text-[16px] text-secondary">{"tune"}</span>
                      <span class="font-bold text-secondary uppercase">{"Expert Mode"}</span>
                      <span>{"— Import Existing Slicer Directory or Backup Archive (.zip)"}</span>
                    </div>
                    <span class="material-symbols-outlined text-[18px]">
                      {if *show_expert_init { "expand_less" } else { "expand_more" }}
                    </span>
                  </button>

                  if *show_expert_init {
                    <div class="flex flex-col gap-space-sm pt-space-sm border-t border-surface-container">
                      <p class="text-[11px] text-on-surface-variant">
                        {"Expert Mode reconstructs existing flat OrcaSlicer / AnycubicSlicerNext profile folders into our transformation model and flags any one-off parameter drift."}
                      </p>

                      if let Some(prog) = &*import_progress {
                        <div class="bg-surface-container-highest border border-primary/60 rounded-xl p-space-md flex flex-col gap-space-sm shadow-inner">
                          <div class="flex items-center justify-between gap-space-sm">
                            <div class="flex items-center gap-space-xs text-primary font-mono text-xs font-bold">
                              <span class="material-symbols-outlined text-[16px] animate-spin">{"sync"}</span>
                              <span>{&prog.phase}</span>
                            </div>
                            <span class="bg-primary text-on-primary px-2 py-0.5 rounded text-xs font-mono font-bold">
                              if prog.total_files > 0 {
                                {format!(
                                  "{} / {} files ({}%)",
                                  prog.files_read,
                                  prog.total_files,
                                  ((prog.files_read as f64 / prog.total_files as f64) * 100.0)
                                    .round()
                                    .clamp(1.0, 100.0) as usize
                                )}
                              } else {
                                {"Discovering files..."}
                              }
                            </span>
                          </div>

                          <div class="w-full h-3 bg-surface-container-lowest rounded-full overflow-hidden border border-outline-variant/60">
                            <div
                              class="h-full bg-primary transition-all duration-75 rounded-full"
                              style={format!(
                                "width: {}%;",
                                if prog.total_files > 0 {
                                  ((prog.files_read as f64 / prog.total_files as f64) * 100.0)
                                    .round()
                                    .clamp(2.0, 100.0) as usize
                                } else {
                                  8usize
                                }
                              )}
                            />
                          </div>

                          <div class="text-[11px] font-mono text-on-surface-variant truncate">
                            {format!("File: {}", prog.current_file)}
                          </div>
                        </div>
                      }

                      <div class="flex flex-col sm:flex-row items-stretch sm:items-center justify-between gap-space-sm">
                        <label
                          class={format!(
                            "flex-1 flex items-center justify-center gap-space-sm bg-surface-container-highest text-secondary border border-secondary/40 px-space-md py-space-md rounded-xl text-xs font-mono font-bold transition-colors {}",
                            if import_progress.is_some() {
                              "opacity-50 pointer-events-none"
                            } else {
                              "hover:bg-surface-bright cursor-pointer"
                            }
                          )}
                        >
                          <span class="material-symbols-outlined text-[18px]">{"folder_zip"}</span>
                          {"Import Backup Archive (.zip)"}
                          <input
                            type="file"
                            accept=".zip"
                            class="hidden"
                            onchange={on_import_zip.clone()}
                          />
                        </label>

                        <button
                          disabled={import_progress.is_some()}
                          class={format!(
                            "flex-1 flex items-center justify-center gap-space-xs px-space-md py-space-md rounded-xl text-xs font-mono font-bold border border-outline-variant transition-colors {}",
                            if import_progress.is_some() {
                              "bg-surface-container-high text-on-surface-variant opacity-60 cursor-wait"
                            } else {
                              "bg-surface-container hover:bg-surface-container-high text-on-surface cursor-pointer"
                            }
                          )}
                          onclick={
                            let cb = on_load_reference_snapshot.clone();
                            Callback::from(move |_| cb.emit(()))
                          }
                        >
                          <span class="material-symbols-outlined text-[16px]">
                            {if import_progress.is_some() { "hourglass_top" } else { "folder_open" }}
                          </span>
                          {if import_progress.is_some() {
                            "Importing From Local FS..."
                          } else {
                            "Import From Local FS (Sandboxed)"
                          }}
                        </button>
                      </div>
                    </div>
                  }
                </div>
              </div>
            }
          </div>
        </div>
      }

      // Main Content Body
      <main class="w-full pt-32 px-4 lg:px-8 pb-12 flex-1 max-w-7xl mx-auto">
        // Toast notification
        if let Some(msg) = &*toast_message {
          <div class="mb-4 p-space-md rounded-lg bg-surface-container-high border border-primary text-primary font-code-sm text-code-sm flex items-center justify-between shadow-lg animate-fade-in">
            <div class="flex items-center gap-space-sm">
              <span class="material-symbols-outlined text-[16px]">{"info"}</span>
              <span>{msg}</span>
            </div>
            <button
              class="text-on-surface-variant hover:text-on-surface"
              onclick={
                let tm = toast_message.clone();
                Callback::from(move |_| tm.set(None))
              }
            >
              <span class="material-symbols-outlined text-[16px]">{"close"}</span>
            </button>
          </div>
        }

        // Render Active Tab
        {match *active_tab {
          ActiveTab::Home => html! {
            <HomeTab
              is_config_loaded={*is_config_loaded}
              is_tauri={is_tauri}
              bound_filename={(*bound_filename).clone()}
              bound_path_hint={(*bound_path_hint).clone()}
              detected_hardware_summary={(*detected_hardware_summary).clone()}
              scanned_file_count={scanned_entries.len()}
              last_saved_time={(*last_saved_time).clone()}
              on_load_reference_snapshot={on_load_reference_snapshot}
              on_import_zip={on_import_zip}
              on_backup_zip={on_backup_zip_guarded}
              on_export_webconfig_json={on_export_webconfig_json_guarded}
              on_import_webconfig_json={on_import_webconfig_json}
              on_open_export_modal={on_open_export_modal.clone()}
              on_reset_all={on_reset_all}
              on_commit_save={on_commit_save_guarded.clone()}
              on_export_3mf={on_export_3mf_guarded.clone()}
              on_export_standalone_json={on_export_standalone_json_guarded}
              on_switch_tab={
                let at = active_tab.clone();
                Callback::from(move |tab: ActiveTab| at.set(tab))
              }
              diff_items={eval_result.diff_items.clone()}
              rules={(*rules).clone()}
              on_toggle_rule={on_toggle_rule.clone()}
              conflicts={(*conflicts).clone()}
              safety_warnings={safety_warnings.clone()}
              pending_updates={(*pending_updates).clone()}
              on_resolve_conflict={on_resolve_conflict}
              show_diff_drawer={*show_diff_drawer}
              on_toggle_diff_drawer={
                let sdd = show_diff_drawer.clone();
                Callback::from(move |_| sdd.set(!*sdd))
              }
              active_rig={active_rig.clone()}
              processes={(*processes).clone()}
              active_process_id={(*active_process_id).clone()}
              on_select_process={on_select_process.clone()}
              safe_speed_ceiling={eval_result.safe_speed_ceiling}
              json_diff_text={json_diff_text}
              our_internal_config_json={our_internal_config_json}
              user_disk_config_json={user_disk_config_json}
            />
          },
          ActiveTab::Machine => html! {
            <MachineTab
              is_config_loaded={*is_config_loaded}
              machines={(*machines).clone()}
              active_machine_id={(*active_machine_id).clone()}
              on_set_active_machine={on_set_active_machine}
              on_duplicate_active_rig={on_duplicate_active_rig}
              on_add_machine={
                let m_state = machines.clone();
                let amid = active_machine_id.clone();
                let icl = is_config_loaded.clone();
                let t = toast_message.clone();
                Callback::from(move |mut new_m: MachineRig| {
                  let mut list = (*m_state).clone();
                  for m in &mut list {
                    m.is_active = false;
                  }
                  new_m.is_active = true;
                  let new_id = new_m.id.clone();
                  let new_name = new_m.name.clone();
                  list.push(new_m);
                  m_state.set(list);
                  amid.set(new_id);
                  icl.set(true);
                  t.set(Some(format!("Added new rig '{}' to vault!", new_name)));
                })
              }
              on_change_machine_type={on_change_machine_type}
              on_change_nozzle={on_change_nozzle}
              on_change_nozzle_diameter={on_change_nozzle_diameter}
              on_toggle_addon={on_toggle_addon}
              on_change_ace_units={on_change_ace_units}
              on_change_weather={on_change_weather}
              on_save_rig_to_vault={on_save_rig_to_vault}
              on_open_bind_modal={on_open_bind_modal}
              active_process={active_proc.clone()}
              safe_speed_ceiling={eval_result.safe_speed_ceiling}
            />
          },
          ActiveTab::Filament => html! {
            <FilamentTab
              is_config_loaded={*is_config_loaded}
              is_tauri={is_tauri}
              filaments={(*filaments).clone()}
              active_filament_type={(*active_filament_type).clone()}
              active_nozzle_type={active_rig.nozzle_type.clone()}
              ace_pro_units={active_rig.ace_pro_units}
              flush_matrix={eval_result.flush_matrix.clone()}
              disk_flush_matrix={(*disk_flush_matrix).clone()}
              disk_flush_slot_count={*disk_flush_slot_count}
              flush_multiplier={*flush_multiplier}
              disk_flush_multiplier={*disk_flush_multiplier}
              melt_zone_multiplier={active_rig.melt_zone_multiplier}
              on_change_flush_multiplier={{
                let fm = flush_multiplier.clone();
                let t = toast_message.clone();
                Callback::from(move |new_mult: f64| {
                  fm.set(new_mult);
                  t.set(Some(format!(
                    "Updated Flushing Multiplier to {:.2}× (re-evaluated Lua flushing matrix)!",
                    new_mult
                  )));
                })
              }}
              on_stage_flush_to_disk={{
                let pu = pending_updates.clone();
                let lst = last_saved_time.clone();
                let crp = (*conf_relative_path).clone();
                let cur_mult = *flush_multiplier;
                let d_mult = *disk_flush_multiplier;
                let lua_mat = eval_result.flush_matrix.clone();
                let d_mat = (*disk_flush_matrix).clone();
                let t = toast_message.clone();
                Callback::from(move |_| {
                  let mut list = (*pu).clone();
                  list.retain(|u| {
                    u.json_field_key != "flush_multiplier"
                      && u.json_field_key != "flush_volumes_matrix"
                  });
                  if (cur_mult - d_mult).abs() > 1e-4 {
                    list.push(PendingDiskUpdate {
                      param_key: "conf:flush_multiplier".to_string(),
                      param_label: "Flushing Multiplier (app.flush_multiplier)".to_string(),
                      source_relative_path: crp.clone(),
                      json_field_key: "flush_multiplier".to_string(),
                      old_disk_value: format!("{:.2}", d_mult),
                      new_clean_value: format!("{:.2}", cur_mult),
                    });
                  }
                  let formatted_pipe_matrix = lua_mat
                    .iter()
                    .map(|s| {
                      s.parse::<f64>()
                        .map(|v| format!("{:.6}", v))
                        .unwrap_or_else(|_| "0.000000".to_string())
                    })
                    .collect::<Vec<_>>()
                    .join("|");
                  let old_pipe_summary = if d_mat.is_empty() {
                    "None".to_string()
                  } else {
                    format!("{} cells ({}|...)", d_mat.len(), d_mat.iter().take(4).cloned().collect::<Vec<_>>().join("|"))
                  };
                  list.push(PendingDiskUpdate {
                    param_key: "conf:flush_volumes_matrix".to_string(),
                    param_label: "Flushing Volume Matrix (anycubic_presets[].flush_volumes_matrix)".to_string(),
                    source_relative_path: crp.clone(),
                    json_field_key: "flush_volumes_matrix".to_string(),
                    old_disk_value: old_pipe_summary,
                    new_clean_value: formatted_pipe_matrix,
                  });
                  pu.set(list);
                  lst.set("Unsaved Staged Changes".to_string());
                  t.set(Some(format!(
                    "Staged Lua Flushing Matrix & {:.2}× Multiplier for '{}' — click 'Save to Disk' to write!",
                    cur_mult, crp
                  )));
                })
              }}
              on_select_active_filament={on_select_active_filament}
              on_change_ace_units={on_change_ace_units}
              on_update_spool={on_update_spool}
              on_swap_slot_spool={on_swap_slot_spool}
              on_add_spool={on_add_spool}
              on_remove_spool={on_remove_spool}
              on_clear_spools={on_clear_spools}
              on_create_rule={on_create_rule.clone()}
              on_export_spool_json={on_export_spool_json}
              on_export_3mf={on_export_3mf_guarded}
              on_open_bind_modal={on_open_bind_modal}
            />
          },
          ActiveTab::Wizard => html! {
            <WizardTab
              is_config_loaded={*is_config_loaded}
              is_tauri={is_tauri}
              rules={(*rules).clone()}
              diff_items={eval_result.diff_items.clone()}
              rule_changes={eval_result.rule_changes.clone()}
              processes={(*processes).clone()}
              active_process_id={(*active_process_id).clone()}
              on_select_process={on_select_process}
              on_toggle_rule={on_toggle_rule}
              on_update_rule_lua={on_update_rule_lua}
              on_create_rule={on_create_rule}
              on_delete_rule={on_delete_rule}
              on_reorder_rule={on_reorder_rule}
              on_export_package={on_export_package_guarded}
              on_commit_save={on_commit_save_guarded}
              on_open_bind_modal={on_open_bind_modal}
              execution_logs={eval_result.execution_logs.clone()}
              on_clear_logs={on_clear_logs}
              safe_speed_ceiling={eval_result.safe_speed_ceiling}
            />
          },
        }}
      </main>
    </div>
  }
}
