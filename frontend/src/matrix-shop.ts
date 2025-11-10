// @ts-nocheck
// Matrix Shop JavaScript
// Handles all shop interactions, purchases, inventory management, and Matrix progression

const API_BASE = '/api/shop-enhanced';
let currentUser = null;
let currentItems = [];
let currentFilters = {
    category: 'all',
    rarity: [],
    priceRange: '',
    paymentMethod: [],
    matrixExclusive: false
};
let selectedItem = null;
let userInventory = [];
let matrixProgress = null;

// Initialize shop on page load
document.addEventListener('DOMContentLoaded', () => {
    initializeShop();
    setupEventListeners();
    loadMatrixProgress();
    loadUserCosmetics();
    loadPromotions();
    loadFlashSales();
});

// Initialize shop
async function initializeShop() {
    try {
        // Get user token from session
        const token = localStorage.getItem('token');
        if (!token) {
            window.location.href = '/';
            return;
        }

        // Load shop items
        await loadShopItems();
        
        // Auto-refresh every 30 seconds
        setInterval(() => {
            loadShopItems();
            loadPromotions();
            loadFlashSales();
        }, 30000);

    } catch (error) {
        console.error('Error initializing shop:', error);
        showNotification('Failed to load shop', 'error');
    }
}

// Setup event listeners
function setupEventListeners() {
    // Navigation buttons
    document.querySelectorAll('.nav-btn').forEach(btn => {
        btn.addEventListener('click', (e) => {
            // Remove active from all buttons
            document.querySelectorAll('.nav-btn').forEach(b => b.classList.remove('active'));
            e.target.classList.add('active');

            const category = e.target.dataset.category;
            const filter = e.target.dataset.filter;

            if (category) {
                currentFilters.category = category;
                currentFilters.matrixExclusive = false;
            } else if (filter === 'matrix') {
                currentFilters.matrixExclusive = true;
            }

            applyFilters();
        });
    });

    // Filter checkboxes
    document.querySelectorAll('.filter-checkbox input').forEach(checkbox => {
        checkbox.addEventListener('change', (e) => {
            const filterType = e.target.dataset.filter;
            const value = e.target.value;

            if (filterType === 'rarity') {
                if (e.target.checked) {
                    currentFilters.rarity.push(value);
                } else {
                    currentFilters.rarity = currentFilters.rarity.filter(r => r !== value);
                }
            } else if (filterType === 'payment') {
                if (e.target.checked) {
                    currentFilters.paymentMethod.push(value);
                } else {
                    currentFilters.paymentMethod = currentFilters.paymentMethod.filter(p => p !== value);
                }
            }

            applyFilters();
        });
    });

    // Price filter
    document.getElementById('priceFilter').addEventListener('change', (e) => {
        currentFilters.priceRange = e.target.value;
        applyFilters();
    });

    // Reset filters
    document.getElementById('resetFilters').addEventListener('click', () => {
        currentFilters = {
            category: 'all',
            rarity: [],
            priceRange: '',
            paymentMethod: [],
            matrixExclusive: false
        };

        // Reset UI
        document.querySelectorAll('.filter-checkbox input').forEach(cb => cb.checked = false);
        document.getElementById('priceFilter').value = '';
        document.querySelectorAll('.nav-btn').forEach(b => b.classList.remove('active'));
        document.querySelector('.nav-btn[data-category="all"]').classList.add('active');

        applyFilters();
    });

    // View toggle
    document.querySelectorAll('.view-btn').forEach(btn => {
        btn.addEventListener('click', (e) => {
            document.querySelectorAll('.view-btn').forEach(b => b.classList.remove('active'));
            e.target.classList.add('active');

            const view = e.target.dataset.view;
            const grid = document.getElementById('itemsGrid');

            if (view === 'list') {
                grid.classList.add('list-view');
                grid.querySelectorAll('.item-card').forEach(card => card.classList.add('list-view'));
            } else {
                grid.classList.remove('list-view');
                grid.querySelectorAll('.item-card').forEach(card => card.classList.remove('list-view'));
            }
        });
    });

    // Modal close buttons
    document.querySelectorAll('.modal-close').forEach(btn => {
        btn.addEventListener('click', () => {
            closeModals();
        });
    });

    // Close modal on outside click
    document.querySelectorAll('.modal').forEach(modal => {
        modal.addEventListener('click', (e) => {
            if (e.target === modal) {
                closeModals();
            }
        });
    });

    // Purchase buttons
    document.getElementById('purchaseUSD').addEventListener('click', () => {
        purchaseItem('usd');
    });

    document.getElementById('purchaseDM').addEventListener('click', () => {
        purchaseItem('dark_matter');
    });

    // Promo code
    document.getElementById('applyPromo').addEventListener('click', applyPromoCode);

    // Gift form
    document.getElementById('giftForm').addEventListener('submit', async (e) => {
        e.preventDefault();
        await sendGift();
    });
}

// Load shop items
async function loadShopItems() {
    try {
        const token = localStorage.getItem('token');
        const response = await fetch(`${API_BASE}/cosmetics`, {
            headers: {
                'Authorization': `Bearer ${token}`
            }
        });

        if (!response.ok) throw new Error('Failed to load items');

        const data = await response.json();
        currentItems = data.data || [];
        applyFilters();

    } catch (error) {
        console.error('Error loading items:', error);
        showNotification('Failed to load shop items', 'error');
    }
}

// Apply filters and render items
function applyFilters() {
    let filteredItems = [...currentItems];

    // Category filter
    if (currentFilters.category !== 'all') {
        filteredItems = filteredItems.filter(item => item.category === currentFilters.category);
    }

    // Matrix exclusive filter
    if (currentFilters.matrixExclusive) {
        filteredItems = filteredItems.filter(item => item.rarity === 'matrix_exclusive');
    }

    // Rarity filter
    if (currentFilters.rarity.length > 0) {
        filteredItems = filteredItems.filter(item => currentFilters.rarity.includes(item.rarity));
    }

    // Price range filter
    if (currentFilters.priceRange) {
        const [min, max] = currentFilters.priceRange.split('-').map(v => parseInt(v) || Infinity);
        filteredItems = filteredItems.filter(item => {
            const price = item.price_usd * 100;
            return price >= min && (max === Infinity || price <= max);
        });
    }

    // Payment method filter
    if (currentFilters.paymentMethod.length > 0) {
        filteredItems = filteredItems.filter(item => {
            return currentFilters.paymentMethod.some(method => {
                if (method === 'usd') return item.price_usd > 0;
                if (method === 'dark_matter') return item.price_dm > 0;
                return false;
            });
        });
    }

    renderItems(filteredItems);
}

// Render items in grid
function renderItems(items) {
    const grid = document.getElementById('itemsGrid');
    
    if (items.length === 0) {
        grid.innerHTML = `
            <div class="loading-matrix">
                <p>${i18n.t('matrixShop.noItemsFiltered', { defaultValue: 'No items found matching your filters' })}</p>
            </div>
        `;
        return;
    }

    grid.innerHTML = items.map(item => `
        <div class="item-card" data-item-id="${item.id}" onclick="showItemDetails(${item.id})">
            <img src="${item.image_url || '/images/placeholder-cosmetic.png'}" alt="${item.name}" class="item-image">
            <div class="item-header">
                <div class="item-name">${item.name}</div>
                <div class="rarity-badge ${item.rarity}">${item.rarity.replace('_', ' ')}</div>
            </div>
            <div class="item-description">${item.description}</div>
            <div class="item-footer">
                <div class="item-price">
                    ${item.price_usd > 0 ? `$${item.price_usd.toFixed(2)}` : ''}
                    ${item.price_dm > 0 ? `${item.price_dm} DM` : ''}
                </div>
                <div class="item-actions">
                    <button class="btn-quick-buy" onclick="event.stopPropagation(); quickBuy(${item.id})">${i18n.t('matrixShop.buyNow', { defaultValue: 'Buy Now' })}</button>
                </div>
            </div>
        </div>
    `).join('');
}

// Show item details modal
window.showItemDetails = async function(itemId) {
    const item = currentItems.find(i => i.id === itemId);
    if (!item) return;

        selectedItem = item;
 
    // Populate modal
    document.getElementById('modalImage').src = item.image_url || '/images/placeholder-cosmetic.png';
    document.getElementById('modalImage').alt = item.name;
    document.getElementById('modalTitle').textContent = item.name;
    document.getElementById('modalDescription').textContent = item.description;
    document.getElementById('modalRarity').textContent = item.rarity.replace('_', ' ');
    document.getElementById('modalRarity').className = `rarity-badge ${item.rarity}`;
 
    // Prices
    if (item.price_usd > 0) {
        document.getElementById('modalPriceUSD').textContent = `$${item.price_usd.toFixed(2)}`;
    }
 
    if (item.price_dm > 0) {
        document.getElementById('dmPriceOption').style.display = 'block';
        document.getElementById('modalPriceDM').textContent = `${item.price_dm} DM`;
        document.getElementById('purchaseDM').style.display = 'block';
    } else {
        document.getElementById('dmPriceOption').style.display = 'none';
        document.getElementById('purchaseDM').style.display = 'none';
    }
 
    // Stock info
    if (item.stock_quantity !== null && item.stock_quantity !== undefined) {
        document.getElementById('stockInfo').style.display = 'block';
        document.getElementById('stockText').textContent = i18n.t('matrixShop.stockRemaining', { defaultValue: 'Only {{count}} left in stock!', count: item.stock_quantity, interpolation: { escapeValue: false } }).replace('{{count}}', item.stock_quantity);
    } else {
        document.getElementById('stockInfo').style.display = 'none';
    }
 
    // Show modal
    document.getElementById('itemModal').classList.add('active');
};

// Quick buy function
window.quickBuy = function(itemId) {
    showItemDetails(itemId);
};

// Purchase item
async function purchaseItem(paymentMethod) {
    if (!selectedItem) return;

    try {
        const token = localStorage.getItem('token');
        const promoCode = document.getElementById('promoCode').value;

        const requestData = {
            cosmetic_id: selectedItem.id,
            payment_method: paymentMethod,
            promo_code: promoCode || undefined
        };

        const response = await fetch(`${API_BASE}/cosmetics/purchase`, {
            method: 'POST',
            headers: {
                'Authorization': `Bearer ${token}`,
                'Content-Type': 'application/json'
            },
            body: JSON.stringify(requestData)
        });

        const data = await response.json();

        if (!response.ok) {
            throw new Error(data.error?.message || 'Purchase failed');
        }

        // Handle Stripe payment
        if (paymentMethod === 'usd' && data.data.payment_url) {
            window.location.href = data.data.payment_url;
            return;
        }

        // Dark Matter purchase completed
        showNotification(`Successfully purchased ${selectedItem.name}!`, 'success');
        closeModals();
        
        // Reload inventory and items
        await loadUserCosmetics();
        await loadShopItems();

    } catch (error) {
        console.error('Purchase error:', error);
        showNotification(error.message || 'Purchase failed', 'error');
    }
}

// Apply promo code
async function applyPromoCode() {
    const promoCode = document.getElementById('promoCode').value;
    if (!promoCode) {
        showNotification('Please enter a promo code', 'warning');
        return;
    }

    try {
        const token = localStorage.getItem('token');
        const response = await fetch(`${API_BASE}/promotions/validate`, {
            method: 'POST',
            headers: {
                'Authorization': `Bearer ${token}`,
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({ code: promoCode })
        });

        const data = await response.json();

        if (!response.ok) {
            throw new Error(data.error?.message || 'Invalid promo code');
        }

        const discount = data.data.discount_percentage || data.data.discount_amount || 0;
        showNotification(`Promo code applied! ${discount}% discount`, 'success');

    } catch (error) {
        console.error('Promo code error:', error);
        showNotification(error.message || 'Invalid promo code', 'error');
    }
}

// Load user cosmetics (inventory)
async function loadUserCosmetics() {
    try {
        const token = localStorage.getItem('token');
        const response = await fetch(`${API_BASE}/my-cosmetics`, {
            headers: {
                'Authorization': `Bearer ${token}`
            }
        });

        if (!response.ok) throw new Error('Failed to load inventory');

        const data = await response.json();
        userInventory = data.data || [];
        renderInventory();

    } catch (error) {
        console.error('Error loading inventory:', error);
    }
}

// Render inventory
function renderInventory() {
    const grid = document.getElementById('inventoryGrid');
    
    if (userInventory.length === 0) {
        grid.innerHTML = `
            <p style="text-align: center; color: rgba(0,255,65,0.6);">${i18n.t('matrixShop.noCosmetics', { defaultValue: "You don't own any cosmetics yet" })}</p>`;
        return;
    }

        grid.innerHTML = userInventory.map(item => `
        <div class="inventory-item ${item.is_equipped ? 'equipped' : ''}">
            <img src="${item.cosmetic.image_url || '/images/placeholder-cosmetic.png'}" alt="${item.cosmetic.name}">
            <div class="inventory-info">
                <div class="item-name">${item.cosmetic.name}</div>
                <div class="rarity-badge ${item.cosmetic.rarity}">${item.cosmetic.rarity}</div>
            </div>
            <div class="inventory-actions">
                ${item.is_equipped 
                    ? `<button class="btn-unequip" onclick="unequipItem(${item.id})">${i18n.t('matrixShop.unequip', { defaultValue: 'Unequip' })}</button>`
                    : `<button class="btn-equip" onclick="equipItem(${item.id})">${i18n.t('matrixShop.equip', { defaultValue: 'Equip' })}</button>`
                }
                <button class="btn-gift" onclick="openGiftModal(${item.cosmetic_id})">${i18n.t('matrixShop.gift', { defaultValue: 'Gift' })}</button>
            </div>
        </div>
    `).join('');
}

// Equip item
window.equipItem = async function(userCosmeticId) {
    try {
        const token = localStorage.getItem('token');
        const response = await fetch(`${API_BASE}/cosmetics/equip`, {
            method: 'POST',
            headers: {
                'Authorization': `Bearer ${token}`,
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({ user_cosmetic_id: userCosmeticId })
        });

        if (!response.ok) throw new Error('Failed to equip item');

        showNotification('Item equipped successfully!', 'success');
        await loadUserCosmetics();

    } catch (error) {
        console.error('Equip error:', error);
        showNotification('Failed to equip item', 'error');
    }
};

// Unequip item
window.unequipItem = async function(userCosmeticId) {
    try {
        const token = localStorage.getItem('token');
        const response = await fetch(`${API_BASE}/cosmetics/equip`, {
            method: 'POST',
            headers: {
                'Authorization': `Bearer ${token}`,
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({ user_cosmetic_id: userCosmeticId, equip: false })
        });

        if (!response.ok) throw new Error('Failed to unequip item');

        showNotification('Item unequipped successfully!', 'success');
        await loadUserCosmetics();

    } catch (error) {
        console.error('Unequip error:', error);
        showNotification('Failed to unequip item', 'error');
    }
};

// Open gift modal
window.openGiftModal = function(cosmeticId) {
    selectedItem = currentItems.find(i => i.id === cosmeticId);
    document.getElementById('giftModal').classList.add('active');
};

// Send gift
async function sendGift() {
    const recipientEmail = document.getElementById('giftRecipient').value;
    const message = document.getElementById('giftMessage').value;

    if (!selectedItem || !recipientEmail) {
        showNotification('Please fill in all required fields', 'warning');
        return;
    }

    try {
        const token = localStorage.getItem('token');
        const response = await fetch(`${API_BASE}/gifts/send`, {
            method: 'POST',
            headers: {
                'Authorization': `Bearer ${token}`,
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({
                cosmetic_id: selectedItem.id,
                recipient_email: recipientEmail,
                message: message
            })
        });

        if (!response.ok) throw new Error('Failed to send gift');

        showNotification('Gift sent successfully!', 'success');
        closeModals();

        // Reset form
        document.getElementById('giftForm').reset();

    } catch (error) {
        console.error('Gift error:', error);
        showNotification('Failed to send gift', 'error');
    }
}

// Load Matrix progression
async function loadMatrixProgress() {
    try {
        const token = localStorage.getItem('token');
        const response = await fetch(`${API_BASE}/matrix/progress`, {
            headers: {
                'Authorization': `Bearer ${token}`
            }
        });

        if (!response.ok) throw new Error('Failed to load Matrix progress');

        const data = await response.json();
        matrixProgress = data.data;
        updateMatrixUI();

    } catch (error) {
        console.error('Matrix progress error:', error);
    }
}

// Update Matrix UI
function updateMatrixUI() {
    if (!matrixProgress) return;

    document.getElementById('matrixPoints').textContent = matrixProgress.matrix_points || 0;
    document.getElementById('matrixLevel').textContent = matrixProgress.matrix_level || 1;
    
    // Update progress bar
    const currentXP = matrixProgress.matrix_points || 0;
    const nextLevelXP = getNextLevelXP(matrixProgress.matrix_level || 1);
    const percentage = (currentXP / nextLevelXP) * 100;
    
    document.getElementById('progressBar').style.width = `${percentage}%`;
    document.getElementById('progressText').textContent = `${currentXP} / ${nextLevelXP} XP`;
}

// Get XP required for next level
function getNextLevelXP(currentLevel) {
    const baseXP = 1000;
    return baseXP * currentLevel;
}

// Load promotions
async function loadPromotions() {
    try {
        const token = localStorage.getItem('token');
        const response = await fetch(`${API_BASE}/promotions`, {
            headers: {
                'Authorization': `Bearer ${token}`
            }
        });

        if (!response.ok) return;

        const data = await response.json();
        const promotions = data.data || [];

        if (promotions.length > 0) {
            renderPromotions(promotions);
        }

    } catch (error) {
        console.error('Promotions error:', error);
    }
}

// Render promotions
function renderPromotions(promotions) {
    const section = document.getElementById('promotions');
    section.classList.add('active');

    section.innerHTML = `
        <h3 style="margin-bottom: 15px; color: var(--matrix-warning);">${i18n.t('matrixShop.activePromotions', { defaultValue: '🎉 Active Promotions' })}</h3>
        ${promotions.map(promo => `
            <div class="promo-banner">
                <div class="promo-info">
                    <h3>${promo.name}</h3>
                    <p style="margin: 5px 0;">${promo.description}</p>
                    <div class="promo-code">${i18n.t('matrixShop.promoCodeLabel', { defaultValue: 'Code:' })} ${promo.code}</div>
                </div>
                <div class="promo-timer">
                    <div>${i18n.t('matrixShop.expiresInLabel', { defaultValue: 'Expires in' })}</div>
                    <div class="timer-value" data-expires="${promo.end_date}">${getTimeRemaining(promo.end_date)}</div>
                </div>
            </div>
        `).join('')}
    `;

    // Update timers every second
    setInterval(updatePromotionTimers, 1000);
}

// Load flash sales
async function loadFlashSales() {
    try {
        const token = localStorage.getItem('token');
        const response = await fetch(`${API_BASE}/flash-sales`, {
            headers: {
                'Authorization': `Bearer ${token}`
            }
        });

        if (!response.ok) return;

        const data = await response.json();
        const flashSales = data.data || [];

        if (flashSales.length > 0) {
            renderFlashSales(flashSales);
        }

    } catch (error) {
        console.error('Flash sales error:', error);
    }
}

// Render flash sales
function renderFlashSales(sales) {
    const section = document.getElementById('flashSales');
    section.classList.add('active');

    section.innerHTML = `
        <h3 style="margin-bottom: 15px; color: var(--matrix-danger);">${i18n.t('matrixShop.flashSales', { defaultValue: '⚡ Flash Sales' })}</h3>
        ${sales.map(sale => `
            <div class="promo-banner">
                <div class="promo-info">
                    <h3>${sale.cosmetic.name}</h3>
                    <p style="margin: 5px 0;">${sale.discount_percentage}% OFF</p>
                    <div class="promo-code">${i18n.t('matrixShop.priceWasNow', { defaultValue: 'Was: ${{was}} → Now: ${{now}}', interpolation: { escapeValue: false } }).replace('{{was}}', sale.cosmetic.price_usd.toFixed(2)).replace('{{now}}', (sale.cosmetic.price_usd * (1 - sale.discount_percentage / 100)).toFixed(2))}</div>
                </div>
                <div class="promo-timer">
                    <div>${i18n.t('matrixShop.endsIn', { defaultValue: 'Ends in' })}</div>
                    <div class="timer-value" data-expires="${sale.end_date}">${getTimeRemaining(sale.end_date)}</div>
                </div>
            </div>
        `).join('')}
    `;

    setInterval(updatePromotionTimers, 1000);
}

// Update promotion timers
function updatePromotionTimers() {
    document.querySelectorAll('.timer-value').forEach(timer => {
        const expiresAt = timer.dataset.expires;
        timer.textContent = getTimeRemaining(expiresAt);
    });
}

// Get time remaining
function getTimeRemaining(endDate) {
    const now = new Date().getTime();
    const end = new Date(endDate).getTime();
    const diff = end - now;

    if (diff <= 0) return 'EXPIRED';

    const days = Math.floor(diff / (1000 * 60 * 60 * 24));
    const hours = Math.floor((diff % (1000 * 60 * 60 * 24)) / (1000 * 60 * 60));
    const minutes = Math.floor((diff % (1000 * 60 * 60)) / (1000 * 60));
    const seconds = Math.floor((diff % (1000 * 60)) / 1000);

    if (days > 0) return `${days}d ${hours}h`;
    if (hours > 0) return `${hours}h ${minutes}m`;
    return `${minutes}m ${seconds}s`;
}

// Close all modals
function closeModals() {
    document.querySelectorAll('.modal').forEach(modal => {
        modal.classList.remove('active');
    });
    selectedItem = null;
}

// Show notification
function showNotification(message, type = 'success') {
    const container = document.getElementById('notifications');
    const notification = document.createElement('div');
    notification.className = `notification ${type}`;
    notification.textContent = message;

    container.appendChild(notification);

    // Auto-remove after 5 seconds
    setTimeout(() => {
        notification.style.animation = 'slideOutRight 0.3s ease';
        setTimeout(() => notification.remove(), 300);
    }, 5000);
}

// Toggle inventory section
function toggleInventory() {
    const section = document.getElementById('inventorySection');
    section.style.display = section.style.display === 'none' ? 'block' : 'none';
}

// Export functions for global access
window.toggleInventory = toggleInventory;
