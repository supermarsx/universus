import { Response } from 'express';
import { backgroundService } from './backgroundService';
import {
  getShipAsset,
  getBuildingAsset,
  getResourceIcon,
} from '../config/assetMappings';

export interface TemplateContext {
  title?: string;
  description?: string;
  bodyClass?: string;
  currentPage?: string;
  pageBackground?: string;
  user?: any;
  resources?: {
    metal: number;
    crystal: number;
    deuterium: number;
    energy: number;
  };
  planet?: any;
  production?: any;
  queue?: any[];
  stats?: any;
  events?: any[];
  buildings?: any[];
  technologies?: any[];
  [key: string]: any;
}

/**
 * Template rendering service
 */
export class TemplateService {
  /**
   * Render a template with context data
   */
  static render(res: Response, template: string, context: TemplateContext = {}): void {
    const defaultContext: TemplateContext = {
      brandName: 'Universus',
      version: process.env.APP_VERSION || '1.0.0',
      currentYear: new Date().getFullYear(),
      // Asset helper functions
      getShipAsset,
      getBuildingAsset,
      getResourceIcon,
    };

    const mergedContext = { ...defaultContext, ...context };
    res.render(template, mergedContext);
  }

  /**
   * Render index/login page
   */
  static renderIndex(res: Response, context: TemplateContext = {}): void {
    this.render(res, 'pages/index.njk', context);
  }

  /**
   * Render overview page
   */
  static renderOverview(res: Response, context: TemplateContext = {}): void {
    this.render(res, 'pages/overview.njk', {
      ...context,
      currentPage: 'overview',
      pageBackground: backgroundService.getOverviewBackground(),
    });
  }

  /**
   * Render buildings page
   */
  static renderBuildings(res: Response, context: TemplateContext = {}): void {
    this.render(res, 'pages/buildings.njk', {
      ...context,
      currentPage: 'buildings',
      pageBackground: backgroundService.getBuildingsBackground(),
    });
  }

  /**
   * Render research page
   */
  static renderResearch(res: Response, context: TemplateContext = {}): void {
    this.render(res, 'pages/research.njk', {
      ...context,
      currentPage: 'research',
      pageBackground: backgroundService.getResearchBackground(),
    });
  }

  /**
   * Render shipyard page
   */
  static renderShipyard(res: Response, context: TemplateContext = {}): void {
    this.render(res, 'pages/shipyard.njk', {
      ...context,
      currentPage: 'shipyard',
      pageBackground: backgroundService.getShipyardBackground(),
    });
  }

  /**
   * Render fleet page
   */
  static renderFleet(res: Response, context: TemplateContext = {}): void {
    this.render(res, 'pages/fleet.njk', {
      ...context,
      currentPage: 'fleet',
      pageBackground: backgroundService.getFleetBackground(),
    });
  }

  /**
   * Render galaxy page
   */
  static renderGalaxy(res: Response, context: TemplateContext = {}): void {
    this.render(res, 'pages/galaxy.njk', {
      ...context,
      currentPage: 'galaxy',
      pageBackground: backgroundService.getGalaxyBackground(),
    });
  }

  /**
   * Render leaderboard page
   */
  static renderLeaderboard(res: Response, context: TemplateContext = {}): void {
    this.render(res, 'pages/leaderboard.njk', {
      ...context,
      currentPage: 'leaderboard',
      pageBackground: backgroundService.getLeaderboardBackground(),
    });
  }

  /**
   * Render messages page
   */
  static renderMessages(res: Response, context: TemplateContext = {}): void {
    this.render(res, 'pages/messages.njk', {
      ...context,
      currentPage: 'messages',
      pageBackground: backgroundService.getMessagesBackground(),
    });
  }

  /**
   * Render shop page
   */
  static renderShop(res: Response, context: TemplateContext = {}): void {
    this.render(res, 'pages/shop.njk', {
      ...context,
      currentPage: 'shop',
      pageBackground: backgroundService.getShopBackground(),
    });
  }

  /**
   * Render admin page
   */
  static renderAdmin(res: Response, context: TemplateContext = {}): void {
    this.render(res, 'pages/admin.njk', {
      ...context,
      currentPage: 'admin',
      pageBackground: backgroundService.getAdminBackground(),
    });
  }

  /**
   * Render bot management page
   */
  static renderBotManagement(res: Response, context: TemplateContext = {}): void {
    this.render(res, 'pages/admin/bots.njk', {
      ...context,
      currentPage: 'admin',
    });
  }
}
