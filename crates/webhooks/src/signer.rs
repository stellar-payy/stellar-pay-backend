use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

// === Signing

pub fn sign_payload(secret: &str, payload: &[u8]) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts a key of any size");
    mac.update(payload);
    let result = mac.finalize().into_bytes();

    return hex::encode(result);
}

pub fn verify_signature(secret: &str, payload: &[u8], signature: &str) -> bool {
    let expected = sign_payload(secret, payload);
    return constant_time_eq(expected.as_bytes(), signature.as_bytes());
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }

    return diff == 0;
}

// === Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_a_signature_it_produced() {
        let signature = sign_payload("secret", b"payload");
        assert!(verify_signature("secret", b"payload", &signature));
    }

    #[test]
    fn rejects_a_tampered_payload() {
        let signature = sign_payload("secret", b"payload");
        assert!(!verify_signature("secret", b"tampered", &signature));
    }

    #[test]
    fn rejects_a_wrong_secret() {
        let signature = sign_payload("secret", b"payload");
        assert!(!verify_signature("other-secret", b"payload", &signature));
    }
}
