//! Loopback daemon discovery proof.  This is deliberately separate from
//! liveness: a listener must prove possession of the daemon credential.
use blake3::Hasher;

pub const NONCE_BYTES: usize = 32;
pub const MAC_HEX_LEN: usize = 64;

pub fn mac(token: &str, nonce: &[u8], origin: &str, schema: u16, pid: u32) -> Option<String> {
    let key = hex_decode(token)?;
    if key.len() != 32 || nonce.len() != NONCE_BYTES { return None; }
    let mut h = Hasher::new_keyed(key[..32].try_into().ok()?);
    h.update(b"opencode-rk/discovery-proof/v1\0");
    h.update(&(nonce.len() as u32).to_be_bytes()); h.update(nonce);
    h.update(&(origin.len() as u32).to_be_bytes()); h.update(origin.as_bytes());
    h.update(&schema.to_be_bytes()); h.update(&pid.to_be_bytes());
    Some(h.finalize().to_hex().to_string())
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 { return None; }
    let mut out = Vec::with_capacity(s.len()/2);
    for pair in s.as_bytes().chunks_exact(2) {
        let n = |b: u8| match b { b'0'..=b'9'=>Some(b-b'0'), b'a'..=b'f'=>Some(b-b'a'+10), b'A'..=b'F'=>Some(b-b'A'+10), _=>None };
        out.push(n(pair[0])?.checked_mul(16)?.checked_add(n(pair[1])?)?);
    }
    Some(out)
}

pub fn constant_time_hex_eq(a: &str, b: &str) -> bool {
    let mut d = a.len() ^ b.len();
    for i in 0..a.len().max(b.len()) { d |= usize::from(a.as_bytes().get(i).copied().unwrap_or(0) ^ b.as_bytes().get(i).copied().unwrap_or(0)); }
    d == 0
}

pub fn verify(token: &str, nonce: &[u8], origin: &str, schema: u16, pid: u32, presented: &str) -> bool {
    mac(token, nonce, origin, schema, pid).is_some_and(|expected| constant_time_hex_eq(&expected, presented))
}

#[cfg(test)]
mod tests {
    use super::*;
    const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    #[test]
    fn genuine_proof_accepts_and_wrong_nonce_origin_key_rejects() {
        let nonce = [7u8; NONCE_BYTES];
        let origin = "http://127.0.0.1:4096";
        let proof = mac(TOKEN, &nonce, origin, 1, 42).unwrap();
        assert!(verify(TOKEN, &nonce, origin, 1, 42, &proof));
        assert!(!verify(TOKEN, &[8u8; NONCE_BYTES], origin, 1, 42, &proof));
        assert!(!verify(TOKEN, &nonce, "http://127.0.0.1:4097", 1, 42, &proof));
        assert!(!verify("abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd", &nonce, origin, 1, 42, &proof));
    }
    #[test]
    fn malformed_nonce_and_oversized_proof_fail_closed() {
        assert!(mac(TOKEN, &[0u8; NONCE_BYTES - 1], "http://127.0.0.1:1", 1, 1).is_none());
        assert!(!verify(TOKEN, &[0u8; NONCE_BYTES], "http://127.0.0.1:1", 1, 1, &"a".repeat(MAC_HEX_LEN + 1)));
    }
}
