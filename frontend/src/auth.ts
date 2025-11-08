// @ts-nocheck
// Authentication Logic

function createBotProtectionClient() {
    let enabled = false;
    let hasFetched = false;
    let activeToken = null;
    let activeAnswer = null;
    let pendingFetch = null;

    const fetchChallenge = async () => {
        try {
            pendingFetch = fetch('/api/auth/bot-challenge')
                .then((res) => res.json())
                .then((data) => {
                    hasFetched = true;
                    enabled = !!data.enabled;
                    if (!enabled) {
                        activeToken = null;
                        activeAnswer = null;
                        return;
                    }

                    if (!data.token || !Array.isArray(data.operands)) {
                        throw new Error('Malformed bot challenge');
                    }

                    activeToken = data.token;
                    activeAnswer = data.operands
                        .map((value) => Number(value))
                        .filter((value) => !Number.isNaN(value))
                        .reduce((sum, value) => sum + value, 0);
                })
                .catch((error) => {
                    console.warn('Bot challenge request failed:', error);
                    enabled = false;
                    activeToken = null;
                    activeAnswer = null;
                })
                .finally(() => {
                    pendingFetch = null;
                });

            return await pendingFetch;
        } catch (error) {
            console.warn('Bot challenge fetch failed:', error);
            enabled = false;
            activeToken = null;
            activeAnswer = null;
        }
    };

    const ensureChallengeReady = async () => {
        if (pendingFetch) {
            await pendingFetch;
            return;
        }

        if (!hasFetched || (enabled && (!activeToken || activeAnswer === null))) {
            await fetchChallenge();
        }
    };

    const preparePayload = async () => {
        await ensureChallengeReady();

        if (!enabled) {
            return undefined;
        }

        if (!activeToken || typeof activeAnswer !== 'number') {
            throw new Error('Unable to initialize bot protection. Please refresh and try again.');
        }

        const payload = {
            token: activeToken,
            response: activeAnswer,
        };

        activeToken = null;
        activeAnswer = null;

        // Pre-fetch the next challenge without blocking the form submission
        fetchChallenge();

        return payload;
    };

    return {
        init: () => fetchChallenge(),
        preparePayload,
    };
}

document.addEventListener('DOMContentLoaded', () => {
    // Check if already logged in
    const token = localStorage.getItem('token');
    if (token) {
        window.location.href = '/overview.html';
        return;
    }

    const loginForm = document.getElementById('loginForm');
    const registerForm = document.getElementById('registerForm');
    const tabButtons = document.querySelectorAll('.tab-button');
    const botProtection = createBotProtectionClient();
    botProtection.init();

    // Tab switching
    tabButtons.forEach(button => {
        button.addEventListener('click', () => {
            const tab = button.dataset.tab;
            
            // Update active tab button
            tabButtons.forEach(btn => btn.classList.remove('active'));
            button.classList.add('active');

            // Show corresponding form
            if (tab === 'login') {
                loginForm.classList.add('active');
                registerForm.classList.remove('active');
            } else {
                registerForm.classList.add('active');
                loginForm.classList.remove('active');
            }
        });
    });

    // Login form submission
    loginForm.addEventListener('submit', async (e) => {
        e.preventDefault();
        
        const username = document.getElementById('loginUsername').value.trim();
        const password = document.getElementById('loginPassword').value;
        const errorElement = document.getElementById('loginError');

        errorElement.classList.remove('show');
        errorElement.textContent = '';

        try {
            const botPayload = await botProtection.preparePayload();
            const response = await fetch('/api/auth/login', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({
                    username,
                    password,
                    bot_challenge: botPayload,
                }),
            });

            const data = await response.json();

            if (!response.ok) {
                throw new Error(data.error || 'Login failed');
            }

            // Store token and user data
            localStorage.setItem('token', data.token);
            localStorage.setItem('user', JSON.stringify(data.user));

            // Redirect to game
            window.location.href = '/overview.html';
        } catch (error) {
            errorElement.textContent = error.message;
            errorElement.classList.add('show');
        }
    });

    // Register form submission
    registerForm.addEventListener('submit', async (e) => {
        e.preventDefault();
        
        const username = document.getElementById('registerUsername').value.trim();
        const email = document.getElementById('registerEmail').value.trim();
        const password = document.getElementById('registerPassword').value;
        const passwordConfirm = document.getElementById('registerPasswordConfirm').value;
        const errorElement = document.getElementById('registerError');

        errorElement.classList.remove('show');
        errorElement.textContent = '';

        // Validate passwords match
        if (password !== passwordConfirm) {
            errorElement.textContent = 'Passwords do not match';
            errorElement.classList.add('show');
            return;
        }

        try {
            const botPayload = await botProtection.preparePayload();
            const response = await fetch('/api/auth/register', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({
                    username,
                    email,
                    password,
                    bot_challenge: botPayload,
                }),
            });

            const data = await response.json();

            if (!response.ok) {
                throw new Error(data.error || 'Registration failed');
            }

            // Store token and user data
            localStorage.setItem('token', data.token);
            localStorage.setItem('user', JSON.stringify(data.user));

            // Redirect to game
            window.location.href = '/overview.html';
        } catch (error) {
            errorElement.textContent = error.message;
            errorElement.classList.add('show');
        }
    });
});
