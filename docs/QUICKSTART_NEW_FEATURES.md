# Quick Start Guide - New Features

## Overview

This guide covers the newly implemented Jump Gate and Moon Destruction features for moons.

## New Features Implemented

### 1. Jump Gate Usage

- **Description:** Allows instant transfer of fleets between two moons with Jump Gates, subject to a cooldown (1 hour).
- **API Endpoint:**
  ```bash
  POST /api/moons/:moonId/jump-gate
  Authorization: Bearer <token>
  {
    "toMoonId": 2,
    "fleetIds": [101, 102]
  }
  ```
  - Returns `{ success: true }` on success, or `{ success: false, error: "..." }` on error.
- **Cooldown:** 1 hour per moon. Only moons with Jump Gates can be used.
- **UI:** Shows eligible fleets, destination moons, and cooldown timer.

### 2. Moon Destruction

- **Description:** Allows players to attempt to destroy a moon using Deathstar ships. Success chance depends on moon size and number of Deathstars.
- **API Endpoint:**
  ```bash
  POST /api/moons/:moonId/destroy
  Authorization: Bearer <token>
  {
    "numDeathstars": 5
  }
  ```
  - Returns `{ success: true, data: { destroyed, deathstarsLost, chance, lossChance } }` on success, or `{ success: false, error: "..." }` on error.
- **Mechanics:**
  - If successful: moon and all structures are destroyed, fleets in transit remain, planet remains.
  - If failed: chance to lose Deathstars.
- **UI:** "Destroy Moon" mission, confirmation, and result feedback.

---

## Testing & Validation

- Unit and integration tests cover cooldown, fleet transfer, destruction chance, and error cases.
- See `backend/tests/unit/jumpGateService.test.ts`, `backend/tests/unit/destroyMoonService.test.ts`, and `backend/tests/integration/moonsApi.test.ts`.

---

For more details, see the main README and API documentation.
