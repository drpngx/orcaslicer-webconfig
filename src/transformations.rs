use crate::lua_engine::{
  apply_lua_param_change_with_sets, describe_lua_rule, embed_param_defaults_in_lua,
  execute_transformation_lua, extract_lua_rule_spec, resolve_lua_rule_params,
  validate_lua_rule_params, LuaParamKind, LuaParamSpec,
};
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
      TransformationRule {
        id: "transparent_optical_glass".to_string(),
        name: "Transparent / Glass-Like Optical Clarity Mode (Rygar1432 / CNC Kitchen)".to_string(),
        description: "Tunes translucent PETG/PCTG/PLA for maximum optical clarity ('How to Print Glass'): 20 mm/s print speeds (5.0 mm³/s MVS ceiling), +0.05 (+5%) flow ratio boost to flood inter-bead micro-voids, 0% part cooling fan, 0.12mm layer height with 1.25× nozzle line width, 100% Aligned Rectilinear infill at 0°, 1 perimeter wall, 0 top/bottom shells, and 65°C Textured PEI easy-release bed temp.".to_string(),
        category: "Optical & Specialty Rule".to_string(),
        icon: "💎".to_string(),
        enabled: false,
        condition_expr: "filament:PETG".to_string(),
        lua_code: include_str!("../transformations/transparent_optical_glass.lua").to_string(),
        priority: 52,
      },
      TransformationRule {
        id: "tpu_foaming_shore".to_string(),
        name: "Active-Foaming TPU Shore Hardness & Expansion Tuner (60A–92A)".to_string(),
        description: "Tunes variable-density active-foaming TPU (colorFabb varioShore TPU, eSUN eTPU-LW, Siraya Tech Flex TPU Air) by Target Shore A Hardness (92A solid @ 195°C / 0.98 flow → 75A midsole @ 220°C / 0.75 flow → 60A max foam @ 245°C / 0.60 flow), MVS residence-time ceiling, and 0mm Z-hop.".to_string(),
        category: "Filament Tuning Rule".to_string(),
        icon: "🧽".to_string(),
        enabled: false,
        condition_expr: "filament:TPU".to_string(),
        lua_code: include_str!("../transformations/parametric/cal_tpu_foaming_shore.lua").to_string(),
        priority: 53,
      },
      TransformationRule {
        id: "weather_climate_adjustment".to_string(),
        name: "Weather & Ambient Climate Adjustment (Tropical / Hot & Dry / Cold & Dry / Cold & Humid)".to_string(),
        description: "Tunes nozzle/bed temps, +5°C 1st-layer bed & 35 mm/s speed (cold), Max Volumetric Speed (-15–20% hot), part & chamber exhaust fans (80–100% hot, 0% cold), no-fan first layers (3–5 cold), Spiral Z-Hop (0.20mm / Top Only PETG / 0mm TPU), Wipe (1.5mm), Avoid Crossing Walls, and Brim using Ambient Temp (°C) and Relative Humidity (% RH) dials.".to_string(),
        category: "Weather & Ambient Climate Rule".to_string(),
        icon: "🌦️".to_string(),
        enabled: false,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/parametric/sym_weather_climate.lua").to_string(),
        priority: 54,
      },
    ]
  }

  /// Curated rule templates for the interactive "Create / Add New Lua Rule" builder in `WizardTab`.
  pub fn get_rule_templates() -> Vec<TransformationRule> {
    vec![
      TransformationRule {
        id: "tpu_foaming_shore".to_string(),
        name: "Active-Foaming TPU Shore Hardness & Expansion Tuner (60A–92A)".to_string(),
        description: "Dial Target Shore A Hardness (55A–92A) or pick 92A Solid, 85A Firm, 75A Midsole, 65A Soft Cushion, or 60A Max Foam presets to synchronize nozzle temperature, foaming expansion flow ratio, MVS residence time, and single-wall caliper trim.".to_string(),
        category: "Filament Tuning Rule".to_string(),
        icon: "🧽".to_string(),
        enabled: true,
        condition_expr: "filament:TPU".to_string(),
        lua_code: include_str!("../transformations/parametric/cal_tpu_foaming_shore.lua").to_string(),
        priority: 53,
      },
      TransformationRule {
        id: "weather_climate_adjustment".to_string(),
        name: "Weather & Ambient Climate Adjustment (Tropical / Hot & Dry / Cold & Dry / Cold & Humid)".to_string(),
        description: "Dial Ambient Temperature (°C) and Relative Humidity (% RH) or pick Tropical (30°C/80% RH), Hot & Dry (33°C/20% RH), Cold & Dry (14°C/25% RH), or Cold & Humid (15°C/80% RH) presets for PLA, Silk PLA, PETG, TPU, ABS/ASA, and PA/PC.".to_string(),
        category: "Weather & Ambient Climate Rule".to_string(),
        icon: "🌦️".to_string(),
        enabled: true,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/parametric/sym_weather_climate.lua").to_string(),
        priority: 54,
      },
      TransformationRule {
        id: "transparent_optical_glass".to_string(),
        name: "Transparent / Glass-Like Optical Clarity Mode (Rygar1432 / CNC Kitchen)".to_string(),
        description: "Tunes translucent PETG/PCTG/PLA for optical clarity: 20 mm/s ultra-slow speeds (5.0 mm³/s MVS), +0.05 (+5%) flow boost to eliminate air micro-voids, 0% cooling fan, 0.12mm layers, 1.25× line width, 100% Aligned Rectilinear 0° infill, 1 wall, 0 top/bottom shells, and 65°C Textured PEI release.".to_string(),
        category: "Optical & Specialty Rule".to_string(),
        icon: "💎".to_string(),
        enabled: true,
        condition_expr: "filament:PETG".to_string(),
        lua_code: include_str!("../transformations/transparent_optical_glass.lua").to_string(),
        priority: 52,
      },
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
params = {
  {
    id = "scarf_mode",
    label = "Scarf Joint Seam Scope",
    description = "Contour + Inner Wall (all), Contour Only (contour), or Disabled (none).",
    type = "enum",
    default = "all",
    options = {
      { value = "all", label = "Contour + Inner Wall (all)" },
      { value = "contour", label = "Contour Only (contour)" },
      { value = "none", label = "Disabled (none)" },
    },
  },
  {
    id = "scarf_steps",
    label = "Scarf Ramp Steps",
    description = "Number of step segments in the scarf ramp (5 - 30 steps).",
    type = "range",
    min = 5,
    max = 30,
    step = 5,
    default = "15",
    unit = "steps",
    options = {
      { value = "10", label = "10 steps" },
      { value = "15", label = "15 steps (Recommended)" },
      { value = "20", label = "20 steps" },
    },
  },
}

function validate(p, ctx)
  local steps = tonumber(p.scarf_steps) or 15
  if p.scarf_mode ~= "none" and steps < 5 then
    return false, "Scarf ramp requires at least 5 steps."
  end
  return true
end

function apply(config, ctx, p)
  local mode = (p and p.scarf_mode) or "all"
  local steps = tostring(math.floor((tonumber(p and p.scarf_steps) or 15) + 0.5))
  config["seam_slope_type"] = mode
  config["seam_slope_conditional"] = "1"
  config["seam_slope_start_height"] = "10%"
  config["seam_slope_entire_loop"] = "0"
  config["seam_slope_min_length"] = "15"
  config["seam_slope_steps"] = steps
  config["seam_gap"] = "10%"
  config["scarf_angle_threshold"] = "155"
  config["wipe_on_loops"] = "1"

  print(string.format("[Process:ScarfSeams] seam_slope_type -> %s (%s steps, 15mm ramp, 10%% seam gap)", mode, steps))
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
params = {
  {
    id = "wall_generator",
    label = "Perimeter Wall Generator",
    description = "Arachne dynamically varies extrusion width for thin walls; Classic uses fixed width.",
    type = "enum",
    default = "arachne",
    options = {
      { value = "arachne", label = "Arachne (Variable Width)" },
      { value = "classic", label = "Classic (Fixed Width)" },
    },
  },
  {
    id = "transition_angle",
    label = "Wall Transition Angle",
    description = "Wedge angle threshold for Arachne variable width transitions (5 - 30 deg).",
    type = "range",
    min = 5,
    max = 30,
    step = 5,
    default = "10",
    unit = "°",
    options = {
      { value = "10", label = "10° (Recommended)" },
      { value = "15", label = "15°" },
    },
  },
}

function validate(p, ctx)
  return true
end

function apply(config, ctx, p)
  local wg = (p and p.wall_generator) or "arachne"
  local angle = tostring(math.floor((tonumber(p and p.transition_angle) or 10) + 0.5))
  config["wall_generator"] = wg
  config["wall_transition_angle"] = angle
  config["wall_transition_length"] = "100%"
  config["min_feature_size"] = "25%"
  config["min_bead_width"] = "85%"

  print(string.format("[Process:ArachneWalls] wall_generator -> %s (%s deg transition, 85%% min bead)", wg, angle))
end
"#.to_string(),
        priority: 44,
      },
      TransformationRule {
        id: "custom_blank_rule".to_string(),
        name: "Custom Parametric Lua Rule".to_string(),
        description: "Pre-populated custom Lua rule template with enum, range, and boolean parameter specifiers (`params = { ... }`), cross-parameter validation (`validate(p, ctx)`), dynamic description (`describe(p, ctx)`), and `apply(config, ctx, p)`.".to_string(),
        category: "Custom Parametric Rule".to_string(),
        icon: "⚙️".to_string(),
        enabled: true,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/custom_parametric_template.lua").to_string(),
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

  /// Synchronizes the `weather_climate_adjustment` rule in the stack with the active `MachineRig`'s
  /// `climate_preset`, `ambient_temp_c`, and `humidity_pct` dials, enabling the rule automatically
  /// whenever a non-temperate weather preset or ambient temp / humidity dial is selected on the rig.
  pub fn sync_weather_rule_for_rig(rules: &mut [TransformationRule], rig: &MachineRig) {
    let is_non_temperate = rig.climate_preset != "temperate"
      || rig.ambient_temp_c <= 18
      || rig.ambient_temp_c >= 26
      || rig.humidity_pct >= 60;
    let mut params = HashMap::new();
    params.insert("climate_preset".to_string(), rig.climate_preset.clone());
    params.insert("ambient_temp_c".to_string(), rig.ambient_temp_c.to_string());
    params.insert("humidity_pct".to_string(), rig.humidity_pct.to_string());
    params.insert("enable_brim_in_cold".to_string(), "1".to_string());
    let base_lua = include_str!("../transformations/parametric/sym_weather_climate.lua");
    for rule in rules.iter_mut() {
      if rule.id == "weather_climate_adjustment"
        || rule.id.starts_with("wizard_sym_weather_climate")
      {
        rule.lua_code = embed_param_defaults_in_lua(base_lua, &params);
        let ctx = serde_json::json!({
          "nozzle_type": rig.nozzle_type,
          "nozzle_material": rig.nozzle_material,
          "hotend": rig.hotend,
          "addons": rig.addons,
        });
        let scope = rule.condition_expr.clone();
        if let Ok(desc) = describe_lua_rule(&rule.lua_code, &params, &scope, &ctx) {
          if let Some(t) = desc.title {
            rule.name = t;
          }
          if let Some(d) = desc.description {
            rule.description = d;
          }
        }
        if rule.id == "weather_climate_adjustment" {
          rule.enabled = is_non_temperate;
        }
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
    let ams_capacity = rig.ace_pro_units * 4;
    for (slot_idx, spool) in filaments.iter().enumerate() {
      let in_active_ams_slot = rig.ace_pro_units > 0 && slot_idx < ams_capacity && spool.is_loaded;
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

      // Check TPU loaded in an active ACE Pro multi-material slot (slots #1..#ams_capacity; Reserve spools outside AMS are exempt, as are high-durometer 64D/68D TPUs like "TPU for ACE" / "TPU for AMS" / "TPU-CF" / "TPU-GF")
      let mat_up = spool.material_type.to_uppercase();
      let name_up = spool.name.to_uppercase();
      let is_ams_safe_tpu = mat_up.contains("FOR ACE")
        || mat_up.contains("FOR AMS")
        || mat_up.contains("64D")
        || mat_up.contains("68D")
        || mat_up.contains("72D")
        || mat_up.contains("-CF")
        || mat_up.contains("-GF")
        || name_up.contains("FOR ACE")
        || name_up.contains("FOR AMS")
        || name_up.contains("64D")
        || name_up.contains("68D")
        || name_up.contains("72D");
      if in_active_ams_slot && mat_up.contains("TPU") && !is_ams_safe_tpu {
        push_warn(SafetyWarning {
          id: format!("spool_tpu_ace:{}", spool.id),
          severity: SafetySeverity::Warning,
          scope_label: format!("ACE Pro Slot #{} ({})", slot_idx + 1, spool.color_name),
          param_key: "Flexible TPU in Motorized AMS/ACE Hub".to_string(),
          current_value: format!("{} ({})", spool.name, spool.material_type),
          safe_range: "Reserve / Direct Spool Holder Bypass or High-Shore TPU (TPU for ACE / 68D)".to_string(),
          explanation: "Standard soft (85A–95A) TPU loaded in an active ACE Pro Bowden slot frequently buckles inside the motorized filament buffer and long PTFE path. Move soft TPU to a Reserve slot (direct bypass), or use rigid Shore 64D–68D 'TPU for ACE' / 'TPU for AMS'.".to_string(),
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

          if (spool.material_type.contains("-CF")
            || spool.material_type.contains("-GF")
            || spool.finish_type.contains("Carbon"))
            && extract_i32("required_nozzle_HRC") == Some(3)
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
      let mat_up = mat.to_uppercase();
      let (min_nozzle, max_nozzle, warn_nozzle, max_bed) = if mat_up.contains("PCL") {
        (70, 180, 175, 65)
      } else if mat_up.contains("PEEK") {
        (320, 420, 410, 160)
      } else if mat_up.contains("PPS") {
        (295, 365, 360, 125)
      } else if mat_up.contains("PPA")
        || mat_up.contains("PAHT")
        || mat_up.contains("PA6")
        || mat_up.contains("PA12")
        || mat_up.contains("PA612")
        || mat_up.contains("EASY PA")
        || mat_up == "PA"
        || mat_up == "PA-CF"
        || mat_up == "PA-GF"
        || mat_up.starts_with("PA ")
        || mat_up.contains("ULTRAPA")
        || (mat_up.contains("PC") && !mat_up.contains("PCTG"))
        || mat_up.contains("PET-")
        || mat_up.starts_with("SUPPORT FOR PA")
        || mat_up.contains("FOR PA12")
        || mat_up == "PETG-ESD"
      {
        (230, 325, 320, 120)
      } else if mat_up.contains("PCTG") {
        (240, 280, 272, 95)
      } else if mat_up.contains("COPE") {
        (205, 255, 250, 80)
      } else if (mat_up.contains("PETG") || mat_up == "PET") && spool.finish_type == "Conductive" {
        (220, 290, 285, 95)
      } else if mat_up.contains("PETG") || mat_up == "PET" {
        (215, 275, 270, 90)
      } else if mat_up.contains("ABS")
        || mat_up.contains("ASA")
        || mat_up == "SBS"
        || mat_up == "HIPS"
      {
        (230, 315, 310, 115)
      } else if mat_up.contains("PEBA")
        || mat_up.contains("FOAMING")
        || mat_up.contains("AIR")
        || spool.finish_type == "Foaming"
        || mat_up.contains("TPU-CF")
        || mat_up.contains("TPU-GF")
        || mat_up.contains("TPU-AERO")
      {
        (180, 280, 276, 105)
      } else if mat_up.contains("TPU")
        || mat_up.contains("POLYFC")
        || mat_up.contains("POLYFLEX")
        || mat_up == "FLEX"
        || mat_up == "EVA"
      {
        (190, 260, 255, 70)
      } else if mat_up.contains("PCL") {
        (70, 180, 175, 65)
      } else if mat_up.contains("PLA")
        || mat_up.contains("PANCHROMA")
        || mat_up.contains("POLYTERRA")
        || mat_up.contains("POLYLITE")
        || mat_up.contains("POLYMAX")
        || mat_up == "PHA"
      {
        (180, 245, 240, 75)
      } else {
        (180, 305, 295, 115)
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
      let has_foaming_spool = filaments.iter().any(|sp| {
        sp.finish_type.eq_ignore_ascii_case("Foaming") || sp.material_type.contains("Foaming")
      });
      let min_safe_flow = if has_foaming_spool { 0.45 } else { 0.85 };
      if !(min_safe_flow..=1.12).contains(&flow) {
        push_warn(SafetyWarning {
          id: "derived:filament_flow_ratio".to_string(),
          severity: if !(0.40..=1.20).contains(&flow) {
            SafetySeverity::Danger
          } else {
            SafetySeverity::Warning
          },
          scope_label: "Derived Output Profile".to_string(),
          param_key: "filament_flow_ratio".to_string(),
          current_value: format!("{:.3}", flow),
          safe_range: if has_foaming_spool {
            "0.480–1.050 (Active Foaming / Standard)".to_string()
          } else {
            "0.900–1.050".to_string()
          },
          explanation: "Derived flow ratio is outside the normal range; extreme values cause severe under-extrusion gaps or over-extrusion nozzle collisions.".to_string(),
        });
      }
    }

    warnings
  }

  /// Prepackaged, parameterized post-print calibration & tuning wizards loaded directly from
  /// `transformations/parametric/*.lua`. Each Lua script declares `meta = { ... }`,
  /// `params = { ... }` (with description, range/enum bounds, default, unit),
  /// `validate(params, ctx)`, `describe(params, ctx)`, and `apply(config, ctx, params)`.
  pub fn get_parameterized_calibration_templates() -> Vec<ParameterizedCalibrationTemplate> {
    let scripts: &[(&str, &str)] = &[
      (
        "cal_yolo_flow",
        include_str!("../transformations/parametric/cal_yolo_flow.lua"),
      ),
      (
        "cal_pressure_advance",
        include_str!("../transformations/parametric/cal_pressure_advance.lua"),
      ),
      (
        "cal_temp_tower",
        include_str!("../transformations/parametric/cal_temp_tower.lua"),
      ),
      (
        "cal_retraction_tower",
        include_str!("../transformations/parametric/cal_retraction_tower.lua"),
      ),
      (
        "cal_max_volumetric_speed",
        include_str!("../transformations/parametric/cal_max_volumetric_speed.lua"),
      ),
      (
        "cal_dimensional_xy",
        include_str!("../transformations/parametric/cal_dimensional_xy.lua"),
      ),
      (
        "cal_scarf_arachne",
        include_str!("../transformations/parametric/cal_scarf_arachne.lua"),
      ),
      (
        "cal_tpu_foaming_shore",
        include_str!("../transformations/parametric/cal_tpu_foaming_shore.lua"),
      ),
      (
        "sym_stringing",
        include_str!("../transformations/parametric/sym_stringing.lua"),
      ),
      (
        "sym_boogers",
        include_str!("../transformations/parametric/sym_boogers.lua"),
      ),
      (
        "sym_clogging",
        include_str!("../transformations/parametric/sym_clogging.lua"),
      ),
      (
        "sym_warping_adhesion",
        include_str!("../transformations/parametric/sym_warping_adhesion.lua"),
      ),
      (
        "sym_overhang_curl",
        include_str!("../transformations/parametric/sym_overhang_curl.lua"),
      ),
      (
        "sym_weak_layers",
        include_str!("../transformations/parametric/sym_weak_layers.lua"),
      ),
      (
        "sym_under_extrusion",
        include_str!("../transformations/parametric/sym_under_extrusion.lua"),
      ),
      (
        "sym_over_extrusion",
        include_str!("../transformations/parametric/sym_over_extrusion.lua"),
      ),
      (
        "sym_extruder_grinding",
        include_str!("../transformations/parametric/sym_extruder_grinding.lua"),
      ),
      (
        "sym_wet_filament",
        include_str!("../transformations/parametric/sym_wet_filament.lua"),
      ),
      (
        "sym_zits_blobs",
        include_str!("../transformations/parametric/sym_zits_blobs.lua"),
      ),
      (
        "sym_not_sticking",
        include_str!("../transformations/parametric/sym_not_sticking.lua"),
      ),
      (
        "sym_elephants_foot",
        include_str!("../transformations/parametric/sym_elephants_foot.lua"),
      ),
      (
        "sym_first_layer_ripples",
        include_str!("../transformations/parametric/sym_first_layer_ripples.lua"),
      ),
      (
        "sym_visible_seam",
        include_str!("../transformations/parametric/sym_visible_seam.lua"),
      ),
      (
        "sym_ringing_vfa",
        include_str!("../transformations/parametric/sym_ringing_vfa.lua"),
      ),
      (
        "sym_layer_shifting",
        include_str!("../transformations/parametric/sym_layer_shifting.lua"),
      ),
      (
        "sym_z_banding_hull_line",
        include_str!("../transformations/parametric/sym_z_banding_hull_line.lua"),
      ),
      (
        "sym_corner_bulge",
        include_str!("../transformations/parametric/sym_corner_bulge.lua"),
      ),
      (
        "sym_pillowing",
        include_str!("../transformations/parametric/sym_pillowing.lua"),
      ),
      (
        "sym_infill_wall_gaps",
        include_str!("../transformations/parametric/sym_infill_wall_gaps.lua"),
      ),
      (
        "sym_top_surface_scars",
        include_str!("../transformations/parametric/sym_top_surface_scars.lua"),
      ),
      (
        "sym_weak_infill",
        include_str!("../transformations/parametric/sym_weak_infill.lua"),
      ),
      (
        "sym_poor_bridging",
        include_str!("../transformations/parametric/sym_poor_bridging.lua"),
      ),
      (
        "sym_support_scarring",
        include_str!("../transformations/parametric/sym_support_scarring.lua"),
      ),
      (
        "sym_ams_color_bleed",
        include_str!("../transformations/parametric/sym_ams_color_bleed.lua"),
      ),
      (
        "sym_transparent_glass",
        include_str!("../transformations/parametric/sym_transparent_glass.lua"),
      ),
      (
        "sym_weather_climate",
        include_str!("../transformations/parametric/sym_weather_climate.lua"),
      ),
    ];

    scripts
      .iter()
      .filter_map(|(fallback_id, lua_src)| {
        ParameterizedCalibrationTemplate::from_lua_script(fallback_id, lua_src).ok()
      })
      .collect()
  }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CalibrationQuestion {
  pub id: String,
  pub question: String,
  pub hint: String,
  pub kind: LuaParamKind,
  pub default_value: String,
  pub unit: String,
  pub quick_options: Vec<(String, String)>,
  #[serde(default)]
  pub option_sets: HashMap<String, Vec<(String, String)>>,
}

impl CalibrationQuestion {
  pub fn from_lua_param(p: &LuaParamSpec) -> Self {
    Self {
      id: p.id.clone(),
      question: p.label.clone(),
      hint: p.description.clone(),
      kind: p.kind.clone(),
      default_value: p.default_value.clone(),
      unit: p.unit.clone(),
      quick_options: p.options.clone(),
      option_sets: p.option_sets.clone(),
    }
  }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterizedCalibrationTemplate {
  pub id: String,
  pub name: String,
  pub icon: String,
  pub category: String,
  pub tree_category_str: String,
  pub tree_subcategory_str: String,
  pub slicer_menu_path: String,
  pub when_to_run: String,
  pub how_to_read_print: Vec<String>,
  pub hardware_checklist_items: Vec<String>,
  pub formula_summary: String,
  pub default_condition_expr: String,
  pub questions: Vec<CalibrationQuestion>,
  pub lua_template: String,
}

impl ParameterizedCalibrationTemplate {
  /// Construct a `ParameterizedCalibrationTemplate` directly by introspecting a `.lua` rule script.
  pub fn from_lua_script(fallback_id: &str, lua_src: &str) -> Result<Self, String> {
    let spec = extract_lua_rule_spec(lua_src)?;
    let id = spec.meta.id.unwrap_or_else(|| fallback_id.to_string());
    let name = spec.meta.name.unwrap_or_else(|| id.clone());
    let icon = spec.meta.icon.unwrap_or_else(|| "🪄".to_string());
    let category = spec
      .meta
      .category
      .unwrap_or_else(|| "Parameterized Lua Rule".to_string());
    let tree_category_str = spec
      .meta
      .tree_category
      .unwrap_or_else(|| "Post-Print Calibration".to_string());
    let tree_subcategory_str = spec
      .meta
      .tree_subcategory
      .unwrap_or_else(|| "Calibration Test".to_string());
    let slicer_menu_path = spec.meta.slicer_menu_path.unwrap_or_default();
    let when_to_run = spec.meta.when_to_run.unwrap_or_default();
    let formula_summary = spec.meta.formula_summary.unwrap_or_default();
    let default_condition_expr = spec
      .meta
      .default_condition_expr
      .unwrap_or_else(|| "all".to_string());
    let how_to_read_print = spec.meta.how_to_read_print;
    let hardware_checklist_items = spec.meta.hardware_checklist;
    let questions = spec
      .params
      .iter()
      .map(CalibrationQuestion::from_lua_param)
      .collect();

    Ok(Self {
      id,
      name,
      icon,
      category,
      tree_category_str,
      tree_subcategory_str,
      slicer_menu_path,
      when_to_run,
      how_to_read_print,
      hardware_checklist_items,
      formula_summary,
      default_condition_expr,
      questions,
      lua_template: lua_src.to_string(),
    })
  }

  pub fn is_symptom_wizard(&self) -> bool {
    self.id.starts_with("sym_")
  }

  /// Returns the 7 top-level categories in the Print Quality & Troubleshooting Category Tree:
  /// `(category_id, display_label, icon)`
  pub fn symptom_tree_categories() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
      ("all", "All Print Quality Issues (28)", "🗂️"),
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
        "6. Overhangs, Bridging & Climate",
        "🌀",
      ),
      (
        "7. Structural, Mechanical & Multi-Color",
        "7. Structural, Mechanical & AMS",
        "⚙️",
      ),
    ]
  }

  pub fn tree_category(&self) -> &str {
    &self.tree_category_str
  }

  pub fn tree_subcategory(&self) -> &str {
    &self.tree_subcategory_str
  }

  pub fn hardware_checklist(&self) -> Vec<&str> {
    self
      .hardware_checklist_items
      .iter()
      .map(|s| s.as_str())
      .collect()
  }

  pub fn default_answers(&self) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for q in &self.questions {
      map.insert(q.id.clone(), q.default_value.clone());
    }
    map
  }

  /// Apply a parameter change in `answers`, automatically snapping linked dial parameters
  /// when a preset option with `sets = { ... }` is chosen (or switching a linked preset to `"custom"`
  /// when a dial is moved manually).
  pub fn apply_answer_change(
    &self,
    answers: &mut HashMap<String, String>,
    changed_id: &str,
    new_val: String,
  ) {
    if let Ok(spec) = extract_lua_rule_spec(&self.lua_template) {
      apply_lua_param_change_with_sets(&spec.params, answers, changed_id, new_val);
    } else {
      answers.insert(changed_id.to_string(), new_val);
    }
  }

  /// Validate answers against both the Lua parameter schema (`range` min/max, `enum` options)
  /// and the Lua `validate(params, ctx)` function.
  pub fn validate_answers(
    &self,
    answers: &HashMap<String, String>,
    scope: &str,
  ) -> Result<(), String> {
    let ctx = serde_json::json!({ "scope": scope });
    validate_lua_rule_params(&self.lua_template, answers, &ctx).map(|_| ())
  }

  /// Generates `(rule_title, rule_description, computed_summary_badge, lua_code)` by executing
  /// `describe(params, ctx)` in Lua and embedding `configured_params` in the Lua script.
  pub fn generate_from_answers(
    &self,
    answers: &HashMap<String, String>,
    scope: &str,
  ) -> (String, String, String, String) {
    let ctx = serde_json::json!({ "scope": scope });
    let resolved_answers = extract_lua_rule_spec(&self.lua_template)
      .map(|spec| resolve_lua_rule_params(&spec, answers))
      .unwrap_or_else(|_| answers.clone());
    let desc_res =
      describe_lua_rule(&self.lua_template, &resolved_answers, scope, &ctx).unwrap_or_default();
    let title = desc_res.title.unwrap_or_else(|| self.name.clone());
    let description = desc_res
      .description
      .unwrap_or_else(|| self.formula_summary.clone());
    let badge = desc_res
      .badge
      .unwrap_or_else(|| self.formula_summary.clone());
    let lua_code = embed_param_defaults_in_lua(&self.lua_template, &resolved_answers);
    (title, description, badge, lua_code)
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
    assert_eq!(templates.len(), 36);

    // Verify transparent glass wizard generates and executes cleanly on both filament and process JSON
    let glass = templates
      .iter()
      .find(|t| t.id == "sym_transparent_glass")
      .expect("sym_transparent_glass template exists");
    let glass_ans = glass.default_answers();
    let (g_title, _g_desc, g_badge, g_lua) =
      glass.generate_from_answers(&glass_ans, "filament:PETG");
    assert!(g_title.contains("Optical Clarity"), "title: {}", g_title);
    assert!(g_badge.contains("20 mm/s"), "badge: {}", g_badge);
    let (mut g_fil, _, _) = execute_transformation_lua(
      &g_lua,
      &json!({
        "type": "filament",
        "filament_flow_ratio": "0.96",
        "fan_max_speed": "30",
        "textured_plate_temp": "70"
      }),
      &json!({"nozzle_diameter": 0.4}),
    )
    .expect("sym_transparent_glass filament lua runs");
    assert_eq!(
      g_fil
        .as_object_mut()
        .unwrap()
        .get("filament_flow_ratio")
        .and_then(|v| v.as_str()),
      Some("1.010")
    );
    assert_eq!(
      g_fil
        .as_object_mut()
        .unwrap()
        .get("fan_max_speed")
        .and_then(|v| v.as_str()),
      Some("0")
    );
    assert_eq!(
      g_fil
        .as_object_mut()
        .unwrap()
        .get("textured_plate_temp")
        .and_then(|v| v.as_str()),
      Some("65")
    );

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

  #[test]
  fn test_catalog_filaments_and_petg_translucent_equivalence_have_zero_warnings() {
    let catalog = crate::catalog::load_merged_catalog();
    let petg_entry = catalog
      .filaments
      .iter()
      .find(|f| f.code == "PETG")
      .expect("PETG in catalog");
    let petg_trans_entry = catalog
      .filaments
      .iter()
      .find(|f| f.code == "PETG Translucent")
      .expect("PETG Translucent in catalog");

    // 1. Adding PETG and flipping Finish to Translucent must produce the exact same spool
    // as clicking PETG Translucent directly in the catalog (and via apply_material_change)
    let mut petg_flipped = FilamentSpool::from_catalog_entry(1, petg_entry, "orca_hardened_steel");
    petg_flipped.apply_finish_change("Translucent", "orca_hardened_steel");

    let petg_direct = FilamentSpool::from_catalog_entry(1, petg_trans_entry, "orca_hardened_steel");

    assert_eq!(petg_flipped.name, "OC - PETG Translucent • 0.4 Hardened");
    assert_eq!(petg_direct.name, "OC - PETG Translucent • 0.4 Hardened");
    assert_eq!(petg_flipped.name, petg_direct.name);
    assert_eq!(petg_flipped.material_type, "PETG");
    assert_eq!(petg_direct.material_type, "PETG");
    assert_eq!(petg_flipped.finish_type, "Translucent");
    assert_eq!(petg_direct.finish_type, "Translucent");
    assert_eq!(petg_flipped.opacity, petg_direct.opacity);
    assert_eq!(petg_flipped.brass_base_temp, petg_direct.brass_base_temp);
    assert_eq!(petg_flipped.hs_base_temp, petg_direct.hs_base_temp);
    assert_eq!(petg_flipped.print_temp, petg_direct.print_temp);
    assert_eq!(petg_flipped.bed_temp, petg_direct.bed_temp);
    assert_eq!(
      petg_flipped.max_volumetric_speed,
      petg_direct.max_volumetric_speed
    );
    assert_eq!(
      petg_flipped.fan_speed_percent,
      petg_direct.fan_speed_percent
    );
    assert_eq!(petg_flipped.raw_disk_json, petg_direct.raw_disk_json);

    // Flipping back to Basic restores OC - PETG • 0.4 Hardened
    let mut petg_back_to_basic = petg_flipped.clone();
    petg_back_to_basic.apply_finish_change("Basic", "orca_hardened_steel");
    assert_eq!(petg_back_to_basic.name, "OC - PETG • 0.4 Hardened");

    // Also verify apply_material_change("PETG") + apply_finish_change("Translucent") matches
    let mut petg_from_dropdown = petg_flipped.clone();
    petg_from_dropdown.apply_material_change("PETG", "orca_hardened_steel");
    petg_from_dropdown.apply_finish_change("Translucent", "orca_hardened_steel");
    assert_eq!(petg_from_dropdown.print_temp, petg_direct.print_temp);
    assert_eq!(petg_from_dropdown.bed_temp, petg_direct.bed_temp);
    assert_eq!(petg_from_dropdown.finish_type, "Translucent");

    // 2. Verify NO catalog filament triggers a print_temp or bed_temp warning on a Hardened Steel rig
    let mut rig = MachineRig::kobra_s1_hardened_steel_png();
    rig.nozzle_type = "orca_hardened_steel".to_string();
    rig.nozzle_material = "Hardened Steel (55 HRC)".to_string();
    rig.ace_pro_units = 1; // 4 active AMS slots (#1..#4), #5+ are Reserve
    let profile = OrcaProfile::get_stock_kobra_s1();

    for (idx, fil_entry) in catalog.filaments.iter().enumerate() {
      let mut sp = FilamentSpool::from_catalog_entry(idx + 1, fil_entry, &rig.nozzle_type);
      // Avoid soft-TPU-in-ACE warning when testing thermal bounds of catalog entries
      sp.is_loaded = false;
      let warnings =
        TransformationEngine::evaluate_safety_warnings(&rig, &[sp.clone()], &profile, &[], &[]);
      assert!(
        warnings.is_empty(),
        "Catalog filament '{}' (material '{}', print_temp {}°C, bed {}°C) triggered unexpected safety warnings: {:?}",
        fil_entry.code,
        sp.material_type,
        sp.print_temp,
        sp.bed_temp,
        warnings
      );
    }

    // 3. Verify soft TPU (TPU 95A) in Reserve slot (#5, index 4 on a 4-slot ACE Pro) does NOT trigger AMS warning,
    // whereas TPU for ACE does not trigger AMS warning even in Slot #1 (index 0)
    let tpu_95a_entry = catalog
      .filaments
      .iter()
      .find(|f| f.code == "TPU 95A")
      .expect("TPU 95A in catalog");
    let tpu_ace_entry = catalog
      .filaments
      .iter()
      .find(|f| f.code == "TPU for ACE")
      .expect("TPU for ACE in catalog");
    let s1 = FilamentSpool::from_catalog_entry(1, tpu_ace_entry, &rig.nozzle_type);
    let s2 = FilamentSpool::from_catalog_entry(2, petg_entry, &rig.nozzle_type);
    let s3 = FilamentSpool::from_catalog_entry(3, petg_entry, &rig.nozzle_type);
    let s4 = FilamentSpool::from_catalog_entry(4, petg_entry, &rig.nozzle_type);
    let s5_reserve_tpu = FilamentSpool::from_catalog_entry(5, tpu_95a_entry, &rig.nozzle_type);
    let reserve_warnings = TransformationEngine::evaluate_safety_warnings(
      &rig,
      &[s1, s2, s3, s4, s5_reserve_tpu],
      &profile,
      &[],
      &[],
    );
    assert!(
      reserve_warnings.is_empty(),
      "Soft TPU in Reserve slot #5 (outside 4-slot AMS) must not trigger AMS buckling warning, got: {:?}",
      reserve_warnings
    );
  }

  #[test]
  fn test_generalized_lua_param_specifier_and_validation_blocks_invalid_combinations() {
    let custom_lua = r#"
meta = {
  id = "custom_nozzle_tune",
  name = "Custom Nozzle & Speed Tuning",
  icon = "🔧",
  category = "Custom Parametric Rule",
}

params = {
  {
    id = "mode",
    label = "Tuning Mode",
    description = "Choose stealth or speed mode.",
    type = "enum",
    default = "stealth",
    options = {
      { value = "stealth", label = "Stealth Quiet Mode" },
      { value = "speed", label = "High Speed Mode" },
    },
  },
  {
    id = "outer_speed",
    label = "Outer Wall Speed",
    description = "Target outer wall speed in mm/s.",
    type = "range",
    min = 20,
    max = 300,
    step = 10,
    default = "80",
    unit = "mm/s",
  },
}

function validate(p, ctx)
  local m = p.mode or "stealth"
  local spd = tonumber(p.outer_speed) or 80
  if m == "stealth" and spd > 120 then
    return false, "Stealth mode requires outer_speed <= 120 mm/s to avoid stepper resonance."
  end
  return true
end

function apply(config, ctx, p)
  config["outer_wall_speed"] = tostring(tonumber(p.outer_speed) or 80)
end
"#;

    let spec = extract_lua_rule_spec(custom_lua).expect("spec parses");
    assert_eq!(spec.params.len(), 2);
    assert_eq!(spec.params[0].id, "mode");
    assert_eq!(spec.params[0].kind, LuaParamKind::Enum);
    assert_eq!(spec.params[1].id, "outer_speed");
    assert_eq!(
      spec.params[1].kind,
      LuaParamKind::Range {
        min: 20.0,
        max: 300.0,
        step: Some(10.0)
      }
    );
    assert!(spec.has_validate_fn);

    // 1. Valid default parameters pass
    let ctx = json!({"scope": "all"});
    assert!(validate_lua_rule_params(custom_lua, &HashMap::new(), &ctx).is_ok());

    // 2. Invalid enum option is rejected by Layer 1 schema validation
    let mut bad_enum = HashMap::new();
    bad_enum.insert("mode".to_string(), "ludicrous".to_string());
    let err_enum = validate_lua_rule_params(custom_lua, &bad_enum, &ctx).unwrap_err();
    assert!(
      err_enum.contains("not one of the allowed enum options"),
      "got: {}",
      err_enum
    );

    // 3. Out-of-range numeric value is rejected by Layer 1 schema validation
    let mut bad_range = HashMap::new();
    bad_range.insert("outer_speed".to_string(), "450".to_string());
    let err_range = validate_lua_rule_params(custom_lua, &bad_range, &ctx).unwrap_err();
    assert!(
      err_range.contains("outside the allowed range"),
      "got: {}",
      err_range
    );

    // 4. Invalid cross-parameter combination (stealth + 200 mm/s) is rejected by Layer 2 Lua validate(params, ctx)
    let mut bad_combo = HashMap::new();
    bad_combo.insert("mode".to_string(), "stealth".to_string());
    bad_combo.insert("outer_speed".to_string(), "200".to_string());
    let err_combo = validate_lua_rule_params(custom_lua, &bad_combo, &ctx).unwrap_err();
    assert!(
      err_combo.contains("Stealth mode requires outer_speed <= 120 mm/s"),
      "got: {}",
      err_combo
    );

    // 5. Execution of invalid embedded parameters is also blocked
    let bad_embedded = embed_param_defaults_in_lua(custom_lua, &bad_combo);
    assert!(execute_transformation_lua(&bad_embedded, &json!({}), &ctx).is_err());

    // 6. Valid combination (speed + 200 mm/s) passes and applies cleanly
    let mut good_combo = HashMap::new();
    good_combo.insert("mode".to_string(), "speed".to_string());
    good_combo.insert("outer_speed".to_string(), "200".to_string());
    let good_embedded = embed_param_defaults_in_lua(custom_lua, &good_combo);
    let (res_cfg, _, _) = execute_transformation_lua(&good_embedded, &json!({}), &ctx).unwrap();
    assert_eq!(res_cfg["outer_wall_speed"], "200");

    // 7. Verify the pre-populated Custom Parametric Lua Rule template (`custom_blank_rule`)
    // has pre-populated `params`, `validate(p, ctx)`, `describe(p, ctx)`, and `apply(config, ctx, p)`
    let rule_templates = TransformationEngine::get_rule_templates();
    let custom_tpl = rule_templates
      .iter()
      .find(|t| t.id == "custom_blank_rule")
      .expect("custom_blank_rule exists");
    let custom_spec =
      extract_lua_rule_spec(&custom_tpl.lua_code).expect("custom_blank_rule spec parses");
    assert_eq!(custom_spec.params.len(), 4);
    assert!(custom_spec.has_validate_fn);
    assert!(custom_spec.has_describe_fn);
    assert!(validate_lua_rule_params(&custom_tpl.lua_code, &HashMap::new(), &ctx).is_ok());

    let mut invalid_custom = HashMap::new();
    invalid_custom.insert("mode".to_string(), "speed".to_string());
    invalid_custom.insert("outer_wall_scale".to_string(), "0.65".to_string());
    assert!(validate_lua_rule_params(&custom_tpl.lua_code, &invalid_custom, &ctx).is_err());
  }

  #[test]
  fn test_weather_climate_adjustment_presets_and_dials() {
    let templates = TransformationEngine::get_parameterized_calibration_templates();
    let weather = templates
      .iter()
      .find(|t| t.id == "sym_weather_climate")
      .expect("sym_weather_climate template exists");

    // 1. Default is Tropical (30°C / 80% RH)
    let mut ans = weather.default_answers();
    assert_eq!(
      ans.get("climate_preset").map(|s| s.as_str()),
      Some("tropical")
    );
    assert_eq!(ans.get("ambient_temp_c").map(|s| s.as_str()), Some("30"));
    assert_eq!(ans.get("humidity_pct").map(|s| s.as_str()), Some("80"));

    // Tropical on PETG (Stock nozzle): relative deltas applied on top of base values
    // Base PETG: 245°C nozzle, 80°C bed, 0.80mm @ 40mm/s retract, 16.0 mm³/s MVS, 200 mm/s outer wall
    // Relative Tropical Stock PETG: -12°C nozzle (-> 233°C), -3°C bed (-> 77°C), +0.35mm retract (-> 1.15mm),
    // 0.82x MVS (-> 13.1 mm³/s), 0.85x outer wall (-> 170 mm/s),
    // 0.20mm Spiral Z-Hop (Top Surfaces Only), 40% exhaust, wipe=1, reduce_crossing_wall=1
    let (_, _, _, tropical_lua) = weather.generate_from_answers(&ans, "filament:PETG");
    let (petg_trop_stock, _, _) = execute_transformation_lua(
      &tropical_lua,
      &json!({
        "nozzle_temperature": "245",
        "textured_plate_temp": "80",
        "filament_retraction_length": "0.80",
        "filament_retraction_speed": "40",
        "filament_max_volumetric_speed": "16.0",
        "outer_wall_speed": "200"
      }),
      &json!({
        "filament_type": "PETG",
        "nozzle_type": "hardened_steel",
        "nozzle_material": "Hardened Steel",
        "hotend": "Kobra S1 All-Metal"
      }),
    )
    .expect("tropical PETG stock nozzle executes");
    assert_eq!(petg_trop_stock["nozzle_temperature"], "233");
    assert_eq!(petg_trop_stock["textured_plate_temp"], "77");
    assert_eq!(petg_trop_stock["filament_retraction_length"], "1.15");
    assert_eq!(petg_trop_stock["filament_retraction_speed"], "40");
    assert_eq!(petg_trop_stock["z_hop"], "0.20");
    assert_eq!(petg_trop_stock["z_hop_types"], "Spiral Lift");
    assert_eq!(petg_trop_stock["retract_lift_enforce"], "Top Surfaces Only");
    assert_eq!(petg_trop_stock["additional_cooling_fan_speed"], "40");
    assert_eq!(petg_trop_stock["filament_max_volumetric_speed"], "13.1");
    assert_eq!(petg_trop_stock["wipe"], "1");
    assert_eq!(petg_trop_stock["reduce_crossing_wall"], "1");
    assert_eq!(petg_trop_stock["outer_wall_speed"], "170");

    // MicroSwiss / FlowTech / Diamondback nozzle in Tropical weather is resistant to heat creep:
    // Halves the heat-creep nozzle drop (-7°C instead of -12°C -> 238°C) and preserves 0.92x MVS (14.7 mm³/s vs 13.1 mm³/s)!
    let (petg_trop_ms, _, _) = execute_transformation_lua(
      &tropical_lua,
      &json!({
        "nozzle_temperature": "245",
        "textured_plate_temp": "80",
        "filament_retraction_length": "0.65",
        "filament_retraction_speed": "45",
        "filament_max_volumetric_speed": "16.0",
        "outer_wall_speed": "200"
      }),
      &json!({
        "filament_type": "PETG",
        "nozzle_type": "diamondback",
        "nozzle_material": "Diamondback PCD",
        "hotend": "MicroSwiss FlowTech All-Metal"
      }),
    )
    .expect("tropical PETG MicroSwiss nozzle executes");
    assert_eq!(petg_trop_ms["nozzle_temperature"], "238");
    assert_eq!(petg_trop_ms["textured_plate_temp"], "78");
    assert_eq!(petg_trop_ms["filament_max_volumetric_speed"], "14.7");
    assert_eq!(petg_trop_ms["filament_retraction_length"], "1.00");

    // Tropical on TPU 95A: Zero Z-Hop (0.00), relative +0.35mm retract (0.80 -> 1.15mm), 0.75x retract speed (30 -> 23mm/s), 0.85x MVS (5.0 -> 4.3 mm³/s), -8°C nozzle (220 -> 212°C)
    let (tpu_trop, _, _) = execute_transformation_lua(
      &tropical_lua,
      &json!({
        "nozzle_temperature": "220",
        "textured_plate_temp": "50",
        "filament_retraction_length": "0.80",
        "filament_retraction_speed": "30",
        "filament_max_volumetric_speed": "5.0"
      }),
      &json!({ "filament_type": "TPU 95A" }),
    )
    .expect("tropical TPU executes");
    assert_eq!(tpu_trop["nozzle_temperature"], "212");
    assert_eq!(tpu_trop["textured_plate_temp"], "45");
    assert_eq!(tpu_trop["z_hop"], "0.00");
    assert_eq!(tpu_trop["filament_retraction_length"], "1.15");
    assert_eq!(tpu_trop["filament_retraction_speed"], "23");
    assert_eq!(tpu_trop["filament_max_volumetric_speed"], "4.2");

    // 2. Selecting "cold_dry" preset snaps Ambient Temp Dial to 14°C and Humidity Dial to 25% RH
    weather.apply_answer_change(&mut ans, "climate_preset", "cold_dry".to_string());
    assert_eq!(ans.get("ambient_temp_c").map(|s| s.as_str()), Some("14"));
    assert_eq!(ans.get("humidity_pct").map(|s| s.as_str()), Some("25"));

    let (_, _, _, cold_dry_lua) = weather.generate_from_answers(&ans, "all");
    let (pla_cold, _, _) = execute_transformation_lua(
      &cold_dry_lua,
      &json!({
        "nozzle_temperature": "215",
        "textured_plate_temp": "55",
        "textured_plate_temp_initial_layer": "55",
        "initial_layer_speed": "50",
        "close_fan_the_first_x_layers": "1"
      }),
      &json!({ "filament_type": "PLA" }),
    )
    .expect("cold_dry PLA executes");
    assert_eq!(pla_cold["nozzle_temperature"], "221");
    assert_eq!(pla_cold["textured_plate_temp"], "60");
    assert_eq!(pla_cold["textured_plate_temp_initial_layer"], "65");
    assert_eq!(pla_cold["additional_cooling_fan_speed"], "0");
    assert_eq!(pla_cold["close_fan_the_first_x_layers"], "3");
    assert_eq!(pla_cold["initial_layer_speed"], "35");
    assert_eq!(pla_cold["brim_type"], "outer_only");
    assert_eq!(pla_cold["brim_width"], "5");

    // 3. Selecting "cold_humid" preset snaps Ambient Temp Dial to 15°C and Humidity Dial to 80% RH
    weather.apply_answer_change(&mut ans, "climate_preset", "cold_humid".to_string());
    assert_eq!(ans.get("ambient_temp_c").map(|s| s.as_str()), Some("15"));
    assert_eq!(ans.get("humidity_pct").map(|s| s.as_str()), Some("80"));
    let (_, _, _, cold_humid_lua) = weather.generate_from_answers(&ans, "all");
    let (pla_cold_humid, _, _) = execute_transformation_lua(
      &cold_humid_lua,
      &json!({
        "nozzle_temperature": "215",
        "textured_plate_temp": "55",
        "textured_plate_temp_initial_layer": "55",
        "close_fan_the_first_x_layers": "1"
      }),
      &json!({ "filament_type": "PLA" }),
    )
    .expect("cold_humid PLA executes");
    // Combines Cold-Weather chamber/bed/exhaust/brim with High-Humidity anti-stringing (+5°C cold -3°C humid = +2°C -> 217°C)!
    assert_eq!(pla_cold_humid["nozzle_temperature"], "217");
    assert_eq!(pla_cold_humid["textured_plate_temp_initial_layer"], "65");
    assert_eq!(pla_cold_humid["additional_cooling_fan_speed"], "0");
    assert_eq!(pla_cold_humid["close_fan_the_first_x_layers"], "3");
    assert_eq!(pla_cold_humid["wipe"], "1");
    assert_eq!(pla_cold_humid["reduce_crossing_wall"], "1");
    assert_eq!(pla_cold_humid["brim_width"], "5");

    // 4. Selecting "hot_dry" preset snaps Ambient Temp Dial to 33°C and Humidity Dial to 20% RH
    weather.apply_answer_change(&mut ans, "climate_preset", "hot_dry".to_string());
    assert_eq!(ans.get("ambient_temp_c").map(|s| s.as_str()), Some("33"));
    assert_eq!(ans.get("humidity_pct").map(|s| s.as_str()), Some("20"));
    assert!(weather.validate_answers(&ans, "all").is_ok());

    // 5. Manually dragging the Humidity Dial to 75% automatically flips `climate_preset` to "custom"
    weather.apply_answer_change(&mut ans, "humidity_pct", "75".to_string());
    assert_eq!(
      ans.get("climate_preset").map(|s| s.as_str()),
      Some("custom")
    );
    assert!(weather.validate_answers(&ans, "all").is_ok());
  }

  #[test]
  fn test_tpu_foaming_shore_hardness_curve_and_microswiss_diameters() {
    // 1. Test `cal_tpu_foaming_shore.lua` presets, dials, nozzle thermal offsets, and validation
    let templates = TransformationEngine::get_parameterized_calibration_templates();
    let foaming = templates
      .iter()
      .find(|t| t.id == "cal_tpu_foaming_shore")
      .expect("cal_tpu_foaming_shore exists");

    let mut ans = foaming.default_answers();
    assert_eq!(
      ans.get("shore_preset").map(|s| s.as_str()),
      Some("75a_midsole")
    );
    assert_eq!(ans.get("target_shore_a").map(|s| s.as_str()), Some("75"));

    // Selecting "65a_soft" snaps target_shore_a to 65 and mvs_ceiling to 3.0
    foaming.apply_answer_change(&mut ans, "shore_preset", "65a_soft".to_string());
    assert_eq!(ans.get("target_shore_a").map(|s| s.as_str()), Some("65"));
    assert_eq!(ans.get("mvs_ceiling").map(|s| s.as_str()), Some("3.0"));

    // Execute on Hardened Steel nozzle (+5°C offset -> 235 + 5 = 240°C, 0.650 flow)
    let (_, _, badge, lua_65a) = foaming.generate_from_answers(&ans, "filament:TPU");
    assert!(badge.contains("65A"), "badge: {}", badge);
    let (cfg_hs, _, _) = execute_transformation_lua(
      &lua_65a,
      &json!({
        "nozzle_temperature": "220",
        "filament_flow_ratio": "0.98"
      }),
      &json!({
        "filament_type": "TPU Foaming",
        "nozzle_type": "hardened_steel",
        "nozzle_material": "Hardened Steel"
      }),
    )
    .expect("foaming TPU 65A on Hardened Steel executes");
    assert_eq!(cfg_hs["nozzle_temperature"], "240");
    assert_eq!(cfg_hs["filament_flow_ratio"], "0.650");
    assert_eq!(cfg_hs["filament_max_volumetric_speed"], "3.0");
    assert_eq!(cfg_hs["filament_z_hop"], "0.00");

    // Execute on Diamondback PCD nozzle (-10°C offset -> 235 - 10 = 225°C)
    let (cfg_db, _, _) = execute_transformation_lua(
      &lua_65a,
      &json!({
        "nozzle_temperature": "220",
        "filament_flow_ratio": "0.98"
      }),
      &json!({
        "filament_type": "TPU Foaming",
        "nozzle_type": "diamondback",
        "nozzle_material": "Diamondback PCD"
      }),
    )
    .expect("foaming TPU 65A on Diamondback executes");
    assert_eq!(cfg_db["nozzle_temperature"], "225");
    assert_eq!(cfg_db["filament_flow_ratio"], "0.650");

    // 2. Test FilamentSpool::apply_foaming_shore_hardness
    let catalog = crate::catalog::load_merged_catalog();
    let tpu_foam_entry = catalog
      .filaments
      .iter()
      .find(|f| f.code == "TPU Foaming")
      .expect("TPU Foaming in catalog");
    let mut spool = FilamentSpool::from_catalog_entry(5, tpu_foam_entry, "hardened_steel");
    assert!(spool.is_foaming_tpu());
    spool.apply_foaming_shore_hardness(65, "hardened_steel");
    assert_eq!(spool.effective_foaming_shore_a(), 65);
    assert!(
      spool.name.contains("TPU Foaming 65A"),
      "spool name: {}",
      spool.name
    );
    assert_eq!(spool.print_temp, 240);
    assert!((spool.flow_ratio - 0.65).abs() < 0.001);

    // 3. Test MicroSwiss nozzle size filtering (`supported_nozzle_diameters_for_type`)
    let diams_for = |nt: &str| -> Vec<&'static str> {
      MachineRig::supported_nozzle_diameters_for_type(nt)
        .iter()
        .map(|(d, _)| *d)
        .collect()
    };
    assert_eq!(diams_for("diamondback"), vec!["0.25", "0.4", "0.6", "0.8"]);
    assert_eq!(diams_for("cm2"), vec!["0.4", "0.6", "0.8"]);
    assert_eq!(diams_for("cht_cm2"), vec!["0.4", "0.6", "0.8"]);
    assert_eq!(diams_for("cht"), vec!["0.4", "0.6", "0.8", "1.0"]);
    assert_eq!(
      diams_for("microswiss_brass"),
      vec!["0.2", "0.3", "0.4", "0.6", "0.8", "1.0"]
    );
  }
}
