-- Migration 38: Notification/email configuration & category

INSERT INTO config_categories (category_name, description, sort_order)
SELECT 'notifications', 'Email and notification delivery settings', 90
WHERE NOT EXISTS (
    SELECT 1 FROM config_categories WHERE category_name = 'notifications'
);

WITH category AS (
    SELECT category_id FROM config_categories WHERE category_name = 'notifications'
)
INSERT INTO config_parameters (
    category_id,
    parameter_key,
    parameter_name,
    description,
    data_type,
    current_value,
    default_value,
    sort_order
)
SELECT category_id, parameter_key, parameter_name, description, data_type, current_value, default_value, sort_order
FROM (
    VALUES
        ('notifications.email_provider', 'Email Provider', 'Primary email delivery provider (smtp, sendgrid, ses, mailersend)', 'string', 'smtp', 'smtp', 10),
        ('notifications.email_from_address', 'From Address', 'Default "from" email address for outbound mail', 'string', 'noreply@universus.game', 'noreply@universus.game', 11),
        ('notifications.email_from_name', 'From Name', 'Default "from" display name', 'string', 'Universus Command', 'Universus Command', 12),
        ('notifications.smtp_host', 'SMTP Host', 'SMTP server hostname', 'string', '', '', 20),
        ('notifications.smtp_port', 'SMTP Port', 'SMTP server port', 'number', '587', '587', 21),
        ('notifications.smtp_secure', 'SMTP Secure TLS', 'Use secure TLS connection for SMTP', 'boolean', 'true', 'true', 22),
        ('notifications.smtp_username', 'SMTP Username', 'SMTP authentication username', 'string', '', '', 23),
        ('notifications.smtp_password', 'SMTP Password', 'SMTP authentication password', 'string', '', '', 24),
        ('notifications.sendgrid_api_key', 'SendGrid API Key', 'API key for SendGrid', 'string', '', '', 30),
        ('notifications.ses_access_key', 'AWS SES Access Key', 'AWS access key for SES', 'string', '', '', 40),
        ('notifications.ses_secret_key', 'AWS SES Secret Key', 'AWS secret key for SES', 'string', '', '', 41),
        ('notifications.ses_region', 'AWS SES Region', 'AWS region for SES (e.g., us-east-1)', 'string', 'us-east-1', 'us-east-1', 42),
        ('notifications.mailersend_api_key', 'MailerSend API Key', 'API key for MailerSend', 'string', '', '', 50),
        ('notifications.queue_enabled', 'Email Queue Enabled', 'Whether to enqueue emails instead of sending inline', 'boolean', 'true', 'true', 60),
        ('notifications.templates', 'Email Templates', 'Localized email templates for transactional messages', 'json', 
            '{"verification":{"en":{"subject":"Verify Your Email","html":"<p>Hello {{username}}, verify your account using <a href=\\"{{verification_link}}\\">this link</a>.</p>","text":"Hello {{username}}, verify using {{verification_link}}."}},"password_reset":{"en":{"subject":"Reset Your Password","html":"<p>Reset your password using <a href=\\"{{reset_link}}\\">this link</a>.</p>","text":"Reset your password using {{reset_link}}."}},"account_transfer_request":{"en":{"subject":"Verify Account Transfer","html":"<p>A transfer from {{from_email}} was requested. Confirm at {{verify_link}}.</p>","text":"Transfer from {{from_email}}. Confirm: {{verify_link}}."}},"account_transfer_notification":{"en":{"subject":"Account Transfer Initiated","html":"<p>Your account transfer to {{to_email}} is pending confirmation.</p>","text":"Account transfer to {{to_email}} in progress."}},"two_factor_enabled":{"en":{"subject":"Two-Factor Authentication Enabled","html":"<p>2FA is now enabled on your account, {{username}}.</p>","text":"Two-factor authentication is enabled."}},"security_alert":{"en":{"subject":"Security Alert: {{alert_type}}","html":"<p>{{alert_details}}</p>","text":"{{alert_type}} - {{alert_details}}"}}}',
            '{"verification":{"en":{"subject":"Verify Your Email","html":"<p>Hello {{username}}, verify your account using <a href=\\"{{verification_link}}\\">this link</a>.</p>","text":"Hello {{username}}, verify using {{verification_link}}."}},"password_reset":{"en":{"subject":"Reset Your Password","html":"<p>Reset your password using <a href=\\"{{reset_link}}\\">this link</a>.</p>","text":"Reset your password using {{reset_link}}."}},"account_transfer_request":{"en":{"subject":"Verify Account Transfer","html":"<p>A transfer from {{from_email}} was requested. Confirm at {{verify_link}}.</p>","text":"Transfer from {{from_email}}. Confirm: {{verify_link}}."}},"account_transfer_notification":{"en":{"subject":"Account Transfer Initiated","html":"<p>Your account transfer to {{to_email}} is pending confirmation.</p>","text":"Account transfer to {{to_email}} in progress."}},"two_factor_enabled":{"en":{"subject":"Two-Factor Authentication Enabled","html":"<p>2FA is now enabled on your account, {{username}}.</p>","text":"Two-factor authentication is enabled."}},"security_alert":{"en":{"subject":"Security Alert: {{alert_type}}","html":"<p>{{alert_details}}</p>","text":"{{alert_type}} - {{alert_details}}"}}}',
            70)
) AS p(parameter_key, parameter_name, description, data_type, current_value, default_value, sort_order)
CROSS JOIN category
ON CONFLICT (parameter_key) DO NOTHING;
