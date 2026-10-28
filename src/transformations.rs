use crate::lua_engine::execute_transformation_lua;
use crate::models::{FilamentSpool, MachineRig};
use crate::orca_profile::{DiffItem, OrcaProfile};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransformationRule {
  pub id: String,
  pub name: String,
  pub description: String,
  pub category: String,
  pub icon: String,
  pub enabled: bool,
  pub condition_expr: String, // e.g. "filament:PETG", "nozzle:diamondback", "all"
  pub lua_code: String,
  pub priority: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExternalConflict {
  pub param_key: String,
  pub param_label: String,
  pub base_value: String,
  pub expected_derived_value: String,
  pub actual_external_value: String,
  pub modifying_rule_id: String,
  #[serde(default)]
  pub source_relative_path: String,
  #[serde(default)]
  pub json_field_key: String,
  #[serde(default)]
  pub clean_raw_value: String,
  #[serde(default)]
  pub disk_raw_value: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PendingDiskUpdate {
  pub param_key: String,
  pub param_label: String,
  pub source_relative_path: String,
  pub json_field_key: String,
  pub old_disk_value: String,
  pub new_clean_value: String,
}

impl PendingDiskUpdate {
  /// Returns `true` only when `old_disk_value` and `new_clean_value` actually differ
  /// (ignoring whitespace, `°C`/`mm` suffixes, or trailing decimal zeros like `0.40` vs `0.4`).
  pub fn has_value_change(&self) -> bool {
    !values_are_equivalent(&self.old_disk_value, &self.new_clean_value)
  }
}

pub fn values_are_equivalent(a: &str, b: &str) -> bool {
  let clean_a = a
    .trim()
    .trim_end_matches("°C")
    .trim_end_matches("mm/s")
    .trim_end_matches("mm")
    .trim();
  let clean_b = b
    .trim()
    .trim_end_matches("°C")
    .trim_end_matches("mm/s")
    .trim_end_matches("mm")
    .trim();
  if clean_a.eq_ignore_ascii_case(clean_b) {
    return true;
  }
  if let (Ok(fa), Ok(fb)) = (clean_a.parse::<f64>(), clean_b.parse::<f64>()) {
    if (fa - fb).abs() < 1e-6 {
      return true;
    }
  }
  false
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuleBeforeAfterChange {
  pub rule_id: String,
  pub rule_name: String,
  pub param_key: String,
  pub param_label: String,
  pub before_value: String,
  pub after_value: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StackEvaluationResult {
  pub resulting_profile: OrcaProfile,
  pub diff_items: Vec<DiffItem>,
  pub rule_changes: Vec<RuleBeforeAfterChange>,
  pub flush_matrix: Vec<String>,
  pub safe_speed_ceiling: f64,
  pub execution_logs: Vec<String>,
  pub active_rule_count: usize,
  pub total_rule_count: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FullExportPackage {
  pub version: String,
  pub export_timestamp_millis: u64,
  pub rules: Vec<TransformationRule>,
  pub active_rig_id: String,
  pub notes: String,
}

impl TransformationRule {
  pub fn is_applicable(&self, context: &serde_json::Value) -> bool {
    let cond = self.condition_expr.trim();
    if cond.is_empty() || cond == "all" || cond == "*" {
      return true;
    }

    if let Some(rest) = cond.strip_prefix("filament:") {
      let target_fil = rest.trim();
      if let Some(fil) = context.get("filament_type").and_then(|v| v.as_str()) {
        if fil.eq_ignore_ascii_case(target_fil) {
          return true;
        }
      }
      if let Some(slots) = context.get("slots").and_then(|v| v.as_array()) {
        if slots.iter().any(|s| {
          s.get("material_type")
            .and_then(|m| m.as_str())
            .map(|m| m.eq_ignore_ascii_case(target_fil))
            .unwrap_or(false)
        }) {
          return true;
        }
      }
      if let Some(all_fils) = context.get("all_filament_types").and_then(|v| v.as_array()) {
        if all_fils.iter().any(|m| {
          m.as_str()
            .map(|s| s.eq_ignore_ascii_case(target_fil))
            .unwrap_or(false)
        }) {
          return true;
        }
      }
      return false;
    }

    if let Some(rest) = cond.strip_prefix("nozzle:") {
      if let Some(noz) = context.get("nozzle_type").and_then(|v| v.as_str()) {
        return noz.eq_ignore_ascii_case(rest.trim());
      }
    }

    true
  }
}

pub struct TransformationEngine;

impl TransformationEngine {
  pub fn get_default_rules() -> Vec<TransformationRule> {
    vec![
      TransformationRule {
        id: "thermal_conductivity".to_string(),
        name: "Thermal Conductivity & Nozzle Material Rule".to_string(),
        description: "Dynamically compensates nozzle temperature, flow ratio, and retraction based on nozzle material (Hardened Steel +5°C matching Anycubic PETG - hotbed 235°C; Diamondback PCD -5°C & 0.94 flow; Bi-Metal CHT +25% max flow).".to_string(),
        category: "Dynamic Combination Rule".to_string(),
        icon: "💎".to_string(),
        enabled: true,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/thermal_conductivity.lua").to_string(),
        priority: 10,
      },
      TransformationRule {
        id: "rig_addons_cooling".to_string(),
        name: "Rig Hardware Add-Ons & Cooling Schedule Rule".to_string(),
        description: "Adjusts cooling schedules, fan thresholds, and melt-zone retraction based on installed rig add-ons (Hollow Front/Back Toolhead Covers, MicroSwiss High-Velocity Dual-Fang Duct, or FlowTech All-Metal Hotend).".to_string(),
        category: "Dynamic Combination Rule".to_string(),
        icon: "🌀".to_string(),
        enabled: true,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/rig_addons_cooling.lua").to_string(),
        priority: 12,
      },
      TransformationRule {
        id: "volumetric_speed_ceiling".to_string(),
        name: "Volumetric Speed Ceiling Rule".to_string(),
        description: "Caps outer wall, inner wall, and infill speeds dynamically to min(Hotend_max_flow, Filament_max_flow) / (Layer_Height * Line_Width) so a wider nozzle or faster process never outruns the melt zone.".to_string(),
        category: "Dynamic Combination Rule".to_string(),
        icon: "🚀".to_string(),
        enabled: true,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/volumetric_speed_ceiling.lua").to_string(),
        priority: 15,
      },
      TransformationRule {
        id: "flush_volumes_matrix".to_string(),
        name: "Upgraded Flushing Volume Matrix (Lua)".to_string(),
        description: "Computes the N×N (4×4 Single ACE Pro or 8×8 Double ACE Pro 2) purge matrix using RGB luminance delta, polymer cross-contamination guard (800 mm³ for PLA↔PETG), pigment stickiness, 1/opacity, and melt zone multiplier.".to_string(),
        category: "Dynamic Combination Rule".to_string(),
        icon: "🎨".to_string(),
        enabled: true,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/flush_volumes_matrix.lua").to_string(),
        priority: 60,
      },
      TransformationRule {
        id: "symptom_stringing_diamondback".to_string(),
        name: "Stringing & Oozing Mitigation (MicroSwiss / High-Conductivity Nozzle)".to_string(),
        description: "High thermal conductivity of MicroSwiss / PCD Diamond keeps the melt zone less viscous at the tip. Sets retraction 0.95mm @ 50mm/s, 0.20mm Spiral Z-Hop, 7000 mm/s² travel accel, and -4°C temp offset.".to_string(),
        category: "Default Hardware Rule".to_string(),
        icon: "⚡".to_string(),
        enabled: true,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/symptom_stringing_diamondback.lua").to_string(),
        priority: 25,
      },
      TransformationRule {
        id: "symptom_clog_ace_pro".to_string(),
        name: "Multi-Material Tip Forming & Anti-Clog Rule (AMS / ACE Pro)".to_string(),
        description: "Prevents heat-creep softening & elongated tips jamming the extruder throat during multi-material swaps. Holds transition flush temp at 240°C, caps cut retraction at 12.0mm, and scales purge volume 1.45x.".to_string(),
        category: "Default Hardware Rule".to_string(),
        icon: "🛡️".to_string(),
        enabled: true,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/symptom_clog_ace_pro.lua").to_string(),
        priority: 30,
      },
      TransformationRule {
        id: "symptom_bed_adhesion_petg".to_string(),
        name: "First-Layer Squish & Bed Adhesion Rule".to_string(),
        description: "Increases first-layer squish and thermal bonding on spring steel plates: widens initial layer bead to 0.55mm, slows first layer speed to 25mm/s, and raises initial bed temp +5°C.".to_string(),
        category: "Default Hardware Rule".to_string(),
        icon: "🧲".to_string(),
        enabled: true,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/symptom_bed_adhesion_petg.lua").to_string(),
        priority: 35,
      },
      TransformationRule {
        id: "symptom_spaghetti_pla".to_string(),
        name: "Non-Crossing Gyroid Infill & Z-Hop Anti-Collision Rule".to_string(),
        description: "Eliminates nozzle collisions with curled overhangs or grid infill intersections: switches infill to non-crossing Gyroid, enables 0.40mm Slope Z-Hop, and sets reduce_crossing_wall = 1.".to_string(),
        category: "Default Hardware Rule".to_string(),
        icon: "🍝".to_string(),
        enabled: true,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/symptom_spaghetti_pla.lua").to_string(),
        priority: 40,
      },
      TransformationRule {
        id: "process_scarf_seams_arachne".to_string(),
        name: "Scarf Joint Seams & Arachne Variable Walls".to_string(),
        description: "Switches wall generator to Arachne variable-width perimeters and enables OrcaSlicer Scarf Joint Seams (contour + inner scarf, 15 steps, 10% start height, 10% seam gap, 155° threshold) to eliminate Z-seam bulges.".to_string(),
        category: "Process Quality Rule".to_string(),
        icon: "🪡".to_string(),
        enabled: true,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/process_scarf_seams_arachne.lua").to_string(),
        priority: 45,
      },
      TransformationRule {
        id: "filament_pctg_antistringing".to_string(),
        name: "PCTG Stringing & Ooze Mitigation".to_string(),
        description: "Targets PCTG's high melt elasticity: sets 0.85mm @ 45mm/s retraction (35mm/s deretract), 70% retract-before-wipe over a 2.0mm wipe distance, 0.20mm Spiral Z-Hop, 8000 mm/s² travel acceleration, and -3°C anti-ooze trim.".to_string(),
        category: "Filament Tuning Rule".to_string(),
        icon: "🧵".to_string(),
        enabled: true,
        condition_expr: "filament:PCTG".to_string(),
        lua_code: include_str!("../transformations/filament_pctg_antistringing.lua").to_string(),
        priority: 48,
      },
    ]
  }

  /// Curated rule templates for the interactive "Create / Add New Lua Rule" builder in `WizardTab`.
  pub fn get_rule_templates() -> Vec<TransformationRule> {
    vec![
      TransformationRule {
        id: "scarf_seams_arachne".to_string(),
        name: "Scarf Joint Seams & Arachne Variable Walls".to_string(),
        description: "Switches wall generator to Arachne variable-width perimeters and enables OrcaSlicer Scarf Joint Seams (15 steps, 10% start height, 10% seam gap, 155° threshold).".to_string(),
        category: "Process Quality Rule".to_string(),
        icon: "🪡".to_string(),
        enabled: true,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/process_scarf_seams_arachne.lua").to_string(),
        priority: 45,
      },
      TransformationRule {
        id: "pctg_antistringing".to_string(),
        name: "PCTG Stringing & Ooze Mitigation".to_string(),
        description: "Reduces stringing and fine cobwebbing for PCTG: 0.85mm @ 45mm/s retraction, 70% retract-before-wipe (2.0mm wipe), 0.20mm Spiral Z-Hop, 8000 mm/s² travel accel, and -3°C anti-ooze trim.".to_string(),
        category: "Filament Tuning Rule".to_string(),
        icon: "🧵".to_string(),
        enabled: true,
        condition_expr: "filament:PCTG".to_string(),
        lua_code: include_str!("../transformations/filament_pctg_antistringing.lua").to_string(),
        priority: 48,
      },
      TransformationRule {
        id: "scarf_seams_only".to_string(),
        name: "Scarf Joint Seams (Contour + Inner Ramp)".to_string(),
        description: "Enables OrcaSlicer Scarf Joint Seams (all perimeters, 15 steps, 15mm min length, 10% seam gap) without changing the wall generator.".to_string(),
        category: "Process Quality Rule".to_string(),
        icon: "📐".to_string(),
        enabled: true,
        condition_expr: "all".to_string(),
        lua_code: r#"-- Scarf Joint Seams Only (Contour + Inner Scarf Ramp)
function apply(config, ctx)
  config["seam_slope_type"] = "all"
  config["seam_slope_conditional"] = "1"
  config["seam_slope_start_height"] = "10%"
  config["seam_slope_entire_loop"] = "0"
  config["seam_slope_min_length"] = "15"
  config["seam_slope_steps"] = "15"
  config["seam_gap"] = "10%"
  config["scarf_angle_threshold"] = "155"
  config["wipe_on_loops"] = "1"

  print("[Process:ScarfSeams] seam_slope_type -> all (15 steps, 15mm ramp, 10% seam gap)")
end
"#.to_string(),
        priority: 46,
      },
      TransformationRule {
        id: "arachne_walls_only".to_string(),
        name: "Arachne Variable-Width Perimeter Engine".to_string(),
        description: "Switches wall_generator from classic to arachne with 10° transition angle, 25% min feature size, and 85% min bead width for crisp thin walls and lettering.".to_string(),
        category: "Process Quality Rule".to_string(),
        icon: "🕸️".to_string(),
        enabled: true,
        condition_expr: "all".to_string(),
        lua_code: r#"-- Arachne Variable-Width Perimeter Engine
function apply(config, ctx)
  config["wall_generator"] = "arachne"
  config["wall_transition_angle"] = "10"
  config["wall_transition_length"] = "100%"
  config["min_feature_size"] = "25%"
  config["min_bead_width"] = "85%"

  print("[Process:ArachneWalls] wall_generator -> arachne (10 deg transition, 85% min bead)")
end
"#.to_string(),
        priority: 44,
      },
      TransformationRule {
        id: "custom_blank_rule".to_string(),
        name: "Custom User Lua Rule".to_string(),
        description: "Write a custom sandboxed Lua transformation rule targeting any machine, nozzle, process, or filament condition.".to_string(),
        category: "Custom Lua Rule".to_string(),
        icon: "⚙️".to_string(),
        enabled: true,
        condition_expr: "all".to_string(),
        lua_code: r#"-- Custom User Transformation Rule
function apply(config, ctx)
  -- Access hardware & spool context via `ctx` (e.g. ctx.filament_type, ctx.nozzle_type)
  -- and mutate `config` keys directly:
  config["wipe_on_loops"] = "1"
  print("[CustomRule] Executing on " .. tostring(ctx.printer_model))
end
"#.to_string(),
        priority: 70,
      },
    ]
  }

  /// Automatically enable all applicable default rules when a nozzle or hardware rig is selected.
  pub fn sync_rules_for_rig(
    rules: &mut [TransformationRule],
    nozzle_type: &str,
    nozzle_material: &str,
    ace_pro_units: usize,
  ) {
    let is_microswiss_or_diamond = nozzle_type.contains("diamondback")
      || nozzle_type.contains("microswiss")
      || nozzle_material.contains("Diamondback")
      || nozzle_material.contains("MicroSwiss");
    for rule in rules.iter_mut() {
      match rule.id.as_str() {
        "thermal_conductivity"
        | "rig_addons_cooling"
        | "volumetric_speed_ceiling"
        | "flush_volumes_matrix"
        | "symptom_bed_adhesion_petg"
        | "symptom_spaghetti_pla" => {
          rule.enabled = true;
        }
        "symptom_stringing_diamondback" => {
          rule.enabled = is_microswiss_or_diamond;
        }
        "symptom_clog_ace_pro" => {
          rule.enabled = ace_pro_units > 0;
        }
        _ => {}
      }
    }
  }

  /// Create a one-off Lua transformation rule when the user chooses to accept an imported
  /// disk difference as a first-class transformation in the stack.
  pub fn create_oneoff_rule_from_conflict(conflict: &ExternalConflict) -> TransformationRule {
    let clean_key = conflict
      .param_key
      .chars()
      .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
      .collect::<String>();
    let lua_code = format!(
      "-- One-off transformation accepted from user disk profile ({})\n\
       -- Base: {} | Previous Rule Derived: {} -> Accepted Disk Override: {}\n\
       config[\"{}\"] = \"{}\"\n\
       log(\"Applied accepted disk one-off override: {} = {}\")\n",
      conflict.modifying_rule_id,
      conflict.base_value,
      conflict.expected_derived_value,
      conflict.actual_external_value,
      conflict.param_key,
      conflict.actual_external_value,
      conflict.param_key,
      conflict.actual_external_value
    );
    TransformationRule {
      id: format!("oneoff_{}", clean_key.to_lowercase()),
      name: format!(
        "Disk One-Off: {} → {}",
        conflict.param_label, conflict.actual_external_value
      ),
      description: format!(
        "Accepted from disk profile ({}) where disk value '{}' differed from base '{}' / derived '{}'.",
        conflict.modifying_rule_id,
        conflict.actual_external_value,
        conflict.base_value,
        conflict.expected_derived_value
      ),
      category: "Accepted Disk One-Off".to_string(),
      icon: "📌".to_string(),
      enabled: true,
      condition_expr: "all".to_string(),
      lua_code,
      priority: 85,
    }
  }

  pub fn evaluate_stack(
    base_profile: &OrcaProfile,
    rules: &[TransformationRule],
    context: &serde_json::Value,
  ) -> StackEvaluationResult {
    let mut current_values = serde_json::Value::Object(base_profile.values.clone());
    let mut execution_logs = Vec::new();
    let mut ordered_rules = rules.to_vec();
    ordered_rules.sort_by_key(|r| r.priority);

    let catalog = OrcaProfile::get_parameter_catalog();
    let mut active_rule_count = 0;
    let mut rule_sources: HashMap<String, String> = HashMap::new();
    let mut rule_changes: Vec<RuleBeforeAfterChange> = Vec::new();

    let extract_scalar = |v: &serde_json::Value| -> String {
      match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => arr
          .first()
          .and_then(|x| x.as_str())
          .unwrap_or("")
          .to_string(),
        _ => v.to_string(),
      }
    };

    for rule in &ordered_rules {
      if !rule.enabled {
        continue;
      }
      if !rule.is_applicable(context) {
        execution_logs.push(format!(
          "⏩ Skipping '{}' (condition not met for context)",
          rule.name
        ));
        continue;
      }

      active_rule_count += 1;
      execution_logs.push(format!("⚙️ Evaluating rule: '{}'", rule.name));

      let before_snapshot = current_values.clone();
      match execute_transformation_lua(&rule.lua_code, &current_values, context) {
        Ok((new_val, stdout, modified)) => {
          for line in stdout {
            execution_logs.push(format!("   {}", line));
          }
          for (mod_key, _, _) in &modified {
            rule_sources.insert(mod_key.clone(), rule.name.clone());
          }
          // Compute exact Before -> After diffs for this rule
          if let (
            serde_json::Value::Object(ref before_map),
            serde_json::Value::Object(ref after_map),
          ) = (&before_snapshot, &new_val)
          {
            let mut keys: Vec<&String> = after_map.keys().collect();
            keys.sort();
            for k in keys {
              if k == "flush_volumes_matrix" || k == "computed_safe_speed_ceiling" {
                continue;
              }
              let after_str = after_map.get(k).map(&extract_scalar).unwrap_or_default();
              let before_str = before_map
                .get(k)
                .map(&extract_scalar)
                .or_else(|| base_profile.get_str(k))
                .unwrap_or_else(|| "(unset)".to_string());
              if before_str != after_str {
                rule_sources.insert(k.clone(), rule.name.clone());
                let meta = catalog.get(k);
                let label = meta
                  .map(|m| m.label.clone())
                  .unwrap_or_else(|| k.replace('_', " "));
                let unit = meta.map(|m| m.unit.as_str()).unwrap_or("");
                let unit_suffix = if unit.is_empty() {
                  String::new()
                } else {
                  format!(" {}", unit)
                };
                let before_fmt = if before_str == "(unset)" {
                  before_str.clone()
                } else {
                  format!("{}{}", before_str, unit_suffix)
                };
                let after_fmt = format!("{}{}", after_str, unit_suffix);
                execution_logs.push(format!(
                  "   ↳ [Before → After] {} ({}): {} → {}",
                  label, k, before_fmt, after_fmt
                ));
                rule_changes.push(RuleBeforeAfterChange {
                  rule_id: rule.id.clone(),
                  rule_name: rule.name.clone(),
                  param_key: k.clone(),
                  param_label: label,
                  before_value: before_fmt,
                  after_value: after_fmt,
                });
              }
            }
          }
          current_values = new_val;
        }
        Err(err) => {
          execution_logs.push(format!("❌ Error in rule '{}': {}", rule.name, err));
        }
      }
    }

    let mut diff_items = Vec::new();
    let mut flush_matrix = Vec::new();
    let mut safe_speed_ceiling = 250.0;

    if let serde_json::Value::Object(ref final_map) = current_values {
      if let Some(arr) = final_map
        .get("flush_volumes_matrix")
        .and_then(|v| v.as_array())
      {
        for item in arr {
          if let Some(s) = item.as_str() {
            flush_matrix.push(s.to_string());
          } else {
            flush_matrix.push(item.to_string());
          }
        }
      }
      if let Some(s) = final_map
        .get("computed_safe_speed_ceiling")
        .and_then(|v| v.as_str())
      {
        safe_speed_ceiling = s.parse::<f64>().unwrap_or(250.0);
      }

      for (k, v) in final_map {
        if k == "flush_volumes_matrix" {
          continue;
        }
        let base_val_str = base_profile.get_str(k).unwrap_or_default();
        let curr_val_str = match v {
          serde_json::Value::String(s) => s.clone(),
          serde_json::Value::Array(arr) => arr
            .first()
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
          _ => v.to_string(),
        };

        let meta = catalog.get(k);
        let is_modified = base_val_str != curr_val_str;

        if is_modified || meta.is_some() {
          let label = meta
            .map(|m| m.label.clone())
            .unwrap_or_else(|| k.replace('_', " "));
          let category = meta
            .map(|m| m.category.clone())
            .unwrap_or_else(|| "Other".to_string());
          let unit = meta.map(|m| m.unit.clone()).unwrap_or_default();
          let source = rule_sources.get(k).cloned().unwrap_or_else(|| {
            if is_modified {
              "Active Transformations".to_string()
            } else {
              "Stock Default".to_string()
            }
          });

          diff_items.push(DiffItem {
            key: k.clone(),
            label,
            category,
            unit,
            base_value: base_val_str,
            override_value: curr_val_str,
            source_rule: source,
            is_modified,
          });
        }
      }
    }

    diff_items.sort_by(|a, b| {
      b.is_modified
        .cmp(&a.is_modified)
        .then(a.category.cmp(&b.category))
        .then(a.label.cmp(&b.label))
    });

    let mut res_profile = base_profile.clone();
    if let serde_json::Value::Object(map) = current_values {
      res_profile.values = map;
    }

    StackEvaluationResult {
      resulting_profile: res_profile,
      diff_items,
      rule_changes,
      flush_matrix,
      safe_speed_ceiling,
      execution_logs,
      active_rule_count,
      total_rule_count: rules.len(),
    }
  }

  pub fn detect_conflicts(
    base_profile: &OrcaProfile,
    derived_profile: &OrcaProfile,
    external_profile: &OrcaProfile,
    rules: &[TransformationRule],
  ) -> Vec<ExternalConflict> {
    let mut conflicts = Vec::new();
    let catalog = OrcaProfile::get_parameter_catalog();

    for (k, ext_val) in &external_profile.values {
      if k == "flush_volumes_matrix" {
        continue;
      }
      let ext_str = match ext_val {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => arr
          .first()
          .and_then(|x| x.as_str())
          .unwrap_or("")
          .to_string(),
        _ => ext_val.to_string(),
      };
      let base_str = base_profile.get_str(k).unwrap_or_default();
      let derived_str = derived_profile.get_str(k).unwrap_or_default();

      if ext_str != derived_str && ext_str != base_str {
        let rule_name = rules
          .iter()
          .find(|r| r.enabled && r.lua_code.contains(k))
          .map(|r| r.name.clone())
          .unwrap_or_else(|| "Active Transformation Stack".to_string());

        let label = catalog
          .get(k)
          .map(|m| m.label.clone())
          .unwrap_or_else(|| k.clone());

        conflicts.push(ExternalConflict {
          param_key: k.clone(),
          param_label: label,
          base_value: base_str,
          expected_derived_value: derived_str.clone(),
          actual_external_value: ext_str.clone(),
          modifying_rule_id: rule_name,
          source_relative_path: String::new(),
          json_field_key: k.clone(),
          clean_raw_value: derived_str,
          disk_raw_value: ext_str,
        });
      }
    }

    conflicts
  }

  pub fn export_package(rules: &[TransformationRule], active_rig_id: &str) -> String {
    let pkg = FullExportPackage {
      version: "2.1".to_string(),
      export_timestamp_millis: 1729960000,
      rules: rules.to_vec(),
      active_rig_id: active_rig_id.to_string(),
      notes: "OrcaSlicer WebConfig Lua Transformation Stack".to_string(),
    };
    serde_json::to_string_pretty(&pkg).unwrap_or_else(|_| "{}".to_string())
  }

  pub fn import_package(json_str: &str) -> Result<FullExportPackage, String> {
    serde_json::from_str::<FullExportPackage>(json_str)
      .map_err(|e| format!("Failed to parse imported transformations JSON: {}", e))
  }

  /// Evaluates all imported disk conflicts, filament profiles, flushing matrices, and live Lua-derived
  /// output parameters for unsafe (`Danger`) or weird/anomalous (`Warning`) values.
  pub fn evaluate_safety_warnings(
    rig: &MachineRig,
    filaments: &[FilamentSpool],
    derived_profile: &OrcaProfile,
    external_conflicts: &[ExternalConflict],
    disk_flush_matrix: &[String],
  ) -> Vec<SafetyWarning> {
    let mut warnings = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    let mut push_warn = |w: SafetyWarning| {
      if seen_ids.insert(w.id.clone()) {
        warnings.push(w);
      }
    };

    // 1. Check unresolved external disk conflicts
    for conflict in external_conflicts {
      if conflict.json_field_key == "retraction_speed" {
        let val_num = conflict.disk_raw_value.parse::<f32>().unwrap_or(0.0);
        let severity = if !(15.0..=55.0).contains(&val_num) {
          SafetySeverity::Danger
        } else {
          SafetySeverity::Warning
        };
        push_warn(SafetyWarning {
          id: format!("conflict:{}", conflict.param_key),
          severity,
          scope_label: "Machine Profile (Disk Conflict)".to_string(),
          param_key: conflict.param_label.clone(),
          current_value: conflict.actual_external_value.clone(),
          safe_range: "25–45 mm/s (Direct Drive)".to_string(),
          explanation: format!(
            "Disk file '{}' sets retraction_speed to {} (expected {}). High direct-drive retraction speeds (>55 mm/s) grind filament and skip extruder steps.",
            conflict.source_relative_path,
            conflict.actual_external_value,
            conflict.expected_derived_value
          ),
        });
      } else if conflict.json_field_key == "inherits" {
        push_warn(SafetyWarning {
          id: format!("conflict:{}", conflict.param_key),
          severity: SafetySeverity::Danger,
          scope_label: "Filament Inheritance (Disk Conflict)".to_string(),
          param_key: conflict.param_label.clone(),
          current_value: conflict.actual_external_value.clone(),
          safe_range: conflict.expected_derived_value.clone(),
          explanation: format!(
            "Cross-polymer inheritance mismatch in '{}': inherits '{}' instead of '{}'. Inheriting the wrong polymer pulls unsafe nozzle/bed temperatures and cooling schedules.",
            conflict.source_relative_path,
            conflict.actual_external_value,
            conflict.expected_derived_value
          ),
        });
      } else if conflict.json_field_key == "fan_max_speed_HS" {
        push_warn(SafetyWarning {
          id: format!("conflict:{}", conflict.param_key),
          severity: SafetySeverity::Danger,
          scope_label: "Filament Cooling (Disk Conflict)".to_string(),
          param_key: conflict.param_label.clone(),
          current_value: conflict.actual_external_value.clone(),
          safe_range: "0–30% (Warp-Prone ABS/PA/PC)".to_string(),
          explanation: format!(
            "File '{}' sets Hardened Steel max cooling fan to {} while Brass is {}. High fan speed on ABS/PA Nylon causes severe warping off the bed and interlayer delamination.",
            conflict.source_relative_path,
            conflict.actual_external_value,
            conflict.base_value
          ),
        });
      } else {
        push_warn(SafetyWarning {
          id: format!("conflict:{}", conflict.param_key),
          severity: SafetySeverity::Warning,
          scope_label: "Unresolved Disk Drift".to_string(),
          param_key: conflict.param_label.clone(),
          current_value: conflict.actual_external_value.clone(),
          safe_range: conflict.expected_derived_value.clone(),
          explanation: format!(
            "Disk file '{}' has '{}' = {} which diverges from rule '{}' ({}).",
            conflict.source_relative_path,
            conflict.json_field_key,
            conflict.actual_external_value,
            conflict.modifying_rule_id,
            conflict.expected_derived_value
          ),
        });
      }
    }

    // 2. Check all filament spools for hardware compatibility, raw JSON anomalies, and polymer bounds
    let (_, rig_hrc) = rig.orca_machine_nozzle_type_and_hrc();
    for spool in filaments {
      if let Some(compat_msg) = spool.compatibility_warning(&rig.nozzle_type, rig_hrc) {
        let is_hrc_danger = spool.required_nozzle_hrc > rig_hrc;
        push_warn(SafetyWarning {
          id: format!("spool_compat:{}", spool.id),
          severity: if is_hrc_danger {
            SafetySeverity::Danger
          } else {
            SafetySeverity::Warning
          },
          scope_label: format!("Filament: {}", spool.name),
          param_key: "Nozzle Compatibility / Hardness (HRC)".to_string(),
          current_value: format!(
            "{} (Req ≥{} HRC) on {} ({} HRC)",
            spool.material_type, spool.required_nozzle_hrc, rig.nozzle_material, rig_hrc
          ),
          safe_range: format!("Nozzle ≥{} HRC", spool.required_nozzle_hrc),
          explanation: compat_msg,
        });
      }

      // Check TPU loaded in ACE Pro multi-material hub
      if spool.is_loaded
        && rig.ace_pro_units > 0
        && spool.material_type.to_uppercase().contains("TPU")
      {
        push_warn(SafetyWarning {
          id: format!("spool_tpu_ace:{}", spool.id),
          severity: SafetySeverity::Warning,
          scope_label: format!("ACE Pro Slot ({})", spool.color_name),
          param_key: "Flexible TPU in Motorized AMS/ACE Hub".to_string(),
          current_value: format!("{} ({})", spool.name, spool.material_type),
          safe_range: "Direct Spool Holder Bypass Recommended".to_string(),
          explanation: "Flexible 95A TPU loaded in an active ACE Pro Bowden slot frequently buckles inside the motorized filament buffer and long PTFE path. Feed TPU via direct bypass unless using rigid high-Shore TPU.".to_string(),
        });
      }

      // Check non-reconstructed deltas on the filament (e.g. inverted initial layer temp, Brass < min range, PA = 0.08)
      for delta in &spool.deltas_from_base {
        if !delta.reconstructed_by_lua {
          let is_danger = delta.param_key == "inherits"
            || delta.param_key == "fan_max_speed_HS"
            || delta.param_key == "nozzle_temperature_BRASS";
          push_warn(SafetyWarning {
            id: format!("spool_delta:{}:{}", spool.id, delta.param_key),
            severity: if is_danger {
              SafetySeverity::Danger
            } else {
              SafetySeverity::Warning
            },
            scope_label: format!("Filament: {}", spool.name),
            param_key: delta.param_key.clone(),
            current_value: delta.disk_value.clone(),
            safe_range: delta.base_value.clone(),
            explanation: delta.rule_explanation.clone(),
          });
        }
      }

      // Check raw disk JSON for Carbon Fiber HRC mismatch, Brass < range_low, and inverted initial-layer temperatures
      if let Some(raw_str) = &spool.raw_disk_json {
        if (spool.material_type.contains("-CF")
          || spool.material_type.contains("-GF")
          || spool.finish_type.contains("Carbon"))
          && raw_str.contains("\"required_nozzle_HRC\"")
          && raw_str.contains("\"3\"")
        {
          push_warn(SafetyWarning {
            id: format!("spool_raw_hrc:{}", spool.id),
            severity: SafetySeverity::Warning,
            scope_label: format!("Filament Disk JSON: {}", spool.name),
            param_key: "required_nozzle_HRC (On Disk)".to_string(),
            current_value: "3 HRC on disk (Auto-healed to 55 HRC on Save)".to_string(),
            safe_range: "≥55 HRC (Hardened Steel / Ruby / DiamondBack)".to_string(),
            explanation: format!(
              "Raw disk file for abrasive {} had `required_nozzle_HRC = 3` (soft Brass allowed) and lacked `nozzle_temperature_HS`. WebConfig upgrades this to 55 HRC and {}°C HS upon save.",
              spool.name, spool.print_temp
            ),
          });
        }

        if let Ok(raw_val) = serde_json::from_str::<serde_json::Value>(raw_str) {
          let extract_i32 = |k: &str| -> Option<i32> {
            raw_val
              .get(k)
              .and_then(|v| {
                v.as_str().map(|s| s.to_string()).or_else(|| {
                  v.as_array()
                    .and_then(|a| a.first())
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string())
                })
              })
              .and_then(|s| s.parse::<i32>().ok())
          };

          if let (Some(brass_t), Some(range_low)) = (
            extract_i32("nozzle_temperature_BRASS").or_else(|| extract_i32("nozzle_temperature")),
            extract_i32("nozzle_temperature_range_low"),
          ) {
            if brass_t < range_low {
              push_warn(SafetyWarning {
                id: format!("spool_raw_brass_low:{}", spool.id),
                severity: SafetySeverity::Danger,
                scope_label: format!("Filament Disk JSON: {}", spool.name),
                param_key: "nozzle_temperature_BRASS vs range_low".to_string(),
                current_value: format!("{}°C Brass (< {}°C Min Range)", brass_t, range_low),
                safe_range: format!("≥{}°C (nozzle_temperature_range_low)", range_low),
                explanation: format!(
                  "{} defines Brass nozzle temperature at {}°C, which is {}°C below its own `nozzle_temperature_range_low` ({}°C) and {}°C below its HS temperature ({}°C). Slicing on a Brass nozzle risks cold extrusion.",
                  spool.name,
                  brass_t,
                  range_low - brass_t,
                  range_low,
                  spool.print_temp - brass_t,
                  spool.print_temp
                ),
              });
            }
          }

          if let (Some(init_hs), Some(cont_hs)) = (
            extract_i32("nozzle_temperature_initial_layer_HS"),
            extract_i32("nozzle_temperature_HS"),
          ) {
            if init_hs < cont_hs {
              push_warn(SafetyWarning {
                id: format!("spool_raw_inv_init_nozzle:{}", spool.id),
                severity: SafetySeverity::Warning,
                scope_label: format!("Filament Disk JSON: {}", spool.name),
                param_key: "nozzle_temperature_initial_layer_HS".to_string(),
                current_value: format!("{}°C initial < {}°C continuous", init_hs, cont_hs),
                safe_range: format!("≥{}°C (≥ Continuous Print Temp)", cont_hs),
                explanation: format!(
                  "{} sets initial layer nozzle temp ({}°C) cooler than continuous printing temp ({}°C). First layers normally require equal or higher temperature for bed adhesion.",
                  spool.name, init_hs, cont_hs
                ),
              });
            }
          }

          if let (Some(init_bed), Some(cont_bed)) = (
            extract_i32("textured_plate_temp_initial_layer"),
            extract_i32("textured_plate_temp"),
          ) {
            if init_bed < cont_bed {
              push_warn(SafetyWarning {
                id: format!("spool_raw_inv_init_bed:{}", spool.id),
                severity: SafetySeverity::Warning,
                scope_label: format!("Filament Disk JSON: {}", spool.name),
                param_key: "textured_plate_temp_initial_layer".to_string(),
                current_value: format!("{}°C initial < {}°C continuous", init_bed, cont_bed),
                safe_range: format!("≥{}°C (≥ Continuous Bed Temp)", cont_bed),
                explanation: format!(
                  "{} sets initial layer bed temp ({}°C) cooler than continuous bed temp ({}°C), which can trigger thermal stress and corner lifting.",
                  spool.name, init_bed, cont_bed
                ),
              });
            }
          }
        }
      }

      if spool.pressure_advance > 0.065 {
        push_warn(SafetyWarning {
          id: format!("spool_pa_high:{}", spool.id),
          severity: SafetySeverity::Warning,
          scope_label: format!("Filament: {}", spool.name),
          param_key: "pressure_advance".to_string(),
          current_value: format!("{:.3}", spool.pressure_advance),
          safe_range: "0.025–0.055 (Direct Drive)".to_string(),
          explanation: format!(
            "{} sets direct-drive Pressure Advance to {:.3} (nearly 2× the typical 0.035–0.050 range), which can cause corner thinning and extruder stepper clicking.",
            spool.name, spool.pressure_advance
          ),
        });
      }

      // Check polymer temperature bounds on the spool
      let mat = spool.material_type.as_str();
      let (min_nozzle, max_nozzle, warn_nozzle, max_bed) = match mat {
        "PLA" | "PLA+" | "PLA-CF" => (190, 240, 228, 70),
        "PETG" | "PETG-CF" => (220, 265, 255, 90),
        "PCTG" => (240, 280, 272, 95),
        "TPU" => (200, 245, 235, 65),
        "ABS" | "ASA" => (235, 285, 275, 115),
        "PA" | "PA12" | "PA6-CF" | "PC" => (255, 310, 295, 120),
        _ => (185, 300, 285, 115),
      };

      if spool.print_temp < min_nozzle || spool.print_temp > max_nozzle {
        push_warn(SafetyWarning {
          id: format!("spool_temp_danger:{}", spool.id),
          severity: SafetySeverity::Danger,
          scope_label: format!("Filament: {}", spool.name),
          param_key: "print_temp".to_string(),
          current_value: format!("{}°C", spool.print_temp),
          safe_range: format!("{}–{}°C ({})", min_nozzle, max_nozzle, mat),
          explanation: format!(
            "{} nozzle temperature ({}°C) is outside safe polymer limits ({}–{}°C); risks cold-extrusion clogs or thermal degradation.",
            spool.name, spool.print_temp, min_nozzle, max_nozzle
          ),
        });
      } else if spool.print_temp >= warn_nozzle {
        push_warn(SafetyWarning {
          id: format!("spool_temp_warn:{}", spool.id),
          severity: SafetySeverity::Warning,
          scope_label: format!("Filament: {}", spool.name),
          param_key: "print_temp".to_string(),
          current_value: format!("{}°C", spool.print_temp),
          safe_range: format!("{}–{}°C ({})", min_nozzle, warn_nozzle - 1, mat),
          explanation: format!(
            "{} nozzle temperature ({}°C) is near the upper limit for {}; increases stringing, oozing, and heat-creep risk in an enclosed chamber.",
            spool.name, spool.print_temp, mat
          ),
        });
      }

      if spool.bed_temp > max_bed {
        push_warn(SafetyWarning {
          id: format!("spool_bed_warn:{}", spool.id),
          severity: SafetySeverity::Warning,
          scope_label: format!("Filament: {}", spool.name),
          param_key: "bed_temp".to_string(),
          current_value: format!("{}°C", spool.bed_temp),
          safe_range: format!("≤{}°C ({})", max_bed, mat),
          explanation: format!(
            "{} bed temperature ({}°C) exceeds typical {} softening range (≤{}°C); can cause elephant's foot or deform first layers.",
            spool.name, spool.bed_temp, mat, max_bed
          ),
        });
      }
    }

    // 3. Check raw disk `flush_volumes_matrix` for non-zero diagonal self-purge entries
    if !disk_flush_matrix.is_empty() {
      let n = (disk_flush_matrix.len() as f64).sqrt().round() as usize;
      if n > 0 && n * n == disk_flush_matrix.len() {
        let mut nonzero_diags = Vec::new();
        for i in 0..n {
          let diag_val = disk_flush_matrix[i * n + i]
            .parse::<f64>()
            .unwrap_or(0.0)
            .round() as i32;
          if diag_val != 0 {
            nonzero_diags.push(format!("[{},{}]={}", i + 1, i + 1, diag_val));
          }
        }
        if !nonzero_diags.is_empty() {
          push_warn(SafetyWarning {
            id: "flush_matrix_nonzero_diagonal".to_string(),
            severity: SafetySeverity::Warning,
            scope_label: "AnycubicSlicerNext.conf (Flushing Matrix)".to_string(),
            param_key: "flush_volumes_matrix (Diagonal Cells)".to_string(),
            current_value: nonzero_diags.join(", "),
            safe_range: "0 mm³ on all diagonal [i,i] self-transitions".to_string(),
            explanation: "The imported `flush_volumes_matrix` on disk has non-zero diagonal self-purge values (likely shifted when a slot was inserted/removed). Saving to disk will overwrite it with a clean physics-derived matrix (`0 mm³` diagonals).".to_string(),
          });
        }
      }
    }

    // 4. Check live Lua-derived output profile parameters for out-of-range values
    let get_num = |key: &str| -> Option<f64> {
      derived_profile
        .get_str(key)
        .and_then(|s| s.parse::<f64>().ok())
    };

    if let Some(retract_spd) = get_num("filament_retraction_speed") {
      if !(15.0..=55.0).contains(&retract_spd) {
        push_warn(SafetyWarning {
          id: "derived:filament_retraction_speed".to_string(),
          severity: SafetySeverity::Danger,
          scope_label: "Derived Output Profile".to_string(),
          param_key: "filament_retraction_speed".to_string(),
          current_value: format!("{:.1} mm/s", retract_spd),
          safe_range: "25–50 mm/s (Direct Drive)".to_string(),
          explanation: "Derived retraction speed is outside the safe 15–55 mm/s direct-drive envelope; speeds above 55 mm/s can strip filament or stall the extruder stepper.".to_string(),
        });
      }
    }

    if let Some(retract_len) = get_num("filament_retraction_length") {
      if retract_len > 2.5 {
        push_warn(SafetyWarning {
          id: "derived:filament_retraction_length".to_string(),
          severity: SafetySeverity::Danger,
          scope_label: "Derived Output Profile".to_string(),
          param_key: "filament_retraction_length".to_string(),
          current_value: format!("{:.2} mm", retract_len),
          safe_range: "0.40–1.50 mm (Direct Drive)".to_string(),
          explanation: "Retraction length >2.5 mm on an all-metal direct-drive hotend pulls molten polymer up into the cold heat-break, causing severe heat-creep jams.".to_string(),
        });
      }
    }

    let nozzle_diam_mm = rig.nozzle_diameter.parse::<f64>().unwrap_or(0.4);
    if let Some(lh) = get_num("layer_height") {
      let max_lh = nozzle_diam_mm * 0.78;
      let min_lh = nozzle_diam_mm * 0.12;
      if lh > max_lh || lh < min_lh {
        push_warn(SafetyWarning {
          id: "derived:layer_height".to_string(),
          severity: SafetySeverity::Danger,
          scope_label: "Derived Output Profile".to_string(),
          param_key: "layer_height".to_string(),
          current_value: format!("{:.2} mm", lh),
          safe_range: format!("{:.2}–{:.2} mm ({:.1}mm Nozzle)", min_lh, max_lh, nozzle_diam_mm),
          explanation: format!(
            "Layer height {:.2} mm exceeds safe extrusion geometry for a {:.1} mm nozzle (12%–75% of nozzle diameter).",
            lh, nozzle_diam_mm
          ),
        });
      }
    }

    if let Some(flow) = get_num("filament_flow_ratio") {
      if !(0.85..=1.12).contains(&flow) {
        push_warn(SafetyWarning {
          id: "derived:filament_flow_ratio".to_string(),
          severity: if !(0.75..=1.20).contains(&flow) {
            SafetySeverity::Danger
          } else {
            SafetySeverity::Warning
          },
          scope_label: "Derived Output Profile".to_string(),
          param_key: "filament_flow_ratio".to_string(),
          current_value: format!("{:.3}", flow),
          safe_range: "0.900–1.050".to_string(),
          explanation: "Derived flow ratio is outside the normal 0.85–1.12 range; extreme values cause severe under-extrusion gaps or over-extrusion nozzle collisions.".to_string(),
        });
      }
    }

    warnings
  }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SafetySeverity {
  /// Unsafe value that risks hardware damage, clogs, skipped steps, or print failure.
  Danger,
  /// Weird, drifted, or suboptimal value that merits user review before saving/exporting.
  Warning,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SafetyWarning {
  pub id: String,
  pub severity: SafetySeverity,
  pub scope_label: String,
  pub param_key: String,
  pub current_value: String,
  pub safe_range: String,
  pub explanation: String,
}
