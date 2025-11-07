// Type declarations for speakeasy
declare module 'speakeasy' {
    export interface GeneratedSecret {
        ascii: string;
        hex: string;
        base32: string;
        otpauth_url: string;
    }

    export interface TOTPOptions {
        secret: string;
        encoding?: 'base32' | 'ascii' | 'hex';
    }

    export interface TOTPVerifyOptions extends TOTPOptions {
        token: string;
        window?: number;
    }

    export function generateSecret(options?: {
        length?: number;
        name?: string;
        issuer?: string;
    }): GeneratedSecret;

    export namespace totp {
        function generate(options: TOTPOptions): string;
        function verify(options: TOTPVerifyOptions): boolean;
    }
}
