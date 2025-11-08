// @ts-nocheck
// Authentication Logic

function createBotProtectionClient() {
    let enabled = false;
    let hasFetched = false;
    let activeToken = null;
    let activeAnswer = null;
    let pendingFetch = null;
    let forcedMode = false;
    let lastFetchAt = 0;

    const fetchChallenge = async (force = false) => {
        try {
            const shouldForce = force || forcedMode;
            if (shouldForce) {
                forcedMode = true;
            }
            const query = shouldForce ? '?force=1' : '';
            pendingFetch = fetch(`/api/auth/bot-challenge${query}`)
                .then((res) => res.json())
                .then((data) => {
                    hasFetched = true;
                    lastFetchAt = Date.now();
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
                    if (data.forced) {
                        forcedMode = true;
                    }
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

        const stale = Date.now() - lastFetchAt > 10000;
        if (
            !hasFetched ||
            stale ||
            (enabled && (!activeToken || activeAnswer === null)) ||
            (!enabled && forcedMode)
        ) {
            await fetchChallenge(forcedMode);
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
        enableForcedMode: () => {
            forcedMode = true;
            return fetchChallenge(true);
        },
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
    const loginIdentifierInput = document.getElementById('loginUsername') || document.getElementById('loginEmail');
    const loginPasswordInput = document.getElementById('loginPassword');
    const registerUsernameInput = document.getElementById('registerUsername');
    const registerEmailInput = document.getElementById('registerEmail');
    const registerPasswordInput = document.getElementById('registerPassword');
    const registerConfirmInput = document.getElementById('registerPasswordConfirm') || document.getElementById('registerConfirmPassword');
    const loginError = document.getElementById('loginError');
    const loginInfo = document.getElementById('loginInfo');
    const registerError = document.getElementById('registerError');
    const registerInfo = document.getElementById('registerInfo');
    const resendVerificationButton = document.getElementById('resendVerificationButton');
    let pendingVerificationEmail = '';

    const setMessage = (element, message) => {
        if (!element) return;
        if (message) {
            element.textContent = message;
            element.style.display = 'block';
            element.classList.add('show');
        } else {
            element.textContent = '';
            element.style.display = 'none';
            element.classList.remove('show');
        }
    };

    const showLoginError = (message) => {
        setMessage(loginError, message);
        setMessage(loginInfo, '');
    };

    const setResendVisibility = (visible) => {
        if (!resendVerificationButton) return;
        resendVerificationButton.style.display = visible ? 'block' : 'none';
    };

    // Tab switching
    tabButtons.forEach(button => {
        button.addEventListener('click', () => {
            const tab = button.dataset.tab;
            tabButtons.forEach(btn => btn.classList.remove('active'));
            button.classList.add('active');
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

        const username = loginIdentifierInput ? loginIdentifierInput.value.trim() : '';
        const password = loginPasswordInput ? loginPasswordInput.value : '';
        setMessage(loginError, '');
        setMessage(loginInfo, '');
        setResendVisibility(false);
        pendingVerificationEmail = '';

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
                if (data.code === 'captcha_required') {
                    botProtection.enableForcedMode();
                }
                if (data.code === 'email_not_verified') {
                    pendingVerificationEmail = (data.email || username || '').trim();
                    setMessage(loginInfo, data.error || 'Email not verified. Check your inbox or resend the verification email.');
                    setResendVisibility(true);
                }
                throw new Error(data.error || 'Login failed');
            }

            localStorage.setItem('token', data.token);
            localStorage.setItem('user', JSON.stringify(data.user));
            window.location.href = '/overview.html';
        } catch (error) {
            showLoginError(error.message);
        }
    });

    // Register form submission
    registerForm.addEventListener('submit', async (e) => {
        e.preventDefault();

        const username = registerUsernameInput ? registerUsernameInput.value.trim() : '';
        const email = registerEmailInput ? registerEmailInput.value.trim() : '';
        const password = registerPasswordInput ? registerPasswordInput.value : '';
        const passwordConfirm = registerConfirmInput ? registerConfirmInput.value : '';

        setMessage(registerError, '');
        setMessage(registerInfo, '');

        if (password !== passwordConfirm) {
            setMessage(registerError, 'Passwords do not match');
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
                if (data.code === 'captcha_required') {
                    botProtection.enableForcedMode();
                }
                throw new Error(data.error || 'Registration failed');
            }

            setMessage(registerInfo, data.message || 'Account created. Check your email to verify before logging in.');
            if (registerForm) {
                registerForm.reset();
            }
            if (loginIdentifierInput) {
                loginIdentifierInput.value = email;
            }
            const loginTab = Array.from(tabButtons).find(btn => btn.dataset.tab === 'login');
            if (loginTab) {
                loginTab.click();
            }
        } catch (error) {
            setMessage(registerError, error.message);
        }
    });

    if (resendVerificationButton) {
        resendVerificationButton.addEventListener('click', async () => {
            if (!pendingVerificationEmail) {
                setMessage(loginInfo, 'Enter your email before requesting a new verification link.');
                return;
            }

            try {
                resendVerificationButton.setAttribute('disabled', 'true');
                const botPayload = await botProtection.preparePayload();
                const response = await fetch('/api/auth/resend-verification', {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json',
                    },
                    body: JSON.stringify({
                        email: pendingVerificationEmail,
                        bot_challenge: botPayload,
                    }),
                });
                const data = await response.json();
                if (!response.ok) {
                    if (data.code === 'captcha_required') {
                        botProtection.enableForcedMode();
                    }
                    throw new Error(data.error || 'Unable to resend verification email');
                }

                setMessage(loginInfo, data.message || 'Verification email sent. Check your inbox.');
                setMessage(loginError, '');
            } catch (error) {
                showLoginError(error.message);
            } finally {
                resendVerificationButton.removeAttribute('disabled');
            }
        });
    }
});
