use crate::components::nav::ActiveTab;
use crate::models::{MachineRig, ProcessProfile};
use crate::orca_profile::DiffItem;
use crate::transformations::{
  ExternalConflict, ParameterProvenance, PendingDiskUpdate, SafetySeverity, SafetyWarning,
  TransformationRule,
};
use web_sys::HtmlInputElement;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct HomeTabProps {
  pub is_config_loaded: bool,
  pub is_tauri: bool,
  pub bound_filename: String,
  pub bound_path_hint: String,
  pub detected_hardware_summary: String,
  pub scanned_file_count: usize,
  pub last_saved_time: String,
  pub on_load_reference_snapshot: Callback<()>,
  pub on_import_zip: Callback<Event>,
  pub on_backup_zip: Callback<()>,
  pub on_export_webconfig_json: Callback<()>,
  pub on_import_webconfig_json: Callback<Event>,
  pub on_open_export_modal: Callback<()>,
  pub on_reset_all: Callback<()>,
  pub on_commit_save: Callback<()>,
  pub on_export_3mf: Callback<()>,
  pub on_export_standalone_json: Callback<()>,
  pub on_switch_tab: Callback<ActiveTab>,
  pub diff_items: Vec<DiffItem>,
  pub provenance_items: Vec<ParameterProvenance>,
  pub rules: Vec<TransformationRule>,
  pub on_toggle_rule: Callback<String>,
  pub conflicts: Vec<ExternalConflict>,
  pub safety_warnings: Vec<SafetyWarning>,
  pub pending_updates: Vec<PendingDiskUpdate>,
  pub on_resolve_conflict: Callback<(String, bool)>,
  pub show_diff_drawer: bool,
  pub on_toggle_diff_drawer: Callback<()>,
  pub active_rig: MachineRig,
  pub processes: Vec<ProcessProfile>,
  pub active_process_id: String,
  pub on_select_process: Callback<String>,
  pub safe_speed_ceiling: f64,
  pub json_diff_text: String,
  pub base_profile_json: String,
  pub derived_profile_json: String,
  pub our_internal_config_json: String,
  pub user_disk_config_json: String,
}

#[function_component]
pub fn HomeTab(props: &HomeTabProps) -> Html {
  let on_toggle_rule = props.on_toggle_rule.clone();
  let on_resolve_conflict = props.on_resolve_conflict.clone();
  let on_select_process = props.on_select_process.clone();
  let diff_view_mode = use_state(|| "provenance".to_string()); // "provenance", "github", or "side_by_side"
  let diff_only_changes = use_state(|| false);
  let provenance_filter = use_state(|| "modified".to_string()); // "modified", "multi_step", or "all"
  let provenance_search = use_state(String::new);
  let expanded_override_key = use_state(|| Option::<String>::None);

  if !props.is_config_loaded {
    return html! {
      <div class="flex flex-col items-center justify-center bg-surface-container-low p-space-xl rounded-2xl border border-outline-variant gap-space-md text-center my-8">
        <span class="material-symbols-outlined text-primary text-5xl">{"folder_managed"}</span>
        <h2 class="font-headline-lg text-headline-lg text-on-surface font-bold">
          {"Nothing Loaded — Use the Startup Import Modal"}
        </h2>
        <p class="text-body-md text-on-surface-variant max-w-xl">
          {"The application state is completely clean. Use the Import Modal (or click Reset All in the top right) to import your configuration directory."}
        </p>
      </div>
    };
  }

  let active_proc = props
    .processes
    .iter()
    .find(|p| p.id == props.active_process_id)
    .cloned()
    .unwrap_or_else(|| ProcessProfile::get_default_processes().remove(0));

  let changed_pending_updates: Vec<&PendingDiskUpdate> = props
    .pending_updates
    .iter()
    .filter(|u| u.has_value_change())
    .collect();

  let danger_count = props
    .safety_warnings
    .iter()
    .filter(|w| w.severity == SafetySeverity::Danger)
    .count();
  let warning_count = props
    .safety_warnings
    .iter()
    .filter(|w| w.severity == SafetySeverity::Warning)
    .count();

  let modified_count = props
    .provenance_items
    .iter()
    .filter(|p| p.is_modified || !p.steps.is_empty())
    .count();
  let multi_step_count = props
    .provenance_items
    .iter()
    .filter(|p| p.steps.len() >= 2)
    .count();
  let total_step_events: usize = props.provenance_items.iter().map(|p| p.steps.len()).sum();
  let q_lower = provenance_search.trim().to_lowercase();
  let filtered_prov: Vec<&ParameterProvenance> = props
    .provenance_items
    .iter()
    .filter(|p| match provenance_filter.as_str() {
      "multi_step" => p.steps.len() >= 2,
      "all" => true,
      _ => p.is_modified || !p.steps.is_empty(),
    })
    .filter(|p| {
      if q_lower.is_empty() {
        return true;
      }
      p.param_key.to_lowercase().contains(&q_lower)
        || p.param_label.to_lowercase().contains(&q_lower)
        || p.category.to_lowercase().contains(&q_lower)
        || p.steps.iter().any(|s| {
          s.rule_name.to_lowercase().contains(&q_lower)
            || s.rule_category.to_lowercase().contains(&q_lower)
            || s.rule_log.to_lowercase().contains(&q_lower)
        })
    })
    .collect();

  html! {
    <div class="flex flex-col w-full gap-space-lg">
      // Auto-Detected Hardware & Directory Backup Banner
      <div class="bg-surface-container-low border border-primary/50 p-space-md rounded-xl shadow-md flex flex-wrap items-center justify-between gap-space-md">
        <div class="flex items-center gap-space-md">
          <div class="w-10 h-10 rounded-lg bg-primary-container/30 border border-primary flex items-center justify-center text-primary">
            <span class="material-symbols-outlined">{"verified"}</span>
          </div>
          <div class="flex flex-col">
            <div class="flex items-center gap-space-xs">
              <span class="text-[10px] font-mono uppercase bg-primary text-on-primary px-2 py-0.5 rounded font-bold">
                {"Auto-Detected From Disk"}
              </span>
              <span class="text-xs font-mono text-tertiary">
                {format!("{} files scanned in directory", props.scanned_file_count)}
              </span>
            </div>
            <span class="font-headline-sm text-body-md text-on-surface font-bold mt-0.5">
              {&props.detected_hardware_summary}
            </span>
          </div>
        </div>

        <div class="flex items-center gap-space-sm flex-wrap">
          <button
            class="flex items-center gap-space-xs bg-primary hover:opacity-90 text-on-primary px-space-md py-1.5 rounded-lg text-xs font-mono font-bold transition-opacity shadow cursor-pointer"
            title={if props.is_tauri { "Open Save & Export menu" } else { "Open Export menu" }}
            onclick={
              let cb = props.on_open_export_modal.clone();
              Callback::from(move |_| cb.emit(()))
            }
          >
            <span class="material-symbols-outlined text-[16px]">{"ios_share"}</span>
            {if props.is_tauri { "Save & Export" } else { "Export" }}
          </button>
          <button
            class="flex items-center gap-space-xs bg-error-container/30 hover:bg-error-container text-error border border-error/50 px-space-md py-1.5 rounded-lg text-xs font-mono font-bold transition-colors cursor-pointer"
            title="Clear everything and return to the startup Import Modal"
            onclick={
              let cb = props.on_reset_all.clone();
              Callback::from(move |_| cb.emit(()))
            }
          >
            <span class="material-symbols-outlined text-[16px]">{"restart_alt"}</span>
            {"Reset & Clear All"}
          </button>
        </div>
      </div>

      // Safety & Warning Validator Panel
      if !props.safety_warnings.is_empty() {
        <div class={format!(
          "p-space-lg rounded-xl shadow-lg flex flex-col gap-space-sm border-2 {}",
          if danger_count > 0 {
            "bg-red-950/25 border-red-500"
          } else {
            "bg-amber-950/15 border-amber-500/70"
          }
        )}>
          <div class="flex items-center justify-between flex-wrap gap-space-sm">
            <div class="flex items-center gap-space-sm">
              <span class="text-lg">
                {if danger_count > 0 { "🛑" } else { "⚠️" }}
              </span>
              <span class="font-headline-sm text-headline-sm font-bold text-on-surface">
                {"Safety & Warning Audit (Checked Across Imported Files & Active Stack)"}
              </span>
            </div>
            <div class="flex items-center gap-space-xs font-mono text-xs">
              if danger_count > 0 {
                <span class="bg-red-600 text-white border border-red-400 px-2.5 py-0.5 rounded-full font-bold flex items-center gap-1 shadow-sm">
                  <span>{"🛑"}</span>
                  <span>{format!("{} UNSAFE", danger_count)}</span>
                </span>
              }
              if warning_count > 0 {
                <span class="bg-amber-500/20 text-amber-300 border border-amber-400/60 px-2.5 py-0.5 rounded-full font-bold flex items-center gap-1">
                  <span>{"⚠️"}</span>
                  <span>{format!("{} {}", warning_count, if warning_count == 1 { "WARNING" } else { "WARNINGS" })}</span>
                </span>
              }
              <span class="text-[11px] text-on-surface-variant ml-1">
                {"Requires confirmation before Save / Export"}
              </span>
            </div>
          </div>

          <p class="text-body-sm text-on-surface-variant">
            {"Automated safety check across all imported machine/filament/process diffs, AnycubicSlicerNext.conf flushing matrices, and active Lua outputs. Any Export or Save to Disk will prompt a confirmation dialog while unsafe values or warnings remain."}
          </p>

          <div class="grid grid-cols-1 md:grid-cols-2 gap-space-sm mt-space-xs">
            {for props.safety_warnings.iter().map(|w| {
              let is_danger = w.severity == SafetySeverity::Danger;
              html! {
                <div class={format!(
                  "p-space-md rounded-lg border flex flex-col justify-between gap-space-xs {}",
                  if is_danger {
                    "bg-red-950/30 border-red-500/80"
                  } else {
                    "bg-surface-container border-amber-500/50"
                  }
                )}>
                  <div class="flex items-start justify-between gap-2">
                    <div class="flex flex-col">
                      <span class="text-[10px] font-mono uppercase text-on-surface-variant">
                        {&w.scope_label}
                      </span>
                      <span class="font-code-md text-code-md text-on-surface font-bold">
                        {&w.param_key}
                      </span>
                    </div>
                    <span class={format!(
                      "text-[10px] font-mono uppercase px-2 py-0.5 rounded font-bold shrink-0 flex items-center gap-1 {}",
                      if is_danger {
                        "bg-red-600 text-white border border-red-400 shadow-sm"
                      } else {
                        "bg-amber-500/20 text-amber-300 border border-amber-400/50"
                      }
                    )}>
                      <span>{if is_danger { "🛑" } else { "⚠️" }}</span>
                      <span>{if is_danger { "UNSAFE" } else { "WARNING" }}</span>
                    </span>
                  </div>

                  <div class="grid grid-cols-2 gap-space-xs bg-surface-container-lowest p-2 rounded font-mono text-xs">
                    <div class="flex flex-col">
                      <span class="text-[10px] uppercase text-on-surface-variant">{"Flagged Value:"}</span>
                      <span class={if is_danger { "text-red-400 font-bold" } else { "text-amber-300 font-bold" }}>
                        {&w.current_value}
                      </span>
                    </div>
                    <div class="flex flex-col">
                      <span class="text-[10px] uppercase text-on-surface-variant">{"Safe / Expected Range:"}</span>
                      <span class="text-tertiary font-bold">{&w.safe_range}</span>
                    </div>
                  </div>

                  <p class="text-[11px] text-on-surface-variant leading-relaxed">
                    {&w.explanation}
                  </p>
                </div>
              }
            })}
          </div>
        </div>
      }

      // Re-Created Configs vs Disk Differences Banner (Requirement #4)
      if !props.conflicts.is_empty() {
        <div class="bg-error-container/15 border border-error p-space-lg rounded-xl shadow-lg flex flex-col gap-space-sm">
          <div class="flex items-center justify-between flex-wrap gap-space-sm">
            <div class="flex items-center gap-space-sm text-error">
              <span class="material-symbols-outlined">{"rule"}</span>
              <span class="font-headline-sm text-headline-sm font-bold">
                {"Re-Created Configs via Transformations: Flagged Disk One-Off Differences"}
              </span>
            </div>
            <div class="flex items-center gap-space-sm">
              <span class="text-xs bg-error/20 text-error px-2.5 py-0.5 rounded-full font-mono font-bold">
                {format!("{} parameter(s) differ from clean base + rules", props.conflicts.len())}
              </span>
              <button
                class="bg-error text-on-error hover:opacity-90 px-space-md py-1 rounded text-xs font-bold transition-opacity cursor-pointer"
                onclick={
                  let cb = props.on_reset_all.clone();
                  Callback::from(move |_| cb.emit(()))
                }
              >
                {"Cancel Import & Reset"}
              </button>
            </div>
          </div>
          <p class="text-body-sm text-on-surface-variant">
            {"We reconstructed your disk profiles using system base profiles + Lua transformation rules (Hardened Steel +5°C thermal rule & +5°C PETG bed rule). Where your disk files contain additional one-off tweaks, you can accept them as new Lua transformation rules in our stack, discard them to keep the clean rule value, or cancel and reset."}
          </p>
          <div class="grid grid-cols-1 md:grid-cols-2 gap-space-md mt-space-xs">
            {for props.conflicts.iter().map(|c| {
              let k_accept = c.param_key.clone();
              let k_discard = c.param_key.clone();
              let cb_accept = on_resolve_conflict.clone();
              let cb_discard = on_resolve_conflict.clone();

              html! {
                <div class="bg-surface-container p-space-md rounded-lg flex flex-col justify-between border border-outline-variant gap-space-sm">
                  <div class="flex justify-between items-start gap-2">
                    <div class="flex flex-col">
                      <span class="font-code-md text-code-md text-on-surface font-semibold">
                        {&c.param_label}
                      </span>
                      <span class="font-code-sm text-code-sm text-on-surface-variant font-mono">
                        {format!("Key: {} (System Base: {})", c.param_key, c.base_value)}
                      </span>
                    </div>
                    <span class="text-[11px] text-primary font-mono text-right">{&c.modifying_rule_id}</span>
                  </div>

                  <div class="grid grid-cols-2 gap-space-sm bg-surface-container-lowest p-space-sm rounded font-code-sm text-code-sm">
                    <div class="flex flex-col">
                      <span class="text-on-surface-variant text-[10px] uppercase font-bold">
                        {"User Disk Config Value:"}
                      </span>
                      <span class="text-error font-bold">{&c.actual_external_value}</span>
                    </div>
                    <div class="flex flex-col">
                      <span class="text-on-surface-variant text-[10px] uppercase font-bold">
                        {"Re-Created by Lua Rule:"}
                      </span>
                      <span class="text-tertiary font-bold">{&c.expected_derived_value}</span>
                    </div>
                  </div>

                  <div class="flex items-center gap-space-sm justify-end pt-space-xs">
                    <button
                      class="bg-primary-container hover:bg-primary text-on-primary-container px-space-md py-1 rounded text-xs font-semibold transition-colors cursor-pointer"
                      onclick={Callback::from(move |_| cb_accept.emit((k_accept.clone(), true)))}
                    >
                      {"Accept One-Off as Lua Transform"}
                    </button>
                    <button
                      class="bg-surface-container-high hover:bg-surface-bright text-on-surface px-space-md py-1 rounded text-xs font-semibold transition-colors cursor-pointer"
                      onclick={Callback::from(move |_| cb_discard.emit((k_discard.clone(), false)))}
                    >
                      {"Keep Clean Rule Value (Stage for Save)"}
                    </button>
                  </div>
                </div>
              }
            })}
          </div>
        </div>
      }

      // Staged Clean Rule Values Ready to Save to Disk Banner (only shown in Tauri mode when values actually differ!)
      if props.is_tauri && !changed_pending_updates.is_empty() {
        <div class="bg-primary-container/15 border-2 border-primary p-space-lg rounded-xl shadow-lg flex flex-col gap-space-sm">
          <div class="flex items-center justify-between flex-wrap gap-space-sm">
            <div class="flex items-center gap-space-sm text-primary">
              <span class="material-symbols-outlined">{"pending_actions"}</span>
              <span class="font-headline-sm text-headline-sm font-bold">
                {format!(
                  "{} Staged Clean Rule Value(s) Ready to Save to Disk",
                  changed_pending_updates.len()
                )}
              </span>
            </div>
            <button
              class="flex items-center gap-space-xs bg-primary-container hover:bg-primary text-on-primary-container px-space-lg py-2 rounded-lg text-xs font-mono font-bold shadow-lg transition-colors cursor-pointer"
              onclick={
                let cb = props.on_commit_save.clone();
                Callback::from(move |_| cb.emit(()))
              }
            >
              <span class="material-symbols-outlined text-[16px]">{"save"}</span>
              {format!("Save {} Staged Change(s) to Disk Now", changed_pending_updates.len())}
            </button>
          </div>
          <p class="text-body-sm text-on-surface-variant">
            {"You selected 'Keep Clean Rule Value' for the parameters below. Your disk files have NOT been modified yet — click 'Save Staged Change(s) to Disk Now' above (or 'Save to Disk' in the top bar) to write these clean rule values to your local configuration files."}
          </p>
          <div class="grid grid-cols-1 md:grid-cols-2 gap-space-sm mt-space-xs">
            {for changed_pending_updates.iter().map(|u| {
              html! {
                <div class="bg-surface-container p-space-sm rounded-lg border border-primary/40 flex flex-col gap-1 font-mono text-xs">
                  <div class="flex items-center justify-between">
                    <span class="text-on-surface font-bold">{&u.param_label}</span>
                    <span class="bg-primary/20 text-primary px-2 py-0.5 rounded text-[10px] font-bold">
                      {"Pending Save"}
                    </span>
                  </div>
                  <span class="text-[11px] text-on-surface-variant truncate">
                    {format!("File: {}", u.source_relative_path)}
                  </span>
                  <div class="flex items-center gap-2 text-[11px] mt-0.5">
                    <span class="text-error line-through">{format!("Disk: {}", u.old_disk_value)}</span>
                    <span class="text-on-surface-variant">{"→"}</span>
                    <span class="text-tertiary font-bold">{format!("Clean Rule: {}", u.new_clean_value)}</span>
                  </div>
                </div>
              }
            })}
          </div>
        </div>
      }

      // 1-Click Loadout & Dual-Export Strategy Banner (No scattered import buttons here!)
      <div class="grid grid-cols-1 lg:grid-cols-12 gap-space-lg">
        // Left 7 cols: Active Directory Status & Dual-Export Actions
        <div class="lg:col-span-7 bg-surface-container-low p-space-lg rounded-xl shadow-md border border-outline-variant flex flex-col justify-between gap-space-md">
          <div class="flex flex-col gap-space-xs">
            <div class="flex items-center justify-between flex-wrap gap-space-sm">
              <div class="flex items-center gap-space-sm">
                <span class="material-symbols-outlined text-primary">{"link"}</span>
                <h2 class="font-headline-sm text-headline-sm text-on-surface font-semibold">
                  {"Active Config Source & Dual-Export"}
                </h2>
                <span class="bg-tertiary-container/30 text-tertiary text-xs px-2 py-0.5 rounded-full font-mono flex items-center gap-1">
                  <span class="w-1.5 h-1.5 rounded-full bg-tertiary animate-ping"></span>
                  {"Local FS / ZIP Active"}
                </span>
              </div>

              <div class="flex items-center gap-space-xs flex-wrap">
                <button
                  class="flex items-center gap-space-xs bg-primary-container hover:bg-primary text-on-primary-container px-space-md py-space-xs rounded text-xs font-bold transition-colors shadow cursor-pointer"
                  onclick={
                    let cb = props.on_open_export_modal.clone();
                    Callback::from(move |_| cb.emit(()))
                  }
                >
                  <span class="material-symbols-outlined text-[14px]">{"ios_share"}</span>
                  if props.is_tauri {
                    if !props.pending_updates.is_empty() {
                      {format!("Save & Export ({} Staged)", props.pending_updates.len())}
                    } else {
                      {"Save & Export"}
                    }
                  } else {
                    {"Export"}
                  }
                </button>
              </div>
            </div>

            <div class="text-body-sm text-on-surface-variant font-mono mt-1 bg-surface-container-lowest p-space-sm rounded border border-outline-variant flex items-center justify-between">
              <span class="truncate">{&props.bound_path_hint}</span>
              <span class="text-xs text-on-surface-variant shrink-0 ml-2">
                {format!("Status: {}", props.last_saved_time)}
              </span>
            </div>
          </div>

          // Footer controls: Local FS Refresh + Import/Export ZIP Backup
          <div class="flex items-center justify-between flex-wrap gap-space-md pt-space-xs border-t border-surface-container">
            <span class="text-xs text-on-surface-variant font-mono">
              {"Sources: Local Filesystem Bridge • Start-From-Scratch Wizard • ZIP Archive"}
            </span>
            <div class="flex items-center gap-space-sm flex-wrap">
              <label class="flex items-center gap-1 bg-surface-container-highest hover:bg-surface-bright text-secondary border border-secondary/40 px-2.5 py-1 rounded text-xs font-mono font-semibold cursor-pointer transition-colors">
                <span class="material-symbols-outlined text-[14px]">{"upload_file"}</span>
                {"Import .zip"}
                <input
                  type="file"
                  accept=".zip"
                  class="hidden"
                  onchange={props.on_import_zip.clone()}
                />
              </label>
              if props.is_tauri {
                <button
                  class="flex items-center gap-1 text-xs text-primary hover:underline font-semibold cursor-pointer"
                  onclick={
                    let cb = props.on_load_reference_snapshot.clone();
                    Callback::from(move |_| cb.emit(()))
                  }
                >
                  <span class="material-symbols-outlined text-[14px]">{"sync"}</span>
                  {"Refresh Local FS"}
                </button>
              }
            </div>
          </div>

          <div class="bg-amber-500/10 border border-amber-400/40 rounded-lg px-space-md py-space-sm text-[11px] text-on-surface-variant flex flex-col gap-0.5">
            <span class="font-mono font-bold text-amber-300">
              {"⚠️ Slicer Import Note: .zip Config Import vs. .3mf Flushing Volumes"}
            </span>
            <span>
              {"OrcaSlicer's "}
              <span class="font-mono text-on-surface">{"File → Import → Import Configs... (.zip)"}</span>
              {" only loads Machine/Filament/Process "}
              <span class="font-mono text-on-surface">{".json"}</span>
              {" presets and ignores "}
              <span class="font-mono text-on-surface">{".conf"}</span>
              {" (where "}
              <span class="font-mono text-on-surface">{"flush_volumes_matrix"}</span>
              {" is stored). To import flushing volumes into the slicer, use "}
              <strong class="text-on-surface">{"Export .3mf"}</strong>
              {" (or "}
              <strong class="text-on-surface">{"Save to Disk"}</strong>
              {" while the slicer is closed), and set "}
              <span class="font-mono text-amber-200">{"Preferences → Control → Behaviour → \"Auto flush after changing...\" = None"}</span>
              {" so auto-recalculate does not overwrite them."}
            </span>
          </div>
        </div>

        // Right 5 cols: Three Modular Pillars Summary (Hardware + Process + Safe Speed Ceiling)
        <div class="lg:col-span-5 bg-surface-container-low p-space-lg rounded-xl shadow-md border border-outline-variant flex flex-col justify-between gap-space-md">
          <div class="flex items-start justify-between">
            <div class="flex flex-col">
              <span class="text-xs uppercase tracking-wider text-primary font-bold">
                {"Pillar 1 & 3: Hardware + Process Combination"}
              </span>
              <h3 class="font-headline-sm text-headline-sm text-on-surface font-semibold">
                {props.active_rig.format_concise_title()}
              </h3>
              if props.active_rig.name != props.active_rig.format_concise_title() {
                <span class="text-[11px] font-mono text-on-surface-variant truncate">
                  {format!("Disk Preset: {}", props.active_rig.name)}
                </span>
              }
            </div>
            <button
              class="text-xs text-secondary hover:underline flex items-center gap-0.5 cursor-pointer"
              onclick={
                let cb = props.on_switch_tab.clone();
                Callback::from(move |_| cb.emit(ActiveTab::Machine))
              }
            >
              {"Duplicate / Mod Rig"}
              <span class="material-symbols-outlined text-[14px]">{"arrow_forward"}</span>
            </button>
          </div>

          <div class="grid grid-cols-3 gap-space-xs bg-surface-container-lowest p-space-sm rounded text-xs font-mono text-center">
            <div class="flex flex-col">
              <span class="text-on-surface-variant text-[10px] uppercase">{"Nozzle"}</span>
              <span class="text-primary font-medium">
                {format!("{} {}mm", props.active_rig.nozzle_material, props.active_rig.nozzle_diameter)}
              </span>
            </div>
            <div class="flex flex-col">
              <span class="text-on-surface-variant text-[10px] uppercase">{"Add-Ons"}</span>
              <span class="text-on-surface font-medium">
                {format!("{} active", props.active_rig.addons.len())}
              </span>
            </div>
            <div class="flex flex-col">
              <span class="text-on-surface-variant text-[10px] uppercase">{"Hotend Cap"}</span>
              <span class="text-secondary font-medium">
                {format!("{:.1} mm³/s", props.active_rig.max_volumetric_flow_limit)}
              </span>
            </div>
          </div>

          // Process Export Summary & Notes
          <div class="flex flex-col gap-space-xs">
            <div class="flex items-center justify-between text-xs flex-wrap gap-1 bg-surface-container-lowest px-space-sm py-1.5 rounded-lg border border-outline-variant/60">
              <span class="text-on-surface-variant font-semibold">
                {"Exported Process Suite:"}
              </span>
              <span class="font-mono text-tertiary text-[11px]">
                {"All 5 Presets Auto-Exported (0.12 HQ · 0.20 HQ · 0.20 Std [Default .3mf] · 0.20 Spd · 0.28 Spd)"}
              </span>
            </div>
            if !props.active_rig.notes.is_empty() || !active_proc.notes.is_empty() {
              <div class="grid grid-cols-1 md:grid-cols-2 gap-space-xs mt-1">
                <div class="bg-surface-container-lowest p-space-xs rounded border border-outline-variant/50 flex flex-col gap-0.5">
                  <span class="text-[10px] uppercase font-mono text-primary font-bold">
                    {"Machine Notes (printer_notes):"}
                  </span>
                  <pre class="text-[10px] font-mono text-on-surface-variant whitespace-pre-wrap leading-tight max-h-24 overflow-y-auto">
                    {&props.active_rig.notes}
                  </pre>
                </div>
                <div class="bg-surface-container-lowest p-space-xs rounded border border-outline-variant/50 flex flex-col gap-0.5">
                  <span class="text-[10px] uppercase font-mono text-tertiary font-bold">
                    {"Process Notes (notes):"}
                  </span>
                  <pre class="text-[10px] font-mono text-on-surface-variant whitespace-pre-wrap leading-tight max-h-24 overflow-y-auto">
                    {&active_proc.notes}
                  </pre>
                </div>
              </div>
            }
          </div>
        </div>
      </div>

      // Dual Config Architecture View: Our Internal Config Data Structure vs User Disk Configs
      <div class="bg-surface-container-low p-space-lg rounded-xl shadow-md border border-outline-variant flex flex-col gap-space-md">
        <div class="flex items-center justify-between flex-wrap gap-space-sm">
          <div class="flex items-center gap-space-sm">
            <span class="material-symbols-outlined text-primary">{"compare_arrows"}</span>
            <div class="flex flex-col">
              <h3 class="font-headline-sm text-headline-sm text-on-surface font-semibold">
                {"Two Distinct Config Models: Our Transformation Data Structure vs User Disk Configs"}
              </h3>
              <span class="text-xs text-on-surface-variant">
                {"Left: Our internal relational/transformation state (Rig + Add-ons + Base Deltas + Lua Rules). Right: The flat OrcaSlicer/AnycubicSlicerNext JSON files on disk."}
              </span>
            </div>
          </div>
        </div>

        <div class="grid grid-cols-1 lg:grid-cols-2 gap-space-md">
          <div class="flex flex-col gap-1 bg-surface-container-lowest p-space-md rounded-lg border border-primary/40">
            <div class="flex items-center justify-between text-xs font-mono pb-1 border-b border-surface-container">
              <span class="text-primary font-bold uppercase">
                {"1. Our Config (Transformation Stack & Staged Clean State)"}
              </span>
              <span class="text-on-surface-variant">{"orcaslicer_webconfig_state.json"}</span>
            </div>
            <pre class="text-[11px] font-mono text-on-surface overflow-x-auto max-h-56 whitespace-pre mt-1">
              {&props.our_internal_config_json}
            </pre>
          </div>

          <div class="flex flex-col gap-1 bg-surface-container-lowest p-space-md rounded-lg border border-secondary/40">
            <div class="flex items-center justify-between text-xs font-mono pb-1 border-b border-surface-container">
              <span class="text-secondary font-bold uppercase">
                {"2. User Config on Disk (Actual Disk Files — Updates on Save)"}
              </span>
              if !props.pending_updates.is_empty() {
                <span class="bg-error/20 text-error px-2 py-0.5 rounded text-[10px] font-bold">
                  {format!("{} Unsaved Staged Change(s)", props.pending_updates.len())}
                </span>
              } else {
                <span class="text-on-surface-variant">{&props.bound_filename}</span>
              }
            </div>
            <pre class="text-[11px] font-mono text-on-surface overflow-x-auto max-h-56 whitespace-pre mt-1">
              {&props.user_disk_config_json}
            </pre>
          </div>
        </div>
      </div>

      // Middle Grid: Computed Overrides Table (Left) + Quick Symptom Resolver (Right)
      <div class="grid grid-cols-1 lg:grid-cols-12 gap-space-lg items-start">
        // Computed Overrides vs Stock Defaults Table (8 cols)
        <div class="lg:col-span-8 flex flex-col gap-space-md bg-surface-container-low p-space-lg rounded-xl shadow-md border border-outline-variant">
          <div class="flex items-center justify-between flex-wrap gap-space-sm">
            <div class="flex items-center gap-space-sm">
              <span class="material-symbols-outlined text-secondary">{"tune"}</span>
              <div class="flex flex-col">
                <h3 class="font-headline-sm text-headline-sm text-on-surface font-semibold">
                  {"Re-Created Overrides vs System Base Defaults"}
                </h3>
                <span class="text-[11px] text-on-surface-variant">
                  {"Click any parameter row to inspect its step-by-step Provenance & Lineage chain inline, or open the full Provenance & Lineage view below."}
                </span>
              </div>
            </div>
            <div class="flex items-center gap-space-sm flex-wrap">
              <div class="flex items-center gap-1.5 bg-surface-container-lowest px-2 py-1 rounded-lg border border-outline-variant font-mono text-[11px]">
                <span class="text-on-surface-variant">{"Previewing Process Preset:"}</span>
                {for props.processes.iter().map(|proc_item| {
                  let is_sel = proc_item.id == active_proc.id;
                  let target_pid = proc_item.id.clone();
                  let cb = on_select_process.clone();
                  let short_label = format!("{:.2} {}", proc_item.layer_height, match proc_item.quality_tier() {
                    "high_quality" => "HQ",
                    "high_speed" => "Spd",
                    _ => "Std",
                  });
                  html! {
                    <button
                      class={format!(
                        "px-1.5 py-0.5 rounded transition-colors cursor-pointer {}",
                        if is_sel {
                          "bg-primary text-on-primary font-bold"
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
                      {short_label}
                    </button>
                  }
                })}
              </div>
              <span class="text-xs text-on-surface-variant font-mono">
                {format!(
                  "{} modified parameters ({} active Lua rules)",
                  props.diff_items.iter().filter(|d| d.is_modified).count(),
                  props.rules.iter().filter(|r| r.enabled).count()
                )}
              </span>
              <button
                class="bg-tertiary/15 hover:bg-tertiary/25 text-tertiary border border-tertiary/50 px-2.5 py-1 rounded text-xs font-mono font-bold transition-colors cursor-pointer flex items-center gap-1"
                onclick={
                  let dvm = diff_view_mode.clone();
                  let sdd = props.on_toggle_diff_drawer.clone();
                  let is_open = props.show_diff_drawer;
                  Callback::from(move |_| {
                    dvm.set("provenance".to_string());
                    if !is_open {
                      sdd.emit(());
                    }
                  })
                }
              >
                <span>{"🧬"}</span>
                <span>{"Provenance & Lineage View"}</span>
              </button>
            </div>
          </div>

          <div class="flex flex-col overflow-x-auto">
            // Header
            <div class="grid grid-cols-12 px-space-md py-space-xs bg-surface-container-lowest text-on-surface-variant font-label-caps text-label-caps uppercase rounded">
              <div class="col-span-4">{"Parameter"}</div>
              <div class="col-span-2">{"Category"}</div>
              <div class="col-span-2 text-right">{"System Base"}</div>
              <div class="col-span-2 text-right">{"Re-Created"}</div>
              <div class="col-span-2 text-right">{"Provenance / Rule Chain"}</div>
            </div>

            // Rows
            <div class="flex flex-col gap-1 mt-1">
              {for props.diff_items.iter().map(|item| {
                let row_bg = if item.is_modified {
                  "bg-surface-container border-l-2 border-primary"
                } else {
                  "bg-surface-container/40 opacity-70"
                };

                let unit_suffix = if item.unit.is_empty() {
                  String::new()
                } else {
                  format!(" {}", item.unit)
                };

                let prov_opt = props
                  .provenance_items
                  .iter()
                  .find(|p| p.param_key == item.key);
                let step_count = prov_opt.map(|p| p.steps.len()).unwrap_or(0);
                let is_expanded = expanded_override_key.as_deref() == Some(item.key.as_str());
                let key_for_toggle = item.key.clone();
                let exp_state = expanded_override_key.clone();

                html! {
                  <div class="flex flex-col rounded overflow-hidden">
                    <div
                      class={format!("grid grid-cols-12 px-space-md py-space-sm items-center hover:bg-surface-container-high transition-colors font-code-sm text-code-sm cursor-pointer {}", row_bg)}
                      onclick={Callback::from(move |_| {
                        if exp_state.as_deref() == Some(key_for_toggle.as_str()) {
                          exp_state.set(None);
                        } else {
                          exp_state.set(Some(key_for_toggle.clone()));
                        }
                      })}
                    >
                      <div class="col-span-4 flex flex-col">
                        <div class="flex items-center gap-1.5">
                          <span class="text-on-surface font-medium">{&item.label}</span>
                          if step_count > 0 {
                            <span class={format!(
                              "px-1.5 py-0.2 rounded text-[10px] font-mono font-bold border {}",
                              if step_count >= 2 {
                                "bg-tertiary/20 text-tertiary border-tertiary/50"
                              } else {
                                "bg-primary/20 text-primary border-primary/40"
                              }
                            )}>
                              {format!("🧬 {} {}", step_count, if step_count == 1 { "step" } else { "steps" })}
                            </span>
                          }
                        </div>
                        <span class="text-on-surface-variant text-[10px] font-mono">{&item.key}</span>
                      </div>
                      <div class="col-span-2 text-on-surface-variant text-xs">{&item.category}</div>
                      <div class="col-span-2 text-right text-on-surface-variant font-mono">
                        {format!("{}{}", item.base_value, unit_suffix)}
                      </div>
                      <div class={format!("col-span-2 text-right font-mono font-bold {}", if item.is_modified { "text-primary" } else { "text-on-surface" })}>
                        {format!("{}{}", item.override_value, unit_suffix)}
                      </div>
                      <div class="col-span-2 text-right text-xs truncate text-secondary flex items-center justify-end gap-1" title={item.source_rule.clone()}>
                        <span class="truncate">{&item.source_rule}</span>
                        <span class="material-symbols-outlined text-[14px] text-on-surface-variant shrink-0">
                          {if is_expanded { "expand_less" } else { "expand_more" }}
                        </span>
                      </div>
                    </div>

                    if is_expanded {
                      if let Some(prov) = prov_opt {
                        <div class="bg-surface-container-lowest border-x border-b border-primary/40 p-space-sm flex flex-col gap-space-xs">
                          <div class="flex items-center justify-between flex-wrap gap-2 text-[11px] font-mono border-b border-outline-variant/40 pb-1">
                            <span class="text-tertiary font-bold uppercase">
                              {format!("🧬 Parameter Provenance & Lineage: {} ({})", prov.param_label, prov.param_key)}
                            </span>
                            <span class="bg-emerald-950/60 text-emerald-300 border border-emerald-500/40 px-2 py-0.5 rounded font-bold">
                              {format!("Net Δ: {}", prov.net_delta_summary)}
                            </span>
                          </div>
                          <div class="flex items-center flex-wrap gap-2 pt-1 text-xs font-mono">
                            <div class="bg-surface-container px-2.5 py-1.5 rounded border border-outline-variant flex flex-col">
                              <span class="text-[9px] uppercase text-on-surface-variant">{"Stage 0 • Baseline Origin"}</span>
                              <span class="text-on-surface font-bold">{&prov.origin_value}</span>
                              <span class="text-[9px] text-on-surface-variant truncate max-w-[220px]">{&prov.origin_source}</span>
                            </div>
                            {for prov.steps.iter().map(|st| html! {
                              <>
                                <span class="text-primary font-bold">{"➔"}</span>
                                <div class="bg-surface-container-high px-2.5 py-1.5 rounded border border-primary/50 flex flex-col gap-0.5">
                                  <div class="flex items-center gap-1 text-[10px]">
                                    <span>{if st.rule_icon.is_empty() { "⚙️" } else { &st.rule_icon }}</span>
                                    <span class="text-primary font-bold">{format!("Step #{}", st.step_number)}</span>
                                    <span class="text-on-surface font-semibold">{&st.rule_name}</span>
                                  </div>
                                  <div class="flex items-center gap-1.5 text-[11px]">
                                    <span class="text-red-300 line-through">{&st.before_value}</span>
                                    <span class="text-on-surface-variant">{"→"}</span>
                                    <span class="text-emerald-300 font-bold">{&st.after_value}</span>
                                    <span class="bg-primary/20 text-primary px-1.5 py-0.2 rounded text-[10px]">
                                      {&st.delta_summary}
                                    </span>
                                  </div>
                                  if !st.rule_log.is_empty() {
                                    <span class="text-[10px] text-on-surface-variant italic truncate max-w-md" title={st.rule_log.clone()}>
                                      {format!("↳ {}", st.rule_log)}
                                    </span>
                                  }
                                </div>
                              </>
                            })}
                            <span class="text-emerald-400 font-bold">{"="}</span>
                            <div class="bg-emerald-950/40 px-2.5 py-1.5 rounded border border-emerald-500/60 flex flex-col">
                              <span class="text-[9px] uppercase text-emerald-300">{"Final Derived Value"}</span>
                              <span class="text-emerald-300 font-bold">{&prov.final_value}</span>
                              <span class="text-[9px] text-emerald-200/80">{&prov.net_delta_summary}</span>
                            </div>
                          </div>
                        </div>
                      }
                    }
                  </div>
                }
              })}
            </div>
          </div>
        </div>

        // Quick Symptom & Dynamic Rule Resolver (4 cols)
        <div class="lg:col-span-4 flex flex-col gap-space-md bg-surface-container-low p-space-lg rounded-xl shadow-md border border-outline-variant">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-space-sm">
              <span class="material-symbols-outlined text-primary">{"healing"}</span>
              <h3 class="font-headline-sm text-headline-sm text-on-surface font-semibold">
                {"Lua Transformation Stack"}
              </h3>
            </div>
            <button
              class="text-xs text-secondary hover:underline cursor-pointer"
              onclick={
                let cb = props.on_switch_tab.clone();
                Callback::from(move |_| cb.emit(ActiveTab::Wizard))
              }
            >
              {"Edit Lua Scripts"}
            </button>
          </div>

          <p class="text-body-sm text-on-surface-variant">
            {"Toggle dynamic combination rules, hardware add-on schedules, and accepted one-offs:"}
          </p>

          <div class="flex flex-col gap-space-sm mt-space-xs">
            {for props.rules.iter().map(|rule| {
              let rid = rule.id.clone();
              let cb = on_toggle_rule.clone();
              let is_enabled = rule.enabled;

              html! {
                <div
                  class={format!(
                    "p-space-md rounded-lg flex flex-col gap-space-xs border transition-all cursor-pointer {}",
                    if is_enabled {
                      "bg-surface-container border-primary shadow-sm"
                    } else {
                      "bg-surface-container-lowest border-outline-variant hover:border-surface-bright"
                    }
                  )}
                  onclick={Callback::from(move |_| cb.emit(rid.clone()))}
                >
                  <div class="flex items-center justify-between">
                    <div class="flex items-center gap-space-sm">
                      <span class="text-lg">{&rule.icon}</span>
                      <div class="flex flex-col">
                        <span class="font-headline-sm text-body-md text-on-surface font-semibold">
                          {&rule.name}
                        </span>
                        <span class="text-[10px] font-mono text-secondary uppercase">
                          {&rule.category}
                        </span>
                      </div>
                    </div>
                    <div
                      class={format!(
                        "w-5 h-5 rounded flex items-center justify-center text-xs {}",
                        if is_enabled {
                          "bg-primary text-on-primary font-bold"
                        } else {
                          "bg-surface-container-highest text-transparent"
                        }
                      )}
                    >
                      {if is_enabled { "✓" } else { "" }}
                    </div>
                  </div>
                  <p class="text-xs text-on-surface-variant line-clamp-2 mt-1">
                    {&rule.description}
                  </p>
                </div>
              }
            })}
          </div>
        </div>
      </div>

      // JSON Diff & Parameter Provenance / Lineage Preview
      <div class="bg-surface-container-low p-space-lg rounded-xl shadow-md border border-outline-variant flex flex-col gap-space-md">
        <div class="flex items-center justify-between flex-wrap gap-space-sm">
          <div
            class="flex items-center gap-space-sm cursor-pointer"
            onclick={
              let cb = props.on_toggle_diff_drawer.clone();
              Callback::from(move |_| cb.emit(()))
            }
          >
            <span class="material-symbols-outlined text-secondary">{"difference"}</span>
            <h3 class="font-headline-sm text-headline-sm text-on-surface font-semibold">
              {"Diff & Parameter Provenance / Lineage Inspector (System Base vs Re-Created Output)"}
            </h3>
            <span class="material-symbols-outlined text-on-surface-variant text-[18px]">
              {if props.show_diff_drawer { "expand_less" } else { "expand_more" }}
            </span>
          </div>

          <div class="flex items-center gap-space-sm flex-wrap">
            // View Mode Toggle: Provenance & Lineage vs GitHub / Unified vs Side-by-Side
            <div class="flex items-center bg-surface-container-lowest p-0.5 rounded-lg border border-outline-variant font-mono text-xs">
              <button
                class={format!(
                  "px-2.5 py-1 rounded transition-colors cursor-pointer flex items-center gap-1 {}",
                  if *diff_view_mode == "provenance" {
                    "bg-primary text-on-primary font-bold"
                  } else {
                    "text-on-surface-variant hover:text-on-surface"
                  }
                )}
                onclick={
                  let dvm = diff_view_mode.clone();
                  let sdd = props.on_toggle_diff_drawer.clone();
                  let is_open = props.show_diff_drawer;
                  Callback::from(move |_| {
                    dvm.set("provenance".to_string());
                    if !is_open {
                      sdd.emit(());
                    }
                  })
                }
              >
                <span>{"🧬"}</span>
                <span>{"Provenance & Lineage"}</span>
              </button>
              <button
                class={format!(
                  "px-2.5 py-1 rounded transition-colors cursor-pointer {}",
                  if *diff_view_mode == "github" {
                    "bg-primary text-on-primary font-bold"
                  } else {
                    "text-on-surface-variant hover:text-on-surface"
                  }
                )}
                onclick={
                  let dvm = diff_view_mode.clone();
                  let sdd = props.on_toggle_diff_drawer.clone();
                  let is_open = props.show_diff_drawer;
                  Callback::from(move |_| {
                    dvm.set("github".to_string());
                    if !is_open {
                      sdd.emit(());
                    }
                  })
                }
              >
                {"GitHub / Unified"}
              </button>
              <button
                class={format!(
                  "px-2.5 py-1 rounded transition-colors cursor-pointer {}",
                  if *diff_view_mode == "side_by_side" {
                    "bg-primary text-on-primary font-bold"
                  } else {
                    "text-on-surface-variant hover:text-on-surface"
                  }
                )}
                onclick={
                  let dvm = diff_view_mode.clone();
                  let sdd = props.on_toggle_diff_drawer.clone();
                  let is_open = props.show_diff_drawer;
                  Callback::from(move |_| {
                    dvm.set("side_by_side".to_string());
                    if !is_open {
                      sdd.emit(());
                    }
                  })
                }
              >
                {"Side-by-Side"}
              </button>
            </div>

            if *diff_view_mode != "provenance" {
              // Filter Toggle: All Lines vs Changed Only
              <button
                class={format!(
                  "px-2.5 py-1 rounded-lg border font-mono text-xs transition-colors cursor-pointer {}",
                  if *diff_only_changes {
                    "bg-secondary/20 text-secondary border-secondary font-bold"
                  } else {
                    "bg-surface-container-lowest text-on-surface-variant border-outline-variant hover:text-on-surface"
                  }
                )}
                onclick={
                  let doc = diff_only_changes.clone();
                  let sdd = props.on_toggle_diff_drawer.clone();
                  let is_open = props.show_diff_drawer;
                  Callback::from(move |_| {
                    doc.set(!*doc);
                    if !is_open {
                      sdd.emit(());
                    }
                  })
                }
              >
                {if *diff_only_changes { "Showing: Changed Lines Only" } else { "Showing: Full JSON" }}
              </button>
            }

            if props.is_tauri {
              <button
                class="flex items-center gap-space-xs bg-primary hover:bg-primary-fixed-dim text-on-primary-fixed px-space-md py-space-xs rounded text-xs font-semibold transition-colors cursor-pointer"
                onclick={
                  let cb = props.on_commit_save.clone();
                  Callback::from(move |_| cb.emit(()))
                }
              >
                <span class="material-symbols-outlined text-[14px]">{"save"}</span>
                {"Commit Changes to Local Disk"}
              </button>
            }
          </div>
        </div>

        if props.show_diff_drawer {
          if *diff_view_mode == "provenance" {
            <div class="bg-surface-container-lowest rounded-lg p-space-md border border-outline-variant flex flex-col gap-space-md">
                  // Top Controls & Summary Bar
                  <div class="flex items-center justify-between flex-wrap gap-space-sm border-b border-outline-variant/50 pb-space-sm">
                    <div class="flex flex-col gap-0.5">
                      <span class="text-xs font-mono font-bold text-primary uppercase">
                        {"🧬 Parameter Provenance & Lineage Pipeline (Stage 0 Origin ➔ Successive Lua Rule Steps ➔ Final Derived Value)"}
                      </span>
                      <span class="text-[11px] text-on-surface-variant">
                        {format!(
                          "Traces how each parameter starts from its System Base + Spool/Process Baseline and transforms step-by-step across {} total rule evaluations ({} multi-rule parameter chains).",
                          total_step_events, multi_step_count
                        )}
                      </span>
                    </div>

                    <div class="flex items-center gap-space-xs flex-wrap">
                      {for [
                        ("modified", format!("Modified Lineage ({})", modified_count)),
                        ("multi_step", format!("Multi-Rule Chains ≥2 ({})", multi_step_count)),
                        ("all", format!("All Catalog Params ({})", props.provenance_items.len())),
                      ].into_iter().map(|(fid, flabel)| {
                        let is_sel = *provenance_filter == fid;
                        let pf = provenance_filter.clone();
                        let fid_str = fid.to_string();
                        html! {
                          <button
                            class={format!(
                              "px-2.5 py-1 rounded text-xs font-mono border transition-colors cursor-pointer {}",
                              if is_sel {
                                "bg-primary/20 text-primary border-primary font-bold"
                              } else {
                                "bg-surface-container text-on-surface-variant border-outline-variant hover:text-on-surface"
                              }
                            )}
                            onclick={Callback::from(move |_| pf.set(fid_str.clone()))}
                          >
                            {flabel}
                          </button>
                        }
                      })}

                      <input
                        type="text"
                        placeholder="Filter parameter, key, or Lua rule..."
                        value={(*provenance_search).clone()}
                        class="bg-surface-container px-2.5 py-1 rounded border border-outline-variant text-xs font-mono text-on-surface w-60"
                        oninput={
                          let ps = provenance_search.clone();
                          Callback::from(move |e: InputEvent| {
                            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                              ps.set(input.value());
                            }
                          })
                        }
                      />
                    </div>
                  </div>

                  if filtered_prov.is_empty() {
                    <div class="p-space-lg text-center text-xs font-mono text-on-surface-variant bg-surface-container/30 rounded border border-outline-variant/40">
                      {"No parameters match the current Provenance & Lineage filter. Try switching to 'Modified Lineage' or 'All Catalog Params' or enabling more Lua transformation rules."}
                    </div>
                  } else {
                    <div class="flex flex-col gap-space-sm max-h-[540px] overflow-y-auto pr-1">
                      {for filtered_prov.into_iter().map(|prov| {
                        let step_len = prov.steps.len();
                        html! {
                          <div class={format!(
                            "rounded-lg p-space-md border flex flex-col gap-space-sm transition-colors {}",
                            if step_len >= 2 {
                              "bg-surface-container border-tertiary/60 shadow-sm"
                            } else if prov.is_modified {
                              "bg-surface-container border-primary/40"
                            } else {
                              "bg-surface-container/40 border-outline-variant/50 opacity-80"
                            }
                          )}>
                            // Card Header: Parameter Label, Key, Category, Step Count & Net Delta
                            <div class="flex items-center justify-between flex-wrap gap-2 border-b border-outline-variant/40 pb-1.5">
                              <div class="flex items-center gap-2 flex-wrap">
                                <span class="font-headline-sm text-sm font-bold text-on-surface">
                                  {&prov.param_label}
                                </span>
                                <span class="bg-surface-container-lowest px-2 py-0.5 rounded text-[11px] font-mono text-secondary border border-outline-variant/60">
                                  {&prov.param_key}
                                </span>
                                <span class="bg-surface-container-high px-2 py-0.5 rounded text-[10px] font-mono uppercase text-on-surface-variant">
                                  {&prov.category}
                                </span>
                                if step_len >= 2 {
                                  <span class="bg-tertiary/20 text-tertiary border border-tertiary/50 px-2 py-0.5 rounded text-[10px] font-mono font-bold">
                                    {format!("⛓️ {}-Step Non-Commutative Chain", step_len)}
                                  </span>
                                } else if step_len == 1 {
                                  <span class="bg-primary/20 text-primary border border-primary/40 px-2 py-0.5 rounded text-[10px] font-mono font-bold">
                                    {"1-Step Transformation"}
                                  </span>
                                } else {
                                  <span class="bg-surface-container-highest text-on-surface-variant px-2 py-0.5 rounded text-[10px] font-mono">
                                    {"0 Steps (Baseline Retained)"}
                                  </span>
                                }
                              </div>

                              <div class="flex items-center gap-2 font-mono text-xs">
                                <span class="text-on-surface-variant">
                                  {format!("{} → {}", prov.origin_value, prov.final_value)}
                                </span>
                                <span class={format!(
                                  "px-2 py-0.5 rounded text-[11px] font-bold border {}",
                                  if prov.is_modified {
                                    "bg-emerald-950/60 text-emerald-300 border-emerald-500/50"
                                  } else {
                                    "bg-surface-container-lowest text-on-surface-variant border-outline-variant"
                                  }
                                )}>
                                  {format!("Net Δ: {}", prov.net_delta_summary)}
                                </span>
                              </div>
                            </div>

                            // Visual Step-by-Step Provenance & Lineage Pipeline
                            <div class="flex items-stretch flex-wrap gap-2 pt-0.5 font-mono text-xs">
                              // Stage 0: Origin Baseline Node
                              <div class="bg-surface-container-lowest p-2.5 rounded-lg border border-outline-variant flex flex-col justify-between min-w-[185px]">
                                <div class="flex items-center justify-between gap-2">
                                  <span class="text-[10px] uppercase font-bold text-on-surface-variant">
                                    {"Stage 0 • Origin Baseline"}
                                  </span>
                                  <span class="text-[10px] bg-surface-container px-1.5 py-0.2 rounded text-on-surface-variant">
                                    {"Input"}
                                  </span>
                                </div>
                                <div class="text-sm font-bold text-on-surface my-1 break-all">
                                  {&prov.origin_value}
                                </div>
                                <div class="text-[10px] text-on-surface-variant truncate max-w-[230px]" title={prov.origin_source.clone()}>
                                  {&prov.origin_source}
                                </div>
                              </div>

                              // Successive Lua Transformation Steps
                              {for prov.steps.iter().map(|step| {
                                html! {
                                  <>
                                    <div class="flex flex-col items-center justify-center px-1 text-primary select-none">
                                      <span class="text-[10px] font-bold">{format!("#{}", step.step_number)}</span>
                                      <span class="text-base leading-none">{"➔"}</span>
                                    </div>
                                    <div class="bg-surface-container-lowest p-2.5 rounded-lg border border-primary/50 flex flex-col justify-between min-w-[230px] flex-1">
                                      <div class="flex items-center justify-between gap-2 flex-wrap">
                                        <div class="flex items-center gap-1">
                                          <span>{if step.rule_icon.is_empty() { "⚙️" } else { &step.rule_icon }}</span>
                                          <span class="text-[10px] uppercase font-bold text-primary">
                                            {format!("Step #{} (Prio #{})", step.step_number, step.rule_priority)}
                                          </span>
                                        </div>
                                        if !step.rule_category.is_empty() {
                                          <span class="text-[9px] bg-secondary/15 text-secondary px-1.5 py-0.2 rounded uppercase">
                                            {&step.rule_category}
                                          </span>
                                        }
                                      </div>

                                      <div class="text-xs font-bold text-on-surface mt-0.5">
                                        {&step.rule_name}
                                      </div>

                                      <div class="flex items-center gap-1.5 my-1 flex-wrap">
                                        <span class="bg-red-950/50 text-red-300 border border-red-500/30 px-1.5 py-0.5 rounded text-[11px] line-through break-all">
                                          {&step.before_value}
                                        </span>
                                        <span class="text-on-surface-variant font-bold">{"→"}</span>
                                        <span class="bg-emerald-950/50 text-emerald-300 border border-emerald-500/40 px-1.5 py-0.5 rounded text-[11px] font-bold break-all">
                                          {&step.after_value}
                                        </span>
                                        <span class="bg-primary/20 text-primary border border-primary/40 px-1.5 py-0.5 rounded text-[10px] font-bold">
                                          {&step.delta_summary}
                                        </span>
                                      </div>

                                      if !step.rule_log.is_empty() {
                                        <div class="text-[10px] text-on-surface-variant bg-surface-container/60 px-2 py-1 rounded border border-outline-variant/40 mt-0.5 break-words">
                                          <span class="text-tertiary font-bold">{"Lua Audit: "}</span>
                                          <span>{&step.rule_log}</span>
                                        </div>
                                      }
                                    </div>
                                  </>
                                }
                              })}

                              // Final Derived Output Node
                              <div class="flex flex-col items-center justify-center px-1 text-emerald-400 select-none">
                                <span class="text-[10px] font-bold">{"Out"}</span>
                                <span class="text-base leading-none">{"="}</span>
                              </div>
                              <div class={format!(
                                "p-2.5 rounded-lg border flex flex-col justify-between min-w-[185px] {}",
                                if prov.is_modified {
                                  "bg-emerald-950/35 border-emerald-500/60"
                                } else {
                                  "bg-surface-container-lowest border-outline-variant"
                                }
                              )}>
                                <div class="flex items-center justify-between gap-2">
                                  <span class="text-[10px] uppercase font-bold text-emerald-300">
                                    {"Final Derived Output"}
                                  </span>
                                  <span class="text-[10px] bg-emerald-900/50 text-emerald-200 px-1.5 py-0.2 rounded font-bold">
                                    {format!("{} {}", step_len, if step_len == 1 { "rule" } else { "rules" })}
                                  </span>
                                </div>
                                <div class="text-sm font-bold text-emerald-300 my-1 break-all">
                                  {&prov.final_value}
                                </div>
                                <div class="text-[10px] text-emerald-200/90 font-semibold">
                                  {&prov.net_delta_summary}
                                </div>
                              </div>
                            </div>
                          </div>
                        }
                      })}
                    </div>
                  }
            </div>
          } else if *diff_view_mode == "side_by_side" {
            <div class="bg-surface-container-lowest rounded-lg font-mono text-xs overflow-x-auto max-h-96 border border-outline-variant">
              <div class="grid grid-cols-2 border-b border-surface-container bg-surface-container text-[11px] font-bold uppercase tracking-wider sticky top-0 z-10">
                <div class="px-space-md py-1.5 text-red-300 border-r border-surface-container flex items-center justify-between">
                  <span>{"System Base Profile (Before)"}</span>
                  <span class="text-[10px] opacity-80">{"- Base"}</span>
                </div>
                <div class="px-space-md py-1.5 text-emerald-300 flex items-center justify-between">
                  <span>{"Re-Created Output (After Lua Stack)"}</span>
                  <span class="text-[10px] opacity-80">{"+ Derived"}</span>
                </div>
              </div>
              <div class="divide-y divide-surface-container/40">
                {for props.base_profile_json.lines().zip(props.derived_profile_json.lines()).enumerate().filter_map(|(idx, (b_line, d_line))| {
                  let is_changed = b_line != d_line;
                  if *diff_only_changes && !is_changed {
                    return None;
                  }
                  let line_no = idx + 1;
                  Some(html! {
                    <div class="grid grid-cols-2">
                      <div class={format!(
                        "px-space-sm py-0.5 whitespace-pre overflow-x-auto border-r border-surface-container flex items-start gap-2 {}",
                        if is_changed {
                          "bg-red-950/45 text-red-300 border-l-2 border-l-red-500 font-semibold"
                        } else {
                          "text-on-surface-variant/80"
                        }
                      )}>
                        <span class="w-8 shrink-0 text-right text-[10px] opacity-50 select-none">{line_no}</span>
                        <span class="w-3 shrink-0 select-none">{if is_changed { "-" } else { " " }}</span>
                        <span>{b_line}</span>
                      </div>
                      <div class={format!(
                        "px-space-sm py-0.5 whitespace-pre overflow-x-auto flex items-start gap-2 {}",
                        if is_changed {
                          "bg-emerald-950/45 text-emerald-300 border-l-2 border-l-emerald-500 font-semibold"
                        } else {
                          "text-on-surface-variant/80"
                        }
                      )}>
                        <span class="w-8 shrink-0 text-right text-[10px] opacity-50 select-none">{line_no}</span>
                        <span class="w-3 shrink-0 select-none">{if is_changed { "+" } else { " " }}</span>
                        <span>{d_line}</span>
                      </div>
                    </div>
                  })
                })}
              </div>
            </div>
          } else {
            <div class="bg-surface-container-lowest rounded-lg font-mono text-xs overflow-x-auto max-h-96 border border-outline-variant divide-y divide-surface-container/30">
              {for props.json_diff_text.lines().enumerate().filter_map(|(idx, line)| {
                let is_del = line.starts_with("- ");
                let is_add = line.starts_with("+ ");
                if *diff_only_changes && !is_del && !is_add {
                  return None;
                }
                let row_cls = if is_del {
                  "bg-red-950/45 text-red-300 border-l-2 border-red-500 font-semibold"
                } else if is_add {
                  "bg-emerald-950/45 text-emerald-300 border-l-2 border-emerald-500 font-semibold"
                } else {
                  "text-on-surface-variant/80"
                };
                Some(html! {
                  <div class={format!("px-space-md py-0.5 whitespace-pre flex items-start gap-3 {}", row_cls)}>
                    <span class="w-8 shrink-0 text-right text-[10px] opacity-50 select-none">{idx + 1}</span>
                    <span>{line}</span>
                  </div>
                })
              })}
            </div>
          }
        }
      </div>
    </div>
  }
}
