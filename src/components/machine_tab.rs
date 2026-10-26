use yew::prelude::*;
use crate::models::MachineRig;

#[derive(Properties, PartialEq)]
pub struct MachineTabProps {
    pub machines: Vec<MachineRig>,
    pub active_machine_id: String,
    pub on_set_active_machine: Callback<String>,
    pub on_add_machine: Callback<MachineRig>,
    pub on_change_nozzle: Callback<String>, // "diamondback", "brass", "hardened_steel"
    pub on_save_rig_to_vault: Callback<()>,
}

#[function_component]
pub fn MachineTab(props: &MachineTabProps) -> Html {
    let on_set_active = props.on_set_active_machine.clone();
    let on_change_nozzle = props.on_change_nozzle.clone();

    let active_rig = props.machines.iter()
        .find(|m| m.id == props.active_machine_id)
        .cloned()
        .unwrap_or_else(MachineRig::default_kobra_diamondback);

    html! {
        <div class="flex flex-col w-full gap-space-lg">
            // Top Header Banner
            <div class="flex flex-col lg:flex-row lg:items-center justify-between gap-space-lg bg-surface-container-low p-space-xl rounded-xl shadow-md border border-outline-variant">
                <div class="flex flex-col gap-space-xs">
                    <div class="flex items-center gap-space-sm">
                        <span class="material-symbols-outlined text-primary text-3xl">{"precision_manufacturing"}</span>
                        <h1 class="font-headline-xl text-headline-xl text-on-surface font-bold">
                            {"Printer & Rig Vault — Saved Machine Configurations"}
                        </h1>
                    </div>
                    <p class="text-body-md text-on-surface-variant max-w-3xl">
                        {"Manage physical printer builds, custom toolheads, and hotend assemblies. Switching active machines recalculates kinematics, volumetric flow ceilings, and applies nozzle-specific Lua transformations."}
                    </p>
                </div>

                <div class="flex flex-wrap items-center gap-space-md">
                    <button 
                        class="flex items-center gap-space-sm bg-primary-container hover:bg-primary text-on-primary-container px-space-lg py-space-md rounded-lg font-semibold shadow-md transition-colors"
                        onclick={
                            let cb = props.on_save_rig_to_vault.clone();
                            Callback::from(move |_| cb.emit(()))
                        }
                    >
                        <span class="material-symbols-outlined text-[18px]">{"bookmark_add"}</span>
                        {"Save Current Rig to Vault"}
                    </button>
                </div>
            </div>

            // Machine Bank Grid
            <div class="flex flex-col gap-space-md">
                <div class="flex items-center justify-between">
                    <div class="flex items-center gap-space-md">
                        <span class="font-headline-sm text-headline-sm text-on-surface font-semibold">{"Machine Bank"}</span>
                        <span class="bg-surface-container-highest text-on-surface px-space-sm py-0.5 rounded text-xs font-mono">
                            {format!("{} Rigs Available", props.machines.len())}
                        </span>
                    </div>
                </div>

                <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-space-lg">
                    {for props.machines.iter().map(|machine| {
                        let is_active = machine.id == props.active_machine_id;
                        let mid = machine.id.clone();
                        let cb_set = on_set_active.clone();

                        html! {
                            <div class={format!("flex flex-col justify-between p-space-lg rounded-xl border transition-all duration-200 {}",
                                if is_active {
                                    "bg-surface-container border-primary shadow-lg ring-1 ring-primary/40"
                                } else {
                                    "bg-surface-container-low border-outline-variant hover:border-surface-bright"
                                }
                            )}>
                                <div class="flex flex-col gap-space-md">
                                    <div class="flex items-start justify-between gap-space-sm">
                                        <div class="flex flex-col">
                                            <span class="text-xs uppercase text-primary font-mono font-semibold">{&machine.printer_model}</span>
                                            <h2 class="font-headline-sm text-body-md text-on-surface font-bold mt-0.5">{&machine.name}</h2>
                                        </div>
                                        if is_active {
                                            <span class="bg-primary text-on-primary text-[10px] font-bold uppercase px-2 py-0.5 rounded-full shrink-0">
                                                {"Active Rig"}
                                            </span>
                                        }
                                    </div>

                                    // Hardware Specs Grid
                                    <div class="grid grid-cols-3 gap-space-sm bg-surface-container-lowest p-space-sm rounded text-xs font-mono text-center">
                                        <div class="flex flex-col">
                                            <span class="text-on-surface-variant text-[10px] uppercase">{"Bed Size"}</span>
                                            <span class="text-on-surface truncate">{&machine.bed_size}</span>
                                        </div>
                                        <div class="flex flex-col">
                                            <span class="text-on-surface-variant text-[10px] uppercase">{"Max Speed"}</span>
                                            <span class="text-on-surface font-semibold">{&machine.max_speed}</span>
                                        </div>
                                        <div class="flex flex-col">
                                            <span class="text-on-surface-variant text-[10px] uppercase">{"Max Vol"}</span>
                                            <span class="text-tertiary font-semibold">{&machine.max_volumetric_flow}</span>
                                        </div>
                                    </div>

                                    // Hardware tags
                                    <div class="flex flex-col gap-1 text-xs text-on-surface-variant bg-surface-container-low p-space-sm rounded">
                                        <div class="flex justify-between">
                                            <span>{"Hotend:"}</span>
                                            <span class="text-on-surface font-medium truncate max-w-[180px]">{&machine.hotend}</span>
                                        </div>
                                        <div class="flex justify-between">
                                            <span>{"Nozzle:"}</span>
                                            <span class="text-primary font-medium">{&machine.nozzle_name}</span>
                                        </div>
                                        <div class="flex justify-between">
                                            <span>{"Firmware:"}</span>
                                            <span class="text-on-surface truncate max-w-[180px]">{&machine.firmware}</span>
                                        </div>
                                    </div>
                                </div>

                                // Card Footer: Set Active Button
                                <div class="flex items-center justify-between pt-space-md mt-space-md border-t border-surface-container">
                                    <span class="text-xs text-on-surface-variant">
                                        {format!("{} rules applied", machine.applied_transformations.len())}
                                    </span>

                                    if is_active {
                                        <div class="flex items-center gap-1 text-primary text-xs font-bold font-mono">
                                            <span class="material-symbols-outlined text-[16px]">{"check_circle"}</span>
                                            {"Current Active"}
                                        </div>
                                    } else {
                                        <button 
                                            class="bg-surface-container-highest hover:bg-surface-bright text-on-surface px-space-md py-1 rounded text-xs font-semibold transition-colors"
                                            onclick={Callback::from(move |_| cb_set.emit(mid.clone()))}
                                        >
                                            {"Set Active"}
                                        </button>
                                    }
                                </div>
                            </div>
                        }
                    })}
                </div>
            </div>

            // Active Rig Hardware Inspector
            <div class="flex flex-col gap-space-md bg-surface-container-low p-space-xl rounded-xl shadow-md border border-outline-variant">
                <div class="flex items-center justify-between flex-wrap gap-space-sm">
                    <div class="flex items-center gap-space-sm">
                        <span class="material-symbols-outlined text-secondary">{"hardware"}</span>
                        <h2 class="font-headline-sm text-headline-sm text-on-surface font-bold">
                            {"Active Rig Hardware Inspector & Transformation Link"}
                        </h2>
                    </div>
                    <span class="text-xs text-secondary font-mono bg-surface-container-lowest px-2 py-1 rounded">
                        {format!("Inspecting: {}", active_rig.name)}
                    </span>
                </div>

                <div class="grid grid-cols-1 lg:grid-cols-12 gap-space-lg items-start">
                    // Chassis & Kinematics (6 cols)
                    <div class="lg:col-span-6 flex flex-col gap-space-md bg-surface-container p-space-lg rounded-xl">
                        <span class="font-headline-sm text-body-md text-on-surface font-semibold flex items-center gap-2">
                            <span class="material-symbols-outlined text-primary text-[18px]">{"grid_view"}</span>
                            {"Chassis & Motion Architecture"}
                        </span>

                        <div class="flex flex-col gap-space-xs text-xs font-mono">
                            <div class="flex justify-between py-1 border-b border-surface-container-high">
                                <span class="text-on-surface-variant">{"Kinematics Type"}</span>
                                <span class="text-on-surface font-bold">{&active_rig.kinematics}</span>
                            </div>
                            <div class="flex justify-between py-1 border-b border-surface-container-high">
                                <span class="text-on-surface-variant">{"Printable Volume (XYZ)"}</span>
                                <span class="text-on-surface">{&active_rig.bed_size}</span>
                            </div>
                            <div class="flex justify-between py-1 border-b border-surface-container-high">
                                <span class="text-on-surface-variant">{"Firmware Platform"}</span>
                                <span class="text-on-surface">{&active_rig.firmware}</span>
                            </div>
                            <div class="flex justify-between py-1 border-b border-surface-container-high">
                                <span class="text-on-surface-variant">{"Peak Travel Speed"}</span>
                                <span class="text-tertiary">{&active_rig.max_speed}</span>
                            </div>
                            <div class="flex justify-between py-1">
                                <span class="text-on-surface-variant">{"Max Kinematic Acceleration"}</span>
                                <span class="text-tertiary">{&active_rig.max_accel}</span>
                            </div>
                        </div>
                    </div>

                    // Hotend & Nozzle Configuration (6 cols)
                    <div class="lg:col-span-6 flex flex-col gap-space-md bg-surface-container p-space-lg rounded-xl">
                        <span class="font-headline-sm text-body-md text-on-surface font-semibold flex items-center gap-2">
                            <span class="material-symbols-outlined text-secondary text-[18px]">{"local_fire_department"}</span>
                            {"Hotend & Nozzle Configuration"}
                        </span>

                        <div class="flex flex-col gap-space-xs text-xs font-mono">
                            <div class="flex justify-between py-1 border-b border-surface-container-high">
                                <span class="text-on-surface-variant">{"Hotend Core"}</span>
                                <span class="text-on-surface font-bold">{&active_rig.hotend}</span>
                            </div>
                            <div class="flex justify-between py-1 border-b border-surface-container-high">
                                <span class="text-on-surface-variant">{"Volumetric Melt Capacity"}</span>
                                <span class="text-secondary font-bold">{&active_rig.max_volumetric_flow}</span>
                            </div>
                        </div>

                        // Nozzle Type Selector (Interchangeable Nozzle support)
                        <div class="flex flex-col gap-space-xs mt-space-xs">
                            <label class="text-xs text-on-surface-variant font-semibold">{"Installed Nozzle Tip:"}</label>
                            <div class="grid grid-cols-3 gap-space-sm">
                                {for vec![
                                    ("diamondback", "💎 Diamondback PCD", "Extreme heat conduction, lowers melt temp 10°C"),
                                    ("brass", "🟡 Standard Brass", "Standard factory conductivity"),
                                    ("hardened_steel", "🛡️ Hardened Steel", "Abrasion resistant, needs +5°C offset"),
                                ].into_iter().map(|(noz_id, noz_label, _desc)| {
                                    let is_sel = active_rig.nozzle_type == noz_id;
                                    let nid = noz_id.to_string();
                                    let cb_noz = on_change_nozzle.clone();

                                    html! {
                                        <button 
                                            class={format!("p-space-sm rounded-lg border text-xs font-mono text-center flex flex-col items-center justify-center transition-all {}",
                                                if is_sel {
                                                    "bg-primary-container/20 border-primary text-primary font-bold shadow-sm"
                                                } else {
                                                    "bg-surface-container-lowest border-outline-variant text-on-surface-variant hover:text-on-surface hover:border-surface-bright"
                                                }
                                            )}
                                            onclick={Callback::from(move |_| cb_noz.emit(nid.clone()))}
                                        >
                                            <span>{noz_label}</span>
                                            if is_sel {
                                                <span class="text-[9px] uppercase font-bold text-primary mt-1">{"Installed"}</span>
                                            }
                                        </button>
                                    }
                                })}
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}
