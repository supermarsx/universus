import { moonConfig } from '../config/moonConfig';

const ALLOWED_BUILDINGS = new Set([
  'lunar_base',
  'moon_robotics_factory',
  'moon_shipyard',
  'moon_nanite_factory',
  'sensor_phalanx',
  'jump_gate',
]);

export interface FieldAvailabilityParams {
  buildingType: string;
  nextLevel: number;
  totalFields?: number;
  usedFields?: number;
}

export interface FieldAdjustment {
  usedFields: number;
  totalFields: number;
}

export class MoonFieldService {
  static assertBuildingAllowed(buildingType: string): void {
    if (!ALLOWED_BUILDINGS.has(buildingType)) {
      throw new Error('This structure cannot be built on a moon');
    }
  }

  static assertFieldAvailability(params: FieldAvailabilityParams): void {
    const total = params.totalFields ?? 0;
    const used = params.usedFields ?? 0;
    const needsSlot = params.nextLevel === 1 && params.buildingType !== 'lunar_base';

    if (needsSlot && used >= total) {
      throw new Error('No available moon fields. Upgrade Lunar Base first.');
    }
  }

  static calculateFieldAdjustments(
    buildingType: string,
    resultLevel: number
  ): FieldAdjustment {
    if (!ALLOWED_BUILDINGS.has(buildingType)) {
      return { usedFields: 0, totalFields: 0 };
    }

    const adjustments: FieldAdjustment = { usedFields: 0, totalFields: 0 };

    if (resultLevel === 1) {
      adjustments.usedFields = 1;
    }

    if (buildingType === 'lunar_base' && resultLevel >= 1) {
      adjustments.totalFields = moonConfig.FIELDS_PER_LUNAR_BASE;
    }

    return adjustments;
  }
}
