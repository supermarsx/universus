/**
 * Millisecond Precision Combat Tracker
 * 
 * Tracks combat events with microsecond precision for:
 * - Coordinated attacks (ACS)
 * - Attack windows (precise timing for simultaneous arrival)
 * - Real-time combat visualization
 * - Server-side reconciliation with client predictions
 * 
 * Performance requirements:
 * - Sub-millisecond timestamp accuracy
 * - Atomic fleet movement updates
 * - Network latency compensation
 */

import { pool } from '../config/database';
import { Pool } from 'pg';

interface CombatEvent {
    eventId: string;
    eventType: 'attack_started' | 'round_executed' | 'attack_completed' | 'fleet_arrival' | 'fleet_departure';
    combatId: number;
    timestamp: bigint; // Microsecond precision
    data: any;
}

interface FleetMovement {
    fleetId: number;
    fromPlanetId: number;
    toPlanetId: number;
    arrivalTime: bigint; // Microsecond precision
    shipCounts: { [key: string]: number };
}

export class MillisecondCombatTracker {
    private pool: Pool;
    private eventBuffer: CombatEvent[] = [];
    private flushInterval: NodeJS.Timeout | null = null;

    constructor(dbPool: Pool) {
        this.pool = dbPool;
        this.startEventFlusher();
    }

    /**
     * Get current time with microsecond precision
     * Using process.hrtime() for high-resolution timing
     */
    private getCurrentTimeMicros(): bigint {
        const [seconds, nanoseconds] = process.hrtime();
        return BigInt(seconds) * BigInt(1000000) + BigInt(Math.floor(nanoseconds / 1000));
    }

    /**
     * Convert microseconds to PostgreSQL timestamp
     */
    private microsToTimestamp(micros: bigint): Date {
        return new Date(Number(micros / BigInt(1000)));
    }

    /**
     * Track fleet departure with precise timestamp
     */
    async trackFleetDeparture(
        userId: number,
        planetId: number,
        targetPlanetId: number,
        ships: { [key: string]: number },
        arrivalTimeMicros: bigint
    ): Promise<number> {
        const departureTime = this.getCurrentTimeMicros();
        
        const query = `
            INSERT INTO fleet_movements_precise (
                user_id,
                from_planet_id,
                to_planet_id,
                ships,
                departure_time_micros,
                arrival_time_micros,
                status
            ) VALUES ($1, $2, $3, $4, $5, $6, 'in_transit')
            RETURNING id
        `;

        const result = await this.pool.query(query, [
            userId,
            planetId,
            targetPlanetId,
            JSON.stringify(ships),
            departureTime.toString(),
            arrivalTimeMicros.toString()
        ]);

        // Log event
        this.logEvent({
            eventId: this.generateEventId(),
            eventType: 'fleet_departure',
            combatId: 0, // Will be set when combat starts
            timestamp: departureTime,
            data: {
                fleetId: result.rows[0].id,
                userId,
                fromPlanet: planetId,
                toPlanet: targetPlanetId,
                arrivalTime: arrivalTimeMicros.toString()
            }
        });

        return result.rows[0].id;
    }

    /**
     * Calculate precise arrival time based on distance and speed
     * Returns microsecond timestamp
     */
    calculateArrivalTime(
        fromCoords: { galaxy: number; system: number; position: number },
        toCoords: { galaxy: number; system: number; position: number },
        fleetSpeed: number,
        speedFactor: number = 1.0
    ): bigint {
        const distance = this.calculateDistance(fromCoords, toCoords);
        
        // Base speed calculation (units per second)
        const baseSpeed = fleetSpeed * speedFactor;
        
        // Travel time in microseconds
        const travelTimeMicros = BigInt(Math.floor((distance / baseSpeed) * 1000000));
        
        const currentTime = this.getCurrentTimeMicros();
        return currentTime + travelTimeMicros;
    }

    /**
     * Calculate distance between two coordinates
     */
    private calculateDistance(
        from: { galaxy: number; system: number; position: number },
        to: { galaxy: number; system: number; position: number }
    ): number {
        if (from.galaxy !== to.galaxy) {
            return 20000 * Math.abs(from.galaxy - to.galaxy);
        } else if (from.system !== to.system) {
            return 2700 + 95 * Math.abs(from.system - to.system);
        } else {
            return 1000 + 5 * Math.abs(from.position - to.position);
        }
    }

    /**
     * Get all fleets arriving within a time window (microseconds)
     * Used for coordinated attacks (ACS)
     */
    async getFleetsInWindow(
        targetPlanetId: number,
        windowCenterMicros: bigint,
        windowSizeMicros: bigint
    ): Promise<FleetMovement[]> {
        const windowStart = windowCenterMicros - windowSizeMicros / BigInt(2);
        const windowEnd = windowCenterMicros + windowSizeMicros / BigInt(2);

        const query = `
            SELECT 
                id as fleet_id,
                user_id,
                from_planet_id,
                to_planet_id,
                ships,
                arrival_time_micros
            FROM fleet_movements_precise
            WHERE to_planet_id = $1
                AND status = 'in_transit'
                AND arrival_time_micros >= $2
                AND arrival_time_micros <= $3
            ORDER BY arrival_time_micros ASC
        `;

        const result = await this.pool.query(query, [
            targetPlanetId,
            windowStart.toString(),
            windowEnd.toString()
        ]);

        return result.rows.map(row => ({
            fleetId: row.fleet_id,
            fromPlanetId: row.from_planet_id,
            toPlanetId: row.to_planet_id,
            arrivalTime: BigInt(row.arrival_time_micros),
            shipCounts: JSON.parse(row.ships)
        }));
    }

    /**
     * Execute combat when fleet(s) arrive
     * Handles multiple fleets arriving simultaneously (ACS)
     */
    async executeCombatAtArrival(
        targetPlanetId: number,
        arrivalTimeMicros: bigint
    ): Promise<number> {
        const combatStartTime = this.getCurrentTimeMicros();
        
        // Create combat record with precise timing
        const combatQuery = `
            INSERT INTO combats_precise (
                planet_id,
                start_time_micros,
                status
            ) VALUES ($1, $2, 'in_progress')
            RETURNING id
        `;

        const combatResult = await this.pool.query(combatQuery, [
            targetPlanetId,
            combatStartTime.toString()
        ]);

        const combatId = combatResult.rows[0].id;

        // Log combat start event
        this.logEvent({
            eventId: this.generateEventId(),
            eventType: 'attack_started',
            combatId,
            timestamp: combatStartTime,
            data: {
                targetPlanet: targetPlanetId,
                scheduledArrival: arrivalTimeMicros.toString(),
                actualStart: combatStartTime.toString()
            }
        });

        return combatId;
    }

    /**
     * Log a combat round with precise timing
     */
    async logCombatRound(
        combatId: number,
        roundNumber: number,
        roundData: any
    ): Promise<void> {
        const roundTime = this.getCurrentTimeMicros();

        const query = `
            INSERT INTO combat_rounds_precise (
                combat_id,
                round_number,
                round_time_micros,
                attacker_ships_remaining,
                defender_ships_remaining,
                damage_dealt_attacker,
                damage_dealt_defender
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
        `;

        await this.pool.query(query, [
            combatId,
            roundNumber,
            roundTime.toString(),
            JSON.stringify(roundData.attackerShips),
            JSON.stringify(roundData.defenderShips),
            roundData.attackerDamage,
            roundData.defenderDamage
        ]);

        this.logEvent({
            eventId: this.generateEventId(),
            eventType: 'round_executed',
            combatId,
            timestamp: roundTime,
            data: {
                roundNumber,
                ...roundData
            }
        });
    }

    /**
     * Complete combat and log final results
     */
    async completeCombat(
        combatId: number,
        winner: 'attacker' | 'defender' | 'draw',
        finalData: any
    ): Promise<void> {
        const completionTime = this.getCurrentTimeMicros();

        const query = `
            UPDATE combats_precise
            SET 
                end_time_micros = $1,
                winner = $2,
                final_data = $3,
                status = 'completed'
            WHERE id = $4
        `;

        await this.pool.query(query, [
            completionTime.toString(),
            winner,
            JSON.stringify(finalData),
            combatId
        ]);

        this.logEvent({
            eventId: this.generateEventId(),
            eventType: 'attack_completed',
            combatId,
            timestamp: completionTime,
            data: {
                winner,
                ...finalData
            }
        });
    }

    /**
     * Get combat statistics with timing analysis
     */
    async getCombatStats(combatId: number): Promise<any> {
        const query = `
            SELECT 
                c.id,
                c.planet_id,
                c.start_time_micros,
                c.end_time_micros,
                c.winner,
                c.final_data,
                (c.end_time_micros - c.start_time_micros) as duration_micros,
                json_agg(
                    json_build_object(
                        'round', r.round_number,
                        'time', r.round_time_micros,
                        'attacker_ships', r.attacker_ships_remaining,
                        'defender_ships', r.defender_ships_remaining
                    ) ORDER BY r.round_number
                ) as rounds
            FROM combats_precise c
            LEFT JOIN combat_rounds_precise r ON c.id = r.combat_id
            WHERE c.id = $1
            GROUP BY c.id
        `;

        const result = await this.pool.query(query, [combatId]);
        
        if (result.rows.length === 0) {
            return null;
        }

        const combat = result.rows[0];
        
        return {
            combatId: combat.id,
            planetId: combat.planet_id,
            startTime: this.microsToTimestamp(BigInt(combat.start_time_micros)),
            endTime: combat.end_time_micros ? this.microsToTimestamp(BigInt(combat.end_time_micros)) : null,
            durationMs: combat.duration_micros ? Number(BigInt(combat.duration_micros) / BigInt(1000)) : null,
            winner: combat.winner,
            finalData: combat.final_data,
            rounds: combat.rounds || []
        };
    }

    /**
     * Log combat event to buffer
     */
    private logEvent(event: CombatEvent): void {
        this.eventBuffer.push(event);
        
        // Flush if buffer is getting large
        if (this.eventBuffer.length >= 100) {
            this.flushEvents();
        }
    }

    /**
     * Generate unique event ID with timestamp
     */
    private generateEventId(): string {
        const timestamp = this.getCurrentTimeMicros();
        const random = Math.floor(Math.random() * 1000000);
        return `${timestamp}-${random}`;
    }

    /**
     * Start periodic event flusher
     */
    private startEventFlusher(): void {
        this.flushInterval = setInterval(() => {
            this.flushEvents();
        }, 1000); // Flush every second
    }

    /**
     * Flush buffered events to database
     */
    private async flushEvents(): Promise<void> {
        if (this.eventBuffer.length === 0) {
            return;
        }

        const eventsToFlush = [...this.eventBuffer];
        this.eventBuffer = [];

        try {
            const query = `
                INSERT INTO combat_events_precise (
                    event_id,
                    event_type,
                    combat_id,
                    timestamp_micros,
                    event_data
                ) VALUES ($1, $2, $3, $4, $5)
            `;

            for (const event of eventsToFlush) {
                await this.pool.query(query, [
                    event.eventId,
                    event.eventType,
                    event.combatId,
                    event.timestamp.toString(),
                    JSON.stringify(event.data)
                ]);
            }
        } catch (error) {
            console.error('Error flushing combat events:', error);
            // Re-add failed events to buffer
            this.eventBuffer.unshift(...eventsToFlush);
        }
    }

    /**
     * Stop event flusher
     */
    stopEventFlusher(): void {
        if (this.flushInterval) {
            clearInterval(this.flushInterval);
            this.flushInterval = null;
        }
        this.flushEvents(); // Final flush
    }

    /**
     * Measure and compensate for network latency
     * Client sends timestamp, we calculate round-trip time
     */
    measureLatency(clientTimestampMicros: bigint): {
        serverTime: bigint;
        estimatedLatency: bigint;
    } {
        const serverTime = this.getCurrentTimeMicros();
        const estimatedLatency = (serverTime - clientTimestampMicros) / BigInt(2);
        
        return {
            serverTime,
            estimatedLatency
        };
    }
}

// Export singleton instance
export const combatTracker = new MillisecondCombatTracker(pool);
