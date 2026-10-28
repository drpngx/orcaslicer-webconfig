use crate::models::ProcessProfile;
use crate::orca_profile::{
  DiffItem, OrcaProfile, SettingValueKind, WizardActionOp, WizardRuleAction, WizardSettingDef,
};
use crate::transformations::{RuleBeforeAfterChange, TransformationEngine, TransformationRule};
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

  // State for the interactive "Create / Add New Lua Rule" Builder & Simple Visual Setting Wizard
  let show_rule_creator = use_state(|| false);
  let creator_mode = use_state(|| "visual_wizard".to_string()); // "visual_wizard" or "preset_templates"
  let setting_catalog = use_memo((), |_| OrcaProfile::get_wizard_setting_catalog());
  let search_query = use_state(String::new);
  let section_filter = use_state(|| "All".to_string());
  let selected_setting_key = use_state(|| "nozzle_temperature".to_string());
  let selected_op = use_state(|| WizardActionOp::AddOffset);
  let custom_operand = use_state(|| "+5".to_string());
  let staged_actions = use_state(Vec::<WizardRuleAction>::new);

  let templates = use_memo((), |_| TransformationEngine::get_rule_templates());
  let selected_template_idx = use_state(|| 0usize);
  let draft_name = use_state(|| "Wizard Rule: Print Temperature +5 °C".to_string());
  let draft_desc = use_state(|| {
    "Created with the Simple Setting Rule Wizard (add/offset or set slicer parameters).".to_string()
  });
  let draft_category = use_state(|| "Visual Wizard Rule".to_string());
  let draft_icon = use_state(|| "🪄".to_string());
  let draft_condition = use_state(|| "all".to_string());
  let draft_lua = use_state(|| {
    templates
      .first()
      .map(|t| t.lua_code.clone())
      .unwrap_or_default()
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

  let active_rule = props
    .rules
    .iter()
    .find(|r| r.id == *selected_rule_id)
    .or_else(|| props.rules.first());

  let active_count = props.rules.iter().filter(|r| r.enabled).count();
  let total_count = props.rules.len();
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

        // Interactive Rule Creator: Simple Visual Setting Search Wizard + Preset Library Drawer
        {if *show_rule_creator {
          let templates_list = (*templates).clone();
          let cur_idx = *selected_template_idx;
          let is_visual_mode = *creator_mode == "visual_wizard";
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
                      {"Simple Setting Rule Wizard & Preset Library"}
                    </span>
                    <span class="text-xs font-mono text-secondary">
                      {"Auto-Generates Sandboxed Lua Transformations"}
                    </span>
                  </div>
                  <h3 class="font-headline-sm text-headline-sm text-on-surface font-bold mt-0.5">
                    {"Search Slicer Settings to Add/Offset or Set Values — or Pick a Preset Template"}
                  </h3>
                  <p class="text-xs text-on-surface-variant">
                    {"Filter by section, search any OrcaSlicer setting (e.g. Print Temperature +5°C, Wall Generator = arachne, Scarf Seam = all), and combine one or more setting tweaks into a single rule."}
                  </p>
                </div>

                // Mode Toggle: Simple Visual Setting Wizard vs Curated Preset Templates
                <div class="flex items-center bg-surface-container-lowest p-1 rounded-xl border border-outline-variant font-mono text-xs">
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
                    {"🪄 Simple Setting Search Wizard"}
                  </button>
                  <button
                    class={format!(
                      "px-space-md py-1.5 rounded-lg transition-colors cursor-pointer {}",
                      if !is_visual_mode {
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
                    {"📚 Curated Preset Templates"}
                  </button>
                </div>
              </div>

              {if is_visual_mode {
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
                            if let Some(existing) = list.iter_mut().find(|x| x.key == s_clone.key) {
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

                    // Staged Setting Actions Summary Bar
                    <div class="flex flex-col gap-1.5 bg-surface-container-lowest p-space-md rounded-xl border border-primary/50">
                      <div class="flex items-center justify-between">
                        <span class="text-[11px] font-mono uppercase text-primary font-bold">
                          {format!("Staged Setting Modifications in This Rule ({}):", staged_actions.len())}
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
                            {"Click any setting option above (e.g. '+5 °C' on Print Temperature, '= arachne' on Wall Generator Engine, or '= all' on Scarf Joint Seam Mode) to stage it into this rule."}
                          </span>
                        }
                      } else {
                        html! {
                          <div class="flex flex-wrap gap-1.5 mt-1">
                            {for staged_actions.iter().map(|act| {
                              let rm_key = act.key.clone();
                              let sa_rm = staged_actions.clone();
                              let d_name = draft_name.clone();
                              let d_desc = draft_desc.clone();
                              let d_lua = draft_lua.clone();

                              html! {
                                <div class="flex items-center gap-1.5 bg-primary/15 border border-primary text-on-surface px-2.5 py-1 rounded-lg text-xs font-mono">
                                  <span class="font-bold text-primary">{act.op.short_symbol()}</span>
                                  <span>{act.human_summary()}</span>
                                  <button
                                    class="text-on-surface-variant hover:text-error ml-1 cursor-pointer font-bold"
                                    onclick={Callback::from(move |_| {
                                      let mut list = (*sa_rm).clone();
                                      list.retain(|x| x.key != rm_key);
                                      let summaries: Vec<String> = list.iter().map(|a| a.human_summary()).collect();
                                      let auto_title = if summaries.is_empty() {
                                        "Custom Wizard Rule".to_string()
                                      } else {
                                        format!("Wizard: {}", summaries.join(" & "))
                                      };
                                      let auto_lua = WizardRuleAction::generate_lua_script(&list, &auto_title);
                                      d_name.set(auto_title);
                                      d_desc.set(format!("Visual Rule Wizard transformation: {}.", summaries.join("; ")));
                                      d_lua.set(auto_lua);
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

              // Step 3: Lua Code Editor
              <div class="flex flex-col gap-1">
                <label class="text-[11px] font-mono uppercase text-on-surface-variant font-bold">
                  {"5. Lua Transformation Script (function apply(config, ctx) ... end):"}
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
                  class="flex items-center gap-1.5 bg-primary text-on-primary hover:bg-primary/90 px-space-lg py-2 rounded-lg text-xs font-mono font-bold shadow-md cursor-pointer"
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
                    let next_num = props.rules.len() + 1;

                    Callback::from(move |_| {
                      let slug: String = name_val
                        .chars()
                        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
                        .collect::<String>()
                        .trim_matches('_')
                        .to_string();
                      let rule_id = format!("user_{}_{}", if slug.is_empty() { "rule" } else { &slug }, next_num);
                      let new_rule = TransformationRule {
                        id: rule_id.clone(),
                        name: name_val.clone(),
                        description: desc_val.clone(),
                        category: cat_val.clone(),
                        icon: icon_val.clone(),
                        enabled: true,
                        condition_expr: cond_val.clone(),
                        lua_code: lua_val.clone(),
                        priority: 50 + (next_num as i32 * 2),
                      };
                      cb_create.emit(new_rule);
                      sel_state.set(rule_id);
                      src.set(false);
                    })
                  }
                >
                  <span class="material-symbols-outlined text-[16px]">{"check_circle"}</span>
                  {"Add Rule to Transformation Stack"}
                </button>
              </div>
            </div>
          }
        } else {
          html! {}
        }}

        <div class="grid grid-cols-1 lg:grid-cols-2 gap-space-lg">
          {for props.rules.iter().map(|rule| {
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
                          <span class="text-[10px] uppercase font-mono text-secondary bg-surface-container-highest px-1.5 py-0.5 rounded">
                            {&rule.category}
                          </span>
                          <span class="text-[10px] font-mono text-primary bg-primary/10 border border-primary/30 px-1.5 py-0.5 rounded">
                            {format!("Scope: {}", rule.condition_expr)}
                          </span>
                          <span class="text-[10px] font-mono text-on-surface-variant">
                            {format!("transformations/{}.lua", rule.id)}
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
                          {"Before → After Changes by This Rule:"}
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

                  <div class="flex items-center justify-between text-xs text-on-surface-variant pt-space-xs">
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

                    <div class="flex items-center gap-2">
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

        html! {
          <div class="flex flex-col gap-space-md bg-surface-container-low p-space-xl rounded-xl shadow-md border border-outline-variant">
            <div class="flex items-center justify-between flex-wrap gap-space-sm">
              <div class="flex items-center gap-space-sm">
                <span class="material-symbols-outlined text-secondary">{"terminal"}</span>
                <h3 class="font-headline-sm text-headline-sm text-on-surface font-semibold">
                  {format!("Live Lua Code Editor — transformations/{}.lua ({})", rule.id, rule.name)}
                </h3>
              </div>
              <span class="text-xs text-on-surface-variant font-mono">
                {"Edits re-run the entire Lua stack immediately"}
              </span>
            </div>

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
