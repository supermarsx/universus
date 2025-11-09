/**
 * @module backend/services/backgroundService
 *
 * Background selection helpers used by the template layer to provide
 * context-appropriate imagery (planet, galaxy, shipyard, etc.). This
 * service centralizes background selection logic and asset lists.
 */

/**
 * Background Service for Universus
 * Provides dynamic background selection for pages
 */

import {
  planetBackgrounds,
  spaceBackgrounds,
  environmentBackgrounds,
  getRandomPlanetBackground,
  getRandomSpaceBackground,
  getRandomEnvironmentBackground,
} from '../config/assetMappings';

export class BackgroundService {
  /**
   * Get background for overview page (planet surface).
   *
   * @returns Path to an asset image used for planet overview backgrounds.
   */
  getOverviewBackground(): string {
    return getRandomPlanetBackground();
  }

  /**
   * Get background image for the galaxy view.
   *
   * @returns Path to a space environment background image.
   */
  getGalaxyBackground(): string {
    return getRandomEnvironmentBackground();
  }

  /**
   * Get a random background appropriate for the fleet page (station/hangar).
   *
   * @returns Asset path string.
   */
  getFleetBackground(): string {
    const backgrounds = [
      '/assets/backgrounds/hangar-interior.png',
      '/assets/stations/repair-dock.png',
      '/assets/stations/military-outpost.png',
    ];
    return backgrounds[Math.floor(Math.random() * backgrounds.length)];
  }

  /**
   * Select a shipyard-related background image.
   *
   * @returns Asset path string selected from a shipyard palette.
   */
  getShipyardBackground(): string {
    const backgrounds = [
      '/assets/backgrounds/hangar-interior.png',
      '/assets/buildings/orbital-shipyard.png',
      '/assets/stations/shipyard-orbital.png',
    ];
    return backgrounds[Math.floor(Math.random() * backgrounds.length)];
  }

  /**
   * Get a buildings/planet-surface background.
   *
   * @returns Asset path string.
   */
  getBuildingsBackground(): string {
    return getRandomPlanetBackground();
  }

  /**
   * Choose a background suitable for the research page.
   *
   * @returns Asset path string.
   */
  getResearchBackground(): string {
    const backgrounds = [
      '/assets/buildings/research-lab-advanced.png',
      '/assets/buildings/quantum-computer.png',
      '/assets/stations/research-station.png',
    ];
    return backgrounds[Math.floor(Math.random() * backgrounds.length)];
  }

  /**
   * Background for messaging/command center pages.
   *
   * @returns Asset path string.
   */
  getMessagesBackground(): string {
    return '/assets/backgrounds/command-room.png';
  }

  /**
   * Get a space background appropriate for the leaderboard view.
   *
   * @returns Asset path string.
   */
  getLeaderboardBackground(): string {
    return getRandomSpaceBackground();
  }

  /**
   * Select a shop/trading hub background image.
   *
   * @returns Asset path string.
   */
  getShopBackground(): string {
    const backgrounds = [
      '/assets/buildings/trading-post.png',
      '/assets/stations/trade-hub.png',
      '/assets/buildings/spaceport.png',
    ];
    return backgrounds[Math.floor(Math.random() * backgrounds.length)];
  }

  /**
   * Background for the admin UI.
   *
   * @returns Asset path string.
   */
  getAdminBackground(): string {
    return '/assets/backgrounds/command-room.png';
  }

  /**
   * Return the full list of planet background image paths.
   *
   * @returns Array of asset path strings for planet backgrounds.
   */
  getAllPlanetBackgrounds(): string[] {
    return planetBackgrounds.map(name => `/assets/planets/${name}.png`);
  }

  /**
   * Return the full list of space/background image paths.
   *
   * @returns Array of asset path strings for space backgrounds.
   */
  getAllSpaceBackgrounds(): string[] {
    return spaceBackgrounds.map(name => `/assets/backgrounds/${name}.png`);
  }
}

// Singleton instance
export const backgroundService = new BackgroundService();
