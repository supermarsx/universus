// Matrix Digital Rain Animation
// Creates the iconic falling green characters effect from The Matrix

class MatrixRain {
    constructor(canvasId) {
        this.canvas = document.getElementById(canvasId);
        if (!this.canvas) {
            console.error(`Canvas with id "${canvasId}" not found`);
            return;
        }

        this.ctx = this.canvas.getContext('2d');
        this.resizeCanvas();
        
        // Matrix characters - mix of katakana, latin, and symbols
        this.characters = 'ｦｱｳｴｵｶｷｸｹｺｻｼｽｾｿﾀﾁﾂﾃﾄﾅﾆﾇﾈﾉﾊﾋﾌﾍﾎﾏﾐﾑﾒﾓﾔﾕﾖﾗﾘﾙﾚﾛﾜﾝABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789@#$%^&*()_+-=[]{}|;:,.<>?';
        
        this.fontSize = 16;
        this.columns = Math.floor(this.canvas.width / this.fontSize);
        this.drops = [];
        
        // Initialize drops
        for (let i = 0; i < this.columns; i++) {
            this.drops[i] = Math.floor(Math.random() * -100);
        }

        this.colors = {
            primary: '#00ff41',      // Bright green
            secondary: '#008f11',    // Dark green
            highlight: '#39ff14',    // Neon green
            fade: 'rgba(0, 0, 0, 0.05)' // Black fade
        };

        // Animation settings
        this.speed = 33; // milliseconds per frame (~30 fps)
        this.running = false;
        
        // Start animation
        this.start();
        
        // Handle window resize
        window.addEventListener('resize', () => {
            this.resizeCanvas();
            this.columns = Math.floor(this.canvas.width / this.fontSize);
            
            // Reinitialize drops for new column count
            const newDrops = [];
            for (let i = 0; i < this.columns; i++) {
                newDrops[i] = i < this.drops.length 
                    ? this.drops[i] 
                    : Math.floor(Math.random() * -100);
            }
            this.drops = newDrops;
        });
    }

    resizeCanvas() {
        this.canvas.width = window.innerWidth;
        this.canvas.height = window.innerHeight;
    }

    draw() {
        // Create fade effect by drawing semi-transparent black rectangle
        this.ctx.fillStyle = this.colors.fade;
        this.ctx.fillRect(0, 0, this.canvas.width, this.canvas.height);

        // Set font
        this.ctx.font = `${this.fontSize}px monospace`;

        // Draw characters
        for (let i = 0; i < this.drops.length; i++) {
            // Randomly select character
            const char = this.characters[Math.floor(Math.random() * this.characters.length)];
            
            // Calculate position
            const x = i * this.fontSize;
            const y = this.drops[i] * this.fontSize;

            // Determine color based on position (create trailing effect)
            if (Math.random() > 0.98) {
                // Occasional bright highlight at the head
                this.ctx.fillStyle = this.colors.highlight;
            } else if (this.drops[i] * this.fontSize > this.canvas.height - 100) {
                // Fade to darker green near bottom
                this.ctx.fillStyle = this.colors.secondary;
            } else {
                // Standard green
                this.ctx.fillStyle = this.colors.primary;
            }

            // Draw the character
            this.ctx.fillText(char, x, y);

            // Randomly reset drop to top when it goes off screen
            if (y > this.canvas.height && Math.random() > 0.975) {
                this.drops[i] = 0;
            }

            // Move drop down
            this.drops[i]++;
        }
    }

    start() {
        if (this.running) return;
        this.running = true;
        this.animate();
    }

    stop() {
        this.running = false;
    }

    animate() {
        if (!this.running) return;
        
        this.draw();
        setTimeout(() => {
            requestAnimationFrame(() => this.animate());
        }, this.speed);
    }

    // Change animation speed (lower = faster)
    setSpeed(speed) {
        this.speed = Math.max(10, Math.min(100, speed));
    }

    // Change color scheme
    setColors(primary, secondary, highlight) {
        this.colors.primary = primary || this.colors.primary;
        this.colors.secondary = secondary || this.colors.secondary;
        this.colors.highlight = highlight || this.colors.highlight;
    }
}

// Initialize Matrix Rain when DOM is loaded
document.addEventListener('DOMContentLoaded', () => {
    const matrixRain = new MatrixRain('matrixCanvas');
    
    // Make it globally accessible for customization
    window.matrixRain = matrixRain;
});

// Additional Matrix Effects
class MatrixEffects {
    constructor() {
        this.glitchElements = document.querySelectorAll('.glitch');
        this.setupGlitchEffect();
    }

    setupGlitchEffect() {
        this.glitchElements.forEach(element => {
            // Random glitch intervals
            setInterval(() => {
                if (Math.random() > 0.95) {
                    element.style.animation = 'none';
                    setTimeout(() => {
                        element.style.animation = '';
                    }, 50);
                }
            }, 3000);
        });
    }

    // Create digital rain text effect for specific element
    createTextRain(elementId) {
        const element = document.getElementById(elementId);
        if (!element) return;

        const text = element.textContent;
        const chars = 'ｦｱｳｴｵｶｷｸｹｺｻｼｽｾｿﾀﾁﾂﾃﾄ0123456789@#$%^&*';
        let iteration = 0;

        const interval = setInterval(() => {
            element.textContent = text
                .split('')
                .map((char, index) => {
                    if (index < iteration) {
                        return text[index];
                    }
                    return chars[Math.floor(Math.random() * chars.length)];
                })
                .join('');

            if (iteration >= text.length) {
                clearInterval(interval);
            }

            iteration += 1 / 3;
        }, 30);
    }

    // Add scanline effect to element
    addScanlines(elementId) {
        const element = document.getElementById(elementId);
        if (!element) return;

        const scanline = document.createElement('div');
        scanline.style.cssText = `
            position: absolute;
            top: 0;
            left: 0;
            width: 100%;
            height: 100%;
            background: linear-gradient(
                to bottom,
                transparent 50%,
                rgba(0, 255, 65, 0.03) 50%
            );
            background-size: 100% 4px;
            pointer-events: none;
            z-index: 1000;
        `;
        
        element.style.position = 'relative';
        element.appendChild(scanline);
    }

    // Create typing effect
    typeEffect(elementId, text, speed = 50) {
        const element = document.getElementById(elementId);
        if (!element) return;

        element.textContent = '';
        let i = 0;

        const typing = setInterval(() => {
            if (i < text.length) {
                element.textContent += text.charAt(i);
                i++;
            } else {
                clearInterval(typing);
            }
        }, speed);
    }

    // Flicker effect
    flicker(elementId, duration = 200) {
        const element = document.getElementById(elementId);
        if (!element) return;

        const originalOpacity = element.style.opacity || '1';
        let flickerCount = 0;
        const maxFlickers = 5;

        const flickerInterval = setInterval(() => {
            element.style.opacity = Math.random() > 0.5 ? '0.3' : '1';
            flickerCount++;

            if (flickerCount >= maxFlickers) {
                clearInterval(flickerInterval);
                element.style.opacity = originalOpacity;
            }
        }, duration / maxFlickers);
    }

    // Create CRT screen effect
    addCRTEffect(elementId) {
        const element = document.getElementById(elementId);
        if (!element) return;

        // Add CSS for CRT effect
        const style = document.createElement('style');
        style.textContent = `
            #${elementId}::before {
                content: " ";
                display: block;
                position: absolute;
                top: 0;
                left: 0;
                bottom: 0;
                right: 0;
                background: linear-gradient(
                    rgba(18, 16, 16, 0) 50%, 
                    rgba(0, 0, 0, 0.25) 50%
                ), 
                linear-gradient(
                    90deg, 
                    rgba(255, 0, 0, 0.06), 
                    rgba(0, 255, 0, 0.02), 
                    rgba(0, 0, 255, 0.06)
                );
                z-index: 2;
                background-size: 100% 2px, 3px 100%;
                pointer-events: none;
            }
            
            #${elementId}::after {
                content: " ";
                display: block;
                position: absolute;
                top: 0;
                left: 0;
                bottom: 0;
                right: 0;
                background: rgba(18, 16, 16, 0.1);
                opacity: 0;
                z-index: 2;
                pointer-events: none;
                animation: flicker 0.15s infinite;
            }
            
            @keyframes flicker {
                0% { opacity: 0.27861; }
                5% { opacity: 0.34769; }
                10% { opacity: 0.23604; }
                15% { opacity: 0.90626; }
                20% { opacity: 0.18128; }
                25% { opacity: 0.83891; }
                30% { opacity: 0.65583; }
                35% { opacity: 0.67807; }
                40% { opacity: 0.26559; }
                45% { opacity: 0.84693; }
                50% { opacity: 0.96019; }
                55% { opacity: 0.08594; }
                60% { opacity: 0.20313; }
                65% { opacity: 0.71988; }
                70% { opacity: 0.53455; }
                75% { opacity: 0.37288; }
                80% { opacity: 0.71428; }
                85% { opacity: 0.70419; }
                90% { opacity: 0.7003; }
                95% { opacity: 0.36108; }
                100% { opacity: 0.24387; }
            }
        `;
        document.head.appendChild(style);
        
        element.style.position = 'relative';
    }
}

// Initialize Matrix Effects
document.addEventListener('DOMContentLoaded', () => {
    const matrixEffects = new MatrixEffects();
    window.matrixEffects = matrixEffects;
});

// Export classes for external use
if (typeof module !== 'undefined' && module.exports) {
    module.exports = { MatrixRain, MatrixEffects };
}
