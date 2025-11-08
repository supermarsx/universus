# OGame‑style Moon Mechanics — Detailed Spec

> Purpose: Define complete game mechanics for “moons” in an OGame‑like browser MMO (server + client rules, balance knobs, data models, APIs, UI/UX, anti‑abuse). This spec is versioned and intentionally parameterized so values can be tuned during playtests.

---

## 1) Core Concept & Lifecycle
**Moons** are special orbital bodies bound to a specific planet (same coordinates, `slot` 1–15). They are created by combat debris and provide strategic utilities: **Sensor Phalanx (lanx scans), Jump Gate (instant ship relocation), stealth basing** (no resource production, limited build grid), and **phalanx evasion** for fleets.

**Moon Lifecycle States**: `Nonexistent → Candidate (moonchance) → Created → Developed → Targeted for Destruction → Destroyed (reverts to planet‑only)`.

---

## 2) Acquisition (Moonchance)
- **Trigger**: A battle at a planet that generates a debris field at or above a threshold.
- **Moonchance Formula (configurable)**:  
  `moonchance_pct = min(MOONCHANCE_CAP, floor(debris_metal_crystal / MOONCHANCE_UNIT))`
  - Default knobs: `MOONCHANCE_UNIT = 100,000`, `MOONCHANCE_CAP = 20` (i.e., max 20%).
  - Only **metal + crystal** in debris count toward chance (deuterium excluded).
- **Roll Timing**: Immediately after combat report resolution. One roll per combat instance.
- **Multiple Rolls**: If multiple combats happen before the debris is collected, each combat can roll independently if its own debris meets threshold.
- **Debris Persistence**: Debris remains regardless of roll outcome.
- **Moon Creation**: On success, spawn a Moon entity attached to the planet’s coordinates.

### 2.1 Moon Size & Fields
- **Diameter (km)** *(configurable functional form)*:  
  `diameter = clamp(BASE_DIAM + SIZE_PER_% * moonchance_pct + rand(-SIZE_JITTER, SIZE_JITTER), DIAM_MIN, DIAM_MAX)`  
  Defaults: `BASE_DIAM=2000`, `SIZE_PER_%=250`, `SIZE_JITTER=200`, bounds `DIAM_MIN=2000`, `DIAM_MAX=12000`.
- **Build Fields**: Moons start compact.  
  `base_fields = 1`  
  **Lunar Base** adds capacity (see §4.1). Net field math accounts for the Lunar Base occupying one field.

---

## 3) Moon Attributes & Rules
- **Orbit & Ownership**: Shares the planet’s coordinate triplet; inherits ownership from planet owner at create time. Ownership only changes via normal account transfer or ban actions, not by fleets.
- **No Resource Production**: Moons cannot build mines or power plants. No native energy economy on moons.
- **Unique Building Roster**: Focus on utility/defense (see §4).
- **Local Queues**: Separate build/shipyard/defense queues from the planet’s queues.
- **Fog of War**: Moons are not visible on galaxy map until created; after creation they appear as a moon icon.

---

## 4) Moon Buildings (Costs, Effects, Caps)
Costs use classic exponential growth. Define with `base_cost`, `multiplier`, and optional `energy/deut` upkeep.

### 4.1 Lunar Base (LB)
- **Role**: Expands buildable fields on the moon and unlocks core utilities.
- **Effect**: `+FIELDS_PER_LB = 3` per level; **net** usable fields per level = `+FIELDS_PER_LB−1` (LB occupies 1 field).
- **Reqs**: None for level 1. Higher utility buildings require LB≥1.
- **Costs** (default): base `{metal:20k, crystal:40k, deut:0}`, `multiplier:2.0`.
- **Cap**: `LB_MAX = 10` (tunable).

### 4.2 Sensor Phalanx (SP)
- **Role**: Scans for active fleet movements in nearby star systems.
- **Range**: `range_systems_each_side = SP_level^2 − 1`.
- **Scan Cost**: `SCAN_COST_DEUT = 5,000` per scan (tunable). No energy requirement.
- **Reqs**: LB≥1, Computer Tech≥8 (tunable).
- **Info Revealed**: For *planet* origins/targets only (never moon↔moon or moon legs): mission type, origin & destination coordinates, fleet timings (arrival/return), speed factor. **No ship composition** is shown.
- **Rate Limits**: `SCAN_COOLDOWN = 3s` per moon; `DAILY_SCAN_CAP` optional.
- **Counters**:  
  - Launch from/to moons (phalanx‑proof legs).  
  - Timing drift: Randomized micro‑jitter on displayed times (±1s) to punish tight interceptions (tunable).  
  - Interdict SP by destroying the moon.

### 4.3 Jump Gate (JG)
- **Role**: Instantly relocates ships between *owned* moons with JG built.
- **Cooldown**: `JG_COOLDOWN = 60 min` per gate (cooldown tracked per‑moon, not global).
- **Transfer Rules**:  
  - Transfers **ships only**; **no resources** move via gate.  
  - No fuel cost.  
  - Cargo ships arrive empty; if they carried resources, those are dropped before jump and remain on origin.
- **Reqs**: LB≥1, Hyperspace Tech≥7, Shipyard≥1 (tunable).
- **Capacity**: Unlimited by tonnage; bounded only by cooldown. (Optional variant: capacity window per jump.)

### 4.4 Shipyard (Moon)
- **Role**: Builds ships/defenses locally on the moon.
- **Reqs/Costs**: As planet Shipyard but separate level & queue.
- **Limits**: All standard ships & defenses allowed **except** IPM/ABM if Missile Silo is disallowed on moons (see §4.6).

### 4.5 Robotics Factory (Moon)
- **Role**: Reduces building times on the moon as on planets. Nanite interacts normally.

### 4.6 Missile Silo (Moon) — **Design Option**
- **Variant A (Default)**: **Disallow** silos on moons to keep moons utility‑centric.  
- **Variant B**: Allow, but missiles cannot target/garrison moons; they still target planets only.

### 4.7 Terraformer/Moon Fields — **Design Option**
- Not in classic rules. Optional **Moon Terraformer** that consumes deuterium to add small field counts beyond LB caps.

---

## 5) Fleet Visibility & Phalanx Rules
- **Planet Legs Visible**: Any fleet leg whose origin **and** destination are *planets* within SP range is scannable.
- **Moon Legs Invisible**: Any leg that **starts or ends at a moon** is **not scannable** (including deploy, harvest, ACS, return legs). This is the primary counterplay.
- **ACS/Group Attacks**: Coordinated attacks show as attack legs to the planet; composition details remain hidden.
- **Recall Handling**: On recall of a scannable leg, subsequent scans reflect the return leg timing.

---

## 6) Moon Destruction (RIP Attacks)
- **Attacker**: Uses **Death Stars (RIPs)** with mission `Destroy Moon` launched to the target planet’s coordinates (auto‑resolves at the moon). Requires RIP tech/ship.
- **Outcomes**: (a) Moon destroyed; (b) Moon survives and attacker RIPs may be lost.
- **Probability Model (configurable, OGame‑like)**:
  - Inputs: `n = RIP_count`, `d = moon_diameter_km`.
  - Success chance:  
    `p_destroy = clamp( (A * sqrt(n)) * max(0, 100 − sqrt(d)) / 100, 0, 100 )`  
    Default `A = 1.0` (tune during tests).
  - RIP loss chance on failure:  
    `p_lose = clamp( B * sqrt(d) / 2, 0, 100 )` with `B = 1.0` (tunable).  
  - On success: Moon entity deleted → planet remains; **all fleets/queues on moon are canceled** (ships/defenses in build are lost), stationed ships are destroyed. JG link severed.  
  - On failure with RIP loss: Resolve RIP casualties via binomial draw with `p_lose` or fixed fraction (design choice).
- **Cooldown/Spam Control**: Destroy missions have a minimum flight time (e.g., ≥ 30 min one way) and cannot be ACS’ed.

---

## 7) Balance & Anti‑Abuse
- **Scan Abuse**: Per‑moon scan cooldown + daily cap + IP throttling server‑side.
- **JG Abuse**: Per‑gate cooldown; jumping clears fleet orders (ships arrive idle) to avoid infinite loop exploits.
- **Parking Abuse**: Optional docking fees on moons are **not** used (keep classic feel). Instead rely on discoverability limits.
- **RIP Spam**: Hard min flight time and launch cooldown per target coordinate.

---

## 8) Server Data Model (Relational Sketch)
- `celestial (id, owner_id, type: {planet, moon}, planet_id_fk, coordinates, slot, created_at, diameter_km, base_fields, fields_used, fields_total)`
- `moon_building (moon_id, building_type, level)`  
  `building_type ∈ {lunar_base, sensor_phalanx, jump_gate, shipyard, robotics, nanite, [missile_silo?], [terraformer?]}`
- `scan_log (id, moon_id, actor_id, ts, range_left, range_right, cost_deut, result_hash)`
- `jump_gate (moon_id, cooldown_until_ts)`
- `rip_attack (id, attacker_id, defender_id, moon_id, n_rips, p_destroy, p_lose, outcome, resolved_ts)`

---

## 9) API Endpoints (REST/GraphQL sketch)
- `POST /moons/{moonId}/scan` → {range, cost, results[]} (auth: owner or any player; cost charged to scanner).  
  Body: `{targetSystem:int}`; server validates SP range and charges `SCAN_COST_DEUT` from scanner’s **current planet** or **moon** fuel tank (design choice: use player global deuterium).
- `POST /moons/{moonId}/jump` → {cooldownUntil, shipsMoved[]}  
  Body: `{toMoonId, shipManifest:{shipType:int}}` (server: validate ownership both sides, JG ready, strip resources).
- `POST /rips/destroyMoon` → mission scheduling; response includes ETA.  
  Body: `{originPlanetId, targetCoords, rips:int, speed:float}`.
- `GET /moons/{moonId}` → public info (`diameter`, has SP/JG, owner alias) with privacy filters.

---

## 10) Client/UI Behaviors
- **Galaxy View**: Moon icon next to planet; tooltip shows `diameter` and `owner` if scouted previously.
- **Moon Overview**: Fields bar (`used/total`), building list, JG panel (destination picker with cooldown clock), SP panel (scan slider: `target system` within range, deut cost).
- **SP Results Modal**: Table of legs: `{mission, origin, dest, arrival_ts, return_ts?, speed_factor}`; export to clipboard; log keeps last N scans.
- **JG Modal**: Ship picklist, “empty cargo” warning, destination dropdown of owned moons with ready gates.
- **Destroy Moon Mission**: Dedicated tooltip that explains risk ranges given `n` RIPs and target `d`.

---

## 11) Tech & Requirements (Default)
- **Sensor Phalanx**: LB≥1, Computer Tech≥8.
- **Jump Gate**: LB≥1, Hyperspace Tech≥7, Shipyard≥1.
- **RIP (Death Star)**: Hyperspace Drive≥6, Graviton Tech≥1 (ship unlock), Shipyard≥12 (typical classic‑like gating).

---

## 12) Timings & Costs (Suggested Defaults)
- **Lunar Base**: base 20k/40k/0; x2 growth; build time uses classic formula with Robotics/Nanite.
- **Sensor Phalanx**: base 20k/40k/20k; x2 growth; no upkeep; scan cost 5k deut.
- **Jump Gate**: base 2M/4M/2M; x2 growth; cooldown 60 min.
- **Shipyard/Robotics/Nanite**: identical to planet but independent levels.

---

## 13) Edge Cases & Rules Clarifications
- **Multiple Moons per Planet**: **Not allowed**. One planet ↔ one moon max.
- **Colonization Slot**: Moon doesn’t occupy an extra colonization slot; it’s attached to its planet.
- **Deletion**: On account deletion, moons follow planet deletion.
- **SP on Vacation Mode**: Scanning allowed? **Default**: disabled to reduce passive intel.
- **Rebuild After Destruction**: If the moon is destroyed, the coordinate becomes eligible for a new moonchance in future battles.

---

## 14) Tuning Matrix (Playtest Knobs)
- `MOONCHANCE_UNIT`, `MOONCHANCE_CAP`  
- `DIAM_MIN/DIAM_MAX/BASE_DIAM/SIZE_PER_%/SIZE_JITTER`  
- `FIELDS_PER_LB`, `LB_MAX`  
- `SP range formula`, `SCAN_COST_DEUT`, `SCAN_COOLDOWN`, `DAILY_SCAN_CAP`  
- `JG_COOLDOWN` (and optional capacity)  
- RIP destruction model coefficients `A`, `B`; min flight times; launch cooldowns.

---

## 15) QA/Tests (Acceptance)
- **Acquisition**: Given debris 2,000,000 metal+crystal → `moonchance_pct=20` → roll success spawns moon with diameter within bounds.
- **Fields**: LB level 1 → total fields increases by `+2` net; building queue blocks when `used == total`.
- **SP Range**: Level 3 → scans ±8 systems.
- **SP Visibility**: Planet↔planet legs visible; moon↔anything legs invisible.
- **JG**: Jump between two owned moons with ready gates, ships arrive instantly, resources are not transferred, both gates enter cooldown.
- **RIP**: With fixed seed, simulate 10k attempts vs a 5,000 km moon with 5 RIPs → empirical p_destroy within ±2% of formula.

---

## 16) Migration/Config
- All knobs exposed in `server.balance.yml` under `moons:` namespace. Client reads readonly mirror (`/meta/balance`) for tooltips.

---

## 17) Roadmap & Variants
- **Variant**: Add limited moon resource “Outpost” (very low‑yield) for non‑classic servers.  
- **Variant**: Phalanx “Interdict” module (temporary no‑scan bubble) using rare items (event‑only).  
- **Variant**: JG alliance sharing (with roles/ACLs) on special universes.

