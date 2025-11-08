-- Migration 37: Authentication security configuration

INSERT INTO config_parameters (
    category_id,
    parameter_key,
    parameter_name,
    description,
    data_type,
    current_value,
    default_value,
    min_value,
    max_value,
    sort_order
)
SELECT
    category_id,
    'gameplay.auth_rate_limit_window_seconds',
    'Auth Rate Limit Window (seconds)',
    'Number of seconds before failed auth attempts reset for throttling.',
    'number',
    '300',
    '300',
    60,
    3600,
    61
FROM config_categories
WHERE category_name = 'gameplay'
ON CONFLICT (parameter_key) DO NOTHING;

INSERT INTO config_parameters (
    category_id,
    parameter_key,
    parameter_name,
    description,
    data_type,
    current_value,
    default_value,
    min_value,
    max_value,
    sort_order
)
SELECT
    category_id,
    'gameplay.auth_rate_limit_max_attempts',
    'Auth Rate Limit - Max Attempts',
    'Maximum number of auth attempts allowed within the rate limit window.',
    'number',
    '10',
    '10',
    3,
    50,
    62
FROM config_categories
WHERE category_name = 'gameplay'
ON CONFLICT (parameter_key) DO NOTHING;

INSERT INTO config_parameters (
    category_id,
    parameter_key,
    parameter_name,
    description,
    data_type,
    current_value,
    default_value,
    min_value,
    max_value,
    sort_order
)
SELECT
    category_id,
    'gameplay.auth_captcha_failure_threshold',
    'Auth CAPTCHA Failure Threshold',
    'Number of consecutive failures before CAPTCHA is required even if disabled globally.',
    'number',
    '3',
    '3',
    1,
    10,
    63
FROM config_categories
WHERE category_name = 'gameplay'
ON CONFLICT (parameter_key) DO NOTHING;
