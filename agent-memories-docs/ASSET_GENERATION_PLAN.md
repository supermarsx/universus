# UNIVERSUS - Visual Asset Generation Plan

## Art Style Direction

### Core Visual Identity
- **Style:** Modern, clean sci-fi aesthetic with cinematic quality
- **Color Palette:** Deep space blues/purples with bright accent colors
- **Atmosphere:** Mysterious, vast, epic space environment
- **Quality:** High-resolution, professional game assets
- **Consistency:** Unified art style across all asset categories

### Color Scheme
- Deep Space Blue: #0A1929
- Nebula Purple: #6B2B84
- Stellar Gold: #FFB923
- Cosmic Cyan: #00D9FF
- Star White: #E8F1F5
- Supernova Red: #FF3366
- Alien Green: #00FF88

## Asset Generation Categories

### 1. Planet Backgrounds (50 assets)

#### Terrestrial Planets (10)
1. Earth-like planet with blue oceans and green continents
2. Desert planet with red-orange sand dunes and canyons
3. Rocky barren planet with gray-brown surface and craters
4. Tropical planet with lush green vegetation and water
5. Arctic planet with ice sheets and frozen landscape
6. Volcanic planet with lava rivers and dark rock
7. Ocean planet completely covered in deep blue water
8. Mountainous planet with tall peaks and valleys
9. Canyon planet with deep gorges and plateaus
10. Jungle planet with dense vegetation and mist

#### Gas Giants (8)
11. Jupiter-like giant with swirling bands and great red spot
12. Saturn-like planet with prominent ring system
13. Blue-green gas giant with atmospheric storms
14. Dark purple gas giant with glowing auroras
15. Golden-orange gas giant with atmospheric turbulence
16. Ice giant with pale blue-cyan coloration
17. Stormy gas giant with visible lightning
18. Ringed gas giant with multiple colored bands

#### Ice Worlds (6)
19. Frozen planet covered in white ice and snow
20. Blue ice planet with crystalline structures
21. Glacial planet with ice cliffs and frozen seas
22. Arctic moon with methane ice surface
23. Crystalline ice world with reflective surface
24. Frost planet with geometric ice patterns

#### Desert Planets (6)
25. Sandy desert world with dunes and dust storms
26. Red desert planet like Mars with iron oxide
27. Salt flat planet with white crystalline surface
28. Rocky desert with scattered boulders
29. Canyon desert with layered rock formations
30. Windswept desert with rippling sand patterns

#### Lava Planets (5)
31. Molten lava world with glowing cracks
32. Volcanic planet with active eruptions
33. Fire planet with red-orange surface
34. Magma planet with flowing lava rivers
35. Hell-like world with intense heat glow

#### Metal Planets (5)
36. Metallic planet with silver-gray surface
37. Iron-rich world with rust coloration
38. Chrome-like reflective planet
39. Copper-colored metallic world
40. Alloy planet with industrial appearance

#### Artificial Worlds (5)
41. Mega-city planet covered in structures
42. Dyson sphere partial construction
43. Terraformed planet with geometric sections
44. Space station ring world
45. Constructed planet with visible panels

#### Exotic Planets (5)
46. Crystal planet with glowing formations
47. Bioluminescent planet with glowing life
48. Dark matter planet with void appearance
49. Plasma planet with energy storms
50. Quantum anomaly planet with reality distortions

### 2. Spacecraft Designs (30 assets)

#### Light Fighters (3)
51. Sleek interceptor with pointed nose and wings
52. Agile starfighter with twin engines
53. Scout fighter with sensor arrays

#### Heavy Fighters (3)
54. Armored combat fighter with weapon pods
55. Assault fighter with heavy plating
56. Strike fighter with missile hardpoints

#### Cruisers (4)
57. Medium cruiser with balanced design
58. Fast attack cruiser with streamlined hull
59. Heavy cruiser with multiple gun turrets
60. Command cruiser with communication arrays

#### Battleships (4)
61. Massive battleship with heavy armor
62. Dreadnought-class with main cannons
63. Siege battleship with bombardment weapons
64. Fleet battleship with support capabilities

#### Dreadnoughts (3)
65. Titan-class super capital ship
66. Mega-dreadnought with multiple decks
67. Fortress-class mobile starbase

#### Cargo Ships (3)
68. Industrial freighter with cargo pods
69. Bulk transport with large holds
70. Container ship with modular design

#### Colony Ships (2)
71. Colony vessel with habitat modules
72. Arkship with terraforming equipment

#### Recyclers (2)
73. Salvage ship with collection arrays
74. Resource reclaimer with processing units

#### Espionage Probes (2)
75. Stealth probe with cloaking device
76. Scanner probe with sensor suite

#### Bombers (2)
77. Heavy bomber with bomb bay
78. Torpedo bomber with missile pods

#### Destroyers (2)
79. Anti-ship destroyer with railguns
80. Fleet destroyer with point defense

### 3. Planetary Structures (40 assets)

#### Resource Production (16)
81-84. Metal mines (4 variations: small, medium, large, mega)
85-88. Crystal mines (4 variations: basic, advanced, deep, ultra)
89-92. Deuterium synthesizers (4 variations: compact, standard, industrial, fusion)
93-96. Solar power plants (4 variations: basic, solar array, concentrated, orbital)

#### Energy Production (4)
97-100. Fusion reactors (4 variations: small, medium, large, mega-reactor)

#### Research Facilities (4)
101-104. Research labs (4 variations: basic, advanced, quantum, super-collider)

#### Military Installations (8)
105-108. Shipyards (4 variations: basic, advanced, orbital, mega-dock)
109-112. Defense systems (4 variations: turrets, missile batteries, shields, fortress)

#### Storage (4)
113-116. Storage facilities (4 variations: warehouse, silos, vaults, mega-storage)

#### Special Buildings (4)
117-120. Terraforming structures (4 variations: atmosphere processor, water generator, biosphere, climate control)

### 4. Space Stations (20 assets)

#### Trading Posts (4)
121-124. Trading stations with docking bays and market areas

#### Military Stations (4)
125-128. Defense platforms and military outposts

#### Research Stations (4)
129-132. Science stations with laboratories

#### Mining Stations (4)
133-136. Asteroid mining platforms

#### Defense Platforms (4)
137-140. Orbital defense systems with weapons

### 5. Environmental Assets (30 assets)

#### Asteroid Fields (6)
141-146. Various asteroid field configurations

#### Nebulae (6)
147-152. Colorful space nebulae backgrounds

#### Space Debris (6)
153-158. Wreckage and floating debris

#### Cosmic Phenomena (6)
159-164. Black holes, pulsars, supernovae effects

#### Star Backgrounds (6)
165-170. Different star field backgrounds

### 6. UI Elements (30 assets)

#### Buttons (6)
171-176. Various button styles for different actions

#### Icons (12)
177-188. Resource icons, action icons, status icons

#### Progress Bars (4)
189-192. Different progress bar styles

#### Panel Backgrounds (4)
193-196. UI panel and window backgrounds

#### Tooltips (4)
197-200. Tooltip frame designs

## Generation Parameters

### Technical Specifications
- **Format:** PNG with transparency where applicable
- **Resolution:** Minimum 1024x1024 for planets, 512x512 for UI elements
- **Style Consistency:** All assets share the same art direction
- **Color Grading:** Consistent color palette across all assets

### Prompt Template Structure
```
[Subject] in [Art Style], [Details], [Atmosphere], [Color Palette], 
[Quality Terms], game asset, clean background
```

## Asset Organization
```
frontend/assets/
├── planets/
│   ├── terrestrial/
│   ├── gas-giants/
│   ├── ice-worlds/
│   ├── desert/
│   ├── lava/
│   ├── metal/
│   ├── artificial/
│   └── exotic/
├── ships/
│   ├── fighters/
│   ├── cruisers/
│   ├── battleships/
│   └── support/
├── buildings/
│   ├── production/
│   ├── energy/
│   ├── research/
│   └── military/
├── stations/
├── environments/
│   ├── asteroids/
│   ├── nebulae/
│   └── phenomena/
└── ui/
    ├── buttons/
    ├── icons/
    ├── bars/
    └── panels/
```

## Implementation Status
- [ ] Planets: 0/50
- [ ] Spacecraft: 0/30
- [ ] Buildings: 0/40
- [ ] Stations: 0/20
- [ ] Environments: 0/30
- [ ] UI Elements: 0/30

**Total Progress: 0/200 assets**
