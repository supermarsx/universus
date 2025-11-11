import { AuthRequest } from "../types";

export function getUserId(req: AuthRequest): number | null {
    if (req.user && typeof (req.user as any).id === 'number') return (req.user as any).id;
    const anyUser = (req as any).user;
    if (anyUser && typeof anyUser.userId === 'number') return anyUser.userId;
    return null;
}
