-- Migration 36: Bot protection toggle for auth flows

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
    'gameplay.bot_protection_enabled',
    'Auth Bot Protection',
    'Toggle to require the hidden captcha challenge for login and registration.',
    'boolean',
    'false',
    'false',
    NULL,
    NULL,
    60
FROM config_categories
WHERE category_name = 'gameplay'
ON CONFLICT (parameter_key) DO NOTHING;
