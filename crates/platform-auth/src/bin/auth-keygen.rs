#![forbid(unsafe_code)]

use ed25519_dalek::SigningKey;
use rand_core::OsRng;

const BASE64URL_CHARS: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn base64url_encode(data: &[u8]) -> String {
    let mut out = String::new();
    let mut index = 0;
    while index + 2 < data.len() {
        let value =
            ((data[index] as u32) << 16) | ((data[index + 1] as u32) << 8) | data[index + 2] as u32;
        out.push(BASE64URL_CHARS[((value >> 18) & 0x3f) as usize] as char);
        out.push(BASE64URL_CHARS[((value >> 12) & 0x3f) as usize] as char);
        out.push(BASE64URL_CHARS[((value >> 6) & 0x3f) as usize] as char);
        out.push(BASE64URL_CHARS[(value & 0x3f) as usize] as char);
        index += 3;
    }
    match data.len() - index {
        2 => {
            let value = ((data[index] as u32) << 16) | ((data[index + 1] as u32) << 8);
            out.push(BASE64URL_CHARS[((value >> 18) & 0x3f) as usize] as char);
            out.push(BASE64URL_CHARS[((value >> 12) & 0x3f) as usize] as char);
            out.push(BASE64URL_CHARS[((value >> 6) & 0x3f) as usize] as char);
        }
        1 => {
            let value = (data[index] as u32) << 16;
            out.push(BASE64URL_CHARS[((value >> 18) & 0x3f) as usize] as char);
            out.push(BASE64URL_CHARS[((value >> 12) & 0x3f) as usize] as char);
        }
        _ => {}
    }
    out
}

fn main() {
    let key_id = std::env::args()
        .nth(1)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "primary".to_string());
    let signing_key = SigningKey::generate(&mut OsRng);
    let private = base64url_encode(&signing_key.to_bytes());
    let public = base64url_encode(&signing_key.verifying_key().to_bytes());

    // This command is an intentional one-shot provisioning surface. Redirect
    // its stdout into a protected secret store; neither service logs nor
    // normal runtime diagnostics expose the private value.
    println!("AUTH_JWT_SIGNING_KEY_ID={key_id}");
    println!("AUTH_JWT_PRIVATE_KEY_BASE64={private}");
    println!("AUTH_JWT_VERIFICATION_KEYS={key_id}:{public}");
}
