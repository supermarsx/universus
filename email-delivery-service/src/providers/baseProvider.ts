import { EmailJob, EmailProviderConfig } from '../types';

export interface EmailProvider {
  send(job: EmailJob, config: EmailProviderConfig): Promise<void>;
}
