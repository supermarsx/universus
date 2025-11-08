import { Request } from 'express';

export interface User {
  id: number;
  username: string;
  email: string;
  dark_matter: number;
  created_at: Date;
  last_login?: Date;
  is_admin: boolean;
  is_banned: boolean;
  alliance_id?: number;
}

export interface Planet {
  id: number;
  user_id: number;
  name: string;
  galaxy: number;
  system: number;
  position: number;
  planet_type: string;
  temperature: number;
  diameter: number;
  
  // Resources
  metal: number;
  crystal: number;
  deuterium: number;
  energy: number;
  last_resource_update: Date;
  
  // Buildings
  [key: string]: any;
}

export interface Fleet {
  id: number;
  user_id: number;
  mission_type: string;
  origin_planet_id: number;
  target_galaxy: number;
  target_system: number;
  target_position: number;
  departure_time: Date;
  arrival_time: Date;
  return_time?: Date;
  ships: { [key: string]: number };
  cargo_metal: number;
  cargo_crystal: number;
  cargo_deuterium: number;
  status: string;
  acs_group_id?: number | null;
}

export interface Research {
  user_id: number;
  [key: string]: number;
}

export interface ConstructionQueue {
  id: number;
  planet_id: number;
  moon_id?: number | null;
  location_type: 'planet' | 'moon';
  building_type: string;
  level: number;
  start_time: Date;
  end_time: Date;
  metal_cost: number;
  crystal_cost: number;
  deuterium_cost: number;
}

export interface ShipyardQueue {
  id: number;
  planet_id: number;
  moon_id?: number | null;
  location_type: 'planet' | 'moon';
  unit_type: string;
  quantity: number;
  start_time: Date;
  end_time: Date;
  metal_cost: number;
  crystal_cost: number;
  deuterium_cost: number;
}

export interface ResearchQueue {
  id: number;
  user_id: number;
  planet_id: number;
  research_type: string;
  level: number;
  start_time: Date;
  end_time: Date;
  metal_cost: number;
  crystal_cost: number;
  deuterium_cost: number;
}

export interface AuthRequest extends Request {
  user?: User;
}
