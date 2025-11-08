-- Migration 35: Gameplay difficulty factor configuration

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
    'gameplay.difficulty_factor',
    'Difficulty Factor',
    'Global gameplay difficulty multiplier (supports two decimal precision)',
    'number',
    '1.00',
    '1.00',
    0.10,
    5.00,
    50
FROM config_categories
WHERE category_name = 'gameplay'
ON CONFLICT (parameter_key) DO NOTHING;
