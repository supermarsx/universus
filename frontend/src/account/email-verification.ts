// @ts-nocheck
/**
 * Email Verification Interface
 * Handles email verification, resend, and email change functionality
 */

class EmailVerificationManager {
    constructor() {
        this.api = '/api/account/email';
        this.cooldownTime = 60; // 60 seconds between resends
        this.cooldownTimer = null;
        this.init();
    }

    init() {
        this.loadVerificationStatus();
        this.attachEventListeners();
        this.checkUrlToken();
    }

    /**
     * Attach event listeners to UI elements
     */
    attachEventListeners() {
        // Send verification email
        const sendBtn = document.getElementById('sendVerificationBtn');
        if (sendBtn) {
            sendBtn.addEventListener('click', () => this.sendVerificationEmail());
        }

        // Resend verification email
        const resendBtn = document.getElementById('resendVerificationBtn');
        if (resendBtn) {
            resendBtn.addEventListener('click', () => this.resendVerificationEmail());
        }

        // Manual verification form
        const manualForm = document.getElementById('manualVerificationForm');
        if (manualForm) {
            manualForm.addEventListener('submit', (e) => {
                e.preventDefault();
                this.verifyEmailManually();
            });
        }

        // Change email form
        const changeEmailForm = document.getElementById('changeEmailForm');
        if (changeEmailForm) {
            changeEmailForm.addEventListener('submit', (e) => {
                e.preventDefault();
                this.changeEmail();
            });
        }

        // Modal close
        const closeModalBtn = document.getElementById('closeModalBtn');
        if (closeModalBtn) {
            closeModalBtn.addEventListener('click', () => this.closeModal());
        }
    }

    /**
     * Check if URL contains verification token
     */
    checkUrlToken() {
        const urlParams = new URLSearchParams(window.location.search);
        const token = urlParams.get('token');
        
        if (token) {
            this.verifyEmailWithToken(token);
        }
    }

    /**
     * Load current verification status
     */
    async loadVerificationStatus() {
        try {
            const response = await fetch(`${this.api}/status`, {
                headers: {
                    'Authorization': `Bearer ${this.getToken()}`
                }
            });

            if (!response.ok) {
                throw new Error('Failed to load verification status');
            }

            const data = await response.json();
            this.displayVerificationStatus(data);
        } catch (error) {
            console.error('Error loading verification status:', error);
            this.showError('Failed to load verification status');
        }
    }

    /**
     * Display verification status on the page
     */
    displayVerificationStatus(data) {
        const { email, verified, verificationSentAt, canResend } = data;

        // Update email display
        document.getElementById('currentEmail').textContent = email;

        // Update verification badge
        const badge = document.getElementById('verificationBadge');
        if (verified) {
            badge.innerHTML = '<span class="badge badge-success">Verified</span>';
        } else {
            badge.innerHTML = '<span class="badge badge-warning">Unverified</span>';
        }

        // Update status card
        const statusIcon = document.getElementById('statusIcon');
        const statusTitle = document.getElementById('statusTitle');
        const statusMessage = document.getElementById('statusMessage');

        if (verified) {
            statusIcon.classList.add('success');
            statusTitle.textContent = 'Email Verified';
            statusMessage.textContent = 'Your email address has been successfully verified.';
            
            // Hide verification actions
            document.getElementById('verificationActions').style.display = 'none';
        } else {
            statusTitle.textContent = 'Email Not Verified';
            statusMessage.textContent = 'Please verify your email address to unlock full account features.';

            // Show appropriate button
            if (verificationSentAt) {
                document.getElementById('sendVerificationBtn').style.display = 'none';
                document.getElementById('resendVerificationBtn').style.display = 'inline-flex';

                if (!canResend) {
                    this.startCooldown();
                }
            }
        }
    }

    /**
     * Send verification email
     */
    async sendVerificationEmail() {
        const btn = document.getElementById('sendVerificationBtn');
        const originalText = btn.innerHTML;
        btn.disabled = true;
        btn.innerHTML = '<span class="spinner"></span> Sending...';

        try {
            const response = await fetch(`${this.api}/send`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${this.getToken()}`,
                    'Content-Type': 'application/json'
                }
            });

            if (!response.ok) {
                const error = await response.json();
                throw new Error(error.message || 'Failed to send verification email');
            }

            const data = await response.json();
            this.showSuccess('Verification Email Sent', 'Please check your email for the verification link.');
            
            // Switch to resend button
            btn.style.display = 'none';
            document.getElementById('resendVerificationBtn').style.display = 'inline-flex';
            this.startCooldown();

        } catch (error) {
            console.error('Error sending verification email:', error);
            this.showError(error.message);
            btn.disabled = false;
            btn.innerHTML = originalText;
        }
    }

    /**
     * Resend verification email
     */
    async resendVerificationEmail() {
        const btn = document.getElementById('resendVerificationBtn');
        const originalText = btn.innerHTML;
        btn.disabled = true;
        btn.innerHTML = '<span class="spinner"></span> Sending...';

        try {
            const response = await fetch(`${this.api}/resend`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${this.getToken()}`,
                    'Content-Type': 'application/json'
                }
            });

            if (!response.ok) {
                const error = await response.json();
                throw new Error(error.message || 'Failed to resend verification email');
            }

            const data = await response.json();
            this.showSuccess('Verification Email Resent', 'Please check your email for the new verification link.');
            this.startCooldown();

        } catch (error) {
            console.error('Error resending verification email:', error);
            this.showError(error.message);
            btn.disabled = false;
            btn.innerHTML = originalText;
        }
    }

    /**
     * Verify email with token from URL
     */
    async verifyEmailWithToken(token) {
        try {
            const response = await fetch(`${this.api}/verify`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${this.getToken()}`,
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({ token })
            });

            if (!response.ok) {
                const error = await response.json();
                throw new Error(error.message || 'Verification failed');
            }

            this.showSuccess('Email Verified!', 'Your email address has been successfully verified.');
            
            // Reload status after 2 seconds
            setTimeout(() => {
                window.location.href = window.location.pathname;
            }, 2000);

        } catch (error) {
            console.error('Error verifying email:', error);
            this.showError(error.message);
        }
    }

    /**
     * Manually verify email with code
     */
    async verifyEmailManually() {
        const token = document.getElementById('verificationToken').value.trim();
        
        if (!token) {
            this.showError('Please enter a verification code');
            return;
        }

        await this.verifyEmailWithToken(token);
    }

    /**
     * Change email address
     */
    async changeEmail() {
        const newEmail = document.getElementById('newEmail').value.trim();
        const password = document.getElementById('confirmPassword').value;

        if (!newEmail || !password) {
            this.showError('Please fill in all fields');
            return;
        }

        if (!this.validateEmail(newEmail)) {
            this.showError('Please enter a valid email address');
            return;
        }

        try {
            const response = await fetch(`${this.api}/change`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${this.getToken()}`,
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({ newEmail, password })
            });

            if (!response.ok) {
                const error = await response.json();
                throw new Error(error.message || 'Failed to change email');
            }

            this.showSuccess(
                'Email Change Requested',
                'We\'ve sent a verification link to your new email address. Please verify it to complete the change.'
            );

            // Clear form
            document.getElementById('changeEmailForm').reset();

            // Reload status
            setTimeout(() => {
                this.loadVerificationStatus();
            }, 2000);

        } catch (error) {
            console.error('Error changing email:', error);
            this.showError(error.message);
        }
    }

    /**
     * Start cooldown timer
     */
    startCooldown() {
        let remaining = this.cooldownTime;
        const btn = document.getElementById('resendVerificationBtn');
        const cooldownMsg = document.getElementById('cooldownMessage');
        const cooldownText = document.getElementById('cooldownText');

        btn.disabled = true;
        cooldownMsg.style.display = 'flex';

        this.cooldownTimer = setInterval(() => {
            remaining--;
            cooldownText.textContent = `Please wait ${remaining} seconds before resending...`;

            if (remaining <= 0) {
                clearInterval(this.cooldownTimer);
                btn.disabled = false;
                cooldownMsg.style.display = 'none';
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

// Initialize on page load
document.addEventListener('DOMContentLoaded', () => {
    new EmailVerificationManager();
});
