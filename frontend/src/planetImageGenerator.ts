// @ts-nocheck
/**
 * Planet Image Generator
 * Procedurally generates unique planet visuals using HTML5 Canvas
 * Based on planet coordinates, temperature, and type
 * 
 * Planet Types:
 * - terrestrial: Earth-like planets with continents and oceans
 * - gas_giant: Jupiter-like planets with atmospheric bands
 * - ice_world: Frozen planets with ice coverage
 * - desert: Arid planets with sandy landscapes
 * - lava: Molten surface planets
 * - metal: Metallic surface planets
 * - artificial: Geometric, constructed worlds
 */

class PlanetImageGenerator {
    constructor() {
        this.cache = new Map();
        this.canvas = document.createElement('canvas');
        this.ctx = this.canvas.getContext('2d');
    }

    /**
     * Generate a unique planet image
     * @param {Object} planetData - Planet properties
     * @param {number} planetData.galaxy - Galaxy coordinate
     * @param {number} planetData.system - System coordinate
     * @param {number} planetData.position - Position coordinate
     * @param {string} planetData.type - Planet type
     * @param {number} planetData.temperature - Temperature (-100 to 100)
     * @param {number} size - Canvas size in pixels (default: 256)
     * @returns {string} Data URL of the generated image
     */
    generate(planetData, size = 256) {
        const cacheKey = this.getCacheKey(planetData, size);
        
        // Check cache first
        if (this.cache.has(cacheKey)) {
            return this.cache.get(cacheKey);
        }

        // Setup canvas
        this.canvas.width = size;
        this.canvas.height = size;
        this.ctx.clearRect(0, 0, size, size);

        // Create seeded random number generator
        const seed = this.generateSeed(planetData);
        const rng = this.createRNG(seed);

        // Draw planet based on type
        const planetType = planetData.type || this.determinePlanetType(planetData.temperature, rng);
        
        switch (planetType) {
            case 'terrestrial':
                this.drawTerrestrialPlanet(size, planetData, rng);
                break;
            case 'gas_giant':
                this.drawGasGiant(size, planetData, rng);
                break;
            case 'ice_world':
                this.drawIceWorld(size, planetData, rng);
                break;
            case 'desert':
                this.drawDesertPlanet(size, planetData, rng);
                break;
            case 'lava':
                this.drawLavaPlanet(size, planetData, rng);
                break;
            case 'metal':
                this.drawMetalPlanet(size, planetData, rng);
                break;
            case 'artificial':
                this.drawArtificialPlanet(size, planetData, rng);
                break;
            default:
                this.drawTerrestrialPlanet(size, planetData, rng);
        }

        // Add atmospheric glow
        this.addAtmosphericGlow(size, planetType, rng);

        // Optionally add rings
        if (rng() > 0.7 && (planetType === 'gas_giant' || planetType === 'ice_world')) {
            this.drawRings(size, planetData, rng);
        }

        // Get data URL and cache it
        const dataUrl = this.canvas.toDataURL('image/png');
        this.cache.set(cacheKey, dataUrl);

        return dataUrl;
    }

    /**
     * Generate a unique seed from planet coordinates
     */
    generateSeed(planetData) {
        return (planetData.galaxy * 1000000 + planetData.system * 1000 + planetData.position);
    }

    /**
     * Create a seeded random number generator
     */
    createRNG(seed) {
        let state = seed;
        return function() {
            state = (state * 9301 + 49297) % 233280;
            return state / 233280;
        };
    }

    /**
     * Determine planet type based on temperature
     */
    determinePlanetType(temperature, rng) {
        const rand = rng();
        
        if (temperature < -50) {
            return rand > 0.5 ? 'ice_world' : 'metal';
        } else if (temperature < -20) {
            return rand > 0.7 ? 'ice_world' : 'terrestrial';
        } else if (temperature < 30) {
            return rand > 0.8 ? 'gas_giant' : 'terrestrial';
        } else if (temperature < 60) {
            return rand > 0.6 ? 'desert' : 'terrestrial';
        } else {
            return rand > 0.5 ? 'lava' : 'desert';
        }
    }

    /**
     * Draw a terrestrial (Earth-like) planet
     */
    drawTerrestrialPlanet(size, planetData, rng) {
        const centerX = size / 2;
        const centerY = size / 2;
        const radius = size * 0.4;

        // Base ocean color
        const oceanGradient = this.ctx.createRadialGradient(
            centerX - radius * 0.3, centerY - radius * 0.3, 0,
            centerX, centerY, radius
        );
        oceanGradient.addColorStop(0, '#4fa3d1');
        oceanGradient.addColorStop(0.7, '#2874a6');
        oceanGradient.addColorStop(1, '#1a4d6f');

        this.ctx.fillStyle = oceanGradient;
        this.ctx.beginPath();
        this.ctx.arc(centerX, centerY, radius, 0, Math.PI * 2);
        this.ctx.fill();

        // Draw continents using Perlin-like noise
        this.ctx.save();
        this.ctx.beginPath();
        this.ctx.arc(centerX, centerY, radius, 0, Math.PI * 2);
        this.ctx.clip();

        const numContinents = Math.floor(rng() * 3) + 3;
        for (let i = 0; i < numContinents; i++) {
            const continentX = centerX + (rng() - 0.5) * radius * 1.5;
            const continentY = centerY + (rng() - 0.5) * radius * 1.5;
            const continentSize = radius * (0.3 + rng() * 0.4);

            const landGradient = this.ctx.createRadialGradient(
                continentX, continentY, 0,
                continentX, continentY, continentSize
            );
            landGradient.addColorStop(0, '#90c968');
            landGradient.addColorStop(0.6, '#6b9642');
            landGradient.addColorStop(1, 'transparent');

            this.ctx.fillStyle = landGradient;
            this.ctx.beginPath();
            this.ctx.arc(continentX, continentY, continentSize, 0, Math.PI * 2);
            this.ctx.fill();
        }

        // Add clouds
        this.ctx.globalAlpha = 0.4;
        for (let i = 0; i < 20; i++) {
            const cloudX = centerX + (rng() - 0.5) * radius * 1.8;
            const cloudY = centerY + (rng() - 0.5) * radius * 1.8;
            const cloudSize = radius * (0.1 + rng() * 0.2);

            this.ctx.fillStyle = 'white';
            this.ctx.beginPath();
            this.ctx.arc(cloudX, cloudY, cloudSize, 0, Math.PI * 2);
            this.ctx.fill();
        }

        this.ctx.restore();
        this.ctx.globalAlpha = 1;
    }

    /**
     * Draw a gas giant planet
     */
    drawGasGiant(size, planetData, rng) {
        const centerX = size / 2;
        const centerY = size / 2;
        const radius = size * 0.45;

        // Base color
        const colors = [
            ['#c9a56e', '#a8844f', '#8b6e42'],
            ['#e8d4a8', '#c9b589', '#a89670'],
            ['#d4a373', '#b88655', '#9c6f42']
        ];
        const colorSet = colors[Math.floor(rng() * colors.length)];

        // Draw atmospheric bands
        for (let y = -radius; y <= radius; y += 1) {
            const distFromCenter = Math.abs(y);
            const alpha = Math.sqrt(1 - (distFromCenter / radius) ** 2);
            
            if (alpha > 0) {
                const bandIndex = Math.floor((y + radius) / (radius * 0.15 + rng() * 10)) % colorSet.length;
                const turbulence = (rng() - 0.5) * 0.1;
                
                this.ctx.strokeStyle = this.adjustColor(colorSet[bandIndex], turbulence);
                this.ctx.globalAlpha = alpha;
                this.ctx.beginPath();
                this.ctx.moveTo(centerX - alpha * radius, centerY + y);
                this.ctx.lineTo(centerX + alpha * radius, centerY + y);
                this.ctx.stroke();
            }
        }

        this.ctx.globalAlpha = 1;

        // Add the Great Red Spot (or similar storm)
        if (rng() > 0.5) {
            const spotX = centerX + (rng() - 0.5) * radius * 0.8;
            const spotY = centerY + (rng() - 0.5) * radius * 0.6;
            const spotRadiusX = radius * 0.2;
            const spotRadiusY = radius * 0.12;

            this.ctx.globalAlpha = 0.6;
            this.ctx.fillStyle = '#d45f47';
            this.ctx.beginPath();
            this.ctx.ellipse(spotX, spotY, spotRadiusX, spotRadiusY, rng() * Math.PI, 0, Math.PI * 2);
            this.ctx.fill();
            this.ctx.globalAlpha = 1;
        }

        // Sphere shading
        this.addSphereShading(size, radius);
    }

    /**
     * Draw an ice world
     */
    drawIceWorld(size, planetData, rng) {
        const centerX = size / 2;
        const centerY = size / 2;
        const radius = size * 0.4;

        // Base ice gradient
        const iceGradient = this.ctx.createRadialGradient(
            centerX - radius * 0.3, centerY - radius * 0.3, 0,
            centerX, centerY, radius
        );
        iceGradient.addColorStop(0, '#e8f4f8');
        iceGradient.addColorStop(0.6, '#b8d4e0');
        iceGradient.addColorStop(1, '#7ba5b8');

        this.ctx.fillStyle = iceGradient;
        this.ctx.beginPath();
        this.ctx.arc(centerX, centerY, radius, 0, Math.PI * 2);
        this.ctx.fill();

        // Add ice cracks and formations
        this.ctx.save();
        this.ctx.beginPath();
        this.ctx.arc(centerX, centerY, radius, 0, Math.PI * 2);
        this.ctx.clip();

        this.ctx.strokeStyle = 'rgba(200, 220, 230, 0.5)';
        this.ctx.lineWidth = 2;
        for (let i = 0; i < 30; i++) {
            const x1 = centerX + (rng() - 0.5) * radius * 1.8;
            const y1 = centerY + (rng() - 0.5) * radius * 1.8;
            const x2 = x1 + (rng() - 0.5) * radius * 0.3;
            const y2 = y1 + (rng() - 0.5) * radius * 0.3;

            this.ctx.beginPath();
            this.ctx.moveTo(x1, y1);
            this.ctx.lineTo(x2, y2);
            this.ctx.stroke();
        }

        this.ctx.restore();
        this.addSphereShading(size, radius);
    }

    /**
     * Draw a desert planet
     */
    drawDesertPlanet(size, planetData, rng) {
        const centerX = size / 2;
        const centerY = size / 2;
        const radius = size * 0.4;

        // Sandy gradient
        const sandGradient = this.ctx.createRadialGradient(
            centerX - radius * 0.3, centerY - radius * 0.3, 0,
            centerX, centerY, radius
        );
        sandGradient.addColorStop(0, '#e8c48f');
        sandGradient.addColorStop(0.6, '#d4a373');
        sandGradient.addColorStop(1, '#b8884f');

        this.ctx.fillStyle = sandGradient;
        this.ctx.beginPath();
        this.ctx.arc(centerX, centerY, radius, 0, Math.PI * 2);
        this.ctx.fill();

        // Add dunes pattern
        this.ctx.save();
        this.ctx.beginPath();
        this.ctx.arc(centerX, centerY, radius, 0, Math.PI * 2);
        this.ctx.clip();

        for (let i = 0; i < 15; i++) {
            const duneY = centerY + (rng() - 0.5) * radius * 1.6;
            const duneHeight = radius * 0.1;
            
            this.ctx.strokeStyle = `rgba(180, 136, 79, ${0.2 + rng() * 0.2})`;
            this.ctx.lineWidth = duneHeight;
            this.ctx.beginPath();
            this.ctx.moveTo(centerX - radius, duneY);
            this.ctx.lineTo(centerX + radius, duneY);
            this.ctx.stroke();
        }

        this.ctx.restore();
        this.addSphereShading(size, radius);
    }

    /**
     * Draw a lava planet
     */
    drawLavaPlanet(size, planetData, rng) {
        const centerX = size / 2;
        const centerY = size / 2;
        const radius = size * 0.4;

        // Molten gradient
        const lavaGradient = this.ctx.createRadialGradient(
            centerX - radius * 0.3, centerY - radius * 0.3, 0,
            centerX, centerY, radius
        );
        lavaGradient.addColorStop(0, '#ff6b3d');
        lavaGradient.addColorStop(0.5, '#d4452f');
        lavaGradient.addColorStop(1, '#8b2f21');

        this.ctx.fillStyle = lavaGradient;
        this.ctx.beginPath();
        this.ctx.arc(centerX, centerY, radius, 0, Math.PI * 2);
        this.ctx.fill();

        // Add lava flows
        this.ctx.save();
        this.ctx.beginPath();
        this.ctx.arc(centerX, centerY, radius, 0, Math.PI * 2);
        this.ctx.clip();

        for (let i = 0; i < 25; i++) {
            const flowX = centerX + (rng() - 0.5) * radius * 1.8;
            const flowY = centerY + (rng() - 0.5) * radius * 1.8;
            const flowSize = radius * (0.05 + rng() * 0.15);

            const flowGradient = this.ctx.createRadialGradient(
                flowX, flowY, 0,
                flowX, flowY, flowSize
            );
            flowGradient.addColorStop(0, '#ffcc00');
            flowGradient.addColorStop(0.5, '#ff6b3d');
            flowGradient.addColorStop(1, 'transparent');

            this.ctx.fillStyle = flowGradient;
            this.ctx.beginPath();
            this.ctx.arc(flowX, flowY, flowSize, 0, Math.PI * 2);
            this.ctx.fill();
        }

        this.ctx.restore();
    }

    /**
     * Draw a metal planet
     */
    drawMetalPlanet(size, planetData, rng) {
        const centerX = size / 2;
        const centerY = size / 2;
        const radius = size * 0.4;

        // Metallic gradient
        const metalGradient = this.ctx.createRadialGradient(
            centerX - radius * 0.3, centerY - radius * 0.3, 0,
            centerX, centerY, radius
        );
        metalGradient.addColorStop(0, '#c0c0c0');
        metalGradient.addColorStop(0.5, '#989898');
        metalGradient.addColorStop(1, '#606060');

        this.ctx.fillStyle = metalGradient;
        this.ctx.beginPath();
        this.ctx.arc(centerX, centerY, radius, 0, Math.PI * 2);
        this.ctx.fill();

        // Add metallic texture
        this.ctx.save();
        this.ctx.beginPath();
        this.ctx.arc(centerX, centerY, radius, 0, Math.PI * 2);
        this.ctx.clip();

        for (let i = 0; i < 40; i++) {
            const x = centerX + (rng() - 0.5) * radius * 1.8;
            const y = centerY + (rng() - 0.5) * radius * 1.8;
            const size = rng() * 3 + 1;

            this.ctx.fillStyle = `rgba(${128 + rng() * 100}, ${128 + rng() * 100}, ${128 + rng() * 100}, ${0.3 + rng() * 0.3})`;
            this.ctx.beginPath();
            this.ctx.arc(x, y, size, 0, Math.PI * 2);
            this.ctx.fill();
        }

        this.ctx.restore();
        this.addSphereShading(size, radius);
    }

    /**
     * Draw an artificial planet
     */
    drawArtificialPlanet(size, planetData, rng) {
        const centerX = size / 2;
        const centerY = size / 2;
        const radius = size * 0.4;

        // Base metallic surface
        this.ctx.fillStyle = '#4a5568';
        this.ctx.beginPath();
        this.ctx.arc(centerX, centerY, radius, 0, Math.PI * 2);
        this.ctx.fill();

        // Add geometric patterns
        this.ctx.save();
        this.ctx.beginPath();
        this.ctx.arc(centerX, centerY, radius, 0, Math.PI * 2);
        this.ctx.clip();

        // Grid pattern
        this.ctx.strokeStyle = '#6d7f99';
        this.ctx.lineWidth = 1;
        const gridSize = radius / 8;
        
        for (let x = centerX - radius; x <= centerX + radius; x += gridSize) {
            this.ctx.beginPath();
            this.ctx.moveTo(x, centerY - radius);
            this.ctx.lineTo(x, centerY + radius);
            this.ctx.stroke();
        }

        for (let y = centerY - radius; y <= centerY + radius; y += gridSize) {
            this.ctx.beginPath();
            this.ctx.moveTo(centerX - radius, y);
            this.ctx.lineTo(centerX + radius, y);
            this.ctx.stroke();
        }

        // Add panels
        for (let i = 0; i < 15; i++) {
            const panelX = centerX + (rng() - 0.5) * radius * 1.5;
            const panelY = centerY + (rng() - 0.5) * radius * 1.5;
            const panelSize = radius * (0.05 + rng() * 0.1);

            this.ctx.fillStyle = rng() > 0.5 ? '#3d5a80' : '#ee6c4d';
            this.ctx.fillRect(
                panelX - panelSize / 2,
                panelY - panelSize / 2,
                panelSize,
                panelSize
            );
        }

        this.ctx.restore();
    }

    /**
     * Add atmospheric glow effect
     */
    addAtmosphericGlow(size, planetType, rng) {
        const centerX = size / 2;
        const centerY = size / 2;
        const radius = size * 0.4;

        const glowColors = {
            terrestrial: 'rgba(100, 150, 200, 0.3)',
            gas_giant: 'rgba(200, 170, 120, 0.2)',
            ice_world: 'rgba(180, 220, 250, 0.3)',
            desert: 'rgba(220, 180, 100, 0.2)',
            lava: 'rgba(255, 100, 50, 0.4)',
            metal: 'rgba(150, 150, 150, 0.2)',
            artificial: 'rgba(70, 130, 180, 0.3)'
        };

        const glowGradient = this.ctx.createRadialGradient(
            centerX, centerY, radius,
            centerX, centerY, radius * 1.2
        );
        glowGradient.addColorStop(0, glowColors[planetType] || glowColors.terrestrial);
        glowGradient.addColorStop(1, 'transparent');

        this.ctx.fillStyle = glowGradient;
        this.ctx.beginPath();
        this.ctx.arc(centerX, centerY, radius * 1.2, 0, Math.PI * 2);
        this.ctx.fill();
    }

    /**
     * Add sphere shading for 3D effect
     */
    addSphereShading(size, radius) {
        const centerX = size / 2;
        const centerY = size / 2;

        // Shadow gradient
        const shadowGradient = this.ctx.createRadialGradient(
            centerX + radius * 0.3, centerY + radius * 0.3, 0,
            centerX, centerY, radius
        );
        shadowGradient.addColorStop(0, 'transparent');
        shadowGradient.addColorStop(0.7, 'rgba(0, 0, 0, 0.2)');
        shadowGradient.addColorStop(1, 'rgba(0, 0, 0, 0.5)');

        this.ctx.fillStyle = shadowGradient;
        this.ctx.beginPath();
        this.ctx.arc(centerX, centerY, radius, 0, Math.PI * 2);
        this.ctx.fill();
    }

    /**
     * Draw planetary rings
     */
    drawRings(size, planetData, rng) {
        const centerX = size / 2;
        const centerY = size / 2;
        const planetRadius = size * 0.4;
        const ringInner = planetRadius * 1.3;
        const ringOuter = planetRadius * 1.8;

        this.ctx.globalAlpha = 0.4;
        
        // Draw multiple ring bands
        const numBands = 3 + Math.floor(rng() * 3);
        for (let i = 0; i < numBands; i++) {
            const bandInner = ringInner + (ringOuter - ringInner) * (i / numBands);
            const bandOuter = ringInner + (ringOuter - ringInner) * ((i + 1) / numBands);
            
            const brightness = 150 + Math.floor(rng() * 80);
            this.ctx.strokeStyle = `rgba(${brightness}, ${brightness - 20}, ${brightness - 40}, 0.6)`;
            this.ctx.lineWidth = (bandOuter - bandInner) * 0.8;
            
            this.ctx.beginPath();
            this.ctx.ellipse(centerX, centerY, (bandInner + bandOuter) / 2, (bandInner + bandOuter) / 2 * 0.2, 0, 0, Math.PI * 2);
            this.ctx.stroke();
        }

        this.ctx.globalAlpha = 1;
    }

    /**
     * Adjust color brightness
     */
    adjustColor(hexColor, adjustment) {
        const hex = hexColor.replace('#', '');
        const r = Math.max(0, Math.min(255, parseInt(hex.substr(0, 2), 16) + adjustment * 100));
        const g = Math.max(0, Math.min(255, parseInt(hex.substr(2, 2), 16) + adjustment * 100));
        const b = Math.max(0, Math.min(255, parseInt(hex.substr(4, 2), 16) + adjustment * 100));
        return `rgb(${r}, ${g}, ${b})`;
    }

    /**
     * Generate cache key
     */
    getCacheKey(planetData, size) {
        return `${planetData.galaxy}-${planetData.system}-${planetData.position}-${size}`;
    }

    /**
     * Clear the cache
     */
    clearCache() {
        this.cache.clear();
    }
}

// Export for use in other scripts
if (typeof module !== 'undefined' && module.exports) {
    module.exports = PlanetImageGenerator;
}
