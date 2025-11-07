/**
 * Password Recovery Interface
 * Handles password reset request, validation, and completion
 */

class PasswordRecoveryManager {
    constructor() {
        this.api = '/api/account/password-recovery';
        this.currentStep = 1;
        this.resetEmail = '';
        this.cooldownTime = 60;
        this.cooldownTimer = null;
        this.init();
    }

    init() {
        this.attachEventListeners();
        this.checkUrlToken();
        this.loadActiveRequests();
    }

    /**
     * Attach event listeners
     */
    attachEventListeners() {
        // Request reset form
        const requestForm = document.getElementById('requestResetForm');
        if (requestForm) {
            requestForm.addEventListener('submit', (e) => {
                e.preventDefault();
                this.requestPasswordReset();
            });
        }

        // Resend button
        const resendBtn = document.getElementById('resendResetBtn');
        if (resendBtn) {
            resendBtn.addEventListener('click', () => this.resendResetLink());
        }

        // Back to request
        const backBtn = document.getElementById('backToRequestBtn');
        if (backBtn) {
            backBtn.addEventListener('click', () => this.goToStep(1));
        }

        // New password form
        const newPasswordForm = document.getElementById('newPasswordForm');
        if (newPasswordForm) {
            newPasswordForm.addEventListener('submit', (e) => {
                e.preventDefault();
                this.submitNewPassword();
            });
        }

        // Password strength checker
        const newPasswordInput = document.getElementById('newPassword');
        if (newPasswordInput) {
            newPasswordInput.addEventListener('input', (e) => {
                this.checkPasswordStrength(e.target.value);
            });
        }

        // Cancel all requests
        const cancelAllBtn = document.getElementById('cancelAllRequestsBtn');
        if (cancelAllBtn) {
            cancelAllBtn.addEventListener('click', () => this.cancelAllRequests());
        }
    }

    /**
     * Check if URL contains reset token
     */
    checkUrlToken() {
        const urlParams = new URLSearchParams(window.location.search);
        const token = urlParams.get('token');
        
        if (token) {
            this.validateResetToken(token);
        }
    }

    /**
     * Request password reset
     */
    async requestPasswordReset() {
        const email = document.getElementById('resetEmail').value.trim();

        if (!email) {
            this.showError('Please enter your email address');
            return;
        }

        if (!this.validateEmail(email)) {
            this.showError('Please enter a valid email address');
            return;
        }

        const btn = document.querySelector('#requestResetForm button[type="submit"]');
        const originalText = btn.innerHTML;
        btn.disabled = true;
        btn.innerHTML = '<span class="spinner"></span> Sending...';

        try {
            const response = await fetch(`${this.api}/initiate`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({ email })
            });

            if (!response.ok) {
                const error = await response.json();
                throw new Error(error.message || 'Failed to send reset link');
            }

            this.resetEmail = email;
            document.getElementById('sentEmailDisplay').textContent = email;
            this.goToStep(2);
            this.startCooldown();

        } catch (error) {
            console.error('Error requesting password reset:', error);
            this.showError(error.message);
            btn.disabled = false;
            btn.innerHTML = originalText;
        }
    }

    /**
     * Resend reset link
     */
    async resendResetLink() {
        if (!this.resetEmail) {
            this.showError('Email address not found');
            return;
        }

        const btn = document.getElementById('resendResetBtn');
        const originalText = btn.innerHTML;
        btn.disabled = true;
        btn.innerHTML = '<span class="spinner"></span> Sending...';

        try {
            const response = await fetch(`${this.api}/initiate`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({ email: this.resetEmail })
            });

            if (!response.ok) {
                const error = await response.json();
                throw new Error(error.message || 'Failed to resend reset link');
            }

            this.showSuccess('Reset link has been resent to your email');
            this.startCooldown();

        } catch (error) {
            console.error('Error resending reset link:', error);
            this.showError(error.message);
            btn.disabled = false;
            btn.innerHTML = originalText;
        }
    }

    /**
     * Validate reset token
     */
    async validateResetToken(token) {
        try {
            const response = await fetch(`${this.api}/validate`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({ token })
            });

            if (!response.ok) {
                const error = await response.json();
                throw new Error(error.message || 'Invalid or expired reset token');
            }

            const data = await response.json();
            document.getElementById('resetToken').value = token;
            this.goToStep(3);

        } catch (error) {
            console.error('Error validating reset token:', error);
            this.showError(error.message);
            this.goToStep(1);
        }
    }

    /**
     * Submit new password
     */
    async submitNewPassword() {
        const token = document.getElementById('resetToken').value;
        const newPassword = document.getElementById('newPassword').value;
        const confirmPassword = document.getElementById('confirmNewPassword').value;

        if (!token) {
            this.showError('Reset token not found');
            return;
        }

        if (!newPassword || !confirmPassword) {
            this.showError('Please fill in all fields');
            return;
        }

        if (newPassword !== confirmPassword) {
            this.showError('Passwords do not match');
            return;
        }

        if (!this.validatePasswordStrength(newPassword)) {
            this.showError('Password does not meet requirements');
            return;
        }

        const btn = document.querySelector('#newPasswordForm button[type="submit"]');
        const originalText = btn.innerHTML;
        btn.disabled = true;
        btn.innerHTML = '<span class="spinner"></span> Resetting...';

        try {
            const response = await fetch(`${this.api}/complete`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({ token, newPassword })
            });

            if (!response.ok) {
                const error = await response.json();
                throw new Error(error.message || 'Failed to reset password');
            }

            this.goToStep(4);

        } catch (error) {
            console.error('Error resetting password:', error);
            this.showError(error.message);
            btn.disabled = false;
            btn.innerHTML = originalText;
        }
    }

    /**
     * Load active reset requests
     */
    async loadActiveRequests() {
        const token = this.getToken();
        if (!token) return;

        try {
            const response = await fetch(`${this.api}/active`, {
                headers: {
                    'Authorization': `Bearer ${token}`
                }
            });

            if (!response.ok) return;

            const data = await response.json();
            if (data.requests && data.requests.length > 0) {
                this.displayActiveRequests(data.requests);
            }

        } catch (error) {
            console.error('Error loading active requests:', error);
        }
    }

    /**
     * Display active reset requests
     */
    displayActiveRequests(requests) {
        const card = document.getElementById('activeRequestsCard');
        const list = document.getElementById('activeRequestsList');

        if (requests.length === 0) {
            card.style.display = 'none';
            return;
        }

        card.style.display = 'block';
        list.innerHTML = requests.map(req => `
            <div class="request-item">
                <div class="request-info">
                    <strong>Requested:</strong> ${new Date(req.created_at).toLocaleString()}
                    <br>
                    <strong>Expires:</strong> ${new Date(req.expires_at).toLocaleString()}
                </div>
                <button class="btn btn-sm btn-danger" onclick="passwordRecovery.cancelRequest('${req.id}')">
                    Cancel
                </button>
            </div>
        `).join('');
    }

    /**
     * Cancel a single request
     */
    async cancelRequest(requestId) {
        try {
            const response = await fetch(`${this.api}/cancel`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${this.getToken()}`,
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({ requestId })
            });

            if (!response.ok) {
                throw new Error('Failed to cancel request');
            }

            this.showSuccess('Reset request cancelled');
            this.loadActiveRequests();

        } catch (error) {
            console.error('Error cancelling request:', error);
            this.showError(error.message);
        }
    }

    /**
     * Cancel all requests
     */
    async cancelAllRequests() {
        if (!confirm('Are you sure you want to cancel all active reset requests?')) {
            return;
        }

        try {
            const response = await fetch(`${this.api}/cancel-all`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${this.getToken()}`
                }
            });

            if (!response.ok) {
                throw new Error('Failed to cancel requests');
            }

            this.showSuccess('All reset requests cancelled');
            this.loadActiveRequests();

        } catch (error) {
            console.error('Error cancelling requests:', error);
            this.showError(error.message);
        }
    }

    /**
     * Check password strength
     */
    checkPasswordStrength(password) {
        const strengthDiv = document.getElementById('passwordStrength');
        const requirements = {
            length: password.length >= 8,
            uppercase: /[A-Z]/.test(password),
            lowercase: /[a-z]/.test(password),
            number: /[0-9]/.test(password),
            special: /[^A-Za-z0-9]/.test(password)
        };

        // Update requirement indicators
        for (const [key, met] of Object.entries(requirements)) {
            const reqElement = document.getElementById(`req-${key}`);
            if (reqElement) {
                reqElement.className = met ? 'met' : '';
            }
        }

        // Calculate strength
        const metCount = Object.values(requirements).filter(v => v).length;
        let strength = 'weak';
        let color = '#ef4444';

        if (metCount >= 5) {
            strength = 'strong';
            color = '#22c55e';
        } else if (metCount >= 3) {
            strength = 'medium';
            color = '#f59e0b';
        }

        strengthDiv.innerHTML = `
            <div class="strength-bar">
                <div class="strength-fill" style="width: ${metCount * 20}%; background: ${color};"></div>
            </div>
            <span class="strength-text">Password strength: <strong>${strength}</strong></span>
        `;
    }

    /**
     * Validate password strength
     */
    validatePasswordStrength(password) {
        return password.length >= 8 &&
               /[A-Z]/.test(password) &&
               /[a-z]/.test(password) &&
               /[0-9]/.test(password) &&
               /[^A-Za-z0-9]/.test(password);
    }

    /**
     * Navigate to step
     */
    goToStep(step) {
        this.currentStep = step;

        // Hide all cards
        document.getElementById('requestResetCard').style.display = 'none';
        document.getElementById('emailSentCard').style.display = 'none';
        document.getElementById('newPasswordCard').style.display = 'none';
        document.getElementById('successCard').style.display = 'none';

        // Show current card
        const cards = ['requestResetCard', 'emailSentCard', 'newPasswordCard', 'successCard'];
        document.getElementById(cards[step - 1]).style.display = 'block';

        // Update step indicators
        for (let i = 1; i <= 4; i++) {
            const stepEl = document.getElementById(`step${i}`);
            if (stepEl) {
                stepEl.setAttribute('data-active', i === step ? 'true' : 'false');
                stepEl.setAttribute('data-complete', i < step ? 'true' : 'false');
            }
        }
    }

    /**
     * Start cooldown timer
     */
    startCooldown() {
        let remaining = this.cooldownTime;
        const btn = document.getElementById('resendResetBtn');
        const cooldownDiv = document.getElementById('resendCooldown');
        const cooldownText = document.getElementById('resendCooldownText');

        btn.disabled = true;
        cooldownDiv.style.display = 'flex';

        this.cooldownTimer = setInterval(() => {
            remaining--;
            cooldownText.textContent = `Please wait ${remaining} seconds before resending...`;

            if (remaining <= 0) {
                clearInterval(this.cooldownTimer);
                btn.disabled = false;
                cooldownDiv.style.display = 'none';
            }
        }, 1000);
    }

    /**
     * Validate email format
     */
    validateEmail(email) {
        const re = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
        return re.test(email);
    }

    /**
     * Show success message
     */
    showSuccess(message) {
        if (window.toast) { window.toast.success(message); } else { window.toast ? window.toast.success(message) : alert(message); }
    }

    /**
     * Show error message
     */
    showError(message) {
        if (window.toast) { window.toast.error(message); } else { window.toast ? window.toast.error(message) : alert(`Error: ${message}`); }
    }

    /**
     * Get authentication token
     */
    getToken() {
        return localStorage.getItem('token') || '';
    }
}

// Initialize and expose globally
let passwordRecovery;
document.addEventListener('DOMContentLoaded', () => {
    passwordRecovery = new PasswordRecoveryManager();
});
