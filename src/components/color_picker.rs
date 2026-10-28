use crate::catalog::{load_merged_catalog, CatalogColorSwatch};
use yew::prelude::*;

/// Parse `#RRGGBB`, `RRGGBB`, or `#RGB` into `(r, g, b)`.
pub fn hex_to_rgb(hex: &str) -> Option<(u8, u8, u8)> {
  let clean = hex.trim().trim_start_matches('#');
  if clean.len() == 6 {
    let r = u8::from_str_radix(&clean[0..2], 16).ok()?;
    let g = u8::from_str_radix(&clean[2..4], 16).ok()?;
    let b = u8::from_str_radix(&clean[4..6], 16).ok()?;
    Some((r, g, b))
  } else if clean.len() == 3 {
    let r = u8::from_str_radix(&clean[0..1], 16).ok()? * 17;
    let g = u8::from_str_radix(&clean[1..2], 16).ok()? * 17;
    let b = u8::from_str_radix(&clean[2..3], 16).ok()? * 17;
    Some((r, g, b))
  } else {
    None
  }
}

/// Format `(r, g, b)` into uppercase `#RRGGBB`.
pub fn rgb_to_hex(r: u8, g: u8, b: u8) -> String {
  format!("#{:02X}{:02X}{:02X}", r, g, b)
}

/// Convert `(r, g, b)` (`0..=255`) into `(h, s, l)` where `h` is `0..=360`, `s` is `0..=100`, `l` is `0..=100`.
pub fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (u16, u8, u8) {
  let rf = r as f32 / 255.0;
  let gf = g as f32 / 255.0;
  let bf = b as f32 / 255.0;

  let max = rf.max(gf).max(bf);
  let min = rf.min(gf).min(bf);
  let delta = max - min;

  let l = (max + min) / 2.0;

  let (h, s) = if delta.abs() < 1e-5 {
    (0.0, 0.0)
  } else {
    let s_val = delta / (1.0 - (2.0 * l - 1.0).abs());
    let h_deg = if (max - rf).abs() < 1e-5 {
      60.0 * (((gf - bf) / delta) % 6.0)
    } else if (max - gf).abs() < 1e-5 {
      60.0 * (((bf - rf) / delta) + 2.0)
    } else {
      60.0 * (((rf - gf) / delta) + 4.0)
    };
    let h_norm = if h_deg < 0.0 { h_deg + 360.0 } else { h_deg };
    (h_norm, s_val)
  };

  (
    (h.round() as u16).min(360),
    ((s * 100.0).round() as u8).min(100),
    ((l * 100.0).round() as u8).min(100),
  )
}

/// Convert `(h, s, l)` (`h: 0..=360`, `s: 0..=100`, `l: 0..=100`) into `(r, g, b)`.
pub fn hsl_to_rgb(h: u16, s: u8, l: u8) -> (u8, u8, u8) {
  let hf = (h % 360) as f32;
  let sf = (s.min(100) as f32) / 100.0;
  let lf = (l.min(100) as f32) / 100.0;

  let c = (1.0 - (2.0 * lf - 1.0).abs()) * sf;
  let x = c * (1.0 - (((hf / 60.0) % 2.0) - 1.0).abs());
  let m = lf - c / 2.0;

  let (r1, g1, b1) = if hf < 60.0 {
    (c, x, 0.0)
  } else if hf < 120.0 {
    (x, c, 0.0)
  } else if hf < 180.0 {
    (0.0, c, x)
  } else if hf < 240.0 {
    (0.0, x, c)
  } else if hf < 300.0 {
    (x, 0.0, c)
  } else {
    (c, 0.0, x)
  };

  let r = ((r1 + m) * 255.0).round().clamp(0.0, 255.0) as u8;
  let g = ((g1 + m) * 255.0).round().clamp(0.0, 255.0) as u8;
  let b = ((b1 + m) * 255.0).round().clamp(0.0, 255.0) as u8;
  (r, g, b)
}

#[derive(Clone, Debug, PartialEq)]
pub struct ColorSelection {
  pub hex: String,
  pub name: String,
}

#[derive(Properties, PartialEq)]
pub struct ColorPickerProps {
  pub value_hex: String,
  pub on_change: Callback<ColorSelection>,
  #[prop_or_default]
  pub compact: bool,
  #[prop_or_default]
  pub title_label: Option<String>,
}

#[function_component]
pub fn ColorPicker(props: &ColorPickerProps) -> Html {
  let is_open = use_state(|| false);
  let hex_draft = use_state(|| props.value_hex.clone());

  let shortlist: Vec<CatalogColorSwatch> = use_memo((), |_| load_merged_catalog().standard_colors)
    .iter()
    .cloned()
    .collect();

  // Keep hex_draft synced when external props.value_hex changes
  {
    let hex_draft = hex_draft.clone();
    let ext_hex = props.value_hex.clone();
    use_effect_with(ext_hex, move |new_hex| {
      hex_draft.set(new_hex.clone());
      || ()
    });
  }

  let (r, g, b) = hex_to_rgb(&props.value_hex).unwrap_or((26, 26, 26));
  let (h, s, l) = rgb_to_hsl(r, g, b);

  let matched_swatch_name = shortlist
    .iter()
    .find(|sw| sw.hex.eq_ignore_ascii_case(&props.value_hex))
    .map(|sw| sw.name.clone());

  let display_name = matched_swatch_name
    .clone()
    .unwrap_or_else(|| props.value_hex.to_uppercase());

  let emit_rgb = {
    let on_change = props.on_change.clone();
    let shortlist = shortlist.clone();
    move |nr: u8, ng: u8, nb: u8| {
      let new_hex = rgb_to_hex(nr, ng, nb);
      let name = shortlist
        .iter()
        .find(|sw| sw.hex.eq_ignore_ascii_case(&new_hex))
        .map(|sw| sw.name.clone())
        .unwrap_or_else(|| new_hex.clone());
      on_change.emit(ColorSelection { hex: new_hex, name });
    }
  };

  let toggle_open = {
    let is_open = is_open.clone();
    Callback::from(move |e: MouseEvent| {
      e.stop_propagation();
      is_open.set(!*is_open);
    })
  };

  let close_modal = {
    let is_open = is_open.clone();
    Callback::from(move |e: MouseEvent| {
      e.stop_propagation();
      is_open.set(false);
    })
  };

  html! {
    <div class="inline-flex items-center">
      // Swatch Trigger Button (replaces native <input type="color">)
      <button
        type="button"
        title={format!("Color Picker: {} ({}) — Click for Shortlist, HEX, RGB & HSL", display_name, props.value_hex)}
        onclick={toggle_open}
        class={format!(
          "flex items-center gap-1.5 rounded-lg border border-outline-variant hover:border-primary bg-surface-container-lowest transition-all cursor-pointer shrink-0 {}",
          if props.compact {
            "p-1"
          } else {
            "px-2.5 py-1 text-xs"
          }
        )}
      >
        <span
          class={format!(
            "rounded border border-outline shrink-0 shadow-inner {}",
            if props.compact { "w-5 h-5" } else { "w-5 h-5" }
          )}
          style={format!("background-color: {};", props.value_hex)}
        />
        {if !props.compact {
          html! {
            <>
              <span class="font-mono text-on-surface truncate max-w-[96px]">
                {display_name.clone()}
              </span>
              <span class="material-symbols-outlined text-xs text-on-surface-variant">{"palette"}</span>
            </>
          }
        } else {
          html! {}
        }}
      </button>

      // Full Color Picker Modal Popover (Shortlist + HEX + RGB + HSL)
      {if *is_open {
        html! {
          <div
            onclick={close_modal.clone()}
            class="fixed inset-0 z-50 bg-background/70 backdrop-blur-xs flex items-center justify-center p-4"
          >
            <div
              onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}
              class="bg-surface-container-low border border-outline-variant rounded-2xl shadow-2xl max-w-lg w-full p-space-lg flex flex-col gap-space-md text-xs font-mono"
            >
              // Header with Live Swatch Preview
              <div class="flex items-center justify-between gap-space-md border-b border-outline-variant/60 pb-space-sm">
                <div class="flex items-center gap-space-sm">
                  <div
                    class="w-10 h-10 rounded-xl border-2 border-white/80 shadow-md shrink-0"
                    style={format!("background-color: {};", props.value_hex)}
                  />
                  <div class="flex flex-col">
                    <span class="text-sm font-bold text-on-surface font-sans">
                      {props.title_label.clone().unwrap_or_else(|| "Filament Color Picker".to_string())}
                    </span>
                    <span class="text-[11px] text-primary font-bold">
                      {format!(
                        "{} • {} • RGB({}, {}, {}) • HSL({}°, {}%, {}%)",
                        matched_swatch_name.unwrap_or_else(|| "Custom Color".to_string()),
                        props.value_hex.to_uppercase(),
                        r, g, b,
                        h, s, l
                      )}
                    </span>
                  </div>
                </div>
                <button
                  type="button"
                  onclick={close_modal}
                  class="bg-primary text-on-primary px-3 py-1.5 rounded-lg font-bold text-xs hover:bg-primary-container hover:text-on-primary-container transition-colors"
                >
                  {"Done"}
                </button>
              </div>

              // 1. Shortlist Standard Filament Colors Grid
              <div class="flex flex-col gap-1.5">
                <span class="text-[11px] font-bold text-on-surface-variant uppercase tracking-wider">
                  {"1. Standard Filament Color Shortlist"}
                </span>
                <div class="grid grid-cols-8 gap-1.5 bg-surface-container-lowest p-2.5 rounded-xl border border-outline-variant/60">
                  {for shortlist.iter().map(|sw| {
                    let sw_clone = sw.clone();
                    let on_change = props.on_change.clone();
                    let is_selected = props.value_hex.eq_ignore_ascii_case(&sw.hex);
                    html! {
                      <button
                        type="button"
                        title={format!("{} ({})", sw.name, sw.hex)}
                        onclick={Callback::from(move |_| {
                          on_change.emit(ColorSelection {
                            hex: sw_clone.hex.clone(),
                            name: sw_clone.name.clone(),
                          });
                        })}
                        class={format!(
                          "h-7 rounded-lg border transition-transform flex items-center justify-center {}",
                          if is_selected {
                            "ring-2 ring-primary border-white scale-110 z-10"
                          } else {
                            "border-outline-variant/80 hover:scale-105"
                          }
                        )}
                        style={format!("background-color: {};", sw.hex)}
                      >
                        <span class="sr-only">{&sw.name}</span>
                      </button>
                    }
                  })}
                </div>
              </div>

              // 2. HEX Input
              <div class="flex items-center justify-between gap-space-md bg-surface-container-lowest p-2.5 rounded-xl border border-outline-variant/60">
                <span class="text-[11px] font-bold text-on-surface-variant uppercase">
                  {"2. HEX (#RRGGBB)"}
                </span>
                <div class="flex items-center gap-2">
                  <input
                    type="text"
                    maxlength="7"
                    value={(*hex_draft).clone()}
                    placeholder="#RRGGBB"
                    class="bg-surface-container-low border border-outline-variant rounded-lg px-2.5 py-1 text-xs text-on-surface font-mono w-28 uppercase"
                    oninput={{
                      let hex_draft = hex_draft.clone();
                      let emit = emit_rgb.clone();
                      Callback::from(move |e: InputEvent| {
                        let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                        let val = input.value();
                        hex_draft.set(val.clone());
                        if let Some((nr, ng, nb)) = hex_to_rgb(&val) {
                          if val.trim().trim_start_matches('#').len() == 6 {
                            emit(nr, ng, nb);
                          }
                        }
                      })
                    }}
                  />
                </div>
              </div>

              // 3. HSL Sliders & Numeric Inputs
              <div class="flex flex-col gap-2 bg-surface-container-lowest p-2.5 rounded-xl border border-outline-variant/60">
                <span class="text-[11px] font-bold text-on-surface-variant uppercase">
                  {"3. HSL (Hue • Saturation • Lightness)"}
                </span>

                // Hue (0 - 360)
                <div class="grid grid-cols-12 items-center gap-2">
                  <span class="col-span-2 text-[11px] text-on-surface">{"H (0–360°)"}</span>
                  <input
                    type="range"
                    min="0"
                    max="360"
                    value={h.to_string()}
                    style="background: linear-gradient(to right, #ff0000 0%, #ffff00 17%, #00ff00 33%, #00ffff 50%, #0000ff 67%, #ff00ff 83%, #ff0000 100%);"
                    class="col-span-8 h-2.5 rounded-lg appearance-none cursor-pointer"
                    oninput={{
                      let emit = emit_rgb.clone();
                      Callback::from(move |e: InputEvent| {
                        let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                        if let Ok(nh) = input.value().parse::<u16>() {
                          let (nr, ng, nb) = hsl_to_rgb(nh, s, l);
                          emit(nr, ng, nb);
                        }
                      })
                    }}
                  />
                  <input
                    type="number"
                    min="0"
                    max="360"
                    value={h.to_string()}
                    class="col-span-2 bg-surface-container-low border border-outline-variant rounded px-1.5 py-0.5 text-center text-on-surface"
                    oninput={{
                      let emit = emit_rgb.clone();
                      Callback::from(move |e: InputEvent| {
                        let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                        if let Ok(nh) = input.value().parse::<u16>() {
                          let (nr, ng, nb) = hsl_to_rgb(nh.min(360), s, l);
                          emit(nr, ng, nb);
                        }
                      })
                    }}
                  />
                </div>

                // Saturation (0 - 100%)
                <div class="grid grid-cols-12 items-center gap-2">
                  <span class="col-span-2 text-[11px] text-on-surface">{"S (0–100%)"}</span>
                  <input
                    type="range"
                    min="0"
                    max="100"
                    value={s.to_string()}
                    style={format!(
                      "background: linear-gradient(to right, hsl({}, 0%, {}%), hsl({}, 100%, {}%));",
                      h, l, h, l
                    )}
                    class="col-span-8 h-2.5 rounded-lg appearance-none cursor-pointer"
                    oninput={{
                      let emit = emit_rgb.clone();
                      Callback::from(move |e: InputEvent| {
                        let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                        if let Ok(ns) = input.value().parse::<u8>() {
                          let (nr, ng, nb) = hsl_to_rgb(h, ns.min(100), l);
                          emit(nr, ng, nb);
                        }
                      })
                    }}
                  />
                  <input
                    type="number"
                    min="0"
                    max="100"
                    value={s.to_string()}
                    class="col-span-2 bg-surface-container-low border border-outline-variant rounded px-1.5 py-0.5 text-center text-on-surface"
                    oninput={{
                      let emit = emit_rgb.clone();
                      Callback::from(move |e: InputEvent| {
                        let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                        if let Ok(ns) = input.value().parse::<u8>() {
                          let (nr, ng, nb) = hsl_to_rgb(h, ns.min(100), l);
                          emit(nr, ng, nb);
                        }
                      })
                    }}
                  />
                </div>

                // Lightness (0 - 100%)
                <div class="grid grid-cols-12 items-center gap-2">
                  <span class="col-span-2 text-[11px] text-on-surface">{"L (0–100%)"}</span>
                  <input
                    type="range"
                    min="0"
                    max="100"
                    value={l.to_string()}
                    style={format!(
                      "background: linear-gradient(to right, #000000 0%, hsl({}, {}%, 50%) 50%, #ffffff 100%);",
                      h, s
                    )}
                    class="col-span-8 h-2.5 rounded-lg appearance-none cursor-pointer"
                    oninput={{
                      let emit = emit_rgb.clone();
                      Callback::from(move |e: InputEvent| {
                        let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                        if let Ok(nl) = input.value().parse::<u8>() {
                          let (nr, ng, nb) = hsl_to_rgb(h, s, nl.min(100));
                          emit(nr, ng, nb);
                        }
                      })
                    }}
                  />
                  <input
                    type="number"
                    min="0"
                    max="100"
                    value={l.to_string()}
                    class="col-span-2 bg-surface-container-low border border-outline-variant rounded px-1.5 py-0.5 text-center text-on-surface"
                    oninput={{
                      let emit = emit_rgb.clone();
                      Callback::from(move |e: InputEvent| {
                        let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                        if let Ok(nl) = input.value().parse::<u8>() {
                          let (nr, ng, nb) = hsl_to_rgb(h, s, nl.min(100));
                          emit(nr, ng, nb);
                        }
                      })
                    }}
                  />
                </div>
              </div>

              // 4. RGB Sliders & Numeric Inputs
              <div class="flex flex-col gap-2 bg-surface-container-lowest p-2.5 rounded-xl border border-outline-variant/60">
                <span class="text-[11px] font-bold text-on-surface-variant uppercase">
                  {"4. RGB (Red • Green • Blue 0–255)"}
                </span>

                // Red (0 - 255)
                <div class="grid grid-cols-12 items-center gap-2">
                  <span class="col-span-2 text-[11px] text-red-400 font-bold">{"R (0–255)"}</span>
                  <input
                    type="range"
                    min="0"
                    max="255"
                    value={r.to_string()}
                    style={format!(
                      "background: linear-gradient(to right, rgb(0, {}, {}), rgb(255, {}, {}));",
                      g, b, g, b
                    )}
                    class="col-span-8 h-2.5 rounded-lg appearance-none cursor-pointer"
                    oninput={{
                      let emit = emit_rgb.clone();
                      Callback::from(move |e: InputEvent| {
                        let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                        if let Ok(nr) = input.value().parse::<u8>() {
                          emit(nr, g, b);
                        }
                      })
                    }}
                  />
                  <input
                    type="number"
                    min="0"
                    max="255"
                    value={r.to_string()}
                    class="col-span-2 bg-surface-container-low border border-outline-variant rounded px-1.5 py-0.5 text-center text-on-surface"
                    oninput={{
                      let emit = emit_rgb.clone();
                      Callback::from(move |e: InputEvent| {
                        let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                        if let Ok(nr) = input.value().parse::<u16>() {
                          emit(nr.min(255) as u8, g, b);
                        }
                      })
                    }}
                  />
                </div>

                // Green (0 - 255)
                <div class="grid grid-cols-12 items-center gap-2">
                  <span class="col-span-2 text-[11px] text-green-400 font-bold">{"G (0–255)"}</span>
                  <input
                    type="range"
                    min="0"
                    max="255"
                    value={g.to_string()}
                    style={format!(
                      "background: linear-gradient(to right, rgb({}, 0, {}), rgb({}, 255, {}));",
                      r, b, r, b
                    )}
                    class="col-span-8 h-2.5 rounded-lg appearance-none cursor-pointer"
                    oninput={{
                      let emit = emit_rgb.clone();
                      Callback::from(move |e: InputEvent| {
                        let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                        if let Ok(ng) = input.value().parse::<u8>() {
                          emit(r, ng, b);
                        }
                      })
                    }}
                  />
                  <input
                    type="number"
                    min="0"
                    max="255"
                    value={g.to_string()}
                    class="col-span-2 bg-surface-container-low border border-outline-variant rounded px-1.5 py-0.5 text-center text-on-surface"
                    oninput={{
                      let emit = emit_rgb.clone();
                      Callback::from(move |e: InputEvent| {
                        let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                        if let Ok(ng) = input.value().parse::<u16>() {
                          emit(r, ng.min(255) as u8, b);
                        }
                      })
                    }}
                  />
                </div>

                // Blue (0 - 255)
                <div class="grid grid-cols-12 items-center gap-2">
                  <span class="col-span-2 text-[11px] text-blue-400 font-bold">{"B (0–255)"}</span>
                  <input
                    type="range"
                    min="0"
                    max="255"
                    value={b.to_string()}
                    style={format!(
                      "background: linear-gradient(to right, rgb({}, {}, 0), rgb({}, {}, 255));",
                      r, g, r, g
                    )}
                    class="col-span-8 h-2.5 rounded-lg appearance-none cursor-pointer"
                    oninput={{
                      let emit = emit_rgb.clone();
                      Callback::from(move |e: InputEvent| {
                        let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                        if let Ok(nb) = input.value().parse::<u8>() {
                          emit(r, g, nb);
                        }
                      })
                    }}
                  />
                  <input
                    type="number"
                    min="0"
                    max="255"
                    value={b.to_string()}
                    class="col-span-2 bg-surface-container-low border border-outline-variant rounded px-1.5 py-0.5 text-center text-on-surface"
                    oninput={{
                      let emit = emit_rgb.clone();
                      Callback::from(move |e: InputEvent| {
                        let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                        if let Ok(nb) = input.value().parse::<u16>() {
                          emit(r, g, nb.min(255) as u8);
                        }
                      })
                    }}
                  />
                </div>
              </div>
            </div>
          </div>
        }
      } else {
        html! {}
      }}
    </div>
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_hex_rgb_hsl_roundtrip() {
    let (r, g, b) = hex_to_rgb("#E10600").unwrap();
    assert_eq!((r, g, b), (225, 6, 0));
    assert_eq!(rgb_to_hex(r, g, b), "#E10600");

    let (h, s, l) = rgb_to_hsl(r, g, b);
    assert!(h <= 5);
    assert_eq!(s, 100);
    assert_eq!(l, 44);

    let (r2, g2, b2) = hsl_to_rgb(h, s, l);
    assert!((r2 as i32 - r as i32).abs() <= 3);
    assert!((g2 as i32 - g as i32).abs() <= 3);
    assert!((b2 as i32 - b as i32).abs() <= 3);
  }
}
