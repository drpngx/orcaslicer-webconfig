use yew::prelude::*;
use crate::components::nav::ActiveTab;
use crate::orca_profile::DiffItem;
use crate::transformations::{ExternalConflict, TransformationRule};

#[derive(Properties, PartialEq)]
pub struct HomeTabProps {
    pub is_file_bound: bool,
    pub bound_filename: String,
    pub bound_path_hint: String,
    pub last_saved_time: String,
    pub disconnect_after_save: bool,
    pub on_toggle_disconnect_after_save: Callback<()>,
    pub on_disconnect_file: Callback<()>,
    pub on_rebind_file: Callback<()>,
    pub on_commit_save: Callback<()>,
    pub on_reset_defaults: Callback<()>,
    pub on_switch_tab: Callback<ActiveTab>,
    pub diff_items: Vec<DiffItem>,
    pub rules: Vec<TransformationRule>,
    pub on_toggle_rule: Callback<String>,
    pub conflicts: Vec<ExternalConflict>,
    pub on_resolve_conflict: Callback<(String, bool)>, // (param_key, accept: true = keep external, false = discard)
    pub show_diff_drawer: bool,
    pub on_toggle_diff_drawer: Callback<()>,
    pub active_rig_name: String,
    pub json_diff_text: String,
}

#[function_component]
pub fn HomeTab(props: &HomeTabProps) -> Html {
    let on_toggle_rule = props.on_toggle_rule.clone();
    let on_resolve_conflict = props.on_resolve_conflict.clone();

    html! {
        <div class="flex flex-col w-full gap-space-lg">
            // External Conflict / Drift Resolution Banner
            if !props.conflicts.is_empty() {
                <div class="bg-error-container/20 border border-error p-space-md rounded-xl shadow-lg flex flex-col gap-space-sm">
                    <div class="flex items-center justify-between">
                        <div class="flex items-center gap-space-sm text-error">
                            <span class="material-symbols-outlined">{"warning"}</span>
                            <span class="font-headline-sm text-headline-sm font-bold">{"External File Changes Detected (Config Drift)"}</span>
                        </div>
                        <span class="text-xs bg-error/20 text-error px-2 py-0.5 rounded-full font-mono">
                            {format!("{} conflicting parameter(s)", props.conflicts.len())}
                        </span>
                    </div>
                    <p class="text-body-sm text-on-surface-variant">
                        {"The bound OrcaSlicer configuration file was modified externally on disk. Below are the parameters where your transformation rules conflict with the external changes. Choose whether to accept the external values into your baseline or discard them."}
                    </p>
                    <div class="grid grid-cols-1 md:grid-cols-2 gap-space-md mt-space-xs">
                        {for props.conflicts.iter().map(|c| {
                            let k_accept = c.param_key.clone();
                            let k_discard = c.param_key.clone();
                            let cb_accept = on_resolve_conflict.clone();
                            let cb_discard = on_resolve_conflict.clone();

                            html! {
                                <div class="bg-surface-container p-space-md rounded-lg flex flex-col justify-between border border-outline-variant gap-space-sm">
                                    <div class="flex justify-between items-start">
                                        <div class="flex flex-col">
                                            <span class="font-code-md text-code-md text-on-surface font-semibold">{&c.param_label}</span>
                                            <span class="font-code-sm text-code-sm text-on-surface-variant font-mono">{format!("Key: {}", c.param_key)}</span>
                                        </div>
                                        <span class="text-xs text-primary font-mono">{&c.modifying_rule_id}</span>
                                    </div>

                                    <div class="grid grid-cols-2 gap-space-sm bg-surface-container-lowest p-space-sm rounded font-code-sm text-code-sm">
                                        <div class="flex flex-col">
                                            <span class="text-on-surface-variant text-[10px] uppercase font-bold">{"External File Found:"}</span>
                                            <span class="text-error font-bold">{&c.actual_external_value}</span>
                                        </div>
                                        <div class="flex flex-col">
                                            <span class="text-on-surface-variant text-[10px] uppercase font-bold">{"Rule Transformation:"}</span>
                                            <span class="text-tertiary font-bold">{&c.expected_derived_value}</span>
                                        </div>
                                    </div>

                                    <div class="flex items-center gap-space-sm justify-end pt-space-xs">
                                        <button 
                                            class="bg-surface-container-high hover:bg-surface-bright text-on-surface px-space-md py-1 rounded text-xs font-semibold transition-colors"
                                            onclick={Callback::from(move |_| cb_accept.emit((k_accept.clone(), true)))}
                                        >
                                            {"Accept External"}
                                        </button>
                                        <button 
                                            class="bg-primary-container hover:bg-primary text-on-primary-container px-space-md py-1 rounded text-xs font-semibold transition-colors"
                                            onclick={Callback::from(move |_| cb_discard.emit((k_discard.clone(), false)))}
                                        >
                                            {"Discard & Overwrite"}
                                        </button>
                                    </div>
                                </div>
                            }
                        })}
                    </div>
                </div>
            }

            // Top Status Bar / Connection & Rig Cards
            <div class="grid grid-cols-1 lg:grid-cols-12 gap-space-lg">
                // Left 7 cols: Connection & Bound Profile card
                <div class="lg:col-span-7 bg-surface-container-low p-space-lg rounded-xl shadow-md border border-outline-variant flex flex-col justify-between gap-space-md">
                    <div class="flex flex-col gap-space-xs">
                        <div class="flex items-center justify-between flex-wrap gap-space-sm">
                            <div class="flex items-center gap-space-sm">
                                <span class="material-symbols-outlined text-primary">{"link"}</span>
                                <h2 class="font-headline-sm text-headline-sm text-on-surface font-semibold">{"Direct Configuration Link"}</h2>
                                if props.is_file_bound {
                                    <span class="bg-tertiary-container/30 text-tertiary text-xs px-2 py-0.5 rounded-full font-mono flex items-center gap-1">
                                        <span class="w-1.5 h-1.5 rounded-full bg-tertiary animate-ping"></span>
                                        {"Live Synced"}
                                    </span>
                                } else {
                                    <span class="bg-surface-container-highest text-on-surface-variant text-xs px-2 py-0.5 rounded-full font-mono">
                                        {"In-Memory Preset"}
                                    </span>
                                }
                            </div>

                            <div class="flex items-center gap-space-xs">
                                <button 
                                    class="flex items-center gap-space-xs bg-surface-container-high hover:bg-surface-bright text-on-surface px-space-md py-space-xs rounded text-xs transition-colors"
                                    onclick={
                                        let cb = props.on_reset_defaults.clone();
                                        Callback::from(move |_| cb.emit(()))
                                    }
                                >
                                    <span class="material-symbols-outlined text-[14px]">{"restart_alt"}</span>
                                    {"Reset to Stock"}
                                </button>
                                <button 
                                    class="flex items-center gap-space-xs bg-primary-container hover:bg-primary text-on-primary-container px-space-md py-space-xs rounded text-xs font-semibold transition-colors"
                                    onclick={
                                        let cb = props.on_commit_save.clone();
                                        Callback::from(move |_| cb.emit(()))
                                    }
                                >
                                    <span class="material-symbols-outlined text-[14px]">{"bolt"}</span>
                                    {"Push to OrcaSlicer"}
                                </button>
                            </div>
                        </div>

                        <div class="text-body-sm text-on-surface-variant font-mono mt-1 bg-surface-container-lowest p-space-sm rounded border border-outline-variant flex items-center justify-between">
                            <span class="truncate">{&props.bound_path_hint}</span>
                            <span class="text-xs text-on-surface-variant shrink-0 ml-2">{format!("Last Save: {}", props.last_saved_time)}</span>
                        </div>
                    </div>

                    // Controls for Disconnect after save & Rebind
                    <div class="flex items-center justify-between flex-wrap gap-space-md pt-space-xs border-t border-surface-container">
                        <label class="flex items-center gap-space-sm cursor-pointer select-none text-xs text-on-surface">
                            <input 
                                type="checkbox"
                                class="rounded bg-surface-container-lowest border-outline-variant text-primary focus:ring-0"
                                checked={props.disconnect_after_save}
                                onchange={
                                    let cb = props.on_toggle_disconnect_after_save.clone();
                                    Callback::from(move |_| cb.emit(()))
                                }
                            />
                            <span>{"Auto-Disconnect after save (releases file locks so OrcaSlicer can reload)"}</span>
                        </label>

                        <div class="flex items-center gap-space-xs">
                            if props.is_file_bound {
                                <button 
                                    class="flex items-center gap-space-xs bg-surface-container-high hover:bg-error-container hover:text-error text-on-surface px-space-md py-space-xs rounded text-xs transition-colors"
                                    onclick={
                                        let cb = props.on_disconnect_file.clone();
                                        Callback::from(move |_| cb.emit(()))
                                    }
                                >
                                    <span class="material-symbols-outlined text-[14px]">{"link_off"}</span>
                                    {"Disconnect File"}
                                </button>
                            }
                            <button 
                                class="flex items-center gap-space-xs bg-surface-container-high hover:bg-surface-bright text-on-surface px-space-md py-space-xs rounded text-xs transition-colors"
                                onclick={
                                    let cb = props.on_rebind_file.clone();
                                    Callback::from(move |_| cb.emit(()))
                                }
                            >
                                <span class="material-symbols-outlined text-[14px]">{"folder_open"}</span>
                                {"Select Config File..."}
                            </button>
                        </div>
                    </div>
                </div>

                // Right 5 cols: Active Rig Summary card
                <div class="lg:col-span-5 bg-surface-container-low p-space-lg rounded-xl shadow-md border border-outline-variant flex flex-col justify-between gap-space-md">
                    <div class="flex items-start justify-between">
                        <div class="flex flex-col">
                            <span class="text-xs uppercase tracking-wider text-primary font-bold">{"Target Printer Hardware"}</span>
                            <h3 class="font-headline-sm text-headline-sm text-on-surface font-semibold">{&props.active_rig_name}</h3>
                        </div>
                        <button 
                            class="text-xs text-secondary hover:underline flex items-center gap-0.5"
                            onclick={
                                let cb = props.on_switch_tab.clone();
                                Callback::from(move |_| cb.emit(ActiveTab::Machine))
                            }
                        >
                            {"Manage Vault"}
                            <span class="material-symbols-outlined text-[14px]">{"arrow_forward"}</span>
                        </button>
                    </div>

                    <div class="grid grid-cols-3 gap-space-sm bg-surface-container-lowest p-space-sm rounded text-xs font-mono text-center">
                        <div class="flex flex-col">
                            <span class="text-on-surface-variant text-[10px] uppercase">{"Hotend"}</span>
                            <span class="text-on-surface font-medium">{"FlowTech"}</span>
                        </div>
                        <div class="flex flex-col">
                            <span class="text-on-surface-variant text-[10px] uppercase">{"Nozzle"}</span>
                            <span class="text-primary font-medium">{"Diamondback"}</span>
                        </div>
                        <div class="flex flex-col">
                            <span class="text-on-surface-variant text-[10px] uppercase">{"Max Flow"}</span>
                            <span class="text-tertiary font-medium">{"32.0 mm³/s"}</span>
                        </div>
                    </div>

                    <div class="flex items-center justify-between text-xs text-on-surface-variant">
                        <span>{"Stacked Lua Rules:"}</span>
                        <span class="text-primary font-mono font-bold">
                            {format!("{} Active / {} Total", props.rules.iter().filter(|r| r.enabled).count(), props.rules.len())}
                        </span>
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
                            <h3 class="font-headline-sm text-headline-sm text-on-surface font-semibold">{"Computed Overrides vs Stock Defaults"}</h3>
                        </div>
                        <span class="text-xs text-on-surface-variant font-mono">
                            {format!("{} modified parameters", props.diff_items.iter().filter(|d| d.is_modified).count())}
                        </span>
                    </div>

                    <div class="flex flex-col overflow-x-auto">
                        // Header
                        <div class="grid grid-cols-12 px-space-md py-space-xs bg-surface-container-lowest text-on-surface-variant font-label-caps text-label-caps uppercase rounded">
                            <div class="col-span-4">{"Parameter"}</div>
                            <div class="col-span-2">{"Category"}</div>
                            <div class="col-span-2 text-right">{"Stock Default"}</div>
                            <div class="col-span-2 text-right">{"Computed"}</div>
                            <div class="col-span-2 text-right">{"Source"}</div>
                        </div>

                        // Rows
                        <div class="flex flex-col gap-1 mt-1">
                            {for props.diff_items.iter().map(|item| {
                                let row_bg = if item.is_modified {
                                    "bg-surface-container border-l-2 border-primary"
                                } else {
                                    "bg-surface-container/40 opacity-70"
                                };

                                let unit_suffix = if item.unit.is_empty() { String::new() } else { format!(" {}", item.unit) };

                                html! {
                                    <div class={format!("grid grid-cols-12 px-space-md py-space-sm items-center rounded hover:bg-surface-container-high transition-colors font-code-sm text-code-sm {}", row_bg)}>
                                        <div class="col-span-4 flex flex-col">
                                            <span class="text-on-surface font-medium">{&item.label}</span>
                                            <span class="text-on-surface-variant text-[10px] font-mono">{&item.key}</span>
                                        </div>
                                        <div class="col-span-2 text-on-surface-variant text-xs">{&item.category}</div>
                                        <div class="col-span-2 text-right text-on-surface-variant font-mono">
                                            {format!("{}{}", item.base_value, unit_suffix)}
                                        </div>
                                        <div class={format!("col-span-2 text-right font-mono font-bold {}", if item.is_modified { "text-primary" } else { "text-on-surface" })}>
                                            {format!("{}{}", item.override_value, unit_suffix)}
                                        </div>
                                        <div class="col-span-2 text-right text-xs truncate text-secondary" title={item.source_rule.clone()}>
                                            {&item.source_rule}
                                        </div>
                                    </div>
                                }
                            })}
                        </div>
                    </div>
                </div>

                // Quick Symptom Resolver (4 cols)
                <div class="lg:col-span-4 flex flex-col gap-space-md bg-surface-container-low p-space-lg rounded-xl shadow-md border border-outline-variant">
                    <div class="flex items-center justify-between">
                        <div class="flex items-center gap-space-sm">
                            <span class="material-symbols-outlined text-primary">{"healing"}</span>
                            <h3 class="font-headline-sm text-headline-sm text-on-surface font-semibold">{"Quick Symptom Resolver"}</h3>
                        </div>
                        <button 
                            class="text-xs text-secondary hover:underline"
                            onclick={
                                let cb = props.on_switch_tab.clone();
                                Callback::from(move |_| cb.emit(ActiveTab::Wizard))
                            }
                        >
                            {"Open Wizard"}
                        </button>
                    </div>

                    <p class="text-body-sm text-on-surface-variant">
                        {"Toggle targeted Lua patch transformations without changing your underlying baseline config values:"}
                    </p>

                    <div class="flex flex-col gap-space-sm mt-space-xs">
                        {for props.rules.iter().map(|rule| {
                            let rid = rule.id.clone();
                            let cb = on_toggle_rule.clone();
                            let is_enabled = rule.enabled;

                            html! {
                                <div class={format!("p-space-md rounded-lg flex flex-col gap-space-xs border transition-all cursor-pointer {}",
                                    if is_enabled { "bg-surface-container border-primary shadow-sm" } else { "bg-surface-container-lowest border-outline-variant hover:border-surface-bright" }
                                )}
                                onclick={Callback::from(move |_| cb.emit(rid.clone()))}
                                >
                                    <div class="flex items-center justify-between">
                                        <div class="flex items-center gap-space-sm">
                                            <span class="text-lg">{&rule.icon}</span>
                                            <span class="font-headline-sm text-body-md text-on-surface font-semibold">{&rule.name}</span>
                                        </div>
                                        <div class={format!("w-5 h-5 rounded flex items-center justify-center text-xs {}",
                                            if is_enabled { "bg-primary text-on-primary font-bold" } else { "bg-surface-container-highest text-transparent" }
                                        )}>
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

            // Interactive JSON Diff Drawer Preview
            <div class="bg-surface-container-low p-space-lg rounded-xl shadow-md border border-outline-variant flex flex-col gap-space-md">
                <div class="flex items-center justify-between flex-wrap gap-space-sm">
                    <div class="flex items-center gap-space-sm cursor-pointer" onclick={
                        let cb = props.on_toggle_diff_drawer.clone();
                        Callback::from(move |_| cb.emit(()))
                    }>
                        <span class="material-symbols-outlined text-secondary">{"difference"}</span>
                        <h3 class="font-headline-sm text-headline-sm text-on-surface font-semibold">{"Interactive JSON Diff Preview"}</h3>
                        <span class="material-symbols-outlined text-on-surface-variant text-[18px]">
                            {if props.show_diff_drawer { "expand_less" } else { "expand_more" }}
                        </span>
                    </div>

                    <div class="flex items-center gap-space-sm">
                        <button 
                            class="flex items-center gap-space-xs bg-primary hover:bg-primary-fixed-dim text-on-primary-fixed px-space-md py-space-xs rounded text-xs font-semibold transition-colors"
                            onclick={
                                let cb = props.on_commit_save.clone();
                                Callback::from(move |_| cb.emit(()))
                            }
                        >
                            <span class="material-symbols-outlined text-[14px]">{"save"}</span>
                            {"Commit Changes to Local Disk"}
                        </button>
                    </div>
                </div>

                if props.show_diff_drawer {
                    <div class="bg-surface-container-lowest p-space-md rounded-lg font-mono text-xs overflow-x-auto max-h-96 border border-outline-variant">
                        <pre class="text-on-surface whitespace-pre">
                            {&props.json_diff_text}
                        </pre>
                    </div>
                }
            </div>
        </div>
    }
}
