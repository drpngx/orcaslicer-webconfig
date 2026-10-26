use yew::prelude::*;
use crate::transformations::TransformationRule;

#[derive(Properties, PartialEq)]
pub struct WizardTabProps {
    pub rules: Vec<TransformationRule>,
    pub on_toggle_rule: Callback<String>,
    pub on_update_rule_lua: Callback<(String, String)>,
    pub on_add_custom_rule: Callback<()>,
    pub on_export_package: Callback<()>,
    pub on_import_package_trigger: Callback<()>,
    pub on_commit_save: Callback<()>,
    pub execution_logs: Vec<String>,
    pub on_clear_logs: Callback<()>,
}

#[function_component]
pub fn WizardTab(props: &WizardTabProps) -> Html {
    let on_toggle_rule = props.on_toggle_rule.clone();
    let selected_rule_id = use_state(|| {
        props.rules.get(0).map(|r| r.id.clone()).unwrap_or_default()
    });

    let active_rule = props.rules.iter()
        .find(|r| r.id == *selected_rule_id)
        .or_else(|| props.rules.get(0));

    let active_count = props.rules.iter().filter(|r| r.enabled).count();
    let total_count = props.rules.len();
    let percentage = if total_count > 0 {
        (active_count * 100) / total_count
    } else {
        0
    };

    html! {
        <div class="flex flex-col w-full gap-space-lg">
            // Header Banner
            <div class="flex flex-col lg:flex-row items-stretch lg:items-center justify-between gap-space-lg bg-surface-container-low p-space-xl rounded-xl shadow-md border border-outline-variant">
                <div class="flex flex-col gap-space-xs">
                    <div class="flex items-center gap-space-sm">
                        <span class="material-symbols-outlined text-primary text-3xl">{"auto_fix_high"}</span>
                        <h1 class="font-headline-lg text-headline-xl text-on-surface font-bold">
                            {"Symptom-Driven Auto-Tuner & Config Changer"}
                        </h1>
                    </div>
                    <p class="text-body-md text-on-surface-variant max-w-3xl">
                        {"The wizard is a patcher of configs: it does not store absolute values, but stacks Lua transformation rules. Change nozzles or filament without redoing your whole profile tuning."}
                    </p>
                </div>

                <div class="flex flex-wrap items-center gap-space-md">
                    <button 
                        class="flex items-center gap-space-xs bg-surface-container-high hover:bg-surface-bright text-on-surface px-space-md py-space-sm rounded-lg text-xs font-semibold transition-colors"
                        onclick={
                            let cb = props.on_import_package_trigger.clone();
                            Callback::from(move |_| cb.emit(()))
                        }
                    >
                        <span class="material-symbols-outlined text-[16px]">{"upload_file"}</span>
                        {"Import Rules (1-Click)"}
                    </button>

                    <button 
                        class="flex items-center gap-space-xs bg-surface-container-high hover:bg-surface-bright text-on-surface px-space-md py-space-sm rounded-lg text-xs font-semibold transition-colors"
                        onclick={
                            let cb = props.on_export_package.clone();
                            Callback::from(move |_| cb.emit(()))
                        }
                    >
                        <span class="material-symbols-outlined text-[16px]">{"file_download"}</span>
                        {"Export Rules (1-Click)"}
                    </button>

                    <button 
                        class="flex items-center gap-space-sm bg-primary-container hover:bg-primary text-on-primary-container px-space-lg py-space-md rounded-lg font-semibold shadow-md transition-colors"
                        onclick={
                            let cb = props.on_commit_save.clone();
                            Callback::from(move |_| cb.emit(()))
                        }
                    >
                        <span class="material-symbols-outlined text-[18px]">{"publish"}</span>
                        {"Push Rules to OrcaSlicer"}
                    </button>
                </div>
            </div>

            // Metric Counters Grid
            <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-space-lg">
                <div class="flex flex-col gap-space-sm bg-surface-container p-space-lg rounded-xl shadow-sm border border-outline-variant">
                    <span class="text-xs uppercase text-on-surface-variant font-mono">{"Lua Engine Latency"}</span>
                    <span class="font-headline-xl text-headline-xl text-on-surface font-semibold font-mono">{"< 1ms"}</span>
                    <span class="text-xs text-tertiary">{"Zero-overhead pure Rust sandbox"}</span>
                </div>
                <div class="flex flex-col gap-space-sm bg-surface-container p-space-lg rounded-xl shadow-sm border border-outline-variant">
                    <span class="text-xs uppercase text-on-surface-variant font-mono">{"Active Transformation Stack"}</span>
                    <span class="font-headline-xl text-headline-xl text-primary font-semibold font-mono">
                        {format!("{} / {}", active_count, total_count)}
                    </span>
                    <span class="text-xs text-on-surface-variant">{"Stacked rules executing in sequence"}</span>
                </div>
                <div class="flex flex-col gap-space-sm bg-surface-container p-space-lg rounded-xl shadow-sm border border-outline-variant">
                    <span class="text-xs uppercase text-on-surface-variant font-mono">{"Max Flow Capacity"}</span>
                    <span class="font-headline-xl text-headline-xl text-secondary font-semibold font-mono">{"32.0 mm³/s"}</span>
                    <span class="text-xs text-secondary">{"FlowTech + Diamondback thermal head"}</span>
                </div>
                <div class="flex flex-col gap-space-sm bg-surface-container p-space-lg rounded-xl shadow-sm border border-outline-variant">
                    <span class="text-xs uppercase text-on-surface-variant font-mono">{"Rules Applied"}</span>
                    <span class="font-headline-xl text-headline-xl text-tertiary font-semibold font-mono">
                        {format!("{}%", percentage)}
                    </span>
                    <span class="text-xs text-tertiary">{"Tuning coverage over baseline"}</span>
                </div>
            </div>

            // Targeted Symptom Solutions (2x2 Grid)
            <div class="flex flex-col gap-space-md">
                <div class="flex items-center justify-between">
                    <div class="flex items-center gap-space-sm">
                        <span class="material-symbols-outlined text-secondary">{"healing"}</span>
                        <h2 class="font-headline-lg text-headline-sm text-on-surface font-semibold">{"Targeted Symptom Solutions"}</h2>
                    </div>
                    <button 
                        class="text-xs text-primary hover:underline flex items-center gap-1"
                        onclick={
                            let cb = props.on_add_custom_rule.clone();
                            Callback::from(move |_| cb.emit(()))
                        }
                    >
                        <span class="material-symbols-outlined text-[14px]">{"add"}</span>
                        {"Create New Lua Transformation"}
                    </button>
                </div>

                <div class="grid grid-cols-1 lg:grid-cols-2 gap-space-lg">
                    {for props.rules.iter().map(|rule| {
                        let rid = rule.id.clone();
                        let cb_toggle = on_toggle_rule.clone();
                        let is_active = rule.enabled;
                        let is_selected = *selected_rule_id == rule.id;
                        let sel_state = selected_rule_id.clone();
                        let r_for_click = rule.id.clone();

                        html! {
                            <div class={format!("flex flex-col justify-between p-space-lg rounded-xl border transition-all {}",
                                if is_active {
                                    "bg-surface-container border-primary shadow-md"
                                } else {
                                    "bg-surface-container-low border-outline-variant hover:border-surface-bright"
                                }
                            )}>
                                <div class="flex flex-col gap-space-md">
                                    <div class="flex items-start justify-between gap-space-sm">
                                        <div class="flex items-center gap-space-sm">
                                            <span class="text-2xl">{&rule.icon}</span>
                                            <div class="flex flex-col">
                                                <h3 class="font-headline-sm text-body-md text-on-surface font-bold">{&rule.name}</h3>
                                                <span class="text-xs text-on-surface-variant font-mono">{format!("Condition: {}", rule.condition_expr)}</span>
                                            </div>
                                        </div>

                                        <button 
                                            class={format!("px-space-md py-1 rounded text-xs font-mono font-bold transition-colors {}",
                                                if is_active { "bg-primary text-on-primary" } else { "bg-surface-container-highest text-on-surface-variant hover:text-on-surface" }
                                            )}
                                            onclick={Callback::from(move |_| cb_toggle.emit(rid.clone()))}
                                        >
                                            {if is_active { "Active (Revert)" } else { "Apply Fix" }}
                                        </button>
                                    </div>

                                    <p class="text-body-sm text-on-surface-variant">
                                        {&rule.description}
                                    </p>

                                    // Lua preview badge
                                    <div class="flex items-center justify-between text-xs text-on-surface-variant pt-space-xs">
                                        <button 
                                            class={format!("text-xs font-mono hover:underline flex items-center gap-1 {}",
                                                if is_selected { "text-secondary font-bold" } else { "text-on-surface-variant" }
                                            )}
                                            onclick={Callback::from(move |_| sel_state.set(r_for_click.clone()))}
                                        >
                                            <span class="material-symbols-outlined text-[14px]">{"code"}</span>
                                            {if is_selected { "Editing Lua Script ▼" } else { "Inspect / Edit Lua Script" }}
                                        </button>

                                        <span class="text-[10px] uppercase bg-surface-container-lowest px-2 py-0.5 rounded font-mono text-tertiary">
                                            {"Pure Lua 5.4 Sandbox"}
                                        </span>
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
                                    {format!("Live Lua Code Editor: {}", rule.name)}
                                </h3>
                            </div>
                            <span class="text-xs text-on-surface-variant font-mono">
                                {"Modifications apply instantly to config stack"}
                            </span>
                        </div>

                        <p class="text-body-sm text-on-surface-variant">
                            {"Edit the transformation script below. Scripts safely operate on `config` tables and receive the printer `ctx` context:"}
                        </p>

                        <div class="bg-surface-container-lowest p-space-md rounded-lg border border-outline-variant">
                            <textarea 
                                class="w-full h-48 bg-transparent text-on-surface font-mono text-xs focus:outline-none resize-y leading-relaxed"
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

            // Integrated G-Code & Diff Output Terminal
            <div class="flex flex-col gap-space-md bg-surface-container-lowest p-space-lg rounded-xl shadow-lg border border-outline-variant">
                <div class="flex items-center justify-between">
                    <div class="flex items-center gap-space-sm">
                        <span class="material-symbols-outlined text-tertiary">{"terminal"}</span>
                        <span class="font-headline-sm text-body-md text-on-surface font-semibold">{"Integrated Lua Execution & Evaluation Terminal"}</span>
                    </div>

                    <button 
                        class="text-xs text-on-surface-variant hover:text-primary transition-colors flex items-center gap-1 font-mono"
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
                            "text-primary"
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
