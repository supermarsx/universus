export interface EmailJob {
  to: string;
  subject: string;
  html: string;
  text?: string;
  from?: string;
  metadata?: Record<string, any>;
  template?: string;
  context?: Record<string, any>;
  created_at?: string;
}

export interface EmailProviderConfig {
  email_provider: string;
  email_from_address: string;
  email_from_name: string;
  smtp_host?: string;
  smtp_port?: number;
  smtp_secure?: boolean;
  smtp_username?: string;
  smtp_password?: string;
  sendgrid_api_key?: string;
  ses_access_key?: string;
  ses_secret_key?: string;
  ses_region?: string;
  mailersend_api_key?: string;
  queue_enabled?: boolean;
}
