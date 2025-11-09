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
   * Get background for overview page (planet surface)
   */
  getOverviewBackground(): string {
    return getRandomPlanetBackground();
  }

  /**
   * Get background for galaxy page (deep space)
   */
  getGalaxyBackground(): string {
    return getRandomEnvironmentBackground();
  }

  /**
   * Get background for fleet page (space station/hangar)
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
   * Get background for shipyard page (construction facility)
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
   * Get background for buildings page (planet surface)
   */
  getBuildingsBackground(): string {
    return getRandomPlanetBackground();
  }

  /**
   * Get background for research page (research facility)
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
   * Get background for messages page (command center)
   */
  getMessagesBackground(): string {
    return '/assets/backgrounds/command-room.png';
  }

  /**
   * Get background for leaderboard page (space backdrop)
   */
  getLeaderboardBackground(): string {
    return getRandomSpaceBackground();
  }

  /**
   * Get background for shop page (trading hub)
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
   * Get background for admin page (command center)
   */
  getAdminBackground(): string {
    return '/assets/backgrounds/command-room.png';
  }

  /**
   * Get all planet backgrounds
   */
  getAllPlanetBackgrounds(): string[] {
    return planetBackgrounds.map(name => `/assets/planets/${name}.png`);
  }

  /**
   * Get all space backgrounds
   */
  getAllSpaceBackgrounds(): string[] {
    return spaceBackgrounds.map(name => `/assets/backgrounds/${name}.png`);
  }
}

// Singleton instance
export const backgroundService = new BackgroundService();
