use yew::prelude::*;
use crate::models::FilamentSpool;

#[derive(Properties, PartialEq)]
pub struct FilamentTabProps {
    pub filaments: Vec<FilamentSpool>,
    pub active_filament_type: String,
    pub on_select_active_filament: Callback<String>, // material type "PLA", "PETG", "TPU", "ABS"
}

#[function_component]
pub fn FilamentTab(props: &FilamentTabProps) -> Html {
    let filter_material = use_state(|| "ALL".to_string());
    let search_query = use_state(|| String::new());

    let current_filter = (*filter_material).clone();
    let current_search = (*search_query).clone().to_lowercase();

    let filtered_spools: Vec<FilamentSpool> = props.filaments.iter()
        .filter(|s| {
            let matches_filter = current_filter == "ALL" || s.material_type.eq_ignore_ascii_case(&current_filter);
            let matches_search = current_search.is_empty() 
                || s.name.to_lowercase().contains(&current_search) 
                || s.brand.to_lowercase().contains(&current_search);
            matches_filter && matches_search
        })
        .cloned()
        .collect();

    let on_select = props.on_select_active_filament.clone();

    html! {
        <div class="flex flex-col w-full gap-space-lg">
            // Physical AMS / ACE Pro Bank Banner
            <div class="flex flex-wrap items-center justify-between gap-space-md bg-surface-container-low p-space-md rounded-xl shadow-md border border-outline-variant">
                <div class="flex items-center gap-space-md">
                    <span class="material-symbols-outlined text-primary text-2xl">{"swap_horiz"}</span>
                    <div class="flex items-center gap-space-sm">
                        <span class="font-headline-sm text-headline-sm text-on-surface font-semibold">{"Anycubic ACE Pro Physical Bank"}</span>
                        <span class="bg-tertiary-container/30 text-tertiary text-xs px-2 py-0.5 rounded-full font-mono">
                            {"4/4 Slots Ready"}
                        </span>
                    </div>
                </div>

                <div class="flex items-center gap-space-sm">
                    <span class="text-xs text-on-surface-variant font-mono">
                        {"Active Slicing Material: "}
                        <span class="text-primary font-bold">{&props.active_filament_type}</span>
                    </span>
                </div>
            </div>

            // 4 Spool Slots Grid
            <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-space-md">
                {for props.filaments.iter().take(4).enumerate().map(|(idx, spool)| {
                    let is_active = spool.material_type.eq_ignore_ascii_case(&props.active_filament_type);
                    let mat = spool.material_type.clone();
                    let cb = on_select.clone();

                    html! {
                        <div class={format!("flex flex-col justify-between p-space-md rounded-xl border transition-all {}",
                            if is_active {
                                "bg-surface-container border-primary shadow-lg ring-1 ring-primary/40"
                            } else {
                                "bg-surface-container-low border-outline-variant hover:border-surface-bright"
                            }
                        )}>
                            <div class="flex flex-col">
                                // Slot header
                                <div class="flex items-center justify-between mb-space-sm">
                                    <div class="flex items-center gap-space-xs font-code-sm text-code-sm text-on-surface-variant">
                                        <span class="font-bold">{format!("Slot #{}", idx + 1)}</span>
                                        <span class="text-[10px] uppercase bg-surface-container-highest px-1.5 py-0.5 rounded">
                                            {&spool.brand}
                                        </span>
                                    </div>
                                    <div class="w-3.5 h-3.5 rounded-full shadow-inner border border-white/20" style={format!("background-color: {};", spool.color_hex)}></div>
                                </div>

                                // Spool name & material
                                <div class="flex flex-col">
                                    <span class="font-headline-sm text-body-md text-on-surface font-bold truncate">{&spool.name}</span>
                                    <span class="text-xs text-primary font-mono font-semibold">{&spool.material_type}</span>
                                </div>

                                // Parameters summary
                                <div class="grid grid-cols-2 gap-space-sm bg-surface-container-lowest p-space-sm rounded text-xs font-mono mt-space-sm">
                                    <div class="flex flex-col">
                                        <span class="text-on-surface-variant text-[10px] uppercase">{"Hotend / Bed"}</span>
                                        <span class="text-on-surface">{format!("{}° / {}°C", spool.print_temp, spool.bed_temp)}</span>
                                    </div>
                                    <div class="flex flex-col">
                                        <span class="text-on-surface-variant text-[10px] uppercase">{"Flow / PA"}</span>
                                        <span class="text-secondary">{format!("{:.2} / {:.3}", spool.flow_ratio, spool.pressure_advance)}</span>
                                    </div>
                                </div>

                                <div class="flex items-center justify-between text-xs text-on-surface-variant font-mono mt-space-xs">
                                    <span>{"Remaining:"}</span>
                                    <span class="text-tertiary font-bold">{format!("{}g / 1000g", spool.remaining_grams)}</span>
                                </div>
                            </div>

                            // Action button
                            <div class="pt-space-sm mt-space-sm border-t border-surface-container flex justify-end">
                                if is_active {
                                    <span class="text-xs text-primary font-bold font-mono flex items-center gap-1">
                                        <span class="material-symbols-outlined text-[14px]">{"radio_button_checked"}</span>
                                        {"Active Slicing Target"}
                                    </span>
                                } else {
                                    <button 
                                        class="bg-surface-container-highest hover:bg-surface-bright text-on-surface px-space-md py-1 rounded text-xs font-semibold transition-colors"
                                        onclick={Callback::from(move |_| cb.emit(mat.clone()))}
                                    >
                                        {"Select Material"}
                                    </button>
                                }
                            </div>
                        </div>
                    }
                })}
            </div>

            // Saved Filament Bank & Filter Table
            <div class="flex flex-col gap-space-md bg-surface-container-low p-space-xl rounded-xl shadow-md border border-outline-variant">
                <div class="flex flex-wrap items-center justify-between gap-space-sm">
                    <div class="flex items-center gap-space-sm">
                        <span class="material-symbols-outlined text-secondary">{"inventory_2"}</span>
                        <span class="font-headline-lg text-headline-sm text-on-surface font-semibold">{"Saved Filament Bank"}</span>
                    </div>

                    // Search & filter
                    <div class="flex items-center gap-space-sm flex-wrap">
                        <div class="flex items-center gap-1 bg-surface-container rounded-lg p-0.5 border border-outline-variant">
                            {for vec!["ALL", "PLA", "PETG", "TPU", "ABS"].into_iter().map(|mat| {
                                let is_active = (*filter_material) == mat;
                                let m = mat.to_string();
                                let fm = filter_material.clone();

                                html! {
                                    <button 
                                        class={format!("px-space-md py-1 rounded text-xs font-mono transition-colors {}",
                                            if is_active { "bg-primary text-on-primary font-bold" } else { "text-on-surface-variant hover:text-on-surface" }
                                        )}
                                        onclick={Callback::from(move |_| fm.set(m.clone()))}
                                    >
                                        {mat}
                                    </button>
                                }
                            })}
                        </div>
                    </div>
                </div>

                // Filaments Table
                <div class="flex flex-col overflow-x-auto">
                    <div class="grid grid-cols-12 px-space-md py-space-xs bg-surface-container-lowest text-on-surface-variant font-label-caps text-label-caps uppercase rounded">
                        <div class="col-span-3">{"Spool Name & Brand"}</div>
                        <div class="col-span-2">{"Material"}</div>
                        <div class="col-span-2 text-right">{"Nozzle Temp"}</div>
                        <div class="col-span-1 text-right">{"Bed Temp"}</div>
                        <div class="col-span-2 text-right">{"Flow Ratio"}</div>
                        <div class="col-span-2 text-right">{"Action"}</div>
                    </div>

                    <div class="flex flex-col gap-1 mt-1">
                        {for filtered_spools.iter().map(|spool| {
                            let is_active = spool.material_type.eq_ignore_ascii_case(&props.active_filament_type);
                            let mat = spool.material_type.clone();
                            let cb = on_select.clone();

                            html! {
                                <div class="grid grid-cols-12 px-space-md py-space-sm items-center rounded bg-surface-container hover:bg-surface-container-high transition-colors font-code-sm text-code-sm">
                                    <div class="col-span-3 flex items-center gap-space-sm">
                                        <div class="w-3 h-3 rounded-full shrink-0" style={format!("background-color: {};", spool.color_hex)}></div>
                                        <div class="flex flex-col truncate">
                                            <span class="text-on-surface font-semibold truncate">{&spool.name}</span>
                                            <span class="text-on-surface-variant text-[10px]">{&spool.brand}</span>
                                        </div>
                                    </div>
                                    <div class="col-span-2">
                                        <span class="bg-surface-container-highest px-2 py-0.5 rounded text-xs text-primary font-bold">
                                            {&spool.material_type}
                                        </span>
                                    </div>
                                    <div class="col-span-2 text-right font-mono text-on-surface">{format!("{}°C", spool.print_temp)}</div>
                                    <div class="col-span-1 text-right font-mono text-on-surface">{format!("{}°C", spool.bed_temp)}</div>
                                    <div class="col-span-2 text-right font-mono text-secondary">{format!("{:.2}", spool.flow_ratio)}</div>
                                    <div class="col-span-2 text-right">
                                        if is_active {
                                            <span class="text-xs text-tertiary font-bold">{"Active"}</span>
                                        } else {
                                            <button 
                                                class="bg-surface-container-highest hover:bg-surface-bright text-on-surface px-space-sm py-0.5 rounded text-xs transition-colors"
                                                onclick={Callback::from(move |_| cb.emit(mat.clone()))}
                                            >
                                                {"Switch"}
                                            </button>
                                        }
                                    </div>
                                </div>
                            }
                        })}
                    </div>
                </div>
            </div>

            // Multi-Material Purge Transition Matrix
            <div class="flex flex-col gap-space-md bg-surface-container-low p-space-xl rounded-xl shadow-md border border-outline-variant">
                <div class="flex items-center gap-space-sm">
                    <span class="material-symbols-outlined text-tertiary">{"grid_on"}</span>
                    <h2 class="font-headline-sm text-headline-sm text-on-surface font-semibold">
                        {"Filament Change Transition Flush Matrix (mm³)"}
                    </h2>
                </div>
                <p class="text-body-sm text-on-surface-variant">
                    {"Purge volumes automatically scale when transitioning between high-contrast colors and materials to prevent bleed:"}
                </p>

                <div class="grid grid-cols-5 gap-space-xs text-center font-mono text-xs">
                    <div class="bg-surface-container-lowest p-2 font-bold text-on-surface-variant">{"From \\ To"}</div>
                    <div class="bg-surface-container-lowest p-2 font-bold text-cyan-400">{"PLA (Cyan)"}</div>
                    <div class="bg-surface-container-lowest p-2 font-bold text-orange-400">{"PETG (Org)"}</div>
                    <div class="bg-surface-container-lowest p-2 font-bold text-slate-300">{"PLA (Matte)"}</div>
                    <div class="bg-surface-container-lowest p-2 font-bold text-emerald-400">{"TPU (Grn)"}</div>

                    <div class="bg-surface-container p-2 font-bold text-cyan-400">{"PLA (Cyan)"}</div>
                    <div class="bg-surface-container-lowest p-2 text-on-surface-variant">{"0"}</div>
                    <div class="bg-surface-container-lowest p-2 text-secondary">{"240"}</div>
                    <div class="bg-surface-container-lowest p-2 text-on-surface">{"180"}</div>
                    <div class="bg-surface-container-lowest p-2 text-tertiary">{"320"}</div>

                    <div class="bg-surface-container p-2 font-bold text-orange-400">{"PETG (Org)"}</div>
                    <div class="bg-surface-container-lowest p-2 text-secondary">{"280"}</div>
                    <div class="bg-surface-container-lowest p-2 text-on-surface-variant">{"0"}</div>
                    <div class="bg-surface-container-lowest p-2 text-secondary">{"290"}</div>
                    <div class="bg-surface-container-lowest p-2 text-tertiary">{"380"}</div>

                    <div class="bg-surface-container p-2 font-bold text-slate-300">{"PLA (Matte)"}</div>
                    <div class="bg-surface-container-lowest p-2 text-on-surface">{"190"}</div>
                    <div class="bg-surface-container-lowest p-2 text-secondary">{"260"}</div>
                    <div class="bg-surface-container-lowest p-2 text-on-surface-variant">{"0"}</div>
                    <div class="bg-surface-container-lowest p-2 text-tertiary">{"310"}</div>

                    <div class="bg-surface-container p-2 font-bold text-emerald-400">{"TPU (Grn)"}</div>
                    <div class="bg-surface-container-lowest p-2 text-tertiary">{"340"}</div>
                    <div class="bg-surface-container-lowest p-2 text-tertiary">{"410"}</div>
                    <div class="bg-surface-container-lowest p-2 text-tertiary">{"330"}</div>
                    <div class="bg-surface-container-lowest p-2 text-on-surface-variant">{"0"}</div>
                </div>
            </div>
        </div>
    }
}
