use crate::catalog::{
  load_merged_catalog, CatalogAmsOption, CatalogBed, CatalogFilament, CatalogMachine, CatalogNozzle,
};
use crate::components::color_picker::{ColorPicker, ColorSelection};
use crate::models::{FilamentDeltaEntry, FilamentSpool, MachineRig, ProcessProfile};
use crate::orca_profile::OrcaProfile;
use crate::transformations::{PendingDiskUpdate, TransformationEngine, TransformationRule};
use yew::prelude::*;

/// Output payload produced when the user finishes the 5-Step Start-From-Scratch Setup Wizard.
#[derive(Clone, Debug, PartialEq)]
pub struct ScratchWizardOutput {
  pub rig: MachineRig,
  pub filaments: Vec<FilamentSpool>,
  pub processes: Vec<ProcessProfile>,
  pub base_profile: OrcaProfile,
  pub rules: Vec<TransformationRule>,
  pub initial_pending_updates: Vec<PendingDiskUpdate>,
  pub summary_label: String,
}

#[derive(Properties, PartialEq)]
pub struct ScratchWizardProps {
  pub on_complete: Callback<ScratchWizardOutput>,
  pub on_back_to_import: Callback<()>,
}

#[derive(Clone, Debug, PartialEq)]
struct WizardSpoolDraft {
  pub name: String,
  pub material_type: String,
  pub color_hex: String,
  pub finish_type: String,
}

fn find_filament_entry<'a>(
  catalog: &'a [CatalogFilament],
  code: &str,
  finish_type: &str,
) -> Option<&'a CatalogFilament> {
  let variant_code = format!("{} {}", code, finish_type);
  catalog
    .iter()
    .find(|e| e.code.eq_ignore_ascii_case(&variant_code))
    .or_else(|| {
      catalog.iter().find(|e| {
        e.base_material_code().eq_ignore_ascii_case(code)
          && e.default_finish.eq_ignore_ascii_case(finish_type)
      })
    })
    .or_else(|| catalog.iter().find(|e| e.code.eq_ignore_ascii_case(code)))
}

#[function_component]
pub fn ScratchWizard(props: &ScratchWizardProps) -> Html {
  let catalog = use_memo((), |_| load_merged_catalog());

  // Wizard step: 1 = Machine, 2 = Nozzle, 3 = Bed, 4 = AMS, 5 = Filaments
  let step = use_state(|| 1usize);

  // Per-step search & filter state
  let machine_search = use_state(String::new);
  let machine_vendor_filter = use_state(|| "All".to_string());
  let nozzle_search = use_state(String::new);
  let nozzle_brand_filter = use_state(|| "All".to_string());
  let bed_search = use_state(String::new);
  let ams_search = use_state(String::new);
  let filament_catalog_search = use_state(String::new);
  let filament_category_filter = use_state(|| "All".to_string());
  let added_spools_search = use_state(String::new);

  // Selected Hardware & Loadout State
  let selected_machine_id = use_state(|| {
    catalog
      .machines
      .first()
      .map(|m| m.id.clone())
      .unwrap_or_else(|| "anycubic_anycubic_kobra_s1".to_string())
  });
  let selected_nozzle_id = use_state(|| {
    let default_mat = catalog
      .machines
      .first()
      .map(|m| m.default_nozzle_material.as_str())
      .unwrap_or("Brass");
    match default_mat {
      "Hardened Steel" => "orca_hardened_steel".to_string(),
      "Stainless Steel" => "orca_stainless_steel".to_string(),
      _ => "orca_brass".to_string(),
    }
  });
  let nozzle_diameter = use_state(|| 0.4f32);
  let selected_addons = use_state(Vec::<String>::new);
  let selected_bed_id = use_state(|| "textured_pei".to_string());
  let selected_ams_id = use_state(|| "ace_pro_single".to_string());

  // Requirement: Default filaments must be EMPTY in the wizard!
  let spool_drafts = use_state(Vec::<WizardSpoolDraft>::new);

  let cur_machine: CatalogMachine = catalog
    .machines
    .iter()
    .find(|m| m.id == *selected_machine_id)
    .cloned()
    .unwrap_or_else(|| catalog.machines[0].clone());

  let cur_nozzle: CatalogNozzle = catalog
    .nozzles
    .iter()
    .find(|n| n.id == *selected_nozzle_id)
    .cloned()
    .unwrap_or_else(|| catalog.nozzles[0].clone());

  let valid_beds: Vec<CatalogBed> = catalog
    .beds
    .iter()
    .filter(|b| cur_machine.is_bed_compatible(b))
    .cloned()
    .collect();

  let cur_bed: CatalogBed = valid_beds
    .iter()
    .find(|b| b.id == *selected_bed_id)
    .or_else(|| valid_beds.first())
    .cloned()
    .unwrap_or_else(|| catalog.beds[0].clone());

  let valid_ams: Vec<CatalogAmsOption> = catalog
    .ams_options
    .iter()
    .filter(|a| cur_machine.is_ams_compatible(a))
    .cloned()
    .collect();

  let cur_ams: CatalogAmsOption = valid_ams
    .iter()
    .find(|a| a.id == *selected_ams_id)
    .or_else(|| valid_ams.first())
    .cloned()
    .unwrap_or_else(|| catalog.ams_options[0].clone());

  // Calculate max flow scaling with orifice diameter
  let diam_ratio = (*nozzle_diameter / 0.4).powf(1.35);
  let effective_max_flow = (cur_nozzle.base_flow_mm3s * diam_ratio * 10.0).round() / 10.0;

  let on_finish_wizard = {
    let on_complete = props.on_complete.clone();
    let cur_machine = cur_machine.clone();
    let cur_nozzle = cur_nozzle.clone();
    let cur_bed = cur_bed.clone();
    let cur_ams = cur_ams.clone();
    let diam = *nozzle_diameter;
    let addons = (*selected_addons).clone();
    let drafts = (*spool_drafts).clone();
    let fil_catalog = catalog.filaments.clone();

    Callback::from(move |_: MouseEvent| {
      let mut rig = MachineRig {
        id: format!("rig_scratch_{}", cur_machine.id),
        name: String::new(),
        printer_model: cur_machine.name.clone(),
        bed_size: cur_machine.build_volume.clone(),
        kinematics: cur_machine.kinematics.clone(),
        firmware: "OrcaSlicer 2.4.2 Profile".to_string(),
        hotend: cur_nozzle.name.clone(),
        nozzle_type: cur_nozzle.id.clone(),
        nozzle_material: cur_nozzle.material.clone(),
        nozzle_name: format!("{} {:.2}mm", cur_nozzle.name, diam),
        nozzle_diameter: format!("{:.1}", diam),
        nozzle_hrc: 55,
        max_volumetric_flow_limit: effective_max_flow as f64,
        max_volumetric_flow: format!("{:.1} mm³/s", effective_max_flow),
        base_retraction_length: cur_nozzle.retraction_mm as f64,
        melt_zone_multiplier: cur_nozzle.melt_zone_mult as f64,
        ace_pro_units: cur_ams.units.max(1),
        addons: addons.clone(),
        max_speed: "600 mm/s".to_string(),
        max_accel: "20,000 mm/s²".to_string(),
        is_active: true,
        applied_transformations: vec![
          "Thermal Conductivity & Nozzle Material Rule".to_string(),
          "Volumetric Speed Ceiling Rule".to_string(),
          "Upgraded Flushing Volume Matrix (Lua)".to_string(),
        ],
        notes: String::new(),
        source_disk_profile: None,
      };
      let (_, computed_hrc) = rig.orca_machine_nozzle_type_and_hrc();
      rig.nozzle_hrc = computed_hrc;
      rig.refresh_title_and_notes(true);
      rig.notes = format!(
        "{}\n• Bed Surface: {} ({:+}°C bed offset)",
        rig.notes, cur_bed.name, cur_bed.temp_offset_c
      );

      let short_mach = rig.short_model_tag();
      let layer_std = ((diam * 0.5 * 100.0).round() / 100.0) as f64;
      let layer_hq = ((diam * 0.3 * 100.0).round() / 100.0) as f64;
      let layer_speed_thick = ((diam * 0.7 * 100.0).round() / 100.0) as f64;
      let line_width = ((diam * 1.05 * 100.0).round() / 100.0) as f64;
      let line_width_thick = ((diam * 1.10 * 100.0).round() / 100.0) as f64;

      // 1. 0.20mm Standard
      let mut proc_std = ProcessProfile {
        id: format!("proc_{:.2}mm_standard", layer_std),
        name: String::new(),
        inherits: "0.20mm Standard @Anycubic Kobra S1 0.4 nozzle".to_string(),
        layer_height: layer_std,
        line_width,
        outer_wall_speed: 200.0,
        inner_wall_speed: 300.0,
        sparse_infill_speed: 270.0,
        initial_layer_speed: 50.0,
        default_acceleration: 10000.0,
        print_sequence: "by object".to_string(),
        wall_generator: "arachne".to_string(),
        notes: String::new(),
      };
      proc_std.name = proc_std.format_concise_title(&short_mach);
      proc_std.notes = proc_std.generate_descriptive_notes();

      // 2. 0.20mm Quality
      let mut proc_hq = ProcessProfile {
        id: format!("proc_{:.2}mm_high_quality", layer_std),
        name: String::new(),
        inherits: "0.20mm High Quality @Anycubic Kobra S1 0.4 nozzle".to_string(),
        layer_height: layer_std,
        line_width,
        outer_wall_speed: 60.0,
        inner_wall_speed: 150.0,
        sparse_infill_speed: 250.0,
        initial_layer_speed: 50.0,
        default_acceleration: 5000.0,
        print_sequence: "by object".to_string(),
        wall_generator: "arachne".to_string(),
        notes: String::new(),
      };
      proc_hq.name = proc_hq.format_concise_title(&short_mach);
      proc_hq.notes = proc_hq.generate_descriptive_notes();

      // 3. 0.20mm Speed
      let mut proc_hs = ProcessProfile {
        id: format!("proc_{:.2}mm_high_speed", layer_std),
        name: String::new(),
        inherits: "0.20mm Standard @Anycubic Kobra S1 0.4 nozzle".to_string(),
        layer_height: layer_std,
        line_width,
        outer_wall_speed: 300.0,
        inner_wall_speed: 350.0,
        sparse_infill_speed: 350.0,
        initial_layer_speed: 60.0,
        default_acceleration: 15000.0,
        print_sequence: "by object".to_string(),
        wall_generator: "arachne".to_string(),
        notes: String::new(),
      };
      proc_hs.name = proc_hs.format_concise_title(&short_mach);
      proc_hs.notes = proc_hs.generate_descriptive_notes();

      // 4. 0.12mm Quality
      let mut proc_hq_fine = ProcessProfile {
        id: format!("proc_{:.2}mm_fine_hq", layer_hq),
        name: String::new(),
        inherits: "0.12mm Standard @Anycubic Kobra S1 0.4 nozzle".to_string(),
        layer_height: layer_hq,
        line_width,
        outer_wall_speed: 60.0,
        inner_wall_speed: 150.0,
        sparse_infill_speed: 200.0,
        initial_layer_speed: 50.0,
        default_acceleration: 5000.0,
        print_sequence: "by object".to_string(),
        wall_generator: "arachne".to_string(),
        notes: String::new(),
      };
      proc_hq_fine.name = proc_hq_fine.format_concise_title(&short_mach);
      proc_hq_fine.notes = proc_hq_fine.generate_descriptive_notes();

      // 5. 0.28mm Speed
      let mut proc_hs_thick = ProcessProfile {
        id: format!("proc_{:.2}mm_high_speed", layer_speed_thick),
        name: String::new(),
        inherits: "0.28mm Standard @Anycubic Kobra S1 0.4 nozzle".to_string(),
        layer_height: layer_speed_thick,
        line_width: line_width_thick,
        outer_wall_speed: 250.0,
        inner_wall_speed: 320.0,
        sparse_infill_speed: 320.0,
        initial_layer_speed: 60.0,
        default_acceleration: 12000.0,
        print_sequence: "by object".to_string(),
        wall_generator: "arachne".to_string(),
        notes: String::new(),
      };
      proc_hs_thick.name = proc_hs_thick.format_concise_title(&short_mach);
      proc_hs_thick.notes = proc_hs_thick.generate_descriptive_notes();

      let mut initial_pending_updates: Vec<PendingDiskUpdate> = Vec::new();
      let machine_rel_path = format!(
        "user/default/machine/{}",
        crate::models::sanitize_profile_filename(&rig.name)
      );
      let (orca_machine_noz_type, orca_machine_hrc) = rig.orca_machine_nozzle_type_and_hrc();
      let default_orca_noz = if cur_machine
        .default_nozzle_material
        .to_lowercase()
        .contains("hardened")
      {
        "hardened_steel"
      } else {
        "brass"
      };
      let noz_type_upd = PendingDiskUpdate {
        param_key: "machine.nozzle_type".to_string(),
        param_label: format!("Machine Nozzle Type ({})", cur_nozzle.name),
        source_relative_path: machine_rel_path.clone(),
        json_field_key: "nozzle_type".to_string(),
        old_disk_value: default_orca_noz.to_string(),
        new_clean_value: orca_machine_noz_type.to_string(),
      };
      if noz_type_upd.has_value_change() {
        initial_pending_updates.push(noz_type_upd);
      }

      if orca_machine_hrc > 0 {
        initial_pending_updates.push(PendingDiskUpdate {
          param_key: "machine.nozzle_hrc".to_string(),
          param_label: format!("Machine Nozzle Hardness HRC ({})", cur_nozzle.name),
          source_relative_path: machine_rel_path.clone(),
          json_field_key: "nozzle_hrc".to_string(),
          old_disk_value: "0".to_string(),
          new_clean_value: orca_machine_hrc.to_string(),
        });
      }

      let default_diam_str = format!("{:.2}", cur_machine.default_nozzle_diameter)
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string();
      let new_diam_str = format!("{:.2}", diam)
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string();
      let noz_diam_upd = PendingDiskUpdate {
        param_key: "machine.nozzle_diameter".to_string(),
        param_label: "Nozzle Orifice Diameter (mm)".to_string(),
        source_relative_path: machine_rel_path.clone(),
        json_field_key: "nozzle_diameter".to_string(),
        old_disk_value: default_diam_str,
        new_clean_value: new_diam_str,
      };
      if noz_diam_upd.has_value_change() {
        initial_pending_updates.push(noz_diam_upd);
      }

      initial_pending_updates.push(PendingDiskUpdate {
        param_key: "machine.printer_notes".to_string(),
        param_label: format!("Machine Profile Notes ({})", rig.name),
        source_relative_path: machine_rel_path,
        json_field_key: "printer_notes".to_string(),
        old_disk_value: "(none)".to_string(),
        new_clean_value: rig.notes.clone(),
      });

      for p in [&proc_std, &proc_hq, &proc_hs, &proc_hq_fine, &proc_hs_thick] {
        let proc_rel_path = format!(
          "user/default/process/{}",
          crate::models::sanitize_profile_filename(&p.name)
        );
        initial_pending_updates.push(PendingDiskUpdate {
          param_key: format!("process.{}.notes", p.id),
          param_label: format!("Process Profile Notes ({})", p.name),
          source_relative_path: proc_rel_path,
          json_field_key: "notes".to_string(),
          old_disk_value: "(none)".to_string(),
          new_clean_value: p.notes.clone(),
        });
      }

      let short_noz = rig.short_nozzle_tag();
      let filaments: Vec<FilamentSpool> = drafts
        .iter()
        .enumerate()
        .map(|(idx, d)| {
          let mat_info = find_filament_entry(&fil_catalog, &d.material_type, &d.finish_type);
          let base_nozzle = mat_info.map(|m| m.base_nozzle_c as i32).unwrap_or(215);
          let base_bed = mat_info.map(|m| m.base_bed_c as i32).unwrap_or(60);
          let max_vol = mat_info.map(|m| m.max_vol_mm3s).unwrap_or(15.0);
          let fan_pct = mat_info.map(|m| m.fan_pct as i32).unwrap_or(100);
          let dry_c = mat_info.map(|m| m.dry_temp_c as i32).unwrap_or(45);
          let dry_hr = mat_info.map(|m| m.dry_hr as i32).unwrap_or(4);

          let effective_nozzle = (base_nozzle + cur_nozzle.temp_offset_c).clamp(175, 350);
          let effective_bed = (base_bed + cur_bed.temp_offset_c).clamp(30, 130);
          let hs_nozzle = (base_nozzle + 5).clamp(180, 350);
          let is_pctg_or_petg =
            d.material_type.contains("PCTG") || d.material_type.contains("PETG");
          let default_pa = if d.material_type.contains("PCTG") {
            0.048f32
          } else if d.material_type.contains("PETG") {
            0.045f32
          } else {
            0.035f32
          };
          let default_flow = if d.finish_type == "Foaming" {
            0.58f32
          } else if is_pctg_or_petg {
            0.95f32
          } else {
            0.98f32
          };

          let vendor_prefix = if cur_machine.vendor.eq_ignore_ascii_case("Anycubic") {
            "Anycubic"
          } else {
            "Generic"
          };
          let base_mat_tag = match d.material_type.as_str() {
            "PCTG" | "PCTG-CF" | "PCTG-GF" => "PETG",
            "PLA-CF" | "PLA-GF" | "PLA Glow" => "PLA",
            other => other,
          };
          let inherits = format!(
            "{} {} @{} {:.1} nozzle",
            vendor_prefix, base_mat_tag, cur_machine.name, diam
          );
          let clean_draft_name = d
            .name
            .trim_start_matches("OC - ")
            .trim_start_matches("OC -")
            .trim_start_matches("OC: ")
            .trim_start_matches("OC:")
            .trim();
          let draft_name_with_finish =
            if let Some(suf) = FilamentSpool::finish_title_suffix(&d.finish_type) {
              if !clean_draft_name
                .to_lowercase()
                .contains(&suf.to_lowercase())
              {
                format!("{} {}", clean_draft_name, suf)
              } else {
                clean_draft_name.to_string()
              }
            } else {
              clean_draft_name.to_string()
            };
          let profile_name = format!(
            "OC - {} • {:.1} {}",
            draft_name_with_finish, diam, short_noz
          );
          let fil_rel_path = format!(
            "user/default/filament/{}",
            crate::models::sanitize_profile_filename(&profile_name)
          );
          let is_abrasive = mat_info.map(|m| m.requires_hardened).unwrap_or(false)
            || d.finish_type.contains("Carbon")
            || d.finish_type.contains("Glass")
            || d.finish_type.contains("Wood")
            || d.finish_type.contains("Metal")
            || d.finish_type.contains("Glow")
            || d.finish_type.contains("Conductive")
            || d.finish_type.contains("Marble")
            || d.material_type.contains("-CF")
            || d.material_type.contains("-GF");
          let req_hrc: u32 = if is_abrasive { 55 } else { 3 };

          let noz_temp_upd = PendingDiskUpdate {
            param_key: format!("filament.{}.nozzle_temperature", idx + 1),
            param_label: format!("{} Nozzle Temp (°C)", profile_name),
            source_relative_path: fil_rel_path.clone(),
            json_field_key: "nozzle_temperature".to_string(),
            old_disk_value: base_nozzle.to_string(),
            new_clean_value: effective_nozzle.to_string(),
          };
          if noz_temp_upd.has_value_change() {
            initial_pending_updates.push(noz_temp_upd);
          }

          let bed_temp_upd = PendingDiskUpdate {
            param_key: format!("filament.{}.hot_plate_temp", idx + 1),
            param_label: format!("{} Bed Temp (°C)", profile_name),
            source_relative_path: fil_rel_path,
            json_field_key: "hot_plate_temp".to_string(),
            old_disk_value: base_bed.to_string(),
            new_clean_value: effective_bed.to_string(),
          };
          if bed_temp_upd.has_value_change() {
            initial_pending_updates.push(bed_temp_upd);
          }

          let mut deltas_from_base = Vec::new();
          if effective_nozzle != base_nozzle {
            deltas_from_base.push(FilamentDeltaEntry {
              param_key: "nozzle_temperature".to_string(),
              base_value: format!("{}°C", base_nozzle),
              disk_value: format!("{}°C", effective_nozzle),
              reconstructed_by_lua: true,
              rule_explanation: format!(
                "{} {:+}°C thermal rule",
                cur_nozzle.material, cur_nozzle.temp_offset_c
              ),
            });
          }
          if effective_bed != base_bed {
            deltas_from_base.push(FilamentDeltaEntry {
              param_key: "hot_plate_temp".to_string(),
              base_value: format!("{}°C", base_bed),
              disk_value: format!("{}°C", effective_bed),
              reconstructed_by_lua: true,
              rule_explanation: format!(
                "{} {:+}°C bed plate rule",
                cur_bed.surface_code, cur_bed.temp_offset_c
              ),
            });
          }

          let raw_disk_json = serde_json::to_string_pretty(&serde_json::json!({
            "from": "User",
            "inherits": inherits,
            "name": profile_name,
            "filament_settings_id": [profile_name],
            "filament_type": [d.material_type],
            "required_nozzle_HRC": [req_hrc.to_string()],
            "filament_colour": [d.color_hex],
            "default_filament_colour": [d.color_hex],
            "nozzle_temperature": [effective_nozzle.to_string()],
            "nozzle_temperature_BRASS": [base_nozzle.to_string()],
            "nozzle_temperature_HS": [hs_nozzle.to_string()],
            "nozzle_temperature_initial_layer": [(effective_nozzle + 5).min(350).to_string()],
            "nozzle_temperature_initial_layer_BRASS": [(base_nozzle + 5).min(350).to_string()],
            "nozzle_temperature_initial_layer_HS": [(hs_nozzle + 5).min(350).to_string()],
            "nozzle_temperature_range_low": [(base_nozzle - 10).max(175).to_string()],
            "nozzle_temperature_range_high": [(hs_nozzle + 15).min(350).to_string()],
            "hot_plate_temp": [effective_bed.to_string()],
            "hot_plate_temp_initial_layer": [effective_bed.to_string()],
            "textured_plate_temp": [effective_bed.to_string()],
            "textured_plate_temp_initial_layer": [effective_bed.to_string()],
            "filament_flow_ratio": [format!("{:.2}", default_flow)],
            "pressure_advance": [format!("{:.3}", default_pa)],
            "filament_max_volumetric_speed": [format!("{:.1}", max_vol)],
            "fan_max_speed": [fan_pct.to_string()],
            "fan_max_speed_BRASS": [fan_pct.to_string()],
            "fan_max_speed_HS": [fan_pct.to_string()],
            "filament_z_hop": [if is_pctg_or_petg { "0.15" } else { "0.2" }],
            "filament_z_hop_types": [if is_pctg_or_petg { "Spiral Lift" } else { "Normal Lift" }],
            "filament_retract_before_wipe": [if is_pctg_or_petg { "70%" } else { "0%" }],
            "filament_deretraction_speed": [if is_pctg_or_petg { "30" } else { "0" }],
            "version": "2.0.0.0"
          }))
          .unwrap_or_default();

          FilamentSpool {
            id: format!("spool_scratch_{}", idx + 1),
            name: profile_name,
            brand: "Custom".to_string(),
            inherits,
            material_type: d.material_type.clone(),
            nozzle_type: cur_nozzle.id.clone(),
            tuned_for_nozzle_type: cur_nozzle.id.clone(),
            required_nozzle_hrc: req_hrc,
            color_hex: d.color_hex.clone(),
            color_name: d.color_hex.clone(),
            brass_base_temp: base_nozzle,
            hs_base_temp: hs_nozzle,
            base_print_temp: base_nozzle,
            print_temp: effective_nozzle,
            base_bed_temp: base_bed,
            bed_temp: effective_bed,
            flow_ratio: default_flow,
            pressure_advance: default_pa,
            max_volumetric_speed: max_vol,
            fan_speed_percent: fan_pct,
            remaining_grams: 1000,
            is_loaded: true,
            finish_type: d.finish_type.clone(),
            opacity: match d.finish_type.as_str() {
              "Translucent" => 0.35,
              "Glow-in-the-Dark" => 0.65,
              "Photochromic" => 0.70,
              _ => 1.0,
            },
            pigment_stickiness: match d.finish_type.as_str() {
              "Carbon Fiber" => 1.15,
              "Glass Fiber" => 1.10,
              "Silk" => 1.20,
              "Matte" => 1.05,
              "Wood-Filled" => 1.25,
              "Metal-Filled" => 1.35,
              "Glow-in-the-Dark" => 1.30,
              "Foaming" => 1.20,
              "Conductive" => 1.45,
              "Marble/Stone/Terracotta" => 1.15,
              "Photochromic" => 1.10,
              _ if d.material_type.contains("-CF") => 1.15,
              _ => 1.0,
            },
            drying_temp_c: dry_c,
            drying_time_hr: dry_hr,
            is_rfid_synced: false,
            deltas_from_base,
            raw_disk_json: Some(raw_disk_json),
          }
        })
        .collect();

      let base_profile = OrcaProfile::get_stock_kobra_s1();
      let mut rules = TransformationEngine::get_default_rules();
      TransformationEngine::sync_rules_for_rig(
        &mut rules,
        &cur_nozzle.id,
        &cur_nozzle.material,
        cur_ams.units,
      );
      // When starting from scratch, PCTG spools already have Spiral Z-hop, 70% retract-before-wipe,
      // 30 mm/s deretract, and 0.048 PA baked into their base profile, so keep the extra post-print
      // `filament_pctg_antistringing` trim rule disabled unless the user enables it after a retraction test.
      if let Some(pctg_rule) = rules
        .iter_mut()
        .find(|r| r.id == "filament_pctg_antistringing")
      {
        pctg_rule.enabled = false;
      }
      let summary_label = format!("{} ({} Spools)", rig.name, filaments.len());

      on_complete.emit(ScratchWizardOutput {
        rig,
        filaments,
        processes: vec![proc_std, proc_hq, proc_hs, proc_hq_fine, proc_hs_thick],
        base_profile,
        rules,
        initial_pending_updates,
        summary_label,
      });
    })
  };

  let steps_meta = [
    (
      1usize,
      "1. Machine",
      "precision_manufacturing",
      cur_machine.name.clone(),
    ),
    (
      2usize,
      "2. Nozzle",
      "flare",
      format!("{:.2}mm {}", *nozzle_diameter, cur_nozzle.material),
    ),
    (
      3usize,
      "3. Bed Plate",
      "grid_on",
      cur_bed.surface_code.clone(),
    ),
    (
      4usize,
      "4. AMS / MMU",
      "view_week",
      if cur_ams.units == 0 {
        "Single Spool".to_string()
      } else {
        format!("{} Slots ({})", cur_ams.total_slots, cur_ams.vendor)
      },
    ),
    (
      5usize,
      "5. Filaments",
      "Autorenew",
      format!("{} Spools Added", spool_drafts.len()),
    ),
  ];

  // Vendor list for Step 1 filter pills
  let mut vendor_list: Vec<String> = vec![
    "All".to_string(),
    "Anycubic".to_string(),
    "Bambu Lab".to_string(),
    "Prusa".to_string(),
    "Creality".to_string(),
    "Voron".to_string(),
    "Qidi".to_string(),
    "Sovol".to_string(),
    "RatRig".to_string(),
    "Elegoo".to_string(),
    "Flashforge".to_string(),
  ];
  for m in &catalog.machines {
    if !vendor_list.contains(&m.vendor) {
      vendor_list.push(m.vendor.clone());
    }
  }

  html! {
    <div class="fixed inset-0 z-50 bg-background/95 backdrop-blur-md flex items-center justify-center p-4 overflow-y-auto">
      <div class="bg-surface-container-low border border-outline-variant rounded-2xl shadow-2xl max-w-6xl w-full p-space-xl flex flex-col gap-space-lg my-auto max-h-[94vh] overflow-y-auto">
        // Top Header
        <div class="flex flex-wrap items-center justify-between gap-space-md border-b border-outline-variant/60 pb-space-md">
          <div class="flex items-center gap-space-md">
            <div class="w-11 h-11 rounded-2xl bg-primary/15 border border-primary/40 flex items-center justify-center">
              <span class="material-symbols-outlined text-primary text-2xl">{"auto_fix_high"}</span>
            </div>
            <div class="flex flex-col">
              <div class="flex items-center gap-2 flex-wrap">
                <h2 class="font-headline-md text-headline-md text-on-surface font-bold">
                  {"Start From Scratch — 5-Step Setup Wizard"}
                </h2>
                <span class="bg-primary/15 text-primary border border-primary/30 text-[10px] px-2 py-0.5 rounded font-mono">
                  {format!(
                    "@orcaslicer_repo ({} machines • {} materials) + additional_catalog.json",
                    catalog.machines.len(),
                    catalog.filaments.len()
                  )}
                </span>
              </div>
              <span class="text-xs text-on-surface-variant">
                {"Generated from OrcaSlicer GitHub http_archive + genrule merged with MicroSwiss & specialty polymer catalog"}
              </span>
            </div>
          </div>

          <button
            onclick={
              let cb = props.on_back_to_import.clone();
              Callback::from(move |_| cb.emit(()))
            }
            class="bg-surface-container-high hover:bg-surface-bright text-on-surface-variant hover:text-on-surface px-space-md py-1.5 rounded-lg text-xs font-semibold flex items-center gap-1.5 border border-outline-variant transition-colors"
          >
            <span class="material-symbols-outlined text-sm">{"arrow_back"}</span>
            {"Back to Startup Choice"}
          </button>
        </div>

        // 5-Step Progress Indicator Bar
        <div class="grid grid-cols-2 md:grid-cols-5 gap-2">
          {for steps_meta.iter().map(|(idx, title, icon, summary)| {
            let is_current = *step == *idx;
            let is_done = *step > *idx;
            let s_state = step.clone();
            let target_step = *idx;
            html! {
              <button
                onclick={Callback::from(move |_| s_state.set(target_step))}
                class={format!(
                  "flex flex-col items-start p-2.5 rounded-xl border text-left transition-all {}",
                  if is_current {
                    "bg-primary/15 border-primary shadow-md"
                  } else if is_done {
                    "bg-tertiary-container/15 border-tertiary/40 hover:border-tertiary"
                  } else {
                    "bg-surface-container-lowest border-outline-variant/60 hover:border-outline"
                  }
                )}
              >
                <div class="flex items-center justify-between w-full mb-1">
                  <div class="flex items-center gap-1.5">
                    <span class={format!(
                      "material-symbols-outlined text-base {}",
                      if is_current { "text-primary" } else if is_done { "text-tertiary" } else { "text-on-surface-variant" }
                    )}>{*icon}</span>
                    <span class="text-xs font-bold text-on-surface">{*title}</span>
                  </div>
                  {if is_done {
                    html! { <span class="material-symbols-outlined text-tertiary text-sm">{"check_circle"}</span> }
                  } else {
                    html! {}
                  }}
                </div>
                <span class="text-[11px] font-mono text-on-surface-variant truncate w-full">{summary.clone()}</span>
              </button>
            }
          })}
        </div>

        // STEP 1: SELECT MACHINE (FROM @orcaslicer_repo HTTP_ARCHIVE + GENRULE)
        {if *step == 1 {
          let search_q = (*machine_search).to_lowercase();
          let v_filter = (*machine_vendor_filter).clone();
          let filtered_machines: Vec<CatalogMachine> = catalog
            .machines
            .iter()
            .filter(|m| {
              let vendor_ok = v_filter == "All" || m.vendor == v_filter;
              let search_ok = search_q.is_empty()
                || m.name.to_lowercase().contains(&search_q)
                || m.vendor.to_lowercase().contains(&search_q)
                || m.kinematics.to_lowercase().contains(&search_q)
                || m.build_volume.to_lowercase().contains(&search_q);
              vendor_ok && search_ok
            })
            .cloned()
            .collect();

          html! {
            <div class="flex flex-col gap-space-md bg-surface-container-lowest p-space-lg rounded-xl border border-outline-variant">
              <div class="flex flex-wrap items-center justify-between gap-space-sm">
                <div class="flex flex-col gap-0.5">
                  <h3 class="text-sm font-bold text-on-surface flex items-center gap-2">
                    <span class="material-symbols-outlined text-primary">{"precision_manufacturing"}</span>
                    {format!("Step 1: Select Printer Model ({} Matching of {} in OrcaSlicer Repo)", filtered_machines.len(), catalog.machines.len())}
                  </h3>
                  <p class="text-xs text-on-surface-variant">
                    {"Imported directly from OrcaSlicer GitHub resources/profiles via Bazel http_archive + genrule."}
                  </p>
                </div>

                // Search input
                <div class="relative w-full sm:w-72">
                  <span class="material-symbols-outlined text-sm text-on-surface-variant absolute left-2.5 top-1/2 -translate-y-1/2">{"search"}</span>
                  <input
                    type="text"
                    value={(*machine_search).clone()}
                    placeholder="Filter machines (e.g. Kobra S1, X1C, H2D, Prusa)..."
                    class="w-full bg-surface-container-low border border-outline-variant rounded-lg pl-8 pr-7 py-1.5 text-xs text-on-surface font-mono"
                    oninput={
                      let ms = machine_search.clone();
                      Callback::from(move |e: InputEvent| {
                        let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                        ms.set(input.value());
                      })
                    }
                  />
                  {if !machine_search.is_empty() {
                    let ms = machine_search.clone();
                    html! {
                      <button
                        onclick={Callback::from(move |_| ms.set(String::new()))}
                        class="absolute right-2 top-1/2 -translate-y-1/2 text-on-surface-variant hover:text-on-surface text-xs"
                      >{"✕"}</button>
                    }
                  } else {
                    html! {}
                  }}
                </div>
              </div>

              // Vendor Quick-Filter Pills
              <div class="flex items-center gap-1.5 flex-wrap max-h-20 overflow-y-auto pr-1">
                {for vendor_list.iter().map(|v| {
                  let active = *machine_vendor_filter == *v;
                  let v_str = v.clone();
                  let vf = machine_vendor_filter.clone();
                  html! {
                    <button
                      onclick={Callback::from(move |_| vf.set(v_str.clone()))}
                      class={format!(
                        "px-2.5 py-1 rounded-full text-xs font-mono border transition-colors {}",
                        if active {
                          "bg-primary text-on-primary border-primary font-bold"
                        } else {
                          "bg-surface-container-low text-on-surface-variant border-outline-variant hover:text-on-surface"
                        }
                      )}
                    >
                      {v.clone()}
                    </button>
                  }
                })}
              </div>

              <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-space-sm max-h-80 overflow-y-auto pr-1">
                {for filtered_machines.iter().take(90).map(|m| {
                  let active = *selected_machine_id == m.id;
                  let m_clone = m.clone();
                  let sm = selected_machine_id.clone();
                  let sn = selected_nozzle_id.clone();
                  let nd = nozzle_diameter.clone();
                  let sb = selected_bed_id.clone();
                  let sams = selected_ams_id.clone();
                  let diams_badge = m
                    .available_nozzle_diameters
                    .iter()
                    .map(|d| format!("{:.2}", d).trim_end_matches('0').trim_end_matches('.').to_string())
                    .collect::<Vec<_>>()
                    .join(" / ");
                  html! {
                    <button
                      onclick={Callback::from(move |_| {
                        sm.set(m_clone.id.clone());
                        nd.set(m_clone.default_nozzle_diameter);
                        let default_noz_id = if m_clone.default_nozzle_material == "Hardened Steel" {
                          "orca_hardened_steel"
                        } else if m_clone.default_nozzle_material == "Stainless Steel" {
                          "orca_stainless_steel"
                        } else {
                          "orca_brass"
                        };
                        sn.set(default_noz_id.to_string());
                        if !m_clone.supported_bed_ids.contains(&(*sb)) {
                          sb.set(m_clone.default_bed_id.clone());
                        }
                        sams.set(m_clone.default_ams_id.clone());
                      })}
                      class={format!(
                        "flex flex-col items-start p-space-md rounded-xl border text-left transition-all gap-1 {}",
                        if active {
                          "bg-primary/15 border-primary ring-1 ring-primary"
                        } else {
                          "bg-surface-container-low border-outline-variant hover:border-outline"
                        }
                      )}
                    >
                      <div class="flex items-center justify-between w-full">
                        <span class="text-[10px] font-mono uppercase tracking-wider px-2 py-0.5 rounded bg-surface-container-high text-primary font-bold">
                          {&m.vendor}
                        </span>
                        <span class="text-[10px] font-mono text-on-surface-variant">{&m.build_volume}</span>
                      </div>
                      <span class="text-sm font-bold text-on-surface mt-0.5">{&m.name}</span>
                      <span class="text-[11px] text-on-surface-variant">{&m.kinematics}</span>
                      <div class="flex items-center gap-1.5 mt-1 flex-wrap">
                        <span class="text-[10px] font-mono bg-tertiary-container/20 text-tertiary px-1.5 py-0.5 rounded">
                          {format!("Stock: {} ({}mm)", m.default_nozzle_material, diams_badge)}
                        </span>
                      </div>
                    </button>
                  }
                })}
              </div>
            </div>
          }
        } else {
          html! {}
        }}

        // STEP 2: SELECT NOZZLE (MICROSWISS DIAMONDBACK / CM2 / CHT / BRASS + STOCK ORCASLICER, VALID DIAMETERS ONLY)
        {if *step == 2 {
          let search_q = (*nozzle_search).to_lowercase();
          let b_filter = (*nozzle_brand_filter).clone();
          let filtered_nozzles: Vec<CatalogNozzle> = catalog
            .nozzles
            .iter()
            .filter(|n| {
              let brand_ok = match b_filter.as_str() {
                "All" => true,
                "MicroSwiss" => n.brand == "MicroSwiss",
                "OrcaSlicer Standard" => n.brand == "OrcaSlicer Standard",
                "Aftermarket" => n.brand != "MicroSwiss" && n.brand != "OrcaSlicer Standard",
                _ => true,
              };
              let search_ok = search_q.is_empty()
                || n.name.to_lowercase().contains(&search_q)
                || n.material.to_lowercase().contains(&search_q)
                || n.brand.to_lowercase().contains(&search_q)
                || n.description.to_lowercase().contains(&search_q);
              brand_ok && search_ok
            })
            .cloned()
            .collect();

          let all_candidate_diameters: [f32; 5] = [0.2, 0.25, 0.4, 0.6, 0.8];

          html! {
            <div class="flex flex-col gap-space-md bg-surface-container-lowest p-space-lg rounded-xl border border-outline-variant">
              <div class="flex flex-wrap items-center justify-between gap-space-sm">
                <div class="flex flex-col gap-0.5">
                  <h3 class="text-sm font-bold text-on-surface flex items-center gap-2">
                    <span class="material-symbols-outlined text-primary">{"flare"}</span>
                    {"Step 2: Select Hotend Nozzle & Valid Orifice Diameter"}
                  </h3>
                  <p class="text-xs text-on-surface-variant">
                    {"Note: MicroSwiss Diamondback™, CM2™, and CHT High-Flow only exist in 0.4mm; MicroSwiss Plated Brass is required for 0.2mm or 0.6mm."}
                  </p>
                </div>

                // Search input
                <div class="relative w-full sm:w-72">
                  <span class="material-symbols-outlined text-sm text-on-surface-variant absolute left-2.5 top-1/2 -translate-y-1/2">{"search"}</span>
                  <input
                    type="text"
                    value={(*nozzle_search).clone()}
                    placeholder="Filter nozzles (e.g. MicroSwiss, Diamondback, CM2, Brass)..."
                    class="w-full bg-surface-container-low border border-outline-variant rounded-lg pl-8 pr-7 py-1.5 text-xs text-on-surface font-mono"
                    oninput={
                      let ns = nozzle_search.clone();
                      Callback::from(move |e: InputEvent| {
                        let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                        ns.set(input.value());
                      })
                    }
                  />
                  {if !nozzle_search.is_empty() {
                    let ns = nozzle_search.clone();
                    html! {
                      <button
                        onclick={Callback::from(move |_| ns.set(String::new()))}
                        class="absolute right-2 top-1/2 -translate-y-1/2 text-on-surface-variant hover:text-on-surface text-xs"
                      >{"✕"}</button>
                    }
                  } else {
                    html! {}
                  }}
                </div>
              </div>

              // Brand Filter Pills
              <div class="flex items-center gap-1.5 flex-wrap">
                {for ["All", "MicroSwiss", "OrcaSlicer Standard", "Aftermarket"].iter().map(|b| {
                  let active = *nozzle_brand_filter == *b;
                  let b_str = b.to_string();
                  let bf = nozzle_brand_filter.clone();
                  html! {
                    <button
                      onclick={Callback::from(move |_| bf.set(b_str.clone()))}
                      class={format!(
                        "px-2.5 py-1 rounded-full text-xs font-mono border transition-colors {}",
                        if active {
                          "bg-primary text-on-primary border-primary font-bold"
                        } else {
                          "bg-surface-container-low text-on-surface-variant border-outline-variant hover:text-on-surface"
                        }
                      )}
                    >
                      {*b}
                    </button>
                  }
                })}
              </div>

              // Valid Orifice Diameter Selector (enforces cur_nozzle.available_diameters!)
              <div class="flex flex-col gap-1.5 bg-surface-container-low p-space-md rounded-xl border border-outline-variant/60">
                <div class="flex flex-wrap items-center justify-between gap-2">
                  <div class="flex items-center gap-space-sm flex-wrap">
                    <span class="text-xs font-bold text-on-surface">{"Orifice Diameter (mm):"}</span>
                    {for all_candidate_diameters.iter().map(|d| {
                      let d_val = *d;
                      let is_supported = cur_nozzle.supports_diameter(d_val);
                      let active = (*nozzle_diameter - d_val).abs() < 0.005;
                      let nd = nozzle_diameter.clone();
                      let label = format!("{:.2} mm", d_val).replace(".00", ".0").replace("0 mm", " mm");
                      html! {
                        <button
                          disabled={!is_supported}
                          title={if is_supported {
                            format!("Select {} orifice", label)
                          } else {
                            format!("{} is not manufactured in {}", cur_nozzle.name, label)
                          }}
                          onclick={Callback::from(move |_| {
                            if is_supported {
                              nd.set(d_val);
                            }
                          })}
                          class={format!(
                            "px-3 py-1 rounded-lg font-mono text-xs border transition-colors {}",
                            if active && is_supported {
                              "bg-primary text-on-primary border-primary font-bold"
                            } else if is_supported {
                              "bg-surface-container-lowest text-on-surface border-outline-variant hover:border-outline"
                            } else {
                              "bg-surface-container-lowest/40 text-on-surface-variant/35 border-outline-variant/25 cursor-not-allowed line-through"
                            }
                          )}
                        >
                          {if is_supported {
                            label
                          } else {
                            format!("{} (N/A)", label)
                          }}
                        </button>
                      }
                    })}
                  </div>
                  <span class="text-xs font-mono text-tertiary">
                    {format!("Effective Volumetric Ceiling: {:.1} mm³/s", effective_max_flow)}
                  </span>
                </div>
                {if cur_nozzle.brand == "MicroSwiss" && cur_nozzle.available_diameters == vec![0.4] {
                  html! {
                    <div class="text-[11px] font-mono text-amber-300 flex items-center gap-1.5 mt-1">
                      <span class="material-symbols-outlined text-sm">{"info"}</span>
                      {format!(
                        "{} is only manufactured in 0.4 mm. To use 0.2 mm or 0.6 mm on MicroSwiss FlowTech™, select MicroSwiss FlowTech™ Plated Brass.",
                        cur_nozzle.name
                      )}
                    </div>
                  }
                } else {
                  html! {}
                }}
              </div>

              // Nozzle Material & Brand Cards
              <div class="grid grid-cols-1 md:grid-cols-2 gap-space-sm max-h-72 overflow-y-auto pr-1">
                {for filtered_nozzles.iter().map(|n| {
                  let active = *selected_nozzle_id == n.id;
                  let n_clone = n.clone();
                  let sn = selected_nozzle_id.clone();
                  let nd = nozzle_diameter.clone();
                  let cur_d = *nozzle_diameter;
                  let diams_str = n
                    .available_diameters
                    .iter()
                    .map(|d| format!("{:.2}mm", d).replace("0mm", "mm"))
                    .collect::<Vec<_>>()
                    .join(", ");
                  html! {
                    <button
                      onclick={Callback::from(move |_| {
                        sn.set(n_clone.id.clone());
                        // Enforce valid nozzle + diameter combination!
                        if !n_clone.supports_diameter(cur_d) {
                          if let Some(&first_valid) = n_clone.available_diameters.first() {
                            nd.set(first_valid);
                          }
                        }
                      })}
                      class={format!(
                        "flex flex-col items-start p-space-md rounded-xl border text-left transition-all gap-1 {}",
                        if active {
                          "bg-primary/15 border-primary ring-1 ring-primary"
                        } else {
                          "bg-surface-container-low border-outline-variant hover:border-outline"
                        }
                      )}
                    >
                      <div class="flex items-center justify-between w-full gap-2 flex-wrap">
                        <div class="flex items-center gap-1.5">
                          <span class={format!(
                            "text-[10px] font-mono px-2 py-0.5 rounded font-bold {}",
                            if n.brand == "MicroSwiss" {
                              "bg-amber-500/20 text-amber-300 border border-amber-500/30"
                            } else {
                              "bg-surface-container-high text-primary"
                            }
                          )}>
                            {&n.brand}
                          </span>
                          <span class="text-sm font-bold text-on-surface">{&n.name}</span>
                        </div>
                        <span class={format!(
                          "text-[10px] font-mono px-2 py-0.5 rounded {}",
                          if n.temp_offset_c < 0 {
                            "bg-tertiary-container/30 text-tertiary"
                          } else if n.temp_offset_c > 0 {
                            "bg-amber-500/20 text-amber-300"
                          } else {
                            "bg-surface-container-high text-on-surface-variant"
                          }
                        )}>
                          {format!("{:+}°C Rule", n.temp_offset_c)}
                        </span>
                      </div>
                      <span class="text-xs text-on-surface-variant">{&n.description}</span>
                      <div class="flex items-center justify-between w-full mt-1 flex-wrap gap-1">
                        <span class="text-[10px] font-mono text-primary">
                          {format!(
                            "Base Flow (0.4mm): {:.0} mm³/s • Retract: {:.1}mm • Melt Zone: {:.2}×",
                            n.base_flow_mm3s, n.retraction_mm, n.melt_zone_mult
                          )}
                        </span>
                        <span class={format!(
                          "text-[10px] font-mono px-2 py-0.5 rounded font-bold {}",
                          if n.available_diameters.len() == 1 {
                            "bg-amber-500/15 text-amber-300 border border-amber-500/30"
                          } else {
                            "bg-tertiary-container/20 text-tertiary"
                          }
                        )}>
                          {format!("Sizes: {}", diams_str)}
                        </span>
                      </div>
                    </button>
                  }
                })}
              </div>

              // Optional Toolhead Cooling Add-ons
              <div class="flex flex-col gap-1.5 pt-1 border-t border-outline-variant/50">
                <span class="text-xs font-bold text-on-surface">{"Optional Toolhead & Cooling Add-ons:"}</span>
                <div class="flex flex-wrap gap-2">
                  {for [
                    "MicroSwiss FlowTech High-Flow Cooling Duct",
                    "Hollow Front/Back Toolhead Covers (+Cooling)",
                    "Silicone Sock Insulated Hotend",
                    " CPAP / Auxiliary Part Cooling Blower"
                  ].iter().map(|addon| {
                    let a_str = addon.to_string();
                    let active = selected_addons.contains(&a_str);
                    let sa = selected_addons.clone();
                    html! {
                      <button
                        onclick={Callback::from(move |_| {
                          let mut list = (*sa).clone();
                          if let Some(pos) = list.iter().position(|x| x == &a_str) {
                            list.remove(pos);
                          } else {
                            list.push(a_str.clone());
                          }
                          sa.set(list);
                        })}
                        class={format!(
                          "px-3 py-1.5 rounded-lg text-xs font-mono border flex items-center gap-1.5 transition-colors {}",
                          if active {
                            "bg-tertiary-container/25 border-tertiary text-tertiary font-bold"
                          } else {
                            "bg-surface-container-low border-outline-variant text-on-surface-variant hover:text-on-surface"
                          }
                        )}
                      >
                        <span class="material-symbols-outlined text-sm">
                          {if active { "check_box" } else { "check_box_outline_blank" }}
                        </span>
                        {*addon}
                      </button>
                    }
                  })}
                </div>
              </div>
            </div>
          }
        } else {
          html! {}
        }}

        // STEP 3: SELECT VALID BED PLATE FOR MACHINE
        {if *step == 3 {
          let search_q = (*bed_search).to_lowercase();
          let filtered_beds: Vec<CatalogBed> = valid_beds
            .iter()
            .filter(|b| {
              search_q.is_empty()
                || b.name.to_lowercase().contains(&search_q)
                || b.surface_code.to_lowercase().contains(&search_q)
                || b.description.to_lowercase().contains(&search_q)
            })
            .cloned()
            .collect();

          html! {
            <div class="flex flex-col gap-space-md bg-surface-container-lowest p-space-lg rounded-xl border border-outline-variant">
              <div class="flex flex-wrap items-center justify-between gap-space-sm">
                <div class="flex flex-col gap-0.5">
                  <h3 class="text-sm font-bold text-on-surface flex items-center gap-2">
                    <span class="material-symbols-outlined text-primary">{"grid_on"}</span>
                    {format!("Step 3: Select Valid Build Plate for {} ({})", cur_machine.name, cur_machine.vendor)}
                  </h3>
                  <p class="text-xs text-on-surface-variant">
                    {"Showing build plates compatible with your selected machine from OrcaSlicer GitHub + Additional Catalog."}
                  </p>
                </div>

                // Search input
                <div class="relative w-full sm:w-72">
                  <span class="material-symbols-outlined text-sm text-on-surface-variant absolute left-2.5 top-1/2 -translate-y-1/2">{"search"}</span>
                  <input
                    type="text"
                    value={(*bed_search).clone()}
                    placeholder="Filter valid plates (e.g. PEI, CryoGrip, G10)..."
                    class="w-full bg-surface-container-low border border-outline-variant rounded-lg pl-8 pr-7 py-1.5 text-xs text-on-surface font-mono"
                    oninput={
                      let bs = bed_search.clone();
                      Callback::from(move |e: InputEvent| {
                        let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                        bs.set(input.value());
                      })
                    }
                  />
                  {if !bed_search.is_empty() {
                    let bs = bed_search.clone();
                    html! {
                      <button
                        onclick={Callback::from(move |_| bs.set(String::new()))}
                        class="absolute right-2 top-1/2 -translate-y-1/2 text-on-surface-variant hover:text-on-surface text-xs"
                      >{"✕"}</button>
                    }
                  } else {
                    html! {}
                  }}
                </div>
              </div>

              <div class="grid grid-cols-1 md:grid-cols-2 gap-space-sm">
                {for filtered_beds.iter().map(|b| {
                  let active = cur_bed.id == b.id;
                  let b_id = b.id.clone();
                  let sb = selected_bed_id.clone();
                  html! {
                    <button
                      onclick={Callback::from(move |_| sb.set(b_id.clone()))}
                      class={format!(
                        "flex flex-col items-start p-space-md rounded-xl border text-left transition-all gap-1 {}",
                        if active {
                          "bg-primary/15 border-primary ring-1 ring-primary"
                        } else {
                          "bg-surface-container-low border-outline-variant hover:border-outline"
                        }
                      )}
                    >
                      <div class="flex items-center justify-between w-full">
                        <span class="text-sm font-bold text-on-surface">{&b.name}</span>
                        <span class={format!(
                          "text-[10px] font-mono px-2 py-0.5 rounded {}",
                          if b.temp_offset_c < 0 {
                            "bg-tertiary-container/30 text-tertiary"
                          } else if b.temp_offset_c > 0 {
                            "bg-amber-500/20 text-amber-300"
                          } else {
                            "bg-surface-container-high text-on-surface-variant"
                          }
                        )}>
                          {format!("{:+}°C Bed Rule", b.temp_offset_c)}
                        </span>
                      </div>
                      <span class="text-xs text-on-surface-variant">{&b.description}</span>
                      <span class="text-[10px] font-mono text-primary mt-0.5">{&b.source}</span>
                    </button>
                  }
                })}
              </div>
            </div>
          }
        } else {
          html! {}
        }}

        // STEP 4: SELECT COMPATIBLE AMS / MMU CONFIGURATION FOR MACHINE
        {if *step == 4 {
          let search_q = (*ams_search).to_lowercase();
          let filtered_ams: Vec<CatalogAmsOption> = valid_ams
            .iter()
            .filter(|a| {
              search_q.is_empty()
                || a.name.to_lowercase().contains(&search_q)
                || a.vendor.to_lowercase().contains(&search_q)
                || a.description.to_lowercase().contains(&search_q)
            })
            .cloned()
            .collect();

          html! {
            <div class="flex flex-col gap-space-md bg-surface-container-lowest p-space-lg rounded-xl border border-outline-variant">
              <div class="flex flex-wrap items-center justify-between gap-space-sm">
                <div class="flex flex-col gap-0.5">
                  <h3 class="text-sm font-bold text-on-surface flex items-center gap-2">
                    <span class="material-symbols-outlined text-primary">{"view_week"}</span>
                    {format!("Step 4: Select Compatible AMS / MMU for {} ({})", cur_machine.name, cur_machine.vendor)}
                  </h3>
                  <p class="text-xs text-on-surface-variant">
                    {"Showing only multi-material units and spool feeders compatible with your selected machine."}
                  </p>
                </div>

                // Search input
                <div class="relative w-full sm:w-72">
                  <span class="material-symbols-outlined text-sm text-on-surface-variant absolute left-2.5 top-1/2 -translate-y-1/2">{"search"}</span>
                  <input
                    type="text"
                    value={(*ams_search).clone()}
                    placeholder="Filter compatible AMS / feeder..."
                    class="w-full bg-surface-container-low border border-outline-variant rounded-lg pl-8 pr-7 py-1.5 text-xs text-on-surface font-mono"
                    oninput={
                      let as_state = ams_search.clone();
                      Callback::from(move |e: InputEvent| {
                        let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                        as_state.set(input.value());
                      })
                    }
                  />
                  {if !ams_search.is_empty() {
                    let as_state = ams_search.clone();
                    html! {
                      <button
                        onclick={Callback::from(move |_| as_state.set(String::new()))}
                        class="absolute right-2 top-1/2 -translate-y-1/2 text-on-surface-variant hover:text-on-surface text-xs"
                      >{"✕"}</button>
                    }
                  } else {
                    html! {}
                  }}
                </div>
              </div>

              <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-space-sm">
                {for filtered_ams.iter().map(|a| {
                  let active = cur_ams.id == a.id;
                  let a_id = a.id.clone();
                  let sams = selected_ams_id.clone();
                  html! {
                    <button
                      onclick={Callback::from(move |_| sams.set(a_id.clone()))}
                      class={format!(
                        "flex flex-col items-start p-space-md rounded-xl border text-left transition-all gap-1.5 {}",
                        if active {
                          "bg-primary/15 border-primary ring-1 ring-primary"
                        } else {
                          "bg-surface-container-low border-outline-variant hover:border-outline"
                        }
                      )}
                    >
                      <div class="flex items-center justify-between w-full">
                        <span class="text-[10px] font-mono px-2 py-0.5 rounded bg-surface-container-high text-primary font-bold">
                          {&a.vendor}
                        </span>
                        <span class="text-xs font-mono bg-tertiary-container/30 text-tertiary px-2 py-0.5 rounded font-bold">
                          {format!("{} Slots", a.total_slots)}
                        </span>
                      </div>
                      <span class="text-sm font-bold text-on-surface">{&a.name}</span>
                      <span class="text-xs text-on-surface-variant">{&a.description}</span>
                    </button>
                  }
                })}
              </div>
            </div>
          }
        } else {
          html! {}
        }}

        // STEP 5: FILAMENTS (STARTS EMPTY • NO PLA-COMPOSITE BOX • DISTINCT CF/GF • PEBA/PVA/PHTA/PPA/REBOUND TPU • COLOR SWATCH GRID)
        {if *step == 5 {
          let cat_search = (*filament_catalog_search).to_lowercase();
          let cat_filter = (*filament_category_filter).clone();

          let filtered_catalog: Vec<CatalogFilament> = catalog
            .filaments
            .iter()
            .filter(|f| {
              let cat_ok = cat_filter == "All" || f.category == cat_filter;
              let search_ok = cat_search.is_empty()
                || f.code.to_lowercase().contains(&cat_search)
                || f.label.to_lowercase().contains(&cat_search)
                || f.category.to_lowercase().contains(&cat_search);
              cat_ok && search_ok
            })
            .cloned()
            .collect();

          let spool_search = (*added_spools_search).to_lowercase();
          // Note: "PLA Composite (-CF/-GF)" is removed because PLA-CF and PLA-GF are in "PLA Family"
          let categories = [
            "All",
            "PLA Family",
            "PETG / PCTG / PET",
            "PC / Engineering",
            "ABS / ASA",
            "Nylon (PA / PAHT / PPA)",
            "TPU / TPE / PEBA Flex",
            "PVA / Support",
          ];

          html! {
            <div class="flex flex-col gap-space-md bg-surface-container-lowest p-space-lg rounded-xl border border-outline-variant">
              <div class="flex flex-wrap items-center justify-between gap-space-sm">
                <div class="flex flex-col gap-0.5">
                  <h3 class="text-sm font-bold text-on-surface flex items-center gap-2">
                    <span class="material-symbols-outlined text-primary">{"Autorenew"}</span>
                    {"Step 5: Build Your Filament Spool Loadout (Starts Empty — Add, Color-Pick, Reorder & Remove)"}
                  </h3>
                  <p class="text-xs text-on-surface-variant">
                    {"Includes OrcaSlicer GitHub polymers + PEBA, PVA, PAHT/PHTA, PPA, Rebound TPU, and distinct -CF vs -GF profiles."}
                  </p>
                </div>
                <span class="text-xs font-mono text-tertiary bg-tertiary-container/20 px-2.5 py-1 rounded">
                  {format!(
                    "Active Rules: {} {:+}°C • {} {:+}°C",
                    cur_nozzle.material, cur_nozzle.temp_offset_c, cur_bed.surface_code, cur_bed.temp_offset_c
                  )}
                </span>
              </div>

              <div class="flex flex-col gap-space-md">
                // TOP SECTION (FULL WIDTH): Added Spools / Slots (Starts Empty, Spacious Non-Overlapping Rows)
                <div class="flex flex-col gap-space-sm bg-surface-container-low p-space-md rounded-xl border border-outline-variant">
                  <div class="flex flex-wrap items-center justify-between gap-2">
                    <span class="text-xs font-bold text-on-surface uppercase tracking-wider font-mono">
                      {format!("1. Added Spools / Slots ({})", spool_drafts.len())}
                    </span>
                    <div class="flex items-center gap-3">
                      {if spool_drafts.len() > 1 {
                        html! {
                          <div class="relative w-48 sm:w-64">
                            <span class="material-symbols-outlined text-sm text-on-surface-variant absolute left-2.5 top-1/2 -translate-y-1/2">{"search"}</span>
                            <input
                              type="text"
                              value={(*added_spools_search).clone()}
                              placeholder="Filter added spools..."
                              class="w-full bg-surface-container-lowest border border-outline-variant rounded-lg pl-8 pr-7 py-1 text-xs text-on-surface font-mono"
                              oninput={
                                let ass = added_spools_search.clone();
                                Callback::from(move |e: InputEvent| {
                                  let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                                  ass.set(input.value());
                                })
                              }
                            />
                          </div>
                        }
                      } else {
                        html! {}
                      }}
                      {if !spool_drafts.is_empty() {
                        let sd = spool_drafts.clone();
                        html! {
                          <button
                            onclick={Callback::from(move |_| {
                              sd.set(Vec::new());
                            })}
                            class="text-[11px] font-mono text-error hover:underline"
                          >
                            {"Clear All"}
                          </button>
                        }
                      } else {
                        html! {}
                      }}
                    </div>
                  </div>

                  {if spool_drafts.is_empty() {
                    html! {
                      <div class="flex items-center justify-center border border-dashed border-outline-variant rounded-xl py-4 px-space-md text-center gap-2">
                        <span class="material-symbols-outlined text-xl text-on-surface-variant/60">{"inventory_2"}</span>
                        <span class="text-xs text-on-surface-variant">
                          {"No filaments added yet — click "}
                          <strong class="text-primary font-mono">{"+ Add"}</strong>
                          {" on any polymer below to add a spool, then click its color square to pick a color."}
                        </span>
                      </div>
                    }
                  } else {
                    let total_len = spool_drafts.len();
                    html! {
                      <div class="flex flex-col gap-1.5 max-h-56 overflow-y-auto pr-1">
                        {for (*spool_drafts).iter().enumerate().filter(|(_, d)| {
                          spool_search.is_empty()
                            || d.name.to_lowercase().contains(&spool_search)
                            || d.material_type.to_lowercase().contains(&spool_search)
                            || d.finish_type.to_lowercase().contains(&spool_search)
                        }).map(|(idx, draft)| {
                          let sd_name = spool_drafts.clone();
                          let sd_col = spool_drafts.clone();
                          let sd_fin = spool_drafts.clone();
                          let sd_up = spool_drafts.clone();
                          let sd_down = spool_drafts.clone();
                          let sd_del = spool_drafts.clone();

                          let mat_info = find_filament_entry(
                            &catalog.filaments,
                            &draft.material_type,
                            &draft.finish_type,
                          );
                          let base_n = mat_info.map(|m| m.base_nozzle_c).unwrap_or(215);
                          let base_b = mat_info.map(|m| m.base_bed_c).unwrap_or(60);
                          let eff_nozzle = (base_n as i32 + cur_nozzle.temp_offset_c).clamp(175, 350);
                          let eff_bed = (base_b as i32 + cur_bed.temp_offset_c).clamp(30, 130);

                          html! {
                            <div class="flex flex-wrap sm:flex-nowrap items-center justify-between bg-surface-container-lowest px-3 py-2 rounded-lg border border-outline-variant gap-3 font-mono text-xs">
                              <div class="flex items-center gap-2.5 min-w-0 flex-1">
                                <span class="bg-primary/20 text-primary px-2 py-0.5 rounded font-bold shrink-0 text-xs">
                                  {format!("#{}", idx + 1)}
                                </span>

                                // Compact Color Swatch Square (no HEX string clutter!)
                                <ColorPicker
                                  compact=true
                                  value_hex={draft.color_hex.clone()}
                                  title_label={Some(format!("Slot #{} — {} Color", idx + 1, draft.name))}
                                  on_change={Callback::from(move |sel: ColorSelection| {
                                    let mut list = (*sd_col).clone();
                                    if let Some(item) = list.get_mut(idx) {
                                      item.color_hex = sel.hex;
                                    }
                                    sd_col.set(list);
                                  })}
                                />

                                <input
                                  type="text"
                                  value={draft.name.clone()}
                                  class="bg-surface-container-low border border-outline-variant/60 rounded px-2.5 py-1 text-on-surface flex-1 min-w-[140px] text-xs"
                                  oninput={Callback::from(move |e: InputEvent| {
                                    let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                                    let mut list = (*sd_name).clone();
                                    if let Some(item) = list.get_mut(idx) {
                                      item.name = input.value();
                                    }
                                    sd_name.set(list);
                                  })}
                                />

                                <span class="text-[11px] bg-surface-container-high text-primary px-2 py-0.5 rounded font-bold shrink-0">
                                  {&draft.material_type}
                                </span>
                              </div>

                              <div class="flex items-center gap-2 shrink-0">
                                <select
                                  class="bg-surface-container-low border border-outline-variant/60 rounded px-2 py-1 text-xs text-on-surface"
                                  value={draft.finish_type.clone()}
                                  onchange={Callback::from(move |e: Event| {
                                    let sel: web_sys::HtmlInputElement = e.target_unchecked_into();
                                    let mut list = (*sd_fin).clone();
                                    if let Some(item) = list.get_mut(idx) {
                                      let new_fin = sel.value();
                                      if item.name == item.material_type
                                        || item.name.starts_with(&format!("{} ", item.material_type))
                                      {
                                        item.name = if let Some(suf) =
                                          FilamentSpool::finish_title_suffix(&new_fin)
                                        {
                                          format!("{} {}", item.material_type, suf)
                                        } else {
                                          item.material_type.clone()
                                        };
                                      }
                                      item.finish_type = new_fin;
                                    }
                                    sd_fin.set(list);
                                  })}
                                >
                                  {for crate::catalog::FINISH_OPTIONS.iter().map(|(val, label)| {
                                    html! {
                                      <option value={*val} selected={draft.finish_type == *val}>{*label}</option>
                                    }
                                  })}
                                </select>

                                <span class="text-xs text-tertiary bg-tertiary-container/20 px-2 py-0.5 rounded shrink-0">
                                  {format!("{}°C / {}°C", eff_nozzle, eff_bed)}
                                </span>

                                // Reorder Up (`↑`)
                                <button
                                  disabled={idx == 0}
                                  title="Move slot up"
                                  onclick={Callback::from(move |_| {
                                    if idx > 0 {
                                      let mut list = (*sd_up).clone();
                                      list.swap(idx, idx - 1);
                                      sd_up.set(list);
                                    }
                                  })}
                                  class={format!(
                                    "px-2 py-1 rounded border text-xs {}",
                                    if idx == 0 {
                                      "border-outline-variant/30 text-on-surface-variant/30 cursor-not-allowed"
                                    } else {
                                      "border-outline-variant text-on-surface hover:bg-surface-container-high"
                                    }
                                  )}
                                >
                                  {"↑"}
                                </button>

                                // Reorder Down (`↓`)
                                <button
                                  disabled={idx + 1 >= total_len}
                                  title="Move slot down"
                                  onclick={Callback::from(move |_| {
                                    let mut list = (*sd_down).clone();
                                    if idx + 1 < list.len() {
                                      list.swap(idx, idx + 1);
                                      sd_down.set(list);
                                    }
                                  })}
                                  class={format!(
                                    "px-2 py-1 rounded border text-xs {}",
                                    if idx + 1 >= total_len {
                                      "border-outline-variant/30 text-on-surface-variant/30 cursor-not-allowed"
                                    } else {
                                      "border-outline-variant text-on-surface hover:bg-surface-container-high"
                                    }
                                  )}
                                >
                                  {"↓"}
                                </button>

                                // Remove (`✕`)
                                <button
                                  title="Remove spool"
                                  onclick={Callback::from(move |_| {
                                    let mut list = (*sd_del).clone();
                                    if idx < list.len() {
                                      list.remove(idx);
                                    }
                                    sd_del.set(list);
                                  })}
                                  class="px-2 py-1 rounded border border-error/40 text-error hover:bg-error/20 text-xs"
                                >
                                  {"✕"}
                                </button>
                              </div>
                            </div>
                          }
                        })}
                      </div>
                    }
                  }}
                </div>

                // BOTTOM SECTION (FULL WIDTH): Searchable Polymer & Composite Catalog
                <div class="flex flex-col gap-space-sm bg-surface-container-low p-space-md rounded-xl border border-outline-variant">
                  <div class="flex flex-wrap items-center justify-between gap-2">
                    <span class="text-xs font-bold text-on-surface uppercase tracking-wider font-mono">
                      {format!("2. Polymer Catalog ({} Materials)", filtered_catalog.len())}
                    </span>
                    <div class="relative w-full sm:w-72">
                      <span class="material-symbols-outlined text-sm text-on-surface-variant absolute left-2.5 top-1/2 -translate-y-1/2">{"search"}</span>
                      <input
                        type="text"
                        value={(*filament_catalog_search).clone()}
                        placeholder="Search PEBA, PVA, PHTA, PPA, Rebound TPU, -CF, -GF..."
                        class="w-full bg-surface-container-lowest border border-outline-variant rounded-lg pl-8 pr-7 py-1 text-xs text-on-surface font-mono"
                        oninput={
                          let fcs = filament_catalog_search.clone();
                          Callback::from(move |e: InputEvent| {
                            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                            fcs.set(input.value());
                          })
                        }
                      />
                      {if !filament_catalog_search.is_empty() {
                        let fcs = filament_catalog_search.clone();
                        html! {
                          <button
                            onclick={Callback::from(move |_| fcs.set(String::new()))}
                            class="absolute right-2 top-1/2 -translate-y-1/2 text-on-surface-variant hover:text-on-surface text-xs"
                          >{"✕"}</button>
                        }
                      } else {
                        html! {}
                      }}
                    </div>
                  </div>

                  // Category Pills
                  <div class="flex items-center gap-1 flex-wrap">
                    {for categories.iter().map(|cat| {
                      let active = *filament_category_filter == *cat;
                      let cat_str = cat.to_string();
                      let fcf = filament_category_filter.clone();
                      html! {
                        <button
                          onclick={Callback::from(move |_| fcf.set(cat_str.clone()))}
                          class={format!(
                            "px-2.5 py-0.5 rounded-full text-[11px] font-mono border transition-colors {}",
                            if active {
                              "bg-primary text-on-primary border-primary font-bold"
                            } else {
                              "bg-surface-container-lowest text-on-surface-variant border-outline-variant hover:text-on-surface"
                            }
                          )}
                        >
                          {*cat}
                        </button>
                      }
                    })}
                  </div>

                  // Catalog Grid with "+ Add" Button (No color dots; no "Generic " prefix; defaults to gray #8E9299)
                  <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-2 max-h-72 overflow-y-auto pr-1">
                    {for filtered_catalog.iter().map(|mat| {
                      let mat_clone = mat.clone();
                      let sd = spool_drafts.clone();
                      let eff_noz = (mat.base_nozzle_c as i32 + cur_nozzle.temp_offset_c).clamp(175, 350);
                      let eff_bed = (mat.base_bed_c as i32 + cur_bed.temp_offset_c).clamp(30, 130);
                      let warn_brass = mat.requires_hardened && cur_nozzle.material.starts_with("Brass");
                      let is_ruby_cfgf = mat.requires_hardened && cur_nozzle.id == "ruby_brass";

                      html! {
                        <div class="flex items-center justify-between bg-surface-container-lowest p-2.5 rounded-lg border border-outline-variant/60 hover:border-outline gap-2">
                          <div class="flex flex-col min-w-0">
                            <div class="flex items-center gap-1.5 flex-wrap">
                              <span class="text-xs font-bold text-on-surface font-mono">{mat.base_material_code()}</span>
                              <span class="text-[10px] font-mono px-1.5 py-0.5 rounded bg-surface-container-high text-on-surface-variant">
                                {&mat.default_finish}
                              </span>
                              {if warn_brass {
                                html! {
                                  <span
                                    title="Abrasive fiber/glow filament — Hardened, Ruby Tip, or Diamondback nozzle recommended"
                                    class="text-[10px] bg-amber-500/20 text-amber-300 px-1.5 py-0.5 rounded font-mono"
                                  >
                                    {"Abrasive"}
                                  </span>
                                }
                              } else if is_ruby_cfgf {
                                html! {
                                  <span
                                    title="Ruby gemstone tip handles -CF and -GF composite fibers with 0°C Brass thermal conductivity"
                                    class="text-[10px] bg-tertiary-container/30 text-tertiary px-1.5 py-0.5 rounded font-mono"
                                  >
                                    {"Ruby CF/GF Ready"}
                                  </span>
                                }
                              } else {
                                html! {}
                              }}
                            </div>
                            <span class="text-[11px] text-on-surface-variant truncate" title={mat.label.clone()}>
                              {&mat.label}
                            </span>
                            <span class="text-[10px] font-mono text-tertiary">
                              {format!(
                                "{}°C / {}°C • {:.1} mm³/s • Fan {}%",
                                eff_noz, eff_bed, mat.max_vol_mm3s, mat.fan_pct
                              )}
                            </span>
                          </div>

                          <button
                            onclick={Callback::from(move |_| {
                              let mut list = (*sd).clone();
                              let draft_name = if let Some(suf) =
                                FilamentSpool::finish_title_suffix(&mat_clone.default_finish)
                              {
                                format!("{} {}", mat_clone.base_material_code(), suf)
                              } else {
                                mat_clone.base_material_code().to_string()
                              };
                              list.push(WizardSpoolDraft {
                                name: draft_name,
                                material_type: mat_clone.base_material_code().to_string(),
                                color_hex: "#8E9299".to_string(),
                                finish_type: mat_clone.default_finish.clone(),
                              });
                              sd.set(list);
                            })}
                            class="bg-primary/20 hover:bg-primary text-primary hover:text-on-primary border border-primary/40 px-3 py-1.5 rounded-lg text-xs font-bold font-mono shrink-0 transition-colors"
                          >
                            {"+ Add"}
                          </button>
                        </div>
                      }
                    })}
                  </div>
                </div>
              </div>
            </div>
          }
        } else {
          html! {}
        }}

        // Footer Navigation Buttons (Previous / Next / Finish Wizard)
        <div class="flex items-center justify-between pt-space-sm border-t border-outline-variant/60">
          <div>
            {if *step > 1 {
              let s_prev = step.clone();
              let cur = *step;
              html! {
                <button
                  onclick={Callback::from(move |_| s_prev.set(cur - 1))}
                  class="bg-surface-container-high hover:bg-surface-bright text-on-surface px-space-lg py-2.5 rounded-xl font-bold text-xs flex items-center gap-1.5 border border-outline-variant transition-colors"
                >
                  <span class="material-symbols-outlined text-base">{"chevron_left"}</span>
                  {"Previous Step"}
                </button>
              }
            } else {
              html! {}
            }}
          </div>

          <div class="flex items-center gap-space-sm">
            {if *step < 5 {
              let s_next = step.clone();
              let cur = *step;
              html! {
                <button
                  onclick={Callback::from(move |_| s_next.set(cur + 1))}
                  class="bg-primary hover:bg-primary-container text-on-primary hover:text-on-primary-container px-space-xl py-2.5 rounded-xl font-bold text-xs flex items-center gap-1.5 shadow-md transition-colors"
                >
                  {"Next Step"}
                  <span class="material-symbols-outlined text-base">{"chevron_right"}</span>
                </button>
              }
            } else {
              html! {
                <button
                  onclick={on_finish_wizard}
                  class="bg-primary hover:bg-primary-container text-on-primary hover:text-on-primary-container px-space-xl py-2.5 rounded-xl font-bold text-sm flex items-center gap-2 shadow-lg transition-colors"
                >
                  <span class="material-symbols-outlined text-lg">{"check_circle"}</span>
                  {format!("Create Clean Rig & {} Filament Spools", spool_drafts.len())}
                </button>
              }
            }}
          </div>
        </div>
      </div>
    </div>
  }
}
