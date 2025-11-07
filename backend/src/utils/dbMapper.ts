// Database Query Result Mapping Utility
// Converts database snake_case results to TypeScript camelCase interfaces

/**
 * Converts snake_case string to camelCase
 */
function toCamelCase(str: string): string {
  return str.replace(/_([a-z])/g, (match, letter) => letter.toUpperCase());
}

/**
 * Recursively converts all keys in an object from snake_case to camelCase
 */
function toCamelCaseKeys(obj: any): any {
  if (Array.isArray(obj)) {
    return obj.map(toCamelCaseKeys);
  } else if (obj !== null && typeof obj === 'object') {
    const camelObj: any = {};
    for (const key in obj) {
      if (obj.hasOwnProperty(key)) {
        const camelKey = toCamelCase(key);
        camelObj[camelKey] = toCamelCaseKeys(obj[key]);
      }
    }
    return camelObj;
  }
  return obj;
}

/**
 * Maps a single database row to camelCase
 */
export function mapDbRow<T = any>(row: any): T {
  if (!row) return row;
  return toCamelCaseKeys(row) as T;
}

/**
 * Maps an array of database rows to camelCase
 */
export function mapDbRows<T = any>(rows: any[]): T[] {
  if (!Array.isArray(rows)) return [];
  return rows.map(mapDbRow) as T[];
}

/**
 * Maps a single database row result from query
 */
export function mapQueryResult<T = any>(result: { rows: any[] }): T[] {
  if (!result || !result.rows) return [];
  return mapDbRows<T>(result.rows);
}

/**
 * Maps the first row of a query result
 */
export function mapQueryResultRow<T = any>(result: { rows: any[] }): T | null {
  if (!result || !result.rows || result.rows.length === 0) return null;
  return mapDbRow<T>(result.rows[0]);
}

/**
 * Converts camelCase object keys back to snake_case (for database inserts/updates)
 */
function toSnakeCaseKeys(obj: any): any {
  if (Array.isArray(obj)) {
    return obj.map(toSnakeCaseKeys);
  } else if (obj !== null && typeof obj === 'object') {
    const snakeObj: any = {};
    for (const key in obj) {
      if (obj.hasOwnProperty(key)) {
        const snakeKey = key.replace(/[A-Z]/g, letter => `_${letter.toLowerCase()}`);
        snakeObj[snakeKey] = toSnakeCaseKeys(obj[key]);
      }
    }
    return snakeObj;
  }
  return obj;
}

/**
 * Converts camelCase object to snake_case for database operations
 */
export function toDatabaseFormat(obj: any): any {
  return toSnakeCaseKeys(obj);
}

// =============================================================================
// SPECIFIC TYPE MAPPERS
// =============================================================================

/**
 * Maps a user database row to User interface
 */
export function mapUserRow(row: any) {
  return mapDbRow(row);
}

/**
 * Maps a planet database row to Planet interface  
 */
export function mapPlanetRow(row: any) {
  return mapDbRow(row);
}

/**
 * Maps a fleet database row to Fleet interface
 */
export function mapFleetRow(row: any) {
  return mapDbRow(row);
}

/**
 * Maps a research database row to Research interface
 */
export function mapResearchRow(row: any) {
  return mapDbRow(row);
}

/**
 * Maps an alliance database row to Alliance interface
 */
export function mapAllianceRow(row: any) {
  return mapDbRow(row);
}

/**
 * Maps an alliance member database row to AllianceMember interface
 */
export function mapAllianceMemberRow(row: any) {
  return mapDbRow(row);
}

/**
 * Maps an alliance application database row to AllianceApplication interface
 */
export function mapAllianceApplicationRow(row: any) {
  return mapDbRow(row);
}

/**
 * Maps a debris database row to CombatDebris interface
 */
export function mapCombatDebrisRow(row: any) {
  return mapDbRow(row);
}

/**
 * Maps a component database row to ShipComponent interface
 */
export function mapShipComponentRow(row: any) {
  return mapDbRow(row);
}

/**
 * Maps a component inventory database row to PlayerComponentInventory interface
 */
export function mapPlayerComponentInventoryRow(row: any) {
  return mapDbRow(row);
}

/**
 * Maps a salvage operation database row to DebrisSalvageOperation interface
 */
export function mapDebrisSalvageRow(row: any) {
  return mapDbRow(row);
}

/**
 * Maps a server discovery database row to ShardServer interface
 */
export function mapShardServerRow(row: any) {
  return mapDbRow(row);
}

/**
 * Maps a player placement database row to PlayerPlacement interface
 */
export function mapPlayerPlacementRow(row: any) {
  return mapDbRow(row);
}

/**
 * Maps a message database row to Message interface
 */
export function mapMessageRow(row: any) {
  return mapDbRow(row);
}

/**
 * Maps a notification database row to Notification interface
 */
export function mapNotificationRow(row: any) {
  return mapDbRow(row);
}

/**
 * Maps a session database row to Session interface
 */
export function mapSessionRow(row: any) {
  return mapDbRow(row);
}

/**
 * Maps a theme database row to Theme interface
 */
export function mapThemeRow(row: any) {
  return mapDbRow(row);
}

/**
 * Maps a configuration database row to Configuration interface
 */
export function mapConfigurationRow(row: any) {
  return mapDbRow(row);
}

/**
 * Maps a combat report database row to CombatReport interface
 */
export function mapCombatReportRow(row: any) {
  return mapDbRow(row);
}

/**
 * Maps a debris field info database row to DebrisFieldInfo interface
 */
export function mapDebrisFieldInfoRow(row: any) {
  return mapDbRow(row);
}