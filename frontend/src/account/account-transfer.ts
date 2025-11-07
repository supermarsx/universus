// @ts-nocheck
/**
 * Account Transfer Interface
 * Handles account ownership transfer between email addresses
 */

class AccountTransferManager {
    constructor() {
        this.api = '/api/account/transfer';
        this.transferToken = null;
        this.init();
    }

    init() {
        this.loadCurrentEmail();
        this.loadActiveTransfers();
        this.loadIncomingTransfers();
        this.loadTransferHistory();
        this.checkUrlToken();
        this.attachEventListeners();
    }

    /**
     * Attach event listeners
     */
    attachEventListeners() {
        // Initiate transfer form
        const initiateForm = document.getElementById('initiateTransferForm');
        if (initiateForm) {
            initiateForm.addEventListener('submit', (e) => {
                e.preventDefault();
                this.initiateTransfer();
            });
        }

        // Accept transfer form
        const acceptForm = document.getElementById('acceptTransferForm');
        if (acceptForm) {
            acceptForm.addEventListener('submit', (e) => {
                e.preventDefault();
                this.acceptTransfer();
            });
        }

        // Reject transfer button
        const rejectBtn = document.getElementById('rejectTransferBtn');
        if (rejectBtn) {
            rejectBtn.addEventListener('click', () => this.rejectTransfer());
        }

        // Modal close
        const closeModalBtn = document.getElementById('closeModalBtn');
        if (closeModalBtn) {
            closeModalBtn.addEventListener('click', () => this.closeModal());
        }
    }

    /**
     * Check if URL contains transfer token
     */
    checkUrlToken() {
        const urlParams = new URLSearchParams(window.location.search);
        const token = urlParams.get('token');
        
        if (token) {
            this.transferToken = token;
            this.loadTransferDetails(token);
        }
    }

    /**
     * Load current user email
     */
    async loadCurrentEmail() {
        try {
            const response = await fetch('/api/account/profile', {
                headers: {
                    'Authorization': `Bearer ${this.getToken()}`
                }
            });

            if (!response.ok) return;

            const data = await response.json();
            document.getElementById('currentEmail').value = data.email;

        } catch (error) {
            console.error('Error loading current email:', error);
        }
    }

    /**
     * Initiate account transfer
     */
    async initiateTransfer() {
        const newOwnerEmail = document.getElementById('newOwnerEmail').value.trim();
        const password = document.getElementById('transferPassword').value;
        const reason = document.getElementById('transferReason').value.trim();
        const confirmed = document.getElementById('confirmTransfer').checked;

        if (!newOwnerEmail || !password) {
            this.showError('Please fill in all required fields');
            return;
        }

        if (!this.validateEmail(newOwnerEmail)) {
            this.showError('Please enter a valid email address');
            return;
        }

        if (!confirmed) {
            this.showError('Please confirm you understand this action is irreversible');
            return;
        }

        const btn = document.querySelector('#initiateTransferForm button[type="submit"]');
        const originalText = btn.innerHTML;
        btn.disabled = true;
        btn.innerHTML = '<span class="spinner"></span> Initiating...';

        try {
            const response = await fetch(`${this.api}/initiate`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${this.getToken()}`,
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({
                    toEmail: newOwnerEmail,
                    password,
                    reason
                })
            });

            if (!response.ok) {
                const error = await response.json();
                throw new Error(error.message || 'Failed to initiate transfer');
            }

            const data = await response.json();
            this.showSuccess(
                'Transfer Initiated',
                `Transfer request has been sent to ${newOwnerEmail}. The new owner has 24 hours to accept.`
            );

            // Clear form
            document.getElementById('initiateTransferForm').reset();
            
            // Reload transfers
            setTimeout(() => {
                this.loadActiveTransfers();
            }, 1000);

        } catch (error) {
            console.error('Error initiating transfer:', error);
            this.showError(error.message);
            btn.disabled = false;
            btn.innerHTML = originalText;
        }
    }

    /**
     * Load active outgoing transfers
     */
    async loadActiveTransfers() {
        try {
            const response = await fetch(`${this.api}/active`, {
                headers: {
                    'Authorization': `Bearer ${this.getToken()}`
                }
            });

            if (!response.ok) return;

            const data = await response.json();
            if (data.transfers && data.transfers.length > 0) {
                this.displayActiveTransfers(data.transfers);
            }

        } catch (error) {
            console.error('Error loading active transfers:', error);
        }
    }

    /**
     * Display active transfers
     */
    displayActiveTransfers(transfers) {
        const card = document.getElementById('activeTransfersCard');
        const list = document.getElementById('transfersList');

        if (transfers.length === 0) {
            card.style.display = 'none';
            return;
        }

        card.style.display = 'block';
        list.innerHTML = transfers.map(transfer => `
            <div class="transfer-card">
                <div class="transfer-info">
                    <h4>Transfer to ${transfer.to_email}</h4>
                    <p><strong>Status:</strong> <span class="badge badge-warning">${transfer.status}</span></p>
                    <p><strong>Initiated:</strong> ${new Date(transfer.created_at).toLocaleString()}</p>
                    <p><strong>Expires:</strong> ${new Date(transfer.expires_at).toLocaleString()}</p>
                    ${transfer.reason ? `<p><strong>Reason:</strong> ${transfer.reason}</p>` : ''}
                </div>
                <div class="transfer-actions">
                    ${transfer.status === 'pending' ? 
                        `<button class="btn btn-danger-outline btn-sm" onclick="accountTransfer.cancelTransfer('${transfer.id}')">
                            Cancel Transfer
                        </button>` : ''
                    }
                </div>
            </div>
        `).join('');
    }

    /**
     * Load incoming transfers
     */
    async loadIncomingTransfers() {
        try {
            const response = await fetch(`${this.api}/incoming`, {
                headers: {
                    'Authorization': `Bearer ${this.getToken()}`
                }
            });

            if (!response.ok) return;

            const data = await response.json();
            if (data.transfers && data.transfers.length > 0) {
                this.displayIncomingTransfers(data.transfers);
            }

        } catch (error) {
            console.error('Error loading incoming transfers:', error);
        }
    }

    /**
     * Display incoming transfers
     */
    displayIncomingTransfers(transfers) {
        const card = document.getElementById('incomingTransfersCard');
        const list = document.getElementById('incomingTransfersList');

        if (transfers.length === 0) {
            card.style.display = 'none';
            return;
        }

        card.style.display = 'block';
        list.innerHTML = transfers.map(transfer => `
            <div class="transfer-card incoming">
                <div class="transfer-info">
                    <h4>Account from ${transfer.from_email}</h4>
                    <p><strong>Status:</strong> <span class="badge badge-info">${transfer.status}</span></p>
                    <p><strong>Received:</strong> ${new Date(transfer.created_at).toLocaleString()}</p>
                    <p><strong>Expires:</strong> ${new Date(transfer.expires_at).toLocaleString()}</p>
                    ${transfer.reason ? `<p><strong>Reason:</strong> ${transfer.reason}</p>` : ''}
                </div>
                <div class="transfer-actions">
                    <a href="?token=${transfer.verification_token}" class="btn btn-primary btn-sm">
                        Review Transfer
                    </a>
                </div>
            </div>
        `).join('');
    }

    /**
     * Load transfer details for acceptance
     */
    async loadTransferDetails(token) {
        try {
            const response = await fetch(`${this.api}/verify`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({ token })
            });

            if (!response.ok) {
                const error = await response.json();
                throw new Error(error.message || 'Invalid or expired transfer token');
            }

            const data = await response.json();
            this.displayTransferForAcceptance(data.transfer);

        } catch (error) {
            console.error('Error loading transfer details:', error);
            this.showError(error.message);
        }
    }

    /**
     * Display transfer for acceptance
     */
    displayTransferForAcceptance(transfer) {
        document.getElementById('initiateTransferCard').style.display = 'none';
        document.getElementById('acceptTransferCard').style.display = 'block';

        const detailsDiv = document.getElementById('transferDetails');
        detailsDiv.innerHTML = `
            <div class="transfer-summary">
                <h4>Transfer Details</h4>
                <p><strong>From:</strong> ${transfer.from_email}</p>
                <p><strong>To:</strong> ${transfer.to_email}</p>
                <p><strong>Initiated:</strong> ${new Date(transfer.created_at).toLocaleString()}</p>
                <p><strong>Expires:</strong> ${new Date(transfer.expires_at).toLocaleString()}</p>
                ${transfer.reason ? `<p><strong>Reason:</strong> ${transfer.reason}</p>` : ''}
            </div>
        `;
    }

    /**
     * Accept transfer
     */
    async acceptTransfer() {
        if (!this.transferToken) {
            this.showError('Transfer token not found');
            return;
        }

        const password = document.getElementById('acceptPassword').value;
        const confirmed = document.getElementById('confirmAccept').checked;

        if (!password) {
            this.showError('Please enter your password');
            return;
        }

        if (!confirmed) {
            this.showError('Please confirm you understand the consequences');
            return;
        }

        const btn = document.querySelector('#acceptTransferForm button[type="submit"]');
        const originalText = btn.innerHTML;
        btn.disabled = true;
        btn.innerHTML = '<span class="spinner"></span> Accepting...';

        try {
            const response = await fetch(`${this.api}/complete`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({
                    token: this.transferToken,
                    password
                })
            });

            if (!response.ok) {
                const error = await response.json();
                throw new Error(error.message || 'Failed to accept transfer');
            }

            const data = await response.json();
            this.showSuccess(
                'Transfer Complete',
                'Account transfer has been completed successfully. You are now the owner of this account. Please log in with your email address.'
            );

            // Redirect to login after 3 seconds
            setTimeout(() => {
                window.location.href = '/login';
            }, 3000);

        } catch (error) {
            console.error('Error accepting transfer:', error);
            this.showError(error.message);
            btn.disabled = false;
            btn.innerHTML = originalText;
        }
    }

    /**
     * Reject transfer
     */
    async rejectTransfer() {
        if (!confirm('Are you sure you want to reject this transfer?')) {
            return;
        }

        if (!this.transferToken) {
            this.showError('Transfer token not found');
            return;
        }

        try {
            const response = await fetch(`${this.api}/cancel`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({ token: this.transferToken })
            });

            if (!response.ok) {
                throw new Error('Failed to reject transfer');
            }

            this.showSuccess('Transfer Rejected', 'The transfer request has been rejected.');
            
            // Redirect after 2 seconds
            setTimeout(() => {
                window.location.href = window.location.pathname;
            }, 2000);

        } catch (error) {
            console.error('Error rejecting transfer:', error);
            this.showError(error.message);
        }
    }

    /**
     * Cancel transfer
     */
    async cancelTransfer(transferId) {
        if (!confirm('Are you sure you want to cancel this transfer?')) {
            return;
        }

        try {
            const response = await fetch(`${this.api}/cancel/${transferId}`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${this.getToken()}`
                }
            });

            if (!response.ok) {
                throw new Error('Failed to cancel transfer');
            }

            this.showSuccess('Transfer Cancelled', 'The transfer request has been cancelled.');
            this.loadActiveTransfers();

        } catch (error) {
            console.error('Error cancelling transfer:', error);
            this.showError(error.message);
        }
    }

    /**
     * Load transfer history
     */
    async loadTransferHistory() {
        try {
            const response = await fetch(`${this.api}/history`, {
                headers: {
                    'Authorization': `Bearer ${this.getToken()}`
                }
            });

            if (!response.ok) return;

            const data = await response.json();
            this.displayTransferHistory(data.history || []);

        } catch (error) {
            console.error('Error loading transfer history:', error);
        }
    }

    /**
     * Display transfer history
     */
    displayTransferHistory(history) {
        const historyDiv = document.getElementById('transferHistory');

        if (history.length === 0) {
            historyDiv.innerHTML = '<p class="empty-message">No transfer history available</p>';
            return;
        }

        historyDiv.innerHTML = history.map(item => `
            <div class="history-item">
                <div class="history-info">
                    <strong>${item.type === 'outgoing' ? 'Transferred to' : 'Received from'}:</strong>
                    ${item.type === 'outgoing' ? item.to_email : item.from_email}
                </div>
                <div class="history-meta">
                    <span class="badge badge-${this.getStatusClass(item.status)}">${item.status}</span>
                    <span class="history-date">${new Date(item.completed_at || item.created_at).toLocaleString()}</span>
                </div>
            </div>
        `).join('');
    }

    /**
     * Get status badge class
     */
    getStatusClass(status) {
        const classes = {
            'completed': 'success',
            'pending': 'warning',
            'cancelled': 'danger',
            'expired': 'secondary'
        };
        return classes[status] || 'default';
    }

    /**
     * Validate email format
     */
    validateEmail(email) {
        const re = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
        return re.test(email);
    }

    /**
     * Show success modal
     */
    showSuccess(title, message) {
        document.getElementById('modalTitle').textContent = title;
        document.getElementById('modalMessage').textContent = message;
        document.getElementById('successModal').style.display = 'flex';
    }

    /**
     * Show error message
     */
    showError(message) {
        if (window.toast) { window.toast.error(message); } else { window.toast ? window.toast.error(message) : alert(`Error: ${message}`); }
    }

    /**
     * Close modal
     */
    closeModal() {
        document.getElementById('successModal').style.display = 'none';
    }

    /**
     * Get authentication token
     */
    getToken() {
        return localStorage.getItem('token') || '';
    }
}

// Initialize and expose globally
let accountTransfer;
document.addEventListener('DOMContentLoaded', () => {
    accountTransfer = new AccountTransferManager();
});
