import 'jest';

jest.mock('../src/api');

import { FleetManager } from '../src/fleet';
import api from '../src/api';

// Rely on Jest's jsdom environment instead of importing JSDOM
declare const global: any;

// Ensure the global api used in runtime code points to the mocked api
beforeEach(() => {
  global.api = api;
  // Minimal DOM required by FleetManager
  // Use jsdom globals provided by Jest
  document.body.innerHTML = `
    <div id="fleetSelection"></div>
    <div id="selectedShipsSummary"></div>
    <div id="availableCargo"></div>
    <div id="fuelEstimate"></div>
    <div id="targetGalaxy"></div>
    <div id="targetSystem"></div>
    <div id="targetPosition"></div>
    <div id="acsGroupList"></div>
    <div id="acsPanel"></div>
    <div id="selectedAcsBadge"></div>
    <div id="selectedAcsLabel"></div>
    <div id="step1" class="fleet-step"></div>
    <div id="step2" class="fleet-step hidden"></div>
    <div id="step3" class="fleet-step hidden"></div>
    <div id="fleetInventory"></div>
    <div id="activeMissions"></div>
    <div id="combatReports"></div>
    <div id="missionLogEntries"></div>
  `;
  (api.get as jest.Mock).mockClear();
  (api.post as jest.Mock).mockClear();
});

test('renders empty fleet selection when planet has no ships', () => {
  const fm = new FleetManager() as any;
  fm.updatePlanet({ planet: { id: 1, galaxy: 1, system: 1, position: 1 } });
  expect(document.getElementById('fleetSelection').innerHTML).toMatch(/No ships available/);
});

test('selecting ships updates summary and cargo/fuel', () => {
  const planet = { id: 1, galaxy: 1, system: 1, position: 1, small_cargo: 10 };
  const fm = new FleetManager() as any;
  fm.updatePlanet({ planet });

  const input = document.querySelector('#fleetSelection input[data-ship]') as HTMLInputElement;
  expect(input).toBeTruthy();
  input.value = '2';
  input.dispatchEvent(new Event('input'));

  expect(document.getElementById('selectedShipsSummary').textContent).toContain('Small Cargo: 2');
  expect(document.getElementById('availableCargo').textContent).not.toBe('0');
});

test('dispatchFleet posts payload and resets form', async () => {
  (api.post as jest.Mock).mockResolvedValue({ success: true });
  const planet = { id: 1, galaxy: 1, system: 1, position: 1, small_cargo: 5 };
  const fm = new FleetManager() as any;
  fm.updatePlanet({ planet });

  // select 1 ship
  const input = document.querySelector('#fleetSelection input[data-ship]') as HTMLInputElement;
  input.value = '1';
  input.dispatchEvent(new Event('input'));

  // set mission
  fm.selectedMission = 'transport';

  await fm.dispatchFleet();
  expect(api.post).toHaveBeenCalledWith('/fleet/dispatch', expect.any(Object));
  expect(document.getElementById('selectedShipsSummary').textContent).toMatch(/No ships selected/);
});
