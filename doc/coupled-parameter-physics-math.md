# Coupled Parameter Physics & Multi-Domain Differential/Algebraic Equations

**Companion to:** `doc/pwl-nozzle-compensation-math.md`  
**Scope:** Native coupled-physics synchronization (`capture_coupled_baseline`, `sync_coupled_physics`, `coupled_purge_factor`) across `src/lua_engine.rs`, `src/transformations.rs`, `src/threemf.rs`, and `transformations/*.lua`.

---

## 1. Architectural Motivation: Slicer Parameters Are Projections of Coupled PDEs

In fused filament fabrication (FFF), slicer configuration keys are not independent scalar variables. They are low-dimensional projections of four coupled physical conservation laws inside the hotend and across the newly deposited polymer bead:

1. **Capillary Momentum & Compressible Melt Rheology (Hagen–Poiseuille / Rabinowitsch–Mooney):**
   Couples **Pressure Advance** (`pressure_advance`) to **Nozzle Orifice Diameter** ($d$), **Nozzle Bore Tribology / Wall Slip** ($\mu_{\text{nozzle}}$), and **Active Melt Viscosity** ($\eta(T_{\text{eff}})$).
2. **Convective–Conductive Melt-Zone Heat Transfer (Graetz–Nusselt Problem):**
   Couples **Max Volumetric Speed** (`filament_max_volumetric_speed`, $Q_{\text{max}}$) to **Nozzle Thermal Conductivity & Internal Geometry** ($k_{\text{nozzle}}$, CHT splitter fins), **Orifice Exit-Land Diameter** ($d$), and **Net Temperature Excursion** ($\Delta T_{\text{net}}$), bounded above by **Euler Column Buckling** on elastomers.
3. **Nozzle-Tip Radiative & Gap-Conductive Heat Transfer to the Deposited Bead:**
   Couples **Part Cooling & Overhang Fan Curves** (`fan_min_speed`, `fan_max_speed`, `overhang_fan_speed`) to **Nozzle Tip Thermal Conductivity** ($k_{\text{nozzle}} = 543\text{ W/m}\cdot\text{K}$ on DiamondBack PCD + CuCrZr body), **Deposited Bead Linear Enthalpy** ($\propto d^2$), and **Net Temperature Excursion** ($\Delta T_{\text{net}}$).
4. **Laminar Residence-Time Distribution (RTD) & Wall Boundary-Layer Evacuation:**
   Couples the **Multi-Color Flush / Purge Matrix** (`flush_volumes_matrix`) to **Hotend Melt-Zone Internal Volume** (`melt_zone_multiplier`, $L_{\text{melt}}$), **Nozzle Bore Wall-Slip Velocity** ($\mu_{\text{nozzle}}$), and **Active Melt Viscosity Shift** ($\eta(\Delta T_{\text{net}})$).

When early or late transformation rules modify primary variables ($T_{\text{nozzle}}$, $d$, nozzle material $k / \mu$, hotend $L_{\text{melt}}$) without updating these coupled downstream variables:
- A static `0.85` Pressure Advance multiplier calibrated for a $0.4\text{ mm}$ DiamondBack produces severe corner gaps on a $0.8\text{ mm}$ nozzle or corner bulging when a downstream anti-stringing or wet-filament rule drops temperature by $-4\text{ to }-10\text{ °C}$.
- Dropping nozzle temperature for stringing/humidity without scaling `filament_max_volumetric_speed` pushes the hotend beyond its reduced Graetz thermal power limit, causing cold-extrusion extruder clicking.
- Running a $543\text{ W/m}\cdot\text{K}$ PCD tip or a $0.6\text{–}1.0\text{ mm}$ orifice at stock hardened-steel fan speeds leaves overhangs soft from tip radiant heat and high bead enthalpy.
- Calculating ACE Pro multi-color purge volumes without accounting for the FlowTech melt-zone volume, PCD wall-slip evacuation, and cold-melt viscosity either bleeds dark pigments into light filaments or wastes purge mass.

---

## 2. State Representation & Telescoping Multiplicative Synchronization

To ensure that coupled parameter updates:
1. **Compose cleanly across arbitrary rule chains** (e.g. `P10 thermal_conductivity` $\to$ `P25 symptom_stringing_diamondback` $\to$ `P55 sym_wet_filament` $\to$ `P57 structural_strength`),
2. **Preserve intermediate user/custom rule edits** (e.g. a user rule that scales `pressure_advance` by $\times 1.10$), and
3. **Never trigger false-positive rule shadowing (`RuleShadowKind::Overwritten`)** in the counterfactual lineage analyzer,

we formulate `sync_coupled_physics(config, ctx)` as a **telescoping multiplicative/additive group action** tracked via `config["_coupled_state"]`:

### 2.1 Thermal Equilibrium Reference $T_{\text{eq}}$ & Net Melt Excursion $\Delta T_{\text{net}}$

Let $T_0$ be the Brass baseline nozzle temperature of the filament spool (`seed_temp` before nozzle conductivity compensation).
When Rule P10 (`thermal_conductivity.lua`) maps $T_0 \mapsto T_{\text{P10}}$ via the nozzle material's thermal-lag compensation curve:
- On **Hardened Steel** ($k \approx 22\text{ W/m}\cdot\text{K}$), the heater block RTD is raised by $+5.5\text{ to }+7.0\text{ °C}$ ($T_{\text{P10}} \approx T_0 + 6\text{ °C}$) specifically to overcome radial conduction lag so the **internal polymer melt** reaches the **same** nominal temperature as Brass.
- On **DiamondBack Solid PCD** ($k = 543\text{ W/m}\cdot\text{K}$), the heater block RTD is lowered by $-6.5\text{ to }-10.5\text{ °C}$ ($T_{\text{P10}} \approx T_0 - 10\text{ °C}$) because near-zero radial thermal drop brings the bore wall to the melt temperature without superheating.

Thus, $T_{\text{eq}} \equiv T_{\text{P10}}$ defines the **nozzle thermal-equilibrium temperature** at which the polymer melt inside the bore has nominal reference viscosity $\eta_0$.

At any rule step $s \ge 1$ where the current `nozzle_temperature` is $T_{\text{eff}}^{(s)}$ (plus any pre-stack temperature shift $\Delta T_{\text{pre}}$ from climate/wet/structural side-channels), the **net melt-temperature excursion** away from thermal equilibrium is:

$$\Delta T_{\text{net}}^{(s)} = \Delta T_{\text{pre}} + \left(T_{\text{eff}}^{(s)} - T_{\text{eq}}\right)$$

### 2.2 Telescoping Update Law

For each coupled multiplicative parameter $y \in \lbrace\text{PA}, Q_{\text{max}}\rbrace$ with total physical coupling factor $\Phi_y(d, \text{nozzle}, \Delta T_{\text{net}}^{(s)})$:

- **Initial Synchronization ($s = 1$, when `_coupled_state` has no prior factor $\Phi_y^{(0)} = 1.0$):**

$$y^{(1)} = y^{(0)} \cdot \Phi_y\left(d, \text{nozzle}, \Delta T_{\text{net}}^{(1)}\right)$$

- **Subsequent Synchronizations ($s > 1$, after a downstream rule shifts $T_{\text{eff}}^{(s-1)} \to T_{\text{eff}}^{(s)}$):**

$$y^{(s)} = y^{(s-1)} \cdot \frac{\Phi_y\left(d, \text{nozzle}, \Delta T_{\text{net}}^{(s)}\right)}{\Phi_y\left(d, \text{nozzle}, \Delta T_{\text{net}}^{(s-1)}\right)}$$

By the telescoping product identity:

$$y^{(0)} \cdot \Phi_y^{(1)} \cdot \frac{\Phi_y^{(2)}}{\Phi_y^{(1)}} \cdots \frac{\Phi_y^{(k)}}{\Phi_y^{(k-1)}} = y^{(0)} \cdot \Phi_y^{(k)}$$

the final value is **path-independent** while every step $s$ is a genuine relative transformation of $y^{(s-1)}$ (`config[key]`), preserving both user multipliers and counterfactual lineage attribution.

Similarly, for additive fan duty-cycle offsets $\Delta\text{Fan}(d, \text{nozzle}, \Delta T_{\text{net}}^{(s)})$:

$$\text{Fan}^{(s)} = \text{Fan}^{(s-1)} + \left(\Delta\text{Fan}^{(s)} - \Delta\text{Fan}^{(s-1)}\right)$$

---

## 3. Group 1: Capillary Rheology & Pressure Advance — $\text{PA}(T, d, \mu_{\text{nozzle}})$

### 3.1 Physical Derivation

In a direct-drive extruder feeding a capillary orifice of diameter $d$ and land length $L_n$, the Hagen–Poiseuille pressure drop corrected for Navier wall slip (slip length $b(\mu_{\text{nozzle}})$) and shear-thinning power-law index $n$ is:

$$\Delta P = \frac{2\,\eta_{\text{eff}}(T)\,L_n}{R}\left(\frac{3n + 1}{n}\frac{Q}{\pi R^3}\right)^n \cdot \left(1 + \frac{4\,b(\mu_{\text{nozzle}})}{R}\right)^{-n}$$

The filament column between the extruder drive gears and the melt zone acts as a Hookean spring of effective compliance $\kappa_{\text{fila}} = \frac{L_{\text{fila}}}{E_{\text{bulk}} A_{\text{fila}}}$, storing excess elastic volume $\Delta V = \kappa_{\text{fila}} A_{\text{bore}} \Delta P$. Pressure Advance ($\text{PA} = \Delta V / Q$, units: $\text{s}$) is the time constant required to pre-charge and bleed this stored elastic volume during acceleration and deceleration.

Factoring $\text{PA}$ around the reference calibration $\text{PA}_0$ (measured on a $d_0 = 0.40\text{ mm}$ Brass/Hardened-Steel nozzle at $\Delta T_{\text{net}} = 0$):

$$\Phi_{\text{PA}}\left(d, \mu_{\text{nozzle}}, \Delta T_{\text{net}}\right) = c_{d,\text{PA}}(d) \cdot c_{\text{slip}}(\mu_{\text{nozzle}}) \cdot c_\eta(\Delta T_{\text{net}})$$

### 3.2 Component Factors

1. **Orifice Diameter Scaling $c_{d,\text{PA}}(d)$:**
   Across MicroSwiss FlowTech orifices ($d \in \lbrace 0.25, 0.40, 0.60, 0.80, 1.00\rbrace\text{ mm}$) where land-to-diameter ratio $L_n / d$ scales quasi-geometrically and power-law shear thinning ($n \approx 0.38$) moderates the Newtonian $d^{-3}$ dependence:

$$c_{d,\text{PA}}(d) = \left(\frac{0.40}{d}\right)^{1.35}$$

   | Orifice Diameter $d$ ($\text{mm}$) | $c_{d,\text{PA}}(d) = (0.40/d)^{1.35}$ | Physical Interpretation |
   | :---: | :---: | :--- |
   | $0.25$ | $1.886$ | High capillary constriction ($\implies +88.6$% higher PA) |
   | $0.40$ | $1.000$ | Reference orifice ($1.000\times$) |
   | $0.60$ | $0.578$ | Wider bore lowers backpressure ($\implies -42.2$% PA) |
   | $0.80$ | $0.392$ | Low capillary resistance ($\implies -60.8$% PA) |
   | $1.00$ | $0.290$ | Draft orifice ($\implies -71.0$% PA) |

2. **Nozzle Wall-Slip / Friction Factor $c_{\text{slip}}(\mu_{\text{nozzle}})$:**
   Polycrystalline diamond (PCD, $\mu = 0.01$ vs. $\mu \approx 0.25\text{–}0.35$ for Brass/Steel) and fluoropolymer/nanocomposite coatings induce partial wall slip and suppress exit die-swell, reducing capillary backpressure:

   | Nozzle Material / Type | CoF $\mu$ | $c_{\text{slip}}(\mu_{\text{nozzle}})$ |
   | :--- | :---: | :---: |
   | **DiamondBack Solid PCD** (`diamondback`) | $0.01$ | $0.850$ |
   | **MicroSwiss CHT High-Flow** (`cht`, `microswiss_cht`) | $0.18$ (3-bore) | $0.920$ |
   | **Tungsten Carbide / SiC** (`tungsten_carbide`, `sic`) | $0.12$ | $0.920$ |
   | **MicroSwiss CM2 / CHT CM2** (`cm2`, `cht_cm2`, TwinClad XT) | $0.08$ | $0.940$ |
   | **Brass / Hardened Steel / ObXidian** (`brass`, `hardened_steel`) | $0.28$ | $1.000$ |

3. **Arrhenius Melt-Viscosity Shift $c_\eta(\Delta T_{\text{net}})$:**
   Over a $\pm 15\text{ °C}$ window around $T_{\text{eq}}$, polymer melt viscosity follows $\eta(T) \approx \eta(T_{\text{eq}})\,\exp(-\alpha_\eta\,\Delta T_{\text{net}})$ with $\alpha_\eta = 0.012\text{ K}^{-1}$ ($1.2$% per $\text{°C}$):

$$c_\eta(\Delta T_{\text{net}}) = \mathrm{clamp}\left(1.0 - 0.012\,\Delta T_{\text{net}},\; 0.80,\; 1.25\right)$$

   - **Colder melt ($\Delta T_{\text{net}} < 0$):** Viscosity increases $\implies$ capillary backpressure rises $\implies \text{PA}$ increases by $+1.2$% per $-\text{°C}$.
   - **Hotter melt ($\Delta T_{\text{net}} > 0$):** Viscosity decreases $\implies$ capillary backpressure drops $\implies \text{PA}$ decreases by $-1.2$% per $+\text{°C}$.

4. **Domain Bounds:**

$$\text{PA} = \mathrm{clamp}\left(\text{PA}_0 \cdot \Phi_{\text{PA}},\; \text{PA}_{\min},\; \text{PA}_{\max}\right)$$

   with $[\text{PA}_{\min}, \text{PA}_{\max}] = [0.005, 0.120]\text{ s}$ for rigid polymers and $[0.010, 0.250]\text{ s}$ for elastomers (`TPU`, `TPE`, `PEBA`).

---

## 4. Group 2: Graetz Thermal Conductance & Max Volumetric Speed — $Q_{\text{max}}(T, d, k_{\text{nozzle}})$

### 4.1 Physical Derivation

In the hotend melt zone of length $L_{\text{melt}}$, solid filament enters at $T_{\text{amb}}$ and must be heated above its flow transition temperature $T_{\text{flow}}$ before reaching the nozzle exit. The dimensionless Graetz number is:

$$\text{Gz} = \frac{\rho\,c_p\,Q}{\pi\,k_{\text{eff}}\,L_{\text{melt}}}$$

At the $d_0 = 0.40\text{ mm}$ reference orifice and P10 thermal equilibrium ($\Delta T_{\text{net}} = 0$), Rule P10's temperature curve already balances nozzle wall thermal conductivity $k_{\text{nozzle}}$ against the heater setpoint offset $\Delta T_{\text{P10}}$ so that single-bore nozzles deliver $1.00\times$ nominal plasticization capacity. However, when the user changes orifice diameter ($d \ne 0.40\text{ mm}$), uses a multi-bore CHT splitter, or shifts temperature away from P10 equilibrium ($\Delta T_{\text{net}} \ne 0$), $Q_{\text{max}}$ scales as:

$$\Phi_{\text{MVS}}\left(d, \text{nozzle}, \Delta T_{\text{net}}\right) = c_{\text{bore}}(\text{nozzle}) \cdot c_{d,Q}(d, \text{nozzle}) \cdot c_T(\Delta T_{\text{net}})$$

### 4.2 Component Factors

1. **Internal Splitter Factor $c_{\text{bore}}(\text{nozzle})$:**
   - **Single-Bore Nozzles** (Brass, Hardened Steel, DiamondBack PCD, CM2, TC, SiC): $c_{\text{bore}} = 1.000$ at $d = 0.40\text{ mm}$, $\Delta T_{\text{net}} = 0$.
   - **Multi-Bore Splitter Nozzles** (`cht`, `cht_cm2`, `microswiss_cht`): 3 internal melt channels halve the radial conduction path inside the polymer ($R_{\text{poly}} \propto r_{\text{channel}}^2$), giving $c_{\text{bore}} = 1.150$.

2. **Orifice Exit-Land & High-Graetz Scaling $c_{d,Q}(d, \text{nozzle})$:**
   Widening the orifice ($d > 0.40\text{ mm}$) relieves exit-land shear pressure drop ($\Delta P_{\text{land}} \propto d^{-4}$), shifting the bottleneck from stepper torque to radial heat conduction across the melt zone:

$$c_{d,Q}(d, \text{nozzle}) = \left(\frac{d}{0.40}\right)^{\beta_Q(\text{nozzle})}$$

   where the exponent $\beta_Q(\text{nozzle})$ reflects how effectively the nozzle wall sustains radial heat flux at high volumetric throughput:
   - **CHT / CHT CM2:** $\beta_Q = 0.38$
   - **Brass / CM2 / Tungsten Carbide / SiC:** $\beta_Q = 0.34$
   - **DiamondBack PCD:** $\beta_Q = 0.30$ (PCD's $543\text{ W/m}\cdot\text{K}$ conductivity is partially counterbalanced at very high flow by P10's $-10\text{ °C}$ lower wall temperature $\Delta T_{\text{heater}}$, yielding $+13.0$% at $0.6\text{ mm}$, $+23.1$% at $0.8\text{ mm}$, $+31.6$% at $1.0\text{ mm}$, and $-13.2$% at $0.25\text{ mm}$)
   - **Hardened Steel:** $\beta_Q = 0.22$ (radial wall resistance $1/k_{\text{HS}}$ limits high-flow gains on large orifices)

3. **Net Temperature Excursion Factor $c_T(\Delta T_{\text{net}})$:**
   Deviating from the nozzle's P10 equilibrium temperature changes both the thermal driving potential $(T_{\text{wall}} - T_{\text{melt}})$ and the capillary viscosity:

$$c_T(\Delta T_{\text{net}}) = \mathrm{clamp}\left(1.0 + 0.015\,\Delta T_{\text{net}},\; 0.82,\; 1.15\right)$$

   - Dropping temperature by $-2\text{ °C}$ in P25 (`symptom_stringing_diamondback`) trims $Q_{\text{max}}$ by $-3.0$%.
   - Dropping temperature by $-8\text{ °C}$ in P55 (`sym_wet_filament`) trims $Q_{\text{max}}$ by $-12.0$%.
   - Boosting temperature by $+8\text{ °C}$ in P57 (`structural_strength`) increases $Q_{\text{max}}$ by $+12.0$%.

4. **Euler Buckling Guard for Elastomers & Machine Flow Ceiling:**
   - For elastomers (`filament_is_elastomer == true`, e.g. `TPU 95A`, `TPU 85A`, `PEBA`) and foaming filaments, volumetric speed is limited by Euler column buckling in the extruder throat rather than melt-zone conduction. Therefore, for elastomers:

$$\Phi_{\text{MVS,elastomer}} = \min\left(1.0,\; \Phi_{\text{MVS}}\right)$$

     so cold-temperature excursions or $0.25\text{ mm}$ orifices safely reduce $Q_{\text{max}}$, while wide orifices or CHT splitters never push TPU above its buckling limit.
   - Final $Q_{\text{max}}$ is clamped to $[1.5,\; Q_{\text{ceiling}}]\text{ mm}^3/\text{s}$, where $Q_{\text{ceiling}} = \min\left(Q_{\text{hotend-max}},\, \max(Q_0, 22.0)\right)$.

---

## 5. Group 3: Nozzle Tip Radiant Heat Transfer & Part Cooling — $\text{Fan}(k_{\text{nozzle}}, \Delta T_{\text{net}}, d)$

### 5.1 Physical Derivation

As the nozzle traverses overhangs and short-layer-time features, the flat annular face surrounding the orifice transfers heat downward into the freshly deposited polymer bead via gap conduction across the thin air film and thermal radiation:

$$\dot{q}_{\text{tip}\to\text{bead}} = \frac{k_{\text{air}}}{\delta_{\text{layer}}}\,\left(T_{\text{tip-face}}(k_{\text{nozzle}}) - T_{\text{bead}}\right) + \varepsilon\,\sigma\,\left(T_{\text{tip-face}}^4 - T_{\text{bead}}^4\right)$$

On a **Hardened Steel** nozzle ($k \approx 22\text{ W/m}\cdot\text{K}$), part-cooling airflow over the nozzle cone cools the external tip flat by $-8\text{ to }-12\text{ °C}$ below the heater block RTD. On a **DiamondBack Solid PCD** tip ($k = 543\text{ W/m}\cdot\text{K}$) brazed into a **CuCrZr** shank ($k \approx 320\text{ W/m}\cdot\text{K}$), internal conduction replenishes tip heat almost instantaneously, keeping $T_{\text{tip-face}}$ within $1.5\text{ °C}$ of the melt core even when the heater block setpoint is $-10\text{ °C}$ lower.

Simultaneously, increasing orifice diameter ($d > 0.40\text{ mm}$) increases the cross-sectional area of the deposited bead $A_{\text{bead}} = w \cdot h \propto d^2$, increasing the linear enthalpy $\dot{H}' = \rho\,c_p\,A_{\text{bead}}\,(T_{\text{eff}} - T_g)$ that the part-cooling fan must extract per millimeter of travel.

### 5.2 Additive Fan Duty-Cycle Law

For **fan-cooled polymers** (polymers with baseline `fan_min_speed` $\ge 15$% and not in the enclosed warp-sensitive family $\lbrace\text{ABS}, \text{ASA}, \text{PA}, \text{PA-CF}, \text{PA-GF}, \text{PC}, \text{PPS}, \text{PEEK}, \text{HIPS}\rbrace$):

$$\Delta\text{Fan}_{\min}\left(d, k_{\text{nozzle}}, \Delta T_{\text{net}}\right) = \Delta\text{Fan}_{k,\min}(\text{nozzle}) + \Delta\text{Fan}_d(d) + \Delta\text{Fan}_T(\Delta T_{\text{net}})$$

$$\Delta\text{Fan}_{\max}\left(d, k_{\text{nozzle}}, \Delta T_{\text{net}}\right) = \Delta\text{Fan}_{k,\max}(\text{nozzle}) + \mathrm{round}\left(0.6\,\Delta\text{Fan}_d(d)\right) + \mathrm{round}\left(0.5\,\Delta\text{Fan}_T(\Delta T_{\text{net}})\right)$$

where:
1. **Nozzle Tip Conductivity Offset $\Delta\text{Fan}_k(\text{nozzle})$:**
   - **DiamondBack PCD (`diamondback`):** $\Delta\text{Fan}_{k,\min} = +6$%, $\Delta\text{Fan}_{k,\max} = +4$%, $\Delta\text{Fan}_{k,\text{overhang}} = +5$%
   - **Tungsten Carbide / SiC (`tungsten_carbide`, `sic`):** $\Delta\text{Fan}_{k,\min} = +3$%, $\Delta\text{Fan}_{k,\max} = +2$%, $\Delta\text{Fan}_{k,\text{overhang}} = +3$%
   - **CM2 / CHT (`cm2`, `cht`, `cht_cm2`):** $\Delta\text{Fan}_{k,\min} = +2$%, $\Delta\text{Fan}_{k,\max} = +0$%, $\Delta\text{Fan}_{k,\text{overhang}} = +2$%
   - **Brass / Hardened Steel:** $+0$%
2. **Orifice Bead-Enthalpy Offset $\Delta\text{Fan}_d(d)$:**

$$\Delta\text{Fan}_d(d) = \mathrm{round}\left(12.0 \cdot \max(0.0,\; d - 0.40)\right)$$

   giving $+0$% at $d \le 0.40\text{ mm}$, $+2$% at $0.60\text{ mm}$, $+5$% at $0.80\text{ mm}$, and $+7$% at $1.00\text{ mm}$.
3. **Net Temperature Excursion Offset $\Delta\text{Fan}_T(\Delta T_{\text{net}})$:**

$$\Delta\text{Fan}_T(\Delta T_{\text{net}}) = \mathrm{round}\left(\mathrm{clamp}(0.40\,\Delta T_{\text{net}},\; -5.0,\; +6.0)\right)$$

   *(Note: If `_structural_fan_cut_pct > 0` is active from the Structural Strength pack, positive fan boosts on `fan_max_speed` are suppressed so the user's deliberate interlayer-fusion fan cut is honored).*
4. **Monotonicity & Bounds:**
   `fan_max_speed` is clamped to $[0, 100]$%, and `fan_min_speed` is clamped to $[0, \text{fan-max-speed}]$%.

---

## 6. Group 4: Multi-Color Purge / Flush Matrix — $V_{\text{flush}}(L_{\text{melt}}, \mu_{\text{nozzle}}, \Delta T_{\text{net}})$

### 6.1 Physical Derivation

During an ACE Pro multi-color toolchange from slot $i$ to slot $j$ ($i \ne j$), incoming polymer must displace the molten inventory residing in the hotend melt zone ($V_{\text{melt}} \propto L_{\text{melt}}$) and strip residual pigment from the velocity boundary layer along the nozzle wall.

In laminar pipe flow with Navier slip length $b(\mu_{\text{nozzle}})$, the asymptotic tail of the residence-time distribution (RTD) decays with the wall shear rate $\dot{\gamma}_w$. Low-friction, non-wetting surfaces (DiamondBack PCD, $\mu = 0.01$) and uniform wall temperature ($543\text{ W/m}\cdot\text{K}$) accelerate boundary-layer evacuation, whereas a longer hotend melt zone (`melt_zone_multiplier` $> 1.0$) or colder, more viscous melt ($\Delta T_{\text{net}} < 0$) increases required purge volume.

### 6.2 Coupled Purge Factor (`coupled_purge_factor(config, ctx)`)

$$\gamma_{\text{purge}}\left(L_{\text{melt}}, \mu_{\text{nozzle}}, \Delta T_{\text{net}}\right) = \mathrm{clamp}\left(c_{\text{melt}}(L_{\text{melt}}) \cdot c_{\text{wall-slip}}(\mu_{\text{nozzle}}) \cdot c_{\eta,\text{purge}}(\Delta T_{\text{net}}),\; 0.70,\; 1.65\right)$$

where:
1. **Hotend Melt-Zone Volume Multiplier $c_{\text{melt}}(L_{\text{melt}})$:**

$$c_{\text{melt}} = \mathrm{clamp}(\text{ctx.melt-zone-multiplier},\; 0.75,\; 1.60)$$

   (e.g. $1.00$ for stock Kobra S1 hotend; $1.15\text{–}1.25$ for MicroSwiss FlowTech unitary hotend).
2. **Nozzle Wall-Slip Evacuation Factor $c_{\text{wall-slip}}(\mu_{\text{nozzle}})$:**
   - **DiamondBack Solid PCD (`diamondback`):** $c_{\text{wall-slip}} = 0.800$ ($-20$% boundary-layer pigment stagnation; on a $1.25\times$ FlowTech melt zone at $\Delta T_{\text{net}} = 0$, $1.25 \times 0.800 = 1.000$, exactly neutralizing the extra FlowTech internal volume!)
   - **CM2 / CHT CM2 (`cm2`, `cht_cm2`):** $c_{\text{wall-slip}} = 0.920$
   - **Tungsten Carbide / SiC:** $c_{\text{wall-slip}} = 0.940$
   - **MicroSwiss CHT Brass (`cht`):** $c_{\text{wall-slip}} = 1.060$ ($+6$% wetted surface area across the 3 internal splitter fins)
   - **Brass / Hardened Steel:** $c_{\text{wall-slip}} = 1.000$
3. **Melt-Viscosity Purge Factor $c_{\eta,\text{purge}}(\Delta T_{\text{net}})$:**

$$c_{\eta,\text{purge}}(\Delta T_{\text{net}}) = \mathrm{clamp}\left(1.0 - 0.008\,\Delta T_{\text{net}},\; 0.90,\; 1.15\right)$$

   Printing colder ($\Delta T_{\text{net}} < 0$) increases melt viscosity and thickens the slow-moving wall boundary layer, requiring $+0.8$% additional purge volume per $-\text{°C}$.

Each off-diagonal cell $V_{i \to j}$ ($i \ne j$) in `transformations/flush_volumes_matrix.lua` scales by $\gamma_{\text{purge}}$:

$$V_{i \to j} = \mathrm{clamp}\left(\mathrm{round}\left(V_{i \to j,\text{base}} \cdot \text{flush-mult} \cdot \gamma_{\text{purge}}\right),\; 70,\; 900\right)$$

---

## 7. Verification Matrix Across Orifice Diameters, Nozzles & Temperature Excursions

Reference spool: **PETG** ($T_0 = 230\text{ °C}$ Brass, $\text{PA}_0 = 0.040\text{ s}$, $Q_0 = 15.0\text{ mm}^3/\text{s}$, $\text{Fan}_{\min,0} = 40$%, $\text{Fan}_{\max,0} = 70$%).

| Nozzle Type | Orifice $d$ ($\text{mm}$) | Active Rule Stage | $T_{\text{eff}}$ ($\text{°C}$) | $\Delta T_{\text{net}}$ ($\text{°C}$) | Coupled $\text{PA}$ ($\text{s}$) | Coupled $Q_{\text{max}}$ ($\text{mm}^3/\text{s}$) | Coupled $\text{Fan}_{\min} / \text{Fan}_{\max}$ (%) | Coupled $\gamma_{\text{purge}}$ ($L_{\text{melt}}=1.25$) |
| :--- | :---: | :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| **Brass** | $0.40$ | Baseline | $230$ | $0$ | $0.040$ | $15.0$ | $40 / 70$ | $1.250$ |
| **Hardened Steel** | $0.40$ | P10 Equilibrium | $236$ | $0$ | $0.040$ | $15.0$ | $40 / 70$ | $1.250$ |
| **Hardened Steel** | $0.40$ | P10 + P55 Wet ($-8\text{ °C}$) | $228$ | $-8$ | $0.044$ | $13.2$ | $37 / 68$ | $1.330$ |
| **DiamondBack PCD** | $0.25$ | P10 Equilibrium | $220$ | $0$ | $0.064$ | $13.0$ | $46 / 74$ | $1.000$ |
| **DiamondBack PCD** | $0.40$ | P10 Equilibrium | $220$ | $0$ | $0.034$ | $15.0$ | $46 / 74$ | $1.000$ |
| **DiamondBack PCD** | $0.40$ | P10 + P25 ($-2\text{ °C}$) | $218$ | $-2$ | $0.035$ | $14.6$ | $45 / 74$ | $1.016$ |
| **DiamondBack PCD** | $0.60$ | P10 + P25 ($-2\text{ °C}$) | $218$ | $-2$ | $0.020$ | $16.4$ | $47 / 75$ | $1.016$ |
| **DiamondBack PCD** | $0.80$ | P10 + P25 ($-2\text{ °C}$) | $218$ | $-2$ | $0.014$ | $17.9$ | $50 / 77$ | $1.016$ |
| **DiamondBack PCD** | $1.00$ | P10 + P25 ($-2\text{ °C}$) | $218$ | $-2$ | $0.010$ | $19.2$ | $52 / 78$ | $1.016$ |
| **MicroSwiss CHT** | $0.60$ | P10 Equilibrium | $230$ | $0$ | $0.021$ | $20.1$ | $44 / 71$ | $1.325$ |

---

## 8. Additional Coupled Transformations Across the Stack (Groups 5–9)

Auditing all core transformations (`transformations/*.lua`) and all 28 parametric transformations (`transformations/parametric/*.lua`) identified five additional coupled parameter families where modifying a primary parameter previously left coupled downstream parameters unsynchronized:

### 8.1 Group 5: Symptom Temperature Shifts Coupled to Rheology, Melt Power & Cooling
All symptom wizards that shift `nozzle_temperature` (`sym_stringing.lua`, `sym_weak_layers.lua`, `sym_under_extrusion.lua`, `sym_extruder_grinding.lua`, and `sym_clogging.lua`) invoke `sync_coupled_physics(config, ctx)` after applying their temperature adjustment:
- If a rule explicitly modifies `filament_max_volumetric_speed` (e.g. `sym_under_extrusion`, `sym_clogging`) or `fan_max_speed` (e.g. `sym_weak_layers`, `structural_strength`) in the same step, `sync_coupled_physics` honors the rule's explicit write for that key while synchronizing the remaining coupled parameters (`pressure_advance`, `overhang_fan_speed`, etc.) with the temperature shift $\Delta T_{\text{net}}$.
- If a rule runs standalone without P10 (`thermal_conductivity.lua`) preceding it, `sync_coupled_physics` automatically recovers the pre-mutation `nozzle_temperature` from the interpreter's modification log as $T_{\text{eq}}$.

### 8.2 Group 6: Coupled Shell Thickness vs. Shell Layer Count & Layer Height
In OrcaSlicer, effective solid shell thickness is governed by:

$$\text{thickness}_{\text{eff}} = \max\left(N_{\text{shells}} \cdot h_{\text{layer}},\; \text{shell-thickness}\right)$$

- **Optical Glass Rules (`transparent_optical_glass.lua`, `sym_transparent_glass.lua`):** Setting `top_shell_layers = "0"` and `bottom_shell_layers = "0"` without zeroing `top_shell_thickness` / `bottom_shell_thickness` leaves the stock $0.80\text{ mm}$ thickness floor active, causing the slicer to emit solid shells anyway. Both rules now couple `top_shell_thickness = "0.00"` and `bottom_shell_thickness = "0.00"` whenever shell layer count is zeroed.
- **Structural Strength Pack (`structural_strength.lua`):** When `top_shell_thickness` or `bottom_shell_thickness` is present in `config`, it is coupled to:

$$\text{shell-thickness} = N_{\text{shells}} \cdot h_{\text{layer}}$$

### 8.3 Group 7: Coupled Volumetric Speed Ceiling for Solid Infill & Top Surface (`volumetric_speed_ceiling.lua`)
In addition to `outer_wall_speed`, `inner_wall_speed`, and `sparse_infill_speed`, `volumetric_speed_ceiling.lua` now caps `internal_solid_infill_speed` and `top_surface_speed` (when present in `config`) by their feature-specific volumetric ceiling:

$$v_{f,\max} = \left\lfloor \frac{\min(Q_{\text{hotend}}, Q_{\text{filament}})}{h_{\text{layer}} \cdot w_f} \right\rfloor, \quad f \in \lbrace\text{internal-solid-infill}, \text{top-surface}\rbrace$$

### 8.4 Group 8: Coupled Ironing Pass Spacing vs. Nozzle Orifice Diameter (`ironing_tuning.lua`)
During top-surface ironing, constant pass-to-pass overlap ratio across nozzle diameters $d \in [0.25, 1.00]\text{ mm}$ requires scaling the lateral pass step-over $s_{\text{iron}}$ sub-linearly with the nozzle tip flat:

$$s_{\text{iron}}(d) = \mathrm{clamp}\left(\mathrm{round}_{0.01}\left(s_{\text{iron},0} \cdot \left(\frac{d}{0.40}\right)^{0.75}\right),\; 0.08,\; 0.30\right)$$

where $s_{\text{iron},0} = 0.15\text{ mm}$ for PLA/PETG/PCTG and $0.12\text{ mm}$ for ASA/ABS/PC/PA in `auto_polymer` mode.

### 8.5 Group 9: Coupled Support XY Clearance, Support Roof Spacing & Wet-Filament First-Layer Width vs. Nozzle Diameter
1. **Support Tuning (`support_tuning.lua`):**
   - In `auto_polymer` mode (when $z_{\text{gap}} > 0$), support interface line spacing scales with nozzle diameter:

$$s_{\text{interface}}(d) = \mathrm{clamp}\left(\mathrm{round}_{0.01}\left(s_{\text{interface},0} \cdot \sqrt{\frac{d}{0.40}}\right),\; 0.10,\; 0.40\right)$$

   - For wide nozzles ($d > 0.42\text{ mm}$), radial die-swell requires coupling XY clearance between support columns and vertical model walls:

$$d_{xy,\text{support}}(d) = \max(0.35,\; 0.85\,d)$$

2. **Wet Filament First-Layer Pinhole Recovery (`sym_wet_filament.lua`):**
   - First-layer bead width scales proportionally with orifice diameter $d$:

$$w_{\text{init}}(d, \text{RH}) = w_{\text{init},0}(\text{RH}) \cdot \left(\frac{d}{0.40}\right)$$

     preserving $0.56\text{–}0.60\text{ mm}$ at $d = 0.40\text{ mm}$ while preventing over-wide blowout on $0.25\text{ mm}$ nozzles ($0.35\text{–}0.38\text{ mm}$) and under-wide beads on $0.60\text{–}0.80\text{ mm}$ nozzles ($0.84\text{–}1.20\text{ mm}$).

---

## 9. Lua & Rust Engine API Reference

Three native builtins are registered in `LuaEnv::new` (`src/lua_engine.rs`) with zero Lua instruction/gas overhead:

1. **`capture_coupled_baseline(config, ctx)`**:
   Captures the initial `nozzle_temperature` ($T_0$) and initializes `config["_coupled_state"]` if not yet present. Called at the top of `transformations/thermal_conductivity.lua` before P10 mutates `nozzle_temperature`.
2. **`sync_coupled_physics(config, ctx)`**:
   Computes $\Phi_{\text{PA}}$, $\Phi_{\text{MVS}}$, and $\Delta\text{Fan}$ from $(d, \text{nozzle}, \Delta T_{\text{net}})$ and applies the telescoping relative update to `pressure_advance`, `filament_max_volumetric_speed`, `fan_min_speed` (+ `_BRASS` / `_HS`), `fan_max_speed` (+ `_BRASS` / `_HS`), and `overhang_fan_speed` whenever those keys are present in `config` and were not explicitly overridden in the same rule step. Called in:
   - `transformations/thermal_conductivity.lua` (P10)
   - `transformations/symptom_stringing_diamondback.lua` (P25)
   - `transformations/filament_pctg_antistringing.lua` (P48)
   - `transformations/parametric/sym_wet_filament.lua` (P55)
   - `transformations/parametric/structural_strength.lua` (P57)
   - `transformations/parametric/sym_stringing.lua`
   - `transformations/parametric/sym_weak_layers.lua`
   - `transformations/parametric/sym_under_extrusion.lua`
   - `transformations/parametric/sym_extruder_grinding.lua`
   - `transformations/parametric/sym_clogging.lua`
3. **`coupled_purge_factor(config, ctx) -> number`**:
   Returns $\gamma_{\text{purge}}\left(L_{\text{melt}}, \mu_{\text{nozzle}}, \Delta T_{\text{net}}\right) \in [0.70, 1.65]$ for use in `transformations/flush_volumes_matrix.lua`.
4. **`constrain(key, min, max, reason)`**:
   Registers box bounds $[L, U]$ on `key` in `bounds[key]` (tightening via $L \leftarrow \max(L, \text{min})$, $U \leftarrow \min(U, \text{max})$) for projection in the Phase 3 Epilogue Pass.

---

## 10. Phase 3 Epilogue Constraint Projection (`Priority = ∞`) & Deferred Schema Quantization

At the end of `TransformationEngine::evaluate_stack` (`src/transformations.rs`), after all enabled Lua rules (`P10 .. P98`) have executed at full IEEE-754 floating-point precision (`_defer_quantization = "1"`), Phase 3 enforces three final passes:

1. **Phase 3.1 — Box-Constraint Projection & Infeasibility Detection ($L > U$):**
   - For every key with registered `bounds[key]`, checks whether $L > U$ (logging an `Infeasible Constraint Conflict` warning if two rules declared mutually exclusive bounds) and projects the full-precision value onto $[L, U]$ (prioritizing the upper hardware safety ceiling $U$ when $L > U$).
   - Records any active clamp in the Provenance ledger as a `🛡️ Constraint Projection (Priority ∞)` step (`RuleShadowKind::Clamped`).
2. **Phase 3.2 — Cross-Key Relational Invariants (`Priority = ∞`):**
   Enforces coupled multi-parameter invariants before quantization:
   - **(a) First-Layer vs. Upper-Layer Thermal Continuity:** Enforces $T_{\text{initial-layer}} \in [T_{\text{nozzle}} - 5\text{ °C},\; T_{\text{nozzle}} + 20\text{ °C}]$ intersected with `bounds[init_key]`.
   - **(a2) Filament First-Layer Non-Drop Invariant:** On filament presets, enforces $T_{\text{initial-layer}} \ge T_{\text{nozzle}}$ so upper-layer cooling trims never leave Layer 1 colder than upper layers.
   - **(b) Retraction Prime Speed $\le$ Pull Speed:** Enforces `filament_deretraction_speed <= filament_retraction_speed` whenever deretraction speed $> 0$.
   - **(c) Spiral Lift Z-Hop Floor:** When `filament_z_hop_types == "Spiral Lift"` and Z-hop $> 0$, enforces `filament_z_hop >= 0.5 * layer_height` (bounded above by `bounds["filament_z_hop"].max`).
   - **(d) Fan Max Speed $\ge$ Fan Min Speed:** Enforces `fan_max_speed* >= fan_min_speed*`.
   - **(e) Structural Strength Fan Cut Ceiling:** Applies `_structural_fan_cut_pct` across `fan_max_speed*` and clamps `fan_min_speed*` to the reduced ceiling.
   - **(f) Process Preset Speed Policy, Outer-Wall VFA Guard & Seam Enum Normalization:** Projects process speeds against the VFA sub-resonance guard (`55–60 mm/s` vs. `>= 145 mm/s`) and normalizes `seam_slope_type = "contour"` to OrcaSlicer's C++ `"external"` enum.
3. **Phase 3.3 — Single Final Schema Quantization (`format_orca_schema_scalar`):**
   - Quantizes every modified parameter in `current_values` and every step in the Provenance ledger **once** according to OrcaSlicer's per-key schema precision (`0` decimals for temperatures/speeds/accelerations/fans, `1` decimal for `max_volumetric_speed`, `2` decimals for retractions/Z-hop/line widths/layer heights, and `3` decimals for `filament_flow_ratio`, `pressure_advance`, and XY tolerances).
