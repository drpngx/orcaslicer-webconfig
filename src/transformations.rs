use crate::lua_engine::execute_transformation_lua;
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
}
