use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::lua_engine::execute_transformation_lua;
use crate::orca_profile::{DiffItem, OrcaProfile};

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
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StackEvaluationResult {
    pub resulting_profile: OrcaProfile,
    pub diff_items: Vec<DiffItem>,
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
            if let Some(fil) = context.get("filament_type").and_then(|v| v.as_str()) {
                return fil.eq_ignore_ascii_case(rest.trim());
            }
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
                id: "diamondback_nozzle".to_string(),
                name: "Diamondback PCD Nozzle Thermal Tuning".to_string(),
                description: "PCD Diamond features extreme thermal conductivity (5x copper). Lowers necessary melt temp by 10°C while boosting max volumetric speed +20%.".to_string(),
                category: "Hardware Rig".to_string(),
                icon: "💎".to_string(),
                enabled: true,
                condition_expr: "nozzle:diamondback".to_string(),
                lua_code: r#"-- Diamondback Polycrystalline Diamond Nozzle
function apply(config, ctx)
    -- Lower nozzle printing temperature
    if config["nozzle_temperature"] and config["nozzle_temperature"][1] then
        local t = tonumber(config["nozzle_temperature"][1]) or 220
        config["nozzle_temperature"] = { tostring(t - 10) }
        print("[Diamondback] Lowered print temp by 10C (now " .. tostring(t - 10) .. "C)")
    end

    -- Boost volumetric flow rate due to superior heat transfer
    if config["filament_max_volumetric_speed"] and config["filament_max_volumetric_speed"][1] then
        local mvs = tonumber(config["filament_max_volumetric_speed"][1]) or 12.0
        local new_mvs = mvs * 1.20
        config["filament_max_volumetric_speed"] = { string.format("%.1f", new_mvs) }
        print("[Diamondback] Scaled max volumetric speed +20% -> " .. string.format("%.1f", new_mvs) .. " mm3/s")
    end
end
"#.to_string(),
                priority: 10,
            },
            TransformationRule {
                id: "petg_stringing_fix".to_string(),
                name: "Stringing & Oozing Mitigation (PETG)".to_string(),
                description: "Lowers hotend temp by 5°C, boosts retraction length +0.4mm, enables perimeter wipe on loops to prevent whisker formation on travel moves.".to_string(),
                category: "Material Symptom".to_string(),
                icon: "⚡".to_string(),
                enabled: true,
                condition_expr: "filament:PETG".to_string(),
                lua_code: r#"-- PETG Stringing & Oozing Fix
function apply(config, ctx)
    -- Drop temperature by 5°C to reduce melt viscosity ooze
    if config["nozzle_temperature"] and config["nozzle_temperature"][1] then
        local t = tonumber(config["nozzle_temperature"][1]) or 240
        config["nozzle_temperature"] = { tostring(t - 5) }
        print("[StringingFix] Reduced nozzle temp to " .. tostring(t - 5) .. "C")
    end

    -- Increase retraction length
    if config["filament_retraction_length"] and config["filament_retraction_length"][1] then
        local r = tonumber(config["filament_retraction_length"][1]) or 0.8
        config["filament_retraction_length"] = { string.format("%.2f", r + 0.4) }
        print("[StringingFix] Increased retraction to " .. string.format("%.2f", r + 0.4) .. "mm")
    end

    -- Enable wipe on loops
    config["wipe_on_loops"] = "1"
    print("[StringingFix] Activated wipe_on_loops = 1")
end
"#.to_string(),
                priority: 20,
            },
            TransformationRule {
                id: "cold_bed_adhesion".to_string(),
                name: "Poor Bed Adhesion / Cold Plate Squish".to_string(),
                description: "Boosts first layer line width to 0.55mm, increases first layer bed temp +5°C, and slows initial layer speed to 25mm/s for optimal squish.".to_string(),
                category: "Adhesion Symptom".to_string(),
                icon: "🧲".to_string(),
                enabled: false,
                condition_expr: "all".to_string(),
                lua_code: r#"-- Cold Bed Adhesion Booster
function apply(config, ctx)
    config["initial_layer_line_width"] = "0.55"
    config["initial_layer_speed"] = "25"
    
    if config["hot_plate_temp_initial_layer"] and config["hot_plate_temp_initial_layer"][1] then
        local bt = tonumber(config["hot_plate_temp_initial_layer"][1]) or 60
        config["hot_plate_temp_initial_layer"] = { tostring(bt + 5) }
        print("[BedAdhesion] Raised initial bed temp +5C -> " .. tostring(bt + 5) .. "C")
    end
    print("[BedAdhesion] Set first layer width to 0.55mm, speed to 25mm/s")
end
"#.to_string(),
                priority: 30,
            },
            TransformationRule {
                id: "clog_fix_multimaterial".to_string(),
                name: "Filament Change / Transition Anti-Clog".to_string(),
                description: "Eases filament retraction speed before cutter stroke to prevent bulb formation, and scales transition flush volumes +15%.".to_string(),
                category: "Multi-Material".to_string(),
                icon: "🛡️".to_string(),
                enabled: false,
                condition_expr: "all".to_string(),
                lua_code: r#"-- Multi-Material Spool Transition Anti-Clog
function apply(config, ctx)
    -- Lower retract speed to prevent hot end bulb stretching
    config["filament_retraction_speed"] = { "28" }
    -- Increase flush multiplier to guarantee complete purge
    config["flush_multiplier"] = "1.15"
    print("[AntiClog] Retract speed set to 28mm/s, flush multiplier scaled to 1.15x")
end
"#.to_string(),
                priority: 40,
            },
            TransformationRule {
                id: "spaghetti_prevention".to_string(),
                name: "Nozzle Knock & Spaghettization Guard".to_string(),
                description: "Converts infill pattern to Gyroid to eliminate toolhead collisions on crossing perimeters, and adds 0.4mm Z-hop on travel.".to_string(),
                category: "Kinematics Symptom".to_string(),
                icon: "🍝".to_string(),
                enabled: false,
                condition_expr: "all".to_string(),
                lua_code: r#"-- Nozzle Knock / Detachment Guard
function apply(config, ctx)
    config["sparse_infill_pattern"] = "gyroid"
    config["z_hop"] = { "0.4" }
    config["z_hop_types"] = { "Slope" }
    print("[SpaghettiGuard] Infill set to Gyroid, Slope Z-hop enabled at 0.4mm")
end
"#.to_string(),
                priority: 50,
            },
        ]
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

        let mut active_rule_count = 0;
        let mut rule_sources: HashMap<String, String> = HashMap::new();

        for rule in &ordered_rules {
            if !rule.enabled {
                continue;
            }
            if !rule.is_applicable(context) {
                execution_logs.push(format!("⏩ Skipping '{}' (condition not met for context)", rule.name));
                continue;
            }

            active_rule_count += 1;
            execution_logs.push(format!("⚙️ Evaluating rule: '{}'", rule.name));

            match execute_transformation_lua(&rule.lua_code, &current_values, context) {
                Ok((new_val, stdout, modified)) => {
                    current_values = new_val;
                    for line in stdout {
                        execution_logs.push(format!("   {}", line));
                    }
                    for (mod_key, _, _) in modified {
                        rule_sources.insert(mod_key, rule.name.clone());
                    }
                }
                Err(err) => {
                    execution_logs.push(format!("❌ Error in rule '{}': {}", rule.name, err));
                }
            }
        }

        let catalog = OrcaProfile::get_parameter_catalog();
        let mut diff_items = Vec::new();

        if let serde_json::Value::Object(ref final_map) = current_values {
            for (k, v) in final_map {
                let base_val_str = base_profile.get_str(k).unwrap_or_default();
                let curr_val_str = match v {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Array(arr) => arr.get(0).and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    _ => v.to_string(),
                };

                let meta = catalog.get(k);
                let is_modified = base_val_str != curr_val_str;

                if is_modified || meta.is_some() {
                    let label = meta.map(|m| m.label.clone()).unwrap_or_else(|| k.replace('_', " "));
                    let category = meta.map(|m| m.category.clone()).unwrap_or_else(|| "Other".to_string());
                    let unit = meta.map(|m| m.unit.clone()).unwrap_or_default();
                    let source = rule_sources.get(k).cloned().unwrap_or_else(|| {
                        if is_modified { "Active Transformations".to_string() } else { "Stock Default".to_string() }
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

        // Sort diff items: modified first, then by category
        diff_items.sort_by(|a, b| {
            b.is_modified.cmp(&a.is_modified)
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
            let ext_str = match ext_val {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Array(arr) => arr.get(0).and_then(|x| x.as_str()).unwrap_or("").to_string(),
                _ => ext_val.to_string(),
            };
            let base_str = base_profile.get_str(k).unwrap_or_default();
            let derived_str = derived_profile.get_str(k).unwrap_or_default();

            // Conflict occurs if external modified a parameter that differs from what our transformation derived!
            if ext_str != derived_str && ext_str != base_str {
                let rule_name = rules.iter()
                    .find(|r| r.enabled && r.lua_code.contains(k))
                    .map(|r| r.name.clone())
                    .unwrap_or_else(|| "Active Transformation Stack".to_string());

                let label = catalog.get(k).map(|m| m.label.clone()).unwrap_or_else(|| k.clone());

                conflicts.push(ExternalConflict {
                    param_key: k.clone(),
                    param_label: label,
                    base_value: base_str,
                    expected_derived_value: derived_str,
                    actual_external_value: ext_str,
                    modifying_rule_id: rule_name,
                });
            }
        }

        conflicts
    }

    pub fn export_package(rules: &[TransformationRule], active_rig_id: &str) -> String {
        let pkg = FullExportPackage {
            version: "1.0".to_string(),
            export_timestamp_millis: 1729960000,
            rules: rules.to_vec(),
            active_rig_id: active_rig_id.to_string(),
            notes: "OrcaSlicer WebConfig Transformation Stack".to_string(),
        };
        serde_json::to_string_pretty(&pkg).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn import_package(json_str: &str) -> Result<FullExportPackage, String> {
        serde_json::from_str::<FullExportPackage>(json_str)
            .map_err(|e| format!("Failed to parse imported transformations JSON: {}", e))
    }
}
