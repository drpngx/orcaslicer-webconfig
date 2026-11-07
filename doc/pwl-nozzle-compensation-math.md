# Composable PWL nozzle compensation — the mathematics

How the MicroSwiss FlowTech / Diamondback (PCD) nozzle rules compute their values: the
relative piecewise-linear (PWL) curve mathematics, the Rust-provided helper surface the Lua
rules are written against, the verified per-polymer computation matrix, and how the computed
values reach the exported presets. Written 2026-11-03.

The matrix in §6 is asserted by
`lua_engine::tests::test_diamondback_pwl_compensation_chain_matrix`, and the headline export
values by `test_diamondback_pwl_wiring_ships_rule_values_to_exports`, so this document and
the engine cannot silently drift apart.

## 1. Model: relative PWL curves, evaluated per spool

Every nozzle rule is a **strictly relative, monotonic function** over its incoming value:
`x_k = f_k(x_{k-1}, d)`, where `x_0` is the spool's brass baseline and `d` is the nozzle
orifice diameter (0.25 / 0.4 / 0.6 / 0.8 / 1.0 mm). Rules execute in ascending priority and
later writes win — but because every write is a function of the current value, "winning"
means *composing on top*, never discarding the previous step. A brand that ships an unusual
retraction or flow keeps it, transformed rather than replaced.

The curves are piecewise-linear because the underlying physics is regime-based — part-cooling
fan duty bands, viscoelastic vs elastomeric retraction, polymer glass-transition bands — with
smooth transitions between regimes. That is exactly what a 4–5 knot PWL expresses and what a
single offset cannot.

### 1.1 Rust engine surface

The curve machinery lives in the engine, not in each rule file:

| Surface | Kind | Semantics |
|---|---|---|
| `base[key]` | Read-only Lua table (`x_0`) | Immutable Brass-baseline snapshot seeded in Phase 1 before any rule runs. Rules read `scalar_num(base[key], ...)` to anchor PWL regime lookups (`T_0`, `f_0`, `r_0`, `z_0`) while applying deltas/scales relatively to `config[key]`. Attempting to mutate `base` raises `[Lua Immutability Violation]`. |
| `pwl(x, pts)` | Rust-native builtin | `pts` = array of `{x, y}` pairs (any order). Knots are sorted by x, linear interpolation between neighbors, **flat extrapolation** on both ends. Returns a number. Errors on non-table / empty / non-numeric points. |
| `clamp(x, lo, hi)` | Rust-native builtin | `min(max(x, lo), hi)` with max applied first (inverted bounds resolve to `hi`). |
| `constrain(key, min, max, reason)` | Rust-native builtin | Registers a box constraint $[L, U]$ on `key` in `bounds[key]` (tightening existing bounds via `max(L)` / `min(U)`), enforced during the Phase 3 Epilogue Pass at `Priority = ∞`. |
| `math.sqrt(n)` | Rust-native builtin | Standard square root; domain error on negatives. |
| `^` operator | Rust parser/eval | Right-associative, binds tighter than unary minus (`-x^2 == -(x^2)`), unary accepted on the right (`2^-2 == 0.25`) — same precedence as real Lua. |
| `scalar_num(val, default)` | Rust-owned Lua prelude, evaluated into every rule environment | Reads a config slot that may be `"235"` or `{"235"}` → number either way, `default` if absent/non-numeric. |
| `map_num(tbl, fn, fmt)` | Rust-owned Lua prelude | Applies `fn` to every numeric slot of an array- or scalar-valued key, preserving full IEEE-754 float precision (`tostring(r)`) when `config["_defer_quantization"] == "1"` (Phase 2 of `evaluate_stack`) or formatting with `string.format(fmt)` when invoked standalone. Non-numeric slots pass through and **the key's shape is preserved**. |
| `fmt_num(val, fmt)` | Rust-owned Lua prelude | Formats a scalar numeric value with `fmt` when running standalone, or preserves full IEEE-754 precision during Phase 2 (`_defer_quantization == "1"`) for deferred Epilogue quantization. |

### 1.2 Semantics notes

- **Deferred Epilogue Quantization vs. Standalone Execution:**
  - When rules execute inside `TransformationEngine::evaluate_stack`, Phase 1 sets `config["_defer_quantization"] = "1"`. Throughout Phase 2 (`P10 .. P98`) and Phase 3.1/3.2 (Box & Coupled Relational Constraint Projections), all numeric values remain at full IEEE-754 floating-point precision, and are quantized **once** at the very end of Phase 3.3 via `format_orca_schema_scalar`.
  - When rules are executed standalone in unit tests (such as `test_diamondback_pwl_compensation_chain_matrix` without `_defer_quantization`), `map_num` and `fmt_num` format each step immediately with `fmt` (`%.Nf` round-half-to-even at exact decimal ties, e.g. 245 °C base → 235.5 → **236**).
- **Immutable `base` (`x_0`) Anchor:** Because PWL curves evaluate regime position on `base[key]` (`x_0`) and apply the resulting offset/scale to `config[key]` (`x_{k-1}`), rule composition is insensitive to intermediate rounding and immune to counterfactual shadowing (`RuleShadowKind::Overwritten`).
- **Determinism:** the random-config audit maps `--seed 0` to a fixed constant and defaults
  to a fixed seed, so audit runs are reproducible. The generated catalog selects the best
  profile per filament code by iterating profile names in **sorted** order — equal-score ties
  resolve alphabetically — keeping the catalog (and therefore every export) byte-reproducible
  across generator executions.

## 2. Hardware basis

- Printer: **Anycubic Kobra S1** (CoreXY, direct drive, dual ACE Pro multi-material).
- Hotend: **MicroSwiss FlowTech all-metal hotend** (unitary thermal cylinder; retractions
  beyond ~1.2–1.5 mm pull melt into the short, sharp cold zone — heat creep — so 1.25 mm is
  treated as the retraction ceiling everywhere below). MicroSwiss nozzles fit **only**
  FlowTech hotends; selecting any MicroSwiss nozzle auto-installs the FlowTech add-on.
- Nozzle: **MicroSwiss FlowTech DiamondBack** — PCD tip in a CuCrZr body, electroless nickel
  plated. Thermal conductivity **543 W/m·K** (≈ 4.7× brass), hardness **49.8 GPa** (the
  OrcaSlicer schema caps `nozzle_hrc` near 68, so 68 is emitted with this document as the
  provenance), friction coefficient **0.01** (vs ≈ 0.18 for brass), rated 350 °C with a
  **300 °C** community ceiling for the FlowTech heater assembly. Manufactured orifices
  0.25 / 0.4 / 0.6 / 0.8 / 1.0 mm — all selectable.
- Community calibration guidance: print ~−10 °C below brass-baseline temperatures;
  direct-drive retraction 0.8–1.2 mm @ 25–40 mm/s; Diamondback stringing is a **temperature**
  problem, not a retraction problem.
- **Printer coverage:** the nozzle/hotend curves above are printer-agnostic (they depend only
  on the nozzle material, the hotend geometry, and the filament — verified identical on a
  CoreXY and a bed-slinger). The one printer-dependent part is the travel-acceleration
  breakaway policy (§5.5), which follows the rig's **motion class** — `corexy` or
  `bedslinger` — derived from its kinematics description via `constants.json`
  `printer_classes` (token match on the kinematics string, CoreXY as the default class) and
  exposed to the rules as `ctx.printer_kinematics_class` /
  `ctx.travel_accel_target_mm2` / `ctx.travel_accel_floor_mm2`. The P10 temperature ceiling
  likewise intersects the FlowTech 300 °C rating with the machine's own
  `machines[].max_hotend_temp_c` (`ctx.hotend_max_temp_c`). The class policy is grounded in
  community data: CoreXY-class stock travel accel ≈ 10 000 mm/s² (Kobra S1 base) with
  20 000 machine limits; bed-slinger stock ≈ 6 000 mm/s² (Bambu A1) with a 5 000–10 000
  quality sweet spot and 3 000–5 000 typical for Ender-class Klipper machines (sources §9).
  Orca profiles don't declare kinematics type, so the **catalog generator** labels bed-slinger
  families by model name (`printer_classes.bedslinger_name_tokens` — Ender, Kobra 3,
  Bambu A1, Prusa MK*/MINI, Sovol SV06/07, CR-10, Artillery, BIQU B1/…); 60 of the
  catalog's 380 machines across 66 vendors are labeled, everything else defaults to the
  CoreXY class, which preserves the historical behavior for unlabeled machines.

## 3. Rule P10 — `thermal_conductivity.lua` (PCD branch), priority 10

This rule owns what the nozzle *material* physics owns: the temperature curve, the die-swell
flow trim, and the machine-level `nozzle_type = brass` / `nozzle_hrc = 68` declarations
(PCD is non-magnetic and abrasion-proof like brass-class tooling; the slicer treats it as
brass for filament compatibility). It deliberately does not write retraction, speed, or
pressure advance — retraction belongs to the hotend (P12) and anti-stringing (P25) rules,
pressure advance to the spool model's `pa_multiplier`.

### 3.1 Temperature curve $\Delta T_{\mathrm{PCD}}(T_0)$

Brass nozzles lag the thermistor (radial gradient + tip fan cooling); PCD at 543 W/m·K is
nearly isothermal, so the brass profile's inflation can be removed — but the size of the
correction depends on the regime:

- 190–220 °C (PLA/TPU, fans 80–100 %): full −10…−11 °C.
- 225–250 °C (PETG/PCTG, fans 30–50 %): −10 °C (matches the community number at 235 °C).
- 255–300 °C (ABS/ASA/PA-CF/PC, enclosed, fans 0–20 %): taper to −5 °C — inter-layer chain
  reptation needs the enthalpy; over-cooling structural parts delaminates them.

$$\Delta T_{\mathrm{PCD}}(T_0) = \mathrm{PWL}\left(T_0;\ [(190,-8),\ (215,-11),\ (240,-10),\ (265,-7.5),\ (300,-5)]\right)$$

$$T_1 = \mathrm{round}\left(\mathrm{clamp}\left(T_0 + \Delta T_{\mathrm{PCD}}(T_0),\ 175,\ 300\right)\right)$$

Applied independently to `nozzle_temperature`, `nozzle_temperature_initial_layer`,
`nozzle_temperature_HS`, and `nozzle_temperature_initial_layer_HS` — each over its **own**
base value. The ceiling is `min(300, ctx.hotend_max_temp_c)` — the FlowTech assembly rating
(PCD nozzles only fit FlowTech) intersected with the machine's own hotend rating from
`constants.json` `machines[]`: a 300 °C PA/PC base lands at 294–295 °C, never above, and a
270 °C-rated machine clamps at 270.

### 3.2 Die-swell flow trim $f_1(f_0, d)$

PCD's μ = 0.01 introduces partial wall slip → less die swell → marginally less delivered
flow. The trim scales with the orifice (slip effects ∝ √(0.4/d)) and, via a quadratic in the
flow headroom `u`, leaves pre-thinned matte/CF profiles (f0 ≈ 0.88–0.92, negligible die
swell) virtually untouched while trimming glossy high-flow profiles (f0 ≈ 0.96–1.00) by up
to ~1.2 %:

$$u = \mathrm{clamp}\left(\frac{f_0 - 0.85}{0.15},\ 0,\ 1.2\right),\qquad \mathrm{trim} = 0.012\sqrt{\frac{0.4}{\max(d,0.2)}}\;u^2$$

$$f_1 = \mathrm{round}_{0.001}\left(\mathrm{clamp}\left(f_0 (1 - \mathrm{trim}),\ 0.900,\ 1.050\right)\right)$$

The output band [0.900, 1.050] matches the app-wide flow sanity band.

- PETG f0 = 0.960, d = 0.4: u = 0.733 → trim 0.65 % → **0.954**
- Matte/CF f0 = 0.900, d = 0.4: u = 0.333 → trim 0.13 % → raw 0.899, clamped to **0.900**
- PLA f0 = 0.980, d = 0.25: u = 0.867 → trim 1.14 % → **0.969** (strongest, smallest orifice)

## 4. Rule P12 — `rig_addons_cooling.lua` (`flowtech_hotend` branch), priority 12

This rule owns what the *hotend* owns: retraction shaping for the all-metal thermal break.

### 4.1 Viscoelastic retraction length $r_1(r_0, d)$

Rigid filament is a stiff piston: base 0.6–0.9 mm compresses ~10–13 % to stay clear of the
sharp transition zone. Elastomers (r0 ≥ 1.1 mm) are a spring: the first ~0.4 mm only
relieves axial compression, so their travel must be preserved. The molten meniscus volume
scales with orifice area, and the linear displacement needed to evacuate it with √(d/0.4):

$$r_{\mathrm{pwl}} = \mathrm{PWL}\left(r_0;\ [(0.40,0.36),\ (0.80,0.70),\ (1.00,0.86),\ (1.50,1.35)]\right),\qquad S_d = 0.88 + 0.12\sqrt{\frac{d}{0.4}}$$

$$r_1 = \mathrm{round}_{0.01}\left(\mathrm{clamp}\left(r_{\mathrm{pwl}} \cdot S_d,\ 0.35,\ 1.40\right)\right)$$

On a 0.4 nozzle ($S_d = 1.0$): 0.60→0.53, 0.80→0.70, 1.30→1.15. On 1.0 mm ($S_d = 1.070$):
0.80→0.75. On 0.25 mm ($S_d = 0.975$): 0.60→0.52.

### 4.2 Retraction speed $s_1(s_0)$

Crisper pressure relief through the short transition for the rigid 30–44 band, a hard cap
for hot rods — and **elastomer families skip the boost entirely** (via
`ctx.filament_is_elastomer`), keeping their calibrated speed: AMS-grade TPU ships 35 mm/s
and stays 35, matching the app-wide flexible-filament speed policy:

$$s_1 = \mathrm{round}\left(\mathrm{PWL}\left(s_0;\ [(20,20),\ (25,26),\ (35,40),\ (40,44),\ (50,46)]\right)\right)\quad(\text{rigid only})$$

## 5. Rule P25 — `symptom_stringing_diamondback.lua`, priority 25

Anti-stringing mitigation, composing on P10 + P12 outputs. Its PCD-friction-specific parts
apply **only to actual PCD nozzles** (`is_pcd` gate on `ctx.nozzle_type`): the rule is
enabled on every MicroSwiss rig for its generic writes, but a CM2/CHT copper or steel tip is
not a near-zero-friction diamond and must not be cooled or damped as one.

### 5.1 Gated temperature trim $T_2(T_1, T_0, r_0)$

−2 °C only in the sticky PLA/PETG band, **zero** above 258 °C brass baseline (structural welds) and **zero**
for elastomers (cooling TPU further raises viscosity exponentially until it buckles in the
direct-drive gears). Evaluated on the immutable brass baseline `base[key]` ($T_0$, $r_0$) and applied relatively to `config[key]` ($T_1$). The elastomer gate is primary on `ctx.filament_is_elastomer` (derived
in Rust from `constants.json` `hardware_limits.elastomer_family_tokens`) — every app TPU
default ships 0.6 mm retraction, so a retraction-length heuristic alone can never identify
an elastomer; the `(1.22 − r0)/0.25` term stays as a secondary guard for unflagged
long-travel profiles:

$$\Delta T_{\mathrm{str}}(T_0) = \mathrm{PWL}\left(T_0;\ [(190,-1),\ (213,-2),\ (236,-2),\ (258,0),\ (305,0)]\right),\qquad g_{\mathrm{rigid}} = \mathrm{clamp}\left(\frac{1.22 - r_0}{0.25},\ 0,\ 1\right)$$

$$T_2 = \mathrm{round}\left(\max\left(T_1 + \Delta T_{\mathrm{str}}(T_0)\cdot g_{\mathrm{rigid}},\ 175\right)\right)$$

$g_{\mathrm{rigid}}$ is 1 for rigid filaments ($r_0 \le 0.97$), 0 for elastomers, linear between.
PETG: 225→**223**. PLA: 204→**202**. ASA 258→**258**. TPU 214→**214** (gate 0).

### 5.2 Retraction recovery $r_2(r_1)$

A small bump helps break the low-friction meniscus, tapering to zero at the 1.25 mm
heat-creep ceiling and clamped at 1.35:

$$r_2 = \mathrm{round}_{0.01}\left(\min\left(r_1 + 0.07\cdot\mathrm{clamp}\left(\frac{1.25 - r_1}{0.75},\ 0,\ 1\right),\ 1.35\right)\right)$$

The headroom is normalized to [0, 1], so the bump never exceeds +0.07 mm and a rigid
filament never ends above its brass base (the 1.0 mm draft case lands exactly on 0.80 = r0),
while short-retraction elastomers round-trip back to ≈ their base.

### 5.3 Speed damping $s_2(s_1)$

With μ = 0.01 there is no wall drag to hold the melt column together — yanking at 44–50 mm/s
snaps it and leaves a drooling droplet. Quadratic damper, floor 22; PCD-only (wall-drag
physics) and elastomer-skipping:

$$s_2 = \mathrm{round}\left(\max\left(22,\ s_1 - 0.018\max(0,\ s_1 - 25)^2\right)\right)$$

44→**38**, 40→**36**, 26→**26**.

### 5.4 Orifice-scaled Z-hop $z_1(z_0, d)$ — on the per-filament key

OrcaSlicer's per-filament `filament_z_hop` overrides the process-level `z_hop`, so the
anti-stringing lift is written there (plus `filament_z_hop_types = Spiral Lift`), scaled by
a power law in d:

$$\Delta z(d) = 0.02 + 0.05\left(\frac{d}{0.4}\right)^{1.15},\qquad z_1 = \mathrm{round}_{0.01}\left(\mathrm{clamp}\left(z_0 + \Delta z(d),\ 0.12,\ \max(z_0 + 0.04,\ 0.65 d)\right)\right)$$

The ceiling form is deliberate: with `max(z0 + 0.04, 0.65 d)` the rule can never ship *less*
hop than the spool base — a plain `min(·, 0.65 d)` would reduce a small-orifice spool's
existing z0 inside an anti-stringing rule. Examples: 0.25 mm z0 = 0.12 → **0.16** (ceiling
binds), 0.4 mm z0 = 0.20 → **0.26** (ceiling binds), 1.0 mm z0 = 0.20 → **0.36**.

### 5.5 Breakaway travel acceleration $a_1(a_0)$

Snap the strand at the perimeter exit instead of dragging it: an asymptotic boost toward the
**printer-class target** with a class floor (`ctx.travel_accel_target_mm2` /
`ctx.travel_accel_floor_mm2`, from `constants.json` `printer_classes.policies`). Inputs above
the target pass through unchanged. On the CoreXY class (Kobra S1): target 12 000, floor
9 000 — the floor always clears the export-side travel-accel guard (§7), so the rule's
number ships verbatim; on the bed-slinger class: target 8 000, floor 5 000, because a
moving bed needs a gentler breakaway than a gantry:

$$a_1 = \left\lfloor \max\left(\mathrm{floor},\ a_0 + 0.35\max(0,\ \mathrm{target} - a_0)\right) \right\rfloor$$

CoreXY: 10 000 → **10 700**, 7 000 → boost 8 750 then floor **9 000**.
Bed-slinger: 6 000 → **6 700**, 3 000 → boost 4 050 then floor **5 000**, 15 000 →
**15 000** (unchanged). Plus `wipe_on_loops = 1` (mechanical shear before travel).

## 6. Verified computation matrix (8 polymer/nozzle cases)

Stage chain `x0 → P10 → P12 → P25`. Reproduce with
`bazel test //:orcaslicer_webconfig_test --test_filter=test_diamondback_pwl_compensation_chain_matrix --test_output=all`.

| Case (d, polymer) | Stage | Temp °C | Retract mm | Speed mm/s | Flow | filament_z_hop | travel_accel |
|---|---|---|---|---|---|---|---|
| 0.4 PETG (235 / 0.80 @ 40 / 0.960 / 0.15) | x0 → P10 → P12 → **P25** | 235 → 225 → 225 → **223** | 0.80 → 0.80 → 0.70 → **0.75** | 40 → 40 → 44 → **38** | 0.960 → **0.954** | 0.15 → · → · → **0.22 Spiral** | **10700** |
| 0.4 PLA (215 / 0.60 @ 35 / 0.980 / 0.10) | x0 → P10 → P12 → **P25** | 215 → 204 → 204 → **202** | 0.60 → 0.60 → 0.53 → **0.60** | 35 → 35 → 40 → **36** | 0.980 → **0.971** | 0.10 → · → **0.17** | **10700** |
| 0.4 ASA (265 / 0.80 @ 40 / 0.930 / 0.10) | x0 → P10 → P12 → **P25** | 265 → 258 → 258 → **258** | 0.80 → 0.80 → 0.70 → **0.75** | 40 → 40 → 44 → **38** | 0.930 → **0.927** | 0.10 → · → **0.17** | **10700** |
| 0.4 TPU 95A, long-travel brand (225 / 1.30 @ 25 / 1.000 / 0.20) | x0 → P10 → P12 → **P25** | 225 → 214 → 214 → **214** | 1.30 → 1.30 → 1.15 → **1.16** | 25 → 25 → 25 → **25** | 1.000 → **0.988** | 0.20 → · → **0.26** | **10700** |
| 0.4 TPU 95A, app defaults (225 / 0.60 @ 35 / 0.980 / 0.20) | x0 → P10 → P12 → **P25** | 225 → 214 → 214 → **214** | 0.60 → 0.60 → 0.53 → **0.60** | 35 → 35 → 35 → **35** | 0.980 → **0.969** | 0.20 → · → **0.26** | **10700** |
| 0.6 PA-CF (290 / 0.90 @ 35 / 0.900 / 0.15) | x0 → P10 → P12 → **P25** | 290 → 284 → 284 → **284** | 0.90 → 0.90 → 0.80 → **0.84** | 35 → 35 → 40 → **36** | 0.900 → **0.900** (raw 0.899, floor-clamped) | 0.15 → · → **0.25** | **10700** |
| 0.25 PLA (210 / 0.60 @ 35 / 0.980 / 0.12) | x0 → P10 → P12 → **P25** | 210 → 200 → 200 → **198** | 0.60 → 0.60 → 0.52 → **0.59** | 35 → 35 → 40 → **36** | 0.980 → **0.969** | 0.12 → · → **0.16** | **10700** |
| 1.0 PETG draft (240 / 0.80 @ 40 / 0.960 / 0.20) | x0 → P10 → P12 → **P25** | 240 → 230 → 230 → **228** | 0.80 → 0.80 → 0.75 → **0.80** | 40 → 40 → 44 → **38** | 0.960 → **0.956** | 0.20 → · → **0.36** | **10700** |

Machine-verified invariants (asserted for every row): total temp drop ∈ [4, 13] °C and
≤ 8 °C for bases ≥ 265 °C; rigid r2 ≤ r0; elastomer r2 ≥ 85 % of base and s2 = s0 exactly;
elastomer final temp = P10 output exactly (no P25 trim); flow never increases and stays
within [0.900, 1.050]; z1 ∈ [z0, max(z0 + 0.04, 0.65 d)]; a1 ≥ 9000.

## 7. How the rule values reach the exports

Both export paths (standalone filament `.json` and 3MF per-slot columns) build from one
resolver, which runs the rule stack once **per spool**:

- **Seed** = the spool's brass baseline `x_0` composed with every app-level side channel
  *except* nozzle physics (climate deltas, structural boost, wet filament, Kobra
  unbottleneck, brand/disk retraction), so the stack's output is the final value. A
  hand-tuned `print_temp` (user edit or disk override) is honored as the effective baseline
  by inverting the nozzle offset — a 268 °C PCTG spool never snaps back to the catalog
  baseline.
- **Rule-owned keys**: `nozzle_temperature` (+ `_BRASS`/`_HS`/initial-layer variants),
  `filament_flow_ratio`, `filament_retraction_length`, `filament_retraction_speed`,
  `filament_z_hop`, `filament_z_hop_types`. Each is applied only when the stack actually
  changed the seeded value, so rigs without applicable rules keep byte-identical exports.
- **Guards:** Siraya foaming-air and foaming-TPU spools skip the stack entirely (their
  zero/foam-specific retraction is material physics the curves must not touch); z-hop is
  only overridden when the base polymer z-hop is non-zero; temperatures re-clamp to the
  polymer safe band **intersected with the machine's hotend rating**
  (`machines[].max_hotend_temp_c`, mirrored by an audit check — a 290 °C PA spool exports at
  most 260 °C on an Ender-3, 300 °C on a Kobra S1 / Bambu X1C).
- **Policy constants** all live in `constants.json` — no inline numbers in the resolver, the
  audit, or the Lua. In `hardware_limits`: `elastomer_family_tokens` +
  `elastomer_max_retraction_speed_mms` for the flexible-family speed clamp (exported to the
  rules as `ctx.filament_is_elastomer`), `nozzle_temp_floor_relax_c` for the approved
  excursion below a polymer's `safe_nozzle_min_c` (the final temp clamps to floor−relax,
  letting Anycubic PETG ship 223 while its conservative floor is 225; the initial-layer temp
  keeps the hard floor), `hs_base_temp_delta_c` for the hardened-steel base inversion, and
  `min_extrusion_temp_c` as the absolute floor. In `printer_classes`: the motion-class
  tokens and the per-class travel-acceleration stock/target/floor policy.
- **Per-machine constants** (`catalog::machine_constants`): a curated `machines[]` entry is
  a *deliberate override*; every other printer **derives** from the generated Orca catalog
  (its vendor, name, default nozzle) plus the global `hardware_limits` — never a silent
  Kobra fallback. Curation exists only where the Orca repo publishes nothing (machine
  profiles carry no max hotend/bed temps or volumetric limits) or overstates hardware: the
  Ender-3's filament profiles reach 280 °C, but its PTFE-lined hotend is 260 °C sustained,
  so the curated entry says 260.
- **Process keys** (`travel_acceleration`, `wipe_on_loops`) ship through the process preset
  builder. The export-side travel-accel guard is class-aware: a rule value below the class
  floor is replaced with the class stock value (CoreXY 9 000 → 10 000; bed-slinger
  5 000 → 6 000) instead of a hard-coded Kobra number; P25's floor always clears it.
- Because both paths share the resolver, standalone `.json` and 3MF per-slot parity holds by
  construction (and each slot carries its own polymer's curve — sibling slots differ). The
  3MF **import** is machine-aware too: `parse_ace_loadout_from_bytes_or_json` extracts the
  printer model from `printer_settings_id` (matching full names and short tags — "X1C",
  "Kobra S1 Max" beats "Kobra S1") so imported spools' `inherits` point at the right
  vendor's preset instead of a Kobra default.

## 8. Open follow-ups

1. The **non-PCD branches** of `thermal_conductivity.lua` (MS-Brass, CM2, CHT) still write
   absolute retraction/flow constants, which now land in exports for MicroSwiss
   non-diamondback nozzles. Rewrite them as relative curves the way the PCD branch was.
2. `FilamentSpool::recompute_for_nozzle()` still applies the flat `temp_offset_from_brass_c`
   to `print_temp` (UI display + compatibility checks); exports no longer consume it. Decide
   whether the UI should display the rule-stack value instead of the flat approximation.
3. Machine notes / toasts quote curve ranges; once (2) is decided they can quote real
   effective per-spool values.
4. The **bed-slinger travel-acceleration policy** (stock 6 000 / target 8 000 / floor 5 000)
   is grounded in community data for modern bed-slingers (Bambu A1 stock ≈ 6 000, quality
   sweet spot 5 000–10 000; Ender-class Klipper 3 000–5 000) — older Ender-class machines
   may want the lower end; adjust `printer_classes.policies.bedslinger` per calibration.
5. The random-config audit now rotates rigs across **three printers and both motion
   classes** — Anycubic Kobra S1 (CoreXY), Bambu Lab X1 Carbon (CoreXY), and Creality
   Ender-3 (bed-slinger, 260 °C hotend rating) — with per-rig compat-printer strings,
   process filenames (`@Kobra S1` / `@X1C` / `@Ender-3`), inherits templates, and the
   machine hotend ceiling all checked. Adding more vendors is a one-line `base_rig_for_printer`
   call once they exist in `machines[]`; the default process profiles' names still bake in
   "@Kobra S1" until the app's state sync retitles them per rig.
6. `filament_pctg_antistringing.lua` (priority 48, default-enabled) now anchors its −3 °C
   high-temperature trim on `base["nozzle_temperature"]` (`t0 >= 258`), applies `filament_z_hop`
   (`0.10`) and `travel_acceleration` (`10000`) as non-destructive floors (`cur_zhop < 0.10`,
   `cur_ta < 10000`) so P25's higher Z-hop and travel acceleration are preserved, and calls
   `sync_coupled_physics(config, ctx)`; its `retract_len` / `retract_speed` remain user-tunable
   parametric knobs (`fmt_num`, default `1.00 mm @ 45 mm/s`).

## 9. Sources

- Bambu Lab A1 acceleration (bed-slinger stock ≈ 6 000 mm/s², sweet spot 5 000–10 000):
  https://industrialmonitordirect.com/blogs/knowledgebase/bambu-lab-a1-speed-and-acceleration-settings-for-thin-vertical-prints
  and https://www.reddit.com/r/BambuLab/comments/1ao33wf/fastest_a1_mini_settings_share_your_settings
- Ender 3 / bed-slinger Klipper acceleration (3 000–5 000 with input shaping; lower than
  CoreXY to avoid ringing):
  https://www.sovol3d.com/blogs/news/calibrate-3d-printer-acceleration-and-jerk-settings-guide
  and https://www.obico.io/blog/klipper-input-shaping-ender-3
- MicroSwiss FlowTech DiamondBack product page (specs, fits list):
  https://store.micro-swiss.com/products/flowtech-diamondback-nozzles
- Prusa forum, Diamondback temperature (~−10 °C consensus):
  https://forum.prusa3d.com/forum/english-forum-general-discussion-announcements-and-releases/anyone-try-diamondback-nozzles/paged/3
- Prusa forum, stringing after Diamondback (temperature, not retraction):
  https://forum.prusa3d.com/forum/original-prusa-i3-mk3s-mk3-hardware-firmware-and-software-help/stringing-after-switching-to-diamondback-nozzle
- Direct-drive retraction guidance (0.8–1.2 mm @ 25–40 mm/s):
  https://craftlayerindia.com/blogs/3d-printer-settings/best-retraction-distance-direct-drive
  and https://www.3dmag.com/3d-wikipedia/3d-printer-retraction-settings-stringing-fix-guide
- 300 °C max-temp report for the MicroSwiss collab nozzle:
  https://www.reddit.com/r/Creality_k2/comments/1q528s0/microswiss_diamond_nozzle_above_300c
- E3D DiamondBack (background): https://e3d-online.com/pages/diamondback-nozzles
