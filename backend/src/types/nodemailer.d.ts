// Type declarations for nodemailer
declare module 'nodemailer' {
    export interface TransportOptions {
        host?: string;
        port?: number;
        secure?: boolean;
        auth?: {
            user: string | undefined;
            pass: string | undefined;
        };
    }

    export interface MailOptions {
        from: string;
        to: string;
        subject: string;
        html: string;
        text?: string;
    }

    export interface Transporter {
        sendMail(mailOptions: MailOptions): Promise<any>;
    }

    export function createTransport(options: TransportOptions): Transporter;
    export function createTestAccount(): Promise<any>;
    export function getTestMessageUrl(info: any): string;
}

declare module 'nodemailer/lib/mailer' {
    import { Transporter } from 'nodemailer';
    
    export = Transporter;
    
    export interface Options {
        from: string;
        to: string;
        subject: string;
        html: string;
        text?: string;
    }
}
