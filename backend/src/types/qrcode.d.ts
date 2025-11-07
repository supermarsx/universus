// Type declarations for qrcode
declare module 'qrcode' {
    export interface QRCodeOptions {
        errorCorrectionLevel?: 'L' | 'M' | 'Q' | 'H';
        type?: 'image/png' | 'image/jpeg' | 'image/webp';
        quality?: number;
        margin?: number;
        width?: number;
        color?: {
            dark?: string;
            light?: string;
        };
    }

    export function toDataURL(text: string, options?: QRCodeOptions): Promise<string>;
    export function toString(text: string, options?: QRCodeOptions): Promise<string>;
    export function toFile(path: string, text: string, options?: QRCodeOptions): Promise<void>;
}
