import bcrypt from 'bcryptjs';
import jwt, { SignOptions } from 'jsonwebtoken';
import { pool } from '../config/database';
import { User } from '../types';
import { PoolClient } from 'pg';

export class AuthService {
  static async register(
    username: string,
    email: string,
    password: string
  ): Promise<{ user: User; token: string }> {
    // Validate input
    if (username.length < 3) {
      throw new Error('Username must be at least 3 characters');
    }
    if (password.length < 6) {
      throw new Error('Password must be at least 6 characters');
    }

    // Hash password
    const salt = await bcrypt.genSalt(10);
    const passwordHash = await bcrypt.hash(password, salt);

    const client = await pool.connect();
    try {
      await client.query('BEGIN');

      // Create user
      const userResult = await client.query(
        `INSERT INTO users (username, email, password_hash, dark_matter, last_login)
         VALUES ($1, $2, $3, 0, NOW())
         RETURNING id, username, email, dark_matter, created_at, last_login, is_admin, is_banned, alliance_id, email_verified`,
        [username, email, passwordHash]
      );

      const user: User = userResult.rows[0];

      // Create initial research record
      await client.query(
        `INSERT INTO research (user_id) VALUES ($1)`,
        [user.id]
      );

      // Create home planet
      const coordinates = await this.findEmptyCoordinates(client);
      await client.query(
        `INSERT INTO planets (user_id, name, galaxy, system, position, metal, crystal, deuterium, last_resource_update)
         VALUES ($1, $2, $3, $4, $5, 500, 300, 100, NOW())`,
        [user.id, 'Homeworld', coordinates.galaxy, coordinates.system, coordinates.position]
      );

      // Create initial score record
      await client.query(
        `INSERT INTO player_scores (user_id, total_score, economy_score, research_score, military_score)
         VALUES ($1, 0, 0, 0, 0)`,
        [user.id]
      );

      await client.query('COMMIT');

      // Generate JWT token
      const token = this.generateToken(user.id);

      return { user, token };
    } catch (error: any) {
      await client.query('ROLLBACK');
      if (error.code === '23505') {
        // Unique violation
        if (error.constraint === 'users_username_key') {
          throw new Error('Username already exists');
        } else if (error.constraint === 'users_email_key') {
          throw new Error('Email already exists');
        }
      }
      throw error;
    } finally {
      client.release();
    }
  }

  static async login(
    identifier: string,
    password: string
  ): Promise<{ user: User; token: string }> {
    const normalizedIdentifier = identifier.trim();
    const isEmail = normalizedIdentifier.includes('@');
    const query = isEmail
      ? `SELECT id, username, email, password_hash, dark_matter, created_at, last_login, is_admin, is_banned, alliance_id, email_verified 
         FROM users WHERE LOWER(email) = LOWER($1)`
      : `SELECT id, username, email, password_hash, dark_matter, created_at, last_login, is_admin, is_banned, alliance_id, email_verified 
         FROM users WHERE username = $1`;

    const lookupValue = isEmail ? normalizedIdentifier.toLowerCase() : normalizedIdentifier;
    const result = await pool.query(query, [lookupValue]);

    if (result.rows.length === 0) {
      throw new Error('Invalid username or password');
    }

    const user = result.rows[0];

    if (user.is_banned) {
      throw new Error('Account is banned');
    }

    const validPassword = await bcrypt.compare(password, user.password_hash);
    if (!validPassword) {
      throw new Error('Invalid username or password');
    }

    // Update last login
    await pool.query(
      'UPDATE users SET last_login = NOW() WHERE id = $1',
      [user.id]
    );

    const token = this.generateToken(user.id);

    // Remove password hash from returned user
    delete user.password_hash;

    return { user, token };
  }

  private static generateToken(userId: number): string {
    const secret = process.env.JWT_SECRET || 'your_super_secret_jwt_key';
    const expiresIn = process.env.JWT_EXPIRES_IN || '7d';
    const options: SignOptions = { expiresIn: expiresIn as any };
    return jwt.sign({ userId }, secret, options);
  }

  private static async findEmptyCoordinates(
    client: PoolClient
  ): Promise<{ galaxy: number; system: number; position: number }> {
    // Randomly find an empty coordinate
    for (let attempts = 0; attempts < 100; attempts++) {
      const galaxy = Math.floor(Math.random() * 9) + 1;
      const system = Math.floor(Math.random() * 499) + 1;
      const position = Math.floor(Math.random() * 15) + 1;

      const result = await client.query(
        'SELECT id FROM planets WHERE galaxy = $1 AND system = $2 AND position = $3',
        [galaxy, system, position]
      );

      if (result.rows.length === 0) {
        return { galaxy, system, position };
      }
    }

    throw new Error('Could not find empty coordinates');
  }
}
