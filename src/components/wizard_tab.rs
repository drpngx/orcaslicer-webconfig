use crate::models::ProcessProfile;
use crate::orca_profile::{
  DiffItem, OrcaProfile, SettingValueKind, WizardActionOp, WizardRuleAction, WizardSettingDef,
};
use crate::transformations::{
  ParameterizedCalibrationTemplate, RuleBeforeAfterChange, StackReorderAction,
  TransformationEngine, TransformationRule,
};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct WizardTabProps {
  pub is_config_loaded: bool,
  pub rules: Vec<TransformationRule>,
  pub diff_items: Vec<DiffItem>,
  pub rule_changes: Vec<RuleBeforeAfterChange>,
  pub processes: Vec<ProcessProfile>,
  pub active_process_id: String,
  pub on_select_process: Callback<String>,
  pub on_toggle_rule: Callback<String>,
  pub on_update_rule_lua: Callback<(String, String)>,
  pub on_create_rule: Callback<TransformationRule>,
  pub on_delete_rule: Callback<String>,
  pub on_reorder_rule: Callback<(String, StackReorderAction)>,
  pub on_export_package: Callback<()>,
  pub on_commit_save: Callback<()>,
  pub on_open_bind_modal: Callback<()>,
  pub execution_logs: Vec<String>,
  pub on_clear_logs: Callback<()>,
  pub safe_speed_ceiling: f64,
}

#[function_component]
pub fn WizardTab(props: &WizardTabProps) -> Html {
  let on_toggle_rule = props.on_toggle_rule.clone();
  let selected_rule_id = use_state(|| {
    props
      .rules
      .first()
      .map(|r| r.id.clone())
      .unwrap_or_default()
  });

  // State for the interactive "Create / Add New Lua Rule" Builder, Calibration Q&A Wizard & Simple Visual Setting Wizard
  let show_rule_creator = use_state(|| false);
  let creator_mode = use_state(|| "calibration_qa".to_string()); // "calibration_qa", "visual_wizard", or "preset_templates"
  let setting_catalog = use_memo((), |_| OrcaProfile::get_wizard_setting_catalog());
  let search_query = use_state(String::new);
  let section_filter = use_state(|| "All".to_string());
  let selected_setting_key = use_state(|| "nozzle_temperature".to_string());
  let selected_op = use_state(|| WizardActionOp::AddOffset);
  let custom_operand = use_state(|| "+5".to_string());
  let staged_actions = use_state(Vec::<WizardRuleAction>::new);

  let cal_templates = use_memo((), |_| {
    TransformationEngine::get_parameterized_calibration_templates()
  });
  let selected_cal_idx = use_state(|| 0usize);
  let issue_tree_category = use_state(|| "all".to_string());
  let issue_search_query = use_state(String::new);
  let cal_answers = use_state(|| {
    TransformationEngine::get_parameterized_calibration_templates()
      .first()
      .map(|t| t.default_answers())
      .unwrap_or_default()
  });

  let templates = use_memo((), |_| TransformationEngine::get_rule_templates());
  let selected_template_idx = use_state(|| 0usize);
  let draft_name = use_state(|| "Calibration: YOLO Flow Rate (0.960 → 0.940)".to_string());
  let draft_desc = use_state(|| {
    "YOLO Flow Rate Calibration for filament:PCTG: starting flow 0.960 with smoothest chip -0.02 → new filament_flow_ratio = 0.940."
      .to_string()
  });
  let draft_category = use_state(|| "Calibration Print #2 (Flow Rate)".to_string());
  let draft_icon = use_state(|| "🎯".to_string());
  let draft_condition = use_state(|| "filament:PCTG".to_string());
  let draft_lua = use_state(|| {
    let cals = TransformationEngine::get_parameterized_calibration_templates();
    if let Some(first) = cals.first() {
      let ans = first.default_answers();
      let (_, _, _, lua) = first.generate_from_answers(&ans, &first.default_condition_expr);
      lua
    } else {
      String::new()
    }
  });

  if !props.is_config_loaded {
    return html! {
      <div class="flex flex-col items-center justify-center bg-surface-container-low p-space-xl rounded-2xl border border-outline-variant gap-space-md text-center my-8">
        <span class="material-symbols-outlined text-primary text-5xl">{"auto_fix_high"}</span>
        <h2 class="font-headline-lg text-headline-lg text-on-surface font-bold">
          {"Wizard & Lua Transformation Stack Inactive (No Configs Loaded)"}
        </h2>
        <p class="text-body-md text-on-surface-variant max-w-xl">
          {"Nothing is loaded. Use the startup Import Modal (or click Reset All in the top right) to import your configuration directory first."}
        </p>
      </div>
    };
  }

  let mut ordered_rules = props.rules.clone();
  ordered_rules.sort_by_key(|r| r.priority);

  let active_rule = ordered_rules
    .iter()
    .find(|r| r.id == *selected_rule_id)
    .or_else(|| ordered_rules.first());

  let active_count = ordered_rules.iter().filter(|r| r.enabled).count();
  let total_count = ordered_rules.len();
  let modified_diffs: Vec<&DiffItem> = props.diff_items.iter().filter(|d| d.is_modified).collect();
  let active_proc = props
    .processes
    .iter()
    .find(|p| p.id == props.active_process_id)
    .cloned()
    .unwrap_or_else(|| ProcessProfile::get_default_processes().remove(1));
  let on_select_process = props.on_select_process.clone();

  html! {
    <div class="flex flex-col w-full gap-space-lg">
      // Header Banner
      <div class="flex flex-col lg:flex-row items-stretch lg:items-center justify-between gap-space-lg bg-surface-container-low p-space-xl rounded-xl shadow-md border border-outline-variant">
        <div class="flex flex-col gap-space-xs">
          <div class="flex items-center gap-space-sm flex-wrap">
            <span class="material-symbols-outlined text-primary text-3xl">{"auto_fix_high"}</span>
            <h1 class="font-headline-lg text-headline-xl text-on-surface font-bold">
              {"Default Lua Transformation Stack & Before / After Inspector"}
            </h1>
            <span class="bg-primary/20 text-primary border border-primary/40 px-2.5 py-0.5 rounded-full text-xs font-mono font-bold">
              {format!("{} / {} Rules Active", active_count, total_count)}
            </span>
          </div>
          <p class="text-body-md text-on-surface-variant max-w-3xl">
            {"Default hardware and filament rules are applied automatically based on your selected nozzle, add-ons, and multi-material hub. Inspect the exact Before → After effect of every transformation below."}
          </p>
        </div>

        <div class="flex flex-wrap items-center gap-space-md">
          // 3-Way High Quality / Standard / High Speed Toggle
          <div class="flex items-center bg-surface-container-lowest p-1 rounded-lg border border-outline-variant font-mono text-xs">
            {for [
              ("high_quality", "🎯 High Quality"),
              ("standard", "⚖️ Standard"),
              ("high_speed", "🚀 High Speed"),
            ].into_iter().map(|(tier_id, tier_label)| {
              let matching_proc = props
                .processes
                .iter()
                .find(|p| p.quality_tier() == tier_id)
                .or_else(|| props.processes.first());
              let is_sel = active_proc.quality_tier() == tier_id;
              let target_pid = matching_proc.map(|p| p.id.clone()).unwrap_or_default();
              let cb = on_select_process.clone();

              html! {
                <button
                  class={format!(
                    "px-space-md py-1.5 rounded transition-all cursor-pointer {}",
                    if is_sel {
                      "bg-primary text-on-primary font-bold shadow"
                    } else {
                      "text-on-surface-variant hover:text-on-surface"
                    }
                  )}
                  onclick={Callback::from(move |_| {
                    if !target_pid.is_empty() {
                      cb.emit(target_pid.clone());
                    }
                  })}
                >
                  {tier_label}
                </button>
              }
            })}
          </div>

          <button
            class="flex items-center gap-space-xs bg-surface-container-high hover:bg-surface-bright text-on-surface px-space-md py-space-sm rounded-lg text-xs font-semibold transition-colors cursor-pointer"
            onclick={
              let cb = props.on_export_package.clone();
              Callback::from(move |_| cb.emit(()))
            }
          >
            <span class="material-symbols-outlined text-[16px]">{"file_download"}</span>
            {"Export Lua Stack (1-Click)"}
          </button>

          <button
            class="flex items-center gap-space-sm bg-primary-container hover:bg-primary text-on-primary-container px-space-lg py-space-md rounded-lg font-semibold shadow-md transition-colors cursor-pointer"
            onclick={
              let cb = props.on_commit_save.clone();
              Callback::from(move |_| cb.emit(()))
            }
          >
            <span class="material-symbols-outlined text-[18px]">{"publish"}</span>
            {"Sync & Save Changes"}
          </button>
        </div>
      </div>

      // Overall Before -> After Summary Table
      <div class="flex flex-col gap-space-md bg-surface-container-low p-space-lg rounded-xl shadow-md border border-outline-variant">
        <div class="flex items-center justify-between flex-wrap gap-space-sm">
          <div class="flex items-center gap-space-sm">
            <span class="material-symbols-outlined text-tertiary">{"compare_arrows"}</span>
            <h2 class="font-headline-sm text-headline-sm text-on-surface font-semibold">
              {"Transformation Stack Impact — Before (Base Config) vs After (Transformed Output)"}
            </h2>
          </div>
          <span class="text-xs font-mono text-tertiary bg-tertiary-container/20 px-2.5 py-0.5 rounded">
            {format!("{} parameters modified by active stack", modified_diffs.len())}
          </span>
        </div>

        {if modified_diffs.is_empty() {
          html! {
            <div class="text-xs font-mono text-on-surface-variant bg-surface-container-lowest p-space-md rounded-lg border border-outline-variant/60">
              {"No parameters differ from base defaults under the current rule selection."}
            </div>
          }
        } else {
          html! {
            <div class="overflow-x-auto">
              <div class="grid grid-cols-12 px-space-md py-space-xs bg-surface-container-lowest text-on-surface-variant font-label-caps text-label-caps uppercase rounded">
                <div class="col-span-4">{"Parameter"}</div>
                <div class="col-span-2 text-right">{"Before (Base)"}</div>
                <div class="col-span-1 text-center">{"→"}</div>
                <div class="col-span-2 text-left">{"After (Rule)"}</div>
                <div class="col-span-3 text-right">{"Applied Transformation Rule"}</div>
              </div>
              <div class="flex flex-col gap-1 mt-1 max-h-64 overflow-y-auto pr-1">
                {for modified_diffs.iter().map(|item| {
                  let unit_suffix = if item.unit.is_empty() {
                    String::new()
                  } else {
                    format!(" {}", item.unit)
                  };
                  html! {
                    <div class="grid grid-cols-12 px-space-md py-1.5 items-center rounded bg-surface-container border-l-2 border-primary font-code-sm text-code-sm">
                      <div class="col-span-4 flex flex-col">
                        <span class="text-on-surface font-medium">{&item.label}</span>
                        <span class="text-on-surface-variant text-[10px] font-mono">{&item.key}</span>
                      </div>
                      <div class="col-span-2 text-right font-mono text-on-surface-variant line-through">
                        {format!("{}{}", item.base_value, unit_suffix)}
                      </div>
                      <div class="col-span-1 text-center font-mono text-primary font-bold">{"→"}</div>
                      <div class="col-span-2 text-left font-mono text-tertiary font-bold">
                        {format!("{}{}", item.override_value, unit_suffix)}
                      </div>
                      <div class="col-span-3 text-right text-xs font-mono text-secondary truncate" title={item.source_rule.clone()}>
                        {&item.source_rule}
                      </div>
                    </div>
                  }
                })}
              </div>
            </div>
          }
        }}
      </div>

      // Default Hardware & Filament Rules Stack Grid (with per-rule Before -> After badges)
      <div class="flex flex-col gap-space-md">
        <div class="flex items-center justify-between flex-wrap gap-space-sm">
          <div class="flex items-center gap-space-sm">
            <span class="material-symbols-outlined text-secondary">{"layers"}</span>
            <h2 class="font-headline-lg text-headline-sm text-on-surface font-semibold">
              {"Default Hardware, Process & Filament Transformation Rules (transformations/*.lua)"}
            </h2>
          </div>
          <div class="flex items-center gap-space-sm flex-wrap">
            <button
              class="flex items-center gap-1.5 bg-secondary text-on-secondary hover:opacity-90 px-space-md py-1.5 rounded-lg text-xs font-mono font-bold shadow transition-opacity cursor-pointer"
              onclick={{
                let src = show_rule_creator.clone();
                let cm = creator_mode.clone();
                let sci = selected_cal_idx.clone();
                let ca = cal_answers.clone();
                let itc = issue_tree_category.clone();
                let d_name = draft_name.clone();
                let d_desc = draft_desc.clone();
                let d_cat = draft_category.clone();
                let d_icon = draft_icon.clone();
                let d_cond = draft_condition.clone();
                let d_lua = draft_lua.clone();
                let cal_list_c = (*cal_templates).clone();
                Callback::from(move |_| {
                  if let Some((w_idx, w_tpl)) = cal_list_c
                    .iter()
                    .enumerate()
                    .find(|(_, t)| t.id == "sym_weather_climate")
                  {
                    let def_ans = w_tpl.default_answers();
                    let def_scope = w_tpl.default_condition_expr.clone();
                    let (t, d, _, l) = w_tpl.generate_from_answers(&def_ans, &def_scope);
                    sci.set(w_idx);
                    ca.set(def_ans);
                    itc.set(w_tpl.tree_category().to_string());
                    d_name.set(t);
                    d_desc.set(d);
                    d_cat.set(w_tpl.category.clone());
                    d_icon.set(w_tpl.icon.clone());
                    d_cond.set(def_scope);
                    d_lua.set(l);
                  }
                  cm.set("symptom_qa".to_string());
                  src.set(true);
                })
              }}
            >
              <span class="material-symbols-outlined text-[16px]">{"thermostat"}</span>
              {"🌦️ Weather & Climate Dials (Tropical / Cold / Humid)"}
            </button>
            <button
              class="flex items-center gap-1.5 bg-primary/20 text-primary border border-primary/50 hover:bg-primary/30 px-space-md py-1.5 rounded-lg text-xs font-mono font-bold shadow transition-colors cursor-pointer"
              onclick={{
                let src = show_rule_creator.clone();
                let cm = creator_mode.clone();
                let sci = selected_cal_idx.clone();
                let ca = cal_answers.clone();
                let d_name = draft_name.clone();
                let d_desc = draft_desc.clone();
                let d_cat = draft_category.clone();
                let d_icon = draft_icon.clone();
                let d_cond = draft_condition.clone();
                let d_lua = draft_lua.clone();
                let cal_list_c = (*cal_templates).clone();
                Callback::from(move |_| {
                  if let Some((f_idx, f_tpl)) = cal_list_c
                    .iter()
                    .enumerate()
                    .find(|(_, t)| t.id == "cal_tpu_foaming_shore")
                  {
                    let def_ans = f_tpl.default_answers();
                    let def_scope = f_tpl.default_condition_expr.clone();
                    let (t, d, _, l) = f_tpl.generate_from_answers(&def_ans, &def_scope);
                    sci.set(f_idx);
                    ca.set(def_ans);
                    d_name.set(t);
                    d_desc.set(d);
                    d_cat.set(f_tpl.category.clone());
                    d_icon.set(f_tpl.icon.clone());
                    d_cond.set(def_scope);
                    d_lua.set(l);
                  }
                  cm.set("calibration_qa".to_string());
                  src.set(true);
                })
              }}
            >
              <span>{"🧽"}</span>
              {"Foaming TPU Shore (60A–92A)"}
            </button>
            <button
              class="flex items-center gap-1.5 bg-secondary text-on-secondary hover:opacity-90 px-space-md py-1.5 rounded-lg text-xs font-mono font-bold shadow transition-opacity cursor-pointer"
              onclick={{
                let src = show_rule_creator.clone();
                let cm = creator_mode.clone();
                let sci = selected_cal_idx.clone();
                let ca = cal_answers.clone();
                let d_name = draft_name.clone();
                let d_desc = draft_desc.clone();
                let d_cat = draft_category.clone();
                let d_icon = draft_icon.clone();
                let d_cond = draft_condition.clone();
                let d_lua = draft_lua.clone();
                let cal_list_c = (*cal_templates).clone();
                Callback::from(move |_| {
                  if let Some((sym_idx, sym_tpl)) = cal_list_c
                    .iter()
                    .enumerate()
                    .find(|(_, t)| t.is_symptom_wizard())
                  {
                    let def_ans = sym_tpl.default_answers();
                    let def_scope = sym_tpl.default_condition_expr.clone();
                    let (t, d, _, l) = sym_tpl.generate_from_answers(&def_ans, &def_scope);
                    sci.set(sym_idx);
                    ca.set(def_ans);
                    d_name.set(t);
                    d_desc.set(d);
                    d_cat.set(sym_tpl.category.clone());
                    d_icon.set(sym_tpl.icon.clone());
                    d_cond.set(def_scope);
                    d_lua.set(l);
                  }
                  cm.set("symptom_qa".to_string());
                  src.set(true);
                })
              }}
            >
              <span class="material-symbols-outlined text-[16px]">{"healing"}</span>
              {"🩺 Fix Filament Symptoms Q&A (Stringing, Boogers, Clogs)"}
            </button>
            <button
              class="flex items-center gap-1.5 bg-tertiary text-on-tertiary hover:opacity-90 px-space-md py-1.5 rounded-lg text-xs font-mono font-bold shadow transition-opacity cursor-pointer"
              onclick={{
                let src = show_rule_creator.clone();
                let cm = creator_mode.clone();
                let sci = selected_cal_idx.clone();
                let ca = cal_answers.clone();
                let d_name = draft_name.clone();
                let d_desc = draft_desc.clone();
                let d_cat = draft_category.clone();
                let d_icon = draft_icon.clone();
                let d_cond = draft_condition.clone();
                let d_lua = draft_lua.clone();
                let cal_list_c = (*cal_templates).clone();
                Callback::from(move |_| {
                  if let Some((cal_idx, cal_tpl)) = cal_list_c
                    .iter()
                    .enumerate()
                    .find(|(_, t)| !t.is_symptom_wizard())
                  {
                    let def_ans = cal_tpl.default_answers();
                    let def_scope = cal_tpl.default_condition_expr.clone();
                    let (t, d, _, l) = cal_tpl.generate_from_answers(&def_ans, &def_scope);
                    sci.set(cal_idx);
                    ca.set(def_ans);
                    d_name.set(t);
                    d_desc.set(d);
                    d_cat.set(cal_tpl.category.clone());
                    d_icon.set(cal_tpl.icon.clone());
                    d_cond.set(def_scope);
                    d_lua.set(l);
                  }
                  cm.set("calibration_qa".to_string());
                  src.set(true);
                })
              }}
            >
              <span class="material-symbols-outlined text-[16px]">{"science"}</span>
              {"🎯 Post-Print Calibration Q&A (YOLO Flow, PA, Temp)"}
            </button>
            <button
              class="flex items-center gap-1.5 bg-primary text-on-primary hover:bg-primary/90 px-space-md py-1.5 rounded-lg text-xs font-mono font-bold shadow transition-colors cursor-pointer"
              onclick={
                let src = show_rule_creator.clone();
                Callback::from(move |_| src.set(!*src))
              }
            >
              <span class="material-symbols-outlined text-[16px]">
                {if *show_rule_creator { "expand_less" } else { "add_circle" }}
              </span>
              {if *show_rule_creator {
                "Close Rule Wizard"
              } else {
                "+ Create / Add New Rule (Search Settings or Pick Preset)"
              }}
            </button>
          </div>
        </div>

        // Interactive Rule Creator: Symptom Q&A + Calibration Print Q&A + Simple Visual Setting Search Wizard + Preset Library Drawer
        {if *show_rule_creator {
          let templates_list = (*templates).clone();
          let cur_idx = *selected_template_idx;
          let cal_list = (*cal_templates).clone();
          let cur_cal_idx = (*selected_cal_idx).min(cal_list.len().saturating_sub(1));
          let cur_cal = cal_list[cur_cal_idx].clone();
          let cur_mode = (*creator_mode).clone();
          let is_symptom_mode = cur_mode == "symptom_qa";
          let is_cal_mode = cur_mode == "calibration_qa" || is_symptom_mode;
          let is_visual_mode = cur_mode == "visual_wizard";
          let q_lower = search_query.to_lowercase();
          let cur_sec = (*section_filter).clone();
          let filtered_settings: Vec<WizardSettingDef> = setting_catalog
            .iter()
            .filter(|s| {
              let sec_ok = cur_sec == "All" || s.section == cur_sec;
              let q_ok = q_lower.is_empty()
                || s.label.to_lowercase().contains(&q_lower)
                || s.key.to_lowercase().contains(&q_lower)
                || s.section.to_lowercase().contains(&q_lower)
                || s.description.to_lowercase().contains(&q_lower);
              sec_ok && q_ok
            })
            .cloned()
            .collect();

          let sections = [
            ("All", "All Sections"),
            ("Thermal", "🔥 Thermal"),
            ("Walls & Seams", "🪡 Walls & Seams"),
            ("Extrusion & Retraction", "⚙️ Extrusion & Retraction"),
            ("Speed & Kinematics", "🚀 Speed & Kinematics"),
            ("Structure & Infill", "🧱 Structure & Infill"),
            ("Travel & Z-Hop", "✈️ Travel & Z-Hop"),
            ("Cooling & Adhesion", "🌀 Cooling & Adhesion"),
          ];

          html! {
            <div class="flex flex-col gap-space-md bg-surface-container-low p-space-xl rounded-xl border-2 border-primary shadow-xl">
              <div class="flex items-start justify-between gap-space-md flex-wrap">
                <div class="flex flex-col gap-1">
                  <div class="flex items-center gap-2">
                    <span class="bg-primary text-on-primary text-[10px] font-mono font-bold uppercase px-2 py-0.5 rounded">
                      {"Symptom Fixes Q&A • Calibration Prints Q&A • Setting Search • Presets"}
                    </span>
                    <span class="text-xs font-mono text-secondary">
                      {"Auto-Generates Sandboxed Lua Transformations"}
                    </span>
                  </div>
                  <h3 class="font-headline-sm text-headline-sm text-on-surface font-bold mt-0.5">
                    {if is_symptom_mode {
                      "Filament Symptom & Adjustment Assistant — Fix Stringing, Boogers, Clogging, Warping & Overhangs"
                    } else if is_cal_mode {
                      "Post-Print Calibration Assistant — Answer Questions From Your Calibration Print"
                    } else if is_visual_mode {
                      "Simple Setting Search Wizard — Add/Offset (+/-) or Set (=) Any Slicer Parameter"
                    } else {
                      "Curated Prepackaged Lua Rule Templates"
                    }}
                  </h3>
                  <p class="text-xs text-on-surface-variant">
                    {if is_symptom_mode {
                      "Pick the print symptom you want to fix (Stringing/Cobwebs, Nozzle Boogers/Blobs, Clogging/Heat Creep, Bed Warping, Overhang Curling, or Weak Layer Bonding), answer 3–4 simple questions, and add the generated Lua rule to your stack."
                    } else if is_cal_mode {
                      "Print any OrcaSlicer / AnycubicSlicerNext calibration test (YOLO Flow Rate, Pressure Advance, Temperature Tower, Retraction Tower, Max Volumetric Speed, or Dimensional Cube), answer the questions below, and add the computed Lua rule in one click."
                    } else {
                      "Filter by section, search any OrcaSlicer setting (e.g. Print Temperature +5°C, Wall Generator = arachne, Scarf Seam = all), and combine one or more setting tweaks into a single rule."
                    }}
                  </p>
                </div>

                // 4-Way Mode Toggle: Symptom Q&A vs Calibration Q&A vs Simple Visual Setting Wizard vs Curated Preset Templates
                <div class="flex items-center bg-surface-container-lowest p-1 rounded-xl border border-outline-variant font-mono text-xs flex-wrap">
                  <button
                    class={format!(
                      "px-space-md py-1.5 rounded-lg transition-colors cursor-pointer {}",
                      if is_symptom_mode {
                        "bg-secondary text-on-secondary font-bold shadow"
                      } else {
                        "text-on-surface-variant hover:text-on-surface"
                      }
                    )}
                    onclick={{
                      let cm = creator_mode.clone();
                      let sci = selected_cal_idx.clone();
                      let ca = cal_answers.clone();
                      let d_name = draft_name.clone();
                      let d_desc = draft_desc.clone();
                      let d_cat = draft_category.clone();
                      let d_icon = draft_icon.clone();
                      let d_cond = draft_condition.clone();
                      let d_lua = draft_lua.clone();
                      let cal_list_c = cal_list.clone();
                      Callback::from(move |_| {
                        if let Some((sym_idx, sym_tpl)) = cal_list_c
                          .iter()
                          .enumerate()
                          .find(|(_, t)| t.is_symptom_wizard())
                        {
                          let def_ans = sym_tpl.default_answers();
                          let def_scope = sym_tpl.default_condition_expr.clone();
                          let (t, d, _, l) = sym_tpl.generate_from_answers(&def_ans, &def_scope);
                          sci.set(sym_idx);
                          ca.set(def_ans);
                          d_name.set(t);
                          d_desc.set(d);
                          d_cat.set(sym_tpl.category.clone());
                          d_icon.set(sym_tpl.icon.clone());
                          d_cond.set(def_scope);
                          d_lua.set(l);
                        }
                        cm.set("symptom_qa".to_string());
                      })
                    }}
                  >
                    {"🩺 1. Fix Symptoms Q&A"}
                  </button>
                  <button
                    class={format!(
                      "px-space-md py-1.5 rounded-lg transition-colors cursor-pointer {}",
                      if cur_mode == "calibration_qa" {
                        "bg-tertiary text-on-tertiary font-bold shadow"
                      } else {
                        "text-on-surface-variant hover:text-on-surface"
                      }
                    )}
                    onclick={{
                      let cm = creator_mode.clone();
                      let sci = selected_cal_idx.clone();
                      let ca = cal_answers.clone();
                      let d_name = draft_name.clone();
                      let d_desc = draft_desc.clone();
                      let d_cat = draft_category.clone();
                      let d_icon = draft_icon.clone();
                      let d_cond = draft_condition.clone();
                      let d_lua = draft_lua.clone();
                      let cal_list_c = cal_list.clone();
                      Callback::from(move |_| {
                        if let Some((cal_idx, cal_tpl)) = cal_list_c
                          .iter()
                          .enumerate()
                          .find(|(_, t)| !t.is_symptom_wizard())
                        {
                          let def_ans = cal_tpl.default_answers();
                          let def_scope = cal_tpl.default_condition_expr.clone();
                          let (t, d, _, l) = cal_tpl.generate_from_answers(&def_ans, &def_scope);
                          sci.set(cal_idx);
                          ca.set(def_ans);
                          d_name.set(t);
                          d_desc.set(d);
                          d_cat.set(cal_tpl.category.clone());
                          d_icon.set(cal_tpl.icon.clone());
                          d_cond.set(def_scope);
                          d_lua.set(l);
                        }
                        cm.set("calibration_qa".to_string());
                      })
                    }}
                  >
                    {"🎯 2. Calibration Print Q&A"}
                  </button>
                  <button
                    class={format!(
                      "px-space-md py-1.5 rounded-lg transition-colors cursor-pointer {}",
                      if is_visual_mode {
                        "bg-primary text-on-primary font-bold shadow"
                      } else {
                        "text-on-surface-variant hover:text-on-surface"
                      }
                    )}
                    onclick={
                      let cm = creator_mode.clone();
                      Callback::from(move |_| cm.set("visual_wizard".to_string()))
                    }
                  >
                    {"🪄 3. Search Settings (+/- or =)"}
                  </button>
                  <button
                    class={format!(
                      "px-space-md py-1.5 rounded-lg transition-colors cursor-pointer {}",
                      if cur_mode == "preset_templates" {
                        "bg-primary text-on-primary font-bold shadow"
                      } else {
                        "text-on-surface-variant hover:text-on-surface"
                      }
                    )}
                    onclick={
                      let cm = creator_mode.clone();
                      Callback::from(move |_| cm.set("preset_templates".to_string()))
                    }
                  >
                    {"📚 4. Curated Presets"}
                  </button>
                  <button
                    class={format!(
                      "px-space-md py-1.5 rounded-lg transition-colors cursor-pointer {}",
                      if cur_mode == "custom_lua" {
                        "bg-primary text-on-primary font-bold shadow"
                      } else {
                        "text-on-surface-variant hover:text-on-surface"
                      }
                    )}
                    onclick={{
                      let cm = creator_mode.clone();
                      let idx_state = selected_template_idx.clone();
                      let d_name = draft_name.clone();
                      let d_desc = draft_desc.clone();
                      let d_cat = draft_category.clone();
                      let d_icon = draft_icon.clone();
                      let d_cond = draft_condition.clone();
                      let d_lua = draft_lua.clone();
                      let tpls_c = templates_list.clone();
                      Callback::from(move |_| {
                        if let Some((c_idx, c_tpl)) = tpls_c
                          .iter()
                          .enumerate()
                          .find(|(_, t)| t.id == "custom_blank_rule")
                        {
                          idx_state.set(c_idx);
                          d_name.set(c_tpl.name.clone());
                          d_desc.set(c_tpl.description.clone());
                          d_cat.set(c_tpl.category.clone());
                          d_icon.set(c_tpl.icon.clone());
                          d_cond.set(c_tpl.condition_expr.clone());
                          d_lua.set(c_tpl.lua_code.clone());
                        }
                        cm.set("custom_lua".to_string());
                      })
                    }}
                  >
                    {"⚙️ 5. Custom Parametric Lua"}
                  </button>
                </div>
              </div>

              {if is_cal_mode {
                let ans_map = (*cal_answers).clone();
                let cur_scope = (*draft_condition).clone();
                let (_, _, computed_badge, _) = cur_cal.generate_from_answers(&ans_map, &cur_scope);
                let active_tree_cat = (*issue_tree_category).clone();
                let iq_lower = issue_search_query.to_lowercase();
                let tree_cats = ParameterizedCalibrationTemplate::symptom_tree_categories();

                let matching_cals: Vec<(usize, ParameterizedCalibrationTemplate)> = cal_list
                  .iter()
                  .enumerate()
                  .filter(|(_, t)| {
                    if t.is_symptom_wizard() != is_symptom_mode {
                      return false;
                    }
                    if !is_symptom_mode {
                      return true;
                    }
                    let cat_ok = active_tree_cat == "all" || t.tree_category() == active_tree_cat;
                    let q_ok = iq_lower.is_empty()
                      || t.name.to_lowercase().contains(&iq_lower)
                      || t.tree_category().to_lowercase().contains(&iq_lower)
                      || t.tree_subcategory().to_lowercase().contains(&iq_lower)
                      || t.slicer_menu_path.to_lowercase().contains(&iq_lower)
                      || t.when_to_run.to_lowercase().contains(&iq_lower)
                      || t
                        .hardware_checklist()
                        .iter()
                        .any(|h| h.to_lowercase().contains(&iq_lower));
                    cat_ok && q_ok
                  })
                  .map(|(i, t)| (i, t.clone()))
                  .collect();

                let hw_items = cur_cal.hardware_checklist();

                html! {
                  <div class="flex flex-col gap-space-md">
                    {if is_symptom_mode {
                      html! {
                        <div class="flex flex-col gap-space-md">
                          // Search Bar & Reference Guide Links
                          <div class="flex flex-col md:flex-row items-stretch md:items-center justify-between gap-space-sm bg-surface-container-lowest p-space-md rounded-xl border border-outline-variant">
                            <div class="flex items-center flex-1 gap-2">
                              <span class="material-symbols-outlined text-secondary text-lg">{"manage_search"}</span>
                              <input
                                type="text"
                                placeholder="Search 26 print issues & hardware fixes (e.g. 'extruder tension', 'kobra screw', 'stringing', 'boogers', 'elephant', 'pillow', 'ghosting', 'bridge', 'wet')..."
                                class="w-full bg-transparent text-xs font-mono text-on-surface focus:outline-none"
                                value={(*issue_search_query).clone()}
                                oninput={{
                                  let isq = issue_search_query.clone();
                                  Callback::from(move |e: InputEvent| {
                                    if let Some(el) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                                      isq.set(el.value());
                                    }
                                  })
                                }}
                              />
                              {if !issue_search_query.is_empty() {
                                let isq = issue_search_query.clone();
                                html! {
                                  <button
                                    class="text-[11px] font-mono text-on-surface-variant hover:text-on-surface px-2 py-0.5 rounded bg-surface-container cursor-pointer"
                                    onclick={Callback::from(move |_| isq.set(String::new()))}
                                  >
                                    {"Clear ✕"}
                                  </button>
                                }
                              } else {
                                html! {}
                              }}
                            </div>
                            <div class="flex items-center gap-2 shrink-0 text-[11px] font-mono">
                              <span class="bg-secondary/20 text-secondary border border-secondary/40 px-2.5 py-1 rounded font-bold">
                                {format!("{} / 26 Issues Shown", matching_cals.len())}
                              </span>
                            </div>
                          </div>

                          // Two-Column Hierarchical Category Tree (Left) + Visual Defect Cards Grid (Right)
                          <div class="grid grid-cols-1 lg:grid-cols-12 gap-space-md">
                            // Left Column: Hierarchical Category → Sub-Category → Issue Tree
                            <div class="lg:col-span-4 bg-surface-container-lowest p-space-md rounded-xl border border-outline-variant flex flex-col gap-1.5 max-h-[540px] overflow-y-auto">
                              <div class="flex items-center justify-between pb-1 border-b border-outline-variant/50">
                                <span class="text-[11px] font-mono uppercase text-secondary font-bold">
                                  {"🗂️ Troubleshooting Category Tree"}
                                </span>
                                <span class="text-[10px] font-mono text-on-surface-variant">
                                  {"7 Categories • 26 Issues"}
                                </span>
                              </div>

                              {for tree_cats.iter().map(|(cat_id, cat_label, cat_icon)| {
                                let is_cat_active = active_tree_cat == *cat_id;
                                let cat_id_str = cat_id.to_string();
                                let itc = issue_tree_category.clone();

                                // Find all issues belonging to this category
                                let children_in_cat: Vec<(usize, ParameterizedCalibrationTemplate)> = if *cat_id == "all" {
                                  Vec::new()
                                } else {
                                  cal_list
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, t)| t.is_symptom_wizard() && t.tree_category() == *cat_id)
                                    .map(|(i, t)| (i, t.clone()))
                                    .collect()
                                };

                                let show_branch = *cat_id != "all" && (is_cat_active || active_tree_cat == "all" || !iq_lower.is_empty());

                                html! {
                                  <div class="flex flex-col gap-1">
                                    <button
                                      class={format!(
                                        "flex items-center justify-between w-full text-left px-2.5 py-1.5 rounded-lg text-xs font-mono transition-colors cursor-pointer {}",
                                        if is_cat_active {
                                          "bg-secondary text-on-secondary font-bold shadow-sm"
                                        } else {
                                          "bg-surface-container text-on-surface hover:bg-surface-container-high"
                                        }
                                      )}
                                      onclick={Callback::from(move |_| itc.set(cat_id_str.clone()))}
                                    >
                                      <div class="flex items-center gap-1.5 truncate">
                                        <span>{*cat_icon}</span>
                                        <span class="truncate">{*cat_label}</span>
                                      </div>
                                      {if *cat_id != "all" {
                                        html! {
                                          <span class={format!(
                                            "text-[10px] px-1.5 py-0.2 rounded {}",
                                            if is_cat_active {
                                              "bg-on-secondary/20 text-on-secondary"
                                            } else {
                                              "bg-surface-container-lowest text-on-surface-variant"
                                            }
                                          )}>
                                            {children_in_cat.len()}
                                          </span>
                                        }
                                      } else {
                                        html! {}
                                      }}
                                    </button>

                                    // Hierarchical Tree Leaves (Sub-Category -> Issue)
                                    {if show_branch {
                                      html! {
                                        <div class="flex flex-col gap-0.5 pl-3 ml-2 border-l-2 border-outline-variant/60">
                                          {for children_in_cat.into_iter().map(|(leaf_idx, leaf_tpl)| {
                                            let is_leaf_sel = leaf_idx == cur_cal_idx;
                                            let sci = selected_cal_idx.clone();
                                            let ca = cal_answers.clone();
                                            let d_name = draft_name.clone();
                                            let d_desc = draft_desc.clone();
                                            let d_cat = draft_category.clone();
                                            let d_icon = draft_icon.clone();
                                            let d_cond = draft_condition.clone();
                                            let d_lua = draft_lua.clone();
                                            let tpl_c = leaf_tpl.clone();

                                            html! {
                                              <button
                                                class={format!(
                                                  "flex items-center justify-between gap-1 text-left px-2 py-1 rounded text-[11px] font-mono transition-colors cursor-pointer {}",
                                                  if is_leaf_sel {
                                                    "bg-tertiary/20 text-tertiary border border-tertiary/50 font-bold"
                                                  } else {
                                                    "text-on-surface-variant hover:text-on-surface hover:bg-surface-container/60"
                                                  }
                                                )}
                                                onclick={Callback::from(move |_| {
                                                  let def_ans = tpl_c.default_answers();
                                                  let def_scope = tpl_c.default_condition_expr.clone();
                                                  let (t, d, _, l) = tpl_c.generate_from_answers(&def_ans, &def_scope);
                                                  sci.set(leaf_idx);
                                                  ca.set(def_ans);
                                                  d_name.set(t);
                                                  d_desc.set(d);
                                                  d_cat.set(tpl_c.category.clone());
                                                  d_icon.set(tpl_c.icon.clone());
                                                  d_cond.set(def_scope);
                                                  d_lua.set(l);
                                                })}
                                              >
                                                <span class="truncate">
                                                  {format!("{} {}", leaf_tpl.icon, leaf_tpl.name)}
                                                </span>
                                                <span class="text-[9px] text-secondary shrink-0">
                                                  {leaf_tpl.tree_subcategory()}
                                                </span>
                                              </button>
                                            }
                                          })}
                                        </div>
                                      }
                                    } else {
                                      html! {}
                                    }}
                                  </div>
                                }
                              })}
                            </div>

                            // Right Column: Visual Defect Cards Grid with Inline Illustrations
                            <div class="lg:col-span-8 flex flex-col gap-space-sm max-h-[540px] overflow-y-auto pr-1">
                              <div class="flex items-center justify-between">
                                <span class="text-[11px] font-mono uppercase text-tertiary font-bold">
                                  {"Step 1 — Visually Identify Your Print Quality Issue (Click Any Illustrated Card):"}
                                </span>
                                <span class="text-[10px] font-mono text-on-surface-variant">
                                  {"Includes Hardware Checks + Parameterized Lua Fix"}
                                </span>
                              </div>

                              <div class="grid grid-cols-1 md:grid-cols-2 gap-space-sm">
                                {for matching_cals.iter().map(|(c_idx, c_tpl)| {
                                  let is_sel_cal = *c_idx == cur_cal_idx;
                                  let idx_val = *c_idx;
                                  let sci = selected_cal_idx.clone();
                                  let ca = cal_answers.clone();
                                  let d_name = draft_name.clone();
                                  let d_desc = draft_desc.clone();
                                  let d_cat = draft_category.clone();
                                  let d_icon = draft_icon.clone();
                                  let d_cond = draft_condition.clone();
                                  let d_lua = draft_lua.clone();
                                  let tpl_c = c_tpl.clone();

                                  html! {
                                    <button
                                      class={format!(
                                        "flex flex-col items-start text-left p-space-sm rounded-xl border transition-all cursor-pointer gap-1.5 {}",
                                        if is_sel_cal {
                                          "bg-tertiary/15 border-2 border-tertiary text-on-surface shadow-lg"
                                        } else {
                                          "bg-surface-container-lowest border-outline-variant hover:border-tertiary/60 text-on-surface-variant"
                                        }
                                      )}
                                      onclick={Callback::from(move |_| {
                                        let def_ans = tpl_c.default_answers();
                                        let def_scope = tpl_c.default_condition_expr.clone();
                                        let (t, d, _, l) = tpl_c.generate_from_answers(&def_ans, &def_scope);
                                        sci.set(idx_val);
                                        ca.set(def_ans);
                                        d_name.set(t);
                                        d_desc.set(d);
                                        d_cat.set(tpl_c.category.clone());
                                        d_icon.set(tpl_c.icon.clone());
                                        d_cond.set(def_scope);
                                        d_lua.set(l);
                                      })}
                                    >
                                      // Visual Defect SVG Illustration Thumbnail
                                      {render_issue_illustration(&c_tpl.id, true)}

                                      <div class="flex items-center justify-between w-full gap-1 mt-0.5">
                                        <span class="text-[10px] font-mono px-1.5 py-0.5 rounded bg-surface-container-high text-secondary font-bold truncate">
                                          {c_tpl.tree_category()}
                                        </span>
                                        <span class="text-[10px] font-mono px-1.5 py-0.5 rounded bg-tertiary/15 text-tertiary font-bold shrink-0">
                                          {c_tpl.tree_subcategory()}
                                        </span>
                                      </div>

                                      <span class="text-xs font-bold text-on-surface leading-snug">
                                        {format!("{} {}", c_tpl.icon, c_tpl.name)}
                                      </span>

                                      <span class="text-[10px] text-on-surface-variant line-clamp-2">
                                        {&c_tpl.when_to_run}
                                      </span>
                                    </button>
                                  }
                                })}
                              </div>
                            </div>
                          </div>
                        </div>
                      }
                    } else {
                      // Post-Print Calibration Print Cards (with Visual Illustrations)
                      html! {
                        <div class="flex flex-col gap-1.5">
                          <span class="text-[11px] font-mono uppercase text-tertiary font-bold">
                            {"Step 1 — Select the Calibration Print You Completed (Recommended Order #1 → #7):"}
                          </span>
                          <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-space-sm">
                            {for matching_cals.iter().map(|(c_idx, c_tpl)| {
                              let is_sel_cal = *c_idx == cur_cal_idx;
                              let idx_val = *c_idx;
                              let sci = selected_cal_idx.clone();
                              let ca = cal_answers.clone();
                              let d_name = draft_name.clone();
                              let d_desc = draft_desc.clone();
                              let d_cat = draft_category.clone();
                              let d_icon = draft_icon.clone();
                              let d_cond = draft_condition.clone();
                              let d_lua = draft_lua.clone();
                              let tpl_c = c_tpl.clone();

                              html! {
                                <button
                                  class={format!(
                                    "flex flex-col items-start text-left p-space-sm rounded-xl border transition-all cursor-pointer gap-1.5 {}",
                                    if is_sel_cal {
                                      "bg-tertiary/15 border-2 border-tertiary text-on-surface shadow-md"
                                    } else {
                                      "bg-surface-container-lowest border-outline-variant hover:border-tertiary/50 text-on-surface-variant"
                                    }
                                  )}
                                  onclick={Callback::from(move |_| {
                                    let def_ans = tpl_c.default_answers();
                                    let def_scope = tpl_c.default_condition_expr.clone();
                                    let (t, d, _, l) = tpl_c.generate_from_answers(&def_ans, &def_scope);
                                    sci.set(idx_val);
                                    ca.set(def_ans);
                                    d_name.set(t);
                                    d_desc.set(d);
                                    d_cat.set(tpl_c.category.clone());
                                    d_icon.set(tpl_c.icon.clone());
                                    d_cond.set(def_scope);
                                    d_lua.set(l);
                                  })}
                                >
                                  {render_issue_illustration(&c_tpl.id, true)}
                                  <div class="flex items-center justify-between w-full">
                                    <span class="text-lg">{&c_tpl.icon}</span>
                                    <span class="text-[10px] font-mono px-1.5 py-0.5 rounded bg-surface-container-high text-tertiary font-bold">
                                      {&c_tpl.category}
                                    </span>
                                  </div>
                                  <span class="text-xs font-bold text-on-surface">
                                    {&c_tpl.name}
                                  </span>
                                  <span class="text-[10px] font-mono text-secondary truncate w-full">
                                    {&c_tpl.slicer_menu_path}
                                  </span>
                                </button>
                              }
                            })}
                          </div>
                        </div>
                      }
                    }}

                    // Step B: Visual Defect & Hardware Illustration + Physical Printer Guide + Slicer Diagnosis
                    <div class="bg-surface-container-lowest p-space-md rounded-xl border-2 border-tertiary/60 flex flex-col gap-space-md">
                      <div class="flex items-center justify-between flex-wrap gap-2">
                        <div class="flex items-center gap-2">
                          <span class="text-2xl">{&cur_cal.icon}</span>
                          <div class="flex flex-col">
                            <div class="flex items-center gap-2 flex-wrap">
                              <span class="text-sm font-bold text-on-surface">
                                {&cur_cal.name}
                              </span>
                              <span class="text-[10px] font-mono px-2 py-0.5 rounded bg-secondary/20 text-secondary font-bold">
                                {format!("{} → {}", cur_cal.tree_category(), cur_cal.tree_subcategory())}
                              </span>
                            </div>
                            <span class="text-[11px] font-mono text-secondary">
                              {format!("Slicer Controls: {}  •  {}", cur_cal.slicer_menu_path, cur_cal.when_to_run)}
                            </span>
                          </div>
                        </div>
                        <span class="bg-tertiary/20 text-tertiary border border-tertiary/50 px-3 py-1 rounded-lg font-mono text-xs font-bold">
                          {format!("Computed Lua Action: {}", computed_badge)}
                        </span>
                      </div>

                      // Full-Size Dual-Panel Visual Defect & Hardware Action Diagram
                      {render_issue_illustration(&cur_cal.id, false)}

                      // Physical / Mechanical Printer Actions Box (e.g. Kobra S1 Extruder Tension Screw, Cold Pull, PEI Dish Soap, Belt Tension)
                      {if !hw_items.is_empty() {
                        html! {
                          <div class="bg-secondary/10 border-l-4 border-secondary p-space-md rounded-r-xl flex flex-col gap-1.5">
                            <div class="flex items-center justify-between flex-wrap gap-1">
                              <span class="text-xs font-mono uppercase font-bold text-secondary flex items-center gap-1.5">
                                <span class="material-symbols-outlined text-[16px]">{"build"}</span>
                                {"Physical & Mechanical Printer Actions (Anycubic Kobra S1 / ACE Pro Hardware Check)"}
                              </span>
                              <span class="text-[10px] font-mono text-on-surface-variant">
                                {"Check these physical items on the printer alongside the Lua rule below"}
                              </span>
                            </div>
                            <div class="flex flex-col gap-1 text-xs text-on-surface">
                              {for hw_items.iter().map(|hw_line| {
                                html! {
                                  <span class="leading-relaxed">{*hw_line}</span>
                                }
                              })}
                            </div>
                          </div>
                        }
                      } else {
                        html! {}
                      }}

                      // Root Cause & How to Inspect the Print
                      <div class="flex flex-col gap-1 pl-3 border-l-2 border-tertiary/60 text-xs text-on-surface-variant">
                        <span class="text-[11px] font-mono uppercase text-tertiary font-bold">
                          {"Root Cause & How to Diagnose on Your Print:"}
                        </span>
                        {for cur_cal.how_to_read_print.iter().map(|step_line| {
                          html! {
                            <span>{step_line}</span>
                          }
                        })}
                      </div>

                      <div class="text-[11px] font-mono text-primary bg-surface-container px-2.5 py-1.5 rounded border border-outline-variant/50">
                        {format!("Slicer Rule Formula: {}", cur_cal.formula_summary)}
                      </div>
                    </div>

                    // Step C: Interactive Questions for This Issue / Calibration Print
                    <div class="grid grid-cols-1 md:grid-cols-2 gap-space-md">
                      {for cur_cal.questions.iter().map(|q| {
                        let q_id = q.id.clone();
                        let cur_val = ans_map
                          .get(&q.id)
                          .cloned()
                          .unwrap_or_else(|| q.default_value.clone());

                        let update_answer = {
                          let ca_state = cal_answers.clone();
                          let d_name = draft_name.clone();
                          let d_desc = draft_desc.clone();
                          let d_cat = draft_category.clone();
                          let d_icon = draft_icon.clone();
                          let d_lua = draft_lua.clone();
                          let scope_str = cur_scope.clone();
                          let cal_clone = cur_cal.clone();
                          let q_id_inner = q_id.clone();

                          move |new_v: String| {
                            let mut next_map = (*ca_state).clone();
                            cal_clone.apply_answer_change(&mut next_map, &q_id_inner, new_v);
                            let (t, d, _, l) = cal_clone.generate_from_answers(&next_map, &scope_str);
                            d_name.set(t);
                            d_desc.set(d);
                            d_cat.set(cal_clone.category.clone());
                            d_icon.set(cal_clone.icon.clone());
                            d_lua.set(l);
                            ca_state.set(next_map);
                          }
                        };

                        let ua_input = update_answer.clone();
                        let ua_slider = update_answer.clone();

                        let kind_badge = match &q.kind {
                          crate::lua_engine::LuaParamKind::Range { min, max, step } => {
                            let step_s = step.map(|s| format!(", step {}", s)).unwrap_or_default();
                            let unit_s = if q.unit.is_empty() { String::new() } else { format!(" {}", q.unit) };
                            format!("Range Dial: {} .. {}{}{}", min, max, unit_s, step_s)
                          }
                          crate::lua_engine::LuaParamKind::Enum => {
                            format!("Enum ({} options)", q.quick_options.len())
                          }
                          crate::lua_engine::LuaParamKind::Boolean => "Boolean".to_string(),
                          crate::lua_engine::LuaParamKind::Text => "Text".to_string(),
                        };

                        html! {
                          <div class="bg-surface-container p-space-md rounded-xl border border-outline-variant flex flex-col justify-between gap-2">
                            <div class="flex flex-col gap-0.5">
                              <div class="flex items-center justify-between gap-2 flex-wrap">
                                <span class="text-xs font-bold text-on-surface">
                                  {&q.question}
                                </span>
                                <span class="text-[10px] font-mono px-2 py-0.5 rounded bg-surface-container-highest text-secondary font-bold shrink-0">
                                  {kind_badge}
                                </span>
                              </div>
                              <span class="text-[11px] text-on-surface-variant">
                                {&q.hint}
                              </span>
                            </div>

                            {if let crate::lua_engine::LuaParamKind::Range { min, max, step } = &q.kind {
                              let step_val = step.unwrap_or(1.0);
                              let cur_num = cur_val.trim().parse::<f64>().unwrap_or(*min);
                              let dial_readout = if q.unit == "°C" {
                                let f_val = (cur_num * 1.8 + 32.0).round() as i64;
                                format!("{} °C ({}°F)", cur_val, f_val)
                              } else if q.unit.is_empty() {
                                cur_val.clone()
                              } else {
                                format!("{} {}", cur_val, q.unit)
                              };
                              html! {
                                <div class="flex flex-col gap-1 bg-surface-container-lowest p-2.5 rounded-lg border border-tertiary/40">
                                  <div class="flex items-center justify-between text-[11px] font-mono">
                                    <span class="text-on-surface-variant">{format!("{}{}", min, if q.unit.is_empty() { "" } else { &q.unit })}</span>
                                    <span class="bg-tertiary/20 text-tertiary border border-tertiary/40 px-2 py-0.5 rounded font-bold">
                                      {format!("🎛️ Dial: {}", dial_readout)}
                                    </span>
                                    <span class="text-on-surface-variant">{format!("{}{}", max, if q.unit.is_empty() { "" } else { &q.unit })}</span>
                                  </div>
                                  <input
                                    type="range"
                                    min={min.to_string()}
                                    max={max.to_string()}
                                    step={step_val.to_string()}
                                    value={cur_val.clone()}
                                    class="w-full accent-tertiary cursor-pointer"
                                    oninput={Callback::from(move |e: InputEvent| {
                                      if let Some(el) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                                        ua_slider(el.value());
                                      }
                                    })}
                                  />
                                </div>
                              }
                            } else {
                              html! {}
                            }}

                            <div class="flex flex-wrap gap-1.5 pt-1">
                              {for q.quick_options.iter().map(|(opt_val, opt_label)| {
                                let is_picked = cur_val == *opt_val;
                                let v_str = opt_val.clone();
                                let ua_btn = update_answer.clone();
                                html! {
                                  <button
                                    class={format!(
                                      "text-[11px] font-mono px-2.5 py-1 rounded-lg border transition-colors cursor-pointer {}",
                                      if is_picked {
                                        "bg-tertiary text-on-tertiary border-tertiary font-bold shadow-sm"
                                      } else {
                                        "bg-surface-container-lowest text-on-surface border-outline-variant hover:border-tertiary"
                                      }
                                    )}
                                    onclick={Callback::from(move |_| ua_btn(v_str.clone()))}
                                  >
                                    {opt_label}
                                  </button>
                                }
                              })}
                            </div>

                            <div class="flex items-center gap-2 pt-1 border-t border-outline-variant/40">
                              <span class="text-[10px] font-mono uppercase text-on-surface-variant font-bold">
                                {"Exact Reading / Custom Value:"}
                              </span>
                              <input
                                type="text"
                                class="w-28 bg-surface-container-lowest border border-outline-variant rounded px-2 py-1 text-xs font-mono text-on-surface focus:outline-none focus:border-tertiary"
                                value={cur_val}
                                oninput={Callback::from(move |e: InputEvent| {
                                  if let Some(el) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                                    ua_input(el.value());
                                  }
                                })}
                              />
                              if !q.unit.is_empty() {
                                <span class="text-[11px] font-mono text-tertiary">{&q.unit}</span>
                              }
                            </div>
                          </div>
                        }
                      })}
                    </div>
                  </div>
                }
              } else if is_visual_mode {
                html! {
                  <div class="flex flex-col gap-space-md">
                    // Search Bar + Section Filter Pills
                    <div class="flex flex-col gap-space-sm bg-surface-container-lowest p-space-md rounded-xl border border-outline-variant">
                      <div class="flex flex-col md:flex-row items-stretch md:items-center gap-space-sm">
                        <div class="relative flex-1 flex items-center">
                          <span class="material-symbols-outlined text-on-surface-variant text-[18px] ml-2">{"search"}</span>
                          <input
                            type="text"
                            placeholder="Search settings by name, key, or keyword (e.g. 'temp', 'wall', 'arachne', 'seam', 'retract', 'infill', 'z_hop', 'fan')..."
                            class="w-full bg-transparent px-2.5 py-1.5 text-xs font-mono text-on-surface focus:outline-none"
                            value={(*search_query).clone()}
                            oninput={
                              let sq = search_query.clone();
                              Callback::from(move |e: InputEvent| {
                                if let Some(el) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                                  sq.set(el.value());
                                }
                              })
                            }
                          />
                          {if !search_query.is_empty() {
                            let sq = search_query.clone();
                            html! {
                              <button
                                class="text-xs font-mono text-on-surface-variant hover:text-on-surface px-2 cursor-pointer"
                                onclick={Callback::from(move |_| sq.set(String::new()))}
                              >
                                {"Clear"}
                              </button>
                            }
                          } else {
                            html! {}
                          }}
                        </div>
                        <span class="text-[11px] font-mono text-tertiary shrink-0 px-2">
                          {format!("{} matching settings", filtered_settings.len())}
                        </span>
                      </div>

                      // Section Filter Pills
                      <div class="flex items-center gap-1.5 flex-wrap pt-1 border-t border-outline-variant/40">
                        <span class="text-[10px] font-mono uppercase text-on-surface-variant font-bold mr-1">
                          {"Section Filter:"}
                        </span>
                        {for sections.into_iter().map(|(sec_id, sec_label)| {
                          let is_active_sec = *section_filter == sec_id;
                          let sf = section_filter.clone();
                          let target_sec = sec_id.to_string();
                          html! {
                            <button
                              class={format!(
                                "text-[11px] font-mono px-2.5 py-1 rounded-lg border transition-colors cursor-pointer {}",
                                if is_active_sec {
                                  "bg-primary text-on-primary border-primary font-bold shadow-sm"
                                } else {
                                  "bg-surface-container text-on-surface-variant border-outline-variant hover:text-on-surface"
                                }
                              )}
                              onclick={Callback::from(move |_| sf.set(target_sec.clone()))}
                            >
                              {sec_label}
                            </button>
                          }
                        })}
                      </div>
                    </div>

                    // Matching Settings Grid (Click to Add/Offset or Set Values)
                    <div class="grid grid-cols-1 md:grid-cols-2 gap-space-sm max-h-96 overflow-y-auto pr-1">
                      {for filtered_settings.iter().map(|s_def| {
                        let s_key = s_def.key.clone();
                        let is_focused = *selected_setting_key == s_def.key;
                        let existing_staged = staged_actions.iter().find(|a| a.key == s_def.key).cloned();
                        let unit_badge = if s_def.unit.is_empty() || s_def.unit == "flag" {
                          String::new()
                        } else {
                          format!(" {}", s_def.unit)
                        };

                        // Helper closure to stage/update an action and refresh draft_name, draft_desc, and draft_lua
                        let stage_helper = {
                          let sa_state = staged_actions.clone();
                          let d_name = draft_name.clone();
                          let d_desc = draft_desc.clone();
                          let d_cat = draft_category.clone();
                          let d_icon = draft_icon.clone();
                          let d_lua = draft_lua.clone();
                          let s_clone = s_def.clone();

                          move |op: WizardActionOp, operand: String| {
                            let mut list = (*sa_state).clone();
                            let act = WizardRuleAction {
                              key: s_clone.key.clone(),
                              label: s_clone.label.clone(),
                              section: s_clone.section.clone(),
                              unit: s_clone.unit.clone(),
                              is_array_in_orca: s_clone.is_array_in_orca,
                              decimals: s_clone.decimals,
                              default_base_value: s_clone.default_base_value.clone(),
                              op,
                              operand,
                            };
                            if let Some(existing) =
                              list.iter_mut().find(|x| x.key == s_clone.key && x.op == op)
                            {
                              *existing = act;
                            } else {
                              list.push(act);
                            }
                            let summaries: Vec<String> = list.iter().map(|a| a.human_summary()).collect();
                            let auto_title = format!("Wizard: {}", summaries.join(" & "));
                            let auto_desc = format!(
                              "Visual Rule Wizard transformation: {}.",
                              summaries.join("; ")
                            );
                            let auto_lua = WizardRuleAction::generate_lua_script(&list, &auto_title);
                            d_name.set(auto_title);
                            d_desc.set(auto_desc);
                            d_cat.set(format!("{} Rule", s_clone.section));
                            d_icon.set("🪄".to_string());
                            d_lua.set(auto_lua);
                            sa_state.set(list);
                          }
                        };

                        html! {
                          <div
                            class={format!(
                              "flex flex-col justify-between gap-2 p-space-md rounded-xl border transition-all {}",
                              if existing_staged.is_some() {
                                "bg-primary/10 border-primary shadow"
                              } else if is_focused {
                                "bg-surface-container border-secondary"
                              } else {
                                "bg-surface-container-lowest border-outline-variant hover:border-primary/50"
                              }
                            )}
                          >
                            <div class="flex items-start justify-between gap-2">
                              <div class="flex flex-col">
                                <div class="flex items-center gap-1.5 flex-wrap">
                                  <span class="text-[10px] font-mono uppercase px-1.5 py-0.5 rounded bg-surface-container-high text-secondary">
                                    {&s_def.section}
                                  </span>
                                  <span class="text-xs font-bold text-on-surface">
                                    {&s_def.label}
                                  </span>
                                  <span class="text-[10px] font-mono text-on-surface-variant">
                                    {format!("({})", s_def.key)}
                                  </span>
                                </div>
                                <span class="text-[11px] text-on-surface-variant mt-0.5">
                                  {&s_def.description}
                                </span>
                              </div>
                              <span class="text-[10px] font-mono px-2 py-0.5 rounded bg-surface-container text-on-surface-variant shrink-0">
                                {format!("Base: {}{}", s_def.default_base_value, unit_badge)}
                              </span>
                            </div>

                            // Action Controls Depending on Setting Kind (Numeric Add/Set/Scale vs Enum Choices vs Boolean Flag vs Percentage)
                            {match &s_def.kind {
                              SettingValueKind::EnumChoices { options, .. } => {
                                html! {
                                  <div class="flex items-center justify-between gap-2 flex-wrap pt-1 border-t border-outline-variant/40">
                                    <span class="text-[10px] font-mono uppercase text-primary font-bold">
                                      {"Set Value (=):"}
                                    </span>
                                    <div class="flex items-center gap-1 flex-wrap">
                                      {for options.iter().map(|opt| {
                                        let is_picked = existing_staged
                                          .as_ref()
                                          .map(|a| a.operand == *opt)
                                          .unwrap_or(false);
                                        let opt_val = opt.clone();
                                        let sh = stage_helper.clone();
                                        html! {
                                          <button
                                            class={format!(
                                              "text-[11px] font-mono px-2 py-0.5 rounded border transition-colors cursor-pointer {}",
                                              if is_picked {
                                                "bg-primary text-on-primary border-primary font-bold"
                                              } else {
                                                "bg-surface-container text-on-surface hover:border-primary border-outline-variant"
                                              }
                                            )}
                                            onclick={Callback::from(move |_| {
                                              sh(WizardActionOp::SetValue, opt_val.clone());
                                            })}
                                          >
                                            {format!("= {}", opt)}
                                          </button>
                                        }
                                      })}
                                    </div>
                                  </div>
                                }
                              }
                              SettingValueKind::BooleanFlag { .. } => {
                                let is_enabled_staged = existing_staged
                                  .as_ref()
                                  .map(|a| a.operand == "1")
                                  .unwrap_or(false);
                                let is_disabled_staged = existing_staged
                                  .as_ref()
                                  .map(|a| a.operand == "0")
                                  .unwrap_or(false);
                                let sh_on = stage_helper.clone();
                                let sh_off = stage_helper.clone();

                                html! {
                                  <div class="flex items-center justify-between gap-2 pt-1 border-t border-outline-variant/40">
                                    <span class="text-[10px] font-mono uppercase text-primary font-bold">
                                      {"Toggle Flag (=):"}
                                    </span>
                                    <div class="flex items-center gap-1.5">
                                      <button
                                        class={format!(
                                          "text-[11px] font-mono px-2.5 py-0.5 rounded border cursor-pointer {}",
                                          if is_enabled_staged {
                                            "bg-primary text-on-primary border-primary font-bold"
                                          } else {
                                            "bg-surface-container text-on-surface border-outline-variant hover:border-primary"
                                          }
                                        )}
                                        onclick={Callback::from(move |_| sh_on(WizardActionOp::SetValue, "1".to_string()))}
                                      >
                                        {"= Enable (1)"}
                                      </button>
                                      <button
                                        class={format!(
                                          "text-[11px] font-mono px-2.5 py-0.5 rounded border cursor-pointer {}",
                                          if is_disabled_staged {
                                            "bg-primary text-on-primary border-primary font-bold"
                                          } else {
                                            "bg-surface-container text-on-surface border-outline-variant hover:border-primary"
                                          }
                                        )}
                                        onclick={Callback::from(move |_| sh_off(WizardActionOp::SetValue, "0".to_string()))}
                                      >
                                        {"= Disable (0)"}
                                      </button>
                                    </div>
                                  </div>
                                }
                              }
                              SettingValueKind::Percentage { presets, suggested } => {
                                let s_key_for_focus = s_key.clone();
                                let ssk = selected_setting_key.clone();
                                let sop = selected_op.clone();
                                let cop = custom_operand.clone();
                                let sug_val = suggested.clone();

                                html! {
                                  <div class="flex flex-col gap-1.5 pt-1 border-t border-outline-variant/40">
                                    <div class="flex items-center justify-between gap-2 flex-wrap">
                                      <span class="text-[10px] font-mono uppercase text-primary font-bold">
                                        {"Set Percentage (=):"}
                                      </span>
                                      <div class="flex items-center gap-1 flex-wrap">
                                        {for presets.iter().map(|pct| {
                                          let is_picked = existing_staged
                                            .as_ref()
                                            .map(|a| a.operand == *pct)
                                            .unwrap_or(false);
                                          let pct_val = pct.clone();
                                          let sh = stage_helper.clone();
                                          html! {
                                            <button
                                              class={format!(
                                                "text-[11px] font-mono px-2 py-0.5 rounded border cursor-pointer {}",
                                                if is_picked {
                                                  "bg-primary text-on-primary border-primary font-bold"
                                                } else {
                                                  "bg-surface-container text-on-surface border-outline-variant hover:border-primary"
                                                }
                                              )}
                                              onclick={Callback::from(move |_| {
                                                sh(WizardActionOp::SetValue, pct_val.clone());
                                              })}
                                            >
                                              {format!("= {}", pct)}
                                            </button>
                                          }
                                        })}
                                        <button
                                          class="text-[10px] font-mono text-secondary hover:underline px-1 cursor-pointer"
                                          onclick={Callback::from(move |_| {
                                            ssk.set(s_key_for_focus.clone());
                                            sop.set(WizardActionOp::SetValue);
                                            cop.set(sug_val.clone());
                                          })}
                                        >
                                          {"Custom…"}
                                        </button>
                                      </div>
                                    </div>
                                  </div>
                                }
                              }
                              SettingValueKind::Numeric { default_op, presets, suggested } => {
                                let s_key_for_focus = s_key.clone();
                                let ssk = selected_setting_key.clone();
                                let sop = selected_op.clone();
                                let cop = custom_operand.clone();
                                let def_op = *default_op;
                                let sug_val = suggested.clone();

                                html! {
                                  <div class="flex flex-col gap-1.5 pt-1 border-t border-outline-variant/40">
                                    <div class="flex items-center justify-between gap-2 flex-wrap">
                                      <span class="text-[10px] font-mono uppercase text-primary font-bold">
                                        {if def_op == WizardActionOp::AddOffset {
                                          "Add / Offset (+/-) or Set (=):"
                                        } else {
                                          "Quick Presets (= or +/-):"
                                        }}
                                      </span>
                                      <div class="flex items-center gap-1 flex-wrap">
                                        {for presets.iter().map(|pre| {
                                          let inferred_op = if pre.starts_with('+') || pre.starts_with('-') {
                                            WizardActionOp::AddOffset
                                          } else if pre.starts_with('x') || pre.starts_with('×') {
                                            WizardActionOp::MultiplyScale
                                          } else {
                                            WizardActionOp::SetValue
                                          };
                                          let is_picked = existing_staged
                                            .as_ref()
                                            .map(|a| a.op == inferred_op && a.operand == *pre)
                                            .unwrap_or(false);
                                          let pre_val = pre.clone();
                                          let sh = stage_helper.clone();
                                          let badge_lbl = if inferred_op == WizardActionOp::SetValue {
                                            format!("= {}{}", pre, unit_badge)
                                          } else {
                                            format!("{}{}", pre, unit_badge)
                                          };

                                          html! {
                                            <button
                                              class={format!(
                                                "text-[11px] font-mono px-2 py-0.5 rounded border cursor-pointer {}",
                                                if is_picked {
                                                  "bg-primary text-on-primary border-primary font-bold"
                                                } else {
                                                  "bg-surface-container text-on-surface border-outline-variant hover:border-primary"
                                                }
                                              )}
                                              onclick={Callback::from(move |_| {
                                                sh(inferred_op, pre_val.clone());
                                              })}
                                            >
                                              {badge_lbl}
                                            </button>
                                          }
                                        })}
                                        <button
                                          class="text-[10px] font-mono text-secondary hover:underline px-1 cursor-pointer"
                                          onclick={Callback::from(move |_| {
                                            ssk.set(s_key_for_focus.clone());
                                            sop.set(def_op);
                                            cop.set(sug_val.clone());
                                          })}
                                        >
                                          {"Custom +/- or =…"}
                                        </button>
                                      </div>
                                    </div>

                                    // Inline Custom Operation & Value Bar when focused
                                    {if is_focused {
                                      let cur_op = *selected_op;
                                      let sop1 = selected_op.clone();
                                      let sop2 = selected_op.clone();
                                      let sop3 = selected_op.clone();
                                      let cop_in = custom_operand.clone();
                                      let cop_val = (*custom_operand).clone();
                                      let sh_custom = stage_helper.clone();

                                      html! {
                                        <div class="flex items-center gap-1.5 flex-wrap bg-surface-container-lowest p-1.5 rounded-lg border border-secondary/60 mt-1">
                                          <button
                                            class={format!(
                                              "text-[10px] font-mono px-2 py-1 rounded cursor-pointer {}",
                                              if cur_op == WizardActionOp::AddOffset {
                                                "bg-secondary text-on-secondary font-bold"
                                              } else {
                                                "bg-surface-container text-on-surface-variant"
                                              }
                                            )}
                                            onclick={Callback::from(move |_| sop1.set(WizardActionOp::AddOffset))}
                                          >
                                            {"+ / - Add Offset"}
                                          </button>
                                          <button
                                            class={format!(
                                              "text-[10px] font-mono px-2 py-1 rounded cursor-pointer {}",
                                              if cur_op == WizardActionOp::SetValue {
                                                "bg-secondary text-on-secondary font-bold"
                                              } else {
                                                "bg-surface-container text-on-surface-variant"
                                              }
                                            )}
                                            onclick={Callback::from(move |_| sop2.set(WizardActionOp::SetValue))}
                                          >
                                            {"= Set Value"}
                                          </button>
                                          <button
                                            class={format!(
                                              "text-[10px] font-mono px-2 py-1 rounded cursor-pointer {}",
                                              if cur_op == WizardActionOp::MultiplyScale {
                                                "bg-secondary text-on-secondary font-bold"
                                              } else {
                                                "bg-surface-container text-on-surface-variant"
                                              }
                                            )}
                                            onclick={Callback::from(move |_| sop3.set(WizardActionOp::MultiplyScale))}
                                          >
                                            {"× Scale"}
                                          </button>
                                          <input
                                            type="text"
                                            class="w-24 bg-surface-container border border-outline-variant rounded px-2 py-0.5 text-xs font-mono text-on-surface focus:outline-none focus:border-primary"
                                            value={cop_val.clone()}
                                            oninput={Callback::from(move |e: InputEvent| {
                                              if let Some(el) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                                                cop_in.set(el.value());
                                              }
                                            })}
                                          />
                                          <button
                                            class="bg-primary text-on-primary text-[11px] font-mono font-bold px-2.5 py-0.5 rounded cursor-pointer"
                                            onclick={Callback::from(move |_| {
                                              sh_custom(cur_op, cop_val.clone());
                                            })}
                                          >
                                            {"Stage"}
                                          </button>
                                        </div>
                                      }
                                    } else {
                                      html! {}
                                    }}
                                  </div>
                                }
                              }
                            }}
                          </div>
                        }
                      })}
                    </div>

                    // Staged Setting Actions Summary Bar (with Intra-Rule Execution Order Controls)
                    <div class="flex flex-col gap-1.5 bg-surface-container-lowest p-space-md rounded-xl border border-primary/50">
                      <div class="flex items-center justify-between flex-wrap gap-2">
                        <span class="text-[11px] font-mono uppercase text-primary font-bold">
                          {format!("Staged Setting Modifications in This Rule ({} — Evaluated Sequentially 1 → N):", staged_actions.len())}
                        </span>
                        {if !staged_actions.is_empty() {
                          let sa_clear = staged_actions.clone();
                          html! {
                            <button
                              class="text-[10px] font-mono text-error hover:underline cursor-pointer"
                              onclick={Callback::from(move |_| sa_clear.set(Vec::new()))}
                            >
                              {"Clear All Staged Settings"}
                            </button>
                          }
                        } else {
                          html! {}
                        }}
                      </div>
                      {if staged_actions.is_empty() {
                        html! {
                          <span class="text-xs font-mono text-on-surface-variant">
                            {"Click any setting option above (e.g. '+5 °C' on Print Temperature, '× 0.85' on Max Volumetric Speed, '= arachne' on Wall Generator) to stage it. You can even combine non-commutative operations (e.g. Scale × then Offset +/-) and reorder them!"}
                          </span>
                        }
                      } else {
                        let total_staged = staged_actions.len();
                        html! {
                          <div class="flex flex-wrap items-center gap-1.5 mt-1">
                            {for staged_actions.iter().enumerate().map(|(act_idx, act)| {
                              let sa_up = staged_actions.clone();
                              let sa_down = staged_actions.clone();
                              let sa_rm = staged_actions.clone();
                              let d_name_up = draft_name.clone();
                              let d_desc_up = draft_desc.clone();
                              let d_lua_up = draft_lua.clone();
                              let d_name_dn = draft_name.clone();
                              let d_desc_dn = draft_desc.clone();
                              let d_lua_dn = draft_lua.clone();
                              let d_name_rm = draft_name.clone();
                              let d_desc_rm = draft_desc.clone();
                              let d_lua_rm = draft_lua.clone();

                              html! {
                                <div class="flex items-center gap-1.5 bg-primary/15 border border-primary text-on-surface px-2.5 py-1 rounded-lg text-xs font-mono">
                                  <span class="bg-primary text-on-primary text-[10px] font-bold px-1.5 py-0.5 rounded">
                                    {format!("#{}", act_idx + 1)}
                                  </span>
                                  <span class="font-bold text-primary">{act.op.short_symbol()}</span>
                                  <span>{act.human_summary()}</span>
                                  {if total_staged > 1 {
                                    html! {
                                      <div class="flex items-center gap-0.5 ml-1">
                                        <button
                                          class="text-[10px] px-1 py-0.5 rounded bg-surface-container hover:bg-surface-bright text-on-surface cursor-pointer disabled:opacity-40"
                                          disabled={act_idx == 0}
                                          title="Move action earlier in rule"
                                          onclick={Callback::from(move |_| {
                                            if act_idx > 0 {
                                              let mut list = (*sa_up).clone();
                                              list.swap(act_idx, act_idx - 1);
                                              let summaries: Vec<String> = list.iter().map(|a| a.human_summary()).collect();
                                              let auto_title = format!("Wizard: {}", summaries.join(" ➔ "));
                                              let auto_lua = WizardRuleAction::generate_lua_script(&list, &auto_title);
                                              d_name_up.set(auto_title);
                                              d_desc_up.set(format!("Visual Rule Wizard transformation (ordered): {}.", summaries.join(" ➔ ")));
                                              d_lua_up.set(auto_lua);
                                              sa_up.set(list);
                                            }
                                          })}
                                        >
                                          {"↑"}
                                        </button>
                                        <button
                                          class="text-[10px] px-1 py-0.5 rounded bg-surface-container hover:bg-surface-bright text-on-surface cursor-pointer disabled:opacity-40"
                                          disabled={act_idx + 1 >= total_staged}
                                          title="Move action later in rule"
                                          onclick={Callback::from(move |_| {
                                            if act_idx + 1 < total_staged {
                                              let mut list = (*sa_down).clone();
                                              list.swap(act_idx, act_idx + 1);
                                              let summaries: Vec<String> = list.iter().map(|a| a.human_summary()).collect();
                                              let auto_title = format!("Wizard: {}", summaries.join(" ➔ "));
                                              let auto_lua = WizardRuleAction::generate_lua_script(&list, &auto_title);
                                              d_name_dn.set(auto_title);
                                              d_desc_dn.set(format!("Visual Rule Wizard transformation (ordered): {}.", summaries.join(" ➔ ")));
                                              d_lua_dn.set(auto_lua);
                                              sa_down.set(list);
                                            }
                                          })}
                                        >
                                          {"↓"}
                                        </button>
                                      </div>
                                    }
                                  } else {
                                    html! {}
                                  }}
                                  <button
                                    class="text-on-surface-variant hover:text-error ml-1 cursor-pointer font-bold"
                                    onclick={Callback::from(move |_| {
                                      let mut list = (*sa_rm).clone();
                                      if act_idx < list.len() {
                                        list.remove(act_idx);
                                      }
                                      let summaries: Vec<String> = list.iter().map(|a| a.human_summary()).collect();
                                      let auto_title = if summaries.is_empty() {
                                        "Custom Wizard Rule".to_string()
                                      } else {
                                        format!("Wizard: {}", summaries.join(" & "))
                                      };
                                      let auto_lua = WizardRuleAction::generate_lua_script(&list, &auto_title);
                                      d_name_rm.set(auto_title);
                                      d_desc_rm.set(format!("Visual Rule Wizard transformation: {}.", summaries.join("; ")));
                                      d_lua_rm.set(auto_lua);
                                      sa_rm.set(list);
                                    })}
                                  >
                                    {"✕"}
                                  </button>
                                </div>
                              }
                            })}
                          </div>
                        }
                      }}
                    </div>
                  </div>
                }
              } else if cur_mode == "custom_lua" {
                // Custom Parametric Lua Rule Bootstrap Mode
                html! {
                  <div class="flex flex-col gap-space-sm bg-surface-container-lowest p-space-md rounded-xl border border-primary/50">
                    <div class="flex items-center justify-between flex-wrap gap-2">
                      <div class="flex items-center gap-2">
                        <span class="text-lg">{"⚙️"}</span>
                        <span class="text-xs font-mono uppercase text-primary font-bold">
                          {"Pre-Populated Parametric Lua Rule Bootstrap (params = { ... }, validate(p, ctx), describe(p, ctx), apply(config, ctx, p))"}
                        </span>
                      </div>
                      <button
                        class="text-[11px] font-mono bg-surface-container hover:bg-surface-container-high text-secondary border border-secondary/40 px-2.5 py-1 rounded-lg cursor-pointer"
                        onclick={{
                          let d_name = draft_name.clone();
                          let d_desc = draft_desc.clone();
                          let d_cat = draft_category.clone();
                          let d_icon = draft_icon.clone();
                          let d_cond = draft_condition.clone();
                          let d_lua = draft_lua.clone();
                          let tpls_c = templates_list.clone();
                          Callback::from(move |_| {
                            if let Some(c_tpl) = tpls_c.iter().find(|t| t.id == "custom_blank_rule") {
                              d_name.set(c_tpl.name.clone());
                              d_desc.set(c_tpl.description.clone());
                              d_cat.set(c_tpl.category.clone());
                              d_icon.set(c_tpl.icon.clone());
                              d_cond.set(c_tpl.condition_expr.clone());
                              d_lua.set(c_tpl.lua_code.clone());
                            }
                          })
                        }}
                      >
                        {"↺ Reset to Pre-Populated Bootstrap Template"}
                      </button>
                    </div>
                    <p class="text-xs text-on-surface-variant font-mono">
                      {"The script below is pre-populated with 4 example parameters (enum, temperature range, speed multiplier range, and boolean flag), a Lua cross-parameter validate(p, ctx) guard, a dynamic describe(p, ctx) helper, and apply(config, ctx, p). Edit or add entries in params = { ... } below and the parameter cards + validation checks will update live!"}
                    </p>
                  </div>
                }
              } else {
                // Curated Preset Templates Mode
                html! {
                  <div class="flex flex-col gap-1.5">
                    <span class="text-[11px] font-mono uppercase text-on-surface-variant font-bold">
                      {"1. Choose a Curated Rule Template Preset:"}
                    </span>
                    <div class="grid grid-cols-1 md:grid-cols-3 lg:grid-cols-5 gap-space-sm">
                      {for templates_list.iter().enumerate().map(|(idx, tpl)| {
                        let is_chosen = idx == cur_idx;
                        let idx_state = selected_template_idx.clone();
                        let d_name = draft_name.clone();
                        let d_desc = draft_desc.clone();
                        let d_cat = draft_category.clone();
                        let d_icon = draft_icon.clone();
                        let d_cond = draft_condition.clone();
                        let d_lua = draft_lua.clone();
                        let tpl_clone = tpl.clone();

                        html! {
                          <button
                            class={format!(
                              "flex flex-col items-start text-left p-space-sm rounded-lg border transition-all cursor-pointer {}",
                              if is_chosen {
                                "bg-primary/15 border-primary text-on-surface shadow"
                              } else {
                                "bg-surface-container-lowest border-outline-variant hover:border-primary/50 text-on-surface-variant"
                              }
                            )}
                            onclick={Callback::from(move |_| {
                              idx_state.set(idx);
                              d_name.set(tpl_clone.name.clone());
                              d_desc.set(tpl_clone.description.clone());
                              d_cat.set(tpl_clone.category.clone());
                              d_icon.set(tpl_clone.icon.clone());
                              d_cond.set(tpl_clone.condition_expr.clone());
                              d_lua.set(tpl_clone.lua_code.clone());
                            })}
                          >
                            <div class="flex items-center justify-between w-full">
                              <span class="text-lg">{&tpl.icon}</span>
                              <span class="text-[10px] font-mono px-1.5 py-0.5 rounded bg-surface-container-high text-secondary">
                                {format!("Scope: {}", tpl.condition_expr)}
                              </span>
                            </div>
                            <span class="text-xs font-bold text-on-surface mt-1 line-clamp-1">
                              {&tpl.name}
                            </span>
                            <span class="text-[10px] text-on-surface-variant line-clamp-2 mt-0.5">
                              {&tpl.description}
                            </span>
                          </button>
                        }
                      })}
                    </div>
                  </div>
                }
              }}

              // Step 2: Rule Name, Condition Scope, and Description
              <div class="grid grid-cols-1 md:grid-cols-3 gap-space-md">
                <div class="flex flex-col gap-1">
                  <label class="text-[11px] font-mono uppercase text-on-surface-variant font-bold">
                    {"2. Rule Title"}
                  </label>
                  <input
                    type="text"
                    class="bg-surface-container-lowest border border-outline-variant rounded-lg px-3 py-2 text-xs font-mono text-on-surface focus:outline-none focus:border-primary"
                    value={(*draft_name).clone()}
                    oninput={
                      let d_name = draft_name.clone();
                      Callback::from(move |e: InputEvent| {
                        if let Some(el) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                          d_name.set(el.value());
                        }
                      })
                    }
                  />
                </div>

                <div class="flex flex-col gap-1">
                  <label class="text-[11px] font-mono uppercase text-on-surface-variant font-bold">
                    {"3. Condition / Target Scope"}
                  </label>
                  <input
                    type="text"
                    class="bg-surface-container-lowest border border-outline-variant rounded-lg px-3 py-2 text-xs font-mono text-on-surface focus:outline-none focus:border-primary"
                    value={(*draft_condition).clone()}
                    oninput={
                      let d_cond = draft_condition.clone();
                      Callback::from(move |e: InputEvent| {
                        if let Some(el) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                          d_cond.set(el.value());
                        }
                      })
                    }
                  />
                  <div class="flex flex-wrap gap-1 mt-1">
                    {for [
                      "all",
                      "filament:PCTG",
                      "filament:PETG",
                      "filament:PLA",
                      "filament:PLA-CF",
                      "nozzle:hardened_steel",
                      "nozzle:diamondback",
                    ].into_iter().map(|scope_opt| {
                      let is_cur = *draft_condition == scope_opt;
                      let d_cond = draft_condition.clone();
                      let s_val = scope_opt.to_string();
                      html! {
                        <button
                          class={format!(
                            "text-[10px] font-mono px-1.5 py-0.5 rounded border cursor-pointer {}",
                            if is_cur {
                              "bg-primary text-on-primary border-primary font-bold"
                            } else {
                              "bg-surface-container-lowest text-on-surface-variant border-outline-variant hover:border-primary/50"
                            }
                          )}
                          onclick={Callback::from(move |_| d_cond.set(s_val.clone()))}
                        >
                          {scope_opt}
                        </button>
                      }
                    })}
                  </div>
                </div>

                <div class="flex flex-col gap-1">
                  <label class="text-[11px] font-mono uppercase text-on-surface-variant font-bold">
                    {"4. Description"}
                  </label>
                  <input
                    type="text"
                    class="bg-surface-container-lowest border border-outline-variant rounded-lg px-3 py-2 text-xs font-mono text-on-surface focus:outline-none focus:border-primary"
                    value={(*draft_desc).clone()}
                    oninput={
                      let d_desc = draft_desc.clone();
                      Callback::from(move |e: InputEvent| {
                        if let Some(el) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                          d_desc.set(el.value());
                        }
                      })
                    }
                  />
                </div>
              </div>

              // Step 3: Lua Code Editor & Generalized Lua Parameter Validation
              {{
                let draft_ctx = serde_json::json!({ "scope": (*draft_condition).clone() });
                let draft_spec_res = crate::lua_engine::extract_lua_rule_spec(&draft_lua);
                let draft_validation_res = crate::lua_engine::validate_lua_rule_params(
                  &draft_lua,
                  &std::collections::HashMap::new(),
                  &draft_ctx,
                );
                let is_valid = draft_validation_res.is_ok();
                let validation_err_msg = draft_validation_res.err();

                html! {
                  <>
                    // If user is in Custom / Preset / Visual mode and the Lua script defines `params = { ... }`,
                    // render dynamic parameter controls directly from the Lua spec!
                    {if !is_cal_mode {
                      if let Ok(spec) = &draft_spec_res {
                        if !spec.params.is_empty() {
                          let resolved_map = crate::lua_engine::resolve_lua_rule_params(
                            spec,
                            &std::collections::HashMap::new(),
                          );
                          html! {
                            <div class="flex flex-col gap-space-sm bg-surface-container-lowest p-space-md rounded-xl border border-secondary/50">
                              <div class="flex items-center justify-between flex-wrap gap-2">
                                <span class="text-xs font-mono uppercase text-secondary font-bold">
                                  {format!("🎛️ Custom Lua Parameter Specifier ({} parameter(s) declared in params = {{ ... }}):", spec.params.len())}
                                </span>
                                <span class="text-[10px] font-mono text-on-surface-variant">
                                  {if spec.has_validate_fn {
                                    "Protected by schema bounds + Lua validate(params, ctx)"
                                  } else {
                                    "Protected by Lua parameter range / enum schema bounds"
                                  }}
                                </span>
                              </div>
                              <div class="grid grid-cols-1 md:grid-cols-2 gap-space-sm">
                                {for spec.params.iter().map(|p_spec| {
                                  let p_id = p_spec.id.clone();
                                  let cur_v = resolved_map
                                    .get(&p_spec.id)
                                    .cloned()
                                    .unwrap_or_else(|| p_spec.default_value.clone());
                                  let kind_str = match &p_spec.kind {
                                    crate::lua_engine::LuaParamKind::Range { min, max, .. } => {
                                      format!("Range [{} .. {} {}]", min, max, p_spec.unit)
                                    }
                                    crate::lua_engine::LuaParamKind::Enum => "Enum".to_string(),
                                    crate::lua_engine::LuaParamKind::Boolean => "Boolean".to_string(),
                                    crate::lua_engine::LuaParamKind::Text => "Text".to_string(),
                                  };
                                  let d_lua_param = draft_lua.clone();
                                  let d_name_param = draft_name.clone();
                                  let d_desc_param = draft_desc.clone();
                                  let cur_scope_param = (*draft_condition).clone();
                                  let has_desc_fn = spec.has_describe_fn;
                                  let spec_params_clone = spec.params.clone();
                                  let cur_map_base = resolved_map.clone();
                                  let update_custom_param = move |new_val: String| {
                                    let mut next_m = cur_map_base.clone();
                                    crate::lua_engine::apply_lua_param_change_with_sets(
                                      &spec_params_clone,
                                      &mut next_m,
                                      &p_id,
                                      new_val,
                                    );
                                    let updated_src = crate::lua_engine::embed_param_defaults_in_lua(&d_lua_param, &next_m);
                                    if has_desc_fn {
                                      let desc_ctx = serde_json::json!({ "scope": cur_scope_param.clone() });
                                      if let Ok(desc_out) = crate::lua_engine::describe_lua_rule(
                                        &updated_src,
                                        &next_m,
                                        &cur_scope_param,
                                        &desc_ctx,
                                      ) {
                                        if let Some(t) = desc_out.title {
                                          d_name_param.set(t);
                                        }
                                        if let Some(d) = desc_out.description {
                                          d_desc_param.set(d);
                                        }
                                      }
                                    }
                                    d_lua_param.set(updated_src);
                                  };
                                  let ucp_in = update_custom_param.clone();
                                  let ucp_slider = update_custom_param.clone();
                                  html! {
                                    <div class="bg-surface-container p-space-sm rounded-lg border border-outline-variant flex flex-col gap-1.5">
                                      <div class="flex items-center justify-between gap-2">
                                        <span class="text-xs font-bold text-on-surface">{&p_spec.label}</span>
                                        <span class="text-[10px] font-mono px-1.5 py-0.5 rounded bg-surface-container-highest text-secondary">{kind_str}</span>
                                      </div>
                                      <span class="text-[11px] text-on-surface-variant">{&p_spec.description}</span>
                                      {if let crate::lua_engine::LuaParamKind::Range { min, max, step } = &p_spec.kind {
                                        let step_val = step.unwrap_or(1.0);
                                        let cur_num = cur_v.trim().parse::<f64>().unwrap_or(*min);
                                        let dial_readout = if p_spec.unit == "°C" {
                                          let f_val = (cur_num * 1.8 + 32.0).round() as i64;
                                          format!("{} °C ({}°F)", cur_v, f_val)
                                        } else if p_spec.unit.is_empty() {
                                          cur_v.clone()
                                        } else {
                                          format!("{} {}", cur_v, p_spec.unit)
                                        };
                                        html! {
                                          <div class="flex flex-col gap-1 bg-surface-container-lowest p-2 rounded-lg border border-secondary/40">
                                            <div class="flex items-center justify-between text-[10px] font-mono">
                                              <span class="text-on-surface-variant">{format!("{}{}", min, &p_spec.unit)}</span>
                                              <span class="text-secondary font-bold">{format!("🎛️ {}", dial_readout)}</span>
                                              <span class="text-on-surface-variant">{format!("{}{}", max, &p_spec.unit)}</span>
                                            </div>
                                            <input
                                              type="range"
                                              min={min.to_string()}
                                              max={max.to_string()}
                                              step={step_val.to_string()}
                                              value={cur_v.clone()}
                                              class="w-full accent-secondary cursor-pointer"
                                              oninput={Callback::from(move |e: InputEvent| {
                                                if let Some(el) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                                                  ucp_slider(el.value());
                                                }
                                              })}
                                            />
                                          </div>
                                        }
                                      } else {
                                        html! {}
                                      }}
                                      <div class="flex flex-wrap gap-1">
                                        {for p_spec.options.iter().map(|(ov, ol)| {
                                          let picked = cur_v == *ov;
                                          let ov_c = ov.clone();
                                          let ucp_btn = update_custom_param.clone();
                                          html! {
                                            <button
                                              class={format!(
                                                "text-[10px] font-mono px-2 py-0.5 rounded border cursor-pointer {}",
                                                if picked {
                                                  "bg-secondary text-on-secondary border-secondary font-bold"
                                                } else {
                                                  "bg-surface-container-lowest text-on-surface border-outline-variant"
                                                }
                                              )}
                                              onclick={Callback::from(move |_| ucp_btn(ov_c.clone()))}
                                            >
                                              {ol}
                                            </button>
                                          }
                                        })}
                                      </div>
                                      <div class="flex items-center gap-2 pt-1 border-t border-outline-variant/40">
                                        <span class="text-[10px] font-mono text-on-surface-variant">{"Value:"}</span>
                                        <input
                                          type="text"
                                          class="w-28 bg-surface-container-lowest border border-outline-variant rounded px-2 py-0.5 text-xs font-mono text-on-surface"
                                          value={cur_v}
                                          oninput={Callback::from(move |e: InputEvent| {
                                            if let Some(el) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                                              ucp_in(el.value());
                                            }
                                          })}
                                        />
                                        <span class="text-[10px] font-mono text-tertiary">{&p_spec.unit}</span>
                                      </div>
                                    </div>
                                  }
                                })}
                              </div>
                            </div>
                          }
                        } else {
                          html! {}
                        }
                      } else {
                        html! {}
                      }
                    } else {
                      html! {}
                    }}

                    // Live Lua Validation Status Banner
                    {if let Some(err_text) = validation_err_msg {
                      html! {
                        <div class="bg-error-container/25 border-2 border-error text-error p-space-md rounded-xl font-mono text-xs flex items-center justify-between gap-2">
                          <div class="flex items-center gap-2">
                            <span class="material-symbols-outlined text-lg">{"error"}</span>
                            <div class="flex flex-col">
                              <span class="font-bold uppercase text-[11px]">
                                {"Lua Parameter Validation Failed — Saving Blocked Until Fixed"}
                              </span>
                              <span>{err_text}</span>
                            </div>
                          </div>
                          <span class="bg-error text-on-error px-2 py-0.5 rounded text-[10px] font-bold shrink-0">
                            {"INVALID PARAMETERS"}
                          </span>
                        </div>
                      }
                    } else {
                      html! {
                        <div class="bg-tertiary/10 border border-tertiary/40 text-tertiary px-space-md py-2 rounded-lg font-mono text-xs flex items-center justify-between">
                          <span>{"✅ Lua Parameter Specifier & validate(params, ctx) Passed"}</span>
                          <span class="text-[10px] text-on-surface-variant">{"Ready to save to transformation stack"}</span>
                        </div>
                      }
                    }}

                    <div class="flex flex-col gap-1">
                      <label class="text-[11px] font-mono uppercase text-on-surface-variant font-bold">
                        {"5. Lua Transformation Script (params = { ... }, validate(params, ctx), apply(config, ctx, params)):"}
                      </label>
                      <div class="bg-surface-container-lowest p-space-md rounded-lg border border-outline-variant">
                        <textarea
                          class="w-full h-44 bg-transparent text-on-surface font-mono text-xs focus:outline-none resize-y leading-relaxed"
                          value={(*draft_lua).clone()}
                          oninput={
                            let d_lua = draft_lua.clone();
                            Callback::from(move |e: InputEvent| {
                              if let Some(el) = e.target_dyn_into::<web_sys::HtmlTextAreaElement>() {
                                d_lua.set(el.value());
                              }
                            })
                          }
                        />
                      </div>
                    </div>

                    <div class="flex items-center justify-end gap-space-sm">
                      <button
                        class="px-space-md py-2 rounded-lg text-xs font-mono text-on-surface-variant hover:text-on-surface border border-outline-variant cursor-pointer"
                        onclick={
                          let src = show_rule_creator.clone();
                          Callback::from(move |_| src.set(false))
                        }
                      >
                        {"Cancel"}
                      </button>
                      <button
                        disabled={!is_valid}
                        class={format!(
                          "flex items-center gap-1.5 px-space-lg py-2 rounded-lg text-xs font-mono font-bold shadow-md {}",
                          if is_valid {
                            "bg-primary text-on-primary hover:bg-primary/90 cursor-pointer"
                          } else {
                            "bg-surface-container-highest text-on-surface-variant opacity-50 cursor-not-allowed"
                          }
                        )}
                        onclick={
                          let cb_create = props.on_create_rule.clone();
                          let src = show_rule_creator.clone();
                          let sel_state = selected_rule_id.clone();
                          let name_val = (*draft_name).clone();
                          let desc_val = (*draft_desc).clone();
                          let cat_val = (*draft_category).clone();
                          let icon_val = (*draft_icon).clone();
                          let cond_val = (*draft_condition).clone();
                          let lua_val = (*draft_lua).clone();
                          let cur_rules = ordered_rules.clone();
                          let next_num = cur_rules.len() + 1;

                          Callback::from(move |_| {
                            let check_ctx = serde_json::json!({ "scope": cond_val.clone() });
                            if crate::lua_engine::validate_lua_rule_params(
                              &lua_val,
                              &std::collections::HashMap::new(),
                              &check_ctx,
                            )
                            .is_err()
                            {
                              return;
                            }
                            let slug: String = name_val
                              .chars()
                              .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
                              .collect::<String>()
                              .trim_matches('_')
                              .to_string();
                            let rule_id = format!("user_{}_{}", if slug.is_empty() { "rule" } else { &slug }, next_num);
                            let append_prio = cur_rules.last().map(|r| r.priority + 10).unwrap_or(10);
                            let new_rule = TransformationRule {
                              id: rule_id.clone(),
                              name: name_val.clone(),
                              description: desc_val.clone(),
                              category: cat_val.clone(),
                              icon: icon_val.clone(),
                              enabled: true,
                              condition_expr: cond_val.clone(),
                              lua_code: lua_val.clone(),
                              priority: append_prio,
                            };
                            cb_create.emit(new_rule);
                            sel_state.set(rule_id);
                            src.set(false);
                          })
                        }
                      >
                        <span class="material-symbols-outlined text-[16px]">{"check_circle"}</span>
                        {if is_valid {
                          "Add Rule to Transformation Stack"
                        } else {
                          "Fix Invalid Parameters to Save Rule"
                        }}
                      </button>
                    </div>
                  </>
                }
              }}
            </div>
          }
        } else {
          html! {}
        }}

        // Sequential Execution Order Pipeline Bar & Non-Commutative Multi-Rule Chain Inspector
        {{
          // Group rule_changes by param_key to detect multi-rule non-commutative chains!
          let mut multi_rule_chains: Vec<(String, String, Vec<RuleBeforeAfterChange>)> = Vec::new();
          for ch in &props.rule_changes {
            if let Some((_, _, ref mut vec)) = multi_rule_chains
              .iter_mut()
              .find(|(k, _, _)| *k == ch.param_key)
            {
              vec.push(ch.clone());
            } else {
              multi_rule_chains.push((
                ch.param_key.clone(),
                ch.param_label.clone(),
                vec![ch.clone()],
              ));
            }
          }
          multi_rule_chains.retain(|(_, _, v)| v.len() > 1);

          html! {
            <div class="flex flex-col gap-space-sm bg-surface-container p-space-lg rounded-xl border border-outline-variant font-mono">
              <div class="flex flex-col gap-0.5">
                <div class="flex items-center gap-2 flex-wrap">
                  <span class="bg-primary text-on-primary text-[10px] font-bold uppercase px-2 py-0.5 rounded">
                    {"Sequential Transformation Pipeline (Step #1 ➔ Step #N)"}
                  </span>
                  <span class="text-xs font-bold text-on-surface">
                    {"Rules Execute Sequentially Top-to-Bottom (New Rules Append at Bottom)"}
                  </span>
                </div>
                <span class="text-[11px] text-on-surface-variant">
                  {"Each rule transforms the output produced by the previous step. Because operations like Set (=), Multiply (×), and Offset (+/-) are non-commutative, use the ⇈ First / ↑ Earlier / ↓ Later / ⇊ Last buttons on any rule card below to adjust execution order."}
                </span>
              </div>

              // Visual Step-by-Step Pipeline Strip
              <div class="flex items-center gap-1.5 overflow-x-auto py-1">
                {for ordered_rules.iter().enumerate().map(|(idx, r)| {
                  let step_num = idx + 1;
                  let sel_state = selected_rule_id.clone();
                  let rid_click = r.id.clone();

                  html! {
                    <>
                      <button
                        class={format!(
                          "flex flex-col items-start px-2.5 py-1.5 rounded-lg border text-left shrink-0 transition-all cursor-pointer {}",
                          if r.enabled {
                            "bg-surface-container-lowest border-primary/60 text-primary shadow-sm"
                          } else {
                            "bg-surface-container-low border-outline-variant/40 text-on-surface-variant opacity-55"
                          }
                        )}
                        onclick={Callback::from(move |_| sel_state.set(rid_click.clone()))}
                      >
                        <div class="flex items-center gap-1.5">
                          <span class="bg-surface-container-highest text-on-surface text-[10px] font-bold px-1.5 py-0.5 rounded">
                            {format!("Step #{}", step_num)}
                          </span>
                          <span class="text-[9px] uppercase text-on-surface-variant">{&r.condition_expr}</span>
                        </div>
                        <span class="text-[11px] font-bold text-on-surface mt-0.5 max-w-44 truncate">
                          {format!("{} {}", r.icon, r.name)}
                        </span>
                      </button>
                      {if step_num < ordered_rules.len() {
                        html! {
                          <span class="text-on-surface-variant font-bold text-xs shrink-0">{"➔"}</span>
                        }
                      } else {
                        html! {}
                      }}
                    </>
                  }
                })}
              </div>

              // Multi-Rule Non-Commutative Parameter Progression Chains (when >= 2 active rules modify the same key)
              {if !multi_rule_chains.is_empty() {
                html! {
                  <div class="bg-secondary/10 border border-secondary/50 rounded-lg p-space-sm flex flex-col gap-1.5 mt-1">
                    <div class="flex items-center gap-1.5">
                      <span class="material-symbols-outlined text-secondary text-[16px]">{"functions"}</span>
                      <span class="text-[11px] uppercase font-bold text-secondary">
                        {format!("Active Multi-Rule Parameter Chains ({} parameter(s) modified by multiple steps — order directly affects final value!):", multi_rule_chains.len())}
                      </span>
                    </div>
                    {for multi_rule_chains.iter().map(|(pkey, plabel, chain)| {
                      let initial_val = chain.first().map(|c| c.before_value.clone()).unwrap_or_default();
                      let final_val = chain.last().map(|c| c.after_value.clone()).unwrap_or_default();
                      html! {
                        <div class="flex items-center justify-between flex-wrap gap-2 bg-surface-container-lowest px-2.5 py-1.5 rounded border border-outline-variant/50 text-[11px]">
                          <span class="font-bold text-on-surface">
                            {format!("{} ({})", plabel, pkey)}
                          </span>
                          <div class="flex items-center flex-wrap gap-1.5">
                            <span class="text-on-surface-variant">{format!("Base: {}", initial_val)}</span>
                            {for chain.iter().map(|step_ch| {
                              html! {
                                <>
                                  <span class="text-primary font-bold">{"➔"}</span>
                                  <span class="bg-primary/15 border border-primary/40 text-on-surface px-1.5 py-0.5 rounded">
                                    {format!("Step #{}: {} ({})", step_ch.step_number, step_ch.rule_name, step_ch.after_value)}
                                  </span>
                                </>
                              }
                            })}
                            <span class="text-tertiary font-bold ml-1">
                              {format!("= Final: {}", final_val)}
                            </span>
                          </div>
                        </div>
                      }
                    })}
                  </div>
                }
              } else {
                html! {}
              }}
            </div>
          }
        }}

        <div class="grid grid-cols-1 lg:grid-cols-2 gap-space-lg">
          {for ordered_rules.iter().enumerate().map(|(rule_idx, rule)| {
            let step_number = rule_idx + 1;
            let is_first = rule_idx == 0;
            let is_last = rule_idx + 1 >= ordered_rules.len();
            let rid = rule.id.clone();
            let cb_toggle = on_toggle_rule.clone();
            let is_active = rule.enabled;
            let is_selected = *selected_rule_id == rule.id;
            let sel_state = selected_rule_id.clone();
            let r_for_click = rule.id.clone();
            let per_rule_diffs: Vec<&RuleBeforeAfterChange> = props
              .rule_changes
              .iter()
              .filter(|c| c.rule_id == rule.id)
              .collect();

            let cb_reorder_top = props.on_reorder_rule.clone();
            let cb_reorder_up = props.on_reorder_rule.clone();
            let cb_reorder_down = props.on_reorder_rule.clone();
            let cb_reorder_bot = props.on_reorder_rule.clone();
            let rid_top = rule.id.clone();
            let rid_up = rule.id.clone();
            let rid_down = rule.id.clone();
            let rid_bot = rule.id.clone();

            html! {
              <div
                class={format!(
                  "flex flex-col justify-between p-space-lg rounded-xl border transition-all {}",
                  if is_active {
                    "bg-surface-container border-primary shadow-md"
                  } else {
                    "bg-surface-container-low border-outline-variant hover:border-surface-bright"
                  }
                )}
              >
                <div class="flex flex-col gap-space-md">
                  <div class="flex items-start justify-between gap-space-sm">
                    <div class="flex items-center gap-space-sm">
                      <span class="text-2xl">{&rule.icon}</span>
                      <div class="flex flex-col">
                        <div class="flex items-center gap-space-xs flex-wrap">
                          <span class="text-[10px] uppercase font-mono font-bold bg-primary text-on-primary px-2 py-0.5 rounded">
                            {format!("Step #{}", step_number)}
                          </span>
                          <span class="text-[10px] uppercase font-mono text-secondary bg-surface-container-highest px-1.5 py-0.5 rounded">
                            {&rule.category}
                          </span>
                          <span class="text-[10px] font-mono text-primary bg-primary/10 border border-primary/30 px-1.5 py-0.5 rounded">
                            {format!("Scope: {}", rule.condition_expr)}
                          </span>
                        </div>
                        <h3 class="font-headline-sm text-body-md text-on-surface font-bold mt-0.5">
                          {&rule.name}
                        </h3>
                      </div>
                    </div>

                    <button
                      class={format!(
                        "px-space-md py-1 rounded text-xs font-mono font-bold transition-colors shrink-0 cursor-pointer {}",
                        if is_active {
                          "bg-primary text-on-primary"
                        } else {
                          "bg-surface-container-highest text-on-surface-variant hover:text-on-surface"
                        }
                      )}
                      onclick={Callback::from(move |_| cb_toggle.emit(rid.clone()))}
                    >
                      {if is_active { "Enabled (Disable)" } else { "Enable Rule" }}
                    </button>
                  </div>

                  <p class="text-body-sm text-on-surface-variant">{&rule.description}</p>

                  // Per-Rule Before -> After Changes Box
                  {if !per_rule_diffs.is_empty() {
                    html! {
                      <div class="bg-surface-container-lowest p-space-sm rounded-lg border border-outline-variant/60 flex flex-col gap-1 font-mono text-xs">
                        <span class="text-[10px] uppercase text-primary font-bold">
                          {format!("Step #{} Before → After Parameter Changes:", step_number)}
                        </span>
                        {for per_rule_diffs.iter().map(|ch| {
                          html! {
                            <div class="flex items-center justify-between gap-2 text-[11px]">
                              <span class="text-on-surface truncate">{format!("{} ({})", ch.param_label, ch.param_key)}</span>
                              <div class="flex items-center gap-1.5 shrink-0">
                                <span class="text-on-surface-variant line-through">{&ch.before_value}</span>
                                <span class="text-primary font-bold">{"→"}</span>
                                <span class="text-tertiary font-bold">{&ch.after_value}</span>
                              </div>
                            </div>
                          }
                        })}
                      </div>
                    }
                  } else if is_active {
                    html! {
                      <div class="bg-surface-container-lowest px-space-sm py-1.5 rounded-lg border border-outline-variant/40 font-mono text-[11px] text-on-surface-variant">
                        {"Active (0 parameter changes needed for current nozzle/filament baseline)"}
                      </div>
                    }
                  } else {
                    html! {}
                  }}

                  <div class="flex items-center justify-between flex-wrap gap-2 text-xs text-on-surface-variant pt-space-xs border-t border-outline-variant/40">
                    <button
                      class={format!(
                        "text-xs font-mono hover:underline flex items-center gap-1 cursor-pointer {}",
                        if is_selected { "text-secondary font-bold" } else { "text-on-surface-variant" }
                      )}
                      onclick={Callback::from(move |_| sel_state.set(r_for_click.clone()))}
                    >
                      <span class="material-symbols-outlined text-[14px]">{"code"}</span>
                      {if is_selected {
                        "Editing Lua Script Below ▼"
                      } else {
                        "Inspect / Edit Lua Source"
                      }}
                    </button>

                    <div class="flex items-center gap-1.5 flex-wrap">
                      // Reorder buttons: Move to First, Move Earlier (Up), Move Later (Down), Move to Last
                      <div class="flex items-center gap-1 bg-surface-container-lowest px-1.5 py-0.5 rounded border border-outline-variant/60 font-mono text-[10px]">
                        <span class="text-on-surface-variant mr-0.5">{"Order:"}</span>
                        <button
                          class="px-1.5 py-0.5 rounded bg-surface-container hover:bg-surface-bright text-on-surface cursor-pointer disabled:opacity-35"
                          disabled={is_first}
                          title="Move rule to Step #1 (First in stack)"
                          onclick={Callback::from(move |_| {
                            cb_reorder_top.emit((rid_top.clone(), StackReorderAction::MoveToTop));
                          })}
                        >
                          {"⇈ First"}
                        </button>
                        <button
                          class="px-1.5 py-0.5 rounded bg-surface-container hover:bg-surface-bright text-on-surface cursor-pointer disabled:opacity-35"
                          disabled={is_first}
                          title="Move rule 1 step earlier"
                          onclick={Callback::from(move |_| {
                            cb_reorder_up.emit((rid_up.clone(), StackReorderAction::MoveUp));
                          })}
                        >
                          {"↑ Earlier"}
                        </button>
                        <button
                          class="px-1.5 py-0.5 rounded bg-surface-container hover:bg-surface-bright text-on-surface cursor-pointer disabled:opacity-35"
                          disabled={is_last}
                          title="Move rule 1 step later"
                          onclick={Callback::from(move |_| {
                            cb_reorder_down.emit((rid_down.clone(), StackReorderAction::MoveDown));
                          })}
                        >
                          {"↓ Later"}
                        </button>
                        <button
                          class="px-1.5 py-0.5 rounded bg-surface-container hover:bg-surface-bright text-on-surface cursor-pointer disabled:opacity-35"
                          disabled={is_last}
                          title="Move rule to last step in stack"
                          onclick={Callback::from(move |_| {
                            cb_reorder_bot.emit((rid_bot.clone(), StackReorderAction::MoveToBottom));
                          })}
                        >
                          {"⇊ Last"}
                        </button>
                      </div>

                      {if rule.id.starts_with("user_") || rule.id.starts_with("custom_") || rule.id.starts_with("oneoff_") {
                        let del_id = rule.id.clone();
                        let cb_del = props.on_delete_rule.clone();
                        html! {
                          <button
                            class="text-[10px] font-mono text-error hover:underline cursor-pointer"
                            onclick={Callback::from(move |_| cb_del.emit(del_id.clone()))}
                          >
                            {"Delete Rule"}
                          </button>
                        }
                      } else {
                        html! {}
                      }}
                      <span class="text-[10px] uppercase bg-surface-container-lowest px-2 py-0.5 rounded font-mono text-tertiary">
                        {format!("Priority #{}", rule.priority)}
                      </span>
                    </div>
                  </div>
                </div>
              </div>
            }
          })}
        </div>
      </div>

      // Interactive Lua Code Editor & Sandbox
      {if let Some(rule) = active_rule {
        let rule_id = rule.id.clone();
        let cb_update = props.on_update_rule_lua.clone();
        let sandbox_check = crate::lua_engine::validate_lua_sandbox_security(&rule.lua_code);
        let rule_ctx = serde_json::json!({ "scope": rule.condition_expr.clone() });
        let rule_spec_res = crate::lua_engine::extract_lua_rule_spec(&rule.lua_code);
        let param_validation_res = crate::lua_engine::validate_lua_rule_params(
          &rule.lua_code,
          &std::collections::HashMap::new(),
          &rule_ctx,
        );
        let runtime_sec_err = props
          .execution_logs
          .iter()
          .find(|l| l.contains(&rule.name) && l.contains("Lua Sandbox Security"))
          .cloned();

        html! {
          <div class="flex flex-col gap-space-md bg-surface-container-low p-space-xl rounded-xl shadow-md border border-outline-variant">
            <div class="flex items-center justify-between flex-wrap gap-space-sm">
              <div class="flex items-center gap-space-sm">
                <span class="material-symbols-outlined text-secondary">{"terminal"}</span>
                <h3 class="font-headline-sm text-headline-sm text-on-surface font-semibold">
                  {format!("Live Lua Code Editor — transformations/{}.lua ({})", rule.id, rule.name)}
                </h3>
              </div>
              <div class="flex items-center gap-2 flex-wrap font-mono text-[11px]">
                <span class="bg-tertiary/15 text-tertiary border border-tertiary/40 px-2.5 py-0.5 rounded font-bold">
                  {"🛡️ Sandbox Protected: No FS/OS (io/os blocked) • 50k Step Cap • Depth-24 Anti-Fork-Bomb • 16KB String / 1K Table Cap"}
                </span>
              </div>
            </div>

            {if let Err(sec_err) = &sandbox_check {
              html! {
                <div class="bg-error-container/25 border-2 border-error text-error p-space-md rounded-xl font-mono text-xs flex items-center gap-2">
                  <span class="material-symbols-outlined text-lg">{"gpp_bad"}</span>
                  <span>{sec_err}</span>
                </div>
              }
            } else if let Some(rt_err) = runtime_sec_err {
              html! {
                <div class="bg-error-container/25 border-2 border-error text-error p-space-md rounded-xl font-mono text-xs flex items-center gap-2">
                  <span class="material-symbols-outlined text-lg">{"gpp_bad"}</span>
                  <span>{rt_err}</span>
                </div>
              }
            } else if let Err(param_err) = &param_validation_res {
              html! {
                <div class="bg-error-container/25 border-2 border-error text-error p-space-md rounded-xl font-mono text-xs flex items-center gap-2">
                  <span class="material-symbols-outlined text-lg">{"error"}</span>
                  <span>{format!("Lua Parameter Validation Error: {}", param_err)}</span>
                </div>
              }
            } else {
              html! {}
            }}

            // Interactive Parameter Controls for any Rule with a `params = { ... }` Specifier
            {if let Ok(spec) = &rule_spec_res {
              if !spec.params.is_empty() {
                let resolved_map = crate::lua_engine::resolve_lua_rule_params(
                  spec,
                  &std::collections::HashMap::new(),
                );
                html! {
                  <div class="bg-surface-container-lowest p-space-md rounded-xl border border-secondary/50 flex flex-col gap-space-sm">
                    <div class="flex items-center justify-between flex-wrap gap-2">
                      <span class="text-xs font-mono uppercase text-secondary font-bold">
                        {format!("🎛️ Rule Parameters ({} declared in params = {{ ... }}):", spec.params.len())}
                      </span>
                      <span class="text-[10px] font-mono text-tertiary">
                        {if spec.has_validate_fn {
                          "✅ Validated by Lua params schema + validate(params, ctx)"
                        } else {
                          "✅ Validated by Lua params range / enum schema"
                        }}
                      </span>
                    </div>
                    <div class="grid grid-cols-1 md:grid-cols-2 gap-space-sm">
                      {for spec.params.iter().map(|p_spec| {
                        let p_id = p_spec.id.clone();
                        let cur_v = resolved_map
                          .get(&p_spec.id)
                          .cloned()
                          .unwrap_or_else(|| p_spec.default_value.clone());
                        let kind_lbl = match &p_spec.kind {
                          crate::lua_engine::LuaParamKind::Range { min, max, .. } => {
                            format!("Range [{} .. {} {}]", min, max, p_spec.unit)
                          }
                          crate::lua_engine::LuaParamKind::Enum => "Enum".to_string(),
                          crate::lua_engine::LuaParamKind::Boolean => "Boolean".to_string(),
                          crate::lua_engine::LuaParamKind::Text => "Text".to_string(),
                        };
                        let rid_p = rule_id.clone();
                        let cb_p = cb_update.clone();
                        let base_src = rule.lua_code.clone();
                        let base_map = resolved_map.clone();
                        let spec_params_live = spec.params.clone();
                        let r_ctx = rule_ctx.clone();
                        let apply_param_edit = move |new_val: String| {
                          let mut next_m = base_map.clone();
                          crate::lua_engine::apply_lua_param_change_with_sets(
                            &spec_params_live,
                            &mut next_m,
                            &p_id,
                            new_val,
                          );
                          let candidate_lua = crate::lua_engine::embed_param_defaults_in_lua(&base_src, &next_m);
                          // Only save if Lua parameter validation passes!
                          if crate::lua_engine::validate_lua_rule_params(
                            &candidate_lua,
                            &std::collections::HashMap::new(),
                            &r_ctx,
                          )
                          .is_ok()
                          {
                            cb_p.emit((rid_p.clone(), candidate_lua));
                          }
                        };
                        let ape_in = apply_param_edit.clone();
                        let ape_slider = apply_param_edit.clone();
                        html! {
                          <div class="bg-surface-container p-space-sm rounded-lg border border-outline-variant flex flex-col gap-1.5">
                            <div class="flex items-center justify-between gap-2">
                              <span class="text-xs font-bold text-on-surface">{&p_spec.label}</span>
                              <span class="text-[10px] font-mono px-1.5 py-0.5 rounded bg-surface-container-highest text-secondary">{kind_lbl}</span>
                            </div>
                            <span class="text-[11px] text-on-surface-variant">{&p_spec.description}</span>
                            {if let crate::lua_engine::LuaParamKind::Range { min, max, step } = &p_spec.kind {
                              let step_val = step.unwrap_or(1.0);
                              let cur_num = cur_v.trim().parse::<f64>().unwrap_or(*min);
                              let dial_readout = if p_spec.unit == "°C" {
                                let f_val = (cur_num * 1.8 + 32.0).round() as i64;
                                format!("{} °C ({}°F)", cur_v, f_val)
                              } else if p_spec.unit.is_empty() {
                                cur_v.clone()
                              } else {
                                format!("{} {}", cur_v, p_spec.unit)
                              };
                              html! {
                                <div class="flex flex-col gap-1 bg-surface-container-lowest p-2 rounded-lg border border-secondary/40">
                                  <div class="flex items-center justify-between text-[10px] font-mono">
                                    <span class="text-on-surface-variant">{format!("{}{}", min, &p_spec.unit)}</span>
                                    <span class="text-secondary font-bold">{format!("🎛️ {}", dial_readout)}</span>
                                    <span class="text-on-surface-variant">{format!("{}{}", max, &p_spec.unit)}</span>
                                  </div>
                                  <input
                                    type="range"
                                    min={min.to_string()}
                                    max={max.to_string()}
                                    step={step_val.to_string()}
                                    value={cur_v.clone()}
                                    class="w-full accent-secondary cursor-pointer"
                                    oninput={Callback::from(move |e: InputEvent| {
                                      if let Some(el) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                                        ape_slider(el.value());
                                      }
                                    })}
                                  />
                                </div>
                              }
                            } else {
                              html! {}
                            }}
                            <div class="flex flex-wrap gap-1">
                              {for p_spec.options.iter().map(|(ov, ol)| {
                                let picked = cur_v == *ov;
                                let ov_c = ov.clone();
                                let ape_btn = apply_param_edit.clone();
                                html! {
                                  <button
                                    class={format!(
                                      "text-[10px] font-mono px-2 py-0.5 rounded border cursor-pointer {}",
                                      if picked {
                                        "bg-secondary text-on-secondary border-secondary font-bold"
                                      } else {
                                        "bg-surface-container-lowest text-on-surface border-outline-variant"
                                      }
                                    )}
                                    onclick={Callback::from(move |_| ape_btn(ov_c.clone()))}
                                  >
                                    {ol}
                                  </button>
                                }
                              })}
                            </div>
                            <div class="flex items-center gap-2 pt-1 border-t border-outline-variant/40">
                              <span class="text-[10px] font-mono text-on-surface-variant">{"Value:"}</span>
                              <input
                                type="text"
                                class="w-28 bg-surface-container-lowest border border-outline-variant rounded px-2 py-0.5 text-xs font-mono text-on-surface"
                                value={cur_v}
                                onchange={Callback::from(move |e: Event| {
                                  if let Some(el) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                                    ape_in(el.value());
                                  }
                                })}
                              />
                              <span class="text-[10px] font-mono text-tertiary">{&p_spec.unit}</span>
                            </div>
                          </div>
                        }
                      })}
                    </div>
                  </div>
                }
              } else {
                html! {}
              }
            } else {
              html! {}
            }}

            <div class="bg-surface-container-lowest p-space-md rounded-lg border border-outline-variant">
              <textarea
                class="w-full h-56 bg-transparent text-on-surface font-mono text-xs focus:outline-none resize-y leading-relaxed"
                value={rule.lua_code.clone()}
                oninput={
                  let rid = rule_id.clone();
                  let cb = cb_update.clone();
                  Callback::from(move |e: InputEvent| {
                    if let Some(target) = e.target_dyn_into::<web_sys::HtmlTextAreaElement>() {
                      cb.emit((rid.clone(), target.value()));
                    }
                  })
                }
              />
            </div>
          </div>
        }
      } else {
        html! {}
      }}

      // Integrated Lua Execution & Evaluation Terminal (with Before -> After log lines)
      <div class="flex flex-col gap-space-md bg-surface-container-lowest p-space-lg rounded-xl shadow-lg border border-outline-variant">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-space-sm">
            <span class="material-symbols-outlined text-tertiary">{"terminal"}</span>
            <span class="font-headline-sm text-body-md text-on-surface font-semibold">
              {"Integrated Lua Execution Log (with Before → After Parameter Traces)"}
            </span>
          </div>

          <button
            class="text-xs text-on-surface-variant hover:text-primary transition-colors flex items-center gap-1 font-mono cursor-pointer"
            onclick={
              let cb = props.on_clear_logs.clone();
              Callback::from(move |_| cb.emit(()))
            }
          >
            <span class="material-symbols-outlined text-[14px]">{"refresh"}</span>
            {"Clear Log"}
          </button>
        </div>

        <div class="bg-surface-container p-space-md rounded-lg font-mono text-xs flex flex-col gap-1 max-h-64 overflow-y-auto">
          {for props.execution_logs.iter().map(|log| {
            let text_color = if log.contains("❌") {
              "text-error"
            } else if log.contains("⚙️") {
              "text-primary font-bold"
            } else if log.contains("[Before → After]") {
              "text-secondary"
            } else if log.contains("⏩") {
              "text-on-surface-variant"
            } else {
              "text-tertiary"
            };

            html! {
              <div class={format!("flex items-center gap-2 {}", text_color)}>
                <span>{log}</span>
              </div>
            }
          })}
        </div>
      </div>
    </div>
  }
}

/// Renders an inline SVG visual illustration for each Print Quality Issue (`sym_*`) or Calibration Test (`cal_*`).
/// - When `compact == true`: renders a thumbnail illustration of the print defect for the Category Tree cards.
/// - When `compact == false`: renders a dual-panel diagnostic illustration showing both:
///   1. Left Panel: What the Defect Looks Like on the 3D Print
///   2. Right Panel: Physical Hardware Check (e.g. Kobra S1 Extruder Tension Screw, Cold Pull, Belt Tension, Dish Soap PEI) + Slicer Toolpath Fix
pub fn render_issue_illustration(issue_id: &str, compact: bool) -> Html {
  let (defect_title, defect_svg, fix_title, fix_svg) = match issue_id {
    "sym_extruder_grinding" => (
      "DEFECT: Ground Half-Moon Notch in Filament",
      r##"
        <rect x="20" y="18" width="160" height="58" rx="6" fill="#141926" stroke="#ff6b6b" stroke-width="1"/>
        <!-- Filament strand with chewed half-moon bite -->
        <path d="M 80 20 L 80 38 C 88 42, 88 52, 80 56 L 80 74 L 94 74 L 94 20 Z" fill="#ffb86c"/>
        <!-- Left & Right Hobbed Drive Gears -->
        <circle cx="64" cy="47" r="14" fill="#2c354d" stroke="#8be9fd" stroke-width="2" stroke-dasharray="3,2"/>
        <circle cx="110" cy="47" r="14" fill="#2c354d" stroke="#ff5555" stroke-width="2" stroke-dasharray="3,2"/>
        <!-- Plastic shavings / dust -->
        <circle cx="76" cy="44" r="1.8" fill="#ff5555"/>
        <circle cx="74" cy="50" r="1.5" fill="#ff5555"/>
        <circle cx="78" cy="53" r="1.4" fill="#ff5555"/>
        <text x="100" y="88" fill="#ff8080" font-size="8.5" font-family="monospace" text-anchor="middle">Chewed Notch + Shavings in Gears</text>
      "##,
      "FIX: Kobra S1 Right-Side Extruder Tension Screw",
      r##"
        <!-- Kobra S1 Toolhead Outline -->
        <rect x="255" y="18" width="88" height="56" rx="7" fill="#1c2436" stroke="#8be9fd" stroke-width="1.5"/>
        <text x="299" y="31" fill="#8be9fd" font-size="7.5" font-family="monospace" text-anchor="middle">KOBRA S1 PRINTHEAD</text>
        <!-- Dual gears inside toolhead -->
        <circle cx="288" cy="48" r="9" fill="#2c354d" stroke="#50fa7b" stroke-width="1.5"/>
        <circle cx="309" cy="48" r="9" fill="#2c354d" stroke="#50fa7b" stroke-width="1.5"/>
        <!-- Right-side Idler Tension Screw + Spring -->
        <rect x="343" y="42" width="14" height="12" rx="2" fill="#ffb86c" stroke="#fff" stroke-width="1"/>
        <line x1="357" y1="48" x2="395" y2="48" stroke="#f1fa8c" stroke-width="2.5"/>
        <text x="398" y="43" fill="#50fa7b" font-size="8" font-family="monospace">CW +1/4 Turn = Grip</text>
        <text x="398" y="55" fill="#ffb86c" font-size="8" font-family="monospace">CCW -1/2 = Soft TPU</text>
        <text x="345" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">Right-Side 2.0mm Hex Tension Screw + Retract ≤35mm/s</text>
      "##,
    ),
    "sym_stringing" => (
      "DEFECT: Cobweb Hairs Across Travel Gap",
      r##"
        <line x1="20" y1="74" x2="180" y2="74" stroke="#6272a4" stroke-width="3"/>
        <rect x="38" y="22" width="26" height="52" fill="#44475a" stroke="#8be9fd" stroke-width="1.5"/>
        <rect x="136" y="22" width="26" height="52" fill="#44475a" stroke="#8be9fd" stroke-width="1.5"/>
        <!-- Cobweb strings between towers -->
        <path d="M 64 28 Q 100 35 136 26" stroke="#ff79c6" stroke-width="1.2" fill="none"/>
        <path d="M 64 38 Q 100 30 136 40" stroke="#ff79c6" stroke-width="1" fill="none"/>
        <path d="M 64 48 Q 100 56 136 46" stroke="#ff79c6" stroke-width="1.3" fill="none"/>
        <path d="M 64 60 Q 100 52 136 62" stroke="#ff79c6" stroke-width="1" fill="none"/>
        <text x="100" y="88" fill="#ff79c6" font-size="8.5" font-family="monospace" text-anchor="middle">Fine Hairs &amp; Oozing Between Towers</text>
      "##,
      "FIX: Spiral Corkscrew Z-Hop + 70% Pre-Wipe",
      r##"
        <line x1="250" y1="74" x2="435" y2="74" stroke="#6272a4" stroke-width="3"/>
        <rect x="268" y="26" width="28" height="48" fill="#44475a" stroke="#50fa7b" stroke-width="1.5"/>
        <rect x="388" y="26" width="28" height="48" fill="#44475a" stroke="#50fa7b" stroke-width="1.5"/>
        <!-- Spiral corkscrew lift path -->
        <path d="M 282 26 C 270 20, 296 16, 284 11 L 395 11" stroke="#50fa7b" stroke-width="2" stroke-dasharray="4,2" fill="none"/>
        <polygon points="395,11 388,8 388,14" fill="#50fa7b"/>
        <text x="342" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">Spiral Lift Shears Strand + Temp -3°C..5°C</text>
      "##,
    ),
    "sym_boogers" => (
      "DEFECT: Nozzle Scraping Infill & Dropping Boogers",
      r##"
        <rect x="25" y="52" width="150" height="22" fill="#383c4a" stroke="#6272a4" stroke-width="1.5"/>
        <!-- Plowed ridge & burnt booger blob on wall -->
        <circle cx="118" cy="49" r="7" fill="#d9534f" stroke="#ffb86c" stroke-width="1.5"/>
        <!-- Nozzle tip coated with molten blob -->
        <polygon points="82,18 106,18 98,38 90,38" fill="#ffb86c" stroke="#fff" stroke-width="1"/>
        <circle cx="94" cy="41" r="5.5" fill="#ff5555"/>
        <text x="100" y="88" fill="#ff8080" font-size="8.5" font-family="monospace" text-anchor="middle">Snowballing Nozzle Blob &amp; Burnt Drip</text>
      "##,
      "FIX: Flow -0.02 + Non-Crossing Gyroid + Slow Deretract",
      r##"
        <rect x="255" y="42" width="175" height="30" rx="4" fill="#1e2738" stroke="#50fa7b" stroke-width="1.5"/>
        <!-- Smooth sinusoidal non-crossing gyroid waves -->
        <path d="M 262 57 Q 280 44, 298 57 T 334 57 T 370 57 T 406 57" stroke="#8be9fd" stroke-width="2.2" fill="none"/>
        <text x="342" y="29" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">Gyroid Never Crosses Same Layer • Deretract 28mm/s</text>
        <text x="342" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">Zero Nozzle Scraping • Clean Silicone Sock</text>
      "##,
    ),
    "sym_zits_blobs" => (
      "DEFECT: Random Pimples & Zits on Outer Wall",
      r##"
        <rect x="42" y="20" width="116" height="54" rx="8" fill="#3b4252" stroke="#8be9fd" stroke-width="1.5"/>
        <circle cx="58" cy="32" r="3.5" fill="#ff5555"/>
        <circle cx="125" cy="38" r="3.5" fill="#ff5555"/>
        <circle cx="84" cy="49" r="3.8" fill="#ff5555"/>
        <circle cx="142" cy="60" r="3.5" fill="#ff5555"/>
        <circle cx="67" cy="63" r="3.2" fill="#ff5555"/>
        <text x="100" y="88" fill="#ff8080" font-size="8.5" font-family="monospace" text-anchor="middle">Scattered Seam Burps / PLR Pause Blobs</text>
      "##,
      "FIX: Aligned Scarf Seam + 26mm/s Smooth Reload",
      r##"
        <rect x="282" y="20" width="116" height="54" rx="8" fill="#3b4252" stroke="#50fa7b" stroke-width="2"/>
        <!-- Single aligned hidden corner seam -->
        <line x1="398" y1="22" x2="398" y2="72" stroke="#f1fa8c" stroke-width="2.5" stroke-dasharray="2,2"/>
        <text x="340" y="49" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">Blemish-Free Outer Wall</text>
        <text x="342" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">Aligned Corner + Scarf Ramp + 10% Seam Gap</text>
      "##,
    ),
    "sym_clogging" => (
      "DEFECT: Heat-Creep Plug / CF Bridge Inside Nozzle",
      r##"
        <!-- Heatbreak & Nozzle Cross-Section -->
        <rect x="78" y="16" width="44" height="28" fill="#2c354d" stroke="#8be9fd" stroke-width="1.5"/>
        <polygon points="74,44 126,44 108,72 92,72" fill="#ffb86c" stroke="#ff5555" stroke-width="1.5"/>
        <!-- Swollen heat-creep plug & black carbon debris -->
        <rect x="86" y="28" width="28" height="22" rx="4" fill="#ff5555"/>
        <circle cx="96" cy="60" r="3" fill="#111"/>
        <circle cx="104" cy="63" r="3" fill="#111"/>
        <text x="100" y="88" fill="#ff8080" font-size="8.5" font-family="monospace" text-anchor="middle">Swollen Heat-Creep Plug / CF Jam</text>
      "##,
      "FIX: Cold Pull (250°C → 95°C) + Retract Cap ≤0.75mm",
      r##"
        <polygon points="314,38 366,38 348,68 332,68" fill="#44475a" stroke="#50fa7b" stroke-width="1.5"/>
        <!-- Cold pull filament yanked upward with stuck debris -->
        <path d="M 333 14 L 347 14 L 347 46 L 340 58 L 333 46 Z" fill="#50fa7b"/>
        <circle cx="340" cy="52" r="3" fill="#111"/>
        <path d="M 378 52 L 378 20" stroke="#f1fa8c" stroke-width="2.5"/>
        <polygon points="378,16 373,24 383,24" fill="#f1fa8c"/>
        <text x="342" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">Cold Pull Extracts Debris • Cap Retract ≤0.75mm</text>
      "##,
    ),
    "sym_under_extrusion" => (
      "DEFECT: Gaps Between Extrusion Lines & Thin Walls",
      r##"
        <rect x="28" y="22" width="144" height="50" rx="4" fill="#181c28" stroke="#6272a4" stroke-width="1"/>
        <!-- Separated thin extrusion lines with visible dark gaps -->
        <line x1="36" y1="32" x2="164" y2="32" stroke="#ff6b6b" stroke-width="3.5" stroke-dasharray="28,6"/>
        <line x1="36" y1="43" x2="164" y2="43" stroke="#ff6b6b" stroke-width="3.5"/>
        <line x1="36" y1="54" x2="164" y2="54" stroke="#ff6b6b" stroke-width="3.5" stroke-dasharray="18,8"/>
        <line x1="36" y1="65" x2="164" y2="65" stroke="#ff6b6b" stroke-width="3.5"/>
        <text x="100" y="88" fill="#ff8080" font-size="8.5" font-family="monospace" text-anchor="middle">Visible Gaps &amp; Porous Under-Extruded Lines</text>
      "##,
      "FIX: Flow +3% + MVS -15% + Kobra Tension Check",
      r##"
        <rect x="268" y="22" width="148" height="50" rx="4" fill="#181c28" stroke="#50fa7b" stroke-width="1.5"/>
        <!-- Tightly fused adjacent extrusion beads -->
        <rect x="276" y="28" width="132" height="9" rx="4" fill="#50fa7b" opacity="0.85"/>
        <rect x="276" y="37" width="132" height="9" rx="4" fill="#50fa7b" opacity="0.95"/>
        <rect x="276" y="46" width="132" height="9" rx="4" fill="#50fa7b" opacity="0.85"/>
        <rect x="276" y="55" width="132" height="9" rx="4" fill="#50fa7b" opacity="0.95"/>
        <text x="342" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">100% Fused Beads • Flow +0.03 &amp; MVS Cushion</text>
      "##,
    ),
    "sym_over_extrusion" => (
      "DEFECT: Nozzle Plowing Sharp Ridges in Top Skin",
      r##"
        <rect x="30" y="44" width="140" height="28" fill="#ffb86c" stroke="#ff5555" stroke-width="1.5"/>
        <!-- Sharp squeezed-up ridges between passes -->
        <polygon points="48,44 53,33 58,44" fill="#ff5555"/>
        <polygon points="78,44 83,31 88,44" fill="#ff5555"/>
        <polygon points="108,44 113,32 118,44" fill="#ff5555"/>
        <polygon points="138,44 143,33 148,44" fill="#ff5555"/>
        <text x="100" y="88" fill="#ff8080" font-size="8.5" font-family="monospace" text-anchor="middle">Plowed Top Ridges &amp; Swollen Dimensions</text>
      "##,
      "FIX: Flow -0.02 & Top Surface Flow = 0.96",
      r##"
        <rect x="270" y="42" width="144" height="30" rx="3" fill="#50fa7b" opacity="0.85" stroke="#8be9fd" stroke-width="1.5"/>
        <line x1="265" y1="42" x2="420" y2="42" stroke="#f1fa8c" stroke-width="2"/>
        <text x="342" y="33" fill="#f1fa8c" font-size="8" font-family="monospace" text-anchor="middle">Glass-Flat Top Surface (0.96 Top Flow)</text>
        <text x="342" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">Zero Ridge Plowing • Exact Dimensional Fit</text>
      "##,
    ),
    "sym_wet_filament" => (
      "DEFECT: Steam Popping, Bubbles & Foamy Walls",
      r##"
        <rect x="35" y="22" width="130" height="50" rx="5" fill="#3b4252" stroke="#ff79c6" stroke-width="1.5"/>
        <!-- Burst steam craters / bubbles -->
        <circle cx="62" cy="36" r="5" fill="#141926" stroke="#ff79c6" stroke-width="1.5"/>
        <circle cx="105" cy="44" r="6" fill="#141926" stroke="#ff79c6" stroke-width="1.5"/>
        <circle cx="82" cy="58" r="4.5" fill="#141926" stroke="#ff79c6" stroke-width="1.5"/>
        <circle cx="136" cy="35" r="4.5" fill="#141926" stroke="#ff79c6" stroke-width="1.5"/>
        <text x="100" y="88" fill="#ff79c6" font-size="8.5" font-family="monospace" text-anchor="middle">Burst Steam Craters &amp; 'Pop/Crackle' Sound</text>
      "##,
      "FIX: ACE Pro Active Dryer (55°C 6h) + Temp -5°C",
      r##"
        <rect x="268" y="20" width="148" height="52" rx="8" fill="#1e2738" stroke="#50fa7b" stroke-width="1.5"/>
        <circle cx="305" cy="46" r="18" fill="none" stroke="#ffb86c" stroke-width="4"/>
        <circle cx="305" cy="46" r="7" fill="#2c354d" stroke="#8be9fd" stroke-width="1.5"/>
        <text x="372" y="40" fill="#ffb86c" font-size="8.5" font-family="monospace" text-anchor="middle">ACE PRO DRYER</text>
        <text x="372" y="53" fill="#50fa7b" font-size="8" font-family="monospace" text-anchor="middle">PETG/PCTG: 55°C 6h</text>
        <text x="372" y="64" fill="#8be9fd" font-size="8" font-family="monospace" text-anchor="middle">PLA: 45°C 4h</text>
        <text x="342" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">Dry Spool in ACE Pro + Nozzle -5°C</text>
      "##,
    ),
    "sym_not_sticking" => (
      "DEFECT: Round Un-Squished Lines Peeling Off PEI",
      r##"
        <line x1="20" y1="68" x2="180" y2="68" stroke="#ffb86c" stroke-width="4"/>
        <!-- Round non-squished beads lifting into spaghetti -->
        <circle cx="52" cy="60" r="6" fill="#ff5555"/>
        <circle cx="72" cy="60" r="6" fill="#ff5555"/>
        <path d="M 92 62 Q 125 22 158 48" stroke="#ff5555" stroke-width="4" fill="none"/>
        <text x="100" y="88" fill="#ff8080" font-size="8.5" font-family="monospace" text-anchor="middle">No Squish / Finger Oils on Textured PEI</text>
      "##,
      "FIX: Dish Soap Wash + 0.50mm Width + 28mm/s L1",
      r##"
        <line x1="255" y1="68" x2="425" y2="68" stroke="#ffb86c" stroke-width="4"/>
        <!-- Wide stadium-shaped squished first-layer beads locked into PEI -->
        <rect x="275" y="54" width="38" height="12" rx="6" fill="#50fa7b"/>
        <rect x="315" y="54" width="38" height="12" rx="6" fill="#50fa7b"/>
        <rect x="355" y="54" width="38" height="12" rx="6" fill="#50fa7b"/>
        <text x="340" y="36" fill="#8be9fd" font-size="8.5" font-family="monospace" text-anchor="middle">Wash PEI with Warm Water + Dish Soap!</text>
        <text x="342" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">125% L1 Width (0.50mm) Locks Into PEI Valleys</text>
      "##,
    ),
    "sym_warping_adhesion" => (
      "DEFECT: Corners Curling & Lifting Off Build Plate",
      r##"
        <line x1="20" y1="70" x2="180" y2="70" stroke="#ffb86c" stroke-width="3.5"/>
        <!-- Warped box with lifted bottom corners -->
        <path d="M 35 50 Q 65 68 100 68 Q 135 68 165 50 L 165 26 L 35 26 Z" fill="#44475a" stroke="#ff5555" stroke-width="2"/>
        <path d="M 38 67 L 38 53" stroke="#f1fa8c" stroke-width="2"/>
        <path d="M 162 67 L 162 53" stroke="#f1fa8c" stroke-width="2"/>
        <text x="100" y="88" fill="#ff8080" font-size="8.5" font-family="monospace" text-anchor="middle">Thermal Contraction Peeling Corners Up</text>
      "##,
      "FIX: Bed +5°C + Fan Off 3 Layers + 5mm Outer Brim",
      r##"
        <line x1="250" y1="70" x2="430" y2="70" stroke="#ffb86c" stroke-width="3.5"/>
        <!-- Brim anchoring left and right corners -->
        <rect x="258" y="63" width="164" height="6" rx="2" fill="#8be9fd"/>
        <rect x="282" y="26" width="116" height="42" fill="#44475a" stroke="#50fa7b" stroke-width="2"/>
        <text x="340" y="50" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">Flat Anchored Base</text>
        <text x="342" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">5mm Brim + Bed +5°C + Fan Off First 3 Layers</text>
      "##,
    ),
    "sym_elephants_foot" => (
      "DEFECT: Bottom Layer Squished Outward ('Elephant Foot')",
      r##"
        <line x1="25" y1="72" x2="175" y2="72" stroke="#ffb86c" stroke-width="3"/>
        <path d="M 52 24 L 148 24 L 148 58 L 162 66 L 162 71 L 38 71 L 38 66 L 52 58 Z" fill="#44475a" stroke="#ff5555" stroke-width="2"/>
        <text x="100" y="88" fill="#ff8080" font-size="8.5" font-family="monospace" text-anchor="middle">Flared Bottom Lip Blocks Mating Parts</text>
      "##,
      "FIX: Elephant Foot Compensation = 0.15mm Inset",
      r##"
        <line x1="260" y1="72" x2="420" y2="72" stroke="#ffb86c" stroke-width="3"/>
        <rect x="290" y="24" width="100" height="47" fill="#44475a" stroke="#50fa7b" stroke-width="2"/>
        <path d="M 274 65 L 288 65" stroke="#8be9fd" stroke-width="2"/>
        <path d="M 406 65 L 392 65" stroke="#8be9fd" stroke-width="2"/>
        <text x="342" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">0.15mm Layer-1 Inset → Flush 90° Vertical Wall</text>
      "##,
    ),
    "sym_first_layer_ripples" => (
      "DEFECT: Wavy 'Tiger-Stripe' Plowing Ripples on L1",
      r##"
        <rect x="30" y="20" width="140" height="52" rx="4" fill="#2c354d" stroke="#ff5555" stroke-width="1.5"/>
        <path d="M 38 32 Q 55 24, 72 32 T 106 32 T 140 32" stroke="#ffb86c" stroke-width="3" fill="none"/>
        <path d="M 38 46 Q 55 38, 72 46 T 106 46 T 140 46" stroke="#ff5555" stroke-width="3" fill="none"/>
        <path d="M 38 60 Q 55 52, 72 60 T 106 60 T 140 60" stroke="#ffb86c" stroke-width="3" fill="none"/>
        <text x="100" y="88" fill="#ff8080" font-size="8.5" font-family="monospace" text-anchor="middle">Nozzle Too Close Plowing Wavy Furrows</text>
      "##,
      "FIX: Initial Layer Height = 0.24mm & Width = 0.42mm",
      r##"
        <rect x="270" y="20" width="144" height="52" rx="4" fill="#2c354d" stroke="#50fa7b" stroke-width="1.5"/>
        <line x1="280" y1="32" x2="404" y2="32" stroke="#50fa7b" stroke-width="3"/>
        <line x1="280" y1="44" x2="404" y2="44" stroke="#50fa7b" stroke-width="3"/>
        <line x1="280" y1="56" x2="404" y2="56" stroke="#50fa7b" stroke-width="3"/>
        <text x="342" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">0.24mm First Layer Absorbs Mesh Variations</text>
      "##,
    ),
    "sym_visible_seam" => (
      "DEFECT: Pronounced Vertical Z-Seam Ridge / Scar",
      r##"
        <ellipse cx="100" cy="46" rx="52" ry="26" fill="#3b4252" stroke="#8be9fd" stroke-width="2"/>
        <!-- Protruding seam bump on cylinder wall -->
        <circle cx="152" cy="46" r="6" fill="#ff5555" stroke="#fff" stroke-width="1"/>
        <text x="100" y="88" fill="#ff8080" font-size="8.5" font-family="monospace" text-anchor="middle">Abrupt Loop Start/Stop Step Scar</text>
      "##,
      "FIX: 20mm Scarf Joint Wedge Ramp + 10% Seam Gap",
      r##"
        <ellipse cx="342" cy="46" rx="52" ry="26" fill="#3b4252" stroke="#50fa7b" stroke-width="2"/>
        <!-- Overlapping tapered scarf joint ramp -->
        <path d="M 382 30 A 52 26 0 0 1 382 62" stroke="#f1fa8c" stroke-width="3" fill="none"/>
        <text x="342" y="49" fill="#50fa7b" font-size="8" font-family="monospace" text-anchor="middle">20mm Scarf Wedge</text>
        <text x="342" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">Seamless Tapered Overlap + Wipe on Loops</text>
      "##,
    ),
    "sym_ringing_vfa" => (
      "DEFECT: Ghosting Echo Ripples Around Features",
      r##"
        <rect x="28" y="20" width="144" height="52" rx="4" fill="#3b4252" stroke="#8be9fd" stroke-width="1.5"/>
        <circle cx="66" cy="46" r="13" fill="#141926" stroke="#8be9fd" stroke-width="1.5"/>
        <!-- Repeating ghost echoes to the right of the hole -->
        <path d="M 88 33 A 14 14 0 0 1 88 59" stroke="#ff5555" stroke-width="2" fill="none"/>
        <path d="M 102 33 A 14 14 0 0 1 102 59" stroke="#ff5555" stroke-width="1.5" opacity="0.75" fill="none"/>
        <path d="M 116 33 A 14 14 0 0 1 116 59" stroke="#ff5555" stroke-width="1" opacity="0.5" fill="none"/>
        <text x="100" y="88" fill="#ff8080" font-size="8.5" font-family="monospace" text-anchor="middle">Gantry Ringing Echoes &amp; Belt VFAs</text>
      "##,
      "FIX: Outer Wall Accel 3000 mm/s² + 160 mm/s + Belt Tune",
      r##"
        <rect x="268" y="20" width="148" height="52" rx="4" fill="#3b4252" stroke="#50fa7b" stroke-width="1.5"/>
        <circle cx="308" cy="46" r="13" fill="#141926" stroke="#50fa7b" stroke-width="2"/>
        <text x="366" y="43" fill="#50fa7b" font-size="8" font-family="monospace" text-anchor="middle">Zero Echoes</text>
        <text x="366" y="55" fill="#8be9fd" font-size="7.5" font-family="monospace" text-anchor="middle">Input Shaper + 3k Accel</text>
        <text x="342" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">Cruises Above 60-115mm/s VFA Resonance Band</text>
      "##,
    ),
    "sym_layer_shifting" => (
      "DEFECT: Upper Layers Shifted Sideways (Step Loss)",
      r##"
        <rect x="44" y="46" width="84" height="26" fill="#44475a" stroke="#8be9fd" stroke-width="1.5"/>
        <!-- Shifted upper half -->
        <rect x="68" y="20" width="84" height="26" fill="#44475a" stroke="#ff5555" stroke-width="2"/>
        <path d="M 44 33 L 64 33" stroke="#f1fa8c" stroke-width="2"/>
        <polygon points="66,33 59,29 59,37" fill="#f1fa8c"/>
        <text x="100" y="88" fill="#ff8080" font-size="8.5" font-family="monospace" text-anchor="middle">Staircase X/Y Shift from Collision or Accel</text>
      "##,
      "FIX: Travel Accel ≤7000 + 0.20mm Z-Hop + Pulley Screws",
      r##"
        <rect x="298" y="20" width="88" height="52" fill="#44475a" stroke="#50fa7b" stroke-width="2"/>
        <text x="342" y="48" fill="#50fa7b" font-size="8" font-family="monospace" text-anchor="middle">Aligned Z-Walls</text>
        <text x="342" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">Check Pulley Grub Screws • 0.20mm Z-Hop • Gyroid</text>
      "##,
    ),
    "sym_z_banding_hull_line" => (
      "DEFECT: Horizontal Gloss Band / Benchy Hull Line",
      r##"
        <rect x="45" y="20" width="110" height="52" fill="#3b4252" stroke="#8be9fd" stroke-width="1.5"/>
        <!-- Pronounced horizontal hull step line where internal solid floor meets wall -->
        <line x1="40" y1="46" x2="160" y2="46" stroke="#ff5555" stroke-width="3.5"/>
        <text x="100" y="88" fill="#ff8080" font-size="8.5" font-family="monospace" text-anchor="middle">Wall Speed/Shrinkage Jump at Deck Transition</text>
      "##,
      "FIX: Lock Outer Wall Speed + Inner-Outer-Inner Order",
      r##"
        <rect x="285" y="20" width="114" height="52" fill="#3b4252" stroke="#50fa7b" stroke-width="2"/>
        <line x1="285" y1="46" x2="399" y2="46" stroke="#8be9fd" stroke-width="1" stroke-dasharray="3,3"/>
        <text x="342" y="42" fill="#50fa7b" font-size="8" font-family="monospace" text-anchor="middle">Constant Wall Speed</text>
        <text x="342" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">dont_slow_down_outer_wall = 1 + Lube Z Screws</text>
      "##,
    ),
    "sym_corner_bulge" => (
      "DEFECT: Swollen Bulging 90° Corners (PA Too Low)",
      r##"
        <rect x="52" y="24" width="96" height="44" fill="#3b4252" stroke="#8be9fd" stroke-width="1.5"/>
        <circle cx="52" cy="24" r="6" fill="#ff5555"/>
        <circle cx="148" cy="24" r="6" fill="#ff5555"/>
        <circle cx="52" cy="68" r="6" fill="#ff5555"/>
        <circle cx="148" cy="68" r="6" fill="#ff5555"/>
        <text x="100" y="88" fill="#ff8080" font-size="8.5" font-family="monospace" text-anchor="middle">Rounded Overshoot Bulges at 90° Corners</text>
      "##,
      "FIX: Enable Pressure Advance (0.034–0.048) + Jerk 8",
      r##"
        <rect x="292" y="22" width="100" height="48" fill="#3b4252" stroke="#50fa7b" stroke-width="2.2"/>
        <text x="342" y="49" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">Crisp 90° Corners</text>
        <text x="342" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">Pressure Advance Relieves Corner Decel Pressure</text>
      "##,
    ),
    "sym_pillowing" => (
      "DEFECT: Bumps & Pinholes in Top Roof Over Infill",
      r##"
        <!-- Sparse infill vertical ribs -->
        <line x1="55" y1="45" x2="55" y2="72" stroke="#6272a4" stroke-width="2"/>
        <line x1="100" y1="45" x2="100" y2="72" stroke="#6272a4" stroke-width="2"/>
        <line x1="145" y1="45" x2="145" y2="72" stroke="#6272a4" stroke-width="2"/>
        <!-- Broken pillowing top roof -->
        <path d="M 35 45 Q 45 30 55 45 Q 77 56 100 45 Q 122 28 145 45" stroke="#ff5555" stroke-width="3" fill="none"/>
        <text x="100" y="88" fill="#ff8080" font-size="8.5" font-family="monospace" text-anchor="middle">Thin Top Shell Sagging &amp; Bubbling Over Infill</text>
      "##,
      "FIX: 5–6 Top Shell Layers (≥1.0mm) + ≥18% Infill",
      r##"
        <line x1="285" y1="45" x2="285" y2="72" stroke="#6272a4" stroke-width="2"/>
        <line x1="322" y1="45" x2="322" y2="72" stroke="#6272a4" stroke-width="2"/>
        <line x1="360" y1="45" x2="360" y2="72" stroke="#6272a4" stroke-width="2"/>
        <line x1="398" y1="45" x2="398" y2="72" stroke="#6272a4" stroke-width="2"/>
        <rect x="270" y="31" width="144" height="14" rx="2" fill="#50fa7b"/>
        <text x="342" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">5 Solid Top Layers (1.0mm) + 18% Gyroid Support</text>
      "##,
    ),
    "sym_infill_wall_gaps" => (
      "DEFECT: Pinholes Between Infill & Perimeter Wall",
      r##"
        <rect x="35" y="20" width="130" height="52" rx="6" fill="none" stroke="#8be9fd" stroke-width="4"/>
        <!-- Diagonal infill stopping short of outer wall -->
        <line x1="48" y1="62" x2="88" y2="28" stroke="#ffb86c" stroke-width="3"/>
        <line x1="72" y1="62" x2="112" y2="28" stroke="#ffb86c" stroke-width="3"/>
        <line x1="96" y1="62" x2="136" y2="28" stroke="#ffb86c" stroke-width="3"/>
        <circle cx="88" cy="26" r="3" fill="#ff5555"/>
        <circle cx="112" cy="26" r="3" fill="#ff5555"/>
        <text x="100" y="88" fill="#ff8080" font-size="8.5" font-family="monospace" text-anchor="middle">Infill Pulls Back Leaving Pinhole Trench</text>
      "##,
      "FIX: Infill/Wall Overlap = 22–25% + Arachne Engine",
      r##"
        <rect x="275" y="20" width="134" height="52" rx="6" fill="#2c354d" stroke="#50fa7b" stroke-width="4"/>
        <line x1="278" y1="68" x2="330" y2="22" stroke="#50fa7b" stroke-width="3.5"/>
        <line x1="305" y1="68" x2="357" y2="22" stroke="#50fa7b" stroke-width="3.5"/>
        <line x1="332" y1="68" x2="384" y2="22" stroke="#50fa7b" stroke-width="3.5"/>
        <text x="342" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">22% Overlap Anchors Infill Deep Into Inner Wall</text>
      "##,
    ),
    "sym_top_surface_scars" => (
      "DEFECT: Diagonal Nozzle Drag Scar Across Top Skin",
      r##"
        <rect x="35" y="20" width="130" height="52" rx="4" fill="#3b4252" stroke="#8be9fd" stroke-width="1.5"/>
        <!-- Ugly diagonal travel scratch -->
        <line x1="42" y1="66" x2="158" y2="26" stroke="#ff5555" stroke-width="3"/>
        <text x="100" y="88" fill="#ff8080" font-size="8.5" font-family="monospace" text-anchor="middle">Nozzle Dragging Across Finished Top Layer</text>
      "##,
      "FIX: Monotonic Line + Z-Hop on Top + Avoid Crossing",
      r##"
        <rect x="275" y="20" width="134" height="52" rx="4" fill="#3b4252" stroke="#50fa7b" stroke-width="2"/>
        <path d="M 285 60 Q 342 12 398 60" stroke="#f1fa8c" stroke-width="2" stroke-dasharray="4,2" fill="none"/>
        <text x="342" y="52" fill="#50fa7b" font-size="8" font-family="monospace" text-anchor="middle">Monotonic Uniform Skin</text>
        <text x="342" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">Always Retract &amp; Lift Over Top Surfaces</text>
      "##,
    ),
    "sym_weak_infill" => (
      "DEFECT: Broken / Stringy Internal Grid Infill",
      r##"
        <rect x="35" y="20" width="130" height="52" rx="4" fill="#181c28" stroke="#ff5555" stroke-width="1.5"/>
        <line x1="45" y1="26" x2="155" y2="66" stroke="#ff6b6b" stroke-width="2" stroke-dasharray="8,5"/>
        <line x1="155" y1="26" x2="45" y2="66" stroke="#ff6b6b" stroke-width="2" stroke-dasharray="6,6"/>
        <circle cx="100" cy="46" r="5" fill="#ff5555"/>
        <text x="100" y="88" fill="#ff8080" font-size="8.5" font-family="monospace" text-anchor="middle">Grid Collision &amp; Speed Outrunning Melt Zone</text>
      "##,
      "FIX: Gyroid / Crosshatch @ 180 mm/s + 0.45mm Width",
      r##"
        <rect x="275" y="20" width="134" height="52" rx="4" fill="#181c28" stroke="#50fa7b" stroke-width="1.5"/>
        <path d="M 282 36 Q 300 22, 318 36 T 354 36 T 390 36" stroke="#50fa7b" stroke-width="2.5" fill="none"/>
        <path d="M 282 56 Q 300 42, 318 56 T 354 56 T 390 56" stroke="#8be9fd" stroke-width="2.5" fill="none"/>
        <text x="342" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">Non-Crossing 0.45mm Gyroid @ Safe 180mm/s</text>
      "##,
    ),
    "sym_overhang_curl" => (
      "DEFECT: Steep Overhang Edge Curling Upward",
      r##"
        <polygon points="45,72 95,72 145,28 65,28" fill="#44475a" stroke="#8be9fd" stroke-width="1.5"/>
        <!-- Curled overhang lip hitting nozzle -->
        <path d="M 142 28 Q 155 16 148 14" stroke="#ff5555" stroke-width="4" fill="none"/>
        <text x="100" y="88" fill="#ff8080" font-size="8.5" font-family="monospace" text-anchor="middle">Uncooled Overhang Lip Curls Up Into Nozzle</text>
      "##,
      "FIX: 95–100% Overhang Fan + 10s Layer Time + Inner/Outer",
      r##"
        <polygon points="285,72 335,72 385,26 305,26" fill="#44475a" stroke="#50fa7b" stroke-width="2"/>
        <path d="M 412 36 L 388 36" stroke="#8be9fd" stroke-width="2.5"/>
        <text x="342" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">Inner-Then-Outer Wall + 100% Fan + 10s Cool Time</text>
      "##,
    ),
    "sym_poor_bridging" => (
      "DEFECT: Sagging / Drooping Strands Under Bridge",
      r##"
        <rect x="35" y="24" width="24" height="48" fill="#44475a" stroke="#8be9fd" stroke-width="1.5"/>
        <rect x="141" y="24" width="24" height="48" fill="#44475a" stroke="#8be9fd" stroke-width="1.5"/>
        <path d="M 59 28 Q 100 62 141 28" stroke="#ff5555" stroke-width="2.5" fill="none"/>
        <path d="M 59 32 Q 100 70 141 32" stroke="#ff5555" stroke-width="2" fill="none"/>
        <text x="100" y="88" fill="#ff8080" font-size="8.5" font-family="monospace" text-anchor="middle">Drooping Loops Underneath Horizontal Span</text>
      "##,
      "FIX: Bridge Flow = 0.92 (Tension) + 28 mm/s + 100% Fan",
      r##"
        <rect x="275" y="24" width="24" height="48" fill="#44475a" stroke="#50fa7b" stroke-width="1.5"/>
        <rect x="385" y="24" width="24" height="48" fill="#44475a" stroke="#50fa7b" stroke-width="1.5"/>
        <line x1="299" y1="27" x2="385" y2="27" stroke="#50fa7b" stroke-width="3.5"/>
        <text x="342" y="48" fill="#f1fa8c" font-size="8" font-family="monospace" text-anchor="middle">0.92 Flow Pulls Strand Taut</text>
        <text x="342" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">Taut Bridge Span • 28mm/s Freeze • 100% Fan</text>
      "##,
    ),
    "sym_support_scarring" => (
      "DEFECT: Supports Welded / Fused to Model Underside",
      r##"
        <rect x="40" y="22" width="120" height="22" fill="#44475a" stroke="#8be9fd" stroke-width="1.5"/>
        <!-- Support welded directly to part with zero Z gap -->
        <rect x="55" y="44" width="90" height="28" fill="#ff5555" opacity="0.6" stroke="#ff5555" stroke-width="1.5"/>
        <text x="100" y="88" fill="#ff8080" font-size="8.5" font-family="monospace" text-anchor="middle">Fused Support Interface Tears Model Surface</text>
      "##,
      "FIX: Top Z-Distance = 0.23mm + 3 Interface Layers + Tree",
      r##"
        <rect x="280" y="20" width="124" height="20" fill="#44475a" stroke="#50fa7b" stroke-width="2"/>
        <!-- Clean 0.23mm release gap -->
        <line x1="285" y1="43" x2="399" y2="43" stroke="#f1fa8c" stroke-width="2" stroke-dasharray="4,2"/>
        <path d="M 320 72 L 330 47 L 354 47 L 364 72" fill="#2c354d" stroke="#8be9fd" stroke-width="1.5"/>
        <text x="342" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">0.23mm Z-Gap + 3-Layer Roof Pops Off Cleanly</text>
      "##,
    ),
    "sym_weak_layers" => (
      "DEFECT: Horizontal Layer Splitting / Delamination",
      r##"
        <rect x="48" y="20" width="104" height="52" fill="#44475a" stroke="#8be9fd" stroke-width="1.5"/>
        <!-- Horizontal crack splitting wall -->
        <path d="M 48 45 L 95 42 L 152 47" stroke="#ff5555" stroke-width="3.5" fill="none"/>
        <text x="100" y="88" fill="#ff8080" font-size="8.5" font-family="monospace" text-anchor="middle">Cold Weld Zone / Over-Cooling Splits Layers</text>
      "##,
      "FIX: Nozzle +5°C + Max Fan ≤35% + 4 Wall Perimeters",
      r##"
        <rect x="290" y="20" width="104" height="52" fill="#44475a" stroke="#50fa7b" stroke-width="3"/>
        <rect x="296" y="25" width="92" height="42" fill="none" stroke="#8be9fd" stroke-width="1.5"/>
        <text x="342" y="49" fill="#50fa7b" font-size="8" font-family="monospace" text-anchor="middle">4 Walls + Deep Fusion</text>
        <text x="342" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">Nozzle +5°C + Fan Cap ≤35% Entangles Chains</text>
      "##,
    ),
    "sym_ams_color_bleed" => (
      "DEFECT: Dark Pigment Bleeding Into White Layer",
      r##"
        <rect x="42" y="20" width="116" height="52" rx="4" fill="#f8f8f2" stroke="#6272a4" stroke-width="1.5"/>
        <rect x="42" y="42" width="116" height="12" fill="#ff5555" opacity="0.45"/>
        <text x="100" y="88" fill="#ff8080" font-size="8.5" font-family="monospace" text-anchor="middle">Residual Nozzle Pigment Staining Light Layer</text>
      "##,
      "FIX: 40mm Prime Tower (45 mm³) + 1.25x Flush Matrix",
      r##"
        <rect x="275" y="20" width="76" height="52" rx="4" fill="#f8f8f2" stroke="#50fa7b" stroke-width="2"/>
        <rect x="368" y="26" width="42" height="46" fill="#44475a" stroke="#8be9fd" stroke-width="1.5"/>
        <text x="389" y="52" fill="#f1fa8c" font-size="7" font-family="monospace" text-anchor="middle">40mm</text>
        <text x="342" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">Crisp Color Swap • Stable 40mm Prime Tower</text>
      "##,
    ),
    "sym_transparent_glass" => (
      "DEFECT: Cloudy / Frosted Micro-Voids & Grid Haze",
      r##"
        <rect x="38" y="20" width="124" height="52" rx="5" fill="#2c354d" opacity="0.85" stroke="#ff5555" stroke-width="1.5"/>
        <line x1="48" y1="26" x2="152" y2="66" stroke="#f8f8f2" stroke-width="1.5" opacity="0.5"/>
        <line x1="152" y1="26" x2="48" y2="66" stroke="#f8f8f2" stroke-width="1.5" opacity="0.5"/>
        <circle cx="76" cy="44" r="3" fill="#ff5555"/>
        <circle cx="100" cy="36" r="2.5" fill="#ff5555"/>
        <circle cx="124" cy="50" r="3" fill="#ff5555"/>
        <text x="100" y="88" fill="#ff8080" font-size="8.5" font-family="monospace" text-anchor="middle">Air Micro-Voids + 90° Crossing Infill Scatter Light</text>
      "##,
      "FIX: 20mm/s + 1.01 Flow + 0% Fan + 100% Aligned 0°",
      r##"
        <rect x="276" y="20" width="132" height="52" rx="5" fill="#8be9fd" opacity="0.22" stroke="#50fa7b" stroke-width="2"/>
        <line x1="284" y1="32" x2="400" y2="32" stroke="#8be9fd" stroke-width="2" opacity="0.65"/>
        <line x1="284" y1="45" x2="400" y2="45" stroke="#8be9fd" stroke-width="2" opacity="0.65"/>
        <line x1="284" y1="58" x2="400" y2="58" stroke="#8be9fd" stroke-width="2" opacity="0.65"/>
        <text x="342" y="48" fill="#f1fa8c" font-size="7.5" font-family="monospace" text-anchor="middle">💎 Homogeneous Fused Glass</text>
        <text x="342" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">Parallel 0° Aligned • +5% Over-Pack • 0% Fan</text>
      "##,
    ),
    "sym_weather_climate" => (
      "DEFECT: Tropical Humidity Stringing & Cold-Room Warping",
      r##"
        <rect x="34" y="20" width="132" height="52" rx="4" fill="#2c354d" stroke="#ff5555" stroke-width="1.5"/>
        <line x1="52" y1="30" x2="148" y2="36" stroke="#ff6b6b" stroke-width="1.2" stroke-dasharray="3,2"/>
        <line x1="52" y1="42" x2="148" y2="38" stroke="#ff6b6b" stroke-width="1.2" stroke-dasharray="2,2"/>
        <path d="M 38 70 Q 65 56 100 68 Q 135 56 162 70" stroke="#ffb86c" stroke-width="2.5" fill="none"/>
        <text x="100" y="54" fill="#ff8080" font-size="7.5" font-family="monospace" text-anchor="middle">🌴 80% RH Steam Ooze • ❄️ &lt;18°C Corner Lift</text>
        <text x="100" y="88" fill="#ff8080" font-size="8.5" font-family="monospace" text-anchor="middle">Tropical Heat Creep/Strings vs Cold Draft Warping</text>
      "##,
      "FIX: Temp & Humidity Dials (Tropical / Hot / Cold / Humid)",
      r##"
        <rect x="274" y="20" width="136" height="52" rx="4" fill="#1e2738" stroke="#50fa7b" stroke-width="2"/>
        <text x="342" y="37" fill="#50fa7b" font-size="8" font-family="monospace" text-anchor="middle">🌴 Tropical: -15% MVS • Spiral Z • Wipe</text>
        <text x="342" y="51" fill="#8be9fd" font-size="8" font-family="monospace" text-anchor="middle">❄️ Cold: 0% Exhaust • +5°C Bed • Brim</text>
        <text x="342" y="64" fill="#f1fa8c" font-size="7.5" font-family="monospace" text-anchor="middle">🎛️ Ambient °C &amp; % RH Dials</text>
        <text x="342" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">Auto-Tunes PLA, Silk, PETG, TPU, ABS/ASA &amp; PA/PC</text>
      "##,
    ),
    "cal_tpu_foaming_shore" => (
      "PHYSICS: Active Micro-Cellular Foaming (190°C → 250°C)",
      r##"
        <rect x="34" y="20" width="132" height="52" rx="4" fill="#2c354d" stroke="#8be9fd" stroke-width="1.5"/>
        <text x="100" y="36" fill="#f1fa8c" font-size="7.5" font-family="monospace" text-anchor="middle">195°C = 92A Solid (1.0x • 0.98 Flow)</text>
        <text x="100" y="50" fill="#8be9fd" font-size="7.5" font-family="monospace" text-anchor="middle">220°C = 75A Midsole (1.3x • 0.75 Flow)</text>
        <text x="100" y="64" fill="#50fa7b" font-size="7.5" font-family="monospace" text-anchor="middle">245°C = 60A Max Foam (1.6x • 0.60 Flow)</text>
        <text x="100" y="88" fill="#8be9fd" font-size="8.5" font-family="monospace" text-anchor="middle">Higher Temp Expands Closed-Cell Bubbles</text>
      "##,
      "TUNER: Shore A Dial (55A–92A) + Expansion Flow",
      r##"
        <rect x="274" y="20" width="136" height="52" rx="4" fill="#1e2738" stroke="#50fa7b" stroke-width="2"/>
        <text x="342" y="37" fill="#50fa7b" font-size="8" font-family="monospace" text-anchor="middle">🧽 Target Shore A ➔ Nozzle °C &amp; Flow</text>
        <text x="342" y="51" fill="#8be9fd" font-size="8" font-family="monospace" text-anchor="middle">⏱️ MVS Ceiling Locks Residence Time</text>
        <text x="342" y="64" fill="#f1fa8c" font-size="7.5" font-family="monospace" text-anchor="middle">🚫 0.00mm Z-Hop • Wipe + Avoid Walls</text>
        <text x="342" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">Compensates Nozzle Offset (+5°C HS / -10°C PCD)</text>
      "##,
    ),
    _ => (
      "CALIBRATION PRINT: Slicer Built-In Test Model",
      r##"
        <rect x="30" y="22" width="140" height="50" rx="6" fill="#1e2738" stroke="#8be9fd" stroke-width="1.5"/>
        <rect x="42" y="32" width="28" height="30" rx="3" fill="#44475a" stroke="#f1fa8c" stroke-width="1"/>
        <rect x="86" y="32" width="28" height="30" rx="3" fill="#50fa7b" opacity="0.4" stroke="#50fa7b" stroke-width="2"/>
        <rect x="130" y="32" width="28" height="30" rx="3" fill="#44475a" stroke="#f1fa8c" stroke-width="1"/>
        <text x="100" y="50" fill="#50fa7b" font-size="7.5" font-family="monospace" text-anchor="middle">-0.02</text>
        <text x="100" y="88" fill="#8be9fd" font-size="8.5" font-family="monospace" text-anchor="middle">Inspect Test Chips / Tower &amp; Enter Reading</text>
      "##,
      "AUTO-GENERATED LUA RULE: Exact Formula Output",
      r##"
        <rect x="265" y="22" width="154" height="50" rx="6" fill="#1e2738" stroke="#50fa7b" stroke-width="1.5"/>
        <text x="342" y="44" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">Flow / PA / Temp / MVS</text>
        <text x="342" y="58" fill="#f1fa8c" font-size="8" font-family="monospace" text-anchor="middle">Computed &amp; Scoped in Lua</text>
        <text x="342" y="88" fill="#50fa7b" font-size="8.5" font-family="monospace" text-anchor="middle">1-Click Add to Active Transformation Stack</text>
      "##,
    ),
  };

  let svg_markup = if compact {
    format!(
      r##"<svg viewBox="0 0 200 94" class="w-full h-24 rounded-lg bg-[#0d111a] border border-outline-variant/60 shrink-0">
        <text x="100" y="12" fill="#8be9fd" font-size="7.5" font-family="monospace" font-weight="bold" text-anchor="middle">{defect_title}</text>
        {defect_svg}
      </svg>"##
    )
  } else {
    format!(
      r##"<svg viewBox="0 0 460 96" class="w-full h-32 md:h-36 rounded-xl bg-[#0d111a] border border-tertiary/50 shadow-inner">
        <text x="100" y="12" fill="#ff8080" font-size="8" font-family="monospace" font-weight="bold" text-anchor="middle">{defect_title}</text>
        {defect_svg}
        <line x1="225" y1="8" x2="225" y2="88" stroke="#44475a" stroke-width="1.5" stroke-dasharray="4,3"/>
        <text x="342" y="12" fill="#50fa7b" font-size="8" font-family="monospace" font-weight="bold" text-anchor="middle">{fix_title}</text>
        {fix_svg}
      </svg>"##
    )
  };

  Html::from_html_unchecked(AttrValue::from(svg_markup))
}
