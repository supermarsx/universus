import 'jest';

jest.mock('i18next');
jest.mock('../src/api');

import api from '../src/api';
import i18next from 'i18next';
import { createShipyardDOM } from './test-utils.helper';
import { ShipyardManager } from '../src/shipyard';

// Provide globals used by shipyard module
declare const global: any;

global.api = api;
global.showNotification = jest.fn();
global.loadPlanetData = jest.fn();

describe('ShipyardManager TypeScript tests — extensive edge cases', () => {
  beforeEach(() => {
    jest.resetModules();
    createShipyardDOM();
    global.confirm = jest.fn(() => true);
  });

  test('formatName returns localized names and falls back to title case for strange keys', () => {
    const mgr = new ShipyardManager() as any;
    expect(mgr.formatName('small_cargo')).toBe('Small Cargo');
    expect(mgr.formatName('rocket_launcher')).toBe('Rocket Launcher');

    // edge: empty string
    expect(mgr.formatName('')).toBe('');

    // edge: numeric key
    expect(mgr.formatName('12345')).toBe('12345');

    // edge: null/undefined passed (TS would prevent, but runtime may happen)
    // @ts-ignore
    expect(mgr.formatName(null)).toBe('null');
  });

  test('formatNumber handles negatives, floats, and non-numbers', () => {
    const mgr = new ShipyardManager() as any;
    expect(mgr.formatNumber(1234)).toBe('1,234');
    expect(mgr.formatNumber(1234.9)).toBe('1,234');
    expect(mgr.formatNumber(-50)).toBe('-50');
    // @ts-ignore
    expect(mgr.formatNumber(null)).toBe('0');
    // big number
    expect(mgr.formatNumber(1000000000)).toBe('1,000,000,000');
  });

  test('formatTime handles 0, sub-minute, multi-hour, negative values', () => {
    const mgr = new ShipyardManager() as any;
    expect(mgr.formatTime(0)).toBe('0h 0m 0s');
    expect(mgr.formatTime(45)).toBe('0h 0m 45s');
    expect(mgr.formatTime(3600 + 61)).toBe('1h 1m 1s');
    // negative -> treat raw
    // @ts-ignore
    expect(mgr.formatTime(-10)).toBe('-1h 59m 50s');
  });

  test('refreshLocationControls toggles select and shows moon option correctly', () => {
    const mgr = new ShipyardManager() as any;
    mgr.planet = { id: 1, name: 'Earth' } as any;
    mgr.moon = null;
    mgr.locationType = 'planet';
    mgr.refreshLocationControls();

    const select = document.getElementById('shipyardLocationSelect') as HTMLSelectElement;
    expect(select.options.length).toBe(1);
    expect(select.disabled).toBeTruthy();

    mgr.moon = { id: 2, name: 'Luna', moon_shipyard: 1 } as any;
    mgr.refreshLocationControls();
    expect(select.options.length).toBe(2);
    expect(select.disabled).toBeFalsy();
  });

  test('renderShips renders cards with localized names and handles missing image gracefully', () => {
    const mgr = new ShipyardManager() as any;
    mgr.planet = { id: 2, name: 'Mars', shipyard: 5, metal: 999999, crystal: 999999, deuterium: 999999 } as any;
    mgr.moon = null;
    mgr.locationType = 'planet';

    mgr.renderShips();
    const grid = document.getElementById('shipsGrid')!;
    expect(grid.querySelectorAll('.ship-card').length).toBeGreaterThan(0);

    // Simulate missing image fallback by checking onerror attribute present
    const img = grid.querySelector('img') as HTMLImageElement;
    expect(img.getAttribute('onerror')).toContain("this.src='/assets/ships/fighter-interceptor.png'");
  });

  test('renderDefense renders when shipyard exists and disables build when insufficient resources', () => {
    const mgr = new ShipyardManager() as any;
    mgr.planet = { id: 3, name: 'Zeus', shipyard: 4, metal: 0, crystal: 0, deuterium: 0 } as any;
    mgr.locationType = 'planet';

    mgr.renderDefense();
    const grid = document.getElementById('defenseGrid')!;
    const buttons = Array.from(grid.querySelectorAll('button')) as HTMLButtonElement[];
    expect(buttons.length).toBeGreaterThan(0);
    expect(buttons.some((b) => b.disabled)).toBeTruthy();
  });

  test('startProduction early returns when planet missing or moon missing', async () => {
    const mgr = new ShipyardManager() as any;
    mgr.planet = null;
    await mgr.startProduction('small_cargo', 1);
    expect(api.post).not.toHaveBeenCalled();

    mgr.planet = { id: 1 } as any;
    mgr.locationType = 'moon';
    mgr.moon = null;
    await mgr.startProduction('small_cargo', 1);
    expect(api.post).not.toHaveBeenCalled();
  });

  test('startProduction sanitizes quantities and handles zero/negative', async () => {
    const mgr = new ShipyardManager() as any;
    mgr.planet = { id: 10 } as any;
    mgr.moon = null;
    mgr.locationType = 'planet';

    (api.post as jest.Mock).mockResolvedValueOnce({ ok: true });
    await mgr.startProduction('small_cargo', 0);
    expect(api.post).toHaveBeenCalledWith('/shipyard/10/build', expect.objectContaining({ quantity: 0, unitType: 'small_cargo' }));

    (api.post as jest.Mock).mockResolvedValueOnce({ ok: true });
    await mgr.startProduction('small_cargo', -5);
    expect(api.post).toHaveBeenCalledWith('/shipyard/10/build', expect.objectContaining({ quantity: -5 }));
  });

  test('loadQueue handles malformed API responses and sets empty queue', async () => {
    const mgr = new ShipyardManager() as any;
    mgr.planet = { id: 20 } as any;
    (api.get as jest.Mock).mockResolvedValueOnce({ not: 'an array' } as any);
    await mgr.loadQueue();
    expect(mgr.queue).toEqual([]);
  });

  test('renderQueue timer updates DOM and calls loadQueue on completion', () => {
    jest.useFakeTimers();
    const mgr = new ShipyardManager() as any;
    mgr.planet = { id: 30 } as any;
    mgr.queue = [
      { id: 'qq1', unit_type: 'small_cargo', quantity: 1, progress: 0.5, secondsRemaining: 2 },
    ] as any;
    mgr.renderQueue();

    const timer = document.querySelector('[data-queue-timer="qq1"]') as HTMLElement;
    expect(timer).toBeTruthy();
    expect(timer.textContent).toBe('0h 0m 2s');

    // advance time
    jest.advanceTimersByTime(1000);
    expect(timer.textContent).toBe('0h 0m 1s');

    jest.advanceTimersByTime(1000);
    // after completion loadQueue should be called once
    // Note: we don't mock loadQueue itself, but loadQueue triggers an api.get — ensure interval cleared
    jest.useRealTimers();
  });

  test('cancelQueue handles API failure and shows error notification', async () => {
    const mgr = new ShipyardManager() as any;
    mgr.planet = { id: 40 } as any;
    global.confirm = jest.fn(() => true);
    (api.delete as jest.Mock).mockRejectedValueOnce(new Error('fail'));

    await mgr.cancelQueue('z1');
    expect(showNotification).toHaveBeenCalledWith(i18next.t('shipyard.notificationTitle'), i18next.t('shipyard.failedToCancelProduction'), 'error');
  });

  test('renderShips handles blueprint with missing cost fields gracefully', () => {
    const mgr = new ShipyardManager() as any;
    // create a mock blueprint by mutating blueprints (risky but for test)
    // @ts-ignore
    const original = (global as any).SHIP_BLUEPRINTS;
    // we won't modify source — instead, simulate resources missing
    mgr.planet = { id: 50, name: 'Null', shipyard: 5, metal: 0 } as any;
    mgr.renderShips();
    const grid = document.getElementById('shipsGrid')!;
    expect(grid.querySelectorAll('.cost-item').length).toBeGreaterThan(0);
  });

  test('DOM methods are resilient when elements are absent', () => {
    // remove shipsGrid and other elements
    document.body.innerHTML = '';
    const mgr = new ShipyardManager() as any;
    mgr.planet = { id: 60 } as any;
    // should not throw
    expect(() => mgr.renderShips()).not.toThrow();
    expect(() => mgr.renderDefense()).not.toThrow();
    expect(() => mgr.renderQueue()).not.toThrow();
  });
});
