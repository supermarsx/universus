/**
 * Bot Management System JavaScript
 * Handles bot creation, configuration, monitoring, and bulk operations
 */

let allBots = [];
let filteredBots = [];

const PERSONALITY_DESCRIPTIONS = {
    aggressive_conqueror: 'Prioritizes military expansion with frequent attacks, rapid fleet building, and aggressive resource acquisition. High aggression, low diplomacy.',
    strategic_builder: 'Focuses on infrastructure development, balanced growth, and defensive strategies. Methodical approach with long-term planning.',
    diplomatic_negotiator: 'Alliance-focused, trade-oriented, and peaceful expansion. Prefers cooperation over conflict, actively seeks diplomatic solutions.',
    resource_hoarder: 'Maximum resource gathering with conservative playstyle and long-term planning. Builds strong economy before military expansion.',
    speed_rusher: 'Early game aggression with rapid technology advancement and timing-based attacks. High-risk, high-reward playstyle.',
    tech_enthusiast: 'Research-focused with advanced technology priorities and innovative strategies. Scientific approach to warfare.',
    alliance_focused: 'Team player who supports allies with coordinated attacks and resource sharing. Strong diplomatic ties.',
    solo_survivor: 'Independent playstyle with self-sufficiency and defensive positioning. Minimal diplomatic engagement.'
};

const PERSONALITY_PRESETS = {
    aggressive_conqueror: { aggression: 90, economy: 30, military: 85, research: 40 },
    strategic_builder: { aggression: 40, economy: 80, military: 50, research: 70 },
    diplomatic_negotiator: { aggression: 20, economy: 60, military: 30, research: 50 },
    resource_hoarder: { aggression: 15, economy: 95, military: 25, research: 45 },
    speed_rusher: { aggression: 85, economy: 40, military: 70, research: 80 },
    tech_enthusiast: { aggression: 35, economy: 55, military: 45, research: 95 },
    alliance_focused: { aggression: 30, economy: 60, military: 55, research: 60 },
    solo_survivor: { aggression: 25, economy: 70, military: 60, research: 55 }
};

/**
 * Initialize bot management system
 */
document.addEventListener('DOMContentLoaded', () => {
    loadBots();
    setInterval(loadBots, 30000); // Refresh every 30 seconds
});

/**
 * Load all bots from API
 */
async function loadBots() {
    try {
        const token = localStorage.getItem('token');
        if (!token) {
            window.location.href = '../login.html';
            return;
        }

        const response = await fetch(`${API_BASE_URL}/api/admin/bots`, {
            headers: {
                'Authorization': `Bearer ${token}`
            }
        });

        if (!response.ok) {
            throw new Error('Failed to load bots');
        }

        const data = await response.json();
        allBots = data.bots || [];
        filteredBots = [...allBots];
        
        updateSummaryCards();
        renderBots();
    } catch (error) {
        console.error('Error loading bots:', error);
        showNotification('Failed to load bots', 'error');
    }
}

/**
 * Update summary statistics cards
 */
function updateSummaryCards() {
    const activeBots = allBots.filter(bot => bot.is_active).length;
    const totalAttacks = allBots.reduce((sum, bot) => sum + (bot.total_attacks_launched || 0), 0);
    const totalPlunder = allBots.reduce((sum, bot) => sum + (bot.total_resources_plundered || 0), 0);

    document.getElementById('totalBots').textContent = allBots.length;
    document.getElementById('activeBots').textContent = activeBots;
    document.getElementById('totalAttacks').textContent = totalAttacks.toLocaleString();
    document.getElementById('totalPlunder').textContent = formatNumber(totalPlunder);
}

/**
 * Render bot cards in grid
 */
function renderBots() {
    const container = document.getElementById('botContainer');
    
    if (filteredBots.length === 0) {
        container.innerHTML = '<div class="loading">No bots found</div>';
        return;
    }

    container.innerHTML = filteredBots.map(bot => createBotCard(bot)).join('');
}

/**
 * Create HTML for a single bot card
 */
function createBotCard(bot) {
    const statusClass = bot.is_active ? 'active' : 'inactive';
    const cardClass = bot.is_active ? '' : 'inactive';
    const winRate = bot.win_rate || 0;
    const lastAction = bot.last_action_at ? new Date(bot.last_action_at).toLocaleString() : 'Never';

    return `
        <div class="bot-card ${cardClass}">
            <div class="bot-header">
                <div class="bot-name">${escapeHtml(bot.username)}</div>
                <div class="bot-status ${statusClass}">${bot.is_active ? 'Active' : 'Inactive'}</div>
            </div>
            
            <div class="personality-badge">${formatPersonality(bot.personality_type)}</div>
            
            <div class="bot-stats">
                <div class="stat-row">
                    <span class="stat-label">Difficulty:</span>
                    <span class="stat-value">${bot.difficulty_level}/10</span>
                </div>
                <div class="stat-row">
                    <span class="stat-label">Win Rate:</span>
                    <span class="stat-value">${winRate.toFixed(1)}%</span>
                </div>
                <div class="stat-row">
                    <span class="stat-label">Attacks:</span>
                    <span class="stat-value">${bot.total_attacks_launched || 0}</span>
                </div>
                <div class="stat-row">
                    <span class="stat-label">Ships Built:</span>
                    <span class="stat-value">${bot.total_ships_built || 0}</span>
                </div>
                <div class="stat-row">
                    <span class="stat-label">Resources Plundered:</span>
                    <span class="stat-value">${formatNumber(bot.total_resources_plundered || 0)}</span>
                </div>
                <div class="stat-row">
                    <span class="stat-label">Last Action:</span>
                    <span class="stat-value" style="font-size: 0.85em;">${lastAction}</span>
                </div>
            </div>

            <div class="progress-bar">
                <div class="progress-fill" style="width: ${bot.aggression_level || 50}%">
                    Aggression: ${bot.aggression_level || 50}
                </div>
            </div>

            <div class="bot-actions">
                <button class="btn btn-primary" onclick="editBot(${bot.id})" title="Edit Bot">Edit</button>
                <button class="btn ${bot.is_active ? 'btn-danger' : 'btn-success'}" 
                        onclick="toggleBotStatus(${bot.id}, ${!bot.is_active})"
                        title="${bot.is_active ? 'Deactivate' : 'Activate'}">
                    ${bot.is_active ? 'Pause' : 'Activate'}
                </button>
                <button class="btn btn-primary" onclick="forceBotThink(${bot.id})" title="Force Think Cycle">Think</button>
                <button class="btn btn-danger" onclick="deleteBot(${bot.id})" title="Delete Bot">Delete</button>
            </div>
        </div>
    `;
}

/**
 * Show create bot modal
 */
function showCreateBotModal() {
    document.getElementById('modalTitle').textContent = 'Create New Bot';
    document.getElementById('botForm').reset();
    document.getElementById('botId').value = '';
    document.getElementById('botModal').style.display = 'block';
    
    // Reset sliders to default
    document.getElementById('difficulty').value = 5;
    document.getElementById('difficultyValue').textContent = '5';
    document.getElementById('aggression').value = 50;
    document.getElementById('aggressionValue').textContent = '50';
    document.getElementById('economy').value = 50;
    document.getElementById('economyValue').textContent = '50';
    document.getElementById('military').value = 50;
    document.getElementById('militaryValue').textContent = '50';
    document.getElementById('research').value = 50;
    document.getElementById('researchValue').textContent = '50';
}

/**
 * Edit existing bot
 */
async function editBot(botId) {
    const bot = allBots.find(b => b.id === botId);
    if (!bot) return;

    document.getElementById('modalTitle').textContent = 'Edit Bot';
    document.getElementById('botId').value = bot.id;
    document.getElementById('username').value = bot.username;
    document.getElementById('email').value = bot.email;
    document.getElementById('personality').value = bot.personality_type;
    document.getElementById('difficulty').value = bot.difficulty_level;
    document.getElementById('difficultyValue').textContent = bot.difficulty_level;
    document.getElementById('aggression').value = bot.aggression_level;
    document.getElementById('aggressionValue').textContent = bot.aggression_level;
    document.getElementById('economy').value = bot.economy_focus;
    document.getElementById('economyValue').textContent = bot.economy_focus;
    document.getElementById('military').value = bot.military_focus;
    document.getElementById('militaryValue').textContent = bot.military_focus;
    document.getElementById('research').value = bot.research_focus;
    document.getElementById('researchValue').textContent = bot.research_focus;
    document.getElementById('thinkInterval').value = bot.think_interval_minutes;
    
    updatePersonalityDescription();
    document.getElementById('botModal').style.display = 'block';
}

/**
 * Close bot modal
 */
function closeBotModal() {
    document.getElementById('botModal').style.display = 'none';
}

/**
 * Save bot (create or update)
 */
async function saveBotsubmit(event) {
    event.preventDefault();

    const botId = document.getElementById('botId').value;
    const botData = {
        username: document.getElementById('username').value,
        email: document.getElementById('email').value,
        personality_type: document.getElementById('personality').value,
        difficulty_level: parseInt(document.getElementById('difficulty').value),
        aggression_level: parseInt(document.getElementById('aggression').value),
        economy_focus: parseInt(document.getElementById('economy').value),
        military_focus: parseInt(document.getElementById('military').value),
        research_focus: parseInt(document.getElementById('research').value),
        think_interval_minutes: parseInt(document.getElementById('thinkInterval').value)
    };

    try {
        const token = localStorage.getItem('token');
        const url = botId 
            ? `${API_BASE_URL}/api/admin/bots/${botId}`
            : `${API_BASE_URL}/api/admin/bots`;
        
        const method = botId ? 'PUT' : 'POST';

        const response = await fetch(url, {
            method: method,
            headers: {
                'Authorization': `Bearer ${token}`,
                'Content-Type': 'application/json'
            },
            body: JSON.stringify(botData)
        });

        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.error || 'Failed to save bot');
        }

        showNotification(botId ? 'Bot updated successfully' : 'Bot created successfully', 'success');
        closeBotModal();
        loadBots();
    } catch (error) {
        console.error('Error saving bot:', error);
        showNotification(error.message, 'error');
    }
}

/**
 * Toggle bot active status
 */
async function toggleBotStatus(botId, newStatus) {
    try {
        const token = localStorage.getItem('token');
        const response = await fetch(`${API_BASE_URL}/api/admin/bots/${botId}`, {
            method: 'PUT',
            headers: {
                'Authorization': `Bearer ${token}`,
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({ is_active: newStatus })
        });

        if (!response.ok) {
            throw new Error('Failed to update bot status');
        }

        showNotification(`Bot ${newStatus ? 'activated' : 'deactivated'}`, 'success');
        loadBots();
    } catch (error) {
        console.error('Error toggling bot status:', error);
        showNotification('Failed to update bot status', 'error');
    }
}

/**
 * Delete bot
 */
async function deleteBot(botId) {
    if (!confirm('Are you sure you want to delete this bot? This action cannot be undone.')) {
        return;
    }

    try {
        const token = localStorage.getItem('token');
        const response = await fetch(`${API_BASE_URL}/api/admin/bots/${botId}`, {
            method: 'DELETE',
            headers: {
                'Authorization': `Bearer ${token}`
            }
        });

        if (!response.ok) {
            throw new Error('Failed to delete bot');
        }

        showNotification('Bot deleted successfully', 'success');
        loadBots();
    } catch (error) {
        console.error('Error deleting bot:', error);
        showNotification('Failed to delete bot', 'error');
    }
}

/**
 * Force bot to execute think cycle
 */
async function forceBotThink(botId) {
    try {
        const token = localStorage.getItem('token');
        const response = await fetch(`${API_BASE_URL}/api/admin/bots/${botId}/think`, {
            method: 'POST',
            headers: {
                'Authorization': `Bearer ${token}`
            }
        });

        if (!response.ok) {
            throw new Error('Failed to process bot');
        }

        const result = await response.json();
        showNotification(`Bot processed: ${result.actionsPerformed || 0} actions taken`, 'success');
        setTimeout(loadBots, 1000);
    } catch (error) {
        console.error('Error processing bot:', error);
        showNotification('Failed to process bot', 'error');
    }
}

/**
 * Process all active bots
 */
async function processAllBots() {
    try {
        const token = localStorage.getItem('token');
        const response = await fetch(`${API_BASE_URL}/api/admin/bots/process/all`, {
            method: 'POST',
            headers: {
                'Authorization': `Bearer ${token}`
            }
        });

        if (!response.ok) {
            throw new Error('Failed to process bots');
        }

        const result = await response.json();
        showNotification(`Processed ${result.processed || 0} bots`, 'success');
        setTimeout(loadBots, 2000);
    } catch (error) {
        console.error('Error processing all bots:', error);
        showNotification('Failed to process bots', 'error');
    }
}

/**
 * Activate all bots
 */
async function activateAllBots() {
    if (!confirm('Activate all bots?')) return;

    for (const bot of allBots) {
        if (!bot.is_active) {
            await toggleBotStatus(bot.id, true);
        }
    }
    loadBots();
}

/**
 * Deactivate all bots
 */
async function deactivateAllBots() {
    if (!confirm('Deactivate all bots?')) return;

    for (const bot of allBots) {
        if (bot.is_active) {
            await toggleBotStatus(bot.id, false);
        }
    }
    loadBots();
}

/**
 * Filter bots based on search criteria
 */
function filterBots() {
    const personality = document.getElementById('filterPersonality').value;
    const status = document.getElementById('filterStatus').value;
    const search = document.getElementById('searchUsername').value.toLowerCase();

    filteredBots = allBots.filter(bot => {
        const matchesPersonality = !personality || bot.personality_type === personality;
        const matchesStatus = !status || (status === 'active' && bot.is_active) || (status === 'inactive' && !bot.is_active);
        const matchesSearch = !search || bot.username.toLowerCase().includes(search);
        
        return matchesPersonality && matchesStatus && matchesSearch;
    });

    renderBots();
}

/**
 * Update personality description when selected
 */
function updatePersonalityDescription() {
    const personality = document.getElementById('personality').value;
    const descElement = document.getElementById('personalityDescription');
    
    if (personality && PERSONALITY_DESCRIPTIONS[personality]) {
        descElement.textContent = PERSONALITY_DESCRIPTIONS[personality];
        descElement.style.display = 'block';
        
        // Apply preset values
        const preset = PERSONALITY_PRESETS[personality];
        if (preset) {
            document.getElementById('aggression').value = preset.aggression;
            document.getElementById('aggressionValue').textContent = preset.aggression;
            document.getElementById('economy').value = preset.economy;
            document.getElementById('economyValue').textContent = preset.economy;
            document.getElementById('military').value = preset.military;
            document.getElementById('militaryValue').textContent = preset.military;
            document.getElementById('research').value = preset.research;
            document.getElementById('researchValue').textContent = preset.research;
        }
    } else {
        descElement.style.display = 'none';
    }
}

/**
 * Format personality type for display
 */
function formatPersonality(personality) {
    return personality.split('_').map(word => 
        word.charAt(0).toUpperCase() + word.slice(1)
    ).join(' ');
}

/**
 * Format large numbers
 */
function formatNumber(num) {
    if (num >= 1000000000) return (num / 1000000000).toFixed(2) + 'B';
    if (num >= 1000000) return (num / 1000000).toFixed(2) + 'M';
    if (num >= 1000) return (num / 1000).toFixed(2) + 'K';
    return num.toString();
}

/**
 * Escape HTML to prevent XSS
 */
function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

/**
 * Show notification message
 */
function showNotification(message, type = 'info') {
    const notification = document.createElement('div');
    notification.style.cssText = `
        position: fixed;
        top: 20px;
        right: 20px;
        padding: 15px 25px;
        background: ${type === 'success' ? 'linear-gradient(135deg, #4caf50, #388e3c)' : 
                      type === 'error' ? 'linear-gradient(135deg, #f44336, #d32f2f)' : 
                      'linear-gradient(135deg, #4a9eff, #2c5f7c)'};
        color: white;
        border-radius: 8px;
        box-shadow: 0 4px 12px rgba(0,0,0,0.3);
        z-index: 10000;
        animation: slideIn 0.3s ease;
    `;
    notification.textContent = message;
    document.body.appendChild(notification);

    setTimeout(() => {
        notification.style.animation = 'slideOut 0.3s ease';
        setTimeout(() => notification.remove(), 300);
    }, 3000);
}

// Close modal when clicking outside
window.onclick = function(event) {
    const modal = document.getElementById('botModal');
    if (event.target === modal) {
        closeBotModal();
    }
};
