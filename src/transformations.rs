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
  #[serde(default)]
  pub step_number: usize,
  #[serde(default)]
  pub rule_priority: i32,
  pub param_key: String,
  pub param_label: String,
  pub before_value: String,
  pub after_value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StackReorderAction {
  MoveUp,
  MoveDown,
  MoveToTop,
  MoveToBottom,
  SetPriority(i32),
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

  /// Reassigns clean sequential priorities (`10, 20, 30, ...`) matching the current slice order.
  pub fn reindex_priorities(rules: &mut [TransformationRule]) {
    for (idx, r) in rules.iter_mut().enumerate() {
      r.priority = ((idx as i32) + 1) * 10;
    }
  }

  /// Sorts the transformation stack by `(priority, original_index)` and normalizes priorities to
  /// clean increments (`10, 20, 30, ...`) so UI array order and evaluation order are always 100% identical.
  pub fn normalize_stack_order(rules: &mut Vec<TransformationRule>) {
    let mut indexed: Vec<(usize, TransformationRule)> = rules.drain(..).enumerate().collect();
    indexed.sort_by_key(|(idx, r)| (r.priority, *idx));
    *rules = indexed.into_iter().map(|(_, r)| r).collect();
    Self::reindex_priorities(rules);
  }

  /// Reorders a rule in the stack (`MoveUp`, `MoveDown`, `MoveToTop`, `MoveToBottom`, or `SetPriority`)
  /// and normalizes priorities (`10, 20, 30, ...`).
  pub fn reorder_rule(
    rules: &mut Vec<TransformationRule>,
    rule_id: &str,
    action: StackReorderAction,
  ) -> String {
    Self::normalize_stack_order(rules);
    match action {
      StackReorderAction::MoveUp => {
        if let Some(pos) = rules.iter().position(|r| r.id == rule_id) {
          if pos > 0 {
            rules.swap(pos, pos - 1);
            Self::reindex_priorities(rules);
            let r = &rules[pos - 1];
            return format!(
              "Moved '{}' earlier to Step #{} (Priority #{})!",
              r.name, pos, r.priority
            );
          }
        }
      }
      StackReorderAction::MoveDown => {
        if let Some(pos) = rules.iter().position(|r| r.id == rule_id) {
          if pos + 1 < rules.len() {
            rules.swap(pos, pos + 1);
            Self::reindex_priorities(rules);
            let r = &rules[pos + 1];
            return format!(
              "Moved '{}' later to Step #{} (Priority #{})!",
              r.name,
              pos + 2,
              r.priority
            );
          }
        }
      }
      StackReorderAction::MoveToTop => {
        if let Some(pos) = rules.iter().position(|r| r.id == rule_id) {
          if pos > 0 {
            let r = rules.remove(pos);
            let r_name = r.name.clone();
            rules.insert(0, r);
            Self::reindex_priorities(rules);
            return format!(
              "Moved '{}' to Step #1 (First in Transformation Pipeline)!",
              r_name
            );
          }
        }
      }
      StackReorderAction::MoveToBottom => {
        if let Some(pos) = rules.iter().position(|r| r.id == rule_id) {
          if pos + 1 < rules.len() {
            let r = rules.remove(pos);
            let r_name = r.name.clone();
            rules.push(r);
            Self::reindex_priorities(rules);
            return format!(
              "Moved '{}' to Step #{} (Last in Transformation Pipeline)!",
              r_name,
              rules.len()
            );
          }
        }
      }
      StackReorderAction::SetPriority(new_prio) => {
        if let Some(r) = rules.iter_mut().find(|r| r.id == rule_id) {
          r.priority = new_prio;
        }
        Self::normalize_stack_order(rules);
        if let Some((pos, r)) = rules.iter().enumerate().find(|(_, r)| r.id == rule_id) {
          return format!(
            "Updated '{}' to Step #{} (Priority #{})!",
            r.name,
            pos + 1,
            r.priority
          );
        }
      }
    }
    "Transformation stack order unchanged.".to_string()
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

    let append_rule_source =
      |sources: &mut HashMap<String, String>, key: &str, step_idx: usize, rule_name: &str| {
        let step_tag = format!("Step #{}: {}", step_idx, rule_name);
        if let Some(existing) = sources.get_mut(key) {
          if !existing.contains(&step_tag) {
            *existing = format!("{} ➔ {}", existing, step_tag);
          }
        } else {
          sources.insert(key.to_string(), step_tag);
        }
      };

    for (stack_idx, rule) in ordered_rules.iter().enumerate() {
      let step_num = stack_idx + 1;
      if !rule.enabled {
        continue;
      }
      if !rule.is_applicable(context) {
        execution_logs.push(format!(
          "⏩ [Step #{} • Priority #{}] Skipping '{}' (condition not met for context)",
          step_num, rule.priority, rule.name
        ));
        continue;
      }

      active_rule_count += 1;
      execution_logs.push(format!(
        "⚙️ [Step #{} • Priority #{}] Evaluating rule: '{}'",
        step_num, rule.priority, rule.name
      ));

      let before_snapshot = current_values.clone();
      match execute_transformation_lua(&rule.lua_code, &current_values, context) {
        Ok((new_val, stdout, modified)) => {
          for line in stdout {
            execution_logs.push(format!("   {}", line));
          }
          for (mod_key, _, _) in &modified {
            append_rule_source(&mut rule_sources, mod_key, step_num, &rule.name);
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
                append_rule_source(&mut rule_sources, k, step_num, &rule.name);
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
                  step_number: step_num,
                  rule_priority: rule.priority,
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

  /// Prepackaged, parameterized post-print calibration & tuning wizards that ask interactive
  /// questions (e.g. YOLO Flow Rate chip value, PA corner value, Temperature Tower floor,
  /// Retraction Tower height, MVS failure height, Dimensional Cube caliper readings) and
  /// generate the exact Lua rule to apply the result.
  pub fn get_parameterized_calibration_templates() -> Vec<ParameterizedCalibrationTemplate> {
    vec![
      ParameterizedCalibrationTemplate {
        id: "cal_yolo_flow".to_string(),
        name: "YOLO Flow Rate Calibration (Single-Pass Chip Test)".to_string(),
        icon: "🎯".to_string(),
        category: "Calibration Print #2 (Flow Rate)".to_string(),
        slicer_menu_path: "Calibration → Flow Rate → YOLO (Recommended) or Pass 1 / Pass 2".to_string(),
        when_to_run: "Run 2nd (right after Temperature Tower) whenever dialing in a new filament spool or nozzle.".to_string(),
        how_to_read_print: vec![
          "1. In AnycubicSlicerNext / OrcaSlicer top menu, click Calibration → Flow Rate → YOLO and print the numbered modifier chips (-0.05 to +0.05).".to_string(),
          "2. Rub your fingernail across the CENTER of each top surface (ignore the outer perimeter seam where the nozzle turns around).".to_string(),
          "3. Pick the chip whose center feels smoothest — no tiny gaps between extrusion lines (under-extrusion) and no rough ridges plowed up between lines (over-extrusion).".to_string(),
          "4. Answer the questions below: in YOLO mode, the chip number is added directly to your starting flow (Flow_new = Flow_old + Chip_Modifier).".to_string(),
        ],
        formula_summary: "YOLO Mode: Flow_new = Flow_old + Chip_Value  •  Classic Pass 1/2: Flow_new = Flow_old × (100 + Pass_Pct) / 100".to_string(),
        default_condition_expr: "filament:PCTG".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "mode".to_string(),
            question: "1. Which Flow Rate calibration test did you print?".to_string(),
            hint: "YOLO mode prints chips labeled -0.05 to +0.05 (direct addition). Classic Pass 1/2 prints chips labeled -10 to +10 (%).".to_string(),
            default_value: "YOLO".to_string(),
            unit: String::new(),
            quick_options: vec![
              ("YOLO".to_string(), "YOLO Mode (-0.05 to +0.05 — Direct Add)".to_string()),
              ("Classic".to_string(), "Classic Pass 1/2 (-20% to +20% — Percentage)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "old_flow".to_string(),
            question: "2. What was your starting Flow Ratio (filament_flow_ratio) when you sliced the test?".to_string(),
            hint: "Check the Filament tab for your spool's current flow ratio (e.g. 0.96 for Anycubic PETG/PLA+, 0.95 for PCTG, 0.98 for PLA).".to_string(),
            default_value: "0.96".to_string(),
            unit: "ratio".to_string(),
            quick_options: vec![
              ("0.98".to_string(), "0.98 (Standard PLA)".to_string()),
              ("0.96".to_string(), "0.96 (Anycubic PLA+ / PETG)".to_string()),
              ("0.95".to_string(), "0.95 (PCTG / Silk)".to_string()),
              ("0.93".to_string(), "0.93 (ABS / ASA / PA)".to_string()),
              ("1.00".to_string(), "1.00 (Unscaled Base)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "chip_val".to_string(),
            question: "3. What number is printed on the smoothest chip?".to_string(),
            hint: "If two adjacent chips (e.g. -0.01 and -0.02) look equally smooth, you can type the midpoint (-0.015) in the input box.".to_string(),
            default_value: "-0.02".to_string(),
            unit: "modifier".to_string(),
            quick_options: vec![
              ("-0.04".to_string(), "-0.04".to_string()),
              ("-0.03".to_string(), "-0.03".to_string()),
              ("-0.02".to_string(), "-0.02".to_string()),
              ("-0.015".to_string(), "-0.015".to_string()),
              ("-0.01".to_string(), "-0.01".to_string()),
              ("0.00".to_string(), "0.00 (Already Spot-On)".to_string()),
              ("+0.01".to_string(), "+0.01".to_string()),
              ("+0.015".to_string(), "+0.015".to_string()),
              ("+0.02".to_string(), "+0.02".to_string()),
              ("+0.03".to_string(), "+0.03".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "cal_pressure_advance".to_string(),
        name: "Pressure Advance (PA) Pattern / Line Calibration".to_string(),
        icon: "📐".to_string(),
        category: "Calibration Print #3 (Pressure Advance)".to_string(),
        slicer_menu_path: "Calibration → Pressure Advance → PA Pattern (Recommended) / PA Line / PA Tower".to_string(),
        when_to_run: "Run 3rd (after Flow Rate is dialed in) to eliminate bulging or gaps at sharp 90° corners and seam starts.".to_string(),
        how_to_read_print: vec![
          "1. Click Calibration → Pressure Advance → PA Pattern (Direct Drive: 0.000 to 0.080, step 0.002) and print the chevron pattern.".to_string(),
          "2. Inspect the sharp 90° corner apex of each numbered chevron: too LOW PA leaves a swollen, rounded bulge; too HIGH PA thins the corner or leaves a pinhole gap.".to_string(),
          "3. Select the PA number next to the sharpest, most uniform corner below.".to_string(),
        ],
        formula_summary: "Sets enable_pressure_advance = 1 and pressure_advance = Measured_Best_Corner_PA".to_string(),
        default_condition_expr: "filament:PCTG".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "pa_val".to_string(),
            question: "1. Which PA value on the test print produced the cleanest, sharpest corner?".to_string(),
            hint: "For Kobra S1 Direct Drive: PLA is typically 0.028–0.038, PETG is 0.040–0.050, and PCTG is 0.044–0.052.".to_string(),
            default_value: "0.048".to_string(),
            unit: "s".to_string(),
            quick_options: vec![
              ("0.028".to_string(), "0.028 (Fast PLA)".to_string()),
              ("0.034".to_string(), "0.034 (Standard PLA+)".to_string()),
              ("0.040".to_string(), "0.040 (PLA-CF / PETG)".to_string()),
              ("0.045".to_string(), "0.045 (PETG)".to_string()),
              ("0.048".to_string(), "0.048 (PCTG Sweet Spot)".to_string()),
              ("0.052".to_string(), "0.052 (PCTG / TPU)".to_string()),
              ("0.060".to_string(), "0.060 (High Elasticity)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "adaptive_pa".to_string(),
            question: "2. Enable Adaptive Pressure Advance on Overhangs & Bridges?".to_string(),
            hint: "Adaptive PA lowers PA during slow overhangs/bridges so steep edges don't curl.".to_string(),
            default_value: "0".to_string(),
            unit: String::new(),
            quick_options: vec![
              ("0".to_string(), "Off — Fixed PA everywhere (Recommended)".to_string()),
              ("1".to_string(), "On — Enable Adaptive PA on overhangs & bridges".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "cal_temp_tower".to_string(),
        name: "Temperature Tower Calibration".to_string(),
        icon: "🌡️".to_string(),
        category: "Calibration Print #1 (Temperature)".to_string(),
        slicer_menu_path: "Calibration → Temperature".to_string(),
        when_to_run: "Run 1st (before Flow Rate or Pressure Advance) to find the optimal melt temperature for layer strength vs. stringing.".to_string(),
        how_to_read_print: vec![
          "1. Print the Temperature Tower for your polymer (e.g. 265°C down to 240°C for PCTG/PETG, or 230°C down to 195°C for PLA).".to_string(),
          "2. Inspect each floor's overhang curve, bridge underside, and stringing cones.".to_string(),
          "3. Try flexing/snapping the tower between floors: pick the lowest temperature floor that still has strong layer-to-layer bonding and a consistent surface sheen.".to_string(),
        ],
        formula_summary: "Sets nozzle_temperature = Best_Floor_Temp and nozzle_temperature_initial_layer = Best_Floor_Temp + First_Layer_Boost".to_string(),
        default_condition_expr: "filament:PCTG".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "best_temp".to_string(),
            question: "1. Which temperature floor (°C) had the best balance of strong layer bonding and low stringing?".to_string(),
            hint: "Remember: Hardened Steel nozzles typically read ~5°C higher than Brass for the same polymer melt temperature.".to_string(),
            default_value: "255".to_string(),
            unit: "°C".to_string(),
            quick_options: vec![
              ("205".to_string(), "205°C (PLA Brass)".to_string()),
              ("215".to_string(), "215°C (PLA+ / PLA-CF HS)".to_string()),
              ("220".to_string(), "220°C (PLA+ HS)".to_string()),
              ("235".to_string(), "235°C (PETG HS)".to_string()),
              ("255".to_string(), "255°C (PCTG Low-Stringing HS)".to_string()),
              ("260".to_string(), "260°C (PCTG High-Strength / ABS HS)".to_string()),
              ("275".to_string(), "275°C (PA6 Nylon HS)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "first_layer_boost".to_string(),
            question: "2. How much hotter should Layer 1 be for bed adhesion (°C)?".to_string(),
            hint: "+5°C on the initial layer lowers melt viscosity so the first layer wets into the Textured PEI plate.".to_string(),
            default_value: "5".to_string(),
            unit: "°C".to_string(),
            quick_options: vec![
              ("0".to_string(), "+0°C (Same as continuous)".to_string()),
              ("5".to_string(), "+5°C (Recommended)".to_string()),
              ("10".to_string(), "+10°C (Extra first-layer tack)".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "cal_retraction_tower".to_string(),
        name: "Retraction & Anti-Stringing Tower Calibration".to_string(),
        icon: "🕸️".to_string(),
        category: "Calibration Print #4 (Retraction & Wipe)".to_string(),
        slicer_menu_path: "Calibration → Retraction Test".to_string(),
        when_to_run: "Run 4th (only if fine cobwebs/stringing remain after drying filament and calibrating Temp + PA).".to_string(),
        how_to_read_print: vec![
          "1. Print the Retraction Tower (default: Start = 0.2 mm, Step = 0.1 mm per 1 mm of Z height).".to_string(),
          "2. Measure the height Z (in mm) from the bottom where stringing between the two posts disappears: Retraction_mm = Start + (Z × Step).".to_string(),
          "3. Choose the shortest retraction length that stops stringing (keep ≤ 1.4 mm on Kobra S1 all-metal direct drive to avoid pulling molten filament into the cold heat-break).".to_string(),
        ],
        formula_summary: "Retraction_mm = Start + (Z_height × Step) • Pairs with Spiral Z-Hop & 70% Retract-Before-Wipe".to_string(),
        default_condition_expr: "filament:PCTG".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "retract_len".to_string(),
            question: "1. What was the lowest Retraction Length (mm) on the tower that stopped stringing?".to_string(),
            hint: "For Kobra S1 Direct Drive: PLA = 0.6–0.8 mm, PETG/PCTG = 0.85–1.10 mm.".to_string(),
            default_value: "0.95".to_string(),
            unit: "mm".to_string(),
            quick_options: vec![
              ("0.60".to_string(), "0.60 mm (DiamondBack / PLA)".to_string()),
              ("0.80".to_string(), "0.80 mm (Stock Kobra S1)".to_string()),
              ("0.95".to_string(), "0.95 mm (PCTG / PETG Balanced)".to_string()),
              ("1.10".to_string(), "1.10 mm (PCTG Anti-Stringing)".to_string()),
              ("1.25".to_string(), "1.25 mm (Max Direct Drive)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "retract_speed".to_string(),
            question: "2. What Retraction Speed (mm/s) should be used?".to_string(),
            hint: "40–45 mm/s snaps the molten strand cleanly; speeds >55 mm/s risk grinding filament.".to_string(),
            default_value: "45".to_string(),
            unit: "mm/s".to_string(),
            quick_options: vec![
              ("35".to_string(), "35 mm/s (Gentle)".to_string()),
              ("40".to_string(), "40 mm/s (Standard)".to_string()),
              ("45".to_string(), "45 mm/s (Recommended for PCTG/PETG)".to_string()),
              ("50".to_string(), "50 mm/s (Fast)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "z_hop_style".to_string(),
            question: "3. Which Z-Hop Lift Style should be used during travel moves?".to_string(),
            hint: "Spiral lift corkscrews sideways as it rises, shearing sticky PCTG/PETG hairs instead of pulling them straight up.".to_string(),
            default_value: "Spiral".to_string(),
            unit: String::new(),
            quick_options: vec![
              ("Spiral".to_string(), "Spiral Lift (0.15mm — Best for PCTG/PETG)".to_string()),
              ("Slope".to_string(), "Slope Lift (0.15mm)".to_string()),
              ("Normal Lift".to_string(), "Normal Vertical Lift (0.20mm)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "retract_before_wipe".to_string(),
            question: "4. What percentage of retraction should occur BEFORE the 2mm wipe?".to_string(),
            hint: "70% relieves nozzle pressure before the wipe motion starts so the tip wipes clean.".to_string(),
            default_value: "70%".to_string(),
            unit: String::new(),
            quick_options: vec![
              ("70%".to_string(), "70% (Recommended for PCTG/PETG)".to_string()),
              ("50%".to_string(), "50% (Moderate)".to_string()),
              ("0%".to_string(), "0% (Standard PLA)".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "cal_max_volumetric_speed".to_string(),
        name: "Max Volumetric Speed (MVS) Race Track Calibration".to_string(),
        icon: "🏎️".to_string(),
        category: "Calibration Print #5 (Volumetric Flow Ceiling)".to_string(),
        slicer_menu_path: "Calibration → More… → Max Volumetric Speed".to_string(),
        when_to_run: "Run 5th to measure the true melting limit (mm³/s) of your hotend + nozzle + filament combination.".to_string(),
        how_to_read_print: vec![
          "1. Print the Max Volumetric Speed test (default: Start = 8.0 mm³/s, Step = 0.5 mm³/s per 1 mm of Z height).".to_string(),
          "2. Use calipers to measure the height Z (in mm) from the bottom build plate right where the glossy wall turns matte, develops wavy under-extrusion ripples, or starts skipping.".to_string(),
          "3. Enter Start Flow, Step per mm, and Measured Height Z below: we compute Peak_Flow = Start + (Z × Step) and apply your safety margin!".to_string(),
        ],
        formula_summary: "Safe_MVS = (Start_Flow + Measured_Height_Z × Step_Per_mm) × (1 - Safety_Margin_Pct / 100)".to_string(),
        default_condition_expr: "filament:PCTG".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "start_flow".to_string(),
            question: "1. What Starting Volumetric Flow (mm³/s) did you set in the MVS dialog?".to_string(),
            hint: "Default in OrcaSlicer / AnycubicSlicerNext is 8.0 mm³/s (or 5.0 mm³/s for TPU).".to_string(),
            default_value: "8.0".to_string(),
            unit: "mm³/s".to_string(),
            quick_options: vec![
              ("5.0".to_string(), "5.0 mm³/s".to_string()),
              ("8.0".to_string(), "8.0 mm³/s (Default)".to_string()),
              ("10.0".to_string(), "10.0 mm³/s".to_string()),
              ("12.0".to_string(), "12.0 mm³/s".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "step_per_mm".to_string(),
            question: "2. What Flow Step (mm³/s per 1 mm of Z height) did you set?".to_string(),
            hint: "Default in OrcaSlicer / AnycubicSlicerNext is 0.50 mm³/s per mm.".to_string(),
            default_value: "0.50".to_string(),
            unit: "mm³/s/mm".to_string(),
            quick_options: vec![
              ("0.25".to_string(), "0.25 mm³/s per mm".to_string()),
              ("0.50".to_string(), "0.50 mm³/s per mm (Default)".to_string()),
              ("1.00".to_string(), "1.00 mm³/s per mm".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "measured_z".to_string(),
            question: "3. At what height Z (mm) from the bottom did the print start turning matte or wavy?".to_string(),
            hint: "Example: Start 8.0 + (14 mm × 0.5) = 15.0 mm³/s peak → 13.5 mm³/s with 10% safety margin.".to_string(),
            default_value: "14.0".to_string(),
            unit: "mm".to_string(),
            quick_options: vec![
              ("10.0".to_string(), "10 mm".to_string()),
              ("12.0".to_string(), "12 mm".to_string()),
              ("14.0".to_string(), "14 mm".to_string()),
              ("16.0".to_string(), "16 mm".to_string()),
              ("20.0".to_string(), "20 mm".to_string()),
              ("24.0".to_string(), "24 mm".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "margin_pct".to_string(),
            question: "4. What safety margin below the failure point should be kept?".to_string(),
            hint: "A 10% safety margin prevents under-extrusion during long high-speed infill runs.".to_string(),
            default_value: "10".to_string(),
            unit: "%".to_string(),
            quick_options: vec![
              ("10".to_string(), "10% Safety Margin (Recommended)".to_string()),
              ("15".to_string(), "15% Conservative Margin".to_string()),
              ("5".to_string(), "5% Aggressive".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "cal_dimensional_xy".to_string(),
        name: "Dimensional Accuracy & X-Y Hole / Contour Compensation".to_string(),
        icon: "📏".to_string(),
        category: "Calibration Print #6 (Dimensional Fit)".to_string(),
        slicer_menu_path: "Calibration → Tolerance Test / 20×20mm Cube with 10mm Center Bore".to_string(),
        when_to_run: "Run when press-fit bearings, dowels, or M3/M5 screw holes print slightly undersized or oversized.".to_string(),
        how_to_read_print: vec![
          "1. Print a 20.00 mm outer calibration cube that has a 10.00 mm vertical hole in the center.".to_string(),
          "2. Wait for the part to cool to room temperature (20°C), then measure the outer width and inner bore diameter with calipers.".to_string(),
          "3. Enter both caliper readings below: we calculate xy_contour_compensation = (20.00 - Outer) / 2 and xy_hole_compensation = (10.00 - Hole) / 2.".to_string(),
        ],
        formula_summary: "xy_contour_compensation = (20.00 - Measured_Outer) / 2  •  xy_hole_compensation = (10.00 - Measured_Hole) / 2".to_string(),
        default_condition_expr: "all".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "measured_outer".to_string(),
            question: "1. What did your calipers measure for the 20.00 mm outer cube width (mm)?".to_string(),
            hint: "If it measured 20.08 mm (slightly fat), xy_contour_compensation will be -0.04 mm.".to_string(),
            default_value: "20.04".to_string(),
            unit: "mm".to_string(),
            quick_options: vec![
              ("19.90".to_string(), "19.90 mm (+0.05mm comp)".to_string()),
              ("19.96".to_string(), "19.96 mm (+0.02mm comp)".to_string()),
              ("20.00".to_string(), "20.00 mm (0.00mm exact)".to_string()),
              ("20.04".to_string(), "20.04 mm (-0.02mm comp)".to_string()),
              ("20.08".to_string(), "20.08 mm (-0.04mm comp)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "measured_hole".to_string(),
            question: "2. What did your calipers measure for the 10.00 mm inner center hole (mm)?".to_string(),
            hint: "Inner holes almost always shrink slightly in FDM (e.g. 9.84 mm → +0.08 mm xy_hole_compensation).".to_string(),
            default_value: "9.84".to_string(),
            unit: "mm".to_string(),
            quick_options: vec![
              ("9.76".to_string(), "9.76 mm (+0.12mm hole comp)".to_string()),
              ("9.80".to_string(), "9.80 mm (+0.10mm hole comp)".to_string()),
              ("9.84".to_string(), "9.84 mm (+0.08mm hole comp)".to_string()),
              ("9.90".to_string(), "9.90 mm (+0.05mm hole comp)".to_string()),
              ("10.00".to_string(), "10.00 mm (0.00mm exact)".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "cal_scarf_arachne".to_string(),
        name: "Scarf Joint Seam & Arachne Wall Parameterized Tuning".to_string(),
        icon: "🪡".to_string(),
        category: "Calibration Print #7 (Seam & Wall Quality)".to_string(),
        slicer_menu_path: "Process → Quality → Scarf Joint Seam & Wall Generator".to_string(),
        when_to_run: "Run when tuning Z-seam visibility on cylinders/round parts or thin tapered walls.".to_string(),
        how_to_read_print: vec![
          "1. Print a test cylinder (25 mm diameter) and inspect the vertical Z-seam joint.".to_string(),
          "2. If you see a ridge at the seam, increase Scarf Ramp Length to 20–25 mm; if inner bore holes have a seam bump, switch Scarf Mode from 'contour' to 'all'.".to_string(),
        ],
        formula_summary: "Configures seam_slope_type, seam_slope_min_length, seam_gap, and wall_generator".to_string(),
        default_condition_expr: "all".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "scarf_mode".to_string(),
            question: "1. Where should Scarf Joint Seams be applied?".to_string(),
            hint: "'contour' applies scarf ramps to visible outer walls; 'all' applies to both outer walls and inner holes.".to_string(),
            default_value: "all".to_string(),
            unit: String::new(),
            quick_options: vec![
              ("contour".to_string(), "Outer Contour Only".to_string()),
              ("all".to_string(), "All Walls (Outer Contour + Inner Holes)".to_string()),
              ("none".to_string(), "Disable Scarf Seams".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "scarf_len".to_string(),
            question: "2. What Scarf Ramp Length (mm) should be used?".to_string(),
            hint: "15 mm is standard; 20–25 mm gives an even flatter ramp on glossy PETG/PCTG cylinders.".to_string(),
            default_value: "20".to_string(),
            unit: "mm".to_string(),
            quick_options: vec![
              ("15".to_string(), "15 mm (Standard)".to_string()),
              ("20".to_string(), "20 mm (Smooth Ramp)".to_string()),
              ("25".to_string(), "25 mm (Extra-Gradual Ramp)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "seam_gap".to_string(),
            question: "3. What Seam Gap (% of nozzle diameter) should be left at loop closure?".to_string(),
            hint: "10% prevents a raised bump where the end of the perimeter overlaps the start.".to_string(),
            default_value: "10%".to_string(),
            unit: String::new(),
            quick_options: vec![
              ("5%".to_string(), "5% (Tight)".to_string()),
              ("10%".to_string(), "10% (Recommended)".to_string()),
              ("15%".to_string(), "15% (Extra Wipe Clearance)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "wall_gen".to_string(),
            question: "4. Which Wall Generator Engine should be used?".to_string(),
            hint: "Arachne dynamically varies extrusion width to fill thin walls and sharp text without gap infill.".to_string(),
            default_value: "arachne".to_string(),
            unit: String::new(),
            quick_options: vec![
              ("arachne".to_string(), "Arachne Variable-Width Walls (Recommended)".to_string()),
              ("classic".to_string(), "Classic Fixed-Width Walls".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "sym_stringing".to_string(),
        name: "Fix Stringing, Cobwebs & Travel Oozing".to_string(),
        icon: "🕸️".to_string(),
        category: "Filament Symptom Fix (Stringing & Oozing)".to_string(),
        slicer_menu_path: "Filament Settings → Setting Overrides (Retraction, Z-Hop, Wipe) & Cooling".to_string(),
        when_to_run: "Apply when a filament (especially PETG, PCTG, PLA+, or TPU) leaves fine hairs, cobwebs, or branching strings across travel moves.".to_string(),
        how_to_read_print: vec![
          "1. Fine hair-like cobwebs are usually caused by the nozzle lifting straight up (Normal Z-Hop) pulling a molten strand — switching to Spiral Z-Hop + 70% Retract-Before-Wipe shears the strand cleanly.".to_string(),
          "2. Thick branching strings or heavy oozing happen when melt-zone viscosity is too low (nozzle temp 3–8°C too hot) or retraction length is slightly too short.".to_string(),
          "3. Answer the questions below to generate a relative anti-stringing Lua rule that scales cleanly across any nozzle or hotend.".to_string(),
        ],
        formula_summary: "Nozzle_new = Nozzle_old + Temp_Drop  •  Retraction_new = Retraction_old + Retract_Boost  •  Deretract × 0.75  •  Spiral Z-Hop (0.75× Layer_Height)".to_string(),
        default_condition_expr: "filament:PCTG".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "temp_drop".to_string(),
            question: "1. How much should we lower the nozzle temperature (°C) relative to current?".to_string(),
            hint: "-3°C is ideal for mild cobwebs (like PCTG); -5°C to -8°C stops heavy branching strings.".to_string(),
            default_value: "-3".to_string(),
            unit: "°C".to_string(),
            quick_options: vec![
              ("0".to_string(), "0°C (Keep Current Temp)".to_string()),
              ("-3".to_string(), "-3°C (Mild Stringing / PCTG)".to_string()),
              ("-5".to_string(), "-5°C (Moderate Stringing / PETG)".to_string()),
              ("-8".to_string(), "-8°C (Heavy Oozing)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "retract_boost".to_string(),
            question: "2. How much extra retraction distance (mm) should we add to your current retraction?".to_string(),
            hint: "Adding +0.15mm to +0.25mm relieves nozzle tip pressure before travel without pulling molten plastic into the cold heatbreak.".to_string(),
            default_value: "+0.15".to_string(),
            unit: "mm".to_string(),
            quick_options: vec![
              ("+0.00".to_string(), "+0.00 mm (Keep Current)".to_string()),
              ("+0.10".to_string(), "+0.10 mm (Subtle)".to_string()),
              ("+0.15".to_string(), "+0.15 mm (Recommended)".to_string()),
              ("+0.25".to_string(), "+0.25 mm (Strong)".to_string()),
              ("+0.35".to_string(), "+0.35 mm (Max Direct-Drive Boost)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "z_hop_style".to_string(),
            question: "3. Which Z-Hop trajectory should be used on travel moves?".to_string(),
            hint: "Spiral (Corkscrew) and Slope wipe sideways as the nozzle lifts (0.75× layer height) so sticky PETG/PCTG strands snap off cleanly.".to_string(),
            default_value: "Spiral".to_string(),
            unit: String::new(),
            quick_options: vec![
              ("Spiral".to_string(), "Spiral / Corkscrew (0.75× Layer Height — Recommended)".to_string()),
              ("Slope".to_string(), "Slope / Diagonal Ramp (0.90× Layer Height)".to_string()),
              ("Normal".to_string(), "Normal Vertical Lift (1.00× Layer Height)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "pre_wipe".to_string(),
            question: "4. What percentage of retraction should happen BEFORE the wipe motion (filament_retract_before_wipe)?".to_string(),
            hint: "70%–80% relieves pressure while the nozzle is still wiping inside the inner wall.".to_string(),
            default_value: "70%".to_string(),
            unit: String::new(),
            quick_options: vec![
              ("50%".to_string(), "50% (Light)".to_string()),
              ("70%".to_string(), "70% (Recommended for PCTG/PETG)".to_string()),
              ("85%".to_string(), "85% (Aggressive Anti-Stringing)".to_string()),
              ("100%".to_string(), "100% (Full Pre-Wipe)".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "sym_boogers".to_string(),
        name: "Fix Boogers, Nozzle Blobs, Zits & Burnt Drips".to_string(),
        icon: "🦠".to_string(),
        category: "Filament Symptom Fix (Boogers & Blobs)".to_string(),
        slicer_menu_path: "Filament Flow Ratio + Relative Deretraction Speed + Non-Crossing Infill + Wipe on Loops".to_string(),
        when_to_run: "Apply when molten plastic snowballs on the nozzle tip and drops burnt boogers onto the print, or leaves zits/pimples along walls.".to_string(),
        how_to_read_print: vec![
          "1. Boogers on PETG/PCTG happen when slight over-extrusion (1–3%) or a crossing infill pattern (Grid/Triangles) causes the nozzle tip to scrape up ridges of plastic until a blob falls off.".to_string(),
          "2. Zits/pimples along outer walls happen when deretraction (reloading filament after travel) is too fast, causing a pressure burp at seam starts.".to_string(),
          "3. Trimming flow by -0.02, scaling deretraction speed down by -25% (0.75×), and using Gyroid/Crosshatch infill eliminates nozzle buildup on any hotend.".to_string(),
        ],
        formula_summary: "Flow_new = Flow_old + Flow_Trim  •  Deretract_new = Deretract_old × (1 - Slowdown%)  •  Infill = Non-Crossing".to_string(),
        default_condition_expr: "filament:PCTG".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "flow_trim".to_string(),
            question: "1. How much should we trim Flow Ratio (filament_flow_ratio) relative to current to stop plastic snow-plowing on the nozzle tip?".to_string(),
            hint: "-0.02 (-2%) is the sweet spot when PETG/PCTG prints fine overall but accumulates small nozzle boogers over long prints.".to_string(),
            default_value: "-0.02".to_string(),
            unit: "ratio".to_string(),
            quick_options: vec![
              ("0.00".to_string(), "0.00 (Keep Current Flow)".to_string()),
              ("-0.01".to_string(), "-0.01 (-1% Subtle Trim)".to_string()),
              ("-0.02".to_string(), "-0.02 (-2% Recommended Anti-Booger)".to_string()),
              ("-0.03".to_string(), "-0.03 (-3% Heavy Buildup Fix)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "deretract_spd".to_string(),
            question: "2. By what percentage should we slow Deretraction (Reload) Speed to prevent seam zits/blobs?".to_string(),
            hint: "Scaling deretraction speed by -25% (0.75× of your current reload speed) prevents the nozzle from 'burping' a blob when resuming extrusion.".to_string(),
            default_value: "-25%".to_string(),
            unit: "%".to_string(),
            quick_options: vec![
              ("-15%".to_string(), "-15% (0.85× Mild Reload Slowdown)".to_string()),
              ("-25%".to_string(), "-25% (0.75× Recommended Anti-Burp)".to_string()),
              ("-35%".to_string(), "-35% (0.65× Ultra-Smooth Seam Restart)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "infill_pattern".to_string(),
            question: "3. Switch sparse infill to a non-crossing pattern so the nozzle never scrapes over infill intersections?".to_string(),
            hint: "Grid infill crosses over itself on the same layer, scraping plastic onto the nozzle tip. Gyroid and Crosshatch never cross on the same layer.".to_string(),
            default_value: "gyroid".to_string(),
            unit: String::new(),
            quick_options: vec![
              ("gyroid".to_string(), "gyroid (Zero Same-Layer Crossings — Best)".to_string()),
              ("crosshatch".to_string(), "crosshatch (Fast Non-Crossing)".to_string()),
              ("keep".to_string(), "Keep Current Infill Pattern".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "seam_gap".to_string(),
            question: "4. What Seam Gap (% of nozzle diameter) should be left at outer wall loop closure?".to_string(),
            hint: "10%–12% of nozzle diameter prevents the end of the outer wall loop from plowing into the start of the loop.".to_string(),
            default_value: "10%".to_string(),
            unit: String::new(),
            quick_options: vec![
              ("8%".to_string(), "8% (Standard)".to_string()),
              ("10%".to_string(), "10% (Recommended)".to_string()),
              ("12%".to_string(), "12% (Extra Anti-Zit Clearance)".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "sym_clogging".to_string(),
        name: "Fix Clogging, Heat Creep, Extruder Clicking & AMS Tip Jams".to_string(),
        icon: "🔥".to_string(),
        category: "Filament Symptom Fix (Clogging & Extruder Skipping)".to_string(),
        slicer_menu_path: "Relative Retraction Trim + Melt-Zone Temp / MVS Scaling + AMS Cut Retraction".to_string(),
        when_to_run: "Apply when the nozzle clogs mid-print, the extruder clicks/skips during fast infill, PLA softens from heat creep, or ACE Pro swaps jam.".to_string(),
        how_to_read_print: vec![
          "1. Heat-Creep Clog (clogging 30–90 min into a print): retraction is pulling softened filament too high into the cold throat — shorten retraction distance by -15% to -25% and avoid excessive chamber heat on PLA.".to_string(),
          "2. Extruder Clicking on Fast Infill: volumetric flow exceeds melt capacity — add +5°C to +10°C nozzle temp and scale Max Volumetric Speed by -15% to -25% (0.85×–0.75×).".to_string(),
          "3. ACE Pro / AMS Swap Jam: long cut retraction (> 14mm) pulls molten strands into the selector — scale cut retraction down by -15% (≤ 12.0mm).".to_string(),
        ],
        formula_summary: "Retraction_new = Retraction_old × (1 - Retract_Trim%)  •  MVS_new = MVS_old × (1 - MVS_Drop%)  •  Nozzle {:+}°C".to_string(),
        default_condition_expr: "all".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "clog_mode".to_string(),
            question: "1. What type of clogging or extruder issue are you experiencing?".to_string(),
            hint: "Select the symptom that best matches when the failure happens.".to_string(),
            default_value: "heat_creep".to_string(),
            unit: String::new(),
            quick_options: vec![
              ("heat_creep".to_string(), "Heat Creep (Clogs 30+ min into print)".to_string()),
              ("melt_underrun".to_string(), "Extruder Clicking on Fast Infill (Melt Underrun)".to_string()),
              ("ams_swap".to_string(), "ACE Pro / AMS Filament Swap Tip Jam".to_string()),
              ("cf_particle".to_string(), "Carbon / Glass Fiber (-CF/-GF) Orifice Bridging".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "temp_adj".to_string(),
            question: "2. How should we adjust the nozzle temperature (°C) relative to current?".to_string(),
            hint: "Use +5°C to +10°C if the extruder clicks during fast extrusion or with -CF filament; use -5°C if PLA is softening from heat creep.".to_string(),
            default_value: "+5".to_string(),
            unit: "°C".to_string(),
            quick_options: vec![
              ("-5".to_string(), "-5°C (Cooler — PLA Heat Creep Fix)".to_string()),
              ("0".to_string(), "0°C (Keep Current Temp)".to_string()),
              ("+5".to_string(), "+5°C (Lower Viscosity / Easier Flow)".to_string()),
              ("+10".to_string(), "+10°C (High-Flow / Carbon Fiber Boost)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "max_retract".to_string(),
            question: "3. By what percentage should we shorten Retraction Distance (filament_retraction_length) to keep molten polymer out of the cold throat?".to_string(),
            hint: "Scaling retraction distance by -20% (0.80× of your current retraction) prevents molten polymer from being pulled into the cold heatbreak throat.".to_string(),
            default_value: "-20%".to_string(),
            unit: "%".to_string(),
            quick_options: vec![
              ("-10%".to_string(), "-10% (0.90× Subtle Retraction Trim)".to_string()),
              ("-20%".to_string(), "-20% (0.80× Recommended All-Metal Safe)".to_string()),
              ("-30%".to_string(), "-30% (0.70× Strict Heat-Creep / CF Trim)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "mvs_drop_pct".to_string(),
            question: "4. By what percentage should we scale down Max Volumetric Speed (mm³/s) for safety?".to_string(),
            hint: "Scaling MVS by -15% to -25% (0.85×–0.75× of your current hotend/filament MVS) stops the extruder from outrunning the heater block.".to_string(),
            default_value: "15".to_string(),
            unit: "%".to_string(),
            quick_options: vec![
              ("0".to_string(), "0% (1.00× Keep Current MVS)".to_string()),
              ("15".to_string(), "-15% MVS (0.85× Recommended Safety Cushion)".to_string()),
              ("25".to_string(), "-25% MVS (0.75× Stop Extruder Clicking / CF Safe)".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "sym_warping_adhesion".to_string(),
        name: "Fix Bed Adhesion, First-Layer Peeling & Corner Warping".to_string(),
        icon: "🧲".to_string(),
        category: "Filament Symptom Fix (Bed Adhesion & Warping)".to_string(),
        slicer_menu_path: "Bed Temp Boost + Relative First Layer Speed Scaling + Initial Cooling Fan Delay + Brim".to_string(),
        when_to_run: "Apply when corners lift off the build plate, first-layer lines don't stick to Textured PEI, or tall parts get knocked loose.".to_string(),
        how_to_read_print: vec![
          "1. First-layer peeling on Textured PEI happens when the first layer is printed too fast for the polymer to wet the surface, the bed is 5°C too cool, or the part cooling fan turns on too early.".to_string(),
          "2. Corner warping on large/boxy parts happens when upper layers cool and contract before the base has locked into the PEI texture.".to_string(),
        ],
        formula_summary: "Bed_new = Bed_old + Bed_Boost  •  Layer1_Speed_new = Layer1_Speed_old × (1 - Slowdown%)  •  Fan_Off_Layers += Extra_Layers".to_string(),
        default_condition_expr: "all".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "bed_boost".to_string(),
            question: "1. How much should we increase the build plate temperature (°C) relative to current?".to_string(),
            hint: "+5°C (e.g. 60→65°C for PLA, 70→75°C for PETG, 75→80°C for PCTG) dramatically improves Textured PEI grip.".to_string(),
            default_value: "+5".to_string(),
            unit: "°C".to_string(),
            quick_options: vec![
              ("0".to_string(), "0°C (Keep Current Bed Temp)".to_string()),
              ("+5".to_string(), "+5°C (Recommended PEI Grip Boost)".to_string()),
              ("+10".to_string(), "+10°C (Maximum Anti-Warp Tack)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "first_layer_spd".to_string(),
            question: "2. By what percentage should we slow First Layer Print Speed (initial_layer_speed & initial_layer_infill_speed)?".to_string(),
            hint: "Scaling first-layer speed by -30% (0.70× of your current first-layer speed) gives molten plastic time to flow deep into Textured PEI micro-valleys on any nozzle.".to_string(),
            default_value: "-30%".to_string(),
            unit: "%".to_string(),
            quick_options: vec![
              ("-20%".to_string(), "-20% (0.80× Moderate First-Layer Slowdown)".to_string()),
              ("-30%".to_string(), "-30% (0.70× Recommended PEI Grip)".to_string()),
              ("-45%".to_string(), "-45% (0.55× Ultra-Slow Maximum Tack)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "fan_off_layers".to_string(),
            question: "3. How many EXTRA initial layers should we keep the part cooling fan completely OFF?".to_string(),
            hint: "Adding +2 extra fan-off layers prevents early thermal shrinkage from curling bottom edges on PETG/PCTG/ASA.".to_string(),
            default_value: "+2".to_string(),
            unit: "layers".to_string(),
            quick_options: vec![
              ("+0".to_string(), "+0 Layers (Keep Current Fan Delay)".to_string()),
              ("+1".to_string(), "+1 Extra Layer Fan Off".to_string()),
              ("+2".to_string(), "+2 Extra Layers Fan Off (Recommended for PETG / PCTG)".to_string()),
              ("+3".to_string(), "+3 Extra Layers Fan Off (Maximum Anti-Curl)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "brim_type".to_string(),
            question: "4. Which Brim mode (brim_type) should we enable to anchor corners?".to_string(),
            hint: "outer_only adds a clean outer brim around the part footprint.".to_string(),
            default_value: "outer_only".to_string(),
            unit: String::new(),
            quick_options: vec![
              ("outer_only".to_string(), "outer_only (Outer Brim — Recommended)".to_string()),
              ("auto_brim".to_string(), "auto_brim (Slicer Automatic)".to_string()),
              ("no_brim".to_string(), "no_brim (No Brim)".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "sym_overhang_curl".to_string(),
        name: "Fix Overhang Curling, Drooping Bridges & Corner Pillowing".to_string(),
        icon: "🌀".to_string(),
        category: "Filament Symptom Fix (Overhangs & Cooling)".to_string(),
        slicer_menu_path: "Overhang Fan Speed Boost + Relative Minimum Layer Time Boost + Inner/Outer Wall Order".to_string(),
        when_to_run: "Apply when steep overhangs curl upward (causing the nozzle to knock into them), bridges sag, or small spires overheat.".to_string(),
        how_to_read_print: vec![
          "1. Overhang curling happens when a new layer is deposited before the underlying overhang perimeter has solidified.".to_string(),
          "2. Adding +3s to +5s to minimum layer time (`slow_down_layer_time`), boosting overhang fan by +15% to +25% (up to 100%), and printing Inner-then-Outer walls anchors overhangs cleanly.".to_string(),
        ],
        formula_summary: "Overhang_Fan_new = min(100%, Fan_old + Fan_Boost%)  •  Layer_Time_new = Layer_Time_old + Extra_Seconds  •  Wall Sequence = inner/outer".to_string(),
        default_condition_expr: "all".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "overhang_fan".to_string(),
            question: "1. How much should we boost Fan Speed on steep overhangs & bridges (overhang_fan_speed)?".to_string(),
            hint: "Adding +20% to +30% (capped at 100%) freezes unsupported overhang edges even on low-fan filaments like PETG/PCTG.".to_string(),
            default_value: "+20%".to_string(),
            unit: "%".to_string(),
            quick_options: vec![
              ("+10%".to_string(), "+10% Overhang Fan Boost (Mild)".to_string()),
              ("+20%".to_string(), "+20% Overhang Fan Boost (Recommended)".to_string()),
              ("+35%".to_string(), "+35% / 100% Max Overhang Freeze".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "layer_time".to_string(),
            question: "2. How many extra seconds should we add to Minimum Layer Time (slow_down_layer_time)?".to_string(),
            hint: "Adding +4s relative to your profile's current layer time ensures small features and overhang tips cool below glass-transition before the next layer arrives.".to_string(),
            default_value: "+4".to_string(),
            unit: "s".to_string(),
            quick_options: vec![
              ("+2".to_string(), "+2s Extra Layer Cooling Time (Fast / High-Flow)".to_string()),
              ("+4".to_string(), "+4s Extra Layer Cooling Time (Recommended Anti-Curl)".to_string()),
              ("+6".to_string(), "+6s Extra Layer Cooling Time (Small Spires / Heavy Overhangs)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "wall_seq".to_string(),
            question: "3. Which Wall Printing Sequence should be used?".to_string(),
            hint: "Inner/Outer prints the inner wall first so the outer overhang perimeter has an adjacent wall to bond against.".to_string(),
            default_value: "inner wall/outer wall".to_string(),
            unit: String::new(),
            quick_options: vec![
              ("inner wall/outer wall".to_string(), "inner wall/outer wall (Best Overhang Support)".to_string()),
              ("inner-outer-inner wall".to_string(), "inner-outer-inner wall (Best Dimensional Accuracy)".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "sym_weak_layers".to_string(),
        name: "Fix Weak Layer Adhesion, Delamination & Brittle Parts".to_string(),
        icon: "💪".to_string(),
        category: "Filament Symptom Fix (Strength & Layer Bonding)".to_string(),
        slicer_menu_path: "Nozzle Temp Boost + Relative Max Fan Reduction + Extra Structural Wall Perimeters".to_string(),
        when_to_run: "Apply when functional parts snap along layer lines, tall thin walls delaminate, or PETG/PCTG/ASA parts feel brittle.".to_string(),
        how_to_read_print: vec![
          "1. Weak inter-layer bonding is caused by excessive part cooling fan freezing the previous layer before polymer chains can entangle, or printing at the bottom of the filament's temp range.".to_string(),
          "2. Adding +5°C nozzle temperature, scaling continuous max fan down by -25% (0.75×), and adding +1 to +2 extra wall loops dramatically increases Z-axis tensile strength.".to_string(),
        ],
        formula_summary: "Nozzle_new = Nozzle_old + Temp_Boost  •  Max_Fan_new = Max_Fan_old × (1 - Fan_Trim%)  •  Wall_Loops_new = Wall_Loops_old + Extra_Walls".to_string(),
        default_condition_expr: "all".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "temp_boost".to_string(),
            question: "1. How much should we raise the nozzle temperature (°C) relative to current for deeper inter-layer fusion?".to_string(),
            hint: "+5°C to +8°C significantly improves Z-axis layer weld strength.".to_string(),
            default_value: "+5".to_string(),
            unit: "°C".to_string(),
            quick_options: vec![
              ("+5".to_string(), "+5°C (Recommended Structural Boost)".to_string()),
              ("+8".to_string(), "+8°C (Maximum Polymer Fusion)".to_string()),
              ("0".to_string(), "0°C (Keep Current Temp)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "fan_cap".to_string(),
            question: "2. By what percentage should we scale down continuous part cooling fan speed (fan_max_speed)?".to_string(),
            hint: "Scaling fan_max_speed by -25% (0.75× of your filament's current max fan) prevents over-cooling the weld zone on any polymer.".to_string(),
            default_value: "-25%".to_string(),
            unit: "%".to_string(),
            quick_options: vec![
              ("-15%".to_string(), "-15% (0.85× Mild Fan Reduction)".to_string()),
              ("-25%".to_string(), "-25% (0.75× Recommended Structural Weld)".to_string()),
              ("-45%".to_string(), "-45% (0.55× Maximum Layer Fusion / Engineering)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "wall_loops".to_string(),
            question: "3. How many EXTRA Wall Perimeters (wall_loops) should we add?".to_string(),
            hint: "Adding +1 to +2 extra walls adds far more bending and impact strength than increasing sparse infill.".to_string(),
            default_value: "+2".to_string(),
            unit: "walls".to_string(),
            quick_options: vec![
              ("+1".to_string(), "+1 Extra Wall Loop (Strong)".to_string()),
              ("+2".to_string(), "+2 Extra Wall Loops (Recommended Heavy-Duty)".to_string()),
              ("+3".to_string(), "+3 Extra Wall Loops (Maximum Shell Strength)".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "sym_under_extrusion".to_string(),
        name: "Under-Extrusion (Gaps Between Lines & Thin/Spongy Walls)".to_string(),
        icon: "📉".to_string(),
        category: "1. Extrusion & Flow (Flow & Volume)".to_string(),
        slicer_menu_path: "Filament Flow Ratio + Max Volumetric Speed + Nozzle Temperature".to_string(),
        when_to_run: "Apply when adjacent lines on top/bottom surfaces don't touch, perimeter walls have gaps, or layers look spongy and porous.".to_string(),
        how_to_read_print: vec![
          "1. Inspect flat top surfaces and perimeters: if you can see the layer below through gaps between parallel lines, the extruder is pushing less plastic than the slicer expects.".to_string(),
          "2. Check if under-extrusion happens EVERYWHERE (Flow Ratio too low or filament diameter undersized) or ONLY on fast long lines (Max Volumetric Speed outrunning the heater block).".to_string(),
        ],
        formula_summary: "Flow_new = Flow_old + Flow_Boost  •  MVS_new = MVS_old × (1 - Speed_Trim%)  •  Nozzle {:+}°C".to_string(),
        default_condition_expr: "all".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "flow_boost".to_string(),
            question: "1. How much should we increase the Flow Ratio (filament_flow_ratio) relative to current?".to_string(),
            hint: "+0.02 (+2%) closes minor line gaps; +0.04 (+4%) fixes noticeable under-extrusion.".to_string(),
            default_value: "+0.03".to_string(),
            unit: "ratio".to_string(),
            quick_options: vec![
              ("+0.02".to_string(), "+0.02 (+2% Mild Gaps)".to_string()),
              ("+0.03".to_string(), "+0.03 (+3% Recommended)".to_string()),
              ("+0.05".to_string(), "+0.05 (+5% Heavy Under-Extrusion)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "mvs_trim".to_string(),
            question: "2. Reduce Max Volumetric Speed (%) relative to current so fast infill/walls don't outrun the melt zone?".to_string(),
            hint: "Scaling MVS by -15% (0.85×) gives the polymer longer residence time in the heater block.".to_string(),
            default_value: "15".to_string(),
            unit: "%".to_string(),
            quick_options: vec![
              ("0".to_string(), "0% (1.00× Keep Current MVS)".to_string()),
              ("15".to_string(), "-15% MVS (0.85× Recommended)".to_string()),
              ("25".to_string(), "-25% MVS (0.75× High-Viscosity Fix)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "temp_boost".to_string(),
            question: "3. Raise nozzle temperature (°C) relative to current to lower melt viscosity?".to_string(),
            hint: "+5°C reduces back-pressure inside the nozzle orifice.".to_string(),
            default_value: "+5".to_string(),
            unit: "°C".to_string(),
            quick_options: vec![
              ("0".to_string(), "0°C (Keep Current)".to_string()),
              ("+5".to_string(), "+5°C (Easier Melt Flow)".to_string()),
              ("+10".to_string(), "+10°C (High-Speed Boost)".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "sym_over_extrusion".to_string(),
        name: "Over-Extrusion (Plowed Top Ridges, Rough Surface & Dimensional Bloat)".to_string(),
        icon: "📈".to_string(),
        category: "1. Extrusion & Flow (Flow & Volume)".to_string(),
        slicer_menu_path: "Filament Flow Ratio Trim + Relative Top Surface Flow Trim".to_string(),
        when_to_run: "Apply when the nozzle plows up sharp ridges between solid top lines, corners bulge with excess plastic, or parts fit too tight.".to_string(),
        how_to_read_print: vec![
          "1. If solid infill layers feel like rough sandpaper with raised ridges where the nozzle squeezes excess plastic upward, flow is 2%–5% too high.".to_string(),
          "2. If walls look great and only the final top skin looks slightly over-stuffed, trim Top Surface Flow Ratio (`top_surface_flow_ratio`) by -0.03 to -0.05 relative to current.".to_string(),
        ],
        formula_summary: "Flow_new = Flow_old + Flow_Drop  •  Top_Flow_new = Top_Flow_old + Top_Flow_Trim".to_string(),
        default_condition_expr: "all".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "flow_drop".to_string(),
            question: "1. How much should we reduce overall Filament Flow Ratio relative to current?".to_string(),
            hint: "-0.02 (-2%) to -0.03 (-3%) stops plastic from piling up around the nozzle tip.".to_string(),
            default_value: "-0.02".to_string(),
            unit: "ratio".to_string(),
            quick_options: vec![
              ("-0.01".to_string(), "-0.01 (-1% Subtle)".to_string()),
              ("-0.02".to_string(), "-0.02 (-2% Recommended)".to_string()),
              ("-0.03".to_string(), "-0.03 (-3% Moderate)".to_string()),
              ("-0.05".to_string(), "-0.05 (-5% Heavy Over-Extrusion)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "top_flow".to_string(),
            question: "2. How much should we trim Top Surface Flow Ratio (top_surface_flow_ratio) relative to current for a glass-smooth top skin?".to_string(),
            hint: "-0.04 (-4% relative to current top_surface_flow_ratio) leaves zero plowed ridges on the final visible top layer.".to_string(),
            default_value: "-0.04".to_string(),
            unit: "ratio".to_string(),
            quick_options: vec![
              ("0.00".to_string(), "0.00 (Keep Current Top Surface Flow)".to_string()),
              ("-0.02".to_string(), "-0.02 (-2% Smooth Top Skin)".to_string()),
              ("-0.04".to_string(), "-0.04 (-4% Recommended Ultra-Flat Top Skin)".to_string()),
              ("-0.06".to_string(), "-0.06 (-6% Heavy Top Ridge Fix)".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "sym_extruder_grinding".to_string(),
        name: "Grinding Filament, Stripped Gears & Extruder Tension (Kobra S1 Screw)".to_string(),
        icon: "⚙️".to_string(),
        category: "1. Extrusion & Flow (Extruder Drive & Tension)".to_string(),
        slicer_menu_path: "Hardware Tension Screw + Relative Retraction Speed Scaling + Minimum Travel Distance Boost".to_string(),
        when_to_run: "Apply when extruder gears chew a half-moon notch into the filament, plastic dust builds up in the gears, or the print stops extruding mid-job.".to_string(),
        how_to_read_print: vec![
          "1. Extruder grinding happens when (a) the Kobra S1 extruder idler spring tension screw is too loose (gears slip) or too tight (crushes filament over repeated retractions), OR (b) dozens of rapid retractions happen over the exact same 1mm section of filament.".to_string(),
          "2. Follow the Kobra S1 Extruder Tension Screw diagram first, then apply this rule to scale down retraction speed and increase minimum travel distance between retractions.".to_string(),
        ],
        formula_summary: "Retract_Speed_new = Retract_Speed_old × (1 - Slowdown%)  •  Min_Travel_new = Min_Travel_old + Travel_Boost  •  Nozzle {:+}°C".to_string(),
        default_condition_expr: "all".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "retract_spd_cap".to_string(),
            question: "1. By what percentage should we slow Retraction & Deretraction Speed so the hobbed gears don't strip the filament?".to_string(),
            hint: "Scaling retraction speed by -25% (0.75× of current speed) prevents the drive teeth from shearing the filament surface during rapid reversals.".to_string(),
            default_value: "-25%".to_string(),
            unit: "%".to_string(),
            quick_options: vec![
              ("-15%".to_string(), "-15% (0.85× Gentle Gear Engagement)".to_string()),
              ("-25%".to_string(), "-25% (0.75× Recommended Anti-Grind)".to_string()),
              ("-35%".to_string(), "-35% (0.65× Ultra-Gentle / Soft TPU)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "min_travel".to_string(),
            question: "2. How much extra Minimum Travel Distance (filament_retraction_minimum_travel, mm) should we add before allowing a retraction?".to_string(),
            hint: "Adding +1.0mm to +1.5mm to minimum travel distance stops the extruder from retracting 10 times on the same tiny detail.".to_string(),
            default_value: "+1.0".to_string(),
            unit: "mm".to_string(),
            quick_options: vec![
              ("+0.5".to_string(), "+0.5 mm Extra Min Travel".to_string()),
              ("+1.0".to_string(), "+1.0 mm Extra Min Travel (Recommended Anti-Chew)".to_string()),
              ("+1.5".to_string(), "+1.5 mm Extra Min Travel (Dense Details / Mesh)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "temp_relief".to_string(),
            question: "3. Add nozzle temperature (°C) relative to current to reduce hotend back-pressure?".to_string(),
            hint: "+5°C reduces the force required by the extruder gears to push filament through the nozzle.".to_string(),
            default_value: "+5".to_string(),
            unit: "°C".to_string(),
            quick_options: vec![
              ("0".to_string(), "0°C (Keep Current)".to_string()),
              ("+5".to_string(), "+5°C (Recommended Back-Pressure Relief)".to_string()),
              ("+10".to_string(), "+10°C (High Back-Pressure Fix)".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "sym_wet_filament".to_string(),
        name: "Wet Filament (Steam Popping, Pockmarks, Bubbles & Matte/Foamy Walls)".to_string(),
        icon: "💧".to_string(),
        category: "1. Extrusion & Flow (Moisture & Drying)".to_string(),
        slicer_menu_path: "ACE Pro Active Dryer + Nozzle Temp Trim + Flow & Retraction Compensation".to_string(),
        when_to_run: "Apply when you hear tiny 'pop/crackle' sounds from the nozzle, walls have random crater pockmarks, or PETG/PCTG/TPU/Nylon oozes uncontrollably.".to_string(),
        how_to_read_print: vec![
          "1. Absorbed moisture turns to superheated steam at 200°C+, exploding into micro-bubbles as it exits the nozzle tip — leaving crater pockmarks and pushing molten strings out during travel.".to_string(),
          "2. Always dry the spool in the ACE Pro active dryer first! If you must finish a print on slightly damp filament, lowering nozzle temp by -5°C and trimming flow by -0.02 reduces steam expansion.".to_string(),
        ],
        formula_summary: "Nozzle_new = Nozzle_old + Temp_Drop (Reduces Steam Boil)  •  Flow_new = Flow_old + Flow_Trim  •  Spiral Wipe".to_string(),
        default_condition_expr: "all".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "temp_drop".to_string(),
            question: "1. Lower nozzle temperature (°C) relative to current to reduce explosive steam boiling inside the melt zone?".to_string(),
            hint: "-5°C noticeably reduces steam popping and foaming when printing slightly damp PETG/PCTG/PLA.".to_string(),
            default_value: "-5".to_string(),
            unit: "°C".to_string(),
            quick_options: vec![
              ("-3".to_string(), "-3°C (Mild Dampness)".to_string()),
              ("-5".to_string(), "-5°C (Recommended Steam Reduction)".to_string()),
              ("-8".to_string(), "-8°C (Heavy Moisture Mitigation)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "flow_trim".to_string(),
            question: "2. Trim Flow Ratio relative to current to compensate for micro-bubble foaming expansion?".to_string(),
            hint: "-0.02 (-2%) prevents foamy over-extrusion roughness on walls.".to_string(),
            default_value: "-0.02".to_string(),
            unit: "ratio".to_string(),
            quick_options: vec![
              ("0.00".to_string(), "0.00 (Unchanged)".to_string()),
              ("-0.02".to_string(), "-0.02 (-2% Anti-Foam Trim)".to_string()),
              ("-0.03".to_string(), "-0.03 (-3% Strong)".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "sym_zits_blobs".to_string(),
        name: "Blobs, Zits & Pimples Scattered on Outer Walls".to_string(),
        icon: "🫧".to_string(),
        category: "2. Oozing, Stringing & Blobs (Surface Blemishes)".to_string(),
        slicer_menu_path: "Seam Position (Aligned) + Scarf Seam + Relative Deretraction Speed Scaling + Wipe on Loops".to_string(),
        when_to_run: "Apply when small pimples, zits, or raised bumps appear across smooth curved surfaces or at layer start/stop points.".to_string(),
        how_to_read_print: vec![
          "1. Check where the zits appear: if they are scattered randomly all over the model, `seam_position` is set to 'random' OR printer Power-Loss Recovery is pausing briefly to write to flash.".to_string(),
          "2. Aligning seams (`aligned`), enabling Scarf Joint ramp (`contour` or `all`), and scaling deretraction reload speed by -25% (0.75×) eliminates start/stop pimples.".to_string(),
        ],
        formula_summary: "seam_position = aligned  •  Scarf Seam = contour  •  Deretract_new = Deretract_old × (1 - Slowdown%)  •  seam_gap = 10%".to_string(),
        default_condition_expr: "all".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "seam_pos".to_string(),
            question: "1. Where should the Z-Seam be placed (seam_position)?".to_string(),
            hint: "'aligned' or 'back' groups all layer starts into a single hidden corner instead of scattering random pimples.".to_string(),
            default_value: "aligned".to_string(),
            unit: String::new(),
            quick_options: vec![
              ("aligned".to_string(), "aligned (Sharpest Corner — Recommended)".to_string()),
              ("back".to_string(), "back (Rear of Model)".to_string()),
              ("nearest".to_string(), "nearest (Fastest Travel)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "deretract_spd".to_string(),
            question: "2. By what percentage should we slow Deretraction (Reload) Speed to stop pressure burps at layer start?".to_string(),
            hint: "Scaling deretraction speed by -25% (0.75× of your current reload speed) reloads the nozzle smoothly without spitting a zit at the start of the perimeter.".to_string(),
            default_value: "-25%".to_string(),
            unit: "%".to_string(),
            quick_options: vec![
              ("-15%".to_string(), "-15% (0.85× Moderate Reload Slowdown)".to_string()),
              ("-25%".to_string(), "-25% (0.75× Recommended Anti-Zit)".to_string()),
              ("-35%".to_string(), "-35% (0.65× Maximum Anti-Zit Smoothness)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "enable_scarf".to_string(),
            question: "3. Enable Scarf Joint Seam Ramp to feather the loop start/end?".to_string(),
            hint: "Scarf joints ramp layer height over 20mm so there is no step or pimple at loop closure.".to_string(),
            default_value: "contour".to_string(),
            unit: String::new(),
            quick_options: vec![
              ("contour".to_string(), "contour (Outer Walls — Recommended)".to_string()),
              ("all".to_string(), "all (Outer + Inner Walls)".to_string()),
              ("none".to_string(), "none (Standard Seam)".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "sym_not_sticking".to_string(),
        name: "Print Not Sticking to Build Plate (First-Layer Detachment / Spaghetti)".to_string(),
        icon: "🧼".to_string(),
        category: "3. First Layer & Bed Adhesion (Bed Grip & Z-Offset)".to_string(),
        slicer_menu_path: "Relative Initial Layer Line Width (+20%) + Relative First Layer Speed (-35%) + Initial Layer Temp (+5°C)".to_string(),
        when_to_run: "Apply when the first layer fails to stick to the Textured PEI plate, lines drag behind the nozzle, or parts detach mid-print.".to_string(),
        how_to_read_print: vec![
          "1. Look at the underside of the first layer: if individual lines look round like spaghetti with gaps between them, the nozzle is too high from the bed or initial layer width/squish is too narrow.".to_string(),
          "2. Wash the Textured PEI plate with warm water and dish soap first! Then scale First Layer Line Width up by +20% (1.20×), add +5°C bed/nozzle boost, and scale first-layer speed down by -35% (0.65×).".to_string(),
        ],
        formula_summary: "Layer1_Width_new = Layer1_Width_old × (1 + Width_Boost%)  •  Layer1_Speed_new = Layer1_Speed_old × (1 - Slowdown%)  •  Layer 1 Nozzle/Bed +5°C".to_string(),
        default_condition_expr: "all".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "fl_width".to_string(),
            question: "1. By what percentage should we widen First Layer Line Width (initial_layer_line_width) to force polymer into the PEI texture?".to_string(),
            hint: "Scaling initial layer line width by +20% (1.20× of your current initial layer width / nozzle diameter) creates higher downward squish pressure on any nozzle size.".to_string(),
            default_value: "+20%".to_string(),
            unit: "%".to_string(),
            quick_options: vec![
              ("+10%".to_string(), "+10% (1.10× Mild Squish Boost)".to_string()),
              ("+20%".to_string(), "+20% (1.20× High-Squish — Recommended)".to_string()),
              ("+30%".to_string(), "+30% (1.30× Maximum PEI Grip)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "fl_speed".to_string(),
            question: "2. By what percentage should we slow First Layer Print Speed (initial_layer_speed & initial_layer_infill_speed)?".to_string(),
            hint: "Scaling first-layer speed by -35% (0.65× of your current first-layer speed) prevents the nozzle from tearing freshly laid lines off the PEI plate around small holes.".to_string(),
            default_value: "-35%".to_string(),
            unit: "%".to_string(),
            quick_options: vec![
              ("-20%".to_string(), "-20% (0.80× Standard Slowdown)".to_string()),
              ("-35%".to_string(), "-35% (0.65× Recommended PEI Grip)".to_string()),
              ("-50%".to_string(), "-50% (0.50× Intricate / Small Parts)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "fl_temp_boost".to_string(),
            question: "3. Boost Initial Layer Nozzle & Bed Temperature (°C) relative to current?".to_string(),
            hint: "+5°C on Layer 1 lowers polymer viscosity so it wets the PEI surface.".to_string(),
            default_value: "+5".to_string(),
            unit: "°C".to_string(),
            quick_options: vec![
              ("+5".to_string(), "+5°C Nozzle & Bed on Layer 1 (Recommended)".to_string()),
              ("+10".to_string(), "+10°C Extra Tack".to_string()),
              ("0".to_string(), "0°C (Keep Current)".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "sym_elephants_foot".to_string(),
        name: "Elephant's Foot (Bulging / Flared Outward First Layers)".to_string(),
        icon: "🐘".to_string(),
        category: "3. First Layer & Bed Adhesion (First-Layer Dimensional)".to_string(),
        slicer_menu_path: "Process → Quality → Precision → Relative Elephant Foot Compensation Boost + Bed Temp Trim".to_string(),
        when_to_run: "Apply when the bottom 1–3 layers of a print flare outward wider than the rest of the wall, preventing mating parts or gears from fitting.".to_string(),
        how_to_read_print: vec![
          "1. Elephant's foot occurs when the weight of the print + high first-layer squish + a hot bed (above glass transition Tg) squashes the first layer outward by 0.10–0.25mm.".to_string(),
          "2. Adding +0.10mm to +0.15mm to `elefant_foot_compensation` insets the first layer contour inward by that exact extra amount so the finished bottom edge is perfectly flush!".to_string(),
        ],
        formula_summary: "EF_Comp_new = EF_Comp_old + Inset_Boost_mm  •  elefant_foot_compensation_layers = 1  •  Bed Temp {:+}°C".to_string(),
        default_condition_expr: "all".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "ef_comp".to_string(),
            question: "1. How much extra Elephant Foot Compensation inset (elefant_foot_compensation, mm) should we add?".to_string(),
            hint: "Measure the bottom lip flare with calipers and divide by 2 (+0.10 mm to +0.15 mm extra inset fixes most first-layer flare).".to_string(),
            default_value: "+0.10".to_string(),
            unit: "mm".to_string(),
            quick_options: vec![
              ("+0.05".to_string(), "+0.05 mm Extra Inset (Subtle)".to_string()),
              ("+0.10".to_string(), "+0.10 mm Extra Inset (Recommended)".to_string()),
              ("+0.15".to_string(), "+0.15 mm Extra Inset (Strong Flare Fix)".to_string()),
              ("+0.20".to_string(), "+0.20 mm Extra Inset (Heavy Squish)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "bed_trim".to_string(),
            question: "2. Lower continuous bed temperature after Layer 1 (°C) relative to current so the base stays rigid?".to_string(),
            hint: "Keeping Layer 1 hot for grip, then dropping continuous bed temp by -5°C stops bottom layers from creeping.".to_string(),
            default_value: "-5".to_string(),
            unit: "°C".to_string(),
            quick_options: vec![
              ("0".to_string(), "0°C (Keep Continuous Bed Temp)".to_string()),
              ("-5".to_string(), "-5°C Continuous Bed (Recommended)".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "sym_first_layer_ripples".to_string(),
        name: "Rough Waves, Ripples & Tiger-Stripe Furrows on First Layer".to_string(),
        icon: "🌊".to_string(),
        category: "3. First Layer & Bed Adhesion (First-Layer Squish)".to_string(),
        slicer_menu_path: "Relative Initial Layer Height Boost (+20%) + Relative First Layer Line Width Trim (-10%)".to_string(),
        when_to_run: "Apply when a large flat first layer develops diagonal wavy ripples, rough tiger-stripe ridges, or translucent thin spots.".to_string(),
        how_to_read_print: vec![
          "1. Wavy 'tiger-stripe' ripples on Layer 1 mean the nozzle is slightly TOO CLOSE to the build plate or extruding too much plastic for the gap: excess plastic builds up sideways until the nozzle hops over it in a repeating wave.".to_string(),
          "2. Scaling Initial Layer Height (`initial_layer_print_height`) up by +20% (1.20×) and trimming initial layer line width by -10% (0.90×) gives the first layer breathing room on any nozzle size.".to_string(),
        ],
        formula_summary: "Layer1_Height_new = Layer1_Height_old × (1 + Height_Boost%)  •  Layer1_Width_new = Layer1_Width_old × (1 - Width_Trim%)".to_string(),
        default_condition_expr: "all".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "fl_height".to_string(),
            question: "1. By what percentage should we increase Initial Layer Height (initial_layer_print_height) to absorb bed mesh micro-variations?".to_string(),
            hint: "+20% (1.20× of your current initial layer height, e.g. 0.20→0.24mm on 0.4mm nozzle or 0.30→0.36mm on 0.6mm nozzle) absorbs Z-offset tightness.".to_string(),
            default_value: "+20%".to_string(),
            unit: "%".to_string(),
            quick_options: vec![
              ("+10%".to_string(), "+10% (1.10× Mild Buffer)".to_string()),
              ("+20%".to_string(), "+20% (1.20× Recommended Anti-Ripple)".to_string()),
              ("+30%".to_string(), "+30% (1.30× Thick Buffer Layer)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "fl_width".to_string(),
            question: "2. By what percentage should we trim Initial Layer Line Width (initial_layer_line_width) to stop adjacent lines from crowding?".to_string(),
            hint: "Scaling initial layer line width by -10% (0.90× of current initial layer width) stops adjacent lines from snow-plowing into each other.".to_string(),
            default_value: "-10%".to_string(),
            unit: "%".to_string(),
            quick_options: vec![
              ("-5%".to_string(), "-5% (0.95× Subtle Trim)".to_string()),
              ("-10%".to_string(), "-10% (0.90× Recommended Anti-Crowding)".to_string()),
              ("-15%".to_string(), "-15% (0.85× Strong De-Crowding)".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "sym_visible_seam".to_string(),
        name: "Visible Z-Seam Scar, Raised Ridge or Seam Pinhole Gap".to_string(),
        icon: "🪡".to_string(),
        category: "4. Walls, Seams & Dimensional (Z-Seam & Scarf Joint)".to_string(),
        slicer_menu_path: "Process → Quality → Seam (Scarf Joint Seam, Relative Seam Gap Offset, Wipe on Loops)".to_string(),
        when_to_run: "Apply when a vertical zipper scar, raised ridge, or pinhole trench is visible where each outer perimeter loop starts and ends.".to_string(),
        how_to_read_print: vec![
          "1. Inspect the vertical Z-seam: if it sticks OUT as a raised ridge, Seam Gap is too small (boost seam_gap by +2% to +4%) or Scarf Ramp is too short.".to_string(),
          "2. If the seam is a sunken groove/pinhole gap, Pressure Advance or Seam Gap is too large (trim seam_gap by -3% to -5%), starving the end of the loop.".to_string(),
        ],
        formula_summary: "Scarf_Length_new = Scarf_Length_old × (1 + Ramp_Boost%)  •  Seam_Gap_new = Seam_Gap_old ± Offset%  •  wipe_on_loops = 1".to_string(),
        default_condition_expr: "all".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "defect_type".to_string(),
            question: "1. Does your Z-Seam look like a RAISED ridge/bump or a SUNKEN pinhole gap?".to_string(),
            hint: "Raised ridges widen seam_gap by +2%; sunken gaps tighten seam_gap by -4% relative to current.".to_string(),
            default_value: "raised".to_string(),
            unit: String::new(),
            quick_options: vec![
              ("raised".to_string(), "Raised Ridge / Outward Bump (+2% Seam Gap Relief)".to_string()),
              ("sunken".to_string(), "Sunken Groove / Pinhole Gap (-4% Seam Gap Tightening)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "scarf_len".to_string(),
            question: "2. By what percentage should we lengthen the Scarf Joint Ramp (seam_slope_min_length)?".to_string(),
            hint: "Scaling scarf ramp length by +25% (1.25×) overlaps the start and end of the perimeter at a shallower wedge angle.".to_string(),
            default_value: "+25%".to_string(),
            unit: "%".to_string(),
            quick_options: vec![
              ("+0%".to_string(), "+0% (1.00× Keep Current Ramp Length)".to_string()),
              ("+25%".to_string(), "+25% (1.25× Recommended Smooth Scarf)".to_string()),
              ("+50%".to_string(), "+50% (1.50× Ultra-Gradual Cylinder Scarf)".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "sym_ringing_vfa".to_string(),
        name: "Ghosting / Ringing (Wavy Echoes) & VFA Vertical Fine Artifacts".to_string(),
        icon: "〰️".to_string(),
        category: "4. Walls, Seams & Dimensional (Resonance & Belts)".to_string(),
        slicer_menu_path: "Relative Outer Wall Speed Adjustment + Relative Outer Wall Acceleration Scaling".to_string(),
        when_to_run: "Apply when faint repeating ripples ('ghost echoes') appear after sharp corners/lettering, or uniform vertical belt-pitch lines (VFAs) show on glossy walls.".to_string(),
        how_to_read_print: vec![
          "1. Ghosting/Ringing (ripples that fade out after a sharp corner or text) comes from high Outer Wall Acceleration shaking the gantry before Input Shaping can damp it.".to_string(),
          "2. VFAs (uniform vertical micro-stripes across the entire wall) occur when outer wall speed sits inside a stepper/belt resonance band — scale outer wall acceleration down by -40% (0.60×) and shift outer wall speed by +20% (1.20×) or -30% (0.70×) out of resonance.".to_string(),
        ],
        formula_summary: "Outer_Accel_new = Outer_Accel_old × (1 - Accel_Drop%)  •  Outer_Speed_new = Outer_Speed_old × (1 ± Speed_Shift%)".to_string(),
        default_condition_expr: "all".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "outer_accel".to_string(),
            question: "1. By what percentage should we reduce Outer Wall Acceleration & Jerk (outer_wall_acceleration) to eliminate corner ringing?".to_string(),
            hint: "Scaling outer wall acceleration by -40% (0.60× of your current outer wall acceleration) eliminates ghosting with almost zero impact on total print time.".to_string(),
            default_value: "-40%".to_string(),
            unit: "%".to_string(),
            quick_options: vec![
              ("-25%".to_string(), "-25% (0.75× Balanced CoreXY Damping)".to_string()),
              ("-40%".to_string(), "-40% (0.60× Recommended Anti-Ringing)".to_string()),
              ("-55%".to_string(), "-55% (0.45× Ultra-Clean Show Quality)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "outer_spd".to_string(),
            question: "2. How should we shift Outer Wall Speed (outer_wall_speed) relative to current to move out of the VFA belt-resonance band?".to_string(),
            hint: "+20% (1.20×) cruises above the 2GT belt-tooth resonance band on high-flow hotends; -30% (0.70×) slows below resonance for silk/detail parts.".to_string(),
            default_value: "+20%".to_string(),
            unit: "%".to_string(),
            quick_options: vec![
              ("+20%".to_string(), "+20% (1.20× Cruise Above VFA Resonance — Recommended)".to_string()),
              ("-30%".to_string(), "-30% (0.70× Slow Below VFA Resonance — Silk/Detail)".to_string()),
              ("0%".to_string(), "0% (1.00× Keep Current Outer Wall Speed)".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "sym_layer_shifting".to_string(),
        name: "Layer Shifting / Staircase Step Loss Mid-Print".to_string(),
        icon: "🪜".to_string(),
        category: "4. Walls, Seams & Dimensional (Motion & Step Loss)".to_string(),
        slicer_menu_path: "Relative Travel Acceleration & Jerk Scaling + Non-Crossing Infill + Relative Z-Hop Boost".to_string(),
        when_to_run: "Apply when the entire upper portion of a print suddenly shifts sideways in X or Y like a staircase step.".to_string(),
        how_to_read_print: vec![
          "1. Layer shifts on CoreXY printers happen for two reasons: (a) the nozzle collides with a curled overhang or crossing Grid infill intersection during a fast travel move, OR (b) Travel Acceleration / Jerk exceeds stepper motor torque.".to_string(),
          "2. Check belt tension and pulley grub screws first! Then scale Travel Acceleration & Jerk down by -30% (0.70×), add +0.10mm extra Z-Hop clearance, and switch away from Grid infill.".to_string(),
        ],
        formula_summary: "Travel_Accel_new = Travel_Accel_old × (1 - Accel_Drop%)  •  Z_Hop_new = Z_Hop_old + Clearance_Boost  •  Infill = gyroid".to_string(),
        default_condition_expr: "all".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "travel_accel".to_string(),
            question: "1. By what percentage should we reduce Travel Acceleration & Jerk (travel_acceleration, travel_jerk)?".to_string(),
            hint: "Scaling travel acceleration & jerk by -30% (0.70× of current) prevents X/Y stepper motors from losing steps during high-speed travel reversals.".to_string(),
            default_value: "-30%".to_string(),
            unit: "%".to_string(),
            quick_options: vec![
              ("-20%".to_string(), "-20% (0.80× Moderate Step Margin)".to_string()),
              ("-30%".to_string(), "-30% (0.70× Recommended CoreXY Safe)".to_string()),
              ("-45%".to_string(), "-45% (0.55× Maximum Step Torque Safety)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "zhop_height".to_string(),
            question: "2. How much extra Z-Hop Clearance Height (filament_z_hop, mm) should we add so the nozzle clears curled edges?".to_string(),
            hint: "Adding +0.10 mm to +0.20 mm relative to your current Z-Hop lifts the nozzle safely above any curled overhang during travel.".to_string(),
            default_value: "+0.10".to_string(),
            unit: "mm".to_string(),
            quick_options: vec![
              ("+0.05".to_string(), "+0.05 mm Extra Z-Hop Clearance".to_string()),
              ("+0.10".to_string(), "+0.10 mm Extra Z-Hop Clearance (Recommended)".to_string()),
              ("+0.20".to_string(), "+0.20 mm Extra Z-Hop Clearance (Tall / Curled Parts)".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "sym_z_banding_hull_line".to_string(),
        name: "Z-Banding, Inconsistent Layer Lines & 'Benchy Hull Line'".to_string(),
        icon: "📏".to_string(),
        category: "4. Walls, Seams & Dimensional (Z-Axis & Cooling Shrinkage)".to_string(),
        slicer_menu_path: "Don't Slow Down Outer Walls + Consistent Wall Order + Z-Axis Lead Screw Check".to_string(),
        when_to_run: "Apply when horizontal bands or a pronounced step line appear at the height where an internal floor/deck transitions to hollow walls (the classic Benchy hull line).".to_string(),
        how_to_read_print: vec![
          "1. When layer print time changes suddenly (e.g. transitioning from sparse infill to a solid deck), cooling slowdown changes the outer wall speed — changing wall glossiness and thermal shrinkage!".to_string(),
          "2. Enabling `dont_slow_down_outer_wall = 1` keeps the outer perimeter at its exact configured speed and glossiness from bottom to top.".to_string(),
        ],
        formula_summary: "dont_slow_down_outer_wall = 1  •  wall_sequence = inner-outer-inner wall".to_string(),
        default_condition_expr: "all".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "lock_outer_spd".to_string(),
            question: "1. Lock Outer Wall Speed (dont_slow_down_outer_wall) so layer-time cooling never changes outer wall sheen?".to_string(),
            hint: "Enabling this keeps the outer wall speed identical on every layer, eliminating gloss bands and hull-line steps.".to_string(),
            default_value: "1".to_string(),
            unit: String::new(),
            quick_options: vec![
              ("1".to_string(), "1 — Lock Constant Outer Wall Speed (Recommended)".to_string()),
              ("0".to_string(), "0 — Allow Outer Wall Slowdown".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "wall_order".to_string(),
            question: "2. Which Wall Printing Sequence should be used for maximum Z-wall consistency?".to_string(),
            hint: "'inner-outer-inner wall' isolates the outer wall from internal infill/deck expansion.".to_string(),
            default_value: "inner-outer-inner wall".to_string(),
            unit: String::new(),
            quick_options: vec![
              ("inner-outer-inner wall".to_string(), "inner-outer-inner wall (Best Dimensional Z-Consistency)".to_string()),
              ("inner wall/outer wall".to_string(), "inner wall/outer wall (Standard)".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "sym_corner_bulge".to_string(),
        name: "Swollen / Bulging 90° Corners or Rounded Sharp Edges".to_string(),
        icon: "📐".to_string(),
        category: "4. Walls, Seams & Dimensional (Pressure Advance & Jerk)".to_string(),
        slicer_menu_path: "Relative Pressure Advance Offset + Relative Outer Wall Jerk Scaling".to_string(),
        when_to_run: "Apply when 90° cube corners bulge outward with thicker plastic than the flat walls, preventing box lids or dovetails from sliding together.".to_string(),
        how_to_read_print: vec![
          "1. As the toolhead decelerates into a 90° corner and accelerates out, molten plastic continues oozing under nozzle pressure unless Pressure Advance (PA) relieves pressure ahead of the corner.".to_string(),
          "2. Boosting Pressure Advance by +0.006 to +0.012 relative to your nozzle/filament baseline and tuning `outer_wall_jerk` keeps corners crisp and square.".to_string(),
        ],
        formula_summary: "enable_pressure_advance = 1  •  PA_new = PA_old + PA_Offset  •  Outer_Jerk_new = Outer_Jerk_old × (1 ± Jerk_Adj%)".to_string(),
        default_condition_expr: "all".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "pa_val".to_string(),
            question: "1. How much should we adjust Pressure Advance (pressure_advance) relative to your current baseline?".to_string(),
            hint: "Adding +0.008 relieves corner deceleration pressure earlier so 90° corners stop bulging outward (works across Brass, DiamondBack, and CHT nozzles).".to_string(),
            default_value: "+0.008".to_string(),
            unit: "s".to_string(),
            quick_options: vec![
              ("+0.004".to_string(), "+0.004 (Subtle Corner Tightening)".to_string()),
              ("+0.008".to_string(), "+0.008 (Recommended Anti-Bulge Boost)".to_string()),
              ("+0.014".to_string(), "+0.014 (Strong Boost for High-Elasticity PETG/PCTG/TPU)".to_string()),
              ("-0.006".to_string(), "-0.006 (Reduce PA if Corners Have Pinhole Gaps)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "outer_jerk".to_string(),
            question: "2. How should we scale Outer Wall Jerk (outer_wall_jerk) relative to current?".to_string(),
            hint: "-15% (0.85×) smooths corner ringing; +15% (1.15×) keeps the nozzle moving briskly through corners with less dwell time.".to_string(),
            default_value: "-15%".to_string(),
            unit: "%".to_string(),
            quick_options: vec![
              ("-15%".to_string(), "-15% (0.85× Smooth Corner Turnaround — Recommended)".to_string()),
              ("-25%".to_string(), "-25% (0.75× Gentle Anti-Ringing Corner)".to_string()),
              ("+15%".to_string(), "+15% (1.15× Brisk Corner / Less Dwell)".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "sym_pillowing".to_string(),
        name: "Pillowing (Bumps, Holes or Craters in Top Solid Surface)".to_string(),
        icon: "🕳️".to_string(),
        category: "5. Top/Bottom Surfaces & Infill (Top Shell & Bridging)".to_string(),
        slicer_menu_path: "Process → Strength → Extra Top Shell Layers (+1 to +3) & Sparse Infill Density Boost (+5%)".to_string(),
        when_to_run: "Apply when the flat top roof of a print has pillow-like bumps, pinholes, or sagged craters over the sparse infill pockets.".to_string(),
        how_to_read_print: vec![
          "1. Pillowing happens when the first solid layer bridging across sparse infill sags or curls upward, and there aren't enough subsequent top layers to cover the gaps.".to_string(),
          "2. Adding +2 extra `top_shell_layers` (and scaling `top_shell_thickness` by +2 × layer_height) + boosting `sparse_infill_density` by +5% gives a flat, hole-free top roof on any layer height.".to_string(),
        ],
        formula_summary: "Top_Layers_new = Top_Layers_old + Extra_Layers  •  Top_Thickness += Extra_Layers × Layer_Height  •  Infill_Density += Boost%".to_string(),
        default_condition_expr: "all".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "top_layers".to_string(),
            question: "1. How many EXTRA Top Solid Shell Layers (top_shell_layers) should we add?".to_string(),
            hint: "Adding +2 top layers completely bridges and smooths out sparse infill pockets regardless of your layer height.".to_string(),
            default_value: "+2".to_string(),
            unit: "layers".to_string(),
            quick_options: vec![
              ("+1".to_string(), "+1 Extra Top Layer".to_string()),
              ("+2".to_string(), "+2 Extra Top Layers (Recommended)".to_string()),
              ("+3".to_string(), "+3 Extra Top Layers (Heavy Duty Roof)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "infill_pct".to_string(),
            question: "2. How much should we boost Sparse Infill Density (sparse_infill_density) relative to current to support the top roof?".to_string(),
            hint: "Adding +5% infill density shrinks the span distance that the first top bridge layer has to cross.".to_string(),
            default_value: "+5%".to_string(),
            unit: "%".to_string(),
            quick_options: vec![
              ("+3%".to_string(), "+3% Infill Density Boost (Subtle)".to_string()),
              ("+5%".to_string(), "+5% Infill Density Boost (Recommended)".to_string()),
              ("+10%".to_string(), "+10% Infill Density Boost (Dense Roof Support)".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "sym_infill_wall_gaps".to_string(),
        name: "Pinhole Gaps Between Top Infill and Outer Perimeter Walls".to_string(),
        icon: "🔍".to_string(),
        category: "5. Top/Bottom Surfaces & Infill (Infill-to-Wall Overlap)".to_string(),
        slicer_menu_path: "Process → Strength → Relative Infill/Wall Overlap Boost + Relative Top Surface Speed Scaling + Arachne".to_string(),
        when_to_run: "Apply when diagonal top surface lines pull away slightly from the inner perimeter wall, leaving a ring of tiny pinholes around holes or edges.".to_string(),
        how_to_read_print: vec![
          "1. When the nozzle reverses direction at the end of a diagonal top-infill stroke, elastic melt tension can pull the line slightly away from the perimeter wall if overlap is too small.".to_string(),
          "2. Boosting `infill_wall_overlap` by +8% relative to current, slowing `top_surface_speed` by -25% (0.75×), and enabling Arachne variable-width walls anchors every infill line firmly into the wall.".to_string(),
        ],
        formula_summary: "Overlap_new = Overlap_old + Overlap_Boost%  •  wall_generator = arachne  •  Top_Speed_new = Top_Speed_old × (1 - Slowdown%)".to_string(),
        default_condition_expr: "all".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "overlap_pct".to_string(),
            question: "1. How much should we increase Infill/Wall Overlap percentage (infill_wall_overlap) relative to current?".to_string(),
            hint: "Adding +8% (e.g. 15% → 23%) fuses top/internal infill lines into the inner wall without gaps.".to_string(),
            default_value: "+8%".to_string(),
            unit: "%".to_string(),
            quick_options: vec![
              ("+5%".to_string(), "+5% Overlap Boost (Mild)".to_string()),
              ("+8%".to_string(), "+8% Overlap Boost (Recommended)".to_string()),
              ("+12%".to_string(), "+12% Overlap Boost (Maximum Perimeter Bond)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "top_spd".to_string(),
            question: "2. By what percentage should we slow Top Surface Speed (top_surface_speed) relative to current?".to_string(),
            hint: "Scaling top surface speed by -25% (0.75× of your current top speed) gives top skin lines time to bond cleanly at the perimeter turnaround, scaling properly with high-flow CHT nozzles.".to_string(),
            default_value: "-25%".to_string(),
            unit: "%".to_string(),
            quick_options: vec![
              ("-15%".to_string(), "-15% (0.85× Mild Top Speed Trim)".to_string()),
              ("-25%".to_string(), "-25% (0.75× Recommended Perimeter Bond)".to_string()),
              ("-40%".to_string(), "-40% (0.60× Ultra-Precise Top Skin)".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "sym_top_surface_scars".to_string(),
        name: "Scars, Nozzle Drag Marks & Diagonal Scratches Across Top Surfaces".to_string(),
        icon: "🗡️".to_string(),
        category: "5. Top/Bottom Surfaces & Infill (Travel & Monotonic Top)".to_string(),
        slicer_menu_path: "Monotonic Top Surface + Avoid Crossing Walls + Z-Hop on Top Surface + Ironing".to_string(),
        when_to_run: "Apply when a finished flat top surface has a shiny diagonal drag line where the hot nozzle traveled across it without lifting.".to_string(),
        how_to_read_print: vec![
          "1. Diagonal scars across top surfaces happen when `reduce_infill_retraction` skips retraction/Z-hop on internal moves, allowing the hot nozzle tip to drag across the finished skin.".to_string(),
          "2. Disabling `reduce_infill_retraction`, using `monotonicline` top pattern, and enabling `reduce_crossing_wall = 1` prevents the nozzle from ever dragging across finished top layers.".to_string(),
        ],
        formula_summary: "top_surface_pattern = monotonicline  •  reduce_infill_retraction = 0  •  reduce_crossing_wall = 1".to_string(),
        default_condition_expr: "all".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "top_pattern".to_string(),
            question: "1. Which Top Surface Pattern (top_surface_pattern) should we use?".to_string(),
            hint: "'monotonicline' prints all top lines in the exact same left-to-right overlap direction so there are no visible ridge seams around holes.".to_string(),
            default_value: "monotonicline".to_string(),
            unit: String::new(),
            quick_options: vec![
              ("monotonicline".to_string(), "monotonicline (Uniform Sheen — Recommended)".to_string()),
              ("monotonic".to_string(), "monotonic (Filled Monotonic)".to_string()),
              ("concentric".to_string(), "concentric (Rings)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "ironing_mode".to_string(),
            question: "2. Enable Top Surface Ironing (ironing_type)?".to_string(),
            hint: "Leave off ('no ironing') for PETG/PCTG (which sticks to the nozzle); 'top' works well on flat PLA badges.".to_string(),
            default_value: "no ironing".to_string(),
            unit: String::new(),
            quick_options: vec![
              ("no ironing".to_string(), "no ironing (Recommended for PETG/PCTG/General)".to_string()),
              ("top".to_string(), "top (Iron All Top Surfaces — PLA Only)".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "sym_weak_infill".to_string(),
        name: "Weak, Snapping, Stringy or Under-Extruded Internal Infill".to_string(),
        icon: "🕸️".to_string(),
        category: "5. Top/Bottom Surfaces & Infill (Infill Pattern & Speed)".to_string(),
        slicer_menu_path: "Sparse Infill Pattern (Gyroid/Crosshatch) + Relative Infill Speed Scaling + Relative Infill Width Boost".to_string(),
        when_to_run: "Apply when internal sparse infill looks like broken cobwebs, snaps with a crunching sound while printing, or leaves weak hollow voids.".to_string(),
        how_to_read_print: vec![
          "1. Broken/stringy infill happens when (a) `sparse_infill_speed` exceeds the filament's melt rate, OR (b) Grid/Triangles infill collides with itself at every intersection.".to_string(),
          "2. Switching to `gyroid` or `crosshatch`, scaling `sparse_infill_line_width` up by +12% (1.12×), and scaling `sparse_infill_speed` down by -25% (0.75×) produces rock-solid internal structure on any nozzle or hotend.".to_string(),
        ],
        formula_summary: "sparse_infill_pattern = gyroid/crosshatch  •  Infill_Speed_new = Infill_Speed_old × (1 - Slowdown%)  •  Infill_Width × 1.12".to_string(),
        default_condition_expr: "all".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "pattern".to_string(),
            question: "1. Which Non-Crossing Infill Pattern should we use?".to_string(),
            hint: "Gyroid distributes stress in 3D without ever crossing on the same layer; Crosshatch is fast and non-crossing.".to_string(),
            default_value: "gyroid".to_string(),
            unit: String::new(),
            quick_options: vec![
              ("gyroid".to_string(), "gyroid (Maximum 3D Isotropic Strength)".to_string()),
              ("crosshatch".to_string(), "crosshatch (Fast Non-Crossing)".to_string()),
              ("honeycomb".to_string(), "honeycomb (High Compression Strength)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "infill_spd".to_string(),
            question: "2. By what percentage should we scale down Sparse Infill Speed (sparse_infill_speed) relative to current?".to_string(),
            hint: "Scaling sparse_infill_speed by -25% (0.75× of your current infill speed) ensures every infill line fuses completely while preserving high-flow CHT speed advantages.".to_string(),
            default_value: "-25%".to_string(),
            unit: "%".to_string(),
            quick_options: vec![
              ("-15%".to_string(), "-15% (0.85× Mild Infill Slowdown)".to_string()),
              ("-25%".to_string(), "-25% (0.75× Recommended Structural Fusion)".to_string()),
              ("-35%".to_string(), "-35% (0.65× Maximum Heavy-Duty Infill Bond)".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "sym_poor_bridging".to_string(),
        name: "Sagging, Drooping or Snapped Horizontal Bridges".to_string(),
        icon: "🌉".to_string(),
        category: "6. Overhangs, Bridging & Supports (Bridge Tension & Flow)".to_string(),
        slicer_menu_path: "Relative Bridge Flow Scaling (-5% to -12%) + Relative Bridge Speed Scaling (-20% to -50%) + 100% Bridge Fan".to_string(),
        when_to_run: "Apply when unsupported horizontal spans sag downward in loops, fail to anchor at the far pillar, or look stringy underneath.".to_string(),
        how_to_read_print: vec![
          "1. A good bridge works by stretching a slightly under-extruded strand (`bridge_flow` scaled by 0.88×–0.95×) at a proportionally reduced speed (`bridge_speed` scaled by 0.50×–0.80×) under 100% cooling airflow so tension pulls it taut like a guitar string.".to_string(),
          "2. Because both Bridge Flow and Bridge Speed are applied as relative multipliers (`×`) on your current profile, a MicroSwiss High-Flow CHT or wider nozzle keeps its proportional speed advantage instead of being clamped to a slow fixed mm/s.".to_string(),
        ],
        formula_summary: "Bridge_Flow_new = Bridge_Flow_old × (1 - Flow_Trim%)  •  Bridge_Speed_new = Bridge_Speed_old × (1 - Speed_Trim%)  •  overhang_fan_speed = 100%".to_string(),
        default_condition_expr: "all".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "bridge_flow".to_string(),
            question: "1. By what percentage should we reduce Bridge Flow Ratio (bridge_flow) relative to current to keep bridged strands taut?".to_string(),
            hint: "-8% (0.92× of current bridge_flow) stretches the extruded strand so it doesn't droop under its own weight.".to_string(),
            default_value: "-8%".to_string(),
            unit: "%".to_string(),
            quick_options: vec![
              ("-5%".to_string(), "-5% (0.95× Mild Strand Tension)".to_string()),
              ("-8%".to_string(), "-8% (0.92× Recommended Strand Tension)".to_string()),
              ("-12%".to_string(), "-12% (0.88× High Tension for Long Spans)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "bridge_spd".to_string(),
            question: "2. By what percentage should we scale Bridge Print Speed (bridge_speed) relative to your current baseline?".to_string(),
            hint: "Scaling bridge_speed by -35% (0.65× of your current bridge speed) gives the part cooling fan time to freeze the strand in mid-air while scaling naturally with MicroSwiss CHT / high-flow setups.".to_string(),
            default_value: "-35%".to_string(),
            unit: "%".to_string(),
            quick_options: vec![
              ("-20%".to_string(), "-20% (0.80× Fast High-Flow CHT / PLA Standard)".to_string()),
              ("-35%".to_string(), "-35% (0.65× Recommended Proportional Bridge Slowdown)".to_string()),
              ("-50%".to_string(), "-50% (0.50× Slow Freeze — Sticky PETG/PCTG Long Spans)".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "sym_support_scarring".to_string(),
        name: "Support Interface Scarring or Fused / Hard-to-Remove Supports".to_string(),
        icon: "🌳".to_string(),
        category: "6. Overhangs, Bridging & Supports (Support Z-Distance)".to_string(),
        slicer_menu_path: "Process → Support → Relative Top Z Distance Offset (+0.02 to +0.08mm) + Support Interface Layers + Tree Supports".to_string(),
        when_to_run: "Apply when supports weld permanently onto the part (especially PETG/PCTG), require pliers that break the model, or leave ugly scarred undersides.".to_string(),
        how_to_read_print: vec![
          "1. When supporting a part with the SAME material (especially sticky PETG or PCTG), a tight `support_top_z_distance` fuses the support interface directly to the part.".to_string(),
          "2. Adding +0.04mm to +0.06mm relative to your current `support_top_z_distance` (or `layer_height`) with 3 support interface layers (`support_interface_top_layers = 3`) lets supports pop off cleanly by hand on any nozzle size!".to_string(),
        ],
        formula_summary: "Support_Z_Gap_new = Support_Z_Gap_old + Z_Gap_Offset  •  support_interface_top_layers = 3  •  support_type = tree(auto)".to_string(),
        default_condition_expr: "all".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "top_z_dist".to_string(),
            question: "1. How much extra Support Top/Bottom Z-Distance (support_top_z_distance, mm) should we add between the support roof and the model?".to_string(),
            hint: "+0.04mm relative to your current Z-gap (or layer height) is ideal for sticky PETG/PCTG so supports snap off cleanly across 0.2mm–0.8mm nozzles.".to_string(),
            default_value: "+0.04".to_string(),
            unit: "mm".to_string(),
            quick_options: vec![
              ("+0.02".to_string(), "+0.02 mm Extra Z-Gap (PLA Clean Release)".to_string()),
              ("+0.04".to_string(), "+0.04 mm Extra Z-Gap (Recommended for PETG / PCTG)".to_string()),
              ("+0.08".to_string(), "+0.08 mm Extra Z-Gap (Easy Peel)".to_string()),
              ("0.00".to_string(), "0.00 mm Zero Z-Gap (ONLY for Dissimilar PLA↔PETG Interface!)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "sup_style".to_string(),
            question: "2. Which Support Structure Type (support_type) should be used?".to_string(),
            hint: "tree(auto) branches around the model without resting on lower model surfaces.".to_string(),
            default_value: "tree(auto)".to_string(),
            unit: String::new(),
            quick_options: vec![
              ("tree(auto)".to_string(), "tree(auto) — Organic Tree Supports (Recommended)".to_string()),
              ("normal(auto)".to_string(), "normal(auto) — Snug Grid Supports".to_string()),
            ],
          },
        ],
      },
      ParameterizedCalibrationTemplate {
        id: "sym_ams_color_bleed".to_string(),
        name: "ACE Pro / AMS Color Bleeding, Purge Blobs & Prime Tower Collapse".to_string(),
        icon: "🎨".to_string(),
        category: "7. Structural, Mechanical & Multi-Color (Multi-Material & ACE Pro)".to_string(),
        slicer_menu_path: "Relative Prime Tower Width Boost (+25%) & Prime Volume Boost (+35%) + Cut Retraction".to_string(),
        when_to_run: "Apply when dark colors (Black/Red/CF) bleed into light colors (White/Yellow) after an ACE Pro swap, or the Prime Tower detaches/collapses.".to_string(),
        how_to_read_print: vec![
          "1. Color bleeding after a filament change means residual pigment in the nozzle melt-zone boundary layer wasn't fully swept out before returning to the model.".to_string(),
          "2. Scaling `prime_tower_width` up by +25% (1.25×) and `prime_volume` / `flush_multiplier` up by +35% (1.35×) stabilizes tall prime towers and sweeps dark pigments cleanly on both standard and high-flow melt zones.".to_string(),
        ],
        formula_summary: "Tower_Width_new = Tower_Width_old × (1 + Width_Boost%)  •  Prime_Vol_new = Prime_Vol_old × (1 + Vol_Boost%)".to_string(),
        default_condition_expr: "all".to_string(),
        questions: vec![
          CalibrationQuestion {
            id: "tower_width".to_string(),
            question: "1. By what percentage should we widen Prime Tower Width (prime_tower_width) relative to current for stability?".to_string(),
            hint: "Scaling prime_tower_width by +25% (1.25×) prevents tall multi-color prime towers from wobbling or getting knocked over.".to_string(),
            default_value: "+25%".to_string(),
            unit: "%".to_string(),
            quick_options: vec![
              ("+15%".to_string(), "+15% (1.15× Moderate Tower Widening)".to_string()),
              ("+25%".to_string(), "+25% (1.25× Recommended Stable Tower)".to_string()),
              ("+45%".to_string(), "+45% (1.45× Tall Multi-Color Prints)".to_string()),
            ],
          },
          CalibrationQuestion {
            id: "prime_vol".to_string(),
            question: "2. By what percentage should we boost Prime Tower Volume (prime_volume) per swap relative to current?".to_string(),
            hint: "Scaling prime_volume by +35% (1.35×) stabilizes nozzle pressure and purges residual boundary-layer pigment after the poop-chute flush.".to_string(),
            default_value: "+35%".to_string(),
            unit: "%".to_string(),
            quick_options: vec![
              ("+20%".to_string(), "+20% (1.20× Mild Purge Boost)".to_string()),
              ("+35%".to_string(), "+35% (1.35× Recommended Anti-Bleed)".to_string()),
              ("+60%".to_string(), "+60% (1.60× High-Contrast Dark→White / CHT Melt Zone)".to_string()),
            ],
          },
        ],
      },
    ]
  }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CalibrationQuestion {
  pub id: String,
  pub question: String,
  pub hint: String,
  pub default_value: String,
  pub unit: String,
  pub quick_options: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterizedCalibrationTemplate {
  pub id: String,
  pub name: String,
  pub icon: String,
  pub category: String,
  pub slicer_menu_path: String,
  pub when_to_run: String,
  pub how_to_read_print: Vec<String>,
  pub formula_summary: String,
  pub default_condition_expr: String,
  pub questions: Vec<CalibrationQuestion>,
}

impl ParameterizedCalibrationTemplate {
  pub fn is_symptom_wizard(&self) -> bool {
    self.id.starts_with("sym_")
  }

  /// Returns the 7 top-level categories in the Print Quality & Troubleshooting Category Tree:
  /// `(category_id, display_label, icon)`
  pub fn symptom_tree_categories() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
      ("all", "All Print Quality Issues (26)", "🗂️"),
      ("1. Extrusion & Flow", "1. Extrusion & Flow", "🔥"),
      (
        "2. Oozing, Stringing & Blobs",
        "2. Oozing, Stringing & Blobs",
        "🕸️",
      ),
      (
        "3. First Layer & Bed Adhesion",
        "3. First Layer & Bed Adhesion",
        "🧲",
      ),
      (
        "4. Walls, Seams & Dimensional",
        "4. Walls, Seams & Dimensional",
        "🪡",
      ),
      (
        "5. Top/Bottom Surfaces & Infill",
        "5. Top/Bottom Surfaces & Infill",
        "🧱",
      ),
      (
        "6. Overhangs, Bridging & Supports",
        "6. Overhangs, Bridging & Supports",
        "🌀",
      ),
      (
        "7. Structural, Mechanical & Multi-Color",
        "7. Structural, Mechanical & AMS",
        "⚙️",
      ),
    ]
  }

  pub fn tree_category(&self) -> &'static str {
    match self.id.as_str() {
      "sym_under_extrusion"
      | "sym_over_extrusion"
      | "sym_clogging"
      | "sym_extruder_grinding"
      | "sym_wet_filament" => "1. Extrusion & Flow",
      "sym_stringing" | "sym_boogers" | "sym_zits_blobs" => "2. Oozing, Stringing & Blobs",
      "sym_not_sticking"
      | "sym_warping_adhesion"
      | "sym_elephants_foot"
      | "sym_first_layer_ripples" => "3. First Layer & Bed Adhesion",
      "sym_visible_seam"
      | "sym_ringing_vfa"
      | "sym_layer_shifting"
      | "sym_z_banding_hull_line"
      | "sym_corner_bulge" => "4. Walls, Seams & Dimensional",
      "sym_pillowing" | "sym_infill_wall_gaps" | "sym_top_surface_scars" | "sym_weak_infill" => {
        "5. Top/Bottom Surfaces & Infill"
      }
      "sym_overhang_curl" | "sym_poor_bridging" | "sym_support_scarring" => {
        "6. Overhangs, Bridging & Supports"
      }
      "sym_weak_layers" | "sym_ams_color_bleed" => "7. Structural, Mechanical & Multi-Color",
      _ => "Post-Print Calibration",
    }
  }

  pub fn tree_subcategory(&self) -> &'static str {
    match self.id.as_str() {
      "sym_under_extrusion" | "sym_over_extrusion" => "Flow & Volume",
      "sym_clogging" => "Hotend & Throat",
      "sym_extruder_grinding" => "Extruder Drive & Tension Screw",
      "sym_wet_filament" => "Moisture & Active Drying",
      "sym_stringing" => "Travel Retraction & Z-Hop",
      "sym_boogers" => "Nozzle Tip Accumulation",
      "sym_zits_blobs" => "Outer Wall Blemishes",
      "sym_not_sticking" => "Bed Cleaning & First-Layer Squish",
      "sym_warping_adhesion" => "Thermal Shrinkage & Brim",
      "sym_elephants_foot" => "Bottom Lip Inset",
      "sym_first_layer_ripples" => "Z-Offset & Plowing Waves",
      "sym_visible_seam" => "Scarf Joint & Loop Closure",
      "sym_ringing_vfa" => "Input Shaping & Belt Resonance",
      "sym_layer_shifting" => "Stepper Torque & Collision Clearance",
      "sym_z_banding_hull_line" => "Z-Lead Screw & Layer-Time Shrinkage",
      "sym_corner_bulge" => "Pressure Advance & Corner Jerk",
      "sym_pillowing" => "Top Shell Roof & Bridging",
      "sym_infill_wall_gaps" => "Perimeter-to-Infill Overlap",
      "sym_top_surface_scars" => "Monotonic Skin & Travel Lift",
      "sym_weak_infill" => "Non-Crossing Infill & Flow",
      "sym_overhang_curl" => "Steep Overhang Cooling",
      "sym_poor_bridging" => "Bridge Strand Tension",
      "sym_support_scarring" => "Support Top Z-Gap & Interface",
      "sym_weak_layers" => "Inter-Layer Polymer Weld",
      "sym_ams_color_bleed" => "ACE Pro Purge & Prime Tower",
      _ => "Calibration Test",
    }
  }

  /// Concrete physical/mechanical checks on the Anycubic Kobra S1 + ACE Pro (and general CoreXY hardware)
  /// to perform alongside the slicer Lua adjustment.
  pub fn hardware_checklist(&self) -> Vec<&'static str> {
    match self.id.as_str() {
      "sym_extruder_grinding" => vec![
        "🔧 Anycubic Kobra S1 Extruder Tension Screw: Locate the recessed 2.0mm hex idler tension screw on the RIGHT side of the printhead cover. Turn CLOCKWISE (CW) 1/4 to 1/2 turn if gears slip on PLA/PETG/PCTG, or COUNTER-CLOCKWISE (CCW) 1/2 turn for soft TPU so the gears don't crush the filament.",
        "🪥 Clean Hobbed Drive Gears: Pop off the magnetic front toolhead cover and use a stiff nylon/brass brush or compressed air to clear shaved plastic dust packed inside the dual extruder teeth.",
        "🧵 Check Spool & ACE Pro Buffer Friction: Verify the filament isn't cross-over tangled on the spool and that the PTFE bowden tube entering the top of the Kobra S1 printhead has a smooth bend radius.",
      ],
      "sym_clogging" => vec![
        "🧲 Cold Pull (Atomic Pull) Procedure: Heat nozzle to 250°C, push 20mm of natural PLA or Nylon through manually, cool to 95°C (PLA) or 130°C (PETG/PCTG), release the extruder tension lever, and pull firmly straight up to extract carbonized debris.",
        "📍 0.35mm Acupuncture Needle: Heat nozzle to printing temp and gently insert the cleaning needle 15mm up the nozzle orifice to break up bridged carbon-fiber (-CF) clumps.",
        "🚪 Open Door/Lid for PLA Heat Creep: When printing PLA/PLA+ with bed ≥ 60°C, crack the Kobra S1 top glass cover or front door open so chamber temperature stays below 38°C.",
      ],
      "sym_under_extrusion" => vec![
        "📏 Check Extruder Tension & Gear Teeth: If the Kobra S1 extruder idler screw on the right side of the toolhead is too loose, the drive gears slip under back-pressure — tighten 1/4 turn clockwise.",
        "🌡️ Verify Nozzle Silicone Sock & Thermistor: Make sure the silicone sock is seated around the heater block so part-cooling air doesn't chill the nozzle tip.",
      ],
      "sym_over_extrusion" => vec![
        "📐 Measure Filament Diameter with Calipers: Check 3 points along your spool (should be 1.75mm ± 0.02mm) and run the YOLO Flow Rate chip calibration test if the spool runs thick.",
      ],
      "sym_wet_filament" => vec![
        "🔥 Run ACE Pro Active Filament Dryer: Enable ACE Pro drying before printing — PLA/PLA+: 45°C for 4h • PETG/PCTG/TPU: 55°C for 6h • ABS/ASA: 60°C for 6h.",
        "👂 Listen at the Nozzle Tip: Steam moisture makes an unmistakable 'pop / crackle / hiss' sound and puffs tiny steam clouds out of the 0.4mm orifice.",
      ],
      "sym_stringing" => vec![
        "🔥 Dry Filament First in ACE Pro: Even perfect retraction settings cannot stop stringing if absorbed moisture is boiling into steam pressure inside the melt zone (dry PETG/PCTG at 55°C for 4–6h).",
        "🧽 Wipe Nozzle Exterior: Heat nozzle to 220°C and wipe any coated polymer film off the outside of the brass/steel tip with a folded paper towel or brass brush.",
      ],
      "sym_boogers" => vec![
        "🧦 Check Silicone Sock & Nozzle Tip: A torn silicone sock or plastic crusted under the sock edge drips burnt brown boogers onto light-colored prints — pull off the sock when cool and peel away any crust.",
        "💎 Consider DiamondBack / Coated Nozzle for PETG/PCTG: Polycrystalline diamond (PCD) and WS2 non-stick nozzles have much lower surface energy than bare steel, preventing PETG/PCTG from climbing the nozzle cone.",
      ],
      "sym_zits_blobs" => vec![
        "⚡ Check Power-Loss Recovery (PLR) Pause Blobs: If zits appear in a spiral pattern on Vase Mode or curved cylinders, firmware SD-card state saving is briefly pausing the toolhead at each layer.",
        "🔥 Check for Moisture Popping: Random crater-like zits halfway along a wall (away from the Z-seam) are burst steam bubbles from wet filament.",
      ],
      "sym_not_sticking" => vec![
        "🧼 Wash Textured PEI Plate with Warm Dish Soap (Not Just IPA!): Isopropyl alcohol (IPA) only spreads finger oils and PLA/PETG sugar residues thin across PEI. Wash at the sink with warm water + Dawn dish soap and a clean sponge.",
        "🔍 Inspect Nozzle Tip Before Auto-Leveling: If a tiny hardened plastic droplet is stuck to the nozzle tip during LeviQ bed probing, the printer will calibrate Z-offset +0.10mm too high in the air!",
      ],
      "sym_warping_adhesion" => vec![
        "🚪 Close Kobra S1 Door & Top Lid for PETG/PCTG/ASA/ABS: Cold room drafts or AC vents blowing across the print bed cause rapid thermal contraction that peels corners upward.",
        "🧼 Scrub Corners of Textured PEI Plate: We habitually grab the front corners of the spring-steel plate with bare thumbs — scrub the plate edges with dish soap.",
      ],
      "sym_elephants_foot" => vec![
        "📏 Re-Run Auto Bed Leveling with Clean Nozzle: If the first layer is squished paper-thin and translucent, the Z-offset is slightly too close to the PEI plate.",
      ],
      "sym_first_layer_ripples" => vec![
        "⬆️ Nudge Z-Offset Up +0.02mm to +0.04mm on Touchscreen: Diagonal 'tiger-stripe' waves on Layer 1 mean the nozzle is plowing too close to the plate, forcing excess plastic sideways until it hops over a ridge.",
      ],
      "sym_visible_seam" => vec![
        "📐 Calibrate Pressure Advance (PA) First: Even with a Scarf Joint Seam enabled, an uncalibrated PA leaves a bulge (PA too low) or pinhole (PA too high) at the seam ramp start.",
      ],
      "sym_ringing_vfa" => vec![
        "🎸 Tension Kobra S1 CoreXY Belts & Run Input Shaper: Check the two belt tensioners on the rear of the Kobra S1 gantry so both X/Y belts have equal tension, then run 'Vibration Compensation (Input Shaping)' from the printer setup menu.",
        "🪨 Place Printer on a Stable Surface: A wobbly table amplifies gantry jerk into ringing echoes.",
      ],
      "sym_layer_shifting" => vec![
        "🔩 Check Motor Pulley Grub Screws (Set Screws): Inspect the A/B CoreXY stepper pulleys to ensure the small hex grub screws are tight against the flat of the motor D-shaft.",
        "🎸 Check Belt Teeth & Idler Bearings: Ensure no stray filament poop or debris from the purge chute has fallen into the Y-axis rails or belt teeth.",
      ],
      "sym_z_banding_hull_line" => vec![
        "🧴 Clean & Lubricate the 3 Z-Axis Lead Screws: Wipe old dark grease off the Kobra S1 Z-axis lead screws and guide rods with a lint-free cloth and apply a light coat of PTFE/lithium grease.",
      ],
      "sym_corner_bulge" => vec![
        "📐 Print the PA Pattern Test (`Calibration → Pressure Advance`): Direct visual verification of the 90° chevron corner gives the exact PA value for your nozzle + filament.",
      ],
      "sym_pillowing" => vec![
        "🌀 Verify Part Cooling Fan Duct Airflow: Ensure both outlets of the toolhead part-cooling fan duct are clear of strings and blowing directly below the nozzle tip.",
      ],
      "sym_infill_wall_gaps" => vec![
        "🎸 Check CoreXY Belt Backlash: If gaps appear only on Two opposite diagonal quadrants of a circle, one of the two CoreXY belts has slack/hysteresis.",
      ],
      "sym_top_surface_scars" => vec![
        "📐 Verify Bed Level & Extruder Flow: If the top layer is slightly over-extruded (+3%), the raised ridges sit higher than the nozzle tip during travel moves.",
      ],
      "sym_weak_infill" => vec![
        "🔧 Check Kobra S1 Extruder Idler Tension: If sparse infill looks like thin cobwebs while slow outer walls look fine, the extruder gears may be slipping at high volumetric flow — turn the right-side extruder tension screw 1/4 turn clockwise.",
      ],
      "sym_overhang_curl" => vec![
        "🌀 Orient Steep Overhangs Toward the Part Cooling Fan: Rotate the model on the build plate so the steepest overhang faces the strongest side of the toolhead cooling duct.",
      ],
      "sym_poor_bridging" => vec![
        "🧭 Check Bridge Angle Orientation: In Slicer Preview, verify that the bridge lines anchor across the SHORTEST distance between support walls rather than diagonally.",
      ],
      "sym_support_scarring" => vec![
        "💡 Pro Tip — Dissimilar Support Interface (PLA ↔ PETG): With an ACE Pro, you can assign PLA as the Support Interface filament for a PETG/PCTG print (or vice versa) with `support_top_z_distance = 0.00mm` — they don't bond to each other and peel off with a mirror-smooth underside!",
      ],
      "sym_weak_layers" => vec![
        "🚪 Keep Chamber Closed for Structural Prints: Blocking cool room air drafts raises ambient chamber temperature by 10–15°C, dramatically reducing internal residual stress.",
      ],
      "sym_ams_color_bleed" => vec![
        "💩 Inspect the Rear Purge Chute ('Poop Chute') Flap: Ensure purged filament coils drop freely into the chute and aren't getting dragged back onto the build plate by the nozzle wiper.",
        "🔢 Review the N×N Flushing Matrix in the Filament Tab: Increase the Flushing Multiplier to 1.15×–1.25× when swapping from Black/Red/CF to White/Clear.",
      ],
      _ => vec![
        "🖨️ Slice and print the built-in test model from the AnycubicSlicerNext / OrcaSlicer top 'Calibration' menu before entering your measurements below.",
      ],
    }
  }

  pub fn default_answers(&self) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for q in &self.questions {
      map.insert(q.id.clone(), q.default_value.clone());
    }
    map
  }

  /// Generates `(rule_title, rule_description, computed_summary_badge, lua_code)` from the user's answers.
  pub fn generate_from_answers(
    &self,
    answers: &HashMap<String, String>,
    scope: &str,
  ) -> (String, String, String, String) {
    let get_ans = |id: &str, fallback: &str| -> String {
      answers
        .get(id)
        .cloned()
        .unwrap_or_else(|| fallback.to_string())
    };

    match self.id.as_str() {
      "cal_yolo_flow" => {
        let mode = get_ans("mode", "YOLO");
        let old_flow: f64 = get_ans("old_flow", "0.96").parse().unwrap_or(0.96);
        let chip_str = get_ans("chip_val", "-0.02");
        let chip_val: f64 = chip_str.trim_start_matches('+').parse().unwrap_or(-0.02);
        let new_flow = if mode == "Classic" {
          (old_flow * (100.0 + chip_val) / 100.0).clamp(0.75, 1.25)
        } else {
          (old_flow + chip_val).clamp(0.75, 1.25)
        };
        let badge = if mode == "Classic" {
          format!(
            "Flow_new = {:.3} × (100 {:+.1}%) / 100 = {:.3}",
            old_flow, chip_val, new_flow
          )
        } else {
          format!(
            "Flow_new = {:.3} (Old) {:+.3} (YOLO Chip) = {:.3}",
            old_flow, chip_val, new_flow
          )
        };
        let title = format!(
          "Calibration: {} Flow Rate ({:.3} → {:.3})",
          mode, old_flow, new_flow
        );
        let desc = format!(
          "{} Flow Rate Calibration for {}: starting flow {:.3} with smoothest chip {} → new filament_flow_ratio = {:.3}.",
          mode, scope, old_flow, chip_str, new_flow
        );
        let lua = format!(
          "-- {title}\n-- Formula: {badge}\nfunction apply(config, ctx)\n  config[\"filament_flow_ratio\"] = \"{new_flow:.3}\"\n  print(\"[Calibration:FlowRate] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "cal_pressure_advance" => {
        let pa_str = get_ans("pa_val", "0.048");
        let pa_num: f64 = pa_str.parse::<f64>().unwrap_or(0.048).clamp(0.0, 0.25);
        let adaptive = get_ans("adaptive_pa", "0");
        let badge = format!(
          "pressure_advance = {:.3} (Adaptive PA: {})",
          pa_num,
          if adaptive == "1" { "On" } else { "Off" }
        );
        let title = format!("Calibration: Pressure Advance = {:.3}", pa_num);
        let desc = format!(
          "Post-print PA Pattern/Line calibration for {}: sets pressure_advance = {:.3} (enable_pressure_advance = 1, adaptive = {}).",
          scope, pa_num, adaptive
        );
        let lua = format!(
          "-- {title}\nfunction apply(config, ctx)\n  config[\"enable_pressure_advance\"] = \"1\"\n  config[\"pressure_advance\"] = \"{pa_num:.3}\"\n  config[\"adaptive_pressure_advance\"] = \"{adaptive}\"\n  print(\"[Calibration:PA] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "cal_temp_tower" => {
        let best_temp: i32 = get_ans("best_temp", "255").parse().unwrap_or(255);
        let boost: i32 = get_ans("first_layer_boost", "5")
          .trim_start_matches('+')
          .parse()
          .unwrap_or(5);
        let init_temp = (best_temp + boost).clamp(180, 350);
        let badge = format!(
          "Nozzle = {}°C Continuous / {}°C First Layer ({:+}°C)",
          best_temp, init_temp, boost
        );
        let title = format!(
          "Calibration: Temp Tower {}°C ({}°C Layer 1)",
          best_temp, init_temp
        );
        let desc = format!(
          "Post-print Temperature Tower calibration for {}: sets nozzle_temperature = {}°C and initial layer = {}°C.",
          scope, best_temp, init_temp
        );
        let lua = format!(
          "-- {title}\nfunction apply(config, ctx)\n  config[\"nozzle_temperature\"] = \"{best_temp}\"\n  config[\"nozzle_temperature_initial_layer\"] = \"{init_temp}\"\n  print(\"[Calibration:TempTower] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "cal_retraction_tower" => {
        let len_num: f64 = get_ans("retract_len", "0.95").parse().unwrap_or(0.95);
        let spd_num: i32 = get_ans("retract_speed", "45").parse().unwrap_or(45);
        let deretract_spd = (spd_num - 15).max(25);
        let z_hop = get_ans("z_hop_style", "Spiral");
        let rbw = get_ans("retract_before_wipe", "70%");
        let badge = format!(
          "{:.2}mm @ {}mm/s (Deretract {}mm/s) • {} Z-Hop • {} Pre-Wipe",
          len_num, spd_num, deretract_spd, z_hop, rbw
        );
        let title = format!(
          "Calibration: Retraction {:.2}mm @ {}mm/s ({})",
          len_num, spd_num, z_hop
        );
        let desc = format!(
          "Post-print Retraction Tower calibration for {}: {:.2}mm @ {}mm/s, {}mm/s deretract, {} Z-Hop, and {} retract-before-wipe.",
          scope, len_num, spd_num, deretract_spd, z_hop, rbw
        );
        let lua = format!(
          "-- {title}\nfunction apply(config, ctx)\n  config[\"filament_retraction_length\"] = \"{len_num:.2}\"\n  config[\"filament_retraction_speed\"] = \"{spd_num}\"\n  config[\"filament_deretraction_speed\"] = \"{deretract_spd}\"\n  config[\"filament_z_hop\"] = \"0.15\"\n  config[\"filament_z_hop_types\"] = \"{z_hop}\"\n  config[\"filament_wipe\"] = \"1\"\n  config[\"filament_wipe_distance\"] = \"2.0\"\n  config[\"filament_retract_before_wipe\"] = \"{rbw}\"\n  print(\"[Calibration:Retraction] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "cal_max_volumetric_speed" => {
        let start: f64 = get_ans("start_flow", "8.0").parse().unwrap_or(8.0);
        let step: f64 = get_ans("step_per_mm", "0.50").parse().unwrap_or(0.50);
        let z_mm: f64 = get_ans("measured_z", "14.0").parse().unwrap_or(14.0);
        let margin: f64 = get_ans("margin_pct", "10").parse().unwrap_or(10.0);
        let peak_flow = start + (z_mm * step);
        let safe_mvs = (peak_flow * (1.0 - margin / 100.0)).clamp(2.0, 50.0);
        let badge = format!(
          "Peak = {:.1} + ({:.1}mm × {:.2}) = {:.1} mm³/s → Safe MVS (-{:.0}%) = {:.1} mm³/s",
          start, z_mm, step, peak_flow, margin, safe_mvs
        );
        let title = format!("Calibration: Max Volumetric Speed = {:.1} mm³/s", safe_mvs);
        let desc = format!(
          "Post-print MVS Race Track calibration for {}: failure at Z={:.1}mm ({:.1} mm³/s peak) with {:.0}% margin → {:.1} mm³/s.",
          scope, z_mm, peak_flow, margin, safe_mvs
        );
        let lua = format!(
          "-- {title}\n-- {badge}\nfunction apply(config, ctx)\n  config[\"filament_max_volumetric_speed\"] = \"{safe_mvs:.1}\"\n  print(\"[Calibration:MVS] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "cal_dimensional_xy" => {
        let outer: f64 = get_ans("measured_outer", "20.04").parse().unwrap_or(20.04);
        let hole: f64 = get_ans("measured_hole", "9.84").parse().unwrap_or(9.84);
        let contour_comp = ((20.00 - outer) / 2.0).clamp(-0.5, 0.5);
        let hole_comp = ((10.00 - hole) / 2.0).clamp(-0.5, 0.5);
        let badge = format!(
          "Contour Comp = {:+.3} mm (from {:.2}mm cube) • Hole Comp = {:+.3} mm (from {:.2}mm bore)",
          contour_comp, outer, hole_comp, hole
        );
        let title = format!(
          "Calibration: X-Y Contour ({:+.2}mm) & Hole ({:+.2}mm) Comp",
          contour_comp, hole_comp
        );
        let desc = format!(
          "Caliper dimensional compensation: 20mm cube measured {:.2}mm (xy_contour_compensation = {:+.3}mm), 10mm bore measured {:.2}mm (xy_hole_compensation = {:+.3}mm).",
          outer, contour_comp, hole, hole_comp
        );
        let lua = format!(
          "-- {title}\n-- {badge}\nfunction apply(config, ctx)\n  config[\"xy_contour_compensation\"] = \"{contour_comp:.3}\"\n  config[\"xy_hole_compensation\"] = \"{hole_comp:.3}\"\n  print(\"[Calibration:DimensionalXY] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "sym_stringing" => {
        let temp_drop: i32 = get_ans("temp_drop", "-3")
          .trim_start_matches('+')
          .parse()
          .unwrap_or(-3);
        let retract_boost: f64 = get_ans("retract_boost", "+0.15")
          .trim_start_matches('+')
          .parse()
          .unwrap_or(0.15);
        let z_hop = get_ans("z_hop_style", "Spiral");
        let pre_wipe = get_ans("pre_wipe", "70%");
        let zh_mult = match z_hop.as_str() {
          "Slope" => 0.90,
          "Normal" => 1.00,
          _ => 0.75,
        };
        let badge = format!(
          "Nozzle {:+}°C • Retract {:+.2}mm • Deretract ×0.75 • {} Z-Hop ({:.2}× LH) • {} Pre-Wipe",
          temp_drop, retract_boost, z_hop, zh_mult, pre_wipe
        );
        let title = format!(
          "Symptom Fix: Stringing ({:+}°C, {:+.2}mm, {} Z-Hop)",
          temp_drop, retract_boost, z_hop
        );
        let desc = format!(
          "Relative anti-stringing adjustment for {}: nozzle {:+}°C, retraction {:+.2}mm, deretract ×0.75, {} Z-Hop ({:.2}× layer_height), and {} retract-before-wipe.",
          scope, temp_drop, retract_boost, z_hop, zh_mult, pre_wipe
        );
        let lua = format!(
          "-- {title}\n-- {badge}\nfunction apply(config, ctx)\n  local cur_t = tonumber(config[\"nozzle_temperature\"]) or 255\n  config[\"nozzle_temperature\"] = tostring(cur_t + ({temp_drop}))\n  local cur_r = tonumber(config[\"filament_retraction_length\"]) or 0.80\n  config[\"filament_retraction_length\"] = string.format(\"%.2f\", cur_r + ({retract_boost:.2}))\n  local cur_d = tonumber(config[\"filament_deretraction_speed\"]) or 40\n  config[\"filament_deretraction_speed\"] = tostring(math.max(15, math.floor(cur_d * 0.75 + 0.5)))\n  local cur_lh = tonumber(config[\"layer_height\"]) or 0.20\n  config[\"filament_z_hop\"] = string.format(\"%.2f\", cur_lh * {zh_mult:.2})\n  config[\"filament_z_hop_types\"] = \"{z_hop}\"\n  config[\"filament_wipe\"] = \"1\"\n  local noz = tonumber(ctx.nozzle_diameter) or 0.40\n  config[\"filament_wipe_distance\"] = string.format(\"%.1f\", noz * 5.0)\n  config[\"filament_retract_before_wipe\"] = \"{pre_wipe}\"\n  print(\"[Symptom:Stringing] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "sym_boogers" => {
        let flow_trim: f64 = get_ans("flow_trim", "-0.02")
          .trim_start_matches('+')
          .parse()
          .unwrap_or(-0.02);
        let deret_pct: f64 = get_ans("deretract_spd", "-25%")
          .trim_end_matches('%')
          .trim_start_matches('+')
          .parse()
          .unwrap_or(-25.0);
        let d_scale = if deret_pct < 0.0 {
          ((100.0 + deret_pct) / 100.0).clamp(0.40, 1.0)
        } else {
          ((100.0 - deret_pct) / 100.0).clamp(0.40, 1.0)
        };
        let drop_label = ((1.0 - d_scale) * 100.0).round();
        let infill = get_ans("infill_pattern", "gyroid");
        let seam_gap = get_ans("seam_gap", "10%");
        let badge = format!(
          "Flow {:+.2} • Deretract -{:.0}% ({:.2}×) • Infill={} • Seam Gap {}",
          flow_trim, drop_label, d_scale, infill, seam_gap
        );
        let title = format!(
          "Symptom Fix: Boogers & Blobs (Flow {:+.2}, Deretract -{:.0}%)",
          flow_trim, drop_label
        );
        let desc = format!(
          "Eliminates nozzle tip boogers and seam blobs for {}: trims flow by {:+.2}, scales deretraction speed by {:.2}× (-{:.0}%), sets {} seam gap, and uses non-crossing infill ({}).",
          scope, flow_trim, d_scale, drop_label, seam_gap, infill
        );
        let infill_line = if infill == "keep" {
          String::new()
        } else {
          format!("  config[\"sparse_infill_pattern\"] = \"{}\"\n", infill)
        };
        let lua = format!(
          "-- {title}\n-- {badge}\nfunction apply(config, ctx)\n  local cur_f = tonumber(config[\"filament_flow_ratio\"]) or 0.96\n  config[\"filament_flow_ratio\"] = string.format(\"%.3f\", cur_f + ({flow_trim:.3}))\n  local cur_d = tonumber(config[\"filament_deretraction_speed\"]) or 40\n  config[\"filament_deretraction_speed\"] = tostring(math.max(15, math.floor(cur_d * {d_scale:.2} + 0.5)))\n  config[\"filament_retract_before_wipe\"] = \"80%\"\n  config[\"seam_gap\"] = \"{seam_gap}\"\n  config[\"wipe_on_loops\"] = \"1\"\n  config[\"reduce_infill_retraction\"] = \"0\"\n{infill_line}  print(\"[Symptom:Boogers] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "sym_clogging" => {
        let clog_mode = get_ans("clog_mode", "heat_creep");
        let temp_adj: i32 = get_ans("temp_adj", "+5")
          .trim_start_matches('+')
          .parse()
          .unwrap_or(5);
        let ret_pct: f64 = get_ans("max_retract", "-20%")
          .trim_end_matches('%')
          .trim_start_matches('+')
          .parse()
          .unwrap_or(-20.0);
        let ret_scale = if ret_pct < 0.0 {
          ((100.0 + ret_pct) / 100.0).clamp(0.50, 1.0)
        } else {
          ((100.0 - ret_pct) / 100.0).clamp(0.50, 1.0)
        };
        let ret_drop_label = ((1.0 - ret_scale) * 100.0).round();
        let mvs_drop: f64 = get_ans("mvs_drop_pct", "15")
          .trim_end_matches('%')
          .parse::<f64>()
          .unwrap_or(15.0)
          .abs();
        let scale = ((100.0 - mvs_drop) / 100.0).clamp(0.5, 1.0);
        let badge = format!(
          "Mode={} • Nozzle {:+}°C • Retract -{:.0}% ({:.2}×) • MVS -{:.0}% ({:.2}×)",
          clog_mode, temp_adj, ret_drop_label, ret_scale, mvs_drop, scale
        );
        let title = format!(
          "Symptom Fix: Anti-Clog ({:+}°C, -{:.0}% Retract, -{:.0}% MVS)",
          temp_adj, ret_drop_label, mvs_drop
        );
        let desc = format!(
          "Prevents clogging & extruder skipping ({}) for {}: nozzle {:+}°C, scales retraction by {:.2}× (-{:.0}%), scales MVS by {:.2}× (-{:.0}%), and trims AMS cut retraction.",
          clog_mode, scope, temp_adj, ret_scale, ret_drop_label, scale, mvs_drop
        );
        let lua = format!(
          "-- {title}\n-- {badge}\nfunction apply(config, ctx)\n  local cur_t = tonumber(config[\"nozzle_temperature\"]) or 230\n  local new_t = cur_t + ({temp_adj})\n  config[\"nozzle_temperature\"] = tostring(new_t)\n  local cur_r = tonumber(config[\"filament_retraction_length\"]) or 0.80\n  config[\"filament_retraction_length\"] = string.format(\"%.2f\", cur_r * {ret_scale:.2})\n  local cur_mvs = tonumber(config[\"filament_max_volumetric_speed\"]) or 15.0\n  config[\"filament_max_volumetric_speed\"] = string.format(\"%.1f\", cur_mvs * {scale:.2})\n  local cur_cut = tonumber(config[\"retraction_distances_when_cut\"]) or 14.0\n  config[\"retraction_distances_when_cut\"] = string.format(\"%.1f\", math.min(12.0, cur_cut * 0.85))\n  config[\"flush_temperature\"] = tostring(new_t)\n  print(\"[Symptom:AntiClog] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "sym_warping_adhesion" => {
        let bed_boost: i32 = get_ans("bed_boost", "+5")
          .trim_start_matches('+')
          .parse()
          .unwrap_or(5);
        let spd_pct: f64 = get_ans("first_layer_spd", "-30%")
          .trim_end_matches('%')
          .trim_start_matches('+')
          .parse()
          .unwrap_or(-30.0);
        let spd_scale = if spd_pct < 0.0 {
          ((100.0 + spd_pct) / 100.0).clamp(0.35, 1.0)
        } else {
          ((100.0 - spd_pct) / 100.0).clamp(0.35, 1.0)
        };
        let spd_drop_label = ((1.0 - spd_scale) * 100.0).round();
        let extra_fan_off: i32 = get_ans("fan_off_layers", "+2")
          .trim_start_matches('+')
          .parse()
          .unwrap_or(2);
        let brim = get_ans("brim_type", "outer_only");
        let badge = format!(
          "Bed {:+}°C • Layer 1 Speed -{:.0}% ({:.2}×) • Fan Off {:+} Layers • Brim={}",
          bed_boost, spd_drop_label, spd_scale, extra_fan_off, brim
        );
        let title = format!(
          "Symptom Fix: Bed Adhesion & Anti-Warp (Bed {:+}°C, -{:.0}% L1 Speed)",
          bed_boost, spd_drop_label
        );
        let desc = format!(
          "Eliminates corner warping and first-layer peeling for {}: bed {:+}°C, scales first-layer speed by {:.2}× (-{:.0}%), adds {:+} fan-off layers, and brim_type = {}.",
          scope, bed_boost, spd_scale, spd_drop_label, extra_fan_off, brim
        );
        let lua = format!(
          "-- {title}\n-- {badge}\nfunction apply(config, ctx)\n  local cur_b = tonumber(config[\"textured_pei_plate_temp\"]) or 65\n  config[\"textured_pei_plate_temp\"] = tostring(cur_b + ({bed_boost}))\n  config[\"textured_pei_plate_temp_initial_layer\"] = tostring(cur_b + ({bed_boost}))\n  local cur_fl_s = tonumber(config[\"initial_layer_speed\"]) or 50\n  config[\"initial_layer_speed\"] = tostring(math.max(15, math.floor(cur_fl_s * {spd_scale:.2} + 0.5)))\n  local cur_fl_is = tonumber(config[\"initial_layer_infill_speed\"]) or cur_fl_s\n  config[\"initial_layer_infill_speed\"] = tostring(math.max(15, math.floor(cur_fl_is * {spd_scale:.2} + 0.5)))\n  local cur_fo = tonumber(config[\"close_fan_the_first_x_layers\"]) or 1\n  config[\"close_fan_the_first_x_layers\"] = tostring(math.max(1, math.floor(cur_fo + ({extra_fan_off}))))\n  config[\"brim_type\"] = \"{brim}\"\n  print(\"[Symptom:Adhesion] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "sym_overhang_curl" => {
        let ov_boost: i32 = get_ans("overhang_fan", "+20%")
          .trim_end_matches('%')
          .trim_start_matches('+')
          .parse()
          .unwrap_or(20);
        let l_time_boost: i32 = get_ans("layer_time", "+4")
          .trim_end_matches('s')
          .trim_start_matches('+')
          .parse()
          .unwrap_or(4);
        let w_seq = get_ans("wall_seq", "inner wall/outer wall");
        let badge = format!(
          "Overhang Fan {:+}% (≤100%) • Min Layer Time {:+}s • Order = {}",
          ov_boost, l_time_boost, w_seq
        );
        let title = format!(
          "Symptom Fix: Overhang Curling ({:+}% Fan, {:+}s Layer Time)",
          ov_boost, l_time_boost
        );
        let desc = format!(
          "Stops steep overhang curling and bridge sag for {}: boosts overhang fan by {:+}% (up to 100%), adds {:+}s to minimum layer time, and sets {} wall sequence.",
          scope, ov_boost, l_time_boost, w_seq
        );
        let lua = format!(
          "-- {title}\n-- {badge}\nfunction apply(config, ctx)\n  local cur_of = tonumber(config[\"overhang_fan_speed\"]) or 80\n  config[\"overhang_fan_speed\"] = tostring(math.min(100, math.floor(cur_of + ({ov_boost}))))\n  local cur_lt = tonumber(config[\"slow_down_layer_time\"]) or 6\n  config[\"slow_down_layer_time\"] = tostring(math.max(4, math.floor(cur_lt + ({l_time_boost}))))\n  config[\"slow_down_for_layer_cooling\"] = \"1\"\n  config[\"wall_sequence\"] = \"{w_seq}\"\n  print(\"[Symptom:Overhangs] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "sym_weak_layers" => {
        let t_boost: i32 = get_ans("temp_boost", "+5")
          .trim_start_matches('+')
          .parse()
          .unwrap_or(5);
        let fan_pct: f64 = get_ans("fan_cap", "-25%")
          .trim_end_matches('%')
          .trim_start_matches('+')
          .parse()
          .unwrap_or(-25.0);
        let fan_scale = if fan_pct < 0.0 {
          ((100.0 + fan_pct) / 100.0).clamp(0.30, 1.0)
        } else {
          ((100.0 - fan_pct) / 100.0).clamp(0.30, 1.0)
        };
        let fan_drop_label = ((1.0 - fan_scale) * 100.0).round();
        let extra_loops: i32 = get_ans("wall_loops", "+2")
          .trim_start_matches('+')
          .parse()
          .unwrap_or(2);
        let badge = format!(
          "Nozzle {:+}°C • Max Fan -{:.0}% ({:.2}×) • Wall Loops {:+}",
          t_boost, fan_drop_label, fan_scale, extra_loops
        );
        let title = format!(
          "Symptom Fix: Layer Bond Strength ({:+}°C, -{:.0}% Fan, {:+} Walls)",
          t_boost, fan_drop_label, extra_loops
        );
        let desc = format!(
          "Increases Z-axis layer adhesion and part strength for {}: nozzle {:+}°C, scales max fan by {:.2}× (-{:.0}%), and adds {:+} wall loops.",
          scope, t_boost, fan_scale, fan_drop_label, extra_loops
        );
        let lua = format!(
          "-- {title}\n-- {badge}\nfunction apply(config, ctx)\n  local cur_t = tonumber(config[\"nozzle_temperature\"]) or 240\n  config[\"nozzle_temperature\"] = tostring(cur_t + ({t_boost}))\n  local cur_fan = tonumber(config[\"fan_max_speed\"]) or 50\n  config[\"fan_max_speed\"] = tostring(math.max(10, math.floor(cur_fan * {fan_scale:.2} + 0.5)))\n  local cur_w = tonumber(config[\"wall_loops\"]) or 2\n  config[\"wall_loops\"] = tostring(math.max(2, math.floor(cur_w + ({extra_loops}))))\n  print(\"[Symptom:LayerStrength] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "sym_under_extrusion" => {
        let f_boost: f64 = get_ans("flow_boost", "+0.03")
          .trim_start_matches('+')
          .parse()
          .unwrap_or(0.03);
        let mvs_trim: f64 = get_ans("mvs_trim", "15")
          .trim_end_matches('%')
          .parse::<f64>()
          .unwrap_or(15.0)
          .abs();
        let t_boost: i32 = get_ans("temp_boost", "+5")
          .trim_start_matches('+')
          .parse()
          .unwrap_or(5);
        let scale = ((100.0 - mvs_trim) / 100.0).clamp(0.5, 1.0);
        let badge = format!(
          "Flow {:+.2} • MVS -{:.0}% ({:.2}×) • Nozzle {:+}°C",
          f_boost, mvs_trim, scale, t_boost
        );
        let title = format!(
          "Symptom Fix: Under-Extrusion (Flow {:+.2}, -{:.0}% MVS, {:+}°C)",
          f_boost, mvs_trim, t_boost
        );
        let desc = format!(
          "Closes under-extrusion line gaps for {}: boosts flow ratio by {:+.2}, scales MVS by {:.2}×, and adds {:+}°C nozzle temp.",
          scope, f_boost, scale, t_boost
        );
        let lua = format!(
          "-- {title}\n-- {badge}\nfunction apply(config, ctx)\n  local cur_f = tonumber(config[\"filament_flow_ratio\"]) or 0.96\n  config[\"filament_flow_ratio\"] = string.format(\"%.3f\", cur_f + ({f_boost:.3}))\n  local cur_mvs = tonumber(config[\"filament_max_volumetric_speed\"]) or 15.0\n  config[\"filament_max_volumetric_speed\"] = string.format(\"%.1f\", cur_mvs * {scale:.2})\n  local cur_t = tonumber(config[\"nozzle_temperature\"]) or 230\n  config[\"nozzle_temperature\"] = tostring(cur_t + ({t_boost}))\n  print(\"[Symptom:UnderExtrusion] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "sym_over_extrusion" => {
        let f_drop: f64 = get_ans("flow_drop", "-0.02")
          .trim_start_matches('+')
          .parse()
          .unwrap_or(-0.02);
        let top_f_trim: f64 = get_ans("top_flow", "-0.04")
          .trim_start_matches('+')
          .parse()
          .unwrap_or(-0.04);
        let badge = format!("Flow {:+.2} • Top Surface Flow {:+.2}", f_drop, top_f_trim);
        let title = format!(
          "Symptom Fix: Over-Extrusion (Flow {:+.2}, Top Flow {:+.2})",
          f_drop, top_f_trim
        );
        let desc = format!(
          "Eliminates plowed top ridges and dimensional bloat for {}: reduces filament flow by {:+.2} and trims top_surface_flow_ratio by {:+.2}.",
          scope, f_drop, top_f_trim
        );
        let lua = format!(
          "-- {title}\n-- {badge}\nfunction apply(config, ctx)\n  local cur_f = tonumber(config[\"filament_flow_ratio\"]) or 0.98\n  config[\"filament_flow_ratio\"] = string.format(\"%.3f\", cur_f + ({f_drop:.3}))\n  local cur_tf = tonumber(config[\"top_surface_flow_ratio\"]) or 1.00\n  config[\"top_surface_flow_ratio\"] = string.format(\"%.2f\", cur_tf + ({top_f_trim:.2}))\n  print(\"[Symptom:OverExtrusion] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "sym_extruder_grinding" => {
        let r_pct: f64 = get_ans("retract_spd_cap", "-25%")
          .trim_end_matches('%')
          .trim_start_matches('+')
          .parse()
          .unwrap_or(-25.0);
        let r_scale = if r_pct < 0.0 {
          ((100.0 + r_pct) / 100.0).clamp(0.40, 1.0)
        } else {
          ((100.0 - r_pct) / 100.0).clamp(0.40, 1.0)
        };
        let r_drop_label = ((1.0 - r_scale) * 100.0).round();
        let min_trv_boost: f64 = get_ans("min_travel", "+1.0")
          .trim_start_matches('+')
          .parse()
          .unwrap_or(1.0);
        let t_rel: i32 = get_ans("temp_relief", "+5")
          .trim_start_matches('+')
          .parse()
          .unwrap_or(5);
        let badge = format!(
          "Retract Speed -{:.0}% ({:.2}×) • Min Travel {:+.1}mm • Nozzle {:+}°C",
          r_drop_label, r_scale, min_trv_boost, t_rel
        );
        let title = format!(
          "Symptom Fix: Extruder Anti-Grind (-{:.0}% Retract Speed, {:+.1}mm Min Travel)",
          r_drop_label, min_trv_boost
        );
        let desc = format!(
          "Stops extruder gears from grinding filament for {}: scales retraction & deretraction speed by {:.2}× (-{:.0}%), adds {:+.1}mm minimum travel before retracting, and adds {:+}°C back-pressure relief.",
          scope, r_scale, r_drop_label, min_trv_boost, t_rel
        );
        let lua = format!(
          "-- {title}\n-- {badge}\nfunction apply(config, ctx)\n  local cur_rs = tonumber(config[\"filament_retraction_speed\"]) or 45\n  config[\"filament_retraction_speed\"] = tostring(math.max(15, math.floor(cur_rs * {r_scale:.2} + 0.5)))\n  local cur_ds = tonumber(config[\"filament_deretraction_speed\"]) or 35\n  config[\"filament_deretraction_speed\"] = tostring(math.max(15, math.floor(cur_ds * {r_scale:.2} + 0.5)))\n  local cur_mt = tonumber(config[\"filament_retraction_minimum_travel\"]) or 1.5\n  config[\"filament_retraction_minimum_travel\"] = string.format(\"%.1f\", cur_mt + ({min_trv_boost:.1}))\n  local cur_t = tonumber(config[\"nozzle_temperature\"]) or 230\n  config[\"nozzle_temperature\"] = tostring(cur_t + ({t_rel}))\n  print(\"[Symptom:ExtruderGrind] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "sym_wet_filament" => {
        let t_drop: i32 = get_ans("temp_drop", "-5")
          .trim_start_matches('+')
          .parse()
          .unwrap_or(-5);
        let f_trim: f64 = get_ans("flow_trim", "-0.02")
          .trim_start_matches('+')
          .parse()
          .unwrap_or(-0.02);
        let badge = format!(
          "Nozzle {:+}°C (Anti-Steam) • Flow {:+.2} (Anti-Foam)",
          t_drop, f_trim
        );
        let title = format!(
          "Symptom Fix: Damp Filament Mitigation ({:+}°C, Flow {:+.2})",
          t_drop, f_trim
        );
        let desc = format!(
          "Reduces steam popping and foaming on slightly damp filament for {}: lowers nozzle {:+}°C and trims flow {:+.2}.",
          scope, t_drop, f_trim
        );
        let lua = format!(
          "-- {title}\n-- {badge}\nfunction apply(config, ctx)\n  local cur_t = tonumber(config[\"nozzle_temperature\"]) or 240\n  config[\"nozzle_temperature\"] = tostring(cur_t + ({t_drop}))\n  local cur_f = tonumber(config[\"filament_flow_ratio\"]) or 0.96\n  config[\"filament_flow_ratio\"] = string.format(\"%.3f\", cur_f + ({f_trim:.3}))\n  config[\"filament_z_hop_types\"] = \"Spiral\"\n  print(\"[Symptom:WetFilament] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "sym_zits_blobs" => {
        let seam_pos = get_ans("seam_pos", "aligned");
        let deret_pct: f64 = get_ans("deretract_spd", "-25%")
          .trim_end_matches('%')
          .trim_start_matches('+')
          .parse()
          .unwrap_or(-25.0);
        let d_scale = if deret_pct < 0.0 {
          ((100.0 + deret_pct) / 100.0).clamp(0.40, 1.0)
        } else {
          ((100.0 - deret_pct) / 100.0).clamp(0.40, 1.0)
        };
        let drop_label = ((1.0 - d_scale) * 100.0).round();
        let scarf = get_ans("enable_scarf", "contour");
        let badge = format!(
          "Seam={} • Deretract -{:.0}% ({:.2}×) • Scarf={} • Wipe on Loops",
          seam_pos, drop_label, d_scale, scarf
        );
        let title = format!(
          "Symptom Fix: Wall Zits & Blobs (Seam={}, Deretract -{:.0}%)",
          seam_pos, drop_label
        );
        let desc = format!(
          "Eliminates pimples and zits on outer walls for {}: sets seam_position={}, scales deretraction speed by {:.2}× (-{:.0}%), scarf={}, and wipe_on_loops=1.",
          scope, seam_pos, d_scale, drop_label, scarf
        );
        let lua = format!(
          "-- {title}\n-- {badge}\nfunction apply(config, ctx)\n  config[\"seam_position\"] = \"{seam_pos}\"\n  local cur_d = tonumber(config[\"filament_deretraction_speed\"]) or 35\n  config[\"filament_deretraction_speed\"] = tostring(math.max(15, math.floor(cur_d * {d_scale:.2} + 0.5)))\n  config[\"seam_slope_type\"] = \"{scarf}\"\n  local cur_sl = tonumber(config[\"seam_slope_min_length\"]) or 15\n  config[\"seam_slope_min_length\"] = tostring(math.floor(cur_sl * 1.25 + 0.5))\n  config[\"seam_gap\"] = \"10%\"\n  config[\"wipe_on_loops\"] = \"1\"\n  print(\"[Symptom:ZitsBlobs] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "sym_not_sticking" => {
        let w_pct: f64 = get_ans("fl_width", "+20%")
          .trim_end_matches('%')
          .trim_start_matches('+')
          .parse::<f64>()
          .unwrap_or(20.0)
          .abs();
        let w_scale = ((100.0 + w_pct) / 100.0).clamp(1.0, 1.5);
        let s_pct: f64 = get_ans("fl_speed", "-35%")
          .trim_end_matches('%')
          .trim_start_matches('+')
          .parse()
          .unwrap_or(-35.0);
        let s_scale = if s_pct < 0.0 {
          ((100.0 + s_pct) / 100.0).clamp(0.35, 1.0)
        } else {
          ((100.0 - s_pct) / 100.0).clamp(0.35, 1.0)
        };
        let s_drop_label = ((1.0 - s_scale) * 100.0).round();
        let fl_t: i32 = get_ans("fl_temp_boost", "+5")
          .trim_start_matches('+')
          .parse()
          .unwrap_or(5);
        let badge = format!(
          "Layer 1 Width +{:.0}% ({:.2}×) • Speed -{:.0}% ({:.2}×) • L1 Temp {:+}°C",
          w_pct, w_scale, s_drop_label, s_scale, fl_t
        );
        let title = format!(
          "Symptom Fix: First-Layer Bed Grip (+{:.0}% Width, -{:.0}% Speed, {:+}°C)",
          w_pct, s_drop_label, fl_t
        );
        let desc = format!(
          "Fixes first-layer detachment on Textured PEI for {}: scales initial line width by {:.2}× (+{:.0}%), scales speed by {:.2}× (-{:.0}%), and adds {:+}°C initial layer temp.",
          scope, w_scale, w_pct, s_scale, s_drop_label, fl_t
        );
        let lua = format!(
          "-- {title}\n-- {badge}\nfunction apply(config, ctx)\n  local noz = tonumber(ctx.nozzle_diameter) or 0.42\n  local cur_w = tonumber(config[\"initial_layer_line_width\"]) or noz\n  config[\"initial_layer_line_width\"] = string.format(\"%.2f\", cur_w * {w_scale:.2})\n  local cur_s = tonumber(config[\"initial_layer_speed\"]) or 45\n  config[\"initial_layer_speed\"] = tostring(math.max(15, math.floor(cur_s * {s_scale:.2} + 0.5)))\n  local cur_is = tonumber(config[\"initial_layer_infill_speed\"]) or cur_s\n  config[\"initial_layer_infill_speed\"] = tostring(math.max(15, math.floor(cur_is * {s_scale:.2} + 0.5)))\n  local cur_nt = tonumber(config[\"nozzle_temperature_initial_layer\"]) or 230\n  config[\"nozzle_temperature_initial_layer\"] = tostring(cur_nt + ({fl_t}))\n  local cur_bt = tonumber(config[\"textured_pei_plate_temp_initial_layer\"]) or 65\n  config[\"textured_pei_plate_temp_initial_layer\"] = tostring(cur_bt + ({fl_t}))\n  local cur_fo = tonumber(config[\"close_fan_the_first_x_layers\"]) or 1\n  config[\"close_fan_the_first_x_layers\"] = tostring(math.max(2, cur_fo + 2))\n  print(\"[Symptom:FirstLayerStick] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "sym_elephants_foot" => {
        let ef_boost: f64 = get_ans("ef_comp", "+0.10")
          .trim_start_matches('+')
          .parse()
          .unwrap_or(0.10);
        let b_trim: i32 = get_ans("bed_trim", "-5")
          .trim_start_matches('+')
          .parse()
          .unwrap_or(-5);
        let badge = format!(
          "elefant_foot_compensation {:+.2}mm • Continuous Bed {:+}°C",
          ef_boost, b_trim
        );
        let title = format!(
          "Symptom Fix: Elephant's Foot ({:+.2}mm Inset, Bed {:+}°C)",
          ef_boost, b_trim
        );
        let desc = format!(
          "Eliminates bottom-layer outward flare for {}: adds {:+.2}mm to elefant_foot_compensation and trims continuous bed temp by {:+}°C.",
          scope, ef_boost, b_trim
        );
        let lua = format!(
          "-- {title}\n-- {badge}\nfunction apply(config, ctx)\n  local cur_ef = tonumber(config[\"elefant_foot_compensation\"]) or 0.05\n  config[\"elefant_foot_compensation\"] = string.format(\"%.2f\", cur_ef + ({ef_boost:.2}))\n  config[\"elefant_foot_compensation_layers\"] = \"1\"\n  local cur_b = tonumber(config[\"textured_pei_plate_temp\"]) or 65\n  config[\"textured_pei_plate_temp\"] = tostring(cur_b + ({b_trim}))\n  print(\"[Symptom:ElephantsFoot] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "sym_first_layer_ripples" => {
        let h_pct: f64 = get_ans("fl_height", "+20%")
          .trim_end_matches('%')
          .trim_start_matches('+')
          .parse::<f64>()
          .unwrap_or(20.0)
          .abs();
        let h_scale = ((100.0 + h_pct) / 100.0).clamp(1.0, 1.5);
        let w_pct: f64 = get_ans("fl_width", "-10%")
          .trim_end_matches('%')
          .trim_start_matches('+')
          .parse()
          .unwrap_or(-10.0);
        let w_scale = if w_pct < 0.0 {
          ((100.0 + w_pct) / 100.0).clamp(0.75, 1.0)
        } else {
          ((100.0 - w_pct) / 100.0).clamp(0.75, 1.0)
        };
        let w_drop_label = ((1.0 - w_scale) * 100.0).round();
        let badge = format!(
          "Initial Layer Height +{:.0}% ({:.2}×) • Initial Line Width -{:.0}% ({:.2}×)",
          h_pct, h_scale, w_drop_label, w_scale
        );
        let title = format!(
          "Symptom Fix: First-Layer Ripples (+{:.0}% Height, -{:.0}% Width)",
          h_pct, w_drop_label
        );
        let desc = format!(
          "Stops wavy plowing ripples on Layer 1 for {}: scales initial_layer_print_height by {:.2}× (+{:.0}%) and initial_layer_line_width by {:.2}× (-{:.0}%).",
          scope, h_scale, h_pct, w_scale, w_drop_label
        );
        let lua = format!(
          "-- {title}\n-- {badge}\nfunction apply(config, ctx)\n  local cur_lh = tonumber(config[\"initial_layer_print_height\"]) or tonumber(config[\"layer_height\"]) or 0.20\n  config[\"initial_layer_print_height\"] = string.format(\"%.2f\", cur_lh * {h_scale:.2})\n  local cur_w = tonumber(config[\"initial_layer_line_width\"]) or 0.45\n  config[\"initial_layer_line_width\"] = string.format(\"%.2f\", cur_w * {w_scale:.2})\n  print(\"[Symptom:FirstLayerRipples] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "sym_visible_seam" => {
        let def_type = get_ans("defect_type", "raised");
        let s_pct: f64 = get_ans("scarf_len", "+25%")
          .trim_end_matches('%')
          .trim_start_matches('+')
          .parse::<f64>()
          .unwrap_or(25.0)
          .abs();
        let s_scale = ((100.0 + s_pct) / 100.0).clamp(1.0, 2.0);
        let gap_delta: i32 = if def_type == "sunken" { -4 } else { 2 };
        let badge = format!(
          "Scarf Ramp +{:.0}% ({:.2}×, All Walls) • Seam Gap {:+}% ({})",
          s_pct, s_scale, gap_delta, def_type
        );
        let title = format!(
          "Symptom Fix: Z-Seam Scar (+{:.0}% Scarf Ramp, {:+}% Gap)",
          s_pct, gap_delta
        );
        let desc = format!(
          "Smooths visible Z-seam ({}) for {}: enables all-wall Scarf Joint (+{:.0}% ramp length), adjusts seam_gap by {:+}%, and sets wipe_on_loops = 1.",
          def_type, scope, s_pct, gap_delta
        );
        let lua = format!(
          "-- {title}\n-- {badge}\nfunction apply(config, ctx)\n  config[\"seam_slope_type\"] = \"all\"\n  config[\"seam_slope_conditional\"] = \"1\"\n  local cur_sl = tonumber(config[\"seam_slope_min_length\"]) or 16\n  config[\"seam_slope_min_length\"] = tostring(math.max(10, math.floor(cur_sl * {s_scale:.2} + 0.5)))\n  config[\"seam_slope_steps\"] = \"15\"\n  local cur_gap = tonumber(config[\"seam_gap\"]) or 8\n  config[\"seam_gap\"] = tostring(math.max(2, math.min(20, math.floor(cur_gap + ({gap_delta}))))) .. \"%\"\n  config[\"wipe_on_loops\"] = \"1\"\n  print(\"[Symptom:VisibleSeam] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "sym_ringing_vfa" => {
        let acc_pct: f64 = get_ans("outer_accel", "-40%")
          .trim_end_matches('%')
          .trim_start_matches('+')
          .parse()
          .unwrap_or(-40.0);
        let acc_scale = if acc_pct < 0.0 {
          ((100.0 + acc_pct) / 100.0).clamp(0.30, 1.0)
        } else {
          ((100.0 - acc_pct) / 100.0).clamp(0.30, 1.0)
        };
        let acc_drop_label = ((1.0 - acc_scale) * 100.0).round();
        let spd_pct: f64 = get_ans("outer_spd", "+20%")
          .trim_end_matches('%')
          .trim_start_matches('+')
          .parse()
          .unwrap_or(20.0);
        let spd_scale = ((100.0 + spd_pct) / 100.0).clamp(0.40, 1.60);
        let badge = format!(
          "Outer Wall Accel -{:.0}% ({:.2}×) • Outer Wall Speed {:+.0}% ({:.2}× VFA Shift)",
          acc_drop_label, acc_scale, spd_pct, spd_scale
        );
        let title = format!(
          "Symptom Fix: Ringing & VFA (-{:.0}% Accel, {:+.0}% Outer Wall Speed)",
          acc_drop_label, spd_pct
        );
        let desc = format!(
          "Eliminates ghosting echoes and VFA belt artifacts for {}: scales outer_wall_acceleration by {:.2}× (-{:.0}%) and outer_wall_speed by {:.2}× ({:+.0}%).",
          scope, acc_scale, acc_drop_label, spd_scale, spd_pct
        );
        let lua = format!(
          "-- {title}\n-- {badge}\nfunction apply(config, ctx)\n  local cur_acc = tonumber(config[\"outer_wall_acceleration\"]) or 5000\n  config[\"outer_wall_acceleration\"] = tostring(math.max(1000, math.floor(cur_acc * {acc_scale:.2} + 0.5)))\n  local cur_spd = tonumber(config[\"outer_wall_speed\"]) or 140\n  config[\"outer_wall_speed\"] = tostring(math.max(30, math.floor(cur_spd * {spd_scale:.2} + 0.5)))\n  local cur_j = tonumber(config[\"outer_wall_jerk\"]) or 9\n  config[\"outer_wall_jerk\"] = tostring(math.max(4, math.floor(cur_j * {acc_scale:.2} + 0.5)))\n  print(\"[Symptom:RingingVFA] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "sym_layer_shifting" => {
        let t_pct: f64 = get_ans("travel_accel", "-30%")
          .trim_end_matches('%')
          .trim_start_matches('+')
          .parse()
          .unwrap_or(-30.0);
        let t_scale = if t_pct < 0.0 {
          ((100.0 + t_pct) / 100.0).clamp(0.35, 1.0)
        } else {
          ((100.0 - t_pct) / 100.0).clamp(0.35, 1.0)
        };
        let t_drop_label = ((1.0 - t_scale) * 100.0).round();
        let zh_boost: f64 = get_ans("zhop_height", "+0.10")
          .trim_start_matches('+')
          .parse()
          .unwrap_or(0.10);
        let badge = format!(
          "Travel Accel/Jerk -{:.0}% ({:.2}×) • Z-Hop {:+.2}mm • Non-Crossing Gyroid",
          t_drop_label, t_scale, zh_boost
        );
        let title = format!(
          "Symptom Fix: Anti-Layer-Shift (-{:.0}% Travel Accel, {:+.2}mm Z-Hop)",
          t_drop_label, zh_boost
        );
        let desc = format!(
          "Prevents stepper step loss and nozzle overhang collisions for {}: scales travel_acceleration & travel_jerk by {:.2}× (-{:.0}%), adds {:+.2}mm Z-Hop, and uses gyroid infill.",
          scope, t_scale, t_drop_label, zh_boost
        );
        let lua = format!(
          "-- {title}\n-- {badge}\nfunction apply(config, ctx)\n  local cur_ta = tonumber(config[\"travel_acceleration\"]) or 10000\n  config[\"travel_acceleration\"] = tostring(math.max(2500, math.floor(cur_ta * {t_scale:.2} + 0.5)))\n  local cur_tj = tonumber(config[\"travel_jerk\"]) or 12\n  config[\"travel_jerk\"] = tostring(math.max(5, math.floor(cur_tj * {t_scale:.2} + 0.5)))\n  local cur_zh = tonumber(config[\"filament_z_hop\"]) or 0.15\n  config[\"filament_z_hop\"] = string.format(\"%.2f\", cur_zh + ({zh_boost:.2}))\n  config[\"sparse_infill_pattern\"] = \"gyroid\"\n  config[\"reduce_infill_retraction\"] = \"0\"\n  print(\"[Symptom:LayerShift] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "sym_z_banding_hull_line" => {
        let lock_spd = get_ans("lock_outer_spd", "1");
        let w_ord = get_ans("wall_order", "inner-outer-inner wall");
        let badge = format!(
          "dont_slow_down_outer_wall = {} • wall_sequence = {}",
          lock_spd, w_ord
        );
        let title = format!(
          "Symptom Fix: Z-Banding & Hull Line (Lock Outer Speed={}, {})",
          lock_spd, w_ord
        );
        let desc = format!(
          "Eliminates Benchy hull-line shrinkage bands and Z-wall gloss changes for {}: sets dont_slow_down_outer_wall = {} and wall_sequence = {}.",
          scope, lock_spd, w_ord
        );
        let lua = format!(
          "-- {title}\n-- {badge}\nfunction apply(config, ctx)\n  config[\"dont_slow_down_outer_wall\"] = \"{lock_spd}\"\n  config[\"wall_sequence\"] = \"{w_ord}\"\n  print(\"[Symptom:ZBanding] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "sym_corner_bulge" => {
        let pa_delta: f64 = get_ans("pa_val", "+0.008")
          .trim_start_matches('+')
          .parse::<f64>()
          .unwrap_or(0.008)
          .clamp(-0.05, 0.10);
        let j_pct: f64 = get_ans("outer_jerk", "-15%")
          .trim_end_matches('%')
          .trim_start_matches('+')
          .parse()
          .unwrap_or(-15.0);
        let j_scale = ((100.0 + j_pct) / 100.0).clamp(0.50, 1.50);
        let badge = format!(
          "PA {:+.3} • Outer Wall Jerk {:+.0}% ({:.2}×)",
          pa_delta, j_pct, j_scale
        );
        let title = format!(
          "Symptom Fix: Corner Bulge (PA {:+.3}, Jerk {:+.0}%)",
          pa_delta, j_pct
        );
        let desc = format!(
          "Sharpens 90° corners and eliminates corner swelling for {}: adjusts pressure_advance by {:+.3} and scales outer_wall_jerk by {:.2}× ({:+.0}%).",
          scope, pa_delta, j_scale, j_pct
        );
        let lua = format!(
          "-- {title}\n-- {badge}\nfunction apply(config, ctx)\n  config[\"enable_pressure_advance\"] = \"1\"\n  local cur_pa = tonumber(config[\"pressure_advance\"]) or 0.036\n  config[\"pressure_advance\"] = string.format(\"%.3f\", math.max(0.005, cur_pa + ({pa_delta:.3})))\n  local cur_j = tonumber(config[\"outer_wall_jerk\"]) or 9\n  config[\"outer_wall_jerk\"] = tostring(math.max(4, math.floor(cur_j * {j_scale:.2} + 0.5)))\n  print(\"[Symptom:CornerBulge] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "sym_pillowing" => {
        let extra_layers: i32 = get_ans("top_layers", "+2")
          .trim_start_matches('+')
          .parse()
          .unwrap_or(2);
        let inf_boost: i32 = get_ans("infill_pct", "+5%")
          .trim_end_matches('%')
          .trim_start_matches('+')
          .parse()
          .unwrap_or(5);
        let badge = format!(
          "Top Shell {:+} Layers • Sparse Infill {:+}%",
          extra_layers, inf_boost
        );
        let title = format!(
          "Symptom Fix: Top Surface Pillowing ({:+} Top Layers, {:+}% Infill)",
          extra_layers, inf_boost
        );
        let desc = format!(
          "Eliminates bumps and holes in top solid surfaces for {}: adds {:+} top_shell_layers (and scales top_shell_thickness by layer_height) and boosts sparse_infill_density by {:+}%.",
          scope, extra_layers, inf_boost
        );
        let lua = format!(
          "-- {title}\n-- {badge}\nfunction apply(config, ctx)\n  local cur_tl = tonumber(config[\"top_shell_layers\"]) or 3\n  local new_tl = math.max(3, math.floor(cur_tl + ({extra_layers})))\n  config[\"top_shell_layers\"] = tostring(new_tl)\n  local lh = tonumber(config[\"layer_height\"]) or 0.20\n  local cur_tt = tonumber(config[\"top_shell_thickness\"]) or (cur_tl * lh)\n  config[\"top_shell_thickness\"] = string.format(\"%.2f\", cur_tt + (({extra_layers}) * lh))\n  local cur_inf = tonumber(config[\"sparse_infill_density\"]) or 15\n  config[\"sparse_infill_density\"] = tostring(math.min(100, math.floor(cur_inf + ({inf_boost})))) .. \"%\"\n  print(\"[Symptom:Pillowing] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "sym_infill_wall_gaps" => {
        let ov_boost: i32 = get_ans("overlap_pct", "+8%")
          .trim_end_matches('%')
          .trim_start_matches('+')
          .parse()
          .unwrap_or(8);
        let spd_pct: f64 = get_ans("top_spd", "-25%")
          .trim_end_matches('%')
          .trim_start_matches('+')
          .parse()
          .unwrap_or(-25.0);
        let spd_scale = if spd_pct < 0.0 {
          ((100.0 + spd_pct) / 100.0).clamp(0.40, 1.0)
        } else {
          ((100.0 - spd_pct) / 100.0).clamp(0.40, 1.0)
        };
        let spd_drop_label = ((1.0 - spd_scale) * 100.0).round();
        let badge = format!(
          "infill_wall_overlap {:+}% • wall_generator = arachne • Top Speed -{:.0}% ({:.2}×)",
          ov_boost, spd_drop_label, spd_scale
        );
        let title = format!(
          "Symptom Fix: Infill-to-Wall Pinholes ({:+}% Overlap, -{:.0}% Top Speed)",
          ov_boost, spd_drop_label
        );
        let desc = format!(
          "Closes pinhole gaps between infill and walls for {}: boosts infill_wall_overlap by {:+}%, enables arachne walls, and scales top_surface_speed by {:.2}× (-{:.0}%).",
          scope, ov_boost, spd_scale, spd_drop_label
        );
        let lua = format!(
          "-- {title}\n-- {badge}\nfunction apply(config, ctx)\n  local cur_ov = tonumber(config[\"infill_wall_overlap\"]) or 15\n  config[\"infill_wall_overlap\"] = tostring(math.min(50, math.floor(cur_ov + ({ov_boost})))) .. \"%\"\n  config[\"wall_generator\"] = \"arachne\"\n  local cur_ts = tonumber(config[\"top_surface_speed\"]) or 80\n  config[\"top_surface_speed\"] = tostring(math.max(20, math.floor(cur_ts * {spd_scale:.2} + 0.5)))\n  print(\"[Symptom:InfillWallGaps] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "sym_top_surface_scars" => {
        let pat = get_ans("top_pattern", "monotonicline");
        let iron = get_ans("ironing_mode", "no ironing");
        let badge = format!(
          "Top Pattern = {} • Avoid Crossing Walls • Ironing = {}",
          pat, iron
        );
        let title = format!(
          "Symptom Fix: Top Surface Scars & Drag Marks ({}, {})",
          pat, iron
        );
        let desc = format!(
          "Prevents nozzle travel drag marks across top skins for {}: sets top_surface_pattern = {}, reduce_infill_retraction = 0, reduce_crossing_wall = 1, and ironing_type = {}.",
          scope, pat, iron
        );
        let lua = format!(
          "-- {title}\n-- {badge}\nfunction apply(config, ctx)\n  config[\"top_surface_pattern\"] = \"{pat}\"\n  config[\"reduce_infill_retraction\"] = \"0\"\n  config[\"reduce_crossing_wall\"] = \"1\"\n  config[\"ironing_type\"] = \"{iron}\"\n  print(\"[Symptom:TopScars] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "sym_weak_infill" => {
        let pat = get_ans("pattern", "gyroid");
        let spd_pct: f64 = get_ans("infill_spd", "-25%")
          .trim_end_matches('%')
          .trim_start_matches('+')
          .parse()
          .unwrap_or(-25.0);
        let spd_scale = if spd_pct < 0.0 {
          ((100.0 + spd_pct) / 100.0).clamp(0.40, 1.0)
        } else {
          ((100.0 - spd_pct) / 100.0).clamp(0.40, 1.0)
        };
        let spd_drop_label = ((1.0 - spd_scale) * 100.0).round();
        let badge = format!(
          "Infill = {} • Speed -{:.0}% ({:.2}×) • Width +12% (1.12×)",
          pat, spd_drop_label, spd_scale
        );
        let title = format!(
          "Symptom Fix: Strong Non-Crossing Infill ({}, -{:.0}% Speed)",
          pat, spd_drop_label
        );
        let desc = format!(
          "Eliminates weak/snapping internal infill for {}: switches to {} pattern, scales sparse_infill_speed by {:.2}× (-{:.0}%), and widens sparse_infill_line_width by 1.12×.",
          scope, pat, spd_scale, spd_drop_label
        );
        let lua = format!(
          "-- {title}\n-- {badge}\nfunction apply(config, ctx)\n  config[\"sparse_infill_pattern\"] = \"{pat}\"\n  local cur_s = tonumber(config[\"sparse_infill_speed\"]) or 240\n  config[\"sparse_infill_speed\"] = tostring(math.max(40, math.floor(cur_s * {spd_scale:.2} + 0.5)))\n  local noz = tonumber(ctx.nozzle_diameter) or 0.40\n  local cur_w = tonumber(config[\"sparse_infill_line_width\"]) or noz\n  config[\"sparse_infill_line_width\"] = string.format(\"%.2f\", cur_w * 1.12)\n  print(\"[Symptom:WeakInfill] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "sym_poor_bridging" => {
        let bf_pct: f64 = get_ans("bridge_flow", "-8%")
          .trim_end_matches('%')
          .trim_start_matches('+')
          .parse()
          .unwrap_or(-8.0);
        let bf_scale = if bf_pct < 0.0 {
          ((100.0 + bf_pct) / 100.0).clamp(0.70, 1.0)
        } else if bf_pct <= 1.2 {
          bf_pct.clamp(0.70, 1.0)
        } else {
          ((100.0 - bf_pct) / 100.0).clamp(0.70, 1.0)
        };
        let bf_drop_label = ((1.0 - bf_scale) * 100.0).round();
        let bs_pct: f64 = get_ans("bridge_spd", "-35%")
          .trim_end_matches('%')
          .trim_start_matches('+')
          .parse()
          .unwrap_or(-35.0);
        let bs_scale = if bs_pct < 0.0 {
          ((100.0 + bs_pct) / 100.0).clamp(0.30, 1.0)
        } else {
          ((100.0 - bs_pct) / 100.0).clamp(0.30, 1.0)
        };
        let bs_drop_label = ((1.0 - bs_scale) * 100.0).round();
        let badge = format!(
          "Bridge Flow -{:.0}% ({:.2}×) • Bridge Speed -{:.0}% ({:.2}×) • Bridge Fan = 100%",
          bf_drop_label, bf_scale, bs_drop_label, bs_scale
        );
        let title = format!(
          "Symptom Fix: Taut Bridging (Flow -{:.0}%, Speed -{:.0}%, 100% Fan)",
          bf_drop_label, bs_drop_label
        );
        let desc = format!(
          "Stops bridge sagging and drooping for {}: scales bridge_flow by {:.2}× (-{:.0}%), scales bridge_speed by {:.2}× (-{:.0}%), and sets overhang_fan_speed = 100%.",
          scope, bf_scale, bf_drop_label, bs_scale, bs_drop_label
        );
        let lua = format!(
          "-- {title}\n-- {badge}\nfunction apply(config, ctx)\n  local cur_bf = tonumber(config[\"bridge_flow\"]) or 1.00\n  config[\"bridge_flow\"] = string.format(\"%.2f\", cur_bf * {bf_scale:.2})\n  local cur_bs = tonumber(config[\"bridge_speed\"]) or 45\n  config[\"bridge_speed\"] = tostring(math.max(15, math.floor(cur_bs * {bs_scale:.2} + 0.5)))\n  config[\"overhang_fan_speed\"] = \"100\"\n  print(\"[Symptom:Bridging] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      "sym_support_scarring" => {
        let zd_raw = get_ans("top_z_dist", "+0.04");
        let is_zero_interface = zd_raw.trim() == "0.00" || zd_raw.trim() == "0";
        let zd_offset: f64 = zd_raw.trim_start_matches('+').parse().unwrap_or(0.04);
        let st = get_ans("sup_style", "tree(auto)");
        let badge = if is_zero_interface {
          format!(
            "support_top_z_distance = 0.00mm (Dissimilar Interface) • Type = {} • 3 Interface Layers",
            st
          )
        } else {
          format!(
            "Support Z-Distance {:+.2}mm • Type = {} • 3 Interface Layers",
            zd_offset, st
          )
        };
        let title = if is_zero_interface {
          format!("Symptom Fix: Zero-Gap Dissimilar Supports (0.00mm, {})", st)
        } else {
          format!(
            "Symptom Fix: Easy-Release Supports ({:+.2}mm Z-Gap, {})",
            zd_offset, st
          )
        };
        let desc = if is_zero_interface {
          format!(
            "Configures zero-gap dissimilar support interface for {}: sets support_top_z_distance = 0.00mm, support_type = {}, and 3 interface layers.",
            scope, st
          )
        } else {
          format!(
            "Prevents supports from fusing to the model for {}: adds {:+.2}mm to support_top_z_distance (and bottom Z-distance), sets support_type = {}, and adds 3 interface layers.",
            scope, zd_offset, st
          )
        };
        let lua = if is_zero_interface {
          format!(
            "-- {title}\n-- {badge}\nfunction apply(config, ctx)\n  config[\"support_top_z_distance\"] = \"0.00\"\n  config[\"support_bottom_z_distance\"] = \"0.00\"\n  config[\"support_type\"] = \"{st}\"\n  config[\"support_interface_top_layers\"] = \"3\"\n  print(\"[Symptom:Supports] {badge}\")\nend\n"
          )
        } else {
          format!(
            "-- {title}\n-- {badge}\nfunction apply(config, ctx)\n  local lh = tonumber(config[\"layer_height\"]) or 0.20\n  local cur_zd = tonumber(config[\"support_top_z_distance\"]) or lh\n  local new_zd = string.format(\"%.2f\", cur_zd + ({zd_offset:.2}))\n  config[\"support_top_z_distance\"] = new_zd\n  config[\"support_bottom_z_distance\"] = new_zd\n  config[\"support_type\"] = \"{st}\"\n  config[\"support_interface_top_layers\"] = \"3\"\n  print(\"[Symptom:Supports] {badge}\")\nend\n"
          )
        };
        (title, desc, badge, lua)
      }
      "sym_ams_color_bleed" => {
        let tw_pct: f64 = get_ans("tower_width", "+25%")
          .trim_end_matches('%')
          .trim_start_matches('+')
          .parse::<f64>()
          .unwrap_or(25.0)
          .abs();
        let tw_scale = ((100.0 + tw_pct) / 100.0).clamp(1.0, 2.0);
        let pv_pct: f64 = get_ans("prime_vol", "+35%")
          .trim_end_matches('%')
          .trim_start_matches('+')
          .parse::<f64>()
          .unwrap_or(35.0)
          .abs();
        let pv_scale = ((100.0 + pv_pct) / 100.0).clamp(1.0, 2.5);
        let badge = format!(
          "Prime Tower Width +{:.0}% ({:.2}×) • Prime Volume +{:.0}% ({:.2}×)",
          tw_pct, tw_scale, pv_pct, pv_scale
        );
        let title = format!(
          "Symptom Fix: ACE Pro Anti-Bleed & Tower (+{:.0}% Width, +{:.0}% Prime Vol)",
          tw_pct, pv_pct
        );
        let desc = format!(
          "Prevents multi-color bleeding and prime tower collapse for {}: scales prime_tower_width by {:.2}× (+{:.0}%), scales prime_volume by {:.2}× (+{:.0}%), and trims cut retraction.",
          scope, tw_scale, tw_pct, pv_scale, pv_pct
        );
        let lua = format!(
          "-- {title}\n-- {badge}\nfunction apply(config, ctx)\n  config[\"enable_prime_tower\"] = \"1\"\n  local cur_tw = tonumber(config[\"prime_tower_width\"]) or 35\n  config[\"prime_tower_width\"] = tostring(math.floor(cur_tw * {tw_scale:.2} + 0.5))\n  local cur_pv = tonumber(config[\"prime_volume\"]) or 35\n  config[\"prime_volume\"] = tostring(math.floor(cur_pv * {pv_scale:.2} + 0.5))\n  local cur_cut = tonumber(config[\"retraction_distances_when_cut\"]) or 14.0\n  config[\"retraction_distances_when_cut\"] = string.format(\"%.1f\", math.min(12.0, cur_cut * 0.85))\n  print(\"[Symptom:AMSColorBleed] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
      _ => {
        let scarf_mode = get_ans("scarf_mode", "all");
        let scarf_len = get_ans("scarf_len", "20");
        let seam_gap = get_ans("seam_gap", "10%");
        let wall_gen = get_ans("wall_gen", "arachne");
        let badge = format!(
          "Scarf = {} ({}mm ramp, {} gap) • Wall Engine = {}",
          scarf_mode, scarf_len, seam_gap, wall_gen
        );
        let title = format!(
          "Tuning: Scarf Seam ({}, {}mm) + {} Walls",
          scarf_mode, scarf_len, wall_gen
        );
        let desc = format!(
          "Parameterized Scarf Seam & Wall tuning: seam_slope_type = {}, ramp = {}mm, seam_gap = {}, wall_generator = {}.",
          scarf_mode, scarf_len, seam_gap, wall_gen
        );
        let lua = format!(
          "-- {title}\nfunction apply(config, ctx)\n  config[\"wall_generator\"] = \"{wall_gen}\"\n  config[\"seam_slope_type\"] = \"{scarf_mode}\"\n  config[\"seam_slope_conditional\"] = \"1\"\n  config[\"seam_slope_start_height\"] = \"10%\"\n  config[\"seam_slope_min_length\"] = \"{scarf_len}\"\n  config[\"seam_slope_steps\"] = \"15\"\n  config[\"seam_gap\"] = \"{seam_gap}\"\n  config[\"scarf_angle_threshold\"] = \"155\"\n  config[\"wipe_on_loops\"] = \"1\"\n  print(\"[Tuning:ScarfArachne] {badge}\")\nend\n"
        );
        (title, desc, badge, lua)
      }
    }
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

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  #[test]
  fn test_parameterized_calibration_templates_build_and_execute() {
    let templates = TransformationEngine::get_parameterized_calibration_templates();
    assert_eq!(templates.len(), 33);

    // 1. Test YOLO Flow Rate calculation (0.96 + (-0.02) = 0.940)
    let yolo = templates
      .iter()
      .find(|t| t.id == "cal_yolo_flow")
      .expect("cal_yolo_flow template exists");
    let mut answers = yolo.default_answers();
    answers.insert("mode".to_string(), "YOLO".to_string());
    answers.insert("old_flow".to_string(), "0.96".to_string());
    answers.insert("chip_val".to_string(), "-0.02".to_string());
    let (title, _desc, badge, _lua) = yolo.generate_from_answers(&answers, "filament:PCTG");
    assert!(title.contains("0.940"), "title was: {}", title);
    assert!(badge.contains("0.940"), "badge was: {}", badge);

    // Execute every template's default Lua script via execute_transformation_lua to ensure valid sandboxed Lua
    let ctx = json!({
      "printer_model": "Anycubic Kobra S1",
      "nozzle_type": "Hardened Steel",
      "nozzle_diameter": 0.4,
      "filament_type": "PCTG"
    });

    for tmpl in &templates {
      let default_ans = tmpl.default_answers();
      let (_rule_title, _rule_desc, _badge, script) =
        tmpl.generate_from_answers(&default_ans, &tmpl.default_condition_expr);
      let base_val = json!({
        "type": "filament",
        "name": "Test Profile",
        "filament_flow_ratio": ["0.96"]
      });
      let res = execute_transformation_lua(&script, &base_val, &ctx);
      assert!(
        res.is_ok(),
        "Template {} failed to execute Lua: {:?}",
        tmpl.id,
        res.err()
      );
    }
  }

  #[test]
  fn test_non_commutative_transformation_stack_order_and_reordering() {
    let mut base = OrcaProfile::new("Test PCTG", "filament");
    base
      .values
      .insert("filament_max_volumetric_speed".to_string(), json!(["20.0"]));
    let ctx = json!({
      "printer_model": "Anycubic Kobra S1",
      "nozzle_type": "Hardened Steel",
      "nozzle_diameter": 0.4,
      "filament_type": "PCTG"
    });

    let rule_mult = TransformationRule {
      id: "rule_scale".to_string(),
      name: "Scale MVS by 0.5x".to_string(),
      description: "Multiply MVS by 0.5".to_string(),
      category: "Test".to_string(),
      icon: "✖️".to_string(),
      enabled: true,
      condition_expr: "all".to_string(),
      lua_code: "function apply(config, ctx)\n  local cur = tonumber(config.filament_max_volumetric_speed and config.filament_max_volumetric_speed[1]) or 20.0\n  config.filament_max_volumetric_speed = { string.format(\"%.1f\", cur * 0.5) }\nend\n".to_string(),
      priority: 10,
    };

    let rule_add = TransformationRule {
      id: "rule_add".to_string(),
      name: "Add +4.0 to MVS".to_string(),
      description: "Add 4.0 to MVS".to_string(),
      category: "Test".to_string(),
      icon: "➕".to_string(),
      enabled: true,
      condition_expr: "all".to_string(),
      lua_code: "function apply(config, ctx)\n  local cur = tonumber(config.filament_max_volumetric_speed and config.filament_max_volumetric_speed[1]) or 20.0\n  config.filament_max_volumetric_speed = { string.format(\"%.1f\", cur + 4.0) }\nend\n".to_string(),
      priority: 20,
    };

    // Order 1: Step #1 (Scale 0.5x) -> Step #2 (Add +4.0): 20.0 * 0.5 + 4.0 = 14.0
    let mut stack = vec![rule_mult, rule_add];
    let res1 = TransformationEngine::evaluate_stack(&base, &stack, &ctx);
    assert_eq!(
      res1
        .resulting_profile
        .get_str("filament_max_volumetric_speed")
        .as_deref(),
      Some("14.0")
    );

    // Move 'rule_add' earlier (Step #1: Add +4.0 -> Step #2: Scale 0.5x): (20.0 + 4.0) * 0.5 = 12.0
    TransformationEngine::reorder_rule(&mut stack, "rule_add", StackReorderAction::MoveUp);
    assert_eq!(stack[0].id, "rule_add");
    assert_eq!(stack[1].id, "rule_scale");
    let res2 = TransformationEngine::evaluate_stack(&base, &stack, &ctx);
    assert_eq!(
      res2
        .resulting_profile
        .get_str("filament_max_volumetric_speed")
        .as_deref(),
      Some("12.0")
    );

    // Move 'rule_add' later (Step #1: Scale 0.5x -> Step #2: Add +4.0) -> back to 14.0!
    TransformationEngine::reorder_rule(&mut stack, "rule_add", StackReorderAction::MoveDown);
    assert_eq!(stack[0].id, "rule_scale");
    assert_eq!(stack[1].id, "rule_add");
    let res3 = TransformationEngine::evaluate_stack(&base, &stack, &ctx);
    assert_eq!(
      res3
        .resulting_profile
        .get_str("filament_max_volumetric_speed")
        .as_deref(),
      Some("14.0")
    );
  }

  #[test]
  fn test_symptom_wizards_use_relative_scaling_across_high_flow_and_standard_rigs() {
    let templates = TransformationEngine::get_parameterized_calibration_templates();

    // 1. Bridging Fix (`sym_poor_bridging`) scales relatively on MicroSwiss High-Flow CHT vs Standard rig
    let bridging = templates
      .iter()
      .find(|t| t.id == "sym_poor_bridging")
      .expect("sym_poor_bridging exists");
    let ans = bridging.default_answers(); // -8% bridge_flow (0.92x), -35% bridge_speed (0.65x)
    let (_title, _desc, _badge, lua_bridge) = bridging.generate_from_answers(&ans, "all");

    let std_cfg = json!({
      "bridge_flow": "1.00",
      "bridge_speed": "40"
    });
    let cht_cfg = json!({
      "bridge_flow": "1.05",
      "bridge_speed": "80"
    });
    let ctx = json!({
      "nozzle_type": "microswiss_cht",
      "nozzle_diameter": 0.6
    });

    let (std_out, _, _) = execute_transformation_lua(&lua_bridge, &std_cfg, &ctx).unwrap();
    let (cht_out, _, _) = execute_transformation_lua(&lua_bridge, &cht_cfg, &ctx).unwrap();

    // Standard (40 mm/s * 0.65 = 26 mm/s, 1.00 * 0.92 = 0.92)
    assert_eq!(std_out["bridge_speed"], "26");
    assert_eq!(std_out["bridge_flow"], "0.92");
    // High-Flow CHT (80 mm/s * 0.65 = 52 mm/s, 1.05 * 0.92 = 0.97) — proportional to baseline!
    assert_eq!(cht_out["bridge_speed"], "52");
    assert_eq!(cht_out["bridge_flow"], "0.97");

    // 2. Weak Infill Fix (`sym_weak_infill`) scales sparse_infill_speed (-25% / 0.75x) and width (+12% / 1.12x)
    let weak_infill = templates
      .iter()
      .find(|t| t.id == "sym_weak_infill")
      .expect("sym_weak_infill exists");
    let (_, _, _, lua_infill) =
      weak_infill.generate_from_answers(&weak_infill.default_answers(), "all");
    let infill_cfg = json!({
      "sparse_infill_speed": "360",
      "sparse_infill_line_width": "0.60"
    });
    let (infill_out, _, _) = execute_transformation_lua(&lua_infill, &infill_cfg, &ctx).unwrap();
    // 360 * 0.75 = 270 mm/s; 0.60 * 1.12 = 0.67 mm
    assert_eq!(infill_out["sparse_infill_speed"], "270");
    assert_eq!(infill_out["sparse_infill_line_width"], "0.67");
  }
}
