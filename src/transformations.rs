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
  /// Declarative UI scope filters (Polymer / Nozzle / Process Tier). Empty lists mean "All".
  /// Evaluated in addition to the legacy `condition_expr`.
  #[serde(default)]
  pub scope: RuleScopeFilter,
}

/// Declarative scope filters that any `TransformationRule` can carry so users don't have to write
/// `if ctx.filament_type == ...` boilerplate inside Lua. Every empty list means "All".
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleScopeFilter {
  /// Canonical polymer keys (`PLA`, `PETG`, `PCTG`, `TPU`, `ASA`, `ABS`, `PC`, `PA`, `PHA`) or
  /// `FIBER` for any fiber-filled (`-CF` / `-GF`) material.
  #[serde(default)]
  pub polymers: Vec<String>,
  /// Nozzle orifice diameters (`0.2`, `0.25`, `0.4`, `0.6`, `0.8`, `1.0`).
  #[serde(default)]
  pub nozzle_diameters: Vec<String>,
  /// Nozzle materials (`brass`, `hardened_steel`, `diamondback`, `cht`, `cm2`).
  #[serde(default)]
  pub nozzle_materials: Vec<String>,
  /// Process tiers (`fine`, `quality`, `standard`, `speed`, `draft`).
  #[serde(default)]
  pub process_tiers: Vec<String>,
}

pub const SCOPE_POLYMER_OPTIONS: &[(&str, &str)] = &[
  ("PLA", "PLA"),
  ("PETG", "PETG"),
  ("PCTG", "PCTG"),
  ("TPU", "TPU"),
  ("ASA", "ASA"),
  ("ABS", "ABS"),
  ("PC", "PC"),
  ("PA", "PA"),
  ("PHA", "PHA"),
  ("FIBER", "Fiber-Filled (-CF/-GF)"),
];

pub const SCOPE_NOZZLE_DIAMETER_OPTIONS: &[&str] = &["0.2", "0.25", "0.4", "0.6", "0.8", "1.0"];

pub const SCOPE_NOZZLE_MATERIAL_OPTIONS: &[(&str, &str)] = &[
  ("brass", "Brass"),
  ("hardened_steel", "Hardened Steel"),
  ("diamondback", "Diamondback PCD"),
  ("cht", "CHT"),
  ("cm2", "CM2"),
];

pub const SCOPE_PROCESS_TIER_OPTIONS: &[(&str, &str)] = &[
  ("fine", "0.12mm Fine"),
  ("quality", "0.20mm Quality"),
  ("standard", "0.20mm Standard"),
  ("speed", "0.20mm Speed"),
  ("draft", "0.28mm Draft"),
];

/// Classify a process into one of the 5 declarative scope tiers (`fine`, `quality`, `standard`,
/// `speed`, `draft`) from its layer height and name.
pub fn classify_process_scope_tier(process: &crate::models::ProcessProfile) -> &'static str {
  if process.layer_height <= 0.145 {
    "fine"
  } else if process.layer_height >= 0.255 {
    "draft"
  } else {
    match process.quality_tier() {
      "high_quality" => "quality",
      "high_speed" => "speed",
      _ => "standard",
    }
  }
}

fn polymer_matches_scope_key(material: &str, key: &str) -> bool {
  let up = material.to_uppercase();
  if key.eq_ignore_ascii_case("FIBER") {
    return up.contains("-CF")
      || up.contains("-GF")
      || up.ends_with(" CF")
      || up.ends_with(" GF")
      || up.contains("CARBON")
      || up.contains("GLASS FIBER");
  }
  let canon = FilamentSpool::canonical_polymer_key(material).to_uppercase();
  let key_up = key.to_uppercase();
  if canon == key_up || up == key_up {
    return true;
  }
  // Family match: `PLA Matte`, `PLA+`, `PETG-HF`, `PA6-CF`, `TPU 95A`, `ABS-GF`, ...
  let base = canon
    .split(|c: char| !c.is_ascii_alphanumeric())
    .next()
    .unwrap_or("");
  let base_trim: String = base
    .chars()
    .take_while(|c| c.is_ascii_alphabetic())
    .collect();
  base_trim == key_up
}

fn nozzle_material_matches_scope_key(nozzle_type: &str, nozzle_material: &str, key: &str) -> bool {
  let nt = nozzle_type.to_lowercase();
  let nm = nozzle_material.to_lowercase();
  match key {
    "brass" => {
      (nt.contains("brass") || nm.contains("brass") || nt == "stock")
        && !nt.contains("cht")
        && !nt.contains("cm2")
    }
    "hardened_steel" => nt.contains("hardened") || nm.contains("hardened"),
    "diamondback" => nt.contains("diamondback") || nm.contains("diamondback") || nm.contains("pcd"),
    "cht" => nt.contains("cht") || nm.contains("cht"),
    "cm2" => nt.contains("cm2") || nm.contains("cm2"),
    other => nt == other || nm == other,
  }
}

impl RuleScopeFilter {
  pub fn is_all(&self) -> bool {
    self.polymers.is_empty()
      && self.nozzle_diameters.is_empty()
      && self.nozzle_materials.is_empty()
      && self.process_tiers.is_empty()
  }

  /// Short human-readable summary such as `PETG, PCTG • 0.4mm • Hardened Steel • Fine, Quality`.
  pub fn summary(&self) -> String {
    if self.is_all() {
      return "All Polymers • All Nozzles • All Process Tiers".to_string();
    }
    let lbl = |opts: &[(&str, &str)], v: &str| -> String {
      opts
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(v))
        .map(|(_, l)| l.to_string())
        .unwrap_or_else(|| v.to_string())
    };
    let mut parts = Vec::new();
    if !self.polymers.is_empty() {
      parts.push(
        self
          .polymers
          .iter()
          .map(|p| lbl(SCOPE_POLYMER_OPTIONS, p))
          .collect::<Vec<_>>()
          .join(", "),
      );
    }
    if !self.nozzle_diameters.is_empty() {
      parts.push(
        self
          .nozzle_diameters
          .iter()
          .map(|d| format!("{}mm", d))
          .collect::<Vec<_>>()
          .join(", "),
      );
    }
    if !self.nozzle_materials.is_empty() {
      parts.push(
        self
          .nozzle_materials
          .iter()
          .map(|m| lbl(SCOPE_NOZZLE_MATERIAL_OPTIONS, m))
          .collect::<Vec<_>>()
          .join(", "),
      );
    }
    if !self.process_tiers.is_empty() {
      parts.push(
        self
          .process_tiers
          .iter()
          .map(|t| lbl(SCOPE_PROCESS_TIER_OPTIONS, t))
          .collect::<Vec<_>>()
          .join(", "),
      );
    }
    parts.join(" • ")
  }

  /// Returns `None` if the context is in scope, or `Some(reason)` explaining which filter excluded it.
  pub fn mismatch_reason(&self, context: &serde_json::Value) -> Option<String> {
    if !self.polymers.is_empty() {
      let mut materials: Vec<String> = Vec::new();
      if let Some(f) = context.get("filament_type").and_then(|v| v.as_str()) {
        materials.push(f.to_string());
      }
      if let Some(slots) = context.get("slots").and_then(|v| v.as_array()) {
        for s in slots {
          if let Some(m) = s.get("material_type").and_then(|m| m.as_str()) {
            materials.push(m.to_string());
          }
        }
      }
      if let Some(all) = context.get("all_filament_types").and_then(|v| v.as_array()) {
        for m in all.iter().filter_map(|m| m.as_str()) {
          materials.push(m.to_string());
        }
      }
      // With no filament information in the context, polymer filters cannot be evaluated: treat as in-scope.
      if !materials.is_empty() {
        let hit = materials.iter().any(|m| {
          self
            .polymers
            .iter()
            .any(|k| polymer_matches_scope_key(m, k))
        });
        if !hit {
          let active = context
            .get("filament_type")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| materials.join(", "));
          return Some(format!(
            "Polymer filter [{}] ≠ loaded filament ({})",
            self.polymers.join(", "),
            active
          ));
        }
      }
    }

    if !self.nozzle_diameters.is_empty() {
      let diam = context.get("nozzle_diameter").and_then(|v| {
        v.as_f64()
          .or_else(|| v.as_str().and_then(|s| s.trim().parse::<f64>().ok()))
      });
      if let Some(d) = diam {
        let hit = self
          .nozzle_diameters
          .iter()
          .filter_map(|s| s.trim().parse::<f64>().ok())
          .any(|t| (t - d).abs() < 0.011);
        if !hit {
          return Some(format!(
            "Nozzle diameter filter [{}mm] ≠ active {:.2}mm nozzle",
            self.nozzle_diameters.join(", "),
            d
          ));
        }
      }
    }

    if !self.nozzle_materials.is_empty() {
      let nt = context
        .get("nozzle_type")
        .and_then(|v| v.as_str())
        .unwrap_or("");
      let nm = context
        .get("nozzle_material")
        .and_then(|v| v.as_str())
        .unwrap_or("");
      if !nt.is_empty() || !nm.is_empty() {
        let hit = self
          .nozzle_materials
          .iter()
          .any(|k| nozzle_material_matches_scope_key(nt, nm, k));
        if !hit {
          return Some(format!(
            "Nozzle material filter [{}] ≠ active nozzle ({})",
            self.nozzle_materials.join(", "),
            if nm.is_empty() { nt } else { nm }
          ));
        }
      }
    }

    if !self.process_tiers.is_empty() {
      if let Some(tier) = context.get("process_tier").and_then(|v| v.as_str()) {
        let tier_lc = tier.trim().to_lowercase();
        let matches_tier = self.process_tiers.iter().any(|t| {
          let tl = t.trim().to_lowercase();
          tl == tier_lc
            || ((tl == "0.12mm" || tl == "0.12mm fine" || tl.contains("fine")) && tier_lc == "fine")
            || ((tl == "0.20mm quality" || tl.contains("quality")) && tier_lc == "quality")
            || ((tl == "0.20mm standard" || tl.contains("standard")) && tier_lc == "standard")
            || ((tl == "0.20mm speed" || tl.contains("speed")) && tier_lc == "speed")
            || ((tl == "0.28mm" || tl == "0.28mm draft" || tl.contains("draft"))
              && tier_lc == "draft")
            || (tl == "0.20mm" && matches!(tier_lc.as_str(), "quality" | "standard" | "speed"))
        });
        if !matches_tier {
          return Some(format!(
            "Process tier filter [{}] ≠ evaluated process tier ({})",
            self.process_tiers.join(", "),
            tier
          ));
        }
      }
    }
    None
  }
}

/// A rule that was enabled but skipped because its declarative scope (or legacy `condition_expr`)
/// excluded the current evaluation context.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkippedRuleInfo {
  pub rule_id: String,
  pub rule_name: String,
  pub rule_icon: String,
  pub step_number: usize,
  pub reason: String,
  /// Parameters this rule would have modified if it were in scope (dry-run).
  pub would_touch_keys: Vec<String>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleShadowKind {
  /// A downstream rule absolutely overwrote this step's value: re-running the downstream rule with this
  /// step's change removed yields the identical result, so this step's contribution was forgotten
  /// (`⚡ Overwritten by later rule`). Opposite-direction relative adjustments are NOT shadowing.
  Overwritten,
  /// The downstream rule discarded this step's direct write to the key, but re-derived the value
  /// from OTHER keys this same step produced (e.g. `thermal_conductivity` sets both
  /// `nozzle_temperature` and `nozzle_temperature_HS`; a later wet-filament rule recomputes `_HS`
  /// from `nozzle_temperature`). The step's intent survives through the sibling key, so this is
  /// informational (`♻ Recomputed by later rule`), not a rule-ordering problem.
  Recomputed,
  /// A downstream guard/clamp rule READ this key and passed the value through unchanged
  /// (e.g. `x = min(x, 20)` with `x` already ≤ 20): the value survived because it satisfied the
  /// later rule's constraint, not because it was recomputed. Detected by perturbing the key in the
  /// downstream rule's input — if the output follows the perturbation, the rule reads the key, and
  /// since it left the value untouched it ACCEPTED it. Informational (`✔ Accepted by later rule`).
  Accepted,
  /// The Epilogue Constraint Projection (`[L, U]`, Priority = ∞) or a Priority-∞ Coupled Invariant
  /// clamped this step's output to enforce a hard hardware, thermal, or kinematic domain bound.
  /// Informational (`🛡️ Clamped by Epilogue Constraint`), not a rule-ordering problem.
  Clamped,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuleBeforeAfterChange {
  pub rule_id: String,
  pub rule_name: String,
  #[serde(default)]
  pub rule_icon: String,
  #[serde(default)]
  pub rule_category: String,
  #[serde(default)]
  pub step_number: usize,
  #[serde(default)]
  pub rule_priority: i32,
  pub param_key: String,
  pub param_label: String,
  pub before_value: String,
  pub after_value: String,
  #[serde(default)]
  pub delta_summary: String,
  #[serde(default)]
  pub rule_log: String,
  /// Present when a downstream rule in the stack absolutely overwrote (clobbered) this step's parameter change.
  #[serde(default)]
  pub shadow_kind: Option<RuleShadowKind>,
  #[serde(default)]
  pub shadowed_by_step: Option<usize>,
  #[serde(default)]
  pub shadowed_by_rule_id: Option<String>,
  #[serde(default)]
  pub shadowed_by_rule_name: Option<String>,
  #[serde(default)]
  pub shadowed_final_value: Option<String>,
}

/// Monotonic interval bound $I_k = [L_k, U_k]$ accumulated across the rule stack via
/// `constrain(key, min_val, max_val, reason)`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ParameterBound {
  pub param_key: String,
  #[serde(default)]
  pub min: Option<f64>,
  #[serde(default)]
  pub max: Option<f64>,
  #[serde(default)]
  pub min_by: String,
  #[serde(default)]
  pub max_by: String,
  #[serde(default)]
  pub min_rule_id: String,
  #[serde(default)]
  pub max_rule_id: String,
  #[serde(default)]
  pub min_step: usize,
  #[serde(default)]
  pub max_step: usize,
}

impl ParameterBound {
  /// Returns `true` when the lower bound strictly exceeds the upper bound ($L_k > U_k$),
  /// indicating an infeasible hardware/polymer constraint conflict.
  pub fn is_infeasible(&self) -> bool {
    matches!((self.min, self.max), (Some(l), Some(u)) if l > u + 1e-9)
  }

  pub fn format_interval(&self, unit: &str) -> String {
    let u = if unit.is_empty() {
      String::new()
    } else {
      format!(" {}", unit)
    };
    let fmt_n = |n: f64| -> String {
      if (n - n.round()).abs() < 1e-6 {
        format!("{:.0}{}", n.round(), u)
      } else {
        format!("{:.2}{}", n, u)
      }
    };
    let lo = self.min.map(fmt_n).unwrap_or_else(|| "-∞".to_string());
    let hi = self.max.map(fmt_n).unwrap_or_else(|| "+∞".to_string());
    format!("[{}, {}]", lo, hi)
  }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterProvenance {
  pub param_key: String,
  pub param_label: String,
  pub category: String,
  pub unit: String,
  pub origin_source: String,
  pub origin_value: String,
  pub final_value: String,
  pub net_delta_summary: String,
  pub steps: Vec<RuleBeforeAfterChange>,
  pub is_modified: bool,
  /// Enabled rules that would have touched this parameter but were out of scope for this context.
  #[serde(default)]
  pub skipped_steps: Vec<SkippedRuleInfo>,
  /// True when >= 1 step on this parameter was absolutely overwritten (clobbered) by a later rule.
  #[serde(default)]
  pub has_shadowed_steps: bool,
  /// Active monotonic constraint interval $I_k = [L_k, U_k]$ narrowed by `constrain(...)` calls.
  #[serde(default)]
  pub active_bound: Option<ParameterBound>,
}

impl ParameterProvenance {
  /// Returns the raw scalar string of `final_value` (with the display `unit` suffix stripped).
  pub fn raw_final_scalar(&self) -> String {
    if self.final_value == "(unset)" {
      return String::new();
    }
    let suffix = if self.unit.is_empty() {
      String::new()
    } else {
      format!(" {}", self.unit)
    };
    if !suffix.is_empty() && self.final_value.ends_with(&suffix) {
      self.final_value[..self.final_value.len() - suffix.len()].to_string()
    } else {
      self.final_value.clone()
    }
  }

  /// Verifies the typed ledger chain invariant:
  /// $$x_0 \xrightarrow{\text{Step } 1} x_1 \xrightarrow{\text{Step } 2} \dots \xrightarrow{\text{Epilogue } [L, U]} x_{\text{shipped}}$$
  /// Specifically:
  /// 1. When `steps` is empty, `origin_value == final_value`.
  /// 2. When `steps` is non-empty:
  ///    - `steps[0].before_value == origin_value` ($x_0$)
  ///    - `steps[i].after_value == steps[i + 1].before_value` for all adjacent steps
  ///    - `steps.last().after_value == final_value` ($x_{\text{shipped}}$)
  ///    - Any step immediately preceding an Epilogue clamp (`rule_priority == i32::MAX`) is tagged
  ///      with `shadow_kind == Some(RuleShadowKind::Clamped)` and `shadowed_by_step == Some(epilogue.step_number)`.
  pub fn verify_ledger_chain(&self) -> Result<(), String> {
    if self.steps.is_empty() {
      if self.origin_value != self.final_value {
        return Err(format!(
          "[Ledger Chain Broken] '{}': 0 steps but origin_value ('{}') != final_value ('{}')",
          self.param_key, self.origin_value, self.final_value
        ));
      }
      return Ok(());
    }
    if self.steps[0].before_value != self.origin_value {
      return Err(format!(
        "[Ledger Chain Broken] '{}': step[0] ('{}') before_value ('{}') != origin_value ('{}')",
        self.param_key, self.steps[0].rule_id, self.steps[0].before_value, self.origin_value
      ));
    }
    for i in 0..self.steps.len() - 1 {
      let cur = &self.steps[i];
      let next = &self.steps[i + 1];
      if cur.after_value != next.before_value {
        return Err(format!(
          "[Ledger Chain Broken] '{}': step[{}] ('{}') after_value ('{}') != step[{}] ('{}') before_value ('{}')",
          self.param_key,
          i,
          cur.rule_id,
          cur.after_value,
          i + 1,
          next.rule_id,
          next.before_value
        ));
      }
      if next.rule_priority == i32::MAX {
        if cur.shadow_kind != Some(RuleShadowKind::Clamped) {
          return Err(format!(
            "[Ledger Epilogue Link Missing] '{}': step[{}] ('{}') precedes Epilogue step '{}' but shadow_kind is {:?} (expected Some(Clamped))",
            self.param_key, i, cur.rule_id, next.rule_id, cur.shadow_kind
          ));
        }
        if cur.shadowed_by_step != Some(next.step_number) {
          return Err(format!(
            "[Ledger Epilogue Link Missing] '{}': step[{}] ('{}') shadowed_by_step is {:?} (expected Some({}))",
            self.param_key, i, cur.rule_id, cur.shadowed_by_step, next.step_number
          ));
        }
      }
    }
    let last = self.steps.last().unwrap();
    if last.after_value != self.final_value {
      return Err(format!(
        "[Ledger Chain Broken] '{}': last step ('{}') after_value ('{}') != final_value ('{}')",
        self.param_key, last.rule_id, last.after_value, self.final_value
      ));
    }
    Ok(())
  }
}

pub fn compute_delta_summary(before_raw: &str, after_raw: &str, unit: &str) -> String {
  let clean_before = before_raw.trim().trim_end_matches('%');
  let clean_after = after_raw.trim().trim_end_matches('%');
  let unit_suffix = if unit.is_empty() {
    String::new()
  } else {
    format!(" {}", unit)
  };

  if before_raw == "(unset)" || before_raw.is_empty() {
    let preview = if after_raw.len() > 28 {
      format!("{}…", &after_raw[..28])
    } else {
      after_raw.to_string()
    };
    return format!("Initialized → {}{}", preview, unit_suffix);
  }

  if let (Ok(b), Ok(a)) = (clean_before.parse::<f64>(), clean_after.parse::<f64>()) {
    let diff = a - b;
    if diff.abs() < 1e-6 {
      return "No net change (0)".to_string();
    }
    let sign = if diff > 0.0 { "+" } else { "" };
    let diff_fmt = if (diff.round() - diff).abs() < 1e-4 {
      format!("{}{:.0}{}", sign, diff, unit_suffix)
    } else if (diff * 100.0 - (diff * 100.0).round()).abs() < 1e-3 {
      format!("{}{:.2}{}", sign, diff, unit_suffix)
    } else {
      format!("{}{:.3}{}", sign, diff, unit_suffix)
    };

    if b.abs() > 1e-6 {
      let ratio = a / b;
      let pct = (ratio - 1.0) * 100.0;
      let pct_sign = if pct > 0.0 { "+" } else { "" };
      return format!("{} ({}{:.1}% • {:.2}×)", diff_fmt, pct_sign, pct, ratio);
    }
    return diff_fmt;
  }

  if before_raw.contains('\n') || after_raw.contains('\n') || after_raw.len() > 42 {
    let before_lines = before_raw.lines().count();
    let after_lines = after_raw.lines().count();
    return format!(
      "Rewrote multi-line G-code/script ({} → {} lines)",
      before_lines, after_lines
    );
  }

  format!("Set {} → {}", before_raw, after_raw)
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
  pub provenance_items: Vec<ParameterProvenance>,
  pub flush_matrix: Vec<String>,
  pub safe_speed_ceiling: f64,
  pub execution_logs: Vec<String>,
  pub active_rule_count: usize,
  pub total_rule_count: usize,
  /// Enabled rules skipped because their declarative scope / condition excluded this context.
  #[serde(default)]
  pub skipped_rules: Vec<SkippedRuleInfo>,
  /// Monotonic constraint lattice $I_k = [L_k, U_k]$ accumulated across the stack.
  #[serde(default)]
  pub parameter_bounds: HashMap<String, ParameterBound>,
  /// Constraints where $L_k > U_k$ (e.g. polymer min extrude temp > hotend thermal ceiling).
  #[serde(default)]
  pub infeasible_constraints: Vec<ParameterBound>,
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
  /// Combined scope check: legacy `condition_expr` AND declarative `scope` filters.
  /// Returns `Some(reason)` when the rule must be skipped for this context.
  pub fn scope_skip_reason(&self, context: &serde_json::Value) -> Option<String> {
    if !self.is_applicable(context) {
      return Some(format!(
        "Condition '{}' not met for active context",
        self.condition_expr.trim()
      ));
    }
    self.scope.mismatch_reason(context)
  }

  pub fn is_applicable(&self, context: &serde_json::Value) -> bool {
    let cond = self.condition_expr.trim();
    if cond.is_empty() || cond == "all" || cond == "*" {
      return true;
    }

    if let Some(rest) = cond.strip_prefix("filament:") {
      let target_fil = rest.trim();
      let matches_fil = |m: &str| -> bool {
        m.eq_ignore_ascii_case(target_fil)
          || FilamentSpool::canonical_polymer_key(m).eq_ignore_ascii_case(target_fil)
      };
      if let Some(fil) = context.get("filament_type").and_then(|v| v.as_str()) {
        if matches_fil(fil) {
          return true;
        }
      }
      if let Some(slots) = context.get("slots").and_then(|v| v.as_array()) {
        if slots.iter().any(|s| {
          s.get("material_type")
            .and_then(|m| m.as_str())
            .map(&matches_fil)
            .unwrap_or(false)
        }) {
          return true;
        }
      }
      if let Some(all_fils) = context.get("all_filament_types").and_then(|v| v.as_array()) {
        if all_fils
          .iter()
          .any(|m| m.as_str().map(&matches_fil).unwrap_or(false))
        {
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
        description: "Dynamically compensates nozzle temperature, flow ratio, and retraction based on nozzle material (Hardened Steel +5°C matching Anycubic PETG - hotbed 235°C; Diamondback PCD piecewise-linear −5…−11°C curve by polymer band + quadratic die-swell flow trim ≤1.2%; Bi-Metal CHT +25% max flow).".to_string(),
        category: "Dynamic Combination Rule".to_string(),
        icon: "💎".to_string(),
        enabled: true,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/thermal_conductivity.lua").to_string(),
        priority: 10,
        scope: crate::transformations::RuleScopeFilter::default(),
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
        scope: crate::transformations::RuleScopeFilter::default(),
      },
      TransformationRule {
        id: "anycubic_pctg_as_petg".to_string(),
        name: "Anycubic PCTG → PETG Filament Type Declaration".to_string(),
        description: "AnycubicSlicerNext's hard-coded filament_type list lacks PCTG: on Anycubic rigs, PCTG-family spools are exported as filament_type = PETG. Nothing else changes — nozzle/bed temps and nozzle_temperature_range_low/high stay on PCTG limits (240–275°C), and PCTG-scoped rules still apply.".to_string(),
        category: "Machine Compatibility Rule".to_string(),
        icon: "🏷️".to_string(),
        enabled: true,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/anycubic_pctg_as_petg.lua").to_string(),
        priority: 13,
        scope: crate::transformations::RuleScopeFilter::default(),
      },
      TransformationRule {
        id: "line_width_geometry".to_string(),
        name: "Nozzle-Proportional Line Width & Extrusion Geometry Optimizer".to_string(),
        description: "Sets every per-feature line width as a % of the active nozzle diameter: Initial Layer 125% (wide adhesion bead), Outer Wall 105%, Top Surface 95% (crisp tops & lettering), Inner Wall & Sparse Infill 125% (thicker, stronger, faster internal ribs). Rescales automatically on 0.2–1.0mm nozzle swaps.".to_string(),
        category: "Core Slicer Config Pack".to_string(),
        icon: "📏".to_string(),
        enabled: true,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/parametric/line_width_geometry.lua").to_string(),
        priority: 15,
        scope: crate::transformations::RuleScopeFilter::default(),
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
        priority: 98,
        scope: crate::transformations::RuleScopeFilter::default(),
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
        scope: crate::transformations::RuleScopeFilter::default(),
      },
      TransformationRule {
        id: "symptom_stringing_diamondback".to_string(),
        name: "Stringing & Oozing Mitigation (MicroSwiss / High-Conductivity Nozzle)".to_string(),
        description: "High thermal conductivity of MicroSwiss / PCD Diamond keeps the melt zone less viscous at the tip. Composes relatively on the P10/P12 curves: elastomer-gated −2°C trim in the PLA/PETG band only, clamped retraction recovery (≤ +0.07mm), quadratic speed damping to 36–38 mm/s, orifice-scaled filament Z-Hop (Spiral), travel accel boosted toward 12000 mm/s², wipe_on_loops=1.".to_string(),
        category: "Default Hardware Rule".to_string(),
        icon: "⚡".to_string(),
        enabled: true,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/symptom_stringing_diamondback.lua").to_string(),
        priority: 25,
        scope: crate::transformations::RuleScopeFilter::default(),
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
        scope: crate::transformations::RuleScopeFilter::default(),
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
        scope: crate::transformations::RuleScopeFilter::default(),
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
        scope: crate::transformations::RuleScopeFilter::default(),
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
        scope: crate::transformations::RuleScopeFilter::default(),
      },
      TransformationRule {
        id: "filament_pctg_antistringing".to_string(),
        name: "PCTG Stringing & Ooze Mitigation".to_string(),
        description: "Targets PCTG's high melt elasticity: sets 1.00mm @ 45mm/s retraction, 40% retract-before-wipe over a 2.0mm wipe distance, 0.10mm Spiral Z-Hop, 10000 mm/s² travel acceleration, and -3°C anti-ooze trim.".to_string(),
        category: "Filament Tuning Rule".to_string(),
        icon: "🧵".to_string(),
        enabled: true,
        condition_expr: "filament:PCTG".to_string(),
        lua_code: include_str!("../transformations/filament_pctg_antistringing.lua").to_string(),
        priority: 48,
        scope: crate::transformations::RuleScopeFilter::default(),
      },
      TransformationRule {
        id: "support_tuning".to_string(),
        name: "Support Detachability & Tree Tuning Pack".to_string(),
        description: "Layer-height & polymer-aware `support_top_z_distance` (`layer_height + 0.02mm` PLA vs. `layer_height + 0.06mm` sticky PETG/PCTG/TPU, or `0.00mm` dissimilar PLA↔PETG), `support_type` (`tree(auto)` / `normal(auto)`), `support_style` (`tree_slim`, `tree_hybrid`, `snug`), and 3-layer roof interface (`0.15mm` spacing @ `60 mm/s`).".to_string(),
        category: "Core Slicer Config Pack".to_string(),
        icon: "🏔️".to_string(),
        enabled: false,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/parametric/support_tuning.lua").to_string(),
        priority: 49,
        scope: crate::transformations::RuleScopeFilter::default(),
      },
      TransformationRule {
        id: "ironing_tuning".to_string(),
        name: "Top Surface Ironing & Gloss Pack".to_string(),
        description: "Polymer-aware `ironing_flow` (`12%` PLA, `18%` PETG/PCTG, `22%` ASA/ABS/PC), `ironing_type` (`topmost` / `top` / `all solid`), `ironing_speed` (`30–40 mm/s`), `ironing_spacing` (`0.12–0.15mm`), `top_surface_pattern = monotonicline`, and `only_one_wall_top = 1`.".to_string(),
        category: "Core Slicer Config Pack".to_string(),
        icon: "✨".to_string(),
        enabled: false,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/parametric/ironing_tuning.lua").to_string(),
        priority: 50,
        scope: crate::transformations::RuleScopeFilter::default(),
      },
      TransformationRule {
        id: "dimensional_precision".to_string(),
        name: "Dimensional Tolerance & Mechanical Fit Pack".to_string(),
        description: "Tunes mechanical tolerances for bearings, dowels, and M3/M5 hardware: `wall_sequence` (`inner-outer-inner` or `outer-inner`), `precise_outer_wall = 1`, `xy_hole_compensation` (`+0.08mm`), `xy_contour_compensation` (`-0.02mm`), and `elefant_foot_compensation` (`0.15mm`).".to_string(),
        category: "Core Slicer Config Pack".to_string(),
        icon: "📐".to_string(),
        enabled: false,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/parametric/dimensional_precision.lua").to_string(),
        priority: 51,
        scope: crate::transformations::RuleScopeFilter::default(),
      },
      TransformationRule {
        id: "bridge_overhang_tuning".to_string(),
        name: "Bridges & Steep Overhangs Pack".to_string(),
        description: "Anti-sag `bridge_flow` (`0.92×` strand tension), `bridge_speed` (`28 mm/s`), progressive 4-stage overhang speed curve (`50 → 35 → 22 → 15 mm/s` across `10–25%`..`75–100%` overhangs), `detect_overhang_wall = 1`, and `overhang_fan_speed = 100%`.".to_string(),
        category: "Core Slicer Config Pack".to_string(),
        icon: "🌉".to_string(),
        enabled: false,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/parametric/bridge_overhang_tuning.lua").to_string(),
        priority: 56,
        scope: crate::transformations::RuleScopeFilter::default(),
      },
      TransformationRule {
        id: "structural_strength".to_string(),
        name: "Structural Strength & Z-Axis Inter-Layer Bonding Pack".to_string(),
        description: "Load-bearing part tuning: nozzle +5–8°C (clamped per spool to each polymer's safe max), max part-cooling fan -20–35%, 4–6 wall loops + alternate_extra_wall, 5–6 top/bottom shells, 20–25% infill/wall overlap, and isotropic gyroid / crosshatch infill.".to_string(),
        category: "Core Slicer Config Pack".to_string(),
        icon: "💪".to_string(),
        enabled: false,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/parametric/structural_strength.lua").to_string(),
        priority: 57,
        scope: crate::transformations::RuleScopeFilter::default(),
      },
      TransformationRule {
        id: "outer_wall_consistency".to_string(),
        name: "CoreXY Outer-Wall Speed Consistency & VFA / Gloss-Banding Guard".to_string(),
        description: "Eliminates CoreXY 70–130 mm/s belt/stepper VFA ripples and Benchy hull-line gloss banding: sets dont_slow_down_outer_wall = 1 (auto-disabled to 0 on 0.12mm Fine high-MVS PLA for miniatures & delicate 3mm spires) and steps outer_wall_speed down to 55–60 mm/s whenever MVS or Silk finish cannot sustain >= 145 mm/s.".to_string(),
        category: "Core Slicer Config Pack".to_string(),
        icon: "〰️".to_string(),
        enabled: true,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/parametric/outer_wall_consistency.lua").to_string(),
        priority: 58,
        scope: crate::transformations::RuleScopeFilter::default(),
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
        scope: crate::transformations::RuleScopeFilter::default(),
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
        scope: crate::transformations::RuleScopeFilter::default(),
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
        scope: crate::transformations::RuleScopeFilter::default(),
      },
      TransformationRule {
        id: "sym_wet_filament".to_string(),
        name: "Wet / Damp Filament — Humidity (% RH) Auto-Tuner (1st-Layer Holes, Mis-Adhesion & Stringing)".to_string(),
        description: "Set your ACE Pro / drybox hygrometer humidity (% RH, e.g. 40% RH) to automatically lower upper-layer nozzle temp (-3°C at 40% RH while keeping 1st layer at full temp), seal 1st-layer steam pinholes (+4°C 1st-layer bed, 0.56mm width @ 35 mm/s, 1.04× bottom flow), trim upper flow (-0.01), and add +0.15mm anti-steam retraction (45% pre-wipe, Spiral Z-Hop).".to_string(),
        category: "Filament Tuning Rule".to_string(),
        icon: "💧".to_string(),
        enabled: false,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/parametric/sym_wet_filament.lua").to_string(),
        priority: 55,
        scope: crate::transformations::RuleScopeFilter::default(),
      },
      TransformationRule {
        id: "mach_fast_start_gcode".to_string(),
        name: "⚠️ Startup G-Code — Rig-Specific Fast-Start & Poop-Saver (POTENTIALLY DANGEROUS)".to_string(),
        description: "⚠️ POTENTIALLY DANGEROUS: Rig-specific G-code invoked at print startup (plus end / toolchange). Kobra S1: unpacks `G9111` into explicit Kobra OS sub-macros (direct 140°C preheat, 1–14× `WIPE_NOZZLE1`, `G28 W`, optional `LEVIQ2_AUTO_ZOFFSET`, saved bed mesh, front or KAMP prime line). Kobra S1 Max: keeps stock `G9111` + ABS heat-soak, plate/PETG Z-offset, anti-ooze end. Bambu Lab X1/P1: Bambu fast-start. Other printers keep their stock G-code. Never home Z above 140°C or skip Z-offset after swapping build plates!".to_string(),
        category: "Machine Start/End G-Code (⚠️ Potentially Dangerous)".to_string(),
        icon: "⚡".to_string(),
        enabled: false,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/parametric/mach_fast_start_gcode.lua").to_string(),
        priority: 58,
        scope: crate::transformations::RuleScopeFilter::default(),
      },
    ]
  }

  /// Curated rule templates for the interactive "Create / Add New Lua Rule" builder in `WizardTab`.
  pub fn get_rule_templates() -> Vec<TransformationRule> {
    vec![
      TransformationRule {
        id: "line_width_geometry".to_string(),
        name: "Nozzle-Proportional Line Width & Extrusion Geometry Optimizer".to_string(),
        description: "Per-feature line widths as % of nozzle diameter: Initial 125%, Outer 105%, Top 95%, Inner Wall & Sparse Infill 125% (Balanced / Strength / Fine Detail presets).".to_string(),
        category: "Core Slicer Config Pack".to_string(),
        icon: "📏".to_string(),
        enabled: true,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/parametric/line_width_geometry.lua").to_string(),
        priority: 15,
        scope: crate::transformations::RuleScopeFilter::default(),
      },
      TransformationRule {
        id: "support_tuning".to_string(),
        name: "Support Detachability & Tree Tuning Pack".to_string(),
        description: "Layer-height & polymer-aware `support_top_z_distance` (`layer_height + 0.02mm` PLA vs. `layer_height + 0.06mm` sticky PETG/PCTG/TPU, or `0.00mm` dissimilar PLA↔PETG), `support_type` (`tree(auto)` / `normal(auto)`), `support_style` (`tree_slim`, `tree_hybrid`, `snug`), and 3-layer roof interface.".to_string(),
        category: "Core Slicer Config Pack".to_string(),
        icon: "🏔️".to_string(),
        enabled: true,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/parametric/support_tuning.lua").to_string(),
        priority: 49,
        scope: crate::transformations::RuleScopeFilter::default(),
      },
      TransformationRule {
        id: "ironing_tuning".to_string(),
        name: "Top Surface Ironing & Gloss Pack".to_string(),
        description: "Polymer-aware `ironing_flow` (`12%` PLA, `18%` PETG/PCTG, `22%` ASA/ABS/PC), `ironing_type` (`topmost` / `top` / `all solid`), `ironing_speed`, `ironing_spacing`, and `top_surface_pattern = monotonicline`.".to_string(),
        category: "Core Slicer Config Pack".to_string(),
        icon: "✨".to_string(),
        enabled: true,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/parametric/ironing_tuning.lua").to_string(),
        priority: 50,
        scope: crate::transformations::RuleScopeFilter::default(),
      },
      TransformationRule {
        id: "dimensional_precision".to_string(),
        name: "Dimensional Tolerance & Mechanical Fit Pack".to_string(),
        description: "Tunes mechanical fit: `wall_sequence` (`inner-outer-inner` or `outer-inner`), `precise_outer_wall = 1`, `xy_hole_compensation` (`+0.08mm`), `xy_contour_compensation` (`-0.02mm`), and `elefant_foot_compensation` (`0.15mm`).".to_string(),
        category: "Core Slicer Config Pack".to_string(),
        icon: "📐".to_string(),
        enabled: true,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/parametric/dimensional_precision.lua").to_string(),
        priority: 51,
        scope: crate::transformations::RuleScopeFilter::default(),
      },
      TransformationRule {
        id: "bridge_overhang_tuning".to_string(),
        name: "Bridges & Steep Overhangs Pack".to_string(),
        description: "Anti-sag `bridge_flow` (`0.92×`), `bridge_speed` (`28 mm/s`), progressive 4-stage overhang speed curve (`50 → 35 → 22 → 15 mm/s`), and `overhang_fan_speed = 100%`.".to_string(),
        category: "Core Slicer Config Pack".to_string(),
        icon: "🌉".to_string(),
        enabled: true,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/parametric/bridge_overhang_tuning.lua").to_string(),
        priority: 56,
        scope: crate::transformations::RuleScopeFilter::default(),
      },
      TransformationRule {
        id: "structural_strength".to_string(),
        name: "Structural Strength & Z-Axis Inter-Layer Bonding Pack".to_string(),
        description: "Load-bearing part tuning: nozzle +5–8°C (clamped per spool to each polymer's safe max), max part-cooling fan -20–35%, 4–6 wall loops + alternate_extra_wall, 5–6 top/bottom shells, 20–25% infill/wall overlap, and isotropic gyroid / crosshatch infill.".to_string(),
        category: "Core Slicer Config Pack".to_string(),
        icon: "💪".to_string(),
        enabled: true,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/parametric/structural_strength.lua").to_string(),
        priority: 57,
        scope: crate::transformations::RuleScopeFilter::default(),
      },
      TransformationRule {
        id: "outer_wall_consistency".to_string(),
        name: "CoreXY Outer-Wall Speed Consistency & VFA / Gloss-Banding Guard".to_string(),
        description: "Eliminates CoreXY 70–130 mm/s belt/stepper VFA ripples and Benchy hull-line gloss banding: sets dont_slow_down_outer_wall = 1 (with a Spire-Safe 0 toggle for miniatures & delicate 3mm tips at 200 mm/s) and cleanly steps outer_wall_speed down to 55–60 mm/s whenever MVS cannot sustain >= 145 mm/s.".to_string(),
        category: "Core Slicer Config Pack".to_string(),
        icon: "〰️".to_string(),
        enabled: true,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/parametric/outer_wall_consistency.lua").to_string(),
        priority: 58,
        scope: crate::transformations::RuleScopeFilter::default(),
      },
      TransformationRule {
        id: "mach_fast_start_gcode".to_string(),
        name: "⚠️ Startup G-Code — Rig-Specific Fast-Start & Poop-Saver (POTENTIALLY DANGEROUS)".to_string(),
        description: "⚠️ POTENTIALLY DANGEROUS: Rig-specific G-code invoked at print startup (plus end / toolchange). Kobra S1: unpacked `G9111` sub-macros. Kobra S1 Max: stock `G9111` + ABS heat-soak, plate/PETG Z-offset, anti-ooze end. Bambu Lab X1/P1: Bambu fast-start. Other printers keep their stock G-code unchanged.".to_string(),
        category: "Machine Start/End G-Code (⚠️ Potentially Dangerous)".to_string(),
        icon: "⚡".to_string(),
        enabled: true,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/parametric/mach_fast_start_gcode.lua").to_string(),
        priority: 58,
        scope: crate::transformations::RuleScopeFilter::default(),
      },
      TransformationRule {
        id: "sym_wet_filament".to_string(),
        name: "Wet / Damp Filament — Humidity (% RH) Auto-Tuner (1st-Layer Holes, Mis-Adhesion & Stringing)".to_string(),
        description: "Set your ACE Pro / drybox hygrometer humidity (% RH, e.g. 40% RH) to automatically lower upper-layer nozzle temp (-3°C at 40% RH while keeping 1st layer at full temp), seal 1st-layer steam pinholes (+4°C 1st-layer bed, 0.56mm width @ 35 mm/s, 1.04× bottom flow), trim upper flow (-0.01), and add +0.15mm anti-steam retraction (45% pre-wipe, Spiral Z-Hop).".to_string(),
        category: "Filament Tuning Rule".to_string(),
        icon: "💧".to_string(),
        enabled: true,
        condition_expr: "all".to_string(),
        lua_code: include_str!("../transformations/parametric/sym_wet_filament.lua").to_string(),
        priority: 55,
        scope: crate::transformations::RuleScopeFilter::default(),
      },
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
        scope: crate::transformations::RuleScopeFilter::default(),
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
        scope: crate::transformations::RuleScopeFilter::default(),
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
        scope: crate::transformations::RuleScopeFilter::default(),
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
        scope: crate::transformations::RuleScopeFilter::default(),
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
        scope: crate::transformations::RuleScopeFilter::default(),
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
        scope: crate::transformations::RuleScopeFilter::default(),
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
        scope: crate::transformations::RuleScopeFilter::default(),
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
        scope: crate::transformations::RuleScopeFilter::default(),
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
        | "anycubic_pctg_as_petg"
        | "line_width_geometry"
        | "outer_wall_consistency"
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
      scope: crate::transformations::RuleScopeFilter::default(),
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

  /// Seeds a base profile and Lua evaluation context with a specific `ProcessProfile`
  /// (`layer_height`, line width, wall/infill speeds, and declarative `process_tier`).
  pub fn seed_process_inputs(
    base_profile: &OrcaProfile,
    context: &serde_json::Value,
    process: &crate::models::ProcessProfile,
  ) -> (OrcaProfile, serde_json::Value) {
    let mut working_base = base_profile.clone();
    working_base.set_str("layer_height", &format!("{:.2}", process.layer_height));
    working_base.set_str("line_width", &format!("{:.2}", process.line_width));
    working_base.set_str(
      "outer_wall_line_width",
      &format!("{:.2}", process.line_width),
    );
    working_base.set_str(
      "outer_wall_speed",
      &format!("{:.0}", process.outer_wall_speed),
    );
    working_base.set_str(
      "inner_wall_speed",
      &format!("{:.0}", process.inner_wall_speed),
    );
    working_base.set_str(
      "sparse_infill_speed",
      &format!("{:.0}", process.sparse_infill_speed),
    );
    working_base.set_str(
      "initial_layer_speed",
      &format!("{:.0}", process.initial_layer_speed),
    );
    working_base.set_str(
      "default_acceleration",
      &format!("{:.0}", process.default_acceleration),
    );
    let mut ctx = context.clone();
    if let serde_json::Value::Object(ref mut m) = ctx {
      m.insert(
        "layer_height".to_string(),
        serde_json::json!(process.layer_height),
      );
      m.insert(
        "line_width".to_string(),
        serde_json::json!(process.line_width),
      );
      m.insert(
        "process_tier".to_string(),
        serde_json::json!(classify_process_scope_tier(process)),
      );
      m.insert("process_name".to_string(), serde_json::json!(process.name));
      m.insert("_process_seeded".to_string(), serde_json::json!(true));
    }
    (working_base, ctx)
  }

  /// Evaluates the full Lua stack independently for every `ProcessProfile`, so Process-Tier-scoped
  /// rules and layer-height-dependent rules produce tailored overrides in each exported preset.
  /// Returns `process.id -> resulting override map`.
  pub fn evaluate_stack_per_process(
    base_profile: &OrcaProfile,
    rules: &[TransformationRule],
    context: &serde_json::Value,
    processes: &[crate::models::ProcessProfile],
  ) -> HashMap<String, serde_json::Map<String, serde_json::Value>> {
    let mut out = HashMap::new();
    for proc in processes {
      let (base, ctx) = Self::seed_process_inputs(base_profile, context, proc);
      let res = Self::evaluate_stack(&base, rules, &ctx);
      out.insert(proc.id.clone(), res.resulting_profile.values);
    }
    out
  }

  /// Enforces the **Exhaustive Key Routing Registry + Zero-Drop Invariant**:
  /// Every parameter key touched by a Lua rule must be routed to an exported OrcaSlicer artifact
  /// (`Machine`, `Process`, `Filament`, or `ProjectFlush`) or explicitly declared `InternalMeta`
  /// (`_`-prefixed side-channels / `computed_safe_speed_ceiling`).
  /// Any unrouted or shadowed dual-scope write fails immediately with
  /// `"Unrouted phantom write in rule <id>: key <key>"`.
  pub fn validate_lua_rule_writes(
    rule_id: &str,
    modified: &[(String, String, String)],
  ) -> Result<(), String> {
    for (mod_key, _, _) in modified {
      if crate::threemf::classify_key_destination(mod_key).is_none() {
        return Err(format!(
          "Unrouted phantom write in rule {}: key {}",
          rule_id, mod_key
        ));
      }
    }
    Ok(())
  }

  /// Canonical per-key OrcaSlicer schema formatter applied once during the Phase 3 Epilogue Pass
  /// after all Phase 2 (`P1 .. PN`) rules have composed in full IEEE-754 floating-point precision.
  ///
  /// - `"%.0f"` (integer rounding) for temperatures (`*temperature*`, `*_plate_temp*`), linear speeds
  ///   (`*_speed`), accelerations (`*_acceleration`), fan percentages (`fan_min_speed*`, `fan_max_speed*`,
  ///   `overhang_fan_speed`, `*_fan_speed`), layer counts (`*_layers`, `wall_loops`), and HRC.
  /// - `"%.1f"` for volumetric speeds (`filament_max_volumetric_speed`, `max_volumetric_speed`).
  /// - `"%.2f"` for retraction lengths, Z-hop distances, wipe distances, line widths, layer heights,
  ///   support Z distances, XY/elephant-foot compensations, shell thicknesses, and bridge flow.
  /// - `"%.3f"` for `pressure_advance` and sub-percent `filament_flow_ratio` trims (or `"%.2f"` when
  ///   the third decimal is zero and the reference baseline used 2 decimals).
  pub fn format_orca_schema_scalar(param_key: &str, val: f64, reference_str: &str) -> String {
    let ref_trimmed = reference_str.trim();
    let ref_decimals = if let Some(dot_idx) = ref_trimmed.find('.') {
      ref_trimmed[dot_idx + 1..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .count()
    } else {
      0
    };
    if matches!(
      param_key,
      "nozzle_temperature"
        | "nozzle_temperature_initial_layer"
        | "nozzle_temperature_HS"
        | "nozzle_temperature_initial_layer_HS"
        | "nozzle_temperature_BRASS"
        | "nozzle_temperature_initial_layer_BRASS"
        | "nozzle_temperature_range_low"
        | "nozzle_temperature_range_high"
        | "hot_plate_temp"
        | "hot_plate_temp_initial_layer"
        | "textured_plate_temp"
        | "textured_plate_temp_initial_layer"
        | "cool_plate_temp"
        | "cool_plate_temp_initial_layer"
        | "eng_plate_temp"
        | "eng_plate_temp_initial_layer"
        | "textured_cool_plate_temp"
        | "textured_cool_plate_temp_initial_layer"
        | "supertack_plate_temp"
        | "supertack_plate_temp_initial_layer"
        | "chamber_temperature"
        | "temperature_vitrification"
        | "filament_retraction_speed"
        | "retraction_speed"
        | "filament_deretraction_speed"
        | "deretraction_speed"
        | "outer_wall_speed"
        | "inner_wall_speed"
        | "small_perimeter_speed"
        | "sparse_infill_speed"
        | "internal_solid_infill_speed"
        | "top_surface_speed"
        | "gap_infill_speed"
        | "support_speed"
        | "support_interface_speed"
        | "bridge_speed"
        | "internal_bridge_speed"
        | "initial_layer_speed"
        | "initial_layer_infill_speed"
        | "travel_speed"
        | "ironing_speed"
        | "slow_down_min_speed"
        | "overhang_1_4_speed"
        | "overhang_2_4_speed"
        | "overhang_3_4_speed"
        | "overhang_4_4_speed"
        | "default_acceleration"
        | "outer_wall_acceleration"
        | "inner_wall_acceleration"
        | "initial_layer_acceleration"
        | "top_surface_acceleration"
        | "travel_acceleration"
        | "sparse_infill_acceleration"
        | "internal_solid_infill_acceleration"
        | "fan_min_speed"
        | "fan_min_speed_BRASS"
        | "fan_min_speed_HS"
        | "fan_max_speed"
        | "fan_max_speed_BRASS"
        | "fan_max_speed_HS"
        | "overhang_fan_speed"
        | "additional_cooling_fan_speed"
        | "during_print_exhaust_fan_speed"
        | "complete_print_exhaust_fan_speed"
        | "chamber_fan_speed"
        | "close_fan_the_first_x_layers"
        | "slow_down_layer_time"
        | "fan_cooling_layer_time"
        | "wall_loops"
        | "top_shell_layers"
        | "bottom_shell_layers"
        | "raft_layers"
        | "elefant_foot_compensation_layers"
        | "support_interface_top_layers"
        | "support_interface_bottom_layers"
        | "nozzle_hrc"
        | "required_nozzle_HRC"
        | "max_travel_detour_distance"
        | "brim_width"
        | "scarf_Joint_seam_steps"
        | "scarf_angle_threshold"
        | "scarf_joint_speed"
        | "tree_support_branch_angle"
        | "support_threshold_angle"
    ) {
      return format!("{:.0}", val);
    }
    if matches!(
      param_key,
      "filament_max_volumetric_speed" | "max_volumetric_speed"
    ) {
      return format!("{:.1}", val);
    }
    if matches!(param_key, "pressure_advance" | "xy_hole_compensation" | "xy_contour_compensation") {
      return format!("{:.3}", val);
    }
    if param_key == "filament_flow_ratio" {
      let f3 = format!("{:.3}", val);
      if ref_decimals == 2 && f3.ends_with('0') {
        return format!("{:.2}", val);
      }
      return f3;
    }
    if matches!(
      param_key,
      "bottom_solid_infill_flow_ratio"
        | "top_solid_infill_flow_ratio"
        | "bridge_flow"
        | "filament_retraction_length"
        | "retraction_length"
        | "filament_z_hop"
        | "z_hop"
        | "layer_height"
        | "initial_layer_print_height"
        | "line_width"
        | "outer_wall_line_width"
        | "inner_wall_line_width"
        | "initial_layer_line_width"
        | "top_surface_line_width"
        | "sparse_infill_line_width"
        | "internal_solid_infill_line_width"
        | "support_line_width"
        | "support_top_z_distance"
        | "support_bottom_z_distance"
        | "support_interface_spacing"
        | "support_object_xy_distance"
        | "top_shell_thickness"
        | "bottom_shell_thickness"
        | "elefant_foot_compensation"
        | "ironing_spacing"
        | "flush_multiplier"
        | "tree_support_branch_distance"
        | "tree_support_branch_diameter"
    ) {
      return format!("{:.2}", val);
    }
    if ref_decimals > 0 {
      format!("{:.prec$}", val, prec = ref_decimals)
    } else if (val - val.round()).abs() < 1e-6 {
      format!("{:.0}", val.round())
    } else if param_key.contains("flow") || param_key.contains("pressure_advance") {
      format!("{:.3}", val)
    } else {
      format!("{:.2}", val)
    }
  }

  pub fn evaluate_stack(
    base_profile: &OrcaProfile,
    rules: &[TransformationRule],
    context: &serde_json::Value,
  ) -> StackEvaluationResult {
    let mut initial_map = base_profile.values.clone();
    let mut base_snapshot_map = serde_json::Map::new();
    for (k, v) in &base_profile.values {
      if k != "_bounds" && k != "_coupled_state" && k != "_base" && k != "_defer_quantization" {
        base_snapshot_map.insert(k.clone(), v.clone());
      }
    }
    initial_map.insert(
      "_base".to_string(),
      serde_json::Value::Object(base_snapshot_map),
    );
    // Keep intermediate values in Phase 2 (`P1 .. PN`) as full-precision IEEE-754 floats
    // and quantize once during the Phase 3 Epilogue Pass via `format_orca_schema_scalar`.
    initial_map.insert(
      "_defer_quantization".to_string(),
      serde_json::Value::String("1".to_string()),
    );
    let mut current_values = serde_json::Value::Object(initial_map);
    let mut execution_logs = Vec::new();
    let mut ordered_rules = rules.to_vec();
    ordered_rules.sort_by_key(|r| r.priority);

    struct RawStepRecord {
      change_idx: usize,
      step_num: usize,
      param_key: String,
      unit: String,
      raw_before: String,
      raw_after: String,
      before_json_val: Option<serde_json::Value>,
    }

    let catalog = OrcaProfile::get_parameter_catalog();
    let mut active_rule_count = 0;
    let mut rule_sources: HashMap<String, String> = HashMap::new();
    let mut rule_changes: Vec<RuleBeforeAfterChange> = Vec::new();
    let mut raw_step_records: Vec<RawStepRecord> = Vec::new();
    let mut step_snapshots: HashMap<usize, (String, serde_json::Value, String, String)> =
      HashMap::new();
    let mut user_rule_scales: HashMap<String, f64> = HashMap::new();
    let mut skipped_rules: Vec<SkippedRuleInfo> = Vec::new();

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
      if let Some(reason) = rule.scope_skip_reason(context) {
        execution_logs.push(format!(
          "⏩ [Step #{} • Priority #{}] Skipping '{}' (out of scope: {})",
          step_num, rule.priority, rule.name, reason
        ));
        // Dry-run the skipped rule on the current state to learn which parameters it would touch,
        // so the Provenance & Lineage view can show it as an out-of-scope step.
        let mut would_touch_keys: Vec<String> = Vec::new();
        if let Ok((dry_val, _, dry_mod)) =
          execute_transformation_lua(&rule.lua_code, &current_values, context)
        {
          if let (
            serde_json::Value::Object(ref before_map),
            serde_json::Value::Object(ref after_map),
          ) = (&current_values, &dry_val)
          {
            for (k, v) in after_map {
              if k.starts_with('_')
                || k == "flush_volumes_matrix"
                || k == "computed_safe_speed_ceiling"
              {
                continue;
              }
              if before_map.get(k) != Some(v) {
                would_touch_keys.push(k.clone());
              }
            }
          }
          for (k, _, _) in dry_mod {
            if !k.starts_with('_') && !would_touch_keys.contains(&k) {
              would_touch_keys.push(k);
            }
          }
        }
        would_touch_keys.sort();
        would_touch_keys.dedup();
        skipped_rules.push(SkippedRuleInfo {
          rule_id: rule.id.clone(),
          rule_name: rule.name.clone(),
          rule_icon: rule.icon.clone(),
          step_number: step_num,
          reason,
          would_touch_keys,
        });
        continue;
      }

      active_rule_count += 1;
      execution_logs.push(format!(
        "⚙️ [Step #{} • Priority #{}] Evaluating rule: '{}'",
        step_num, rule.priority, rule.name
      ));

      let before_snapshot = current_values.clone();
      step_snapshots.insert(
        step_num,
        (
          rule.lua_code.clone(),
          before_snapshot.clone(),
          rule.id.clone(),
          rule.name.clone(),
        ),
      );
      let mut rule_ctx = context.clone();
      if let serde_json::Value::Object(ref mut cm) = rule_ctx {
        cm.insert(
          "_current_rule_id".to_string(),
          serde_json::Value::String(rule.id.clone()),
        );
        cm.insert(
          "_current_rule_name".to_string(),
          serde_json::Value::String(rule.name.clone()),
        );
        cm.insert(
          "_current_step_num".to_string(),
          serde_json::json!(step_num),
        );
      }
      match execute_transformation_lua(&rule.lua_code, &current_values, &rule_ctx) {
        Ok((mut new_val, stdout, mut modified)) => {
          let rule_log_summary = stdout.join(" • ");
          for line in stdout {
            execution_logs.push(format!("   {}", line));
          }
          // Canonical Shadow-Aware Routing:
          // If a custom or external Lua rule writes a shadowed dual-scope key (`z_hop`,
          // `z_hop_types`, `retraction_length`, `retraction_speed`, `deretraction_speed`,
          // `retract_lift_enforce`, `retract_before_wipe`, `wipe`, `wipe_distance`,
          // `retraction_minimum_travel`, `max_volumetric_speed`), canonicalize it onto its
          // `filament_*` target so AnycubicSlicerNext's "non-nil wins" Filament inheritance
          // never silently shadows the rule's write at slice time.
          if let (
            serde_json::Value::Object(ref before_map),
            serde_json::Value::Object(ref mut after_map),
          ) = (&before_snapshot, &mut new_val)
          {
            let mut shadowed_keys: Vec<(String, &'static str)> = Vec::new();
            for k in after_map.keys() {
              if let Some(canon_key) = crate::threemf::canonicalize_dual_scope_key(k) {
                let after_str = after_map.get(k).map(&extract_scalar).unwrap_or_default();
                let before_str = before_map
                  .get(k)
                  .map(&extract_scalar)
                  .or_else(|| base_profile.get_str(k))
                  .unwrap_or_default();
                let explicitly_modified = modified.iter().any(|(mk, _, _)| mk == k);
                if after_str != before_str || explicitly_modified {
                  shadowed_keys.push((k.clone(), canon_key));
                }
              }
            }
            for (shadow_key, canon_key) in shadowed_keys {
              if let Some(written_val) = after_map.get(&shadow_key).cloned() {
                let scalar = extract_scalar(&written_val);
                after_map.insert(canon_key.to_string(), serde_json::json!([scalar]));
              }
              if let Some(orig) = before_map.get(&shadow_key).cloned() {
                after_map.insert(shadow_key.clone(), orig);
              } else {
                after_map.remove(&shadow_key);
              }
              for (mod_k, _, _) in &mut modified {
                if mod_k == &shadow_key {
                  *mod_k = canon_key.to_string();
                }
              }
              execution_logs.push(format!(
                "   ↪ Canonicalized shadowed dual-scope key '{}' → '{}' in rule '{}'",
                shadow_key, canon_key, rule.id
              ));
            }
          }
          // Exhaustive Key Routing Registry + Zero-Drop Invariant:
          // Every key modified by a Lua rule must map to a valid exported artifact destination
          // (`Machine`, `Process`, `Filament`, `ProjectFlush`, or `InternalMeta`).
          if let Err(err_msg) = Self::validate_lua_rule_writes(&rule.id, &modified) {
            execution_logs.push(format!("❌ {}", err_msg));
            #[cfg(debug_assertions)]
            panic!("{}", err_msg);
          }
          for (mod_key, _, _) in &modified {
            if !mod_key.starts_with('_') {
              append_rule_source(&mut rule_sources, mod_key, step_num, &rule.name);
            }
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
              if k.starts_with('_')
                || k == "flush_volumes_matrix"
                || k == "computed_safe_speed_ceiling"
              {
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
                let delta_summary = compute_delta_summary(&before_str, &after_str, unit);
                if !matches!(
                  rule.id.as_str(),
                  "volumetric_speed_ceiling"
                    | "rule_volumetric_speed_ceiling"
                    | "outer_wall_consistency"
                    | "wizard_outer_wall_consistency"
                    | "weather_climate_adjustment"
                    | "rule_weather_climate"
                    | "tpu_foaming_shore"
                    | "transparent_optical_glass"
                ) && matches!(
                  k.as_str(),
                  "outer_wall_speed"
                    | "inner_wall_speed"
                    | "sparse_infill_speed"
                    | "default_acceleration"
                ) {
                  if let (Ok(b_num), Ok(a_num)) = (
                    before_str.trim().parse::<f64>(),
                    after_str.trim().parse::<f64>(),
                  ) {
                    if b_num > 0.0 && a_num > 0.0 {
                      let entry = user_rule_scales.entry(k.clone()).or_insert(1.0);
                      *entry *= a_num / b_num;
                    }
                  }
                }
                execution_logs.push(format!(
                  "   ↳ [Before → After] {} ({}): {} → {} [{}]",
                  label, k, before_fmt, after_fmt, delta_summary
                ));
                let change_idx = rule_changes.len();
                raw_step_records.push(RawStepRecord {
                  change_idx,
                  step_num,
                  param_key: k.clone(),
                  unit: unit.to_string(),
                  raw_before: before_str,
                  raw_after: after_str,
                  before_json_val: before_map.get(k).cloned(),
                });
                rule_changes.push(RuleBeforeAfterChange {
                  rule_id: rule.id.clone(),
                  rule_name: rule.name.clone(),
                  rule_icon: rule.icon.clone(),
                  rule_category: rule.category.clone(),
                  step_number: step_num,
                  rule_priority: rule.priority,
                  param_key: k.clone(),
                  param_label: label,
                  before_value: before_fmt,
                  after_value: after_fmt,
                  delta_summary,
                  rule_log: rule_log_summary.clone(),
                  shadow_kind: None,
                  shadowed_by_step: None,
                  shadowed_by_rule_id: None,
                  shadowed_by_rule_name: None,
                  shadowed_final_value: None,
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

    // Detect Rule Shadowing (⚡ Overwritten by later rule): absolute clobbering only
    let parse_num_for_shadow = |raw: &str, unit: &str| -> Option<f64> {
      if unit == "flag" {
        return None;
      }
      raw.trim().trim_end_matches('%').trim().parse::<f64>().ok()
    };

    let mut records_by_key: HashMap<String, Vec<usize>> = HashMap::new();
    let mut records_by_step: HashMap<usize, Vec<usize>> = HashMap::new();
    for (rec_idx, rec) in raw_step_records.iter().enumerate() {
      records_by_key
        .entry(rec.param_key.clone())
        .or_default()
        .push(rec_idx);
      records_by_step
        .entry(rec.step_num)
        .or_default()
        .push(rec_idx);
    }
    let mut sorted_shadow_keys: Vec<String> = records_by_key.keys().cloned().collect();
    sorted_shadow_keys.sort();
    for key in sorted_shadow_keys {
      let Some(rec_indices) = records_by_key.get(&key) else {
        continue;
      };
      if rec_indices.len() >= 2 {
        let final_change_idx = raw_step_records[*rec_indices.last().unwrap()].change_idx;
        let final_after_fmt = rule_changes[final_change_idx].after_value.clone();

        for pos in 0..(rec_indices.len() - 1) {
          let curr = &raw_step_records[rec_indices[pos]];
          let curr_num_delta = match (
            parse_num_for_shadow(&curr.raw_before, &curr.unit),
            parse_num_for_shadow(&curr.raw_after, &curr.unit),
          ) {
            (Some(b), Some(a)) => Some(a - b),
            _ => None,
          };

          let mut detected: Option<(RuleShadowKind, usize)> = None;

          // Opposite-direction adjustments from different rules (e.g. +5°C for a hardened nozzle, -3°C for
          // wet filament) are normal relative composition and are NOT flagged. We only flag absolute
          // clobbering: a downstream rule whose output is identical even when `key` is restored to
          // `curr.raw_before`, i.e. the downstream rule completely forgot `curr`'s contribution.
          // A zero-delta numeric step contributes nothing and cannot be clobbered.
          let curr_is_noop = matches!(curr_num_delta, Some(d) if d.abs() <= 1e-9);
          if !curr_is_noop {
            // Restore a pre-step value while preserving the LIVE shape the later rule sees
            // (preset values are usually single-element arrays like ["225"]; feeding a bare string
            // would make Lua `config[key][1]` reads return nil and skew the counterfactual).
            let restore_val = |snap_map: &serde_json::Map<String, serde_json::Value>,
                               param_key: &str,
                               before_json: &Option<serde_json::Value>,
                               raw_before: &str|
             -> Option<serde_json::Value> {
              let orig = match before_json {
                Some(v) => Some(v.clone()),
                None if raw_before != "(unset)" => {
                  Some(serde_json::Value::String(raw_before.to_string()))
                }
                None => None,
              };
              orig.map(|v| match snap_map.get(param_key) {
                Some(serde_json::Value::Array(_)) => match &v {
                  serde_json::Value::Array(_) => v,
                  scalar => serde_json::json!([scalar]),
                },
                _ => v,
              })
            };
            for &later_rec_idx in &rec_indices[(pos + 1)..] {
              let later = &raw_step_records[later_rec_idx];
              let Some((later_lua, later_before_snap, _, _)) = step_snapshots.get(&later.step_num)
              else {
                continue;
              };
              let mut cf_snap = later_before_snap.clone();
              if let serde_json::Value::Object(ref mut cf_map) = cf_snap {
                match restore_val(cf_map, &key, &curr.before_json_val, &curr.raw_before) {
                  Some(v) => {
                    cf_map.insert(key.clone(), v);
                  }
                  None => {
                    cf_map.remove(&key);
                  }
                }
              }
              if let Ok((serde_json::Value::Object(cf_out_map), _, _)) =
                execute_transformation_lua(later_lua, &cf_snap, context)
              {
                let cf_after_str = cf_out_map
                  .get(&key)
                  .map(&extract_scalar)
                  .unwrap_or_default();
                if values_are_equivalent(&cf_after_str, &later.raw_after) {
                  // The later rule's output for `key` ignores this step's write to `key`. Before
                  // calling it a true clobber, check whether the step's intent reached the later
                  // rule through SIBLING keys the same step wrote (e.g. it set both
                  // `nozzle_temperature` and `nozzle_temperature_HS`, and the later rule recomputes
                  // `_HS` from `nozzle_temperature`): undo the step's whole contribution and see if
                  // the later output changes. Changed output => Recomputed (benign); identical
                  // output => Overwritten (the contribution was truly forgotten).
                  let mut kind = RuleShadowKind::Overwritten;
                  if let Some(step_recs) = records_by_step.get(&curr.step_num) {
                    let mut cf2_snap = later_before_snap.clone();
                    if let serde_json::Value::Object(ref mut cf2_map) = cf2_snap {
                      let mut restored_any = false;
                      for &sib_idx in step_recs {
                        let sib = &raw_step_records[sib_idx];
                        let (sib_key, before_json, raw_before) = if sib.param_key == key {
                          (&key, &curr.before_json_val, &curr.raw_before)
                        } else {
                          // Only restore siblings the later rule still sees unchanged since this
                          // step wrote them (an intervening rule's edit must not be erased).
                          let sib_current = cf2_map.get(&sib.param_key).map(&extract_scalar);
                          let unchanged = sib_current
                            .as_deref()
                            .map(|v| values_are_equivalent(v, &sib.raw_after))
                            .unwrap_or(false);
                          if !unchanged {
                            continue;
                          }
                          (&sib.param_key, &sib.before_json_val, &sib.raw_before)
                        };
                        match restore_val(cf2_map, sib_key, before_json, raw_before) {
                          Some(v) => {
                            cf2_map.insert(sib_key.clone(), v);
                          }
                          None => {
                            cf2_map.remove(sib_key);
                          }
                        }
                        restored_any = true;
                      }
                      if restored_any {
                        if let Ok((serde_json::Value::Object(cf2_out_map), _, _)) =
                          execute_transformation_lua(later_lua, &cf2_snap, context)
                        {
                          let cf2_after_str = cf2_out_map
                            .get(&key)
                            .map(&extract_scalar)
                            .unwrap_or_default();
                          if !values_are_equivalent(&cf2_after_str, &later.raw_after) {
                            kind = RuleShadowKind::Recomputed;
                          }
                        }
                      }
                    }
                  }
                  detected = Some((kind, later_rec_idx));
                  break;
                }
              }
            }
          }

          if let Some((kind, later_rec_idx)) = detected {
            let later = &raw_step_records[later_rec_idx];
            let later_rule_id = rule_changes[later.change_idx].rule_id.clone();
            let later_rule_name = rule_changes[later.change_idx].rule_name.clone();
            let later_step_num = later.step_num;
            let curr_change = &mut rule_changes[curr.change_idx];
            curr_change.shadow_kind = Some(kind);
            curr_change.shadowed_by_step = Some(later_step_num);
            curr_change.shadowed_by_rule_id = Some(later_rule_id);
            curr_change.shadowed_by_rule_name = Some(later_rule_name.clone());
            curr_change.shadowed_final_value = Some(final_after_fmt.clone());

            let kind_tag = match kind {
              RuleShadowKind::Overwritten => "⚡ Overwritten by Later Rule",
              RuleShadowKind::Recomputed => "♻ Recomputed by Later Rule (intent preserved)",
              RuleShadowKind::Accepted | RuleShadowKind::Clamped => {
                unreachable!("Accepted/Clamped are classified below")
              }
            };
            execution_logs.push(format!(
              "⚠️ [{}] {} ({}): Step #{} ('{}': {} → {}) superseded by Step #{} ('{}' → {})",
              kind_tag,
              curr_change.param_label,
              key,
              curr_change.step_number,
              curr_change.rule_name,
              curr_change.before_value,
              curr_change.after_value,
              later_step_num,
              later_rule_name,
              final_after_fmt
            ));
          }
        }
      }

      // Acceptance classification: this key's LAST surviving write was not clobbered — find the
      // first downstream rule that READ the key (its output follows a perturbation of the key) and
      // still left the value untouched: the guard ACCEPTED the earlier value (e.g. `min(x, 20)`
      // with x already ≤ 20). Informational only — never marks the parameter as shadowed.
      let last_rec_idx = *rec_indices.last().unwrap();
      {
        let last = &raw_step_records[last_rec_idx];
        if rule_changes[last.change_idx].shadow_kind.is_none() {
          let perturbed_scalar = match last.raw_after.trim().parse::<f64>() {
            Ok(n) => format!("{:.3}", n + 123.75),
            Err(_) => format!("ZZ{}", last.raw_after),
          };
          let mut later_step_nums: Vec<usize> = step_snapshots
            .keys()
            .copied()
            .filter(|&sn| sn > last.step_num)
            .collect();
          later_step_nums.sort_unstable();
          for later_step_num in later_step_nums {
            let Some((later_lua, later_before_snap, later_rule_id, later_rule_name)) =
              step_snapshots.get(&later_step_num)
            else {
              continue;
            };
            if !later_lua.contains(&key) {
              continue;
            }
            let mut pert_snap = later_before_snap.clone();
            if let serde_json::Value::Object(ref mut pmap) = pert_snap {
              let pert_val = match pmap.get(&key) {
                Some(serde_json::Value::Array(_)) => {
                  serde_json::json!([perturbed_scalar])
                }
                _ => serde_json::Value::String(perturbed_scalar.clone()),
              };
              pmap.insert(key.clone(), pert_val);
            }
            let Ok((serde_json::Value::Object(pert_out_map), _, _)) =
              execute_transformation_lua(later_lua, &pert_snap, context)
            else {
              continue;
            };
            let pert_out_str = pert_out_map
              .get(&key)
              .map(&extract_scalar)
              .unwrap_or_default();
            // The rule ACCEPTed the key only if it is a self-referential writer: under the
            // perturbation it must TRANSFORM the input (output != perturbed input). A rule that
            // merely reads the key for another purpose echoes the perturbed value back unchanged
            // and must not be classified as a guard.
            if !pert_out_str.is_empty() && !values_are_equivalent(&pert_out_str, &perturbed_scalar)
            {
              let later_rule_id = later_rule_id.clone();
              let later_rule_name = later_rule_name.clone();
              let last_change = &mut rule_changes[last.change_idx];
              last_change.shadow_kind = Some(RuleShadowKind::Accepted);
              last_change.shadowed_by_step = Some(later_step_num);
              last_change.shadowed_by_rule_id = Some(later_rule_id);
              last_change.shadowed_by_rule_name = Some(later_rule_name.clone());
              last_change.shadowed_final_value = Some(last.raw_after.clone());
              execution_logs.push(format!(
                "✔️ [Accepted by Later Rule] {} ({}): Step #{} ('{}': {} → {}) satisfied Step #{} ('{}') constraint and passed through unchanged",
                last_change.param_label,
                key,
                last_change.step_number,
                last_change.rule_name,
                last_change.before_value,
                last_change.after_value,
                later_step_num,
                later_rule_name
              ));
              break;
            }
          }
        }
      }
    }

    // =========================================================================
    // Phase 3: Epilogue Constraint Projection & Coupled Invariants (Priority = ∞)
    // =========================================================================
    let epilogue_step_num = ordered_rules.len() + 1;
    let mut parameter_bounds: HashMap<String, ParameterBound> = HashMap::new();
    let mut infeasible_constraints: Vec<ParameterBound> = Vec::new();

    if let Some(bounds_obj) = current_values.get("_bounds").and_then(|v| v.as_object()) {
      for (k, b_val) in bounds_obj {
        if let Some(b_map) = b_val.as_object() {
          let min = b_map.get("min").and_then(|v| v.as_f64());
          let max = b_map.get("max").and_then(|v| v.as_f64());
          let min_by = b_map
            .get("min_by")
            .and_then(|v| v.as_str())
            .unwrap_or("default")
            .to_string();
          let max_by = b_map
            .get("max_by")
            .and_then(|v| v.as_str())
            .unwrap_or("default")
            .to_string();
          let min_rule_id = b_map
            .get("min_rule_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
          let max_rule_id = b_map
            .get("max_rule_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
          let min_step = b_map
            .get("min_step")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as usize;
          let max_step = b_map
            .get("max_step")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as usize;
          if min.is_some() || max.is_some() {
            parameter_bounds.insert(
              k.clone(),
              ParameterBound {
                param_key: k.clone(),
                min,
                max,
                min_by,
                max_by,
                min_rule_id,
                max_rule_id,
                min_step,
                max_step,
              },
            );
          }
        }
      }
    }

    let format_clamped_scalar = |val: f64, reference_str: &str, param_key: &str| -> String {
      Self::format_orca_schema_scalar(param_key, val, reference_str)
    };

    let write_clamped_value =
      |map: &mut serde_json::Map<String, serde_json::Value>, key: &str, formatted: &str| {
        match map.get_mut(key) {
          Some(serde_json::Value::Array(arr)) => {
            if arr.is_empty() {
              arr.push(serde_json::Value::String(formatted.to_string()));
            } else {
              for elem in arr.iter_mut() {
                *elem = serde_json::Value::String(formatted.to_string());
              }
            }
          }
          _ => {
            map.insert(
              key.to_string(),
              serde_json::Value::String(formatted.to_string()),
            );
          }
        }
      };

    // 0. Phase 3 Deferred Schema Quantization Pass:
    //    During Phase 2 (`P1 .. PN`), values inside `config` were kept as full-precision IEEE-754
    //    floats to eliminate intermediate string-rounding drift across multi-rule compositions.
    //    Here at the start of Phase 3 Epilogue, quantize all modified keys once according to
    //    `format_orca_schema_scalar` and snap intermediate provenance step boundaries so the
    //    Provenance UI displays clean formatted units with a 100% continuous ledger chain.
    if let serde_json::Value::Object(ref mut cmap) = current_values {
      cmap.remove("_defer_quantization");
      let mut touched_sorted: Vec<String> = rule_sources.keys().cloned().collect();
      touched_sorted.sort();
      for k in &touched_sorted {
        let base_ref = base_profile.get_str(k).unwrap_or_default();
        let meta = catalog.get(k);
        let unit = meta.map(|m| m.unit.as_str()).unwrap_or("");
        let unit_suffix = if unit.is_empty() {
          String::new()
        } else {
          format!(" {}", unit)
        };
        if let Some(cur_json) = cmap.get(k).cloned() {
          let cur_str = extract_scalar(&cur_json);
          let ref_for_fmt = if base_ref.is_empty() {
            cur_str.as_str()
          } else {
            base_ref.as_str()
          };
          if !cur_str.trim().ends_with('%') {
            if let Ok(num) = cur_str.trim().parse::<f64>() {
              let quant_str = Self::format_orca_schema_scalar(k, num, ref_for_fmt);
              if quant_str != cur_str {
                write_clamped_value(cmap, k, &quant_str);
              }
            }
          }
        }
        // Format any unquantized intermediate `before_value` / `after_value` in `rule_changes`
        // for this key so adjacent steps remain continuous (`step[i].after == step[i+1].before`)
        // and the final Phase 2 step's `after_value` matches the quantized Phase 3 input.
        let mut prev_after_raw: Option<String> = None;
        for rc in rule_changes.iter_mut().filter(|c| &c.param_key == k) {
          let strip_unit = |s: &str| -> String {
            if !unit_suffix.is_empty() && s.ends_with(&unit_suffix) {
              s[..s.len() - unit_suffix.len()].to_string()
            } else {
              s.to_string()
            }
          };
          let cur_before_raw = strip_unit(&rc.before_value);
          let cur_after_raw = strip_unit(&rc.after_value);
          let ref_for_fmt = if base_ref.is_empty() {
            cur_before_raw.as_str()
          } else {
            base_ref.as_str()
          };
          let quant_before_raw = if let Some(ref pa) = prev_after_raw {
            pa.clone()
          } else {
            cur_before_raw.clone()
          };
          let quant_after_raw = if cur_after_raw == "(unset)" || cur_after_raw.trim().ends_with('%')
          {
            cur_after_raw.clone()
          } else if let Ok(a_num) = cur_after_raw.trim().parse::<f64>() {
            Self::format_orca_schema_scalar(k, a_num, ref_for_fmt)
          } else {
            cur_after_raw.clone()
          };
          rc.before_value = if quant_before_raw == "(unset)" {
            quant_before_raw.clone()
          } else {
            format!("{}{}", quant_before_raw, unit_suffix)
          };
          rc.after_value = if quant_after_raw == "(unset)" {
            quant_after_raw.clone()
          } else {
            format!("{}{}", quant_after_raw, unit_suffix)
          };
          rc.delta_summary = compute_delta_summary(&quant_before_raw, &quant_after_raw, unit);
          prev_after_raw = Some(quant_after_raw);
        }
        if let Some(ref final_quant_raw) = prev_after_raw {
          let final_quant_fmt = format!("{}{}", final_quant_raw, unit_suffix);
          for rc in rule_changes.iter_mut().filter(|c| &c.param_key == k) {
            if rc.shadowed_final_value.is_some() {
              rc.shadowed_final_value = Some(final_quant_fmt.clone());
            }
          }
        }
      }
    }

    // 1. Box-Constraint Projection & Infeasibility Detection (L > U)
    let mut sorted_bound_keys: Vec<String> = parameter_bounds.keys().cloned().collect();
    sorted_bound_keys.sort();
    for k in &sorted_bound_keys {
      let Some(b) = parameter_bounds.get(k) else {
        continue;
      };
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

      if b.is_infeasible() {
        infeasible_constraints.push(b.clone());
        execution_logs.push(format!(
          "❌ [Infeasible Constraint Conflict] {} ({}): lower bound {:.2}{} (by '{}') > upper bound {:.2}{} (by '{}')",
          label,
          k,
          b.min.unwrap_or(0.0),
          unit_suffix,
          b.min_by,
          b.max.unwrap_or(0.0),
          unit_suffix,
          b.max_by
        ));
      }

      if let serde_json::Value::Object(ref mut cmap) = current_values {
        let Some(cur_json) = cmap.get(k) else {
          continue;
        };
        let before_str = extract_scalar(cur_json);
        let Ok(cur_num) = before_str.trim().parse::<f64>() else {
          continue;
        };

        // Apply max(min_v, cur) then min(max_v, .) so hardware upper ceiling wins if L > U.
        let mut clamped_num = cur_num;
        let mut bound_reason: Option<(String, String)> = None;
        if let Some(lo) = b.min {
          if clamped_num < lo - 1e-9 {
            clamped_num = lo;
            let lo_str = format_clamped_scalar(lo, &before_str, k);
            bound_reason = Some((b.min_by.clone(), format!("≥ {}{}", lo_str, unit_suffix)));
          }
        }
        if let Some(hi) = b.max {
          if clamped_num > hi + 1e-9 {
            clamped_num = hi;
            let hi_str = format_clamped_scalar(hi, &before_str, k);
            bound_reason = Some((b.max_by.clone(), format!("≤ {}{}", hi_str, unit_suffix)));
          }
        }

        if let Some((by_who, bound_cond)) = bound_reason {
          let after_str = format_clamped_scalar(clamped_num, &before_str, k);
          if !values_are_equivalent(&before_str, &after_str) {
            write_clamped_value(cmap, k, &after_str);
            let clamp_rule_name = format!("Clamped by {} ({})", by_who, bound_cond);
            append_rule_source(&mut rule_sources, k, epilogue_step_num, &clamp_rule_name);
            let before_fmt = format!("{}{}", before_str, unit_suffix);
            let after_fmt = format!("{}{}", after_str, unit_suffix);
            let delta_summary = compute_delta_summary(&before_str, &after_str, unit);
            let interval_str = b.format_interval(unit);
            execution_logs.push(format!(
              "🛡️ [Constraint Projection • Priority ∞] {} ({}): {} → {} [{} • Feasible {}]",
              label, k, before_fmt, after_fmt, clamp_rule_name, interval_str
            ));
            if let Some(prev) = rule_changes
              .iter_mut()
              .rev()
              .find(|rc| rc.param_key == *k)
            {
              prev.shadow_kind = Some(RuleShadowKind::Clamped);
              prev.shadowed_by_step = Some(epilogue_step_num);
              prev.shadowed_by_rule_id = Some("constraint_projection".to_string());
              prev.shadowed_by_rule_name = Some(clamp_rule_name.clone());
              prev.shadowed_final_value = Some(after_fmt.clone());
            }
            rule_changes.push(RuleBeforeAfterChange {
              rule_id: "constraint_projection".to_string(),
              rule_name: clamp_rule_name,
              rule_icon: "🛡️".to_string(),
              rule_category: "Constraint Projection".to_string(),
              step_number: epilogue_step_num,
              rule_priority: i32::MAX,
              param_key: k.clone(),
              param_label: label,
              before_value: before_fmt,
              after_value: after_fmt,
              delta_summary,
              rule_log: format!(
                "Projected onto feasible interval {} (min by '{}', max by '{}')",
                interval_str, b.min_by, b.max_by
              ),
              shadow_kind: None,
              shadowed_by_step: None,
              shadowed_by_rule_id: None,
              shadowed_by_rule_name: None,
              shadowed_final_value: None,
            });
          }
        }
      }
    }

    // 2. Cross-Key Relational Invariants (Coupled Constraints at Priority = ∞)
    if let serde_json::Value::Object(ref mut cmap) = current_values {
      let mut record_coupled_clamp = |cmap: &mut serde_json::Map<String, serde_json::Value>,
                                      sources: &mut HashMap<String, String>,
                                      key: &str,
                                      before_str: &str,
                                      new_num: f64,
                                      rule_id: &str,
                                      rule_name: &str,
                                      rule_log: String| {
        let after_str = format_clamped_scalar(new_num, before_str, key);
        if values_are_equivalent(before_str, &after_str) {
          return;
        }
        write_clamped_value(cmap, key, &after_str);
        append_rule_source(sources, key, epilogue_step_num, rule_name);
        let meta = catalog.get(key);
        let label = meta
          .map(|m| m.label.clone())
          .unwrap_or_else(|| key.replace('_', " "));
        let unit = meta.map(|m| m.unit.as_str()).unwrap_or("");
        let unit_suffix = if unit.is_empty() {
          String::new()
        } else {
          format!(" {}", unit)
        };
        let before_fmt = format!("{}{}", before_str, unit_suffix);
        let after_fmt = format!("{}{}", after_str, unit_suffix);
        let delta_summary = compute_delta_summary(before_str, &after_str, unit);
        execution_logs.push(format!(
          "🔗 [Coupled Invariant • Priority ∞] {} ({}): {} → {} [{}]",
          label, key, before_fmt, after_fmt, rule_name
        ));
        if let Some(prev) = rule_changes
          .iter_mut()
          .rev()
          .find(|rc| rc.param_key == key)
        {
          prev.shadow_kind = Some(RuleShadowKind::Clamped);
          prev.shadowed_by_step = Some(epilogue_step_num);
          prev.shadowed_by_rule_id = Some(rule_id.to_string());
          prev.shadowed_by_rule_name = Some(rule_name.to_string());
          prev.shadowed_final_value = Some(after_fmt.clone());
        }
        rule_changes.push(RuleBeforeAfterChange {
          rule_id: rule_id.to_string(),
          rule_name: rule_name.to_string(),
          rule_icon: "🔗".to_string(),
          rule_category: "Coupled Invariant".to_string(),
          step_number: epilogue_step_num,
          rule_priority: i32::MAX,
          param_key: key.to_string(),
          param_label: label,
          before_value: before_fmt,
          after_value: after_fmt,
          delta_summary,
          rule_log,
          shadow_kind: None,
          shadowed_by_step: None,
          shadowed_by_rule_id: None,
          shadowed_by_rule_name: None,
          shadowed_final_value: None,
        });
      };

      // (a) First-Layer vs. Other-Layers Thermal Continuity:
      // `nozzle_temperature_initial_layer` must stay within [-5 °C, +20 °C] of `nozzle_temperature`
      // (and respect `bounds[init_key]` if present) whenever a rule touched or constrained temperatures.
      for (cont_key, init_key) in [
        ("nozzle_temperature", "nozzle_temperature_initial_layer"),
        (
          "nozzle_temperature_HS",
          "nozzle_temperature_initial_layer_HS",
        ),
      ] {
        let temp_touched = rule_sources.contains_key(cont_key)
          || rule_sources.contains_key(init_key)
          || parameter_bounds.contains_key(cont_key)
          || parameter_bounds.contains_key(init_key);
        if !temp_touched {
          continue;
        }
        let Some(cont_str) = cmap.get(cont_key).map(&extract_scalar) else {
          continue;
        };
        let Some(init_str) = cmap.get(init_key).map(&extract_scalar) else {
          continue;
        };
        if let (Ok(t_cont), Ok(t_init)) = (
          cont_str.trim().parse::<f64>(),
          init_str.trim().parse::<f64>(),
        ) {
          let mut lo = t_cont - 5.0;
          let mut hi = t_cont + 20.0;
          if let Some(b) = parameter_bounds.get(init_key) {
            if let Some(b_lo) = b.min {
              lo = lo.max(b_lo);
            }
            if let Some(b_hi) = b.max {
              hi = hi.min(b_hi);
            }
          }
          if lo <= hi && (t_init < lo - 1e-9 || t_init > hi + 1e-9) {
            let clamped = t_init.max(lo).min(hi);
            record_coupled_clamp(
              cmap,
              &mut rule_sources,
              init_key,
              &init_str,
              clamped,
              "coupled_constraint_thermal_continuity",
              &format!(
                "Clamped by Coupled First-Layer Thermal Continuity ([{:.0} °C, {:.0} °C] from {} = {:.0} °C)",
                lo, hi, cont_key, t_cont
              ),
              format!(
                "Enforced T_initial_layer ∈ [T_nozzle - 5 °C, T_nozzle + 20 °C] relative to {} = {:.0} °C",
                cont_key, t_cont
              ),
            );
          }
        }
      }

      // (b) Retraction Prime Speed <= Retraction Pull Speed:
      // `filament_deretraction_speed <= filament_retraction_speed` when deretraction speed > 0.
      if let (Some(deret_str), Some(ret_str)) = (
        cmap.get("filament_deretraction_speed").map(&extract_scalar),
        cmap.get("filament_retraction_speed").map(&extract_scalar),
      ) {
        if let (Ok(deret), Ok(ret)) = (
          deret_str.trim().parse::<f64>(),
          ret_str.trim().parse::<f64>(),
        ) {
          if deret > 0.0 && ret > 0.0 && deret > ret + 1e-9 {
            record_coupled_clamp(
              cmap,
              &mut rule_sources,
              "filament_deretraction_speed",
              &deret_str,
              ret,
              "coupled_constraint_deretraction_speed",
              &format!(
                "Clamped by Coupled Retraction Prime ≤ Pull Speed (≤ {} mm/s)",
                ret_str.trim()
              ),
              format!(
                "Enforced filament_deretraction_speed ≤ filament_retraction_speed ({} mm/s) to prevent nozzle tip blobbing on prime",
                ret_str.trim()
              ),
            );
          }
        }
      }

      // (c) Z-Hop >= Layer Height Floor on Spiral Lift:
      // When `filament_z_hop_types` / `z_hop_types` == "Spiral Lift" and Z-hop > 0,
      // ensure `filament_z_hop >= 0.5 * layer_height` (respecting `bounds["filament_z_hop"].max`).
      let zhop_type = cmap
        .get("filament_z_hop_types")
        .or_else(|| cmap.get("z_hop_types"))
        .map(&extract_scalar)
        .unwrap_or_default();
      if zhop_type.eq_ignore_ascii_case("Spiral Lift") {
        let lh_opt = cmap
          .get("layer_height")
          .map(&extract_scalar)
          .and_then(|s| s.trim().parse::<f64>().ok())
          .or_else(|| {
            context.get("layer_height").and_then(|v| {
              v.as_f64()
                .or_else(|| v.as_str().and_then(|s| s.trim().parse::<f64>().ok()))
            })
          });
        if let Some(lh) = lh_opt {
          for zhop_key in ["filament_z_hop", "z_hop"] {
            let zhop_touched = rule_sources.contains_key(zhop_key)
              || rule_sources.contains_key("filament_z_hop_types")
              || rule_sources.contains_key("z_hop_types")
              || parameter_bounds.contains_key(zhop_key);
            if !zhop_touched {
              continue;
            }
            let Some(z_str) = cmap.get(zhop_key).map(&extract_scalar) else {
              continue;
            };
            if let Ok(z_val) = z_str.trim().parse::<f64>() {
              if z_val > 0.0 {
                let mut floor = 0.5 * lh;
                if let Some(b_max) = parameter_bounds.get(zhop_key).and_then(|b| b.max) {
                  floor = floor.min(b_max);
                }
                if z_val < floor - 1e-9 {
                  record_coupled_clamp(
                    cmap,
                    &mut rule_sources,
                    zhop_key,
                    &z_str,
                    floor,
                    "coupled_constraint_spiral_zhop_floor",
                    &format!(
                      "Clamped by Coupled Spiral Lift Z-Hop Floor (≥ {:.2} mm for {:.2} mm layer)",
                      floor, lh
                    ),
                    format!(
                      "Enforced {} ≥ 0.5 × layer_height ({:.2} mm) for Spiral Lift helix clearance",
                      zhop_key, lh
                    ),
                  );
                }
              }
            }
          }
        }
      }

      // (d) Fan Max Speed >= Fan Min Speed:
      // Ensure `fan_max_speed* >= fan_min_speed*` whenever a rule modified or constrained fan speeds.
      for (min_key, max_key) in [
        ("fan_min_speed", "fan_max_speed"),
        ("fan_min_speed_BRASS", "fan_max_speed_BRASS"),
        ("fan_min_speed_HS", "fan_max_speed_HS"),
      ] {
        let fan_touched = rule_sources.contains_key(min_key)
          || rule_sources.contains_key(max_key)
          || parameter_bounds.contains_key(min_key)
          || parameter_bounds.contains_key(max_key);
        if !fan_touched {
          continue;
        }
        let Some(min_str) = cmap.get(min_key).map(&extract_scalar) else {
          continue;
        };
        let Some(max_str) = cmap.get(max_key).map(&extract_scalar) else {
          continue;
        };
        if let (Ok(fmin), Ok(fmax)) = (
          min_str.trim().parse::<f64>(),
          max_str.trim().parse::<f64>(),
        ) {
          if fmax < fmin - 1e-9 {
            record_coupled_clamp(
              cmap,
              &mut rule_sources,
              max_key,
              &max_str,
              fmin,
              "coupled_constraint_fan_max_ge_min",
              &format!(
                "Clamped by Coupled Fan Max ≥ Fan Min Speed (≥ {}%)",
                min_str.trim()
              ),
              format!(
                "Enforced {} ≥ {} ({}%)",
                max_key,
                min_key,
                min_str.trim()
              ),
            );
          }
        }
      }

      // (e) Structural Strength Fan Cut Ceiling (`_structural_fan_cut_pct` side-channel):
      // When the Structural Strength pack is active on a per-spool filament evaluation,
      // scales down `fan_max_speed*` by `(100 - cut_pct)%` and clamps `fan_min_speed* <= cut_max`
      // as an explicit Priority-∞ Epilogue step so Provenance matches the exported filament JSON/3MF.
      let structural_cut_pct = cmap
        .get("_structural_fan_cut_pct")
        .map(&extract_scalar)
        .and_then(|s| s.trim().parse::<i32>().ok())
        .unwrap_or(0)
        .clamp(0, 60);
      if structural_cut_pct > 0 {
        if let Some(max_str) = cmap.get("fan_max_speed").map(&extract_scalar) {
          if let Ok(cur_fmax) = max_str.trim().parse::<f64>() {
            let cut_max = (cur_fmax * (100 - structural_cut_pct) as f64 / 100.0).round();
            for max_key in ["fan_max_speed", "fan_max_speed_BRASS", "fan_max_speed_HS"] {
              if let Some(m_str) = cmap.get(max_key).map(&extract_scalar) {
                record_coupled_clamp(
                  cmap,
                  &mut rule_sources,
                  max_key,
                  &m_str,
                  cut_max,
                  "coupled_constraint_structural_fan_cut",
                  &format!(
                    "Clamped by Structural Strength Fan Cut (-{}% → {:.0}%)",
                    structural_cut_pct, cut_max
                  ),
                  format!(
                    "Scaled {} by (100 - {}%) for inter-layer chain diffusion strength",
                    max_key, structural_cut_pct
                  ),
                );
              }
            }
            for min_key in ["fan_min_speed", "fan_min_speed_BRASS", "fan_min_speed_HS"] {
              if let Some(min_str) = cmap.get(min_key).map(&extract_scalar) {
                if let Ok(cur_fmin) = min_str.trim().parse::<f64>() {
                  if cur_fmin > cut_max + 1e-9 {
                    record_coupled_clamp(
                      cmap,
                      &mut rule_sources,
                      min_key,
                      &min_str,
                      cut_max,
                      "coupled_constraint_structural_fan_cut",
                      &format!(
                        "Clamped by Structural Strength Fan Cut Ceiling (≤ {:.0}%)",
                        cut_max
                      ),
                      format!(
                        "Clamped {} ≤ fan_max_speed ({:.0}%) after -{}% structural fan cut",
                        min_key, cut_max, structural_cut_pct
                      ),
                    );
                  }
                }
              }
            }
          }
        }
      }

      // (f) Process Preset Speed Policy & VFA Epilogue Projection (when seeded via `seed_process_inputs`):
      //     Ensures `evaluate_stack`'s resulting profile and provenance ledger chains match the
      //     exact exported Process preset (`resolve_effective_orca_profile_values`).
      if context.get("_process_seeded").and_then(|v| v.as_bool()) == Some(true) {
        let is_optical_glass_mode = cmap
          .get("sparse_infill_pattern")
          .map(&extract_scalar)
          .as_deref()
          == Some("alignedrectilinear")
          && cmap
            .get("sparse_infill_density")
            .map(&extract_scalar)
            .as_deref()
            == Some("100%");
        let has_wet_filament_first_layer = cmap
          .get("_wet_filament_init_bed_boost_c")
          .map(&extract_scalar)
          .and_then(|s| s.trim().parse::<i32>().ok())
          .unwrap_or(0)
          > 0;
        let proc_lh = base_profile
          .get_str("layer_height")
          .and_then(|s| s.trim().parse::<f64>().ok())
          .unwrap_or(0.20);
        let proc_lw = base_profile
          .get_str("line_width")
          .and_then(|s| s.trim().parse::<f64>().ok())
          .unwrap_or(0.42);

        if !is_optical_glass_mode {
          for key in [
            "outer_wall_speed",
            "inner_wall_speed",
            "sparse_infill_speed",
            "default_acceleration",
          ] {
            let Some(base_spd_str) = base_profile.get_str(key) else {
              continue;
            };
            let Ok(raw_spd) = base_spd_str.trim().parse::<f64>() else {
              continue;
            };
            let scale = user_rule_scales.get(key).copied().unwrap_or(1.0);
            let mut scaled_spd = (raw_spd * scale).round().max(15.0);
            let mut rule_id_tag = "process_preset_speed_policy";
            let mut rule_name_tag = format!("Process Preset Speed Policy ({:.0})", scaled_spd);
            let mut rule_log_tag = format!(
              "Preserved process preset {} ({:.0} × {:.2} scale)",
              key, raw_spd, scale
            );
            if key == "outer_wall_speed" {
              if let Some(vfa_mode) = cmap
                .get("_outer_wall_vfa_mode")
                .map(&extract_scalar)
                .filter(|s| !s.is_empty())
              {
                let sub_spd = cmap
                  .get("_outer_wall_vfa_sub_speed")
                  .map(&extract_scalar)
                  .and_then(|s| s.trim().parse::<f64>().ok())
                  .unwrap_or(55.0);
                let clear_spd = cmap
                  .get("_outer_wall_vfa_clear_speed")
                  .map(&extract_scalar)
                  .and_then(|s| s.trim().parse::<f64>().ok())
                  .unwrap_or(145.0);
                let eff_mvs = cmap
                  .get("_outer_wall_vfa_effective_mvs")
                  .map(&extract_scalar)
                  .and_then(|s| s.trim().parse::<f64>().ok())
                  .unwrap_or(18.0);
                let is_silk = cmap.get("_outer_wall_vfa_is_silk").map(&extract_scalar).as_deref()
                  == Some("1");
                let lock_pref = cmap
                  .get("_outer_wall_vfa_lock_pref")
                  .map(&extract_scalar)
                  .unwrap_or_else(|| "auto".to_string());
                let ow_width = cmap
                  .get("outer_wall_line_width")
                  .map(&extract_scalar)
                  .and_then(|s| s.trim().parse::<f64>().ok())
                  .unwrap_or(proc_lw);
                let cross_sec = (proc_lh * ow_width).max(0.01);
                let mvs_ceiling = (eff_mvs / cross_sec).floor();
                let actual_outer_spd = scaled_spd.min(mvs_ceiling);
                let mut stepped_down = false;
                if vfa_mode == "sub_resonance_gloss"
                  || (is_silk && scaled_spd > sub_spd + 5.0)
                  || (actual_outer_spd > sub_spd + 5.0 && actual_outer_spd < clear_spd)
                {
                  scaled_spd = (sub_spd * scale).round().max(15.0);
                  stepped_down = true;
                  rule_id_tag = "outer_wall_vfa_epilogue";
                  rule_name_tag = format!(
                    "Clamped by Outer-Wall VFA Guard ({} → {:.0} mm/s)",
                    vfa_mode, scaled_spd
                  );
                  rule_log_tag = format!(
                    "VFA sub-resonance guard stepped outer_wall_speed to {:.0} mm/s",
                    scaled_spd
                  );
                }
                if lock_pref == "auto" {
                  let lock_ow = if vfa_mode == "spire_safe"
                    || (!stepped_down
                      && !is_silk
                      && scaled_spd >= clear_spd
                      && proc_lh <= 0.145)
                  {
                    0.0
                  } else {
                    1.0
                  };
                  if let Some(cur_lock_str) = cmap.get("dont_slow_down_outer_wall").map(&extract_scalar) {
                    let target_lock_str = format!("{:.0}", lock_ow);
                    if cur_lock_str != target_lock_str {
                      record_coupled_clamp(
                        cmap,
                        &mut rule_sources,
                        "dont_slow_down_outer_wall",
                        &cur_lock_str,
                        lock_ow,
                        "outer_wall_vfa_epilogue",
                        &format!("Outer-Wall VFA Auto-Lock ({})", target_lock_str),
                        "Synchronized dont_slow_down_outer_wall with per-process VFA speed regime".to_string(),
                      );
                    }
                  }
                }
              }
            }
            if let Some(cur_spd_str) = cmap.get(key).map(&extract_scalar) {
              let target_spd_str = format!("{:.0}", scaled_spd);
              if !values_are_equivalent(&cur_spd_str, &target_spd_str) {
                record_coupled_clamp(
                  cmap,
                  &mut rule_sources,
                  key,
                  &cur_spd_str,
                  scaled_spd,
                  rule_id_tag,
                  &rule_name_tag,
                  rule_log_tag,
                );
              }
            }
          }

          if !has_wet_filament_first_layer {
            if let (Some(base_il_str), Some(cur_il_str)) = (
              base_profile.get_str("initial_layer_speed"),
              cmap.get("initial_layer_speed").map(&extract_scalar),
            ) {
              if let Ok(base_il_num) = base_il_str.trim().parse::<f64>() {
                if !values_are_equivalent(&cur_il_str, &base_il_str) {
                  record_coupled_clamp(
                    cmap,
                    &mut rule_sources,
                    "initial_layer_speed",
                    &cur_il_str,
                    base_il_num,
                    "process_preset_speed_policy",
                    &format!("Clamped by Process Preset Initial-Layer Policy ({:.0} mm/s)", base_il_num),
                    "Preserved process preset initial_layer_speed (unless Wet Filament first-layer recovery is active)".to_string(),
                  );
                }
              }
            }
          }

          for lw_key in ["line_width", "outer_wall_line_width"] {
            if let (Some(base_lw_str), Some(cur_lw_str)) = (
              base_profile.get_str(lw_key),
              cmap.get(lw_key).map(&extract_scalar),
            ) {
              if base_lw_str != "0.42" && cur_lw_str == "0.42" {
                if let Ok(base_lw_num) = base_lw_str.trim().parse::<f64>() {
                  record_coupled_clamp(
                    cmap,
                    &mut rule_sources,
                    lw_key,
                    &cur_lw_str,
                    base_lw_num,
                    "process_preset_speed_policy",
                    &format!("Clamped by Process Preset Line-Width Policy ({:.2} mm)", base_lw_num),
                    format!("Preserved process preset {} ({:.2} mm)", lw_key, base_lw_num),
                  );
                }
              }
            }
          }
        }

        if cmap.get("seam_slope_type").map(&extract_scalar).as_deref() == Some("contour") {
          cmap.insert(
            "seam_slope_type".to_string(),
            serde_json::Value::String("external".to_string()),
          );
          let meta = catalog.get("seam_slope_type");
          let label = meta
            .map(|m| m.label.clone())
            .unwrap_or_else(|| "seam slope type".to_string());
          let rule_name = "Normalized to OrcaSlicer 'external' Seam Enum".to_string();
          append_rule_source(&mut rule_sources, "seam_slope_type", epilogue_step_num, &rule_name);
          if let Some(prev) = rule_changes
            .iter_mut()
            .rev()
            .find(|c| c.param_key == "seam_slope_type")
          {
            if prev.shadow_kind.is_none() {
              prev.shadow_kind = Some(RuleShadowKind::Clamped);
              prev.shadowed_by_step = Some(epilogue_step_num);
              prev.shadowed_by_rule_id = Some("orca_seam_enum_normalization".to_string());
              prev.shadowed_by_rule_name = Some(rule_name.clone());
              prev.shadowed_final_value = Some("external".to_string());
            }
          }
          rule_changes.push(RuleBeforeAfterChange {
            rule_id: "orca_seam_enum_normalization".to_string(),
            rule_name,
            rule_icon: "🛡️".to_string(),
            rule_category: "Coupled Invariant (Priority ∞)".to_string(),
            step_number: epilogue_step_num,
            rule_priority: i32::MAX,
            param_key: "seam_slope_type".to_string(),
            param_label: label,
            before_value: "contour".to_string(),
            after_value: "external".to_string(),
            delta_summary: "contour → external".to_string(),
            rule_log: "Mapped Lua 'contour' scarf enum to OrcaSlicer C++ 'external'".to_string(),
            shadow_kind: None,
            shadowed_by_step: None,
            shadowed_by_rule_id: None,
            shadowed_by_rule_name: None,
            shadowed_final_value: None,
          });
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
        if k.starts_with('_')
          || k == "flush_volumes_matrix"
          || k == "computed_safe_speed_ceiling"
        {
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

        if is_modified
          || meta.is_some()
          || rule_sources.contains_key(k)
          || parameter_bounds.contains_key(k)
        {
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

    let origin_source_name = if base_profile.name.is_empty() {
      "System Base Profile + Active Spool & Process Baseline".to_string()
    } else {
      format!(
        "{} (Base Profile + Spool/Process Baseline)",
        base_profile.name
      )
    };
    let mut provenance_items =
      Self::build_parameter_provenance(&diff_items, &rule_changes, &origin_source_name);
    for item in &mut provenance_items {
      item.skipped_steps = skipped_rules
        .iter()
        .filter(|s| s.would_touch_keys.iter().any(|k| k == &item.param_key))
        .cloned()
        .collect();
      item.active_bound = parameter_bounds.get(&item.param_key).cloned();
    }

    let mut res_profile = base_profile.clone();
    if let serde_json::Value::Object(mut map) = current_values {
      map.remove("_base");
      if let Some(eval_lh) = base_profile.get_str("layer_height") {
        map.insert(
          "_evaluated_for_layer_height".to_string(),
          serde_json::Value::String(eval_lh),
        );
      }
      for (k, scale) in user_rule_scales {
        if (scale - 1.0).abs() > 1e-4 {
          map.insert(
            format!("_user_rule_scale_{}", k),
            serde_json::Value::String(format!("{:.6}", scale)),
          );
        }
      }
      let mut touched_keys: Vec<String> = rule_sources.keys().cloned().collect();
      touched_keys.sort();
      map.insert(
        "_lua_touched_keys".to_string(),
        serde_json::Value::String(touched_keys.join(";")),
      );
      res_profile.values = map;
    }

    StackEvaluationResult {
      resulting_profile: res_profile,
      diff_items,
      rule_changes,
      provenance_items,
      flush_matrix,
      safe_speed_ceiling,
      execution_logs,
      active_rule_count,
      total_rule_count: rules.len(),
      skipped_rules,
      parameter_bounds,
      infeasible_constraints,
    }
  }

  /// Merges the per-spool filament provenance ledger (`spool_eval` from
  /// `threemf::evaluate_spool_filament_provenance`) into `process_eval`'s UI provenance views
  /// (`provenance_items`, `diff_items`, `rule_changes`, `parameter_bounds`), replacing generic
  /// stock-profile filament placeholders with the active spool's true $x_0 \to \dots \to x_{\text{shipped}}$
  /// ledger so the Provenance UI matches the exported Filament `.json` and 3MF slot key-for-key.
  pub fn merge_active_spool_provenance(
    process_eval: &mut StackEvaluationResult,
    spool_eval: &StackEvaluationResult,
  ) {
    let is_fil = |k: &str| crate::threemf::is_filament_scoped_key(k);
    process_eval.diff_items.retain(|d| !is_fil(&d.key));
    for d in &spool_eval.diff_items {
      if is_fil(&d.key) {
        process_eval.diff_items.push(d.clone());
      }
    }
    process_eval.diff_items.sort_by(|a, b| {
      b.is_modified
        .cmp(&a.is_modified)
        .then(a.category.cmp(&b.category))
        .then(a.label.cmp(&b.label))
    });

    process_eval.provenance_items.retain(|p| !is_fil(&p.param_key));
    for p in &spool_eval.provenance_items {
      if is_fil(&p.param_key) {
        process_eval.provenance_items.push(p.clone());
      }
    }
    process_eval.provenance_items.sort_by(|a, b| {
      b.is_modified
        .cmp(&a.is_modified)
        .then(a.category.cmp(&b.category))
        .then(a.param_label.cmp(&b.param_label))
    });

    process_eval.rule_changes.retain(|rc| !is_fil(&rc.param_key));
    for rc in &spool_eval.rule_changes {
      if is_fil(&rc.param_key) {
        process_eval.rule_changes.push(rc.clone());
      }
    }
    process_eval.rule_changes.sort_by(|a, b| {
      a.step_number
        .cmp(&b.step_number)
        .then(a.param_key.cmp(&b.param_key))
    });

    for (k, b) in &spool_eval.parameter_bounds {
      if is_fil(k) {
        process_eval.parameter_bounds.insert(k.clone(), b.clone());
      }
    }
  }

  pub fn build_parameter_provenance(
    diff_items: &[DiffItem],
    rule_changes: &[RuleBeforeAfterChange],
    origin_source: &str,
  ) -> Vec<ParameterProvenance> {
    let mut result = Vec::with_capacity(diff_items.len());
    for item in diff_items {
      let steps: Vec<RuleBeforeAfterChange> = rule_changes
        .iter()
        .filter(|rc| rc.param_key == item.key)
        .cloned()
        .collect();
      let unit_suffix = if item.unit.is_empty() {
        String::new()
      } else {
        format!(" {}", item.unit)
      };
      let origin_raw = if item.base_value.is_empty() {
        steps
          .first()
          .map(|s| s.before_value.clone())
          .unwrap_or_else(|| "(unset)".to_string())
      } else {
        item.base_value.clone()
      };
      let origin_fmt = if origin_raw == "(unset)" || origin_raw.ends_with(&unit_suffix) {
        origin_raw.clone()
      } else {
        format!("{}{}", origin_raw, unit_suffix)
      };
      let final_fmt = if item.override_value.is_empty() {
        "(unset)".to_string()
      } else {
        format!("{}{}", item.override_value, unit_suffix)
      };
      let net_delta_summary = if item.is_modified || !steps.is_empty() {
        compute_delta_summary(&item.base_value, &item.override_value, &item.unit)
      } else {
        "Unmodified (Baseline retained)".to_string()
      };
      // Only true clobbers (intent lost) mark the parameter as shadowed; `Recomputed` steps are
      // informational (the value was re-derived from this step's other outputs).
      let has_shadowed_steps = steps
        .iter()
        .any(|s| s.shadow_kind == Some(RuleShadowKind::Overwritten));
      result.push(ParameterProvenance {
        param_key: item.key.clone(),
        param_label: item.label.clone(),
        category: item.category.clone(),
        unit: item.unit.clone(),
        origin_source: origin_source.to_string(),
        origin_value: origin_fmt,
        final_value: final_fmt,
        net_delta_summary,
        steps,
        is_modified: item.is_modified,
        skipped_steps: Vec::new(),
        has_shadowed_steps,
        active_bound: None,
      });
    }
    result.sort_by(|a, b| {
      let a_active = a.is_modified || !a.steps.is_empty();
      let b_active = b.is_modified || !b.steps.is_empty();
      b_active
        .cmp(&a_active)
        .then(b.has_shadowed_steps.cmp(&a.has_shadowed_steps))
        .then(b.steps.len().cmp(&a.steps.len()))
        .then(a.category.cmp(&b.category))
        .then(a.param_label.cmp(&b.param_label))
    });
    result
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

          if let Some(cp_arr) = raw_val
            .get("compatible_printers")
            .and_then(|v| v.as_array())
          {
            let has_active_printer = cp_arr.is_empty()
              || cp_arr.iter().any(|x| {
                x.as_str()
                  .map(|s| s.contains(&rig.printer_model) || s == rig.name)
                  .unwrap_or(false)
              });
            if !has_active_printer {
              push_warn(SafetyWarning {
                id: format!("spool_raw_compat_printers:{}", spool.id),
                severity: SafetySeverity::Danger,
                scope_label: format!("Filament Disk JSON: {}", spool.name),
                param_key: "compatible_printers (Hidden in Slicer Dropdown)".to_string(),
                current_value: format!("{:?}", cp_arr),
                safe_range: format!("Omit or include '{}'", rig.printer_model),
                explanation: format!(
                  "{} contains a restrictive `compatible_printers` list on disk that excludes '{}', causing OrcaSlicer / AnycubicSlicerNext to hide this filament from the dropdown and reset the slot to a default filament!",
                  spool.name, rig.printer_model
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

      // Check polymer temperature bounds on the spool via centralized `constants.json`
      let mat = spool.material_type.as_str();
      let mat_up = mat.to_uppercase();
      let canon = FilamentSpool::canonical_polymer_key(mat);
      let (base_min_noz, base_max_noz) = FilamentSpool::polymer_safe_nozzle_bounds(mat);
      let (_base_min_bed, base_max_bed) = FilamentSpool::polymer_safe_bed_bounds(mat);
      let (min_nozzle, max_nozzle, warn_nozzle, max_bed) =
        if spool.finish_type.eq_ignore_ascii_case("Foaming")
          && (canon.starts_with("TPU") || canon == "PEBA" || mat_up.contains("AIR"))
        {
          (180, 285, 281, 105)
        } else if spool.finish_type.eq_ignore_ascii_case("Conductive") || mat_up.contains("ESD") {
          let mn = (base_min_noz - 5).max(180);
          let mx = (base_max_noz + 25).max(290).min(300);
          (mn, mx, mx - 4, base_max_bed + 10)
        } else {
          let mn = if canon == "PCL" {
            base_min_noz - 5
          } else {
            (base_min_noz - 5).max(180)
          };
          let mx = base_max_noz + 5;
          let wn = base_max_noz + 1;
          (mn, mx, wn, base_max_bed + 10)
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

    if let Some(start_gcode) = derived_profile
      .get_str("machine_start_gcode")
      .filter(|s| !s.trim().is_empty())
    {
      if !start_gcode.trim().starts_with("G9111 ") {
        let skipped_leviq2 = start_gcode.contains("LEVIQ2_AUTO_ZOFFSET skipped")
          || (start_gcode.contains("G28 W") && !start_gcode.contains("LEVIQ2_AUTO_ZOFFSET"));
        let uses_kamp_prime = start_gcode.contains("first_layer_print_min[0] - 5");
        push_warn(SafetyWarning {
          id: "derived:machine_start_gcode".to_string(),
          severity: if skipped_leviq2 {
            SafetySeverity::Danger
          } else {
            SafetySeverity::Warning
          },
          scope_label: "Derived Machine G-Code (⚠️ Potentially Dangerous)".to_string(),
          param_key: "machine_start_gcode".to_string(),
          current_value: if skipped_leviq2 {
            "Custom Fast-Start G-Code (⚠️ LEVIQ2_AUTO_ZOFFSET Skipped)".to_string()
          } else if uses_kamp_prime {
            "Custom Fast-Start G-Code (Adaptive KAMP Prime Line)".to_string()
          } else {
            "Custom Unpacked / Fast-Start G-Code".to_string()
          },
          safe_range: "Stock G9111 or 140°C Z-Home + LEVIQ2_AUTO_ZOFFSET ON".to_string(),
          explanation: if skipped_leviq2 {
            "⚠️ POTENTIALLY DANGEROUS: Custom startup G-code skips `LEVIQ2_AUTO_ZOFFSET`. Re-enable `LEVIQ2_AUTO_ZOFFSET` immediately if you swap build plates (e.g. Textured PEI ↔ Smooth PEI) or change nozzles, otherwise the nozzle can gouge the bed!".to_string()
          } else {
            "⚠️ POTENTIALLY DANGEROUS: Custom startup G-code replaces the factory macro. Never home Z (`G28 W` / `G28 Z`) above 140°C, and ensure models sit ≥5mm inside X=0 when using the Adaptive KAMP prime line.".to_string()
          },
        });
      }
    }

    // 5. Check for infeasible monotonic interval bounds (L_k > U_k) in `_bounds`
    if let Some(bounds_obj) = derived_profile
      .values
      .get("_bounds")
      .and_then(|v| v.as_object())
    {
      let mut keys: Vec<&String> = bounds_obj.keys().collect();
      keys.sort();
      for k in keys {
        if let Some(b_map) = bounds_obj.get(k).and_then(|v| v.as_object()) {
          if let (Some(lo), Some(hi)) = (
            b_map.get("min").and_then(|v| v.as_f64()),
            b_map.get("max").and_then(|v| v.as_f64()),
          ) {
            if lo > hi + 1e-9 {
              let min_by = b_map
                .get("min_by")
                .and_then(|v| v.as_str())
                .unwrap_or("lower bound rule");
              let max_by = b_map
                .get("max_by")
                .and_then(|v| v.as_str())
                .unwrap_or("upper bound rule");
              push_warn(SafetyWarning {
                id: format!("infeasible_bound:{}", k),
                severity: SafetySeverity::Danger,
                scope_label: "Constraint Lattice (Infeasible Configuration)".to_string(),
                param_key: k.clone(),
                current_value: format!("L = {:.2} > U = {:.2}", lo, hi),
                safe_range: format!("≤ {:.2} ({}) & ≥ {:.2} ({})", hi, max_by, lo, min_by),
                explanation: format!(
                  "Infeasible Configuration: `{}` requires ≥ {:.2} (set by '{}'), which exceeds the hardware/rule ceiling of ≤ {:.2} (set by '{}').",
                  k, lo, min_by, hi, max_by
                ),
              });
            }
          }
        }
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
      (
        "support_tuning",
        include_str!("../transformations/parametric/support_tuning.lua"),
      ),
      (
        "ironing_tuning",
        include_str!("../transformations/parametric/ironing_tuning.lua"),
      ),
      (
        "dimensional_precision",
        include_str!("../transformations/parametric/dimensional_precision.lua"),
      ),
      (
        "bridge_overhang_tuning",
        include_str!("../transformations/parametric/bridge_overhang_tuning.lua"),
      ),
      (
        "line_width_geometry",
        include_str!("../transformations/parametric/line_width_geometry.lua"),
      ),
      (
        "structural_strength",
        include_str!("../transformations/parametric/structural_strength.lua"),
      ),
      (
        "outer_wall_consistency",
        include_str!("../transformations/parametric/outer_wall_consistency.lua"),
      ),
      (
        "mach_fast_start_gcode",
        include_str!("../transformations/parametric/mach_fast_start_gcode.lua"),
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
      || self.id.starts_with("mach_")
      || matches!(
        self.id.as_str(),
        "support_tuning"
          | "ironing_tuning"
          | "dimensional_precision"
          | "bridge_overhang_tuning"
          | "line_width_geometry"
          | "structural_strength"
      )
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
    assert_eq!(templates.len(), 44);

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
      scope: crate::transformations::RuleScopeFilter::default(),
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
      scope: crate::transformations::RuleScopeFilter::default(),
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
    assert_eq!(petg_trop_stock["filament_z_hop"], "0.20");
    assert_eq!(petg_trop_stock["filament_z_hop_types"], "Spiral Lift");
    assert_eq!(
      petg_trop_stock["filament_retract_lift_enforce"],
      "Top Surfaces Only"
    );
    assert_eq!(petg_trop_stock["additional_cooling_fan_speed"], "40");
    assert_eq!(petg_trop_stock["filament_max_volumetric_speed"], "13.1");
    assert_eq!(petg_trop_stock["filament_wipe"], "1");
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
    assert_eq!(tpu_trop["filament_z_hop"], "0.00");
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
    assert_eq!(pla_cold_humid["filament_wipe"], "1");
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
    assert_eq!(
      diams_for("diamondback"),
      vec!["0.25", "0.4", "0.6", "0.8", "1.0"]
    );
    assert_eq!(diams_for("cm2"), vec!["0.4", "0.6", "0.8"]);
    assert_eq!(diams_for("cht_cm2"), vec!["0.4", "0.6", "0.8"]);
    assert_eq!(diams_for("cht"), vec!["0.4", "0.6", "0.8", "1.0"]);
    assert_eq!(
      diams_for("microswiss_brass"),
      vec!["0.2", "0.3", "0.4", "0.6", "0.8", "1.0"]
    );
  }

  /// Step-1 pipeline unification end-to-end check (doc/pwl-nozzle-compensation-math.md §7/§8):
  /// the per-spool Lua evaluation inside `resolve_spool_orca_filament_map_with_rig_and_overrides`
  /// must actually reach the exported filament preset with the exact PWL chain values, and the
  /// P25 process keys (`travel_acceleration`, `wipe_on_loops`) must ship through
  /// `build_process_preset_json`.
  /// Printer-class coverage (doc/pwl-nozzle-compensation-math.md §5.5/§7): the nozzle/hotend
  /// math is printer-agnostic, but the travel-acceleration breakaway policy and the export
  /// clamp follow the rig's motion class from constants.json `printer_classes` — a CoreXY
  /// (Kobra S1) and a bed-slinger (Kobra 3) must get different numbers from the same rule,
  /// and a machine whose hotend is rated below the FlowTech ceiling must clamp to its own
  /// rating.
  #[test]
  fn test_pwl_policies_follow_printer_kinematics_class() {
    let kobra_s1 = MachineRig::kobra_s1_hardened_steel_png();
    let kobra_3 = MachineRig::get_template_catalog()
      .into_iter()
      .find(|r| r.printer_model == "Anycubic Kobra 3")
      .expect("Kobra 3 bed-slinger template rig");

    // Classification: kinematics strings from the app's machine switcher + disk import.
    assert_eq!(kobra_s1.kinematics_class(), "corexy");
    assert_eq!(kobra_3.kinematics_class(), "bedslinger");

    // Classification across the full generated catalog (66 vendors, 380 machines): the
    // generator labels bed-slinger families by model name (constants
    // `bedslinger_name_tokens`), and the runtime classifier picks the class up from the
    // machine's kinematics string — this is what wizard-built rigs carry.
    {
      let catalog = crate::catalog::load_merged_catalog();
      let class_of = |name: &str| -> String {
        let m = catalog
          .machines
          .iter()
          .find(|m| m.name == name)
          .unwrap_or_else(|| panic!("catalog machine {:?} present", name));
        crate::constants::constants().kinematics_class(&m.kinematics)
      };
      let bedslingers = [
        "Creality Ender-3",
        "Anycubic Kobra 3",
        "Bambu Lab A1",
        "Prusa MK4S",
        "Prusa MINI",
        "Sovol SV06",
        "Artillery Sidewinder X2",
        "BIQU B1",
        "Creality CR-10 V2",
      ];
      for name in bedslingers {
        assert_eq!(
          class_of(name),
          "bedslinger",
          "{} must classify as bed-slinger",
          name
        );
      }
      let corexys = [
        "Anycubic Kobra S1",
        "Bambu Lab X1 Carbon",
        "Bambu Lab P1S",
        "Voron 2.4 350",
        "Voron Trident 300",
        "Prusa XL",
        "Prusa CORE One",
        "BIQU BX",
        "Anycubic Kobra S1 Max",
      ];
      for name in corexys {
        assert_eq!(class_of(name), "corexy", "{} must classify as CoreXY", name);
      }

      // Machine constants: curated `machines[]` entries are deliberate overrides; printers
      // without one DERIVE from the catalog (own vendor prefix, global limits) instead of
      // inheriting the Kobra S1's.
      let curated = crate::catalog::machine_constants("Creality Ender-3");
      assert_eq!(
        (curated.vendor_prefix.as_str(), curated.max_hotend_temp_c),
        ("Creality", 260)
      );
      let derived_sovol = crate::catalog::machine_constants("Sovol SV06");
      assert_eq!(derived_sovol.vendor_prefix, "Sovol");
      assert_eq!(derived_sovol.max_hotend_temp_c, 300);
      let derived_prusa = crate::catalog::machine_constants("Prusa MK4S");
      assert_eq!(derived_prusa.vendor_prefix, "Prusa");
      // Rig-aware inherits use the derived vendor too — no "Anycubic @Sovol SV06".
      let petg_entry = catalog.filaments.iter().find(|f| f.code == "PETG").unwrap();
      let spool = FilamentSpool::from_catalog_entry(1, petg_entry, "orca_brass");
      let inherits = crate::threemf::resolve_filament_inherits_for_rig(
        &spool,
        Some(&MachineRig {
          printer_model: "Sovol SV06".to_string(),
          nozzle_diameter: "0.4".to_string(),
          ..kobra_s1.clone()
        }),
      );
      assert_eq!(inherits, "Sovol PETG @Sovol SV06 0.4 nozzle");
    }
    let corexy = kobra_s1.travel_accel_policy();
    let slinger = kobra_3.travel_accel_policy();
    assert_eq!(
      (
        corexy.travel_accel_stock_mm2,
        corexy.travel_accel_target_mm2,
        corexy.travel_accel_floor_mm2
      ),
      (10000, 12000, 9000)
    );
    assert!(slinger.travel_accel_target_mm2 < corexy.travel_accel_target_mm2);
    assert!(slinger.travel_accel_floor_mm2 < corexy.travel_accel_floor_mm2);

    // P25 travel accel follows the class through the real rule file.
    let stringing_lua = include_str!("../transformations/symptom_stringing_diamondback.lua");
    let run_p25 = |ctx: serde_json::Value, a0: &str| {
      let cfg = json!({ "travel_acceleration": a0 });
      let (out, _, _) = crate::lua_engine::execute_transformation_lua(stringing_lua, &cfg, &ctx)
        .expect("stringing rule executes");
      out["travel_acceleration"].as_str().unwrap().to_string()
    };
    let corexy_ctx = json!({
      "nozzle_type": "diamondback", "nozzle_diameter": "0.4",
      "travel_accel_target_mm2": corexy.travel_accel_target_mm2,
      "travel_accel_floor_mm2": corexy.travel_accel_floor_mm2,
    });
    let slinger_ctx = json!({
      "nozzle_type": "diamondback", "nozzle_diameter": "0.4",
      "travel_accel_target_mm2": slinger.travel_accel_target_mm2,
      "travel_accel_floor_mm2": slinger.travel_accel_floor_mm2,
    });
    // CoreXY: 10000 -> 10700 (boost), 7000 -> boost 8750 then floor 9000. Bed-slinger:
    // 6000 -> 6700 (boost toward 8000), 4000 -> 5400 (boost clears the floor), 3000 ->
    // boost 4050 then floor 5000. Above-target inputs pass through unchanged.
    assert_eq!(run_p25(corexy_ctx.clone(), "10000"), "10700");
    assert_eq!(run_p25(corexy_ctx.clone(), "7000"), "9000");
    assert_eq!(run_p25(slinger_ctx.clone(), "6000"), "6700");
    assert_eq!(run_p25(slinger_ctx.clone(), "4000"), "5400");
    assert_eq!(run_p25(slinger_ctx.clone(), "3000"), "5000");
    assert_eq!(run_p25(slinger_ctx.clone(), "15000"), "15000");

    // Export clamp restores the CLASS stock value (not a hard-coded Kobra 10000).
    let procs = crate::models::ProcessProfile::get_default_processes();
    let proc = &procs[0];
    let mut overrides = serde_json::Map::new();
    overrides.insert("seam_slope_type".to_string(), json!("scarf")); // skip stack re-eval
    overrides.insert("travel_acceleration".to_string(), json!("4000"));
    let k3_json = crate::threemf::build_process_preset_json(proc, &kobra_3, &overrides);
    let k3_map: serde_json::Map<String, serde_json::Value> =
      serde_json::from_str::<serde_json::Value>(&k3_json)
        .unwrap()
        .as_object()
        .unwrap()
        .clone();
    assert_eq!(
      k3_map["travel_acceleration"].as_str(),
      Some(slinger.travel_accel_stock_mm2.to_string().as_str()),
      "bed-slinger clamp must restore the class stock value"
    );

    // P10 ceiling: a machine rated below the FlowTech 300 C ceiling clamps to its own rating.
    let thermal_lua = include_str!("../transformations/thermal_conductivity.lua");
    let hot_cfg = json!({ "nozzle_temperature": ["290"] });
    let hot_ctx = json!({
      "nozzle_type": "diamondback", "nozzle_diameter": "0.4",
      "hotend_max_temp_c": 270,
    });
    let (out, _, _) =
      crate::lua_engine::execute_transformation_lua(thermal_lua, &hot_cfg, &hot_ctx)
        .expect("thermal rule executes");
    // 290 -> 284.3 raw, clamped to the 270 C machine rating.
    assert_eq!(out["nozzle_temperature"][0].as_str(), Some("270"));

    // The nozzle/hotend compensation itself is printer-agnostic: the same Diamondback PETG
    // spool exports identical filament values on the bed-slinger as on the CoreXY.
    let rig_db = MachineRig::get_template_catalog()
      .into_iter()
      .find(|r| r.nozzle_type == "diamondback")
      .unwrap();
    let mut k3_db = kobra_3.clone();
    k3_db.nozzle_type = "diamondback".to_string();
    k3_db.nozzle_material = "Diamondback PCD".to_string();
    k3_db.nozzle_diameter = "0.4".to_string();
    k3_db.addons.push("flowtech_hotend".to_string());
    let catalog = crate::catalog::load_merged_catalog();
    let petg_entry = catalog.filaments.iter().find(|f| f.code == "PETG").unwrap();
    let mut spool_db = FilamentSpool::from_catalog_entry(1, petg_entry, &rig_db.nozzle_type);
    spool_db.flow_ratio = 0.96;
    let mk_map = |rig: &MachineRig| {
      let s = crate::threemf::export_standalone_filament_json_with_overrides(
        &spool_db,
        rig,
        &serde_json::Map::new(),
      );
      serde_json::from_str::<serde_json::Value>(&s)
        .unwrap()
        .as_object()
        .unwrap()
        .clone()
    };
    let corexy_map = mk_map(&rig_db);
    let slinger_map = mk_map(&k3_db);
    for key in [
      "nozzle_temperature",
      "filament_retraction_length",
      "filament_retraction_speed",
      "filament_flow_ratio",
      "filament_z_hop",
    ] {
      assert_eq!(
        corexy_map.get(key),
        slinger_map.get(key),
        "nozzle compensation must not depend on printer model: {}",
        key
      );
    }
    assert_eq!(corexy_map["nozzle_temperature"][0], json!("223"));
  }

  #[test]
  fn test_diamondback_pwl_wiring_ships_rule_values_to_exports() {
    let rig = MachineRig::get_template_catalog()
      .into_iter()
      .find(|r| r.nozzle_type == "diamondback")
      .expect("diamondback template rig");
    let catalog = crate::catalog::load_merged_catalog();
    let petg_entry = catalog.filaments.iter().find(|f| f.code == "PETG").unwrap();
    let mut spool = FilamentSpool::from_catalog_entry(1, petg_entry, &rig.nozzle_type);
    // Pin the brand calibration the expectations below assume, so the test cannot drift when
    // the generated catalog's per-code winner changes (pinned OrcaSlicer release bump).
    spool.flow_ratio = 0.96;

    let empty_overrides = serde_json::Map::new();
    let json_str = crate::threemf::export_standalone_filament_json_with_overrides(
      &spool,
      &rig,
      &empty_overrides,
    );
    let map: serde_json::Map<String, serde_json::Value> =
      serde_json::from_str::<serde_json::Value>(&json_str)
        .unwrap()
        .as_object()
        .unwrap()
        .clone();
    let first = |k: &str| map[k][0].as_str().unwrap().to_string();
    // Catalog PETG brass base 230 °C / flow 0.96 / retract 0.80 @ 40 / z-hop 0.15 Spiral:
    // P10 curve 230 -> 220 (-10.4), P25 gated trim -> 218, final clamped to the PETG floor−2
    // (225−2 = 223). P12: 0.80 -> 0.70 @ 44; P25 recovery -> 0.75, quadratic damp -> 38.
    // Flow 0.96 -> 0.954 (die-swell trim). Z-hop 0.15 -> 0.22 Spiral Lift.
    assert_eq!(first("nozzle_temperature"), "223");
    assert_eq!(first("nozzle_temperature_BRASS"), "223");
    assert_eq!(first("nozzle_temperature_HS"), "223");
    assert_eq!(first("filament_retraction_length"), "0.75");
    assert_eq!(first("filament_retraction_speed"), "38");
    assert_eq!(first("filament_flow_ratio"), "0.954");
    assert_eq!(first("filament_z_hop"), "0.22");
    assert_eq!(first("filament_z_hop_types"), "Spiral Lift");
    // Polymer safety band still enforced around the rule output, allowing only the approved
    // −2 °C anti-stringing excursion below the floor (threemf wiring clamps to floor−2).
    let (lo, hi) = FilamentSpool::polymer_safe_nozzle_bounds("PETG");
    let t: i32 = first("nozzle_temperature").parse().unwrap();
    assert!(
      (lo - 2..=hi).contains(&t),
      "PETG temp {} outside [{}, {}]",
      t,
      lo - 2,
      hi
    );

    // Process side: travel_acceleration boost (clears the <8000 -> 10000 clamp) + wipe ship.
    let procs = crate::models::ProcessProfile::get_default_processes();
    let proc = &procs[0];
    let mut base = OrcaProfile::get_stock_kobra_s1();
    base.set_str("layer_height", &format!("{:.2}", proc.layer_height));
    let mut rules = TransformationEngine::get_default_rules();
    TransformationEngine::sync_rules_for_rig(
      &mut rules,
      &rig.nozzle_type,
      &rig.nozzle_material,
      rig.ace_pro_units,
    );
    let ctx = json!({
      "printer_model": rig.printer_model,
      "nozzle_type": rig.nozzle_type,
      "nozzle_material": rig.nozzle_material,
      "nozzle_diameter": rig.nozzle_diameter,
      "hotend_max_flow": rig.max_volumetric_flow_limit,
      "melt_zone_multiplier": rig.melt_zone_multiplier,
      "layer_height": proc.layer_height,
      "line_width": proc.line_width,
      "addons": rig.addons,
    });
    let eval = TransformationEngine::evaluate_stack(&base, &rules, &ctx);
    let proc_json =
      crate::threemf::build_process_preset_json(proc, &rig, &eval.resulting_profile.values);
    let pmap: serde_json::Map<String, serde_json::Value> =
      serde_json::from_str::<serde_json::Value>(&proc_json)
        .unwrap()
        .as_object()
        .unwrap()
        .clone();
    assert_eq!(pmap["travel_acceleration"].as_str(), Some("10700"));
    assert_eq!(pmap["wipe_on_loops"].as_str(), Some("1"));
  }

  #[test]
  fn test_fast_start_gcode_kobra_s1_and_bambu_lab_knobs_and_3mf_parity() {
    let templates = TransformationEngine::get_parameterized_calibration_templates();
    let fast_start = templates
      .iter()
      .find(|t| t.id == "mach_fast_start_gcode")
      .expect("mach_fast_start_gcode template exists");
    assert!(fast_start.name.contains("POTENTIALLY DANGEROUS"));

    // 1. Default preset: Kobra S1 Trimmed Safe (140°C direct, 2× WIPE_NOZZLE1, LEVIQ2_AUTO_ZOFFSET ON, E-14 cut, E-5 end, PETG/ABS unbottleneck)
    let mut ans = fast_start.default_answers();
    let (title1, _desc1, badge1, lua1) = fast_start.generate_from_answers(&ans, "all");
    // Without a rig in context the title is rig-agnostic (never names another brand)
    assert!(
      title1.contains("Startup G-Code — Rig-Specific"),
      "title: {}",
      title1
    );
    assert!(!title1.contains("Bambu"), "title: {}", title1);
    let _ = badge1;
    let kobra_desc = describe_lua_rule(
      &lua1,
      &HashMap::new(),
      "all",
      &json!({ "printer_model": "Anycubic Kobra S1" }),
    )
    .unwrap();
    let kobra_title = kobra_desc.title.unwrap_or_default();
    assert!(
      kobra_title.contains("Startup G-Code: Kobra S1 Unpacked G9111"),
      "title: {}",
      kobra_title
    );
    assert!(!kobra_title.contains("Bambu"), "title: {}", kobra_title);
    assert!(kobra_desc
      .badge
      .unwrap_or_default()
      .contains("LEVIQ2 Z-Offset ON"));
    let (cfg1, _, _) = execute_transformation_lua(
      &lua1,
      &json!({
        "filament_retraction_length": "1.50",
        "filament_max_volumetric_speed": "12.0"
      }),
      &json!({ "printer_model": "Anycubic Kobra S1", "filament_type": "PETG" }),
    )
    .expect("kobra_s1_trimmed_safe executes");
    let gcode1 = cfg1["machine_start_gcode"].as_str().unwrap();
    assert!(gcode1.contains("M109 S140"));
    assert!(gcode1.contains("WIPE_NOZZLE1 ; Single brush wipe pass (1/2)"));
    assert!(gcode1.contains("WIPE_NOZZLE1 ; Single brush wipe pass (2/2)"));
    assert!(!gcode1.contains("WIPE_NOZZLE ; Execute 14x"));
    assert!(gcode1.contains("G28 W"));
    assert!(gcode1.contains("LEVIQ2_AUTO_ZOFFSET"));
    assert!(gcode1.contains("BED_MESH_PROFILE LOAD=\"default\""));
    assert!(gcode1.contains("SET_GCODE_OFFSET Z_ADJUST=-0.03 MOVE=1"));
    assert!(gcode1.contains("SET_GCODE_OFFSET Z_ADJUST=+0.02 MOVE=1"));
    assert!(gcode1.contains("G4 S240 ; Heat-soak for 4 minutes"));
    assert!(cfg1["machine_end_gcode"]
      .as_str()
      .unwrap()
      .contains("G1 E-5 F1800 ; Retract 5mm immediately"));
    assert!(cfg1["machine_end_gcode"]
      .as_str()
      .unwrap()
      .contains("G1 Z150 F3000"));
    assert!(cfg1["change_filament_gcode"]
      .as_str()
      .unwrap()
      .contains("G1 E-14 F1200"));
    assert_eq!(cfg1["flush_multiplier"], "0.65");
    assert_eq!(cfg1["filament_retraction_length"], "0.70");
    assert_eq!(cfg1["filament_max_volumetric_speed"], "19.0");
    assert!(cfg1["before_layer_change_gcode"]
      .as_str()
      .unwrap()
      .contains("BEFORE_LAYER_CHANGE"));

    // 2. Kobra S1 1:1 Unpacked Stock G9111 (170°C -> 14x WIPE_NOZZLE -> 140°C cooldown on WIPE_STOP -> G28 W + LEVIQ2)
    fast_start.apply_answer_change(
      &mut ans,
      "startup_preset",
      "kobra_s1_unpacked_1to1".to_string(),
    );
    assert_eq!(
      ans.get("preheat_wipe_temp_c").map(|s| s.as_str()),
      Some("170")
    );
    assert_eq!(ans.get("wipe_passes").map(|s| s.as_str()), Some("14"));
    let (_, _, _, lua_1to1) = fast_start.generate_from_answers(&ans, "all");
    let (cfg_1to1, _, _) = execute_transformation_lua(
      &lua_1to1,
      &json!({}),
      &json!({ "printer_model": "Anycubic Kobra S1" }),
    )
    .expect("kobra_s1_unpacked_1to1 executes");
    let gcode_1to1 = cfg_1to1["machine_start_gcode"].as_str().unwrap();
    assert!(gcode_1to1.contains("M109 S170 ; Heat nozzle to 170C"));
    assert!(gcode_1to1.contains("WIPE_NOZZLE ; Execute 14x native brush wipe"));
    assert!(gcode_1to1
      .contains("M109 S140 ; GOLDEN RULE: Cool nozzle to 140C on silicone pad before Z-homing!"));
    assert!(gcode_1to1.contains("LEVIQ2_AUTO_ZOFFSET"));

    // 3. Bambu Lab Fast-Start + Keep AMS Loaded at End + E-18 Cut Poop-Saver
    fast_start.apply_answer_change(
      &mut ans,
      "startup_preset",
      "bambu_fast_same_spool".to_string(),
    );
    let (_, _, _, lua_bambu) = fast_start.generate_from_answers(&ans, "all");
    let bambu_desc = describe_lua_rule(
      &lua_bambu,
      &HashMap::new(),
      "all",
      &json!({ "printer_model": "Bambu Lab P1S" }),
    )
    .unwrap();
    let badge_bambu = bambu_desc.badge.unwrap_or_default();
    assert!(
      badge_bambu.contains("Bambu Fast-Start"),
      "badge: {}",
      badge_bambu
    );
    assert!(
      badge_bambu.contains("Keep AMS Loaded"),
      "badge: {}",
      badge_bambu
    );
    let (cfg_bambu, _, _) = execute_transformation_lua(
      &lua_bambu,
      &json!({}),
      &json!({ "printer_model": "Bambu Lab P1S" }),
    )
    .expect("bambu_fast_same_spool executes");
    let gcode_bambu_start = cfg_bambu["machine_start_gcode"].as_str().unwrap();
    let gcode_bambu_end = cfg_bambu["machine_end_gcode"].as_str().unwrap();
    let gcode_bambu_cut = cfg_bambu["change_filament_gcode"].as_str().unwrap();
    assert!(gcode_bambu_start.contains("M140 S[bed_temperature_initial_layer_single]"));
    assert!(gcode_bambu_start.contains("Mech Mode Fast Check Skipped"));
    assert!(gcode_bambu_start.contains("G28 Z ; Home Z with clean 140C nozzle"));
    assert!(gcode_bambu_start.contains("G29.1 Z-0.03"));
    assert!(gcode_bambu_start.contains("G4 S300 ; Dwell 5 minutes"));
    assert!(gcode_bambu_start.contains("first_layer_print_min[0] - 5"));
    assert!(gcode_bambu_end.contains("G1 E-6 F1800 ; Quickly retract 6mm"));
    assert!(gcode_bambu_end.contains("G1 Z150 F3000"));
    assert!(gcode_bambu_end
      .contains("M620 S255 / T255 / M621 S255 AMS cut & unload intentionally skipped"));
    assert!(gcode_bambu_cut.contains("G1 E-18 F1200 ; Pull 18mm"));
    assert_eq!(cfg_bambu["flush_multiplier"], "0.60");
    assert!(!gcode_bambu_start.contains("G9111"));
    assert!(cfg_bambu.get("before_layer_change_gcode").is_none());
    assert!(cfg_bambu.get("_kobra_unbottleneck_petg_abs").is_none());

    // 3b. The family is driven by the rig, NOT the preset: a "Bambu" preset on a Kobra S1 rig emits Kobra G-code
    let (cfg_bambu_preset_on_kobra, _, _) = execute_transformation_lua(
      &lua_bambu,
      &json!({}),
      &json!({ "printer_model": "Anycubic Kobra S1" }),
    )
    .unwrap();
    let k_start = cfg_bambu_preset_on_kobra["machine_start_gcode"]
      .as_str()
      .unwrap();
    assert!(k_start.contains("G28 W") && !k_start.contains("G29.1"));
    assert!(cfg_bambu_preset_on_kobra["change_filament_gcode"]
      .as_str()
      .unwrap()
      .contains("G1 E-14 F1200"));

    // 3c. Kobra S1 Max: stock G9111 wrapper + bed metadata, S1 Max shutdown (M141), no S1-only bits
    let (cfg_s1max, _, _) = execute_transformation_lua(
      &lua1,
      &json!({ "filament_retraction_length": "1.50", "filament_max_volumetric_speed": "12.0" }),
      &json!({ "printer_model": "Anycubic Kobra S1 Max", "filament_type": "PETG" }),
    )
    .unwrap();
    let s1max_start = cfg_s1max["machine_start_gcode"].as_str().unwrap();
    assert!(s1max_start.contains("G9111 bedTemp=[first_layer_bed_temperature]"));
    assert!(s1max_start.contains("; first_layer_fan_speed = 2"));
    assert!(s1max_start.contains("X{print_bed_max[0]*0.5}"));
    assert!(s1max_start.contains("SET_GCODE_OFFSET Z_ADJUST=-0.03 MOVE=1"));
    for s1_only in [
      "WIPE_NOZZLE",
      "TO_THROW_POSITION",
      "LEVIQ2_AUTO_ZOFFSET",
      "X125 Y125",
    ] {
      assert!(
        !s1max_start.contains(s1_only),
        "S1 Max start leaked '{}'",
        s1_only
      );
    }
    let s1max_end = cfg_s1max["machine_end_gcode"].as_str().unwrap();
    assert!(s1max_end.contains("M141 S0") && s1max_end.contains("G1 E-5 F1800"));
    assert!(!s1max_end.contains("X44"));
    assert!(cfg_s1max.get("change_filament_gcode").is_none());
    assert!(cfg_s1max.get("flush_multiplier").is_none());
    assert!(cfg_s1max.get("before_layer_change_gcode").is_none());
    assert_eq!(cfg_s1max["filament_retraction_length"], "1.50");
    let s1max_title = describe_lua_rule(
      &lua1,
      &HashMap::new(),
      "all",
      &json!({ "printer_model": "Anycubic Kobra S1 Max" }),
    )
    .unwrap()
    .title
    .unwrap_or_default();
    assert!(
      s1max_title.contains("Kobra S1 Max"),
      "title: {}",
      s1max_title
    );

    // 3d. Unsupported rigs: stock G-code left completely untouched + clear "no optimized G-code" message
    for model in [
      "Anycubic Kobra 3",
      "Anycubic Kobra 3 Max",
      "Anycubic Kobra 2 Max",
      "Bambu Lab A1 mini",
      "Prusa MK4",
      "",
    ] {
      let base_cfg =
        json!({ "filament_retraction_length": "1.50", "machine_start_gcode": "STOCK" });
      let (cfg_none, _, _) =
        execute_transformation_lua(&lua1, &base_cfg, &json!({ "printer_model": model })).unwrap();
      assert_eq!(
        cfg_none, base_cfg,
        "{} must keep stock machine G-code",
        model
      );
      if !model.is_empty() {
        let d = describe_lua_rule(
          &lua1,
          &HashMap::new(),
          "all",
          &json!({ "printer_model": model }),
        )
        .unwrap();
        let t = d.title.unwrap_or_default();
        assert!(
          t.contains("No Optimized G-Code") && t.contains(model),
          "title: {}",
          t
        );
      }
    }

    // 4. Verify 100% parity between standalone machine/filament .json and .3mf project_settings.config when enabled!
    let rig = MachineRig::kobra_s1_hardened_steel_png();
    let proc = crate::models::ProcessProfile::get_default_processes().remove(1);
    let spools: Vec<FilamentSpool> = crate::dir_import::import_scanned_directory(
      &crate::dir_import::build_reference_anycubic_entries(),
    )
    .unwrap()
    .filaments
    .into_iter()
    .take(4)
    .collect();
    let flush_matrix = vec!["0".to_string(); 16];
    let overrides = cfg1.as_object().unwrap().clone();
    let proj_json = crate::threemf::build_project_settings_config_json(
      &rig,
      &proc,
      &spools,
      &flush_matrix,
      &overrides,
    );
    assert!(proj_json.contains("machine_start_gcode"));
    assert!(proj_json.contains("machine_end_gcode"));
    assert!(proj_json.contains("change_filament_gcode"));
    assert!(proj_json.contains("before_layer_change_gcode"));
  }

  #[test]
  fn test_parameter_provenance_and_lineage_tracks_successive_transformations() {
    let mut base = OrcaProfile::get_stock_kobra_s1();
    base.set_str("nozzle_temperature", "230");
    let rules = vec![
      TransformationRule {
        id: "r_step1".to_string(),
        name: "Hardened Steel +5°C Boost".to_string(),
        description: "Boost nozzle temp by +5°C".to_string(),
        category: "Nozzle Material".to_string(),
        icon: "🔩".to_string(),
        enabled: true,
        priority: 10,
        scope: RuleScopeFilter::default(),
        condition_expr: "all".to_string(),
        lua_code: r#"
local t = tonumber(config.nozzle_temperature or "230")
config.nozzle_temperature = tostring(t + 5)
print("Boosted nozzle_temperature by +5C for Hardened Steel")
"#
        .to_string(),
      },
      TransformationRule {
        id: "r_step2".to_string(),
        name: "High-Flow Thermal Multiplier (×1.02)".to_string(),
        description: "Scale nozzle temp by 2%".to_string(),
        category: "Thermal".to_string(),
        icon: "🔥".to_string(),
        enabled: true,
        priority: 20,
        scope: RuleScopeFilter::default(),
        condition_expr: "all".to_string(),
        lua_code: r#"
local t = tonumber(config.nozzle_temperature or "230")
config.nozzle_temperature = string.format("%.0f", t * 1.02)
print("Scaled nozzle_temperature by 1.02x for high-flow extrusion")
"#
        .to_string(),
      },
    ];

    let eval = TransformationEngine::evaluate_stack(&base, &rules, &json!({}));
    let temp_prov = eval
      .provenance_items
      .iter()
      .find(|p| p.param_key == "nozzle_temperature")
      .expect("nozzle_temperature provenance should exist");

    assert!(temp_prov.is_modified);
    assert_eq!(temp_prov.origin_value, "230 °C");
    assert_eq!(temp_prov.final_value, "240 °C");
    assert_eq!(temp_prov.steps.len(), 2);

    assert_eq!(temp_prov.steps[0].step_number, 1);
    assert_eq!(temp_prov.steps[0].rule_name, "Hardened Steel +5°C Boost");
    assert_eq!(temp_prov.steps[0].before_value, "230 °C");
    assert_eq!(temp_prov.steps[0].after_value, "235 °C");
    assert!(
      temp_prov.steps[0].delta_summary.contains("+5 °C"),
      "step 1 delta_summary: {}",
      temp_prov.steps[0].delta_summary
    );
    assert!(temp_prov.steps[0]
      .rule_log
      .contains("Boosted nozzle_temperature by +5C"));

    assert_eq!(temp_prov.steps[1].step_number, 2);
    assert_eq!(
      temp_prov.steps[1].rule_name,
      "High-Flow Thermal Multiplier (×1.02)"
    );
    assert_eq!(temp_prov.steps[1].before_value, "235 °C");
    assert_eq!(temp_prov.steps[1].after_value, "240 °C");
    assert!(
      temp_prov.steps[1].delta_summary.contains("+5 °C"),
      "step 2 delta_summary: {}",
      temp_prov.steps[1].delta_summary
    );
    assert!(
      temp_prov.net_delta_summary.contains("+10 °C"),
      "net_delta_summary: {}",
      temp_prov.net_delta_summary
    );
  }

  #[test]
  fn test_four_core_slicer_config_transformation_packs_and_per_process_export() {
    let templates = TransformationEngine::get_parameterized_calibration_templates();

    // 1. Pack #1: Support Detachability & Tree Tuning (`support_tuning.lua`)
    let support_tpl = templates
      .iter()
      .find(|t| t.id == "support_tuning")
      .expect("support_tuning template exists");
    let mut sup_ans = support_tpl.default_answers();
    assert_eq!(
      sup_ans.get("support_preset").map(|s| s.as_str()),
      Some("auto_polymer")
    );
    let (_, _, _, sup_lua) = support_tpl.generate_from_answers(&sup_ans, "all");

    // In auto_polymer mode at 0.20mm LH: PLA gets 0.20 + 0.02 = 0.22mm; PETG gets 0.20 + 0.06 = 0.26mm
    let (sup_pla, _, _) = execute_transformation_lua(
      &sup_lua,
      &json!({ "layer_height": "0.20" }),
      &json!({ "filament_type": "PLA", "layer_height": 0.20 }),
    )
    .expect("support_tuning PLA executes");
    assert_eq!(sup_pla["enable_support"], "1");
    assert_eq!(sup_pla["support_type"], "tree(auto)");
    assert_eq!(sup_pla["support_style"], "tree_slim");
    assert_eq!(sup_pla["support_top_z_distance"], "0.22");
    assert_eq!(sup_pla["support_bottom_z_distance"], "0.22");
    assert_eq!(sup_pla["support_interface_top_layers"], "3");
    assert_eq!(sup_pla["support_interface_spacing"], "0.15");
    assert_eq!(sup_pla["support_interface_speed"], "60");

    let (sup_petg, _, _) = execute_transformation_lua(
      &sup_lua,
      &json!({ "layer_height": "0.20" }),
      &json!({ "filament_type": "PETG", "layer_height": 0.20 }),
    )
    .expect("support_tuning PETG executes");
    assert_eq!(sup_petg["support_top_z_distance"], "0.26");

    // Zero-gap dissimilar interface preset snaps top_z_offset_mm and interface_spacing_mm to 0.00
    support_tpl.apply_answer_change(
      &mut sup_ans,
      "support_preset",
      "zero_gap_dissimilar".to_string(),
    );
    assert_eq!(
      sup_ans.get("top_z_offset_mm").map(|s| s.as_str()),
      Some("0.00")
    );
    assert_eq!(
      sup_ans.get("interface_spacing_mm").map(|s| s.as_str()),
      Some("0.00")
    );
    let (_, _, _, sup_zero_lua) = support_tpl.generate_from_answers(&sup_ans, "all");
    let (sup_zero, _, _) = execute_transformation_lua(
      &sup_zero_lua,
      &json!({ "layer_height": "0.20" }),
      &json!({ "filament_type": "PETG", "layer_height": 0.20 }),
    )
    .expect("support_tuning zero_gap executes");
    assert_eq!(sup_zero["support_top_z_distance"], "0.00");
    assert_eq!(sup_zero["support_interface_spacing"], "0.00");

    // Incompatible tree(auto) + snug is blocked by validate()
    sup_ans.insert("support_type".to_string(), "tree(auto)".to_string());
    sup_ans.insert("support_style".to_string(), "snug".to_string());
    assert!(support_tpl.validate_answers(&sup_ans, "all").is_err());

    // 2. Pack #2: Top Surface Ironing & Gloss (`ironing_tuning.lua`)
    let ironing_tpl = templates
      .iter()
      .find(|t| t.id == "ironing_tuning")
      .expect("ironing_tuning template exists");
    let mut iron_ans = ironing_tpl.default_answers();
    let (_, _, _, iron_lua) = ironing_tpl.generate_from_answers(&iron_ans, "all");

    // Polymer-aware auto_polymer: 12% PLA @ 35mm/s, 18% PETG @ 30mm/s, 22% ASA @ 40mm/s
    let (iron_pla, _, _) =
      execute_transformation_lua(&iron_lua, &json!({}), &json!({ "filament_type": "PLA" }))
        .expect("ironing PLA executes");
    assert_eq!(iron_pla["ironing_type"], "topmost");
    assert_eq!(iron_pla["ironing_flow"], "12%");
    assert_eq!(iron_pla["ironing_speed"], "35");
    assert_eq!(iron_pla["ironing_spacing"], "0.15");
    assert_eq!(iron_pla["top_surface_pattern"], "monotonicline");
    assert_eq!(iron_pla["only_one_wall_top"], "1");

    let (iron_petg, _, _) =
      execute_transformation_lua(&iron_lua, &json!({}), &json!({ "filament_type": "PETG" }))
        .expect("ironing PETG executes");
    assert_eq!(iron_petg["ironing_flow"], "18%");
    assert_eq!(iron_petg["ironing_speed"], "30");

    let (iron_asa, _, _) =
      execute_transformation_lua(&iron_lua, &json!({}), &json!({ "filament_type": "ASA" }))
        .expect("ironing ASA executes");
    assert_eq!(iron_asa["ironing_flow"], "22%");
    assert_eq!(iron_asa["ironing_speed"], "40");
    assert_eq!(iron_asa["ironing_spacing"], "0.12");

    // Excessive flow (>25%) at slow speed (<25 mm/s) is blocked by validate()
    iron_ans.insert("ironing_flow_pct".to_string(), "30".to_string());
    iron_ans.insert("ironing_speed_mms".to_string(), "20".to_string());
    assert!(ironing_tpl.validate_answers(&iron_ans, "all").is_err());

    // 3. Pack #3: Dimensional Tolerance & Mechanical Fit (`dimensional_precision.lua`)
    let dim_tpl = templates
      .iter()
      .find(|t| t.id == "dimensional_precision")
      .expect("dimensional_precision template exists");
    let mut dim_ans = dim_tpl.default_answers();
    dim_tpl.apply_answer_change(&mut dim_ans, "fit_preset", "mechanical_exact".to_string());
    assert_eq!(
      dim_ans.get("wall_sequence").map(|s| s.as_str()),
      Some("outer wall/inner wall")
    );
    let (_, _, _, dim_lua) = dim_tpl.generate_from_answers(&dim_ans, "all");
    let (dim_cfg, _, _) = execute_transformation_lua(&dim_lua, &json!({}), &json!({}))
      .expect("dimensional_precision executes");
    assert_eq!(dim_cfg["wall_sequence"], "outer wall/inner wall");
    assert_eq!(dim_cfg["precise_outer_wall"], "1");
    assert_eq!(dim_cfg["xy_hole_compensation"], "0.080");
    assert_eq!(dim_cfg["xy_contour_compensation"], "-0.020");
    assert_eq!(dim_cfg["elefant_foot_compensation"], "0.15");

    // 4. Pack #4: Bridges & Steep Overhangs (`bridge_overhang_tuning.lua`)
    let br_tpl = templates
      .iter()
      .find(|t| t.id == "bridge_overhang_tuning")
      .expect("bridge_overhang_tuning template exists");
    let mut br_ans = br_tpl.default_answers();
    let (_, _, _, br_lua) = br_tpl.generate_from_answers(&br_ans, "all");
    let (br_cfg, _, _) = execute_transformation_lua(&br_lua, &json!({}), &json!({}))
      .expect("bridge_overhang_tuning executes");
    assert_eq!(br_cfg["bridge_flow"], "0.92");
    assert_eq!(br_cfg["bridge_speed"], "28");
    assert_eq!(br_cfg["enable_overhang_speed"], "1");
    assert_eq!(br_cfg["detect_overhang_wall"], "1");
    assert_eq!(br_cfg["overhang_1_4_speed"], "50");
    assert_eq!(br_cfg["overhang_2_4_speed"], "35");
    assert_eq!(br_cfg["overhang_3_4_speed"], "22");
    assert_eq!(br_cfg["overhang_4_4_speed"], "15");
    assert_eq!(br_cfg["overhang_fan_speed"], "100");

    // Non-monotonic overhang speed curve is blocked by validate()
    br_ans.insert("overhang_2_4_speed".to_string(), "60".to_string());
    assert!(br_tpl.validate_answers(&br_ans, "all").is_err());

    // 5. Verify all 4 packs export cleanly across all 5 Process profiles (`0.12mm HQ` .. `0.28mm Speed`)!
    let rig = MachineRig::kobra_s1_hardened_steel_png();
    let processes = crate::models::ProcessProfile::get_default_processes();
    let mut rules = TransformationEngine::get_default_rules();
    for r in &mut rules {
      if matches!(
        r.id.as_str(),
        "support_tuning" | "ironing_tuning" | "dimensional_precision" | "bridge_overhang_tuning"
      ) {
        r.enabled = true;
      }
    }
    let mut base = OrcaProfile::get_stock_kobra_s1();
    base.set_str("layer_height", "0.20");
    let eval = TransformationEngine::evaluate_stack(
      &base,
      &rules,
      &json!({
        "printer_model": rig.printer_model,
        "nozzle_type": rig.nozzle_type,
        "nozzle_material": rig.nozzle_material,
        "nozzle_diameter": rig.nozzle_diameter,
        "hotend_max_flow": rig.max_volumetric_flow_limit,
        "filament_max_flow": 16.0,
        "filament_type": "PETG",
        "layer_height": 0.20,
        "line_width": 0.42,
      }),
    );

    for proc in &processes {
      let proc_json =
        crate::threemf::build_process_preset_json(proc, &rig, &eval.resulting_profile.values);
      let val: serde_json::Value = serde_json::from_str(&proc_json).unwrap();
      let expected_zd = format!("{:.2}", proc.layer_height + 0.06);
      assert_eq!(
        val["support_top_z_distance"].as_str(),
        Some(expected_zd.as_str()),
        "process {}",
        proc.name
      );
      assert_eq!(val["ironing_flow"].as_str(), Some("18%"));
      assert_eq!(val["precise_outer_wall"].as_str(), Some("1"));
      assert_eq!(val["xy_hole_compensation"].as_str(), Some("0.080"));
      assert_eq!(val["bridge_flow"].as_str(), Some("0.92"));
      assert_eq!(val["overhang_4_4_speed"].as_str(), Some("15"));
    }
  }

  #[test]
  fn test_declarative_scope_filters_polymer_nozzle_and_per_process_tier() {
    let mk_rule = |id: &str, prio: i32, lua: &str, scope: RuleScopeFilter| TransformationRule {
      id: id.to_string(),
      name: format!("Rule {}", id),
      description: String::new(),
      category: "Test".to_string(),
      icon: "🧪".to_string(),
      enabled: true,
      condition_expr: "all".to_string(),
      lua_code: lua.to_string(),
      priority: prio,
      scope,
    };

    // 1. Polymer filter: PETG-only rule applies on PETG, skipped on PLA (with provenance skipped step)
    let petg_rule = mk_rule(
      "petg_only",
      10,
      "config[\"bridge_flow\"] = \"0.90\"",
      RuleScopeFilter {
        polymers: vec!["PETG".to_string(), "PCTG".to_string()],
        ..Default::default()
      },
    );
    let mut base = OrcaProfile::get_stock_kobra_s1();
    base.set_str("bridge_flow", "1.00");
    let ctx_petg = json!({ "filament_type": "PETG", "nozzle_type": "hardened_steel", "nozzle_material": "Hardened Steel", "nozzle_diameter": "0.4" });
    let ctx_pla = json!({ "filament_type": "PLA Matte", "nozzle_type": "hardened_steel", "nozzle_material": "Hardened Steel", "nozzle_diameter": "0.4" });
    let r_petg = TransformationEngine::evaluate_stack(&base, &[petg_rule.clone()], &ctx_petg);
    assert_eq!(
      r_petg.resulting_profile.get_str("bridge_flow").as_deref(),
      Some("0.90")
    );
    assert!(r_petg.skipped_rules.is_empty());
    let r_pla = TransformationEngine::evaluate_stack(&base, &[petg_rule], &ctx_pla);
    assert_eq!(
      r_pla.resulting_profile.get_str("bridge_flow").as_deref(),
      Some("1.00")
    );
    assert_eq!(r_pla.skipped_rules.len(), 1);
    assert!(r_pla.skipped_rules[0].reason.contains("Polymer filter"));
    assert_eq!(
      r_pla.skipped_rules[0].would_touch_keys,
      vec!["bridge_flow".to_string()]
    );
    let bf_prov = r_pla
      .provenance_items
      .iter()
      .find(|p| p.param_key == "bridge_flow")
      .expect("bridge_flow provenance");
    assert_eq!(bf_prov.skipped_steps.len(), 1);

    // Fiber filter matches PA6-CF but not plain PETG
    let fiber = RuleScopeFilter {
      polymers: vec!["FIBER".to_string()],
      ..Default::default()
    };
    assert!(fiber
      .mismatch_reason(&json!({ "filament_type": "PA6-CF" }))
      .is_none());
    assert!(fiber
      .mismatch_reason(&json!({ "filament_type": "PETG" }))
      .is_some());

    // 2. Nozzle diameter + material filters
    let noz = RuleScopeFilter {
      nozzle_diameters: vec!["0.6".to_string()],
      nozzle_materials: vec!["diamondback".to_string()],
      ..Default::default()
    };
    assert!(noz
      .mismatch_reason(&json!({ "nozzle_diameter": "0.4", "nozzle_type": "diamondback" }))
      .unwrap()
      .contains("Nozzle diameter"));
    assert!(noz
      .mismatch_reason(&json!({ "nozzle_diameter": 0.6, "nozzle_type": "hardened_steel", "nozzle_material": "Hardened Steel" }))
      .unwrap()
      .contains("Nozzle material"));
    assert!(noz
      .mismatch_reason(&json!({ "nozzle_diameter": "0.60", "nozzle_type": "diamondback", "nozzle_material": "Diamondback PCD" }))
      .is_none());

    // 3. Process Tier filter: evaluated independently per exported process preset
    let processes = crate::models::ProcessProfile::get_default_processes();
    let tiers: Vec<&str> = processes.iter().map(classify_process_scope_tier).collect();
    assert!(tiers.contains(&"fine"));
    assert!(tiers.contains(&"draft"));
    assert!(tiers.contains(&"standard"));
    let fine_only = mk_rule(
      "fine_ironing",
      20,
      "config[\"ironing_type\"] = \"topmost\"",
      RuleScopeFilter {
        process_tiers: vec!["fine".to_string()],
        ..Default::default()
      },
    );
    let per_proc =
      TransformationEngine::evaluate_stack_per_process(&base, &[fine_only], &ctx_petg, &processes);
    let rig = MachineRig::kobra_s1_hardened_steel_png();
    for proc in &processes {
      let ov = per_proc.get(&proc.id).expect("per-process overrides");
      let is_fine = classify_process_scope_tier(proc) == "fine";
      assert_eq!(
        ov.get("ironing_type").and_then(|v| v.as_str()) == Some("topmost"),
        is_fine,
        "process {} (tier {})",
        proc.name,
        classify_process_scope_tier(proc)
      );
      let proc_json = crate::threemf::build_process_preset_json(proc, &rig, ov);
      let val: serde_json::Value = serde_json::from_str(&proc_json).unwrap();
      assert_eq!(
        val.get("ironing_type").and_then(|v| v.as_str()) == Some("topmost"),
        is_fine,
        "exported process {}",
        proc.name
      );
    }

    // 4. Scope round-trips through serde and legacy JSON without `scope` still deserializes
    let legacy = r#"{"id":"x","name":"x","description":"","category":"c","icon":"i","enabled":true,"condition_expr":"all","lua_code":"","priority":10}"#;
    let parsed: TransformationRule = serde_json::from_str(legacy).unwrap();
    assert!(parsed.scope.is_all());
  }

  #[test]
  fn test_line_width_geometry_default_on_nozzle_proportional_and_exported() {
    // 1. Enabled by default in the default stack, right after the hardware rules
    let defaults = TransformationEngine::get_default_rules();
    let lw_rule = defaults
      .iter()
      .find(|r| r.id == "line_width_geometry")
      .expect("line_width_geometry default rule exists");
    assert!(lw_rule.enabled, "line_width_geometry must be default-on");
    let bed_rule = defaults
      .iter()
      .find(|r| r.id == "symptom_bed_adhesion_petg")
      .unwrap();
    assert!(lw_rule.priority < bed_rule.priority);

    // 2. Nozzle-proportional widths on 0.4mm and 0.6mm nozzles
    let (cfg_04, _, _) = execute_transformation_lua(
      &lw_rule.lua_code,
      &json!({}),
      &json!({ "nozzle_diameter": "0.4" }),
    )
    .expect("0.4mm executes");
    assert_eq!(cfg_04["initial_layer_line_width"], "0.50");
    assert_eq!(cfg_04["outer_wall_line_width"], "0.42");
    assert_eq!(cfg_04["top_surface_line_width"], "0.38");
    assert_eq!(cfg_04["inner_wall_line_width"], "0.50");
    assert_eq!(cfg_04["sparse_infill_line_width"], "0.50");
    assert_eq!(cfg_04["internal_solid_infill_line_width"], "0.42");

    let (cfg_06, _, _) = execute_transformation_lua(
      &lw_rule.lua_code,
      &json!({}),
      &json!({ "nozzle_diameter": 0.6 }),
    )
    .expect("0.6mm executes");
    assert_eq!(cfg_06["initial_layer_line_width"], "0.75");
    assert_eq!(cfg_06["outer_wall_line_width"], "0.63");
    assert_eq!(cfg_06["top_surface_line_width"], "0.57");
    assert_eq!(cfg_06["inner_wall_line_width"], "0.75");
    assert_eq!(cfg_06["sparse_infill_line_width"], "0.75");

    // 3. Strength preset & validation
    let tpl = TransformationEngine::get_parameterized_calibration_templates()
      .into_iter()
      .find(|t| t.id == "line_width_geometry")
      .expect("line_width_geometry template");
    let mut ans = tpl.default_answers();
    tpl.apply_answer_change(&mut ans, "geometry_preset", "strength".to_string());
    assert_eq!(ans.get("inner_pct").map(|s| s.as_str()), Some("135"));
    let (_, _, _, strength_lua) = tpl.generate_from_answers(&ans, "all");
    let (cfg_str, _, _) = execute_transformation_lua(
      &strength_lua,
      &json!({}),
      &json!({ "nozzle_diameter": "0.4" }),
    )
    .unwrap();
    assert_eq!(cfg_str["inner_wall_line_width"], "0.54");
    ans.insert("inner_pct".to_string(), "100".to_string());
    ans.insert("outer_pct".to_string(), "110".to_string());
    assert!(tpl.validate_answers(&ans, "all").is_err());

    // 4. Full default stack: bed-adhesion rule still widens the 125% first layer by +10% (0.55mm),
    // and widths land in every exported process preset without double-scaling.
    let rig = MachineRig::kobra_s1_hardened_steel_png();
    let mut rules = defaults.clone();
    TransformationEngine::sync_rules_for_rig(
      &mut rules,
      &rig.nozzle_type,
      &rig.nozzle_material,
      rig.ace_pro_units,
    );
    let base = OrcaProfile::get_stock_kobra_s1();
    let ctx = json!({
      "printer_model": rig.printer_model,
      "nozzle_type": rig.nozzle_type,
      "nozzle_material": rig.nozzle_material,
      "nozzle_diameter": rig.nozzle_diameter,
      "hotend_max_flow": rig.max_volumetric_flow_limit,
      "filament_max_flow": 16.0,
      "filament_type": "PETG",
    });
    let processes = crate::models::ProcessProfile::get_default_processes();
    let per_proc =
      TransformationEngine::evaluate_stack_per_process(&base, &rules, &ctx, &processes);
    for proc in &processes {
      let ov = &per_proc[&proc.id];
      assert_eq!(
        ov.get("initial_layer_line_width").and_then(|v| v.as_str()),
        Some("0.55"),
        "process {}",
        proc.name
      );
      let proc_json = crate::threemf::build_process_preset_json(proc, &rig, ov);
      let val: serde_json::Value = serde_json::from_str(&proc_json).unwrap();
      assert_eq!(
        val["inner_wall_line_width"].as_str(),
        Some("0.50"),
        "process {}",
        proc.name
      );
      assert_eq!(
        val["sparse_infill_line_width"].as_str(),
        Some("0.50"),
        "process {}",
        proc.name
      );
      assert_eq!(
        val["top_surface_line_width"].as_str(),
        Some("0.38"),
        "process {}",
        proc.name
      );
    }
  }

  #[test]
  fn test_structural_strength_pack_process_keys_and_per_spool_clamped_filament_export() {
    let tpl = TransformationEngine::get_parameterized_calibration_templates()
      .into_iter()
      .find(|t| t.id == "structural_strength")
      .expect("structural_strength template");
    let defaults = TransformationEngine::get_default_rules();
    let rule = defaults
      .iter()
      .find(|r| r.id == "structural_strength")
      .expect("structural_strength default rule");
    assert!(!rule.enabled, "structural_strength is opt-in");

    // 1. Load-bearing preset snaps dials and applies process structure + live preview
    let mut ans = tpl.default_answers();
    tpl.apply_answer_change(&mut ans, "strength_preset", "load_bearing".to_string());
    assert_eq!(ans.get("wall_loops").map(|s| s.as_str()), Some("6"));
    let (_, _, _, lua) = tpl.generate_from_answers(&ans, "all");
    let (cfg, _, _) = execute_transformation_lua(
      &lua,
      &json!({
        "nozzle_temperature": ["245"],
        "nozzle_temperature_initial_layer": ["250"],
        "fan_max_speed": ["80"],
        "wall_loops": "2",
        "sparse_infill_pattern": "crosshatch"
      }),
      &json!({ "filament_type": "PETG", "filament_nozzle_max_c": 250 }),
    )
    .expect("structural_strength executes");
    assert_eq!(cfg["_structural_temp_boost_c"], "8");
    assert_eq!(cfg["_structural_fan_cut_pct"], "35");
    assert_eq!(cfg["nozzle_temperature"][0], "250"); // 245 + 8 clamped to 250
    assert_eq!(cfg["nozzle_temperature_initial_layer"][0], "250");
    assert_eq!(cfg["fan_max_speed"][0], "52"); // 80 × 0.65
    assert_eq!(cfg["wall_loops"], "6");
    assert_eq!(cfg["alternate_extra_wall"], "1");
    assert_eq!(cfg["top_shell_layers"], "6");
    assert_eq!(cfg["bottom_shell_layers"], "6");
    assert_eq!(cfg["infill_wall_overlap"], "25%");
    assert_eq!(cfg["sparse_infill_pattern"], "gyroid");

    // 2. Validation: PLA with > 35% fan cut is rejected
    let mut pla_ans = ans.clone();
    pla_ans.insert("fan_cut_pct".to_string(), "45".to_string());
    let pla_ctx = json!({ "scope": "all", "filament_type": "PLA" });
    assert!(validate_lua_rule_params(&tpl.lua_template, &pla_ans, &pla_ctx).is_err());

    // 3. Per-spool filament export: boost applied & clamped against EACH polymer's own safe max
    let rig = MachineRig::kobra_s1_hardened_steel_png();
    let catalog = crate::catalog::load_merged_catalog();
    let mut overrides = serde_json::Map::new();
    overrides.insert("_structural_temp_boost_c".to_string(), json!("8"));
    overrides.insert("_structural_fan_cut_pct".to_string(), json!("35"));
    for code in ["PLA", "PETG", "ASA"] {
      let entry = catalog
        .filaments
        .iter()
        .find(|f| f.code == code)
        .unwrap_or_else(|| panic!("{} in catalog", code));
      let spool = FilamentSpool::from_catalog_entry(1, entry, &rig.nozzle_type);
      let plain: serde_json::Value = serde_json::from_str(
        &crate::threemf::export_standalone_filament_json(&spool, &rig),
      )
      .unwrap();
      let boosted: serde_json::Value = serde_json::from_str(
        &crate::threemf::export_standalone_filament_json_with_overrides(&spool, &rig, &overrides),
      )
      .unwrap();
      let num = |v: &serde_json::Value, k: &str| -> i32 {
        v[k][0].as_str().unwrap().parse::<i32>().unwrap()
      };
      let (_, max_safe) = FilamentSpool::polymer_safe_nozzle_bounds(&spool.material_type);
      let expected = (num(&plain, "nozzle_temperature") + 8).min(max_safe);
      assert_eq!(num(&boosted, "nozzle_temperature"), expected, "{}", code);
      assert!(num(&boosted, "nozzle_temperature") <= max_safe, "{}", code);
      let expected_fan = ((num(&plain, "fan_max_speed") as f64) * 0.65).round() as i32;
      assert_eq!(num(&boosted, "fan_max_speed"), expected_fan, "{}", code);
      assert!(
        num(&boosted, "fan_min_speed") <= num(&boosted, "fan_max_speed"),
        "{}",
        code
      );
    }

    // 4. Process presets export the structural keys
    let processes = crate::models::ProcessProfile::get_default_processes();
    let mut proc_ov = OrcaProfile::get_stock_kobra_s1().values;
    for (k, v) in cfg.as_object().unwrap() {
      proc_ov.insert(k.clone(), v.clone());
    }
    proc_ov.insert("seam_slope_type".to_string(), json!("none"));
    let proc_json = crate::threemf::build_process_preset_json(&processes[0], &rig, &proc_ov);
    let val: serde_json::Value = serde_json::from_str(&proc_json).unwrap();
    assert_eq!(val["wall_loops"].as_str(), Some("6"));
    assert_eq!(val["alternate_extra_wall"].as_str(), Some("1"));
    assert_eq!(val["bottom_shell_layers"].as_str(), Some("6"));
    assert_eq!(val["infill_wall_overlap"].as_str(), Some("25%"));
    assert_eq!(val["sparse_infill_pattern"].as_str(), Some("gyroid"));
  }

  #[test]
  fn test_rule_shadowing_and_contradiction_detector() {
    let mut base = OrcaProfile::get_stock_kobra_s1();
    base.set_str("filament_z_hop_types", "Auto Lift");
    base.set_str("nozzle_temperature", "220");
    base.set_str("outer_wall_line_width", "0.40");
    base.set_str("textured_plate_temp", "60");
    base.set_str("fan_max_speed", "80");

    let mk_rule = |id: &str, name: &str, prio: i32, lua: &str| TransformationRule {
      id: id.to_string(),
      name: name.to_string(),
      description: String::new(),
      category: "Test".to_string(),
      icon: "⚙️".to_string(),
      enabled: true,
      condition_expr: "all".to_string(),
      lua_code: lua.to_string(),
      priority: prio,
      scope: RuleScopeFilter::default(),
    };

    let rules = vec![
      mk_rule(
        "rule_early",
        "Rule Early",
        10,
        r#"
          config["filament_z_hop_types"] = "Spiral Lift"
          local t = tonumber(config["nozzle_temperature"][1]) or 220
          config["nozzle_temperature"] = { tostring(t - 5) }
          config["outer_wall_line_width"] = "0.42"
          local b = tonumber(config["textured_plate_temp"][1]) or 60
          config["textured_plate_temp"] = { tostring(b + 5) }
          local f = tonumber(config["fan_max_speed"][1]) or 80
          config["fan_max_speed"] = { tostring(f - 10) }
        "#,
      ),
      mk_rule(
        "rule_late",
        "Rule Late",
        20,
        r#"
          config["filament_z_hop_types"] = "Normal Lift"
          local t = tonumber(config["nozzle_temperature"][1]) or 220
          config["nozzle_temperature"] = { tostring(t + 8) }
          config["outer_wall_line_width"] = "0.50"
          local b = tonumber(config["textured_plate_temp"][1]) or 60
          config["textured_plate_temp"] = { tostring(b + 5) }
          config["fan_max_speed"] = { "100" }
        "#,
      ),
    ];

    let eval = TransformationEngine::evaluate_stack(&base, &rules, &json!({}));
    let change = |key: &str| {
      eval
        .rule_changes
        .iter()
        .find(|c| c.rule_id == "rule_early" && c.param_key == key)
        .unwrap_or_else(|| panic!("early {} change", key))
        .clone()
    };

    // Absolute enum write clobbers the earlier enum write.
    let early_zhop = change("filament_z_hop_types");
    assert_eq!(early_zhop.shadow_kind, Some(RuleShadowKind::Overwritten));
    assert_eq!(early_zhop.shadowed_by_step, Some(2));
    assert_eq!(
      early_zhop.shadowed_by_rule_name.as_deref(),
      Some("Rule Late")
    );

    // Opposite-direction RELATIVE adjustments (-5°C then +8°C = +3°C) compose normally: NOT shadowed.
    assert_eq!(change("nozzle_temperature").shadow_kind, None);

    // Absolute numeric writes clobber the earlier value.
    assert_eq!(
      change("outer_wall_line_width").shadow_kind,
      Some(RuleShadowKind::Overwritten)
    );
    // Absolute write in the opposite direction is still clobbering (the -10% is forgotten).
    assert_eq!(
      change("fan_max_speed").shadow_kind,
      Some(RuleShadowKind::Overwritten)
    );

    // Additive same-direction offset (+5°C then +5°C = +10°C) is NOT shadowed!
    assert_eq!(change("textured_plate_temp").shadow_kind, None);

    let prov = |key: &str| {
      eval
        .provenance_items
        .iter()
        .find(|p| p.param_key == key)
        .unwrap()
        .clone()
    };
    assert!(!prov("nozzle_temperature").has_shadowed_steps);
    assert!(!prov("textured_plate_temp").has_shadowed_steps);
    assert!(prov("fan_max_speed").has_shadowed_steps);
  }

  /// `RuleShadowKind::Accepted`: a downstream guard that READS the key and passes the value
  /// through unchanged (`x = min(x, 20)` with `x` already below the cap) must be classified as
  /// Accepted — the value survived because it satisfied the constraint, not because it was
  /// recomputed. When the clamp actually engages (x above the cap), the downstream rule writes a
  /// change record and nothing is flagged as Accepted.
  #[test]
  fn test_shadow_kind_accepted_when_guard_passes_value_through() {
    let mut base = OrcaProfile::get_stock_kobra_s1();
    base.set_str("filament_retraction_length", "5.0");
    let mk = |id: &str, prio: i32, lua: &str| TransformationRule {
      id: id.to_string(),
      name: id.to_string(),
      description: String::new(),
      category: "Test".to_string(),
      icon: "⚙️".to_string(),
      enabled: true,
      condition_expr: "all".to_string(),
      lua_code: lua.to_string(),
      priority: prio,
      scope: RuleScopeFilter::default(),
    };
    let calibrator = mk(
      "calibrator",
      10,
      r#"
        config["filament_retraction_length"] = { "0.60" }
      "#,
    );
    let guard = mk(
      "guard",
      20,
      r#"
        local r = tonumber(config["filament_retraction_length"]) or 5.0
        config["filament_retraction_length"] = { string.format("%.2f", math.min(r, 20)) }
      "#,
    );

    // 0.60 < 20 cap: the guard accepts the calibrator's value untouched.
    let eval =
      TransformationEngine::evaluate_stack(&base, &[calibrator.clone(), guard.clone()], &json!({}));
    let calib_change = eval
      .rule_changes
      .iter()
      .find(|c| c.rule_id == "calibrator")
      .unwrap()
      .clone();
    assert_eq!(
      calib_change.shadow_kind,
      Some(RuleShadowKind::Accepted),
      "guard read 0.60mm and passed it through its min(., 20) cap unchanged"
    );
    assert_eq!(calib_change.shadowed_by_rule_id.as_deref(), Some("guard"));
    assert!(
      !eval
        .provenance_items
        .iter()
        .find(|p| p.param_key == "filament_retraction_length")
        .unwrap()
        .has_shadowed_steps
    );

    // 25.0 > 20 cap: the clamp ENGAGES, the guard writes its own change record, and the earlier
    // step's contribution genuinely mattered (restoring the pre-value changes the outcome) — so
    // nothing is Accepted, and the parameter is not shadowed either.
    let engager = mk(
      "engager",
      5,
      r#"
        config["filament_retraction_length"] = { "25.0" }
      "#,
    );
    let eval2 = TransformationEngine::evaluate_stack(&base, &[engager, guard], &json!({}));
    let engager_change = eval2
      .rule_changes
      .iter()
      .find(|c| c.rule_id == "engager")
      .unwrap()
      .clone();
    assert_eq!(
      engager_change.shadow_kind, None,
      "clamped-but-mattering contribution is a normal transformation, not Accepted"
    );
  }

  /// Real-rules classification check: `sym_wet_filament` recomputes `nozzle_temperature_HS` from
  /// `nozzle_temperature`. Directly after `thermal_conductivity` that is a benign `Recomputed`
  /// (the +Δ intent survives through the sibling key); with a relative-temp rule (like
  /// `symptom_stringing_diamondback`'s -4°C) running in between, thermal's value no longer reaches
  /// the wet rule and the HS write is truly `Overwritten`.
  #[test]
  fn test_real_thermal_wet_shadow_classification() {
    let mut base = OrcaProfile::get_stock_kobra_s1();
    base.set_str("nozzle_temperature", "230");
    base.set_str("nozzle_temperature_initial_layer", "235");
    base.set_str("nozzle_temperature_HS", "230");
    let mk = |id: &str, prio: i32, lua: &str| TransformationRule {
      id: id.to_string(),
      name: id.to_string(),
      description: String::new(),
      category: "Test".to_string(),
      icon: "⚙️".to_string(),
      enabled: true,
      condition_expr: "all".to_string(),
      lua_code: lua.to_string(),
      priority: prio,
      scope: RuleScopeFilter::default(),
    };
    let thermal = mk(
      "thermal_conductivity",
      10,
      include_str!("../transformations/thermal_conductivity.lua"),
    );
    let mut wet = mk(
      "sym_wet_filament",
      55,
      include_str!("../transformations/parametric/sym_wet_filament.lua"),
    );
    let mut params = std::collections::HashMap::new();
    params.insert("filament_humidity_pct".to_string(), "65".to_string());
    wet.lua_code = crate::lua_engine::embed_param_defaults_in_lua(&wet.lua_code, &params);
    let stringing = mk(
      "stringing_like",
      25,
      r#"
        local t = tonumber(config["nozzle_temperature"]) or 225
        config["nozzle_temperature"] = tostring(t - 4)
      "#,
    );
    let ctx = json!({ "nozzle_type": "hardened_steel", "nozzle_material": "Hardened Steel", "filament_type": "PETG", "filament_humidity_pct": 65 });

    let hs_kind = |rules: &[TransformationRule]| {
      TransformationEngine::evaluate_stack(&base, rules, &ctx)
        .rule_changes
        .iter()
        .find(|c| c.rule_id == "thermal_conductivity" && c.param_key == "nozzle_temperature_HS")
        .unwrap()
        .shadow_kind
    };
    assert_eq!(
      hs_kind(&[thermal.clone(), wet.clone()]),
      Some(RuleShadowKind::Recomputed),
      "direct chain: wet recomputes HS from thermal's nozzle_temperature -> intent preserved"
    );
    assert_eq!(
      hs_kind(&[thermal, stringing, wet]),
      Some(RuleShadowKind::Overwritten),
      "with an intervening relative-temp rule, thermal's value never reaches wet -> intent lost"
    );
  }

  /// `RuleShadowKind::Recomputed`: when a later rule discards an earlier step's direct write to a
  /// key but re-derives the value from OTHER keys the same step wrote (exactly how
  /// `sym_wet_filament` recomputes `nozzle_temperature_HS` from `nozzle_temperature` that
  /// `thermal_conductivity` set), the earlier step must be classified as `Recomputed` (benign,
  /// intent preserved) — NOT `Overwritten` — and must NOT mark the parameter as shadowed.
  #[test]
  fn test_shadow_kind_recomputed_when_intent_flows_through_sibling_key() {
    let mut base = OrcaProfile::get_stock_kobra_s1();
    base.set_str("nozzle_temperature", "220");
    base.set_str("nozzle_temperature_HS", "220");

    let mk_rule = |id: &str, name: &str, prio: i32, lua: &str| TransformationRule {
      id: id.to_string(),
      name: name.to_string(),
      description: String::new(),
      category: "Test".to_string(),
      icon: "⚙️".to_string(),
      enabled: true,
      condition_expr: "all".to_string(),
      lua_code: lua.to_string(),
      priority: prio,
      scope: RuleScopeFilter::default(),
    };

    // Mirrors thermal_conductivity: writes the effective temp AND the HS variant from it.
    let thermal = mk_rule(
      "thermal_like",
      "Thermal Like",
      10,
      r#"
        local t = tonumber(config["nozzle_temperature"][1]) or 220
        config["nozzle_temperature"] = { tostring(t + 5) }
        config["nozzle_temperature_HS"] = { tostring(t + 5) }
      "#,
    );
    // Mirrors sym_wet_filament: overwrites the HS key with a value derived from the effective
    // nozzle_temperature (a sibling key the early rule wrote), never reading the HS key itself.
    let wet = mk_rule(
      "wet_like",
      "Wet Like",
      20,
      r#"
        local t = tonumber(config["nozzle_temperature"][1]) or 225
        config["nozzle_temperature_HS"] = { tostring(t - 10) }
      "#,
    );

    let eval = TransformationEngine::evaluate_stack(&base, &[thermal, wet], &json!({}));

    let hs_change = eval
      .rule_changes
      .iter()
      .find(|c| c.rule_id == "thermal_like" && c.param_key == "nozzle_temperature_HS")
      .expect("thermal HS change")
      .clone();
    assert_eq!(
      hs_change.shadow_kind,
      Some(RuleShadowKind::Recomputed),
      "HS write must be classified Recomputed (wet derives it from thermal's nozzle_temperature)"
    );
    assert_eq!(hs_change.shadowed_by_rule_id.as_deref(), Some("wet_like"));
    // Final HS = (220 + 5) - 10 = 215: the +5°C intent survived through the sibling key.
    assert_eq!(hs_change.shadowed_final_value.as_deref(), Some("215"));

    // The relative effective-temp step itself is untouched (no flag at all).
    let temp_change = eval
      .rule_changes
      .iter()
      .find(|c| c.rule_id == "thermal_like" && c.param_key == "nozzle_temperature")
      .expect("thermal temp change")
      .clone();
    assert_eq!(temp_change.shadow_kind, None);

    // A recomputed-only parameter is NOT marked as shadowed (no amber alarm / reorder nudge).
    let hs_prov = eval
      .provenance_items
      .iter()
      .find(|p| p.param_key == "nozzle_temperature_HS")
      .unwrap();
    assert!(!hs_prov.has_shadowed_steps);
  }

  #[test]
  fn test_outer_wall_consistency_vfa_guard_and_spire_safe_mode() {
    let cals = TransformationEngine::get_parameterized_calibration_templates();
    let tpl = cals
      .iter()
      .find(|c| c.id == "outer_wall_consistency")
      .expect("outer_wall_consistency in calibration templates");
    let rules = TransformationEngine::get_default_rules();
    let default_rule = rules
      .iter()
      .find(|r| r.id == "outer_wall_consistency")
      .expect("outer_wall_consistency in default rules");
    assert!(
      default_rule.enabled,
      "outer_wall_consistency should be enabled by default"
    );

    // 1. Low-MVS filament (PETG / Silk PLA @ 10 mm3/s): 200 mm/s @ 0.20x0.42mm (16.8 mm3/s)
    // would be clamped by MVS to 119 mm/s (inside 70-130 mm/s VFA band).
    // Auto VFA Guard steps outer_wall_speed down to 55 mm/s and locks dont_slow_down_outer_wall = "1".
    let ans_auto = tpl.default_answers();
    let (_, _, _, lua_auto) = tpl.generate_from_answers(&ans_auto, "all");
    let (cfg_petg, _, _) = execute_transformation_lua(
      &lua_auto,
      &json!({
        "layer_height": "0.20",
        "outer_wall_line_width": "0.42",
        "outer_wall_speed": "200",
        "filament_max_volumetric_speed": ["10"],
      }),
      &json!({ "hotend_max_flow": 22.0, "filament_max_flow": 10.0 }),
    )
    .expect("outer_wall_consistency executes for PETG 10 mm3/s");
    assert_eq!(cfg_petg["dont_slow_down_outer_wall"], "1");
    assert_eq!(cfg_petg["outer_wall_speed"], "55");
    assert_eq!(cfg_petg["small_perimeter_speed"], "50");

    // 2. High-MVS filament (High-Speed PLA @ 18 mm3/s) on 0.20mm Standard: ceiling is 214 mm/s >= 145 mm/s.
    // Keeps outer_wall_speed at 200 mm/s (clears VFA band) and locks dont_slow_down_outer_wall = "1".
    let (cfg_hs_pla, _, _) = execute_transformation_lua(
      &lua_auto,
      &json!({
        "layer_height": "0.20",
        "outer_wall_line_width": "0.42",
        "outer_wall_speed": "200",
        "filament_max_volumetric_speed": ["18"],
      }),
      &json!({ "hotend_max_flow": 22.0, "filament_max_flow": 18.0 }),
    )
    .expect("outer_wall_consistency executes for HS PLA 18 mm3/s");
    assert_eq!(cfg_hs_pla["dont_slow_down_outer_wall"], "1");
    assert_eq!(cfg_hs_pla["outer_wall_speed"], "200");

    // 2b. High-MVS PLA on 0.12mm Fine (miniatures / delicate vertical spires):
    // Auto Guard automatically sets dont_slow_down_outer_wall = "0" so tiny 3mm spires can slow down for cooling!
    let (cfg_fine_pla, _, _) = execute_transformation_lua(
      &lua_auto,
      &json!({
        "layer_height": "0.12",
        "outer_wall_line_width": "0.42",
        "outer_wall_speed": "200",
        "filament_max_volumetric_speed": ["18"],
      }),
      &json!({ "hotend_max_flow": 22.0, "filament_max_flow": 18.0 }),
    )
    .expect("outer_wall_consistency executes for 0.12mm Fine HS PLA");
    assert_eq!(cfg_fine_pla["dont_slow_down_outer_wall"], "0");
    assert_eq!(cfg_fine_pla["outer_wall_speed"], "200");

    // 2c. Silk PLA finish even with nominal 14 mm3/s steps down to 55 mm/s for maximum Silk shine & zero VFA:
    let (cfg_silk, _, _) = execute_transformation_lua(
      &lua_auto,
      &json!({
        "layer_height": "0.20",
        "outer_wall_line_width": "0.42",
        "outer_wall_speed": "200",
      }),
      &json!({ "hotend_max_flow": 22.0, "filament_type": "PLA", "filament_finish": "Silk" }),
    )
    .expect("outer_wall_consistency executes for Silk PLA");
    assert_eq!(cfg_silk["dont_slow_down_outer_wall"], "1");
    assert_eq!(cfg_silk["outer_wall_speed"], "55");

    // 3. Explicit Spire-Safe mode for miniatures & delicate 3mm tips on 0.20mm: sets dont_slow_down_outer_wall = "0"
    let mut ans_spire = tpl.default_answers();
    tpl.apply_answer_change(&mut ans_spire, "consistency_mode", "spire_safe".to_string());
    assert_eq!(
      ans_spire
        .get("dont_slow_down_outer_wall")
        .map(String::as_str),
      Some("0")
    );
    let (_, _, _, lua_spire) = tpl.generate_from_answers(&ans_spire, "all");
    let (cfg_spire, _, _) = execute_transformation_lua(
      &lua_spire,
      &json!({
        "layer_height": "0.20",
        "outer_wall_line_width": "0.42",
        "outer_wall_speed": "200",
        "filament_max_volumetric_speed": ["18"],
      }),
      &json!({ "hotend_max_flow": 22.0, "filament_max_flow": 18.0 }),
    )
    .expect("spire_safe executes");
    assert_eq!(cfg_spire["dont_slow_down_outer_wall"], "0");
    assert_eq!(cfg_spire["outer_wall_speed"], "200");

    // 4. Verify exported process JSON reflects VFA guard per process layer height:
    // For a 10 mm3/s filament:
    // - 0.12mm Quality (0.12 x 0.42 = 0.0504 mm2 -> ceiling 198 mm/s, outer_wall=60 mm/s <= 60): stays 60 mm/s!
    // - 0.20mm Standard (0.20 x 0.42 = 0.084 mm2 -> ceiling 119 mm/s in VFA band): steps down from 200 -> 55 mm/s!
    let rig = MachineRig::kobra_s1_hardened_steel_png();
    let procs = crate::models::ProcessProfile::get_default_processes();
    let mut proc_ov = OrcaProfile::get_stock_kobra_s1().values;
    for (k, v) in cfg_petg.as_object().unwrap() {
      proc_ov.insert(k.clone(), v.clone());
    }
    proc_ov.insert("seam_slope_type".to_string(), json!("none"));
    let p_020_std: serde_json::Value = serde_json::from_str(
      &crate::threemf::build_process_preset_json(&procs[0], &rig, &proc_ov),
    )
    .unwrap();
    assert_eq!(p_020_std["dont_slow_down_outer_wall"].as_str(), Some("1"));
    assert_eq!(p_020_std["outer_wall_speed"].as_str(), Some("55"));
    assert_eq!(p_020_std["small_perimeter_speed"].as_str(), Some("50"));

    let p_012_hq: serde_json::Value = serde_json::from_str(
      &crate::threemf::build_process_preset_json(&procs[3], &rig, &proc_ov),
    )
    .unwrap();
    assert_eq!(p_012_hq["dont_slow_down_outer_wall"].as_str(), Some("1"));
    assert_eq!(p_012_hq["outer_wall_speed"].as_str(), Some("60"));
  }

  #[test]
  fn test_anycubic_pctg_declared_as_petg_keeps_pctg_temperature_brackets() {
    use crate::threemf::{
      export_standalone_filament_json, export_standalone_filament_json_with_overrides,
      PCTG_AS_PETG_NOTE_MARKER, PCTG_AS_PETG_SIDE_CHANNEL,
    };

    let rule = TransformationEngine::get_default_rules()
      .into_iter()
      .find(|r| r.id == "anycubic_pctg_as_petg")
      .expect("anycubic_pctg_as_petg is a built-in default rule");
    assert!(rule.enabled, "rule must be on by default");

    // 1. Rig gating inside Lua: Anycubic arms the side-channel, Bambu does not (unless forced).
    let run = |lua: &str, model: &str| {
      let (cfg, _, _) = execute_transformation_lua(
        lua,
        &json!({}),
        &json!({ "printer_model": model, "filament_type": "PCTG", "all_filament_types": ["PCTG", "PLA"] }),
      )
      .expect("anycubic_pctg_as_petg executes");
      cfg
        .get(PCTG_AS_PETG_SIDE_CHANNEL)
        .and_then(|v| v.as_str())
        .map(String::from)
    };
    assert_eq!(
      run(&rule.lua_code, "Anycubic Kobra S1").as_deref(),
      Some("1")
    );
    assert_eq!(
      run(&rule.lua_code, "Anycubic Kobra S1 Max").as_deref(),
      Some("1")
    );
    assert_eq!(run(&rule.lua_code, "Bambu Lab P1S"), None);
    let mut force_all = HashMap::new();
    force_all.insert("target_printers".to_string(), "all".to_string());
    let lua_all = embed_param_defaults_in_lua(&rule.lua_code, &force_all);
    assert_eq!(run(&lua_all, "Bambu Lab P1S").as_deref(), Some("1"));

    // 2. Full default stack on the Anycubic rig produces the side-channel.
    let rig = MachineRig::kobra_s1_hardened_steel_png();
    let mut rules = TransformationEngine::get_default_rules();
    TransformationEngine::sync_rules_for_rig(
      &mut rules,
      &rig.nozzle_type,
      &rig.nozzle_material,
      rig.ace_pro_units,
    );
    let ctx = json!({
      "printer_model": rig.printer_model,
      "nozzle_type": rig.nozzle_type,
      "nozzle_material": rig.nozzle_material,
      "nozzle_diameter": rig.nozzle_diameter,
      "hotend_max_flow": rig.max_volumetric_flow_limit,
      "filament_type": "PCTG",
      "all_filament_types": ["PCTG", "PETG"],
    });
    let res =
      TransformationEngine::evaluate_stack(&OrcaProfile::get_stock_kobra_s1(), &rules, &ctx);
    let overrides = res.resulting_profile.values.clone();
    assert_eq!(
      overrides
        .get(PCTG_AS_PETG_SIDE_CHANNEL)
        .and_then(|v| v.as_str()),
      Some("1")
    );
    // The PCTG-scoped stringing rule must still see PCTG (filament type is only re-declared on export).
    assert!(!res
      .skipped_rules
      .iter()
      .any(|s| s.rule_id == "filament_pctg_antistringing"));

    // 3. Export: only `filament_type` (+ notes marker) differs; temps & brackets stay PCTG.
    let catalog = crate::catalog::load_merged_catalog();
    let pctg_entry = catalog
      .filaments
      .iter()
      .find(|f| f.code == "PCTG")
      .expect("PCTG catalog entry");
    let pctg = FilamentSpool::from_catalog_entry(1, pctg_entry, &rig.nozzle_type);
    assert_eq!(pctg.material_type, "PCTG");

    let mut no_flag = overrides.clone();
    no_flag.remove(PCTG_AS_PETG_SIDE_CHANNEL);
    let parse = |s: String| -> serde_json::Map<String, serde_json::Value> {
      serde_json::from_str::<serde_json::Value>(&s)
        .unwrap()
        .as_object()
        .unwrap()
        .clone()
    };
    let with_rule = parse(export_standalone_filament_json_with_overrides(
      &pctg, &rig, &overrides,
    ));
    let without_rule = parse(export_standalone_filament_json_with_overrides(
      &pctg, &rig, &no_flag,
    ));
    assert_eq!(with_rule["filament_type"], json!(["PETG"]));
    assert_eq!(without_rule["filament_type"], json!(["PCTG"]));
    assert!(with_rule["filament_notes"][0]
      .as_str()
      .unwrap()
      .contains(PCTG_AS_PETG_NOTE_MARKER));
    for (k, v) in &without_rule {
      if k == "filament_type" || k == "filament_notes" {
        continue;
      }
      assert_eq!(
        with_rule.get(k),
        Some(v),
        "'{}' must be unchanged by PETG declaration",
        k
      );
    }
    assert_eq!(with_rule.len(), without_rule.len());

    let first_i32 = |m: &serde_json::Map<String, serde_json::Value>, k: &str| -> i32 {
      m[k][0].as_str().unwrap().parse().unwrap()
    };
    let (pctg_min, pctg_max) = FilamentSpool::polymer_safe_nozzle_bounds("PCTG");
    let noz = first_i32(&with_rule, "nozzle_temperature");
    let init = first_i32(&with_rule, "nozzle_temperature_initial_layer");
    let lo = first_i32(&with_rule, "nozzle_temperature_range_low");
    let hi = first_i32(&with_rule, "nozzle_temperature_range_high");
    assert!(
      (pctg_min..=pctg_max).contains(&noz),
      "PCTG nozzle {}°C",
      noz
    );
    assert!(
      lo <= noz && init <= hi,
      "range [{}..{}] must cover {}/{}°C",
      lo,
      hi,
      noz,
      init
    );

    // 4. A hot PCTG spool above PETG's 260°C ceiling is NOT clamped to PETG limits.
    let (_, petg_max) = FilamentSpool::polymer_safe_nozzle_bounds("PETG");
    let mut hot = pctg.clone();
    hot.print_temp = 268;
    let hot_map = parse(export_standalone_filament_json_with_overrides(
      &hot, &rig, &overrides,
    ));
    assert_eq!(hot_map["filament_type"], json!(["PETG"]));
    let hot_noz = first_i32(&hot_map, "nozzle_temperature");
    assert!(
      hot_noz > petg_max,
      "PCTG 268°C must not be clamped to PETG {}°C",
      petg_max
    );
    assert!(first_i32(&hot_map, "nozzle_temperature_range_high") >= hot_noz);
    assert!(first_i32(&hot_map, "nozzle_temperature_range_low") <= hot_noz);

    // 5. PETG / PLA spools and exports without overrides are untouched.
    let petg_entry = catalog.filaments.iter().find(|f| f.code == "PETG").unwrap();
    let petg = FilamentSpool::from_catalog_entry(2, petg_entry, &rig.nozzle_type);
    let petg_map = parse(export_standalone_filament_json_with_overrides(
      &petg, &rig, &overrides,
    ));
    assert_eq!(petg_map["filament_type"], json!(["PETG"]));
    assert!(!petg_map["filament_notes"][0]
      .as_str()
      .unwrap()
      .contains(PCTG_AS_PETG_NOTE_MARKER));
    let plain = parse(export_standalone_filament_json(&pctg, &rig));
    assert_eq!(plain["filament_type"], json!(["PCTG"]));

    // 6. Round-trip: a PETG-declared PCTG preset whose name lacks "PCTG" re-imports as PCTG.
    let mut renamed = with_rule.clone();
    renamed.insert("name".to_string(), json!("My Clear Spool"));
    renamed.insert(
      "filament_settings_id".to_string(),
      json!(["My Clear Spool"]),
    );
    renamed.insert("nozzle_temperature".to_string(), json!(["268"]));
    let mut entries = crate::dir_import::build_reference_anycubic_entries();
    entries.push(crate::fs_access::ScannedFileEntry {
      relative_path: "user/434139/filament/My Clear Spool.json".to_string(),
      filename: "My Clear Spool.json".to_string(),
      bytes: serde_json::to_vec(&serde_json::Value::Object(renamed)).unwrap(),
    });
    let imported = crate::dir_import::import_scanned_directory(&entries).unwrap();
    let back = imported
      .filaments
      .iter()
      .find(|f| f.name.contains("My Clear Spool"))
      .expect("renamed PETG-declared spool imported");
    assert_eq!(back.material_type, "PCTG");
  }

  #[test]
  fn test_two_track_constraint_propagation_monotonic_lattice_and_epilogue_projection() {
    let mut base = OrcaProfile::get_stock_kobra_s1();
    base
      .values
      .insert("nozzle_temperature".to_string(), json!(["300"]));
    base.values.insert(
      "nozzle_temperature_initial_layer".to_string(),
      json!(["300"]),
    );
    base
      .values
      .insert("filament_retraction_length".to_string(), json!(["0.80"]));
    base
      .values
      .insert("filament_retraction_speed".to_string(), json!(["40"]));
    base
      .values
      .insert("filament_flow_ratio".to_string(), json!(["0.960"]));
    base
      .values
      .insert("filament_z_hop".to_string(), json!(["0.15"]));
    base.set_str("inner_wall_line_width", "0.50");

    let mk_rule = |id: &str, name: &str, prio: i32, lua: &str| TransformationRule {
      id: id.to_string(),
      name: name.to_string(),
      description: String::new(),
      category: "Test".to_string(),
      icon: "⚙️".to_string(),
      enabled: true,
      condition_expr: "all".to_string(),
      lua_code: lua.to_string(),
      priority: prio,
      scope: RuleScopeFilter::default(),
    };

    // 1. Real P10 (thermal_conductivity) drops 300 -> 295°C and declares `constrain(..., 175, 300, ...)`
    //    and real P12 (rig_addons_cooling) compresses retraction to 0.70mm and declares `constrain("filament_retraction_length", 0.35, 1.15, ...)`.
    //    A late P35 rule adds +10°C (295 -> 305°C) and +0.75mm retraction (0.70 -> 1.45mm).
    //    The Priority = ∞ Epilogue Projection clamps temperature back to 300°C and retraction to 1.15mm
    //    with explicit Provenance & Lineage attribution!
    let p10 = mk_rule(
      "thermal_conductivity",
      "P10 Thermal Conductivity",
      10,
      include_str!("../transformations/thermal_conductivity.lua"),
    );
    let p12 = mk_rule(
      "rig_addons_cooling",
      "P12 FlowTech Hotend",
      12,
      include_str!("../transformations/rig_addons_cooling.lua"),
    );
    let p15_tpl = TransformationEngine::get_parameterized_calibration_templates()
      .into_iter()
      .find(|t| t.id == "line_width_geometry")
      .expect("line_width_geometry");
    let (_, _, _, p15_lua) = p15_tpl.generate_from_answers(&p15_tpl.default_answers(), "all");
    let p15 = mk_rule("line_width_geometry", "P15 Line Width Geometry", 15, &p15_lua);
    let p35_boost = mk_rule(
      "high_flow_boost",
      "P35 High-Speed Volumetric & Structural Boost",
      35,
      r#"
        function apply(config, ctx)
          config["nozzle_temperature"] = map_num(config["nozzle_temperature"], function(t) return t + 10 end, "%.0f")
          config["filament_retraction_length"] = map_num(config["filament_retraction_length"], function(r) return r + 0.75 end, "%.2f")
          local iw = scalar_num(config["inner_wall_line_width"], 0.50)
          config["inner_wall_line_width"] = string.format("%.2f", iw * 1.35)
        end
      "#,
    );

    let ctx = json!({
      "nozzle_material": "Diamondback PCD",
      "nozzle_type": "diamondback",
      "nozzle_diameter": "0.4",
      "addons": ["flowtech_hotend"],
      "filament_is_elastomer": false,
    });

    let eval = TransformationEngine::evaluate_stack(&base, &[p10, p12, p15, p35_boost], &ctx);

    // Temperature: 300 -> 295 (P10) -> 305 (P35) -> Clamped to 300 (Priority ∞)
    assert_eq!(
      eval.resulting_profile.get_str("nozzle_temperature").as_deref(),
      Some("300")
    );
    let temp_prov = eval
      .provenance_items
      .iter()
      .find(|p| p.param_key == "nozzle_temperature")
      .expect("nozzle_temperature provenance");
    assert_eq!(temp_prov.steps.len(), 3);
    assert_eq!(temp_prov.steps[0].after_value, "295 °C");
    assert_eq!(temp_prov.steps[1].after_value, "305 °C");
    assert_eq!(temp_prov.steps[2].rule_priority, i32::MAX);
    assert_eq!(temp_prov.steps[2].after_value, "300 °C");
    assert!(
      temp_prov.steps[2]
        .rule_name
        .contains("Clamped by DiamondBack + FlowTech 300C thermal ceiling"),
      "unexpected clamp attribution: {}",
      temp_prov.steps[2].rule_name
    );
    assert_eq!(
      temp_prov.active_bound.as_ref().and_then(|b| b.max),
      Some(300.0)
    );

    // Retraction length: 0.80 -> 0.70 (P12) -> 1.45 (P35) -> Clamped to 1.15 (Priority ∞)
    assert_eq!(
      eval
        .resulting_profile
        .get_str("filament_retraction_length")
        .as_deref(),
      Some("1.15")
    );
    let ret_prov = eval
      .provenance_items
      .iter()
      .find(|p| p.param_key == "filament_retraction_length")
      .expect("filament_retraction_length provenance");
    assert!(
      ret_prov
        .steps
        .last()
        .unwrap()
        .rule_name
        .contains("Clamped by FlowTech all-metal heat-creep ceiling"),
      "unexpected retraction clamp: {}",
      ret_prov.steps.last().unwrap().rule_name
    );

    // Inner wall line width: 0.50 (P15: 1.25d) -> 0.68 (P35: *1.35 = 1.69d) -> Clamped to 0.64 (1.60d)
    assert_eq!(
      eval
        .resulting_profile
        .get_str("inner_wall_line_width")
        .as_deref(),
      Some("0.64")
    );

    // 2. Order-Independence of Interval Intersection ([175, 300] ∩ [180, 225] = [180, 225])
    let r_hw = mk_rule(
      "r_hw",
      "Hardware Bound",
      5,
      r#"constrain("nozzle_temperature", 175, 300, "FlowTech ceiling")"#,
    );
    let r_pva = mk_rule(
      "r_pva",
      "PVA Polymer Bound",
      50,
      r#"constrain("nozzle_temperature", 180, 225, "PVA thermal degradation ceiling")"#,
    );
    let eval_fwd =
      TransformationEngine::evaluate_stack(&base, &[r_hw.clone(), r_pva.clone()], &json!({}));
    let mut r_hw_late = r_hw;
    r_hw_late.priority = 99;
    let eval_rev = TransformationEngine::evaluate_stack(&base, &[r_pva, r_hw_late], &json!({}));
    let b_fwd = eval_fwd.parameter_bounds.get("nozzle_temperature").unwrap();
    let b_rev = eval_rev.parameter_bounds.get("nozzle_temperature").unwrap();
    assert_eq!((b_fwd.min, b_fwd.max), (Some(180.0), Some(225.0)));
    assert_eq!((b_rev.min, b_rev.max), (Some(180.0), Some(225.0)));
    assert_eq!(b_fwd.min_by, "PVA thermal degradation ceiling");
    assert_eq!(b_fwd.max_by, "PVA thermal degradation ceiling");
    assert_eq!(
      eval_fwd
        .resulting_profile
        .get_str("nozzle_temperature")
        .as_deref(),
      Some("225")
    );
    assert_eq!(
      eval_rev
        .resulting_profile
        .get_str("nozzle_temperature")
        .as_deref(),
      Some("225")
    );
  }

  #[test]
  fn test_infeasible_constraint_conflict_detection_and_coupled_relational_invariants() {
    let mut base = OrcaProfile::get_stock_kobra_s1();
    base
      .values
      .insert("nozzle_temperature".to_string(), json!(["250"]));
    base.values.insert(
      "nozzle_temperature_initial_layer".to_string(),
      json!(["220"]),
    );
    base
      .values
      .insert("filament_retraction_speed".to_string(), json!(["35"]));
    base
      .values
      .insert("filament_deretraction_speed".to_string(), json!(["45"]));
    base
      .values
      .insert("filament_z_hop".to_string(), json!(["0.08"]));
    base
      .values
      .insert("filament_z_hop_types".to_string(), json!(["Spiral Lift"]));
    base.set_str("layer_height", "0.28");

    let mk_rule = |id: &str, prio: i32, lua: &str| TransformationRule {
      id: id.to_string(),
      name: id.to_string(),
      description: String::new(),
      category: "Test".to_string(),
      icon: "⚙️".to_string(),
      enabled: true,
      condition_expr: "all".to_string(),
      lua_code: lua.to_string(),
      priority: prio,
      scope: RuleScopeFilter::default(),
    };

    // 1. Coupled Relational Invariants:
    //    - First-layer temp 220°C vs continuous 250°C (< 250 - 5 = 245°C) -> clamped to 245°C
    //    - Deretraction speed 45 mm/s > retraction speed 35 mm/s -> clamped to 35 mm/s
    //    - Spiral Lift Z-hop 0.08 mm < 0.5 * 0.28 = 0.14 mm -> clamped to 0.14 mm
    let touch_rule = mk_rule(
      "touch_rule",
      10,
      r#"
        config["nozzle_temperature"] = { "250" }
        config["filament_z_hop"] = { "0.08" }
      "#,
    );
    let eval_coupled = TransformationEngine::evaluate_stack(&base, &[touch_rule], &json!({}));
    assert_eq!(
      eval_coupled
        .resulting_profile
        .get_str("nozzle_temperature_initial_layer")
        .as_deref(),
      Some("245")
    );
    assert_eq!(
      eval_coupled
        .resulting_profile
        .get_str("filament_deretraction_speed")
        .as_deref(),
      Some("35")
    );
    assert_eq!(
      eval_coupled
        .resulting_profile
        .get_str("filament_z_hop")
        .as_deref(),
      Some("0.14")
    );

    // 2. Conflict / Infeasibility Detection (L > U):
    //    Hotend ceiling <= 300°C vs High-Temp Polymer requirement >= 310°C
    let r_hotend = mk_rule(
      "hotend_limit",
      10,
      r#"constrain("nozzle_temperature", 175, 300, "FlowTech 300C ceiling")"#,
    );
    let r_peek = mk_rule(
      "peek_polymer",
      20,
      r#"constrain("nozzle_temperature", 310, 340, "PEEK minimum melt temperature")"#,
    );
    let eval_conflict =
      TransformationEngine::evaluate_stack(&base, &[r_hotend, r_peek], &json!({}));
    assert_eq!(eval_conflict.infeasible_constraints.len(), 1);
    let inf = &eval_conflict.infeasible_constraints[0];
    assert_eq!(inf.param_key, "nozzle_temperature");
    assert!(inf.is_infeasible());
    assert_eq!((inf.min, inf.max), (Some(310.0), Some(300.0)));
    assert!(eval_conflict
      .execution_logs
      .iter()
      .any(|l| l.contains("Infeasible Constraint Conflict")));

    let rig = MachineRig::kobra_s1_hardened_steel_png();
    let warnings = TransformationEngine::evaluate_safety_warnings(
      &rig,
      &[],
      &eval_conflict.resulting_profile,
      &[],
      &[],
    );
    assert!(
      warnings
        .iter()
        .any(|w| w.id == "infeasible_bound:nozzle_temperature"
          && w.severity == SafetySeverity::Danger),
      "expected Danger safety warning for infeasible bound L > U, got: {:?}",
      warnings
    );
  }

  #[test]
  fn test_coupled_parameter_physics_groups_and_additional_coupled_transforms() {
    let mk_rule = |id: &str, prio: i32, lua: &str| TransformationRule {
      id: id.to_string(),
      name: id.to_string(),
      description: String::new(),
      category: "Test".to_string(),
      icon: "⚙️".to_string(),
      enabled: true,
      condition_expr: "all".to_string(),
      lua_code: lua.to_string(),
      priority: prio,
      scope: RuleScopeFilter::default(),
    };

    // 1. Group 1 (Pressure Advance), Group 2 (MVS), and Group 3 (Cooling Fan Curves)
    //    across nozzle materials, orifice diameters (0.25, 0.40, 0.60, 0.80 mm), and
    //    telescoping temperature excursions (P10 -> P25 -> P55 Wet -> P57 Structural).
    let mut base = OrcaProfile::new("PETG Coupled Test", "filament");
    base
      .values
      .insert("nozzle_temperature".to_string(), json!(["230"]));
    base.values.insert(
      "nozzle_temperature_initial_layer".to_string(),
      json!(["235"]),
    );
    base
      .values
      .insert("pressure_advance".to_string(), json!(["0.040"]));
    base.values.insert(
      "filament_max_volumetric_speed".to_string(),
      json!(["15.0"]),
    );
    base
      .values
      .insert("fan_min_speed".to_string(), json!(["40"]));
    base
      .values
      .insert("fan_max_speed".to_string(), json!(["70"]));
    base
      .values
      .insert("overhang_fan_speed".to_string(), json!(["85"]));

    let r_p10 = mk_rule(
      "thermal_conductivity",
      10,
      include_str!("../transformations/thermal_conductivity.lua"),
    );
    let r_p25 = mk_rule(
      "symptom_stringing_diamondback",
      25,
      include_str!("../transformations/symptom_stringing_diamondback.lua"),
    );

    // 1a. DiamondBack 0.40mm at P10 equilibrium:
    //     PA = 0.040 * 0.85 = 0.034; MVS = 15.0; Fan_min = 40 + round(0.75*6) = 45; Fan_max = 70 + 3 = 73; Ovh = 85 + 6 = 91
    let ctx_db_04 = json!({
      "nozzle_type": "diamondback",
      "nozzle_material": "Diamondback PCD",
      "nozzle_diameter": "0.4",
      "filament_type": "PETG",
      "hotend_max_flow": 28.0,
    });
    let eval_db_04_p10 =
      TransformationEngine::evaluate_stack(&base, &[r_p10.clone()], &ctx_db_04);
    assert_eq!(
      eval_db_04_p10
        .resulting_profile
        .get_str("pressure_advance")
        .as_deref(),
      Some("0.034")
    );
    assert_eq!(
      eval_db_04_p10
        .resulting_profile
        .get_str("filament_max_volumetric_speed")
        .as_deref(),
      Some("15.0")
    );
    assert_eq!(
      eval_db_04_p10
        .resulting_profile
        .get_str("fan_min_speed")
        .as_deref(),
      Some("45")
    );
    assert_eq!(
      eval_db_04_p10
        .resulting_profile
        .get_str("fan_max_speed")
        .as_deref(),
      Some("73")
    );
    assert_eq!(
      eval_db_04_p10
        .resulting_profile
        .get_str("overhang_fan_speed")
        .as_deref(),
      Some("91")
    );

    // 1b. DiamondBack 0.80mm vs 0.25mm: PA scales with (0.40/d)^1.35 and MVS scales with (d/0.40)^beta_Q
    let ctx_db_08 = json!({
      "nozzle_type": "diamondback",
      "nozzle_material": "Diamondback PCD",
      "nozzle_diameter": "0.8",
      "filament_type": "PETG",
      "hotend_max_flow": 28.0,
    });
    let eval_db_08 =
      TransformationEngine::evaluate_stack(&base, &[r_p10.clone()], &ctx_db_08);
    let pa_08: f64 = eval_db_08
      .resulting_profile
      .get_str("pressure_advance")
      .unwrap()
      .parse()
      .unwrap();
    let mvs_08: f64 = eval_db_08
      .resulting_profile
      .get_str("filament_max_volumetric_speed")
      .unwrap()
      .parse()
      .unwrap();
    assert!(
      (pa_08 - 0.013).abs() <= 0.002,
      "0.8mm DiamondBack PA should scale down to ~0.013, got {}",
      pa_08
    );
    assert!(
      mvs_08 > 19.0,
      "0.8mm DiamondBack MVS should scale up >19.0 mm3/s, got {}",
      mvs_08
    );

    let ctx_db_025 = json!({
      "nozzle_type": "diamondback",
      "nozzle_material": "Diamondback PCD",
      "nozzle_diameter": "0.25",
      "filament_type": "PETG",
      "hotend_max_flow": 28.0,
    });
    let eval_db_025 =
      TransformationEngine::evaluate_stack(&base, &[r_p10.clone()], &ctx_db_025);
    let pa_025: f64 = eval_db_025
      .resulting_profile
      .get_str("pressure_advance")
      .unwrap()
      .parse()
      .unwrap();
    let mvs_025: f64 = eval_db_025
      .resulting_profile
      .get_str("filament_max_volumetric_speed")
      .unwrap()
      .parse()
      .unwrap();
    assert!(
      pa_025 > 0.060,
      "0.25mm DiamondBack PA should scale up >0.060, got {}",
      pa_025
    );
    assert!(
      mvs_025 < 11.0,
      "0.25mm DiamondBack MVS should scale down <11.0 mm3/s, got {}",
      mvs_025
    );

    // 1c. Telescoping temperature excursion: P10 -> P25 (-2°C) -> P55 Wet (55% RH -> -6°C)
    //     Colder melt increases viscosity (PA increases) and reduces thermal power (MVS decreases)
    //     without triggering any Overwritten shadowing warnings.
    let mut r_wet = mk_rule(
      "sym_wet_filament",
      55,
      include_str!("../transformations/parametric/sym_wet_filament.lua"),
    );
    let mut wet_params = HashMap::new();
    wet_params.insert("humidity_pct".to_string(), "55".to_string());
    r_wet.lua_code = embed_param_defaults_in_lua(&r_wet.lua_code, &wet_params);
    let eval_chain = TransformationEngine::evaluate_stack(
      &base,
      &[r_p10.clone(), r_p25.clone(), r_wet.clone()],
      &ctx_db_04,
    );
    let pa_chain: f64 = eval_chain
      .resulting_profile
      .get_str("pressure_advance")
      .unwrap()
      .parse()
      .unwrap();
    let mvs_chain: f64 = eval_chain
      .resulting_profile
      .get_str("filament_max_volumetric_speed")
      .unwrap()
      .parse()
      .unwrap();
    assert!(
      pa_chain > 0.034,
      "colder melt (-8C net) must increase PA above 0.034, got {}",
      pa_chain
    );
    assert!(
      mvs_chain < 15.0,
      "colder melt (-8C net) must reduce MVS below 15.0, got {}",
      mvs_chain
    );
    let pa_prov = eval_chain
      .provenance_items
      .iter()
      .find(|p| p.param_key == "pressure_advance")
      .unwrap();
    assert!(
      !pa_prov.has_shadowed_steps,
      "telescoping relative PA updates must not be flagged as shadowed"
    );

    // 1d. Warp-prone polymers (ASA/ABS/PC/PA) must NOT receive PCD radiant cooling fan boosts
    let ctx_asa = json!({
      "nozzle_type": "diamondback",
      "nozzle_material": "Diamondback PCD",
      "nozzle_diameter": "0.4",
      "filament_type": "ASA",
    });
    let mut base_asa = base.clone();
    base_asa
      .values
      .insert("fan_min_speed".to_string(), json!(["10"]));
    base_asa
      .values
      .insert("fan_max_speed".to_string(), json!(["25"]));
    let eval_asa =
      TransformationEngine::evaluate_stack(&base_asa, &[r_p10.clone()], &ctx_asa);
    assert_eq!(
      eval_asa
        .resulting_profile
        .get_str("fan_min_speed")
        .as_deref(),
      Some("10")
    );
    assert_eq!(
      eval_asa
        .resulting_profile
        .get_str("fan_max_speed")
        .as_deref(),
      Some("25")
    );

    // 2. Group 4: Coupled Flush / Purge Matrix (`flush_volumes_matrix.lua` + `coupled_purge_factor`)
    let flush_lua = include_str!("../transformations/flush_volumes_matrix.lua");
    let flush_spools = json!([
      { "material_type": "PETG", "color_hex": "#111111", "opacity": 1.0 },
      { "material_type": "PETG", "color_hex": "#FFFFFF", "opacity": 1.0 }
    ]);
    // Stock Brass (melt_zone = 1.0, Brass = 1.0) vs FlowTech Hardened Steel (melt_zone = 1.15) vs FlowTech DiamondBack (1.15 * 0.80 = 0.92)
    let (flush_brass, _, _) = execute_transformation_lua(
      flush_lua,
      &json!({}),
      &json!({
        "nozzle_type": "brass",
        "melt_zone_multiplier": 1.0,
        "slots": flush_spools,
      }),
    )
    .unwrap();
    let (flush_ft_hs, _, _) = execute_transformation_lua(
      flush_lua,
      &json!({}),
      &json!({
        "nozzle_type": "hardened_steel",
        "melt_zone_multiplier": 1.15,
        "slots": flush_spools,
      }),
    )
    .unwrap();
    let (flush_ft_db, _, _) = execute_transformation_lua(
      flush_lua,
      &json!({}),
      &json!({
        "nozzle_type": "diamondback",
        "melt_zone_multiplier": 1.15,
        "slots": flush_spools,
      }),
    )
    .unwrap();
    // Cold-extrusion excursion (-10°C below t_eq) increases melt viscosity and requires more purge volume
    let (flush_ft_db_cold, _, _) = execute_transformation_lua(
      flush_lua,
      &json!({
        "nozzle_temperature": ["210"],
        "_coupled_state": { "t_eq": 220.0 }
      }),
      &json!({
        "nozzle_type": "diamondback",
        "melt_zone_multiplier": 1.15,
        "slots": flush_spools,
      }),
    )
    .unwrap();
    let parse_cell_01 = |cfg: &serde_json::Value| -> i32 {
      cfg["flush_volumes_matrix"][1]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
    };
    let v_brass = parse_cell_01(&flush_brass);
    let v_ft_hs = parse_cell_01(&flush_ft_hs);
    let v_ft_db = parse_cell_01(&flush_ft_db);
    let v_ft_db_cold = parse_cell_01(&flush_ft_db_cold);
    assert!(
      v_ft_hs > v_brass,
      "FlowTech 1.15x melt zone on Hardened Steel ({}) must exceed stock Brass ({})",
      v_ft_hs,
      v_brass
    );
    assert!(
      v_ft_db < v_ft_hs,
      "DiamondBack PCD low-CoF wall slip ({}) must evacuate faster than Hardened Steel ({})",
      v_ft_db,
      v_ft_hs
    );
    assert!(
      v_ft_db_cold > v_ft_db,
      "Colder, more viscous melt ({}) must require more purge volume than nominal temp ({})",
      v_ft_db_cold,
      v_ft_db
    );

    // 3. Group 5: Symptom temperature rules (`sym_stringing`, `sym_weak_layers`, `sym_under_extrusion`)
    //    synchronize coupled PA & fans while preserving explicit MVS/fan overrides.
    let (str_cfg, _, _) = execute_transformation_lua(
      include_str!("../transformations/parametric/sym_stringing.lua"),
      &json!({
        "nozzle_temperature": "250",
        "pressure_advance": "0.040",
        "filament_max_volumetric_speed": "16.0",
      }),
      &json!({ "filament_type": "PCTG", "nozzle_diameter": 0.4 }),
    )
    .unwrap();
    assert_eq!(str_cfg["nozzle_temperature"], "247");
    let str_pa: f64 = str_cfg["pressure_advance"].as_str().unwrap().parse().unwrap();
    let str_mvs: f64 = str_cfg["filament_max_volumetric_speed"]
      .as_str()
      .unwrap()
      .parse()
      .unwrap();
    assert!(str_pa > 0.040, "lowering temp by -3C increases PA: {}", str_pa);
    assert!(str_mvs < 16.0, "lowering temp by -3C trims MVS: {}", str_mvs);

    // In `sym_weak_layers`, explicit fan_max_speed 0.75x cut is preserved while PA and MVS sync with +5C boost
    let (wl_cfg, _, _) = execute_transformation_lua(
      include_str!("../transformations/parametric/sym_weak_layers.lua"),
      &json!({
        "nozzle_temperature": "240",
        "fan_max_speed": "60",
        "pressure_advance": "0.040",
        "filament_max_volumetric_speed": "15.0",
      }),
      &json!({ "filament_type": "PETG", "nozzle_diameter": 0.4 }),
    )
    .unwrap();
    assert_eq!(wl_cfg["nozzle_temperature"], "245");
    assert_eq!(wl_cfg["fan_max_speed"], "45"); // 60 * 0.75 explicit cut honored
    let wl_pa: f64 = wl_cfg["pressure_advance"].as_str().unwrap().parse().unwrap();
    assert!(wl_pa < 0.040, "+5C boost lowers viscosity and PA: {}", wl_pa);

    // 4. Group 6: Coupled Shell Thickness (`structural_strength.lua`, `transparent_optical_glass.lua`, `sym_transparent_glass.lua`)
    let (ss_cfg, _, _) = execute_transformation_lua(
      include_str!("../transformations/parametric/structural_strength.lua"),
      &json!({
        "layer_height": "0.24",
        "top_shell_thickness": "0.80",
        "bottom_shell_thickness": "0.80",
      }),
      &json!({ "filament_type": "PETG" }),
    )
    .unwrap();
    assert_eq!(ss_cfg["top_shell_layers"], "5");
    assert_eq!(ss_cfg["top_shell_thickness"], "1.20");
    assert_eq!(ss_cfg["bottom_shell_thickness"], "1.20");

    let (glass_cfg, _, _) = execute_transformation_lua(
      include_str!("../transformations/transparent_optical_glass.lua"),
      &json!({ "top_shell_thickness": "0.80", "bottom_shell_thickness": "0.80" }),
      &json!({ "nozzle_diameter": 0.4 }),
    )
    .unwrap();
    assert_eq!(glass_cfg["top_shell_layers"], "0");
    assert_eq!(glass_cfg["top_shell_thickness"], "0.00");
    assert_eq!(glass_cfg["bottom_shell_thickness"], "0.00");

    // 5. Group 7: Coupled Volumetric Speed Ceiling for Solid Infill & Top Surface (`volumetric_speed_ceiling.lua`)
    let (vsc_cfg, _, _) = execute_transformation_lua(
      include_str!("../transformations/volumetric_speed_ceiling.lua"),
      &json!({
        "layer_height": "0.28",
        "outer_wall_line_width": "0.63",
        "internal_solid_infill_line_width": "0.63",
        "top_surface_line_width": "0.57",
        "internal_solid_infill_speed": "250",
        "top_surface_speed": "200",
      }),
      &json!({ "hotend_max_flow": 18.0, "filament_max_flow": 15.0 }),
    )
    .unwrap();
    // 15.0 / (0.28 * 0.63) = 85 mm/s for solid infill; 15.0 / (0.28 * 0.57) = 93 mm/s for top surface
    assert_eq!(vsc_cfg["internal_solid_infill_speed"], "85");
    assert_eq!(vsc_cfg["top_surface_speed"], "93");

    // 6. Group 8 & Group 9: Orifice-Diameter Coupled Ironing Spacing, Support XY Distance & Roof Spacing, and Wet 1st-Layer Width
    let (iron_06, _, _) = execute_transformation_lua(
      include_str!("../transformations/parametric/ironing_tuning.lua"),
      &json!({}),
      &json!({ "filament_type": "PETG", "nozzle_diameter": 0.6 }),
    )
    .unwrap();
    assert_eq!(iron_06["ironing_spacing"], "0.20");

    let (sup_06, _, _) = execute_transformation_lua(
      include_str!("../transformations/parametric/support_tuning.lua"),
      &json!({ "layer_height": "0.24" }),
      &json!({ "filament_type": "PETG", "nozzle_diameter": 0.6, "layer_height": 0.24 }),
    )
    .unwrap();
    assert_eq!(sup_06["support_interface_spacing"], "0.18");
    assert_eq!(sup_06["support_object_xy_distance"], "0.51");

    let (wet_06, _, _) = execute_transformation_lua(
      include_str!("../transformations/parametric/sym_wet_filament.lua"),
      &json!({ "nozzle_temperature": "250" }),
      &json!({ "filament_type": "PETG", "nozzle_diameter": 0.6 }),
    )
    .unwrap();
    assert_eq!(wet_06["initial_layer_line_width"], "0.84");
  }

  #[test]
  fn test_exhaustive_key_routing_registry_zero_drop_and_canonical_dual_scope_shadowing() {
    use crate::threemf::{
      build_machine_preset_json_with_overrides, build_process_preset_json,
      build_project_settings_config_json, canonicalize_dual_scope_key, classify_key_destination,
      export_standalone_filament_json_with_overrides,
      verify_project_settings_matches_standalone_presets, KeyDestination,
    };

    // 1. Verify `validate_lua_rule_writes` rejects unrouted phantom keys and shadowed dual-scope keys:
    assert_eq!(
      TransformationEngine::validate_lua_rule_writes(
        "bad_rule",
        &[(
          "phantom_nonexistent_key".to_string(),
          "0".to_string(),
          "1".to_string()
        )]
      ),
      Err("Unrouted phantom write in rule bad_rule: key phantom_nonexistent_key".to_string())
    );
    for (shadow_key, canon_key) in [
      ("retraction_length", "filament_retraction_length"),
      ("retraction_speed", "filament_retraction_speed"),
      ("deretraction_speed", "filament_deretraction_speed"),
      ("z_hop", "filament_z_hop"),
      ("z_hop_types", "filament_z_hop_types"),
      ("retract_lift_enforce", "filament_retract_lift_enforce"),
      ("retract_before_wipe", "filament_retract_before_wipe"),
      ("wipe", "filament_wipe"),
      ("wipe_distance", "filament_wipe_distance"),
      (
        "retraction_minimum_travel",
        "filament_retraction_minimum_travel",
      ),
      ("max_volumetric_speed", "filament_max_volumetric_speed"),
    ] {
      assert_eq!(canonicalize_dual_scope_key(shadow_key), Some(canon_key));
      assert_eq!(
        classify_key_destination(shadow_key),
        None,
        "shadowed dual-scope key '{}' must be rejected unless canonicalized to '{}'",
        shadow_key,
        canon_key
      );
      assert_eq!(
        classify_key_destination(canon_key),
        Some(KeyDestination::Filament),
        "canonical key '{}' must route to Filament destination",
        canon_key
      );
    }

    // 2. Execute every built-in rule and every one of the 44 parametric calibration/symptom
    //    templates across representative contexts and verify that EVERY key modified by ANY
    //    Lua script is classified in the Exhaustive Key Routing Registry (Zero-Drop Invariant).
    let base = OrcaProfile::get_stock_kobra_s1();
    // Verify all canonical `filament_*` dual-scope keys are seeded on the base profile!
    for canon_key in [
      "filament_retraction_length",
      "filament_retraction_speed",
      "filament_deretraction_speed",
      "filament_retract_before_wipe",
      "filament_wipe",
      "filament_wipe_distance",
      "filament_z_hop",
      "filament_z_hop_types",
      "filament_retraction_minimum_travel",
      "filament_retract_lift_enforce",
      "filament_max_volumetric_speed",
    ] {
      assert!(
        base.get_str(canon_key).is_some(),
        "base profile must seed canonical dual-scope key '{}'",
        canon_key
      );
    }

    let contexts = [
      json!({
        "printer_model": "Anycubic Kobra S1",
        "nozzle_type": "diamondback",
        "nozzle_material": "Diamondback PCD",
        "nozzle_diameter": "0.4",
        "hotend": "MicroSwiss FlowTech All-Metal",
        "hotend_max_flow": 28.0,
        "filament_type": "PETG",
        "filament_finish": "High-Speed",
        "all_filament_types": ["PETG", "PLA", "PCTG", "ABS"],
        "addons": ["flowtech_hotend", "ace_pro", "activated_carbon_filter", "enclosed_chamber", "pei_textured_plate"],
        "ace_pro_units": 2,
        "layer_height": 0.20,
        "line_width": 0.42,
      }),
      json!({
        "printer_model": "Anycubic Kobra S1",
        "nozzle_type": "hardened_steel",
        "nozzle_material": "Hardened Steel",
        "nozzle_diameter": "0.6",
        "hotend": "Kobra S1 All-Metal",
        "hotend_max_flow": 22.0,
        "filament_type": "TPU 95A",
        "filament_finish": "Foaming",
        "filament_is_elastomer": true,
        "all_filament_types": ["TPU 95A"],
        "addons": ["aux_part_fan"],
        "ace_pro_units": 0,
        "layer_height": 0.24,
        "line_width": 0.62,
      }),
    ];

    for rule in TransformationEngine::get_default_rules() {
      for ctx in &contexts {
        let (_, _, modified) = execute_transformation_lua(
          &rule.lua_code,
          &serde_json::Value::Object(base.values.clone()),
          ctx,
        )
        .unwrap_or_else(|e| panic!("default rule '{}' failed: {}", rule.id, e));
        TransformationEngine::validate_lua_rule_writes(&rule.id, &modified)
          .unwrap_or_else(|e| panic!("{}", e));
      }
    }

    for tpl in TransformationEngine::get_parameterized_calibration_templates() {
      let mut ans = tpl.default_answers();
      if tpl.id == "weather_climate_adjustment" {
        tpl.apply_answer_change(&mut ans, "climate_preset", "tropical".to_string());
      } else if tpl.id == "clogging_heat_creep_reliever" {
        ans.insert("in_ace_pro".to_string(), "yes".to_string());
      } else if tpl.id == "first_layer_adhesion_booster" {
        ans.insert("enable_z_squish".to_string(), "yes".to_string());
      }
      let (_, _, _, lua_code) = tpl.generate_from_answers(&ans, "all");
      for ctx in &contexts {
        let (_, _, modified) = execute_transformation_lua(
          &lua_code,
          &serde_json::Value::Object(base.values.clone()),
          ctx,
        )
        .unwrap_or_else(|e| panic!("template '{}' failed: {}", tpl.id, e));
        TransformationEngine::validate_lua_rule_writes(&tpl.id, &modified)
          .unwrap_or_else(|e| panic!("{}", e));
      }
    }

    // 3. Verify Canonical Dual-Scope Shadowing in `evaluate_stack` + End-to-End Export Routing:
    //    Suppose a user or custom rule writes `config["z_hop"] = "0.32"`, `config["wipe"] = "1"`,
    //    `config["retract_lift_enforce"] = "Top Surfaces Only"`, `config["retraction_distances_when_cut"] = "16"`,
    //    and `config["elefant_foot_compensation_layers"] = "2"`.
    //    `evaluate_stack` canonicalizes `z_hop` -> `filament_z_hop`, `wipe` -> `filament_wipe`,
    //    `retract_lift_enforce` -> `filament_retract_lift_enforce`, and exports all keys to their
    //    exact standalone JSON and 3MF `project_settings.config` destinations with 100% parity!
    let custom_rule = TransformationRule {
      id: "custom_dual_scope_test".to_string(),
      name: "Custom Dual-Scope & Routing Test".to_string(),
      description: String::new(),
      category: "Custom".to_string(),
      icon: "🧪".to_string(),
      enabled: true,
      condition_expr: "all".to_string(),
      lua_code: r#"
        config["z_hop"] = "0.32"
        config["wipe"] = "1"
        config["retract_lift_enforce"] = "Top Surfaces Only"
        config["retraction_distances_when_cut"] = "16"
        config["elefant_foot_compensation_layers"] = "2"
        config["overhang_fan_speed"] = "95"
      "#
      .to_string(),
      priority: 50,
      scope: RuleScopeFilter::default(),
    };
    let eval = TransformationEngine::evaluate_stack(&base, &[custom_rule], &contexts[0]);
    assert_eq!(
      eval.resulting_profile.get_str("filament_z_hop").as_deref(),
      Some("0.32")
    );
    assert_eq!(
      eval
        .resulting_profile
        .get_str("filament_retract_lift_enforce")
        .as_deref(),
      Some("Top Surfaces Only")
    );
    assert!(eval
      .execution_logs
      .iter()
      .any(|l| l.contains("Canonicalized shadowed dual-scope key 'z_hop' → 'filament_z_hop'")));

    let rig = MachineRig::kobra_s1_hardened_steel_png();
    let procs = crate::models::ProcessProfile::get_default_processes();
    let catalog = crate::catalog::load_merged_catalog();
    let petg_entry = catalog
      .filaments
      .iter()
      .find(|f| f.code == "PETG")
      .expect("PETG catalog entry");
    let spool = FilamentSpool::from_catalog_entry(1, petg_entry, &rig.nozzle_type);
    let overrides = eval.resulting_profile.values.clone();

    let mach_str = build_machine_preset_json_with_overrides(&rig, Some(&overrides));
    let mach_json: serde_json::Value = serde_json::from_str(&mach_str).unwrap();
    assert_eq!(
      mach_json["retraction_distances_when_cut"],
      json!(["16"]),
      "retraction_distances_when_cut must route to Machine preset"
    );

    let proc_str = build_process_preset_json(&procs[0], &rig, &overrides);
    let proc_json: serde_json::Value = serde_json::from_str(&proc_str).unwrap();
    assert_eq!(
      proc_json["elefant_foot_compensation_layers"],
      json!("2"),
      "elefant_foot_compensation_layers must route to Process preset"
    );

    let fil_str = export_standalone_filament_json_with_overrides(&spool, &rig, &overrides);
    let fil_json: serde_json::Value = serde_json::from_str(&fil_str).unwrap();
    assert_eq!(
      fil_json["filament_retract_lift_enforce"],
      json!(["Top Surfaces Only"]),
      "canonicalized filament_retract_lift_enforce must route to Filament preset"
    );
    assert_eq!(
      fil_json["overhang_fan_speed"],
      json!(["95"]),
      "touched OPTIONAL_FILAMENT_SETTING_KEYS (overhang_fan_speed) must route to Filament preset"
    );

    let ps_str = build_project_settings_config_json(
      &rig,
      &procs[0],
      std::slice::from_ref(&spool),
      &[],
      &overrides,
    );
    let mut fil_map = std::collections::HashMap::new();
    fil_map.insert(spool.format_concise_title(), fil_str);
    verify_project_settings_matches_standalone_presets(&ps_str, &mach_str, &proc_str, &fil_map)
      .expect("project_settings.config must match standalone Machine, Process, and Filament presets 100%");
  }

  #[test]
  fn test_immutable_base_x0_snapshot_and_order_independent_regime_evaluation() {
    let mk_rule = |id: &str, prio: i32, lua: &str| TransformationRule {
      id: id.to_string(),
      name: id.to_string(),
      description: String::new(),
      category: "Test".to_string(),
      icon: "⚙️".to_string(),
      enabled: true,
      condition_expr: "all".to_string(),
      lua_code: lua.to_string(),
      priority: prio,
      scope: RuleScopeFilter::default(),
    };

    // 1. Read-only immutability enforcement on `base`:
    for bad_script in [
      r#"base["nozzle_temperature"] = "250""#,
      r#"base.nozzle_temperature = "250""#,
      r#"base["nozzle_temperature"][1] = "250""#,
      r#"function apply(config, ctx) base = {} end"#,
    ] {
      let err = execute_transformation_lua(
        bad_script,
        &json!({ "nozzle_temperature": ["230"] }),
        &json!({}),
      )
      .expect_err("mutating read-only `base` must fail");
      assert!(
        err.contains("[Lua Immutability Violation]"),
        "expected Lua Immutability Violation, got: {}",
        err
      );
    }

    // 2. Long-travel flexible filament (r0 = 1.30 mm, T0 = 230 C) where P12 + an intervening P15
    //    rule compress `config["filament_retraction_length"]` from 1.30 -> 0.90 mm (below the old
    //    r1 = 1.10 mm threshold!). Because P25 (`symptom_stringing_diamondback`) reads `r0_ref`
    //    from the immutable `base["filament_retraction_length"]` (1.30 >= 1.22), `rigid_gate` stays
    //    0.0 and P25 applies ZERO extra temperature trim!
    let mut base_flex = OrcaProfile::new("Long-Travel Flex", "filament");
    base_flex
      .values
      .insert("nozzle_temperature".to_string(), json!(["230"]));
    base_flex.values.insert(
      "nozzle_temperature_initial_layer".to_string(),
      json!(["235"]),
    );
    base_flex
      .values
      .insert("filament_retraction_length".to_string(), json!(["1.30"]));
    base_flex
      .values
      .insert("filament_retraction_speed".to_string(), json!(["30"]));
    base_flex
      .values
      .insert("filament_flow_ratio".to_string(), json!(["0.980"]));
    base_flex
      .values
      .insert("filament_z_hop".to_string(), json!(["0.15"]));

    let r_p10 = mk_rule(
      "thermal_conductivity",
      10,
      include_str!("../transformations/thermal_conductivity.lua"),
    );
    let r_p12 = mk_rule(
      "rig_addons_cooling",
      12,
      include_str!("../transformations/rig_addons_cooling.lua"),
    );
    let r_p15_compress = mk_rule(
      "extra_retract_compress",
      15,
      r#"config["filament_retraction_length"] = { "0.90" }"#,
    );
    let r_p25 = mk_rule(
      "symptom_stringing_diamondback",
      25,
      include_str!("../transformations/symptom_stringing_diamondback.lua"),
    );

    let ctx_db = json!({
      "nozzle_type": "diamondback",
      "nozzle_material": "Diamondback PCD",
      "nozzle_diameter": "0.4",
      "addons": ["flowtech_hotend"],
    });
    let eval_flex = TransformationEngine::evaluate_stack(
      &base_flex,
      &[r_p10.clone(), r_p12.clone(), r_p15_compress, r_p25.clone()],
      &ctx_db,
    );
    // P10 drops 230 -> 220 C; P25 sees base["filament_retraction_length"] == 1.30 -> rigid_gate == 0.0 -> stays 220 C!
    assert_eq!(
      eval_flex
        .resulting_profile
        .get_str("nozzle_temperature")
        .as_deref(),
      Some("220"),
      "P25 must read r0=1.30 from immutable `base`, keeping rigid_gate=0 even after P15 compressed config retraction to 0.90mm"
    );

    // 3. Order-independence of regime evaluation between P10 (`thermal_conductivity`) and P25
    //    (`symptom_stringing_diamondback`): whether P10 runs at priority 10 and P25 at 25, OR P25
    //    runs at priority 5 and P10 at 10, both rules evaluate their PWL knots against `base` (T0 = 235 C)
    //    and accumulate the exact same final temperature (235 - 10 - 2 = 223 C)!
    let mut base_petg = OrcaProfile::new("PETG Order Test", "filament");
    base_petg
      .values
      .insert("nozzle_temperature".to_string(), json!(["235"]));
    base_petg.values.insert(
      "nozzle_temperature_initial_layer".to_string(),
      json!(["235"]),
    );
    base_petg
      .values
      .insert("filament_retraction_length".to_string(), json!(["0.80"]));
    base_petg
      .values
      .insert("filament_retraction_speed".to_string(), json!(["40"]));
    base_petg
      .values
      .insert("filament_flow_ratio".to_string(), json!(["0.960"]));
    base_petg
      .values
      .insert("filament_z_hop".to_string(), json!(["0.15"]));

    let eval_p10_then_p25 = TransformationEngine::evaluate_stack(
      &base_petg,
      &[r_p10.clone(), r_p12.clone(), r_p25.clone()],
      &ctx_db,
    );
    let mut r_p25_first = r_p25;
    r_p25_first.priority = 5;
    let eval_p25_then_p10 = TransformationEngine::evaluate_stack(
      &base_petg,
      &[r_p25_first, r_p10, r_p12],
      &ctx_db,
    );
    assert_eq!(
      eval_p10_then_p25
        .resulting_profile
        .get_str("nozzle_temperature")
        .as_deref(),
      Some("223")
    );
    assert_eq!(
      eval_p25_then_p10
        .resulting_profile
        .get_str("nozzle_temperature")
        .as_deref(),
      Some("223"),
      "Evaluating PWL regime knots against immutable `base` (x0) makes P10 + P25 temperature composition order-independent"
    );
  }

  #[test]
  fn test_provenance_ledger_chain_clamped_step_and_three_way_export_parity() {
    use crate::models::ProcessProfile;
    use crate::threemf::{
      build_machine_preset_json_with_overrides, build_process_preset_json,
      build_project_settings_config_json, evaluate_spool_filament_provenance,
      export_standalone_filament_json_with_overrides,
      verify_provenance_matches_standalone_and_3mf_exports,
    };

    let mk_rule = |id: &str, name: &str, prio: i32, lua: &str| TransformationRule {
      id: id.to_string(),
      name: name.to_string(),
      description: String::new(),
      category: "Test".to_string(),
      icon: "⚙️".to_string(),
      enabled: true,
      condition_expr: "all".to_string(),
      lua_code: lua.to_string(),
      priority: prio,
      scope: RuleScopeFilter::default(),
    };

    // 1. Verify exact user scenario:
    //    x0 (0.80 mm) --[P12 (-0.10 mm)]--> x1 (0.70 mm) --[P25 (+0.05 mm)]--> x2 (0.75 mm)
    //    --[Epilogue Clamp [0.40, 0.72]]--> x_shipped (0.72 mm)
    let mut base = OrcaProfile::new("Ledger Clamp Test", "filament");
    base
      .values
      .insert("filament_retraction_length".to_string(), json!(["0.80"]));
    let r_p12 = mk_rule(
      "rule_p12",
      "P12 FlowTech Retraction Trim",
      12,
      r#"
        config["filament_retraction_length"] = {"0.70"}
        return config
      "#,
    );
    let r_p25 = mk_rule(
      "rule_p25",
      "P25 Anti-Stringing Micro-Bump + Ceiling",
      25,
      r#"
        config["filament_retraction_length"] = {"0.75"}
        constrain("filament_retraction_length", 0.40, 0.72, "Direct-drive heatbreak ceiling")
        return config
      "#,
    );
    let eval = TransformationEngine::evaluate_stack(&base, &[r_p12, r_p25], &json!({}));
    let ret_prov = eval
      .provenance_items
      .iter()
      .find(|p| p.param_key == "filament_retraction_length")
      .expect("Expected filament_retraction_length provenance");
    ret_prov
      .verify_ledger_chain()
      .expect("Ledger chain must be continuous from x0 to x_shipped");
    assert_eq!(ret_prov.raw_final_scalar(), "0.72");
    assert_eq!(ret_prov.steps.len(), 3, "Must record P12, P25, and Epilogue Clamp steps");
    assert_eq!(ret_prov.steps[0].rule_id, "rule_p12");
    assert_eq!(ret_prov.steps[0].before_value, "0.80 mm");
    assert_eq!(ret_prov.steps[0].after_value, "0.70 mm");
    assert_eq!(ret_prov.steps[1].rule_id, "rule_p25");
    assert_eq!(ret_prov.steps[1].before_value, "0.70 mm");
    assert_eq!(ret_prov.steps[1].after_value, "0.75 mm");
    assert_eq!(
      ret_prov.steps[1].shadow_kind,
      Some(RuleShadowKind::Clamped),
      "P25 step must be tagged as Clamped by the Epilogue Constraint Projection"
    );
    assert_eq!(
      ret_prov.steps[1].shadowed_by_rule_id.as_deref(),
      Some("constraint_projection")
    );
    assert_eq!(
      ret_prov.steps[1].shadowed_final_value.as_deref(),
      Some("0.72 mm")
    );
    assert_eq!(ret_prov.steps[2].rule_id, "constraint_projection");
    assert_eq!(ret_prov.steps[2].before_value, "0.75 mm");
    assert_eq!(ret_prov.steps[2].after_value, "0.72 mm");

    // 2. Verify full 3-way parity (`x_provenance == x_JSON == x_3MF`) including an Ender-3 (260°C
    //    hotend ceiling) with Hardened Steel nozzle (+5°C thermal boost) and a PA6-CF spool whose
    //    260°C seed gets boosted to 265°C by P10 and then clamped back to 260°C by Phase 3 Epilogue!
    let mut ender_rig = MachineRig::kobra_s1_hardened_steel_png();
    ender_rig.printer_model = "Creality Ender-3".to_string();
    ender_rig.kinematics = "Cartesian (Bed-Slinger)".to_string();
    ender_rig.nozzle_type = "orca_hardened_steel".to_string();
    ender_rig.nozzle_material = "Hardened Steel".to_string();
    let catalog = crate::catalog::load_merged_catalog();
    let pa_entry = catalog
      .filaments
      .iter()
      .find(|f| f.code == "PA-CF" || f.code == "PA6-CF")
      .expect("PA-CF in catalog");
    let pa_spool = FilamentSpool::from_catalog_entry(1, pa_entry, "orca_hardened_steel");
    let spools = vec![pa_spool];
    let procs = ProcessProfile::get_default_processes();
    let active_proc = &procs[0];
    let rules = TransformationEngine::get_default_rules();
    let base_prof = OrcaProfile::get_stock_kobra_s1();
    let eval_ctx = json!({
      "printer_model": ender_rig.printer_model,
      "nozzle_type": ender_rig.nozzle_type,
      "nozzle_diameter": ender_rig.nozzle_diameter,
      "nozzle_material": ender_rig.nozzle_material,
      "hotend_max_temp_c": 260,
      "filament_type": spools[0].material_type,
      "addons": ender_rig.addons,
      "climate_preset": ender_rig.climate_preset,
    });
    let (seeded_base, seeded_ctx) =
      TransformationEngine::seed_process_inputs(&base_prof, &eval_ctx, active_proc);
    let mut proc_eval = TransformationEngine::evaluate_stack(&seeded_base, &rules, &seeded_ctx);
    let per_proc_overrides =
      TransformationEngine::evaluate_stack_per_process(&base_prof, &rules, &eval_ctx, &procs);
    let derived_overrides = per_proc_overrides
      .get(&active_proc.id)
      .cloned()
      .unwrap_or_else(|| proc_eval.resulting_profile.values.clone());

    let spool_eval = evaluate_spool_filament_provenance(
      &spools[0],
      &ender_rig,
      Some(&derived_overrides),
    );
    // Verify PA6-CF on 260°C Ender-3 with Hardened Steel nozzle shows 260 -> 265 (Clamped) -> 260 in provenance!
    let noz_prov = spool_eval
      .provenance_items
      .iter()
      .find(|p| p.param_key == "nozzle_temperature")
      .expect("nozzle_temperature must have provenance (260 -> 265 -> 260)");
    noz_prov.verify_ledger_chain().unwrap();
    assert_eq!(noz_prov.raw_final_scalar(), "260");
    assert!(
      noz_prov
        .steps
        .iter()
        .any(|s| s.shadow_kind == Some(RuleShadowKind::Clamped)),
      "Hardened steel +5°C step must be marked Clamped by the 260°C hotend ceiling: {:?}",
      noz_prov.steps
    );

    // Also verify merge_active_spool_provenance populates the UI StackEvaluationResult with the active spool's ledger
    TransformationEngine::merge_active_spool_provenance(&mut proc_eval, &spool_eval);
    for prov in &proc_eval.provenance_items {
      prov.verify_ledger_chain().unwrap();
    }

    let mach_str = build_machine_preset_json_with_overrides(&ender_rig, Some(&derived_overrides));
    let proc_str = build_process_preset_json(active_proc, &ender_rig, &derived_overrides);
    let fil_str = export_standalone_filament_json_with_overrides(&spools[0], &ender_rig, &derived_overrides);
    let mut fil_map = std::collections::HashMap::new();
    fil_map.insert(spools[0].format_concise_title(), fil_str);
    let proj_cfg = build_project_settings_config_json(
      &ender_rig,
      active_proc,
      &spools,
      &proc_eval.flush_matrix,
      &derived_overrides,
    );
    let slot_provs = vec![spool_eval.provenance_items];
    verify_provenance_matches_standalone_and_3mf_exports(
      &proc_eval.provenance_items,
      &slot_provs,
      &proj_cfg,
      &mach_str,
      &proc_str,
      &fil_map,
    )
    .expect("3-way parity (x_provenance == x_JSON == x_3MF) must hold for all keys");
  }

  #[test]
  fn test_deferred_epilogue_quantization_eliminates_intermediate_rounding_drift() {
    let mk_rule = |id: &str, name: &str, prio: i32, lua: &str| TransformationRule {
      id: id.to_string(),
      name: name.to_string(),
      description: String::new(),
      category: "Test".to_string(),
      icon: "⚙️".to_string(),
      enabled: true,
      condition_expr: "all".to_string(),
      lua_code: lua.to_string(),
      priority: prio,
      scope: RuleScopeFilter::default(),
    };

    // 1. Exact prompt scenario:
    //    Seed `nozzle_temperature = {"235"}`.
    //    Rule A computes ΔT = -10.4 °C (235 -> 224.6; intermediate "%.0f" would round to "225").
    //    Rule B computes ΔT = -1.4 °C (224.6 -> 223.2; reading rounded "225" would give 223.6 -> "224").
    //    True sum: 235 - 10.4 - 1.4 = 223.2 -> "223".
    //    Also test 3-step retraction length:
    //    Seed `filament_retraction_length = {"0.80"}`.
    //    Rule A: -0.094 mm (0.80 -> 0.706; intermediate "%.2f" would round to "0.71").
    //    Rule B: -0.004 mm (0.706 -> 0.702; reading "0.71" would give 0.706 -> "0.71").
    //    Rule C: +0.044 mm (0.702 -> 0.746 -> "0.75"; reading "0.71" would give 0.754 -> "0.75",
    //    or with +0.046: 0.702 + 0.046 = 0.748 -> "0.75" vs 0.71 + 0.046 = 0.756 -> "0.76"!).
    let mut base = OrcaProfile::new("Quantization Drift Test", "filament");
    base
      .values
      .insert("nozzle_temperature".to_string(), json!(["235"]));
    base
      .values
      .insert("filament_retraction_length".to_string(), json!(["0.80"]));
    base
      .values
      .insert("filament_flow_ratio".to_string(), json!(["0.960"]));

    let rule_a = mk_rule(
      "rule_a",
      "Rule A (-10.4 C, -0.094 mm, -0.0034 flow)",
      10,
      r#"
        config["nozzle_temperature"] = map_num(config["nozzle_temperature"], function(t)
          return t - 10.4
        end, "%.0f")
        config["filament_retraction_length"] = map_num(config["filament_retraction_length"], function(r)
          return r - 0.094
        end, "%.2f")
        config["filament_flow_ratio"] = map_num(config["filament_flow_ratio"], function(f)
          return f - 0.0034
        end, "%.3f")
      "#,
    );
    let rule_b = mk_rule(
      "rule_b",
      "Rule B (-1.4 C, -0.004 mm, -0.0034 flow)",
      20,
      r#"
        config["nozzle_temperature"] = map_num(config["nozzle_temperature"], function(t)
          return t - 1.4
        end, "%.0f")
        config["filament_retraction_length"] = map_num(config["filament_retraction_length"], function(r)
          return r - 0.004
        end, "%.2f")
        config["filament_flow_ratio"] = map_num(config["filament_flow_ratio"], function(f)
          return f - 0.0034
        end, "%.3f")
      "#,
    );
    let rule_c = mk_rule(
      "rule_c",
      "Rule C (+0.046 mm)",
      30,
      r#"
        config["filament_retraction_length"] = map_num(config["filament_retraction_length"], function(r)
          return r + 0.046
        end, "%.2f")
      "#,
    );

    let eval = TransformationEngine::evaluate_stack(&base, &[rule_a, rule_b, rule_c], &json!({}));
    assert_eq!(
      eval.resulting_profile.get_str("nozzle_temperature").as_deref(),
      Some("223"),
      "235 - 10.4 - 1.4 = 223.2 must quantize once in Phase 3 Epilogue to '223' (not drift to '224' via intermediate %.0f)"
    );
    assert_eq!(
      eval
        .resulting_profile
        .get_str("filament_retraction_length")
        .as_deref(),
      Some("0.75"),
      "0.80 - 0.094 - 0.004 + 0.046 = 0.748 must quantize once in Phase 3 Epilogue to '0.75' (not drift to '0.76' via intermediate %.2f)"
    );
    assert_eq!(
      eval
        .resulting_profile
        .get_str("filament_flow_ratio")
        .as_deref(),
      Some("0.953"),
      "0.960 - 0.0034 - 0.0034 = 0.9532 must quantize once in Phase 3 Epilogue to '0.953' (not drift to '0.954' via intermediate %.3f)"
    );

    for prov in &eval.provenance_items {
      prov
        .verify_ledger_chain()
        .expect("Provenance ledger chain must remain 100% continuous after Phase 3 Epilogue quantization");
    }

    // 2. Verify monotonicity across a sweep of base temperatures T0 ∈ [195..295] and base
    //    retractions r0 ∈ [0.45..1.05] through the production P10 -> P12 -> P25 stack.
    let r_p10 = mk_rule(
      "thermal_conductivity",
      "P10 Thermal Conductivity",
      10,
      include_str!("../transformations/thermal_conductivity.lua"),
    );
    let r_p12 = mk_rule(
      "rig_addons_cooling",
      "P12 FlowTech Hotend",
      12,
      include_str!("../transformations/rig_addons_cooling.lua"),
    );
    let r_p25 = mk_rule(
      "symptom_stringing_diamondback",
      "P25 Diamondback Anti-Stringing",
      25,
      include_str!("../transformations/symptom_stringing_diamondback.lua"),
    );
    let ctx_db = json!({
      "nozzle_type": "diamondback",
      "nozzle_material": "Diamondback PCD",
      "nozzle_diameter": "0.4",
      "addons": ["flowtech_hotend"],
    });
    let mut prev_shipped_t = 0i32;
    for t0 in (195..=295).step_by(2) {
      let mut p = OrcaProfile::new("Temp Sweep", "filament");
      p.values
        .insert("nozzle_temperature".to_string(), json!([t0.to_string()]));
      p.values.insert(
        "nozzle_temperature_initial_layer".to_string(),
        json!([t0.to_string()]),
      );
      p.values
        .insert("filament_retraction_length".to_string(), json!(["0.80"]));
      p.values
        .insert("filament_retraction_speed".to_string(), json!(["40"]));
      let res = TransformationEngine::evaluate_stack(
        &p,
        &[r_p10.clone(), r_p12.clone(), r_p25.clone()],
        &ctx_db,
      );
      let shipped_t: i32 = res
        .resulting_profile
        .get_str("nozzle_temperature")
        .unwrap()
        .parse()
        .unwrap();
      assert!(
        shipped_t >= prev_shipped_t,
        "Monotonicity violated at T0={}: shipped {} < previous {}",
        t0,
        shipped_t,
        prev_shipped_t
      );
      prev_shipped_t = shipped_t;
    }
  }
}
