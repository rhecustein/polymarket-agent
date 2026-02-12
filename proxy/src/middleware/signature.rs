use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Verify an HMAC-SHA256 signature against the expected secret.
///
/// The agent signs `agent_hash` with the shared HMAC secret, and sends
/// the resulting hex-encoded signature in the `signature` field.
/// This function recomputes the HMAC and compares in constant time.
pub fn verify_signature(hmac_secret: &str, agent_hash: &str, signature: &str) -> bool {
    let Ok(mut mac) = HmacSha256::new_from_slice(hmac_secret.as_bytes()) else {
        return false;
    };

    mac.update(agent_hash.as_bytes());

    let Ok(sig_bytes) = hex::decode(signature) else {
        return false;
    };

    mac.verify_slice(&sig_bytes).is_ok()
}

/// Compute an HMAC-SHA256 signature (for testing or internal use).
pub fn compute_signature(hmac_secret: &str, payload: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(hmac_secret.as_bytes())
        .expect("HMAC key error");
    mac.update(payload.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_verify_roundtrip() {
        let secret = "test_secret_key_12345";
        let payload = "agent_abc123";

        let sig = compute_signature(secret, payload);
        assert!(verify_signature(secret, payload, &sig));
    }

    #[test]
    fn test_invalid_signature() {
        let secret = "test_secret";
        assert!(!verify_signature(secret, "payload", "deadbeef"));
    }

    #[test]
    fn test_wrong_secret() {
        let sig = compute_signature("secret_a", "payload");
        assert!(!verify_signature("secret_b", "payload", &sig));
    }

    #[test]
    fn test_invalid_hex() {
        assert!(!verify_signature("secret", "payload", "not_hex_zzzz"));
    }
}
