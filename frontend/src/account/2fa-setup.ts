// @ts-nocheck
// Two-Factor Authentication Setup JavaScript

(function() {
    'use strict';

    const API_BASE = '/api/account/2fa';
    let currentStep = 1;
    let setupData = {
        secret: null,
        qrCode: null,
        backupCodes: []
    };

    document.addEventListener('DOMContentLoaded', async () => {
        await check2FAStatus();
        setupCodeInput();
        setupEventListeners();
    });

    async function check2FAStatus() {
        try {
            const response = await fetch(`${API_BASE}/status`, {
                headers: { 'Authorization': `Bearer ${getAuthToken()}` }
            });

            if (!response.ok) throw new Error('Failed to check 2FA status');

            const data = await response.json();
            
            if (data.is_enabled) {
                showAlreadyEnabled(data);
            } else {
                showStep(1);
            }
        } catch (error) {
            console.error('Error checking 2FA status:', error);
            showStep(1);
        }
    }

    function showAlreadyEnabled(statusData) {
        document.getElementById('step-already-enabled').classList.remove('hidden');
        document.querySelector('.wizard-steps').style.display = 'none';
        
        const remaining = document.getElementById('backup-codes-remaining');
        remaining.textContent = `${statusData.backup_codes_remaining} codes`;
    }

    async function initiate2FASetup() {
        try {
            const response = await fetch(`${API_BASE}/setup`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${getAuthToken()}`,
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({ method: 'totp' })
            });

            if (!response.ok) throw new Error('Failed to setup 2FA');

            const data = await response.json();
            setupData.secret = data.secret;
            setupData.qrCode = data.qr_code;
            setupData.backupCodes = data.backup_codes;

            displayQRCode(data.qr_code);
            document.getElementById('secret-key-text').textContent = data.secret;
        } catch (error) {
            console.error('Error setting up 2FA:', error);
            alert('Failed to initialize 2FA setup. Please try again.');
        }
    }

    function displayQRCode(qrCodeDataURL) {
        const container = document.getElementById('qr-code-container');
        container.innerHTML = `<img src="${qrCodeDataURL}" alt="QR Code" class="qr-code-image" />`;
    }

    async function verifyAndEnable2FA() {
        const code = getCodeInput();
        if (!code || code.length !== 6) {
            showError('verify-error', 'Please enter a valid 6-digit code');
            return;
        }

        try {
            const response = await fetch(`${API_BASE}/verify`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${getAuthToken()}`,
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({ code })
            });

            if (!response.ok) {
                showError('verify-error', 'Invalid code. Please try again.');
                clearCodeInput();
                return;
            }

            hideError('verify-error');
            displayBackupCodes();
            nextStep();
        } catch (error) {
            console.error('Error verifying 2FA:', error);
            showError('verify-error', 'Verification failed. Please try again.');
        }
    }

    function displayBackupCodes() {
        const list = document.getElementById('backup-codes-list');
        list.innerHTML = setupData.backupCodes.map((code, index) => `
            <div class="backup-code-item">
                <span class="code-number">${index + 1}.</span>
                <code class="backup-code">${code}</code>
            </div>
        `).join('');
    }

    async function disable2FA() {
        const code = document.getElementById('disable-code').value;
        if (!code) {
            showError('disable-error', 'Please enter a code');
            return;
        }

        try {
            const response = await fetch(`${API_BASE}/disable`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${getAuthToken()}`,
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({ code })
            });

            if (!response.ok) {
                showError('disable-error', 'Invalid code');
                return;
            }

            alert('2FA has been disabled');
            window.location.reload();
        } catch (error) {
            console.error('Error disabling 2FA:', error);
            showError('disable-error', 'Failed to disable 2FA');
        }
    }

    async function regenerateBackupCodes() {
        const code = prompt('Enter your current 2FA code to regenerate backup codes:');
        if (!code) return;

        try {
            const response = await fetch(`${API_BASE}/backup-codes/regenerate`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${getAuthToken()}`,
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({ code })
            });

            if (!response.ok) throw new Error('Invalid code');

            const data = await response.json();
            alert(`New backup codes generated:\n\n${data.backup_codes.join('\n')}\n\nPlease save these codes securely.`);
            await check2FAStatus();
        } catch (error) {
            alert('Failed to regenerate backup codes. Invalid code or error occurred.');
        }
    }

    function setupCodeInput() {
        const inputs = document.querySelectorAll('.code-input');
        inputs.forEach((input, index) => {
            input.addEventListener('input', (e) => {
                if (e.target.value.length === 1 && index < inputs.length - 1) {
                    inputs[index + 1].focus();
                }
            });

            input.addEventListener('keydown', (e) => {
                if (e.key === 'Backspace' && !e.target.value && index > 0) {
                    inputs[index - 1].focus();
                }
            });

            input.addEventListener('paste', (e) => {
                e.preventDefault();
                const pastedData = e.clipboardData.getData('text').replace(/\D/g, '');
                const chars = pastedData.split('');
                inputs.forEach((input, i) => {
                    if (chars[i]) {
                        input.value = chars[i];
                    }
                });
                if (chars.length === 6) {
                    inputs[5].focus();
                }
            });
        });
    }

    function getCodeInput() {
        const inputs = document.querySelectorAll('.code-input');
        return Array.from(inputs).map(input => input.value).join('');
    }

    function clearCodeInput() {
        document.querySelectorAll('.code-input').forEach(input => {
            input.value = '';
        });
        document.querySelector('.code-input').focus();
    }

    function setupEventListeners() {
        document.getElementById('show-manual-key')?.addEventListener('click', () => {
            document.getElementById('manual-key-section').classList.toggle('hidden');
        });

        document.getElementById('copy-secret-btn')?.addEventListener('click', () => {
            const secretKey = document.getElementById('secret-key-text').textContent;
            navigator.clipboard.writeText(secretKey);
            alert('Secret key copied to clipboard!');
        });

        document.getElementById('verify-btn')?.addEventListener('click', verifyAndEnable2FA);

        document.getElementById('download-codes-btn')?.addEventListener('click', downloadBackupCodes);
        document.getElementById('print-codes-btn')?.addEventListener('click', printBackupCodes);
        document.getElementById('copy-codes-btn')?.addEventListener('click', copyAllBackupCodes);

        document.getElementById('disable-2fa-btn')?.addEventListener('click', () => {
            document.getElementById('disable-2fa-modal').classList.remove('hidden');
        });

        document.getElementById('disable-2fa-form')?.addEventListener('submit', (e) => {
            e.preventDefault();
            disable2FA();
        });

        document.getElementById('regenerate-codes-btn')?.addEventListener('click', regenerateBackupCodes);
    }

    function downloadBackupCodes() {
        const content = `Universus Space Empire - 2FA Backup Codes\nGenerated: ${new Date().toISOString()}\n\n${setupData.backupCodes.join('\n')}\n\nKeep these codes safe. Each can only be used once.`;
        const blob = new Blob([content], { type: 'text/plain' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = 'universus-2fa-backup-codes.txt';
        a.click();
        URL.revokeObjectURL(url);
    }

    function printBackupCodes() {
        const printWindow = window.open('', '', 'width=600,height=400');
        printWindow.document.write(`
            <html>
                <head>
                    <title>2FA Backup Codes</title>
                    <style>
                        body { font-family: monospace; padding: 20px; }
                        h1 { font-size: 18px; }
                        .code { margin: 10px 0; font-size: 16px; }
                    </style>
                </head>
                <body>
                    <h1>Universus Space Empire - 2FA Backup Codes</h1>
                    <p>Generated: ${formatDateTime(new Date())}</p>
                    ${setupData.backupCodes.map((code, i) => `<div class="code">${i + 1}. ${code}</div>`).join('')}
                    <p style="margin-top: 20px;">Keep these codes safe. Each can only be used once.</p>
                </body>
            </html>
        `);
        printWindow.document.close();
        printWindow.print();
    }

    function copyAllBackupCodes() {
        const text = setupData.backupCodes.join('\n');
        navigator.clipboard.writeText(text);
        alert('All backup codes copied to clipboard!');
    }

    function showStep(step) {
        // Hide all steps
        document.querySelectorAll('.wizard-content').forEach(content => {
            content.classList.add('hidden');
        });

        // Show target step
        document.getElementById(`step-${step}`).classList.remove('hidden');

        // Update progress
        document.querySelectorAll('.step').forEach((stepEl, index) => {
            if (index < step) {
                stepEl.classList.add('completed');
                stepEl.classList.remove('active');
            } else if (index === step - 1) {
                stepEl.classList.add('active');
                stepEl.classList.remove('completed');
            } else {
                stepEl.classList.remove('active', 'completed');
            }
        });

        currentStep = step;

        // Initialize step-specific actions
        if (step === 2 && !setupData.qrCode) {
            initiate2FASetup();
        }
    }

    function nextStep() {
        if (currentStep < 4) {
            showStep(currentStep + 1);
        }
    }

    function prevStep() {
        if (currentStep > 1) {
            showStep(currentStep - 1);
        }
    }

    function showError(elementId, message) {
        const errorEl = document.getElementById(elementId);
        errorEl.textContent = message;
        errorEl.classList.remove('hidden');
    }

    function hideError(elementId) {
        document.getElementById(elementId).classList.add('hidden');
    }

    function getAuthToken() {
        return localStorage.getItem('auth_token') || sessionStorage.getItem('auth_token');
    }

    function formatDateTime(value) {
        const date = value instanceof Date ? value : new Date(value);
        const locale = getLocale();
        if (typeof Intl !== 'undefined' && Intl.DateTimeFormat) {
            return new Intl.DateTimeFormat(locale, {
                year: 'numeric',
                month: 'short',
                day: 'numeric',
                hour: '2-digit',
                minute: '2-digit',
            }).format(date);
        }
        return date.toLocaleString();
    }

    function getLocale() {
        try {
            if (window.i18next && window.i18next.language) {
                return window.i18next.language;
            }
        } catch (error) {
            // ignore i18next access errors
        }
        try {
            const stored = localStorage.getItem('preferredLanguage');
            if (stored) return stored;
        } catch (error) {
            // ignore storage access errors
        }
        return navigator.language || 'en-US';
    }

    window.twoFASetup = {
        nextStep,
        prevStep,
        closeDisableModal: () => {
            document.getElementById('disable-2fa-modal').classList.add('hidden');
        }
    };
})();
