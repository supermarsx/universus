import 'jest';

jest.mock('i18next', () => require('../__mocks__/i18next'));
import i18next from 'i18next';

describe('i18n coverage and fallbacks', () => {
  test('all ship and defense keys return localized names', () => {
    const ships = [
      'small_cargo','large_cargo','light_fighter','heavy_fighter','cruiser','battleship','colony_ship','recycler','espionage_probe','bomber','destroyer','deathstar'
    ];
    ships.forEach((s) => {
      expect(i18next.t(`shipyard.ships.${s}.name`)).not.toBe(`shipyard.ships.${s}.name`);
      expect(i18next.t(`shipyard.ships.${s}.description`)).not.toBe(`shipyard.ships.${s}.description`);
    });

    const defs = ['rocket_launcher','light_laser','heavy_laser','gauss_cannon','ion_cannon','plasma_turret'];
    defs.forEach((d) => {
      expect(i18next.t(`shipyard.defense.${d}.name`)).not.toBe(`shipyard.defense.${d}.name`);
      expect(i18next.t(`shipyard.defense.${d}.description`)).not.toBe(`shipyard.defense.${d}.description`);
    });
  });

  test('missing keys fall back to key string', () => {
    const missing = i18next.t('shipyard.ships.nonexistent_unit.name');
    expect(missing).toBe('shipyard.ships.nonexistent_unit.name');
  });
});
