// @ts-nocheck
// Shop page functionality with Stripe integration
class ShopManager {
    constructor() {
        this.catalog = [];
        this.activePerks = [];
        this.purchaseHistory = [];
        this.stripe = Stripe('pk_test_YOUR_PUBLISHABLE_KEY'); // Replace with actual key
        this.elements = null;
        this.currentPaymentIntent = null;
        
        this.init();
    }
    
    init() {
        this.setupEventListeners();
        this.loadShopData();
        this.loadActivePerks();
        this.loadPurchaseHistory();
        this.updateDarkMatter();
    }
    
    setupEventListeners() {
        const logoutBtn = document.getElementById('logout');
        if (logoutBtn) {
            logoutBtn.addEventListener('click', (e) => {
                e.preventDefault();
                localStorage.removeItem('token');
                window.location.href = 'index.html';
            });
        }
        
        // Tab switching
        document.querySelectorAll('.tab-btn').forEach(btn => {
            btn.addEventListener('click', () => {
                this.switchTab(btn.dataset.tab);
            });
        });
        
        // Modal close
        const closeModal = document.querySelector('.close-modal');
        if (closeModal) {
            closeModal.addEventListener('click', () => this.closePaymentModal());
        }
    }
    
    switchTab(tabName) {
        // Update tab buttons
        document.querySelectorAll('.tab-btn').forEach(btn => {
            btn.classList.toggle('active', btn.dataset.tab === tabName);
        });
        
        // Update tab content
        document.querySelectorAll('.tab-content').forEach(content => {
            content.classList.toggle('active', content.id === `${tabName}-tab`);
        });
    }
    
    async loadShopData() {
        try {
            const response = await api.get('/shop/catalog');
            this.catalog = response.data;
            this.renderShopItems();
        } catch (error) {
            console.error('Failed to load shop catalog:', error);
            this.showNotification('Failed to load shop items', 'error');
        }
    }
    
    renderShopItems() {
        // Group items by type
        const dmItems = this.catalog.filter(item => item.type === 'dark_matter');
        const resourceItems = this.catalog.filter(item => item.type === 'resource_pack');
        const officerItems = this.catalog.filter(item => item.type === 'officer');
        const boostItems = this.catalog.filter(item => item.type === 'boost');
        
        this.renderItemGrid('dm-items', dmItems);
        this.renderItemGrid('resource-items', resourceItems);
        this.renderItemGrid('officer-items', officerItems);
        this.renderItemGrid('boost-items', boostItems);
    }
    
    renderItemGrid(containerId, items) {
        const container = document.getElementById(containerId);
        if (!container) return;
        
        container.innerHTML = '';
        
        items.forEach(item => {
            const card = this.createShopItemCard(item);
            container.appendChild(card);
        });
    }
    
    createShopItemCard(item) {
        const card = document.createElement('div');
        card.className = 'shop-item-card';
        
        const priceUSD = (item.priceUSD / 100).toFixed(2);
        
        card.innerHTML = `
            <div class="item-header">
                <h3>${item.name}</h3>
                <div class="item-price">$${priceUSD}</div>
            </div>
            <div class="item-description">${item.description}</div>
            ${this.renderItemDetails(item)}
            <button class="btn btn-primary buy-btn" data-item-id="${item.id}">
                Purchase
            </button>
        `;
        
        const buyBtn = card.querySelector('.buy-btn');
        buyBtn.addEventListener('click', () => this.initiatePurchase(item));
        
        return card;
    }
    
    renderItemDetails(item) {
        if (item.darkMatterAmount) {
            return `<div class="item-bonus">+${this.formatNumber(item.darkMatterAmount)} DM</div>`;
        }
        
        if (item.resourceAmount) {
            const { metal, crystal, deuterium } = item.resourceAmount;
            return `
                <div class="item-resources">
                    ${metal ? `<div>Metal: ${this.formatNumber(metal)}</div>` : ''}
                    ${crystal ? `<div>Crystal: ${this.formatNumber(crystal)}</div>` : ''}
                    ${deuterium ? `<div>Deuterium: ${this.formatNumber(deuterium)}</div>` : ''}
                </div>
            `;
        }
        
        if (item.duration) {
            return `<div class="item-duration">Duration: ${item.duration} days</div>`;
        }
        
        return '';
    }
    
    async initiatePurchase(item) {
        try {
            // Create payment intent
            const response = await api.post('/shop/create-payment-intent', {
                shopItemId: item.id
            });
            
            this.currentPaymentIntent = response.data;
            
            // Show payment modal
            this.showPaymentModal(item);
            
            // Initialize Stripe Elements
            await this.initializeStripeElements(this.currentPaymentIntent.clientSecret);
            
        } catch (error) {
            console.error('Failed to initiate purchase:', error);
            this.showNotification('Failed to start purchase process', 'error');
        }
    }
    
    showPaymentModal(item) {
        const modal = document.getElementById('payment-modal');
        const detailsDiv = document.getElementById('payment-details');
        
        const priceUSD = (item.priceUSD / 100).toFixed(2);
        
        detailsDiv.innerHTML = `
            <div class="payment-item">
                <h3>${item.name}</h3>
                <p>${item.description}</p>
                <div class="payment-price">Total: $${priceUSD} USD</div>
            </div>
        `;
        
        modal.style.display = 'block';
    }
    
    closePaymentModal() {
        const modal = document.getElementById('payment-modal');
        modal.style.display = 'none';
        
        // Clear Stripe elements
        if (this.elements) {
            this.elements = null;
        }
    }
    
    async initializeStripeElements(clientSecret) {
        const appearance = {
            theme: 'night',
            variables: {
                colorPrimary: '#4a9eff',
                colorBackground: '#0f1322',
                colorText: '#e0e0e0',
                colorDanger: '#ef4444',
                fontFamily: 'Arial, sans-serif',
                spacingUnit: '4px',
                borderRadius: '5px',
            }
        };
        
        this.elements = this.stripe.elements({ appearance, clientSecret });
        
        const paymentElement = this.elements.create('payment');
        paymentElement.mount('#payment-element');
        
        // Setup payment submission
        const submitBtn = document.getElementById('submit-payment');
        submitBtn.onclick = () => this.handlePaymentSubmit();
    }
    
    async handlePaymentSubmit() {
        const submitBtn = document.getElementById('submit-payment');
        const statusDiv = document.getElementById('payment-status');
        
        submitBtn.disabled = true;
        statusDiv.innerHTML = '<div class="loading">Processing payment...</div>';
        
        try {
            const { error } = await this.stripe.confirmPayment({
                elements: this.elements,
                confirmParams: {
                    return_url: window.location.origin + '/shop.html?payment=success',
                },
            });
            
            if (error) {
                statusDiv.innerHTML = `<div class="error">${error.message}</div>`;
                submitBtn.disabled = false;
            }
        } catch (error) {
            console.error('Payment error:', error);
            statusDiv.innerHTML = '<div class="error">Payment failed. Please try again.</div>';
            submitBtn.disabled = false;
        }
    }
    
    async loadActivePerks() {
        try {
            const response = await api.get('/shop/perks');
            this.activePerks = response.data;
            this.renderActivePerks();
        } catch (error) {
            console.error('Failed to load active perks:', error);
        }
    }
    
    renderActivePerks() {
        const container = document.getElementById('active-perks');
        if (!container) return;
        
        if (this.activePerks.length === 0) {
            container.innerHTML = '<div class="empty-state">No active perks</div>';
            return;
        }
        
        container.innerHTML = '';
        
        this.activePerks.forEach(perk => {
            const perkCard = document.createElement('div');
            perkCard.className = 'perk-card';
            
            const expiresAt = new Date(perk.expiresAt);
            const now = new Date();
            const daysLeft = Math.ceil((expiresAt - now) / (1000 * 60 * 60 * 24));
            
            perkCard.innerHTML = `
                <div class="perk-header">
                    <h3>${this.formatPerkName(perk.perkType)}</h3>
                    <div class="perk-type">${perk.type}</div>
                </div>
                <div class="perk-expiry">
                    Expires in: <strong>${daysLeft} days</strong>
                </div>
                <div class="perk-status ${perk.isActive ? 'active' : 'inactive'}">
                    ${perk.isActive ? 'Active' : 'Expired'}
                </div>
            `;
            
            container.appendChild(perkCard);
        });
    }
    
    async loadPurchaseHistory() {
        try {
            const response = await api.get('/shop/purchases?limit=20');
            this.purchaseHistory = response.data;
            this.renderPurchaseHistory();
        } catch (error) {
            console.error('Failed to load purchase history:', error);
        }
    }
    
    renderPurchaseHistory() {
        const container = document.getElementById('purchase-history');
        if (!container) return;
        
        if (this.purchaseHistory.length === 0) {
            container.innerHTML = '<div class="empty-state">No purchases yet</div>';
            return;
        }
        
        container.innerHTML = '';
        
        this.purchaseHistory.forEach(purchase => {
            const purchaseCard = document.createElement('div');
            purchaseCard.className = 'purchase-card';
            
            const date = new Date(purchase.createdAt).toLocaleDateString();
            const amount = (purchase.amount / 100).toFixed(2);
            
            purchaseCard.innerHTML = `
                <div class="purchase-header">
                    <div class="purchase-item">${purchase.shopItemId}</div>
                    <div class="purchase-amount">$${amount}</div>
                </div>
                <div class="purchase-info">
                    <span class="purchase-date">${date}</span>
                    <span class="purchase-status status-${purchase.status}">${purchase.status}</span>
                </div>
            `;
            
            container.appendChild(purchaseCard);
        });
    }
    
    async updateDarkMatter() {
        try {
            const response = await api.get('/users/me');
            const darkMatter = response.data.dark_matter || 0;
            
            const dmDisplay = document.getElementById('dark-matter');
            if (dmDisplay) {
                dmDisplay.textContent = this.formatNumber(darkMatter);
            }
        } catch (error) {
            console.error('Failed to load dark matter:', error);
        }
    }
    
    formatPerkName(perkType) {
        return perkType
            .split('_')
            .map(word => word.charAt(0).toUpperCase() + word.slice(1))
            .join(' ');
    }
    
    formatNumber(num) {
        return new Intl.NumberFormat('en-US').format(Math.floor(num));
    }
    
    showNotification(message, type = 'info') {
        const notification = document.getElementById('notification');
        if (!notification) return;
        
        notification.textContent = message;
        notification.className = `notification ${type} show`;
        
        setTimeout(() => {
            notification.classList.remove('show');
        }, 3000);
    }
}

// Initialize when page loads
document.addEventListener('DOMContentLoaded', () => {
    // Check for payment success redirect
    const urlParams = new URLSearchParams(window.location.search);
    if (urlParams.get('payment') === 'success') {
        const notification = document.getElementById('notification');
        if (notification) {
            notification.textContent = 'Payment successful! Your purchase has been processed.';
            notification.className = 'notification success show';
            
            setTimeout(() => {
                notification.classList.remove('show');
                // Remove query parameter
                window.history.replaceState({}, document.title, '/shop.html');
            }, 5000);
        }
    }
    
    new ShopManager();
});
