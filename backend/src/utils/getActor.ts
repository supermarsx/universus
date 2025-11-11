import { AdminAuthRequest } from '../types/admin';

export interface Actor {
  id: number;
  username: string;
  level: number;
}

export function getActor(req: AdminAuthRequest): Actor | null {
  const id = req.admin?.id ?? req.user?.id;
  if (id === undefined || id === null) {
    return null;
  }

  const username = req.admin?.username ?? req.user?.username ?? 'unknown';
  const level = Number(req.adminLevel ?? req.admin?.level ?? 0);

  return { id, username, level };
}
