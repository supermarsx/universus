import { MoonFieldService } from '../../src/services/moonFieldService';

describe('MoonFieldService', () => {
  test('disallows non-moon buildings', () => {
    expect(() => MoonFieldService.assertBuildingAllowed('metal_mine')).toThrow(
      'This structure cannot be built on a moon'
    );
  });

  test('allows lunar structures', () => {
    expect(() => MoonFieldService.assertBuildingAllowed('lunar_base')).not.toThrow();
  });

  test('blocks builds when fields are full', () => {
    expect(() =>
      MoonFieldService.assertFieldAvailability({
        buildingType: 'moon_robotics_factory',
        nextLevel: 1,
        totalFields: 1,
        usedFields: 1,
      })
    ).toThrow('No available moon fields. Upgrade Lunar Base first.');
  });

  test('lunar base can be queued even at cap', () => {
    expect(() =>
      MoonFieldService.assertFieldAvailability({
        buildingType: 'lunar_base',
        nextLevel: 1,
        totalFields: 1,
        usedFields: 1,
      })
    ).not.toThrow();
  });

  test('field adjustments increment on first-time builds', () => {
    const adjustment = MoonFieldService.calculateFieldAdjustments('moon_shipyard', 1);
    expect(adjustment).toEqual({ usedFields: 1, totalFields: 0 });
  });

  test('lunar base adds fields each level', () => {
    const first = MoonFieldService.calculateFieldAdjustments('lunar_base', 1);
    const second = MoonFieldService.calculateFieldAdjustments('lunar_base', 2);

    expect(first.usedFields).toBe(1);
    expect(first.totalFields).toBeGreaterThan(0);
    expect(second.usedFields).toBe(0);
    expect(second.totalFields).toBe(first.totalFields);
  });
});
