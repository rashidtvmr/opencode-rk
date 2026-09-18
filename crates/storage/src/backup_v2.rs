//! backup_v2: byte-supplied backup manifest, corruption detector, quotas.
//!
//! Caller supplies bytes; no FS IO here. Layout of an encoded manifest
//! (112 bytes, big-endian): magic `BKP2` (4) | schema_version u32 (4) |
//! session_count u64 | message_count u64 | blob_count u64 | total_bytes u64
//! (32) | data_hash SHA-256 (32) | integrity tag SHA-256 over the
//! preceding version+counts+hash body (32). Any tamper breaks the tag.
//!
//! Restore gate: [`VerifiedRestore`] has private fields and no other
//! constructor, so a caller can only obtain one via
//! [`BackupV2::verify_for_restore`]. Overwrite paths must take the token.
#![forbid(unsafe_code)]

/// Manifest schema version. Matches workspace format-2 lineage.
pub const BACKUP_SCHEMA_VERSION: u32 = 2;
/// Hard cap on backed-up payload bytes.
pub const MAX_BACKUP_BYTES: u64 = 64 * 1024 * 1024;
/// Hard cap on total backed-up items (sessions + messages + blobs).
pub const MAX_BACKUP_ITEMS: u64 = 4_000_000;

const MAGIC: [u8; 4] = *b"BKP2";
const ENCODED_LEN: usize = 4 + 4 + 8 * 4 + 32 + 32;

/// Manifest carried alongside opaque backup bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupManifest {
    pub schema_version: u32,
    pub session_count: u64,
    pub message_count: u64,
    pub blob_count: u64,
    pub total_bytes: u64,
    pub data_hash: [u8; 32],
}

/// Backup failure modes. Hash/tag mismatch surfaces as [`BackupError::Corrupt`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackupError {
    Corrupt,
    QuotaExceeded,
    UnsupportedVersion(u32),
}

impl std::fmt::Display for BackupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Corrupt => write!(f, "backup data corrupt"),
            Self::QuotaExceeded => write!(f, "backup quota exceeded"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported backup version {v}"),
        }
    }
}

impl std::error::Error for BackupError {}

/// Byte-supplied backup/restore helpers. Stateless; no IO.
pub struct BackupV2;

impl BackupV2 {
    /// Hash `data`, build its manifest, and return `(manifest, encoded)`.
    pub fn create(
        sessions: u64,
        messages: u64,
        blobs: u64,
        data: &[u8],
    ) -> Result<(BackupManifest, Vec<u8>), BackupError> {
        check_quotas(sessions, messages, blobs, data.len() as u64)?;
        let manifest = BackupManifest {
            schema_version: BACKUP_SCHEMA_VERSION,
            session_count: sessions,
            message_count: messages,
            blob_count: blobs,
            total_bytes: data.len() as u64,
            data_hash: sha256(data),
        };
        let encoded = Self::encode(&manifest);
        Ok((manifest, encoded))
    }

    /// Serialize a manifest to its 112-byte wire form.
    #[must_use]
    pub fn encode(manifest: &BackupManifest) -> Vec<u8> {
        let mut out = Vec::with_capacity(ENCODED_LEN);
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&manifest.schema_version.to_be_bytes());
        out.extend_from_slice(&manifest.session_count.to_be_bytes());
        out.extend_from_slice(&manifest.message_count.to_be_bytes());
        out.extend_from_slice(&manifest.blob_count.to_be_bytes());
        out.extend_from_slice(&manifest.total_bytes.to_be_bytes());
        out.extend_from_slice(&manifest.data_hash);
        let tag = sha256(&out[4..4 + 4 + 32 + 32]);
        out.extend_from_slice(&tag);
        out
    }

    /// Parse and authenticate an encoded manifest. Version gate runs before
    /// the tag check so a cleanly-encoded future version reports
    /// `UnsupportedVersion` instead of `Corrupt`.
    pub fn decode(bytes: &[u8]) -> Result<BackupManifest, BackupError> {
        if bytes.len() != ENCODED_LEN || bytes[..4] != MAGIC {
            return Err(BackupError::Corrupt);
        }
        let version = u32::from_be_bytes(bytes[4..8].try_into().map_err(|_| BackupError::Corrupt)?);
        if version != BACKUP_SCHEMA_VERSION {
            return Err(BackupError::UnsupportedVersion(version));
        }
        let tag = sha256(&bytes[4..ENCODED_LEN - 32]);
        if tag != bytes[ENCODED_LEN - 32..] {
            return Err(BackupError::Corrupt);
        }
        let u64_at = |off: usize| {
            u64::from_be_bytes(bytes[off..off + 8].try_into().map_err(|_| BackupError::Corrupt)?)
                .pipe_ok()
        };
        let manifest = BackupManifest {
            schema_version: version,
            session_count: u64_at(8)?,
            message_count: u64_at(16)?,
            blob_count: u64_at(24)?,
            total_bytes: u64_at(32)?,
            data_hash: bytes[40..72].try_into().map_err(|_| BackupError::Corrupt)?,
        };
        Ok(manifest)
    }

    /// Authenticate `data` against `manifest`: quotas, length, then hash.
    pub fn verify(manifest: &BackupManifest, data: &[u8]) -> Result<(), BackupError> {
        check_quotas(
            manifest.session_count,
            manifest.message_count,
            manifest.blob_count,
            manifest.total_bytes,
        )?;
        if data.len() as u64 != manifest.total_bytes {
            return Err(BackupError::Corrupt);
        }
        if sha256(data) != manifest.data_hash {
            return Err(BackupError::Corrupt);
        }
        Ok(())
    }

    /// Decode + fully verify, returning the only token that authorizes an
    /// overwriting restore. No unverified path yields a [`VerifiedRestore`].
    pub fn verify_for_restore<'a>(
        encoded: &[u8],
        data: &'a [u8],
    ) -> Result<VerifiedRestore<'a>, BackupError> {
        let manifest = Self::decode(encoded)?;
        Self::verify(&manifest, data)?;
        Ok(VerifiedRestore { manifest, data })
    }
}

/// Restore-after-verify marker. Fields private; constructible only via
/// [`BackupV2::verify_for_restore`]. Restore call sites must take this token
/// (not a raw manifest) before overwriting live state.
#[derive(Debug, PartialEq, Eq)]
pub struct VerifiedRestore<'a> {
    manifest: BackupManifest,
    data: &'a [u8],
}

impl<'a> VerifiedRestore<'a> {
    /// Authenticated manifest.
    #[must_use]
    pub fn manifest(&self) -> &BackupManifest {
        &self.manifest
    }

    /// Authenticated backup bytes.
    #[must_use]
    pub fn data(&self) -> &'a [u8] {
        self.data
    }
}

fn check_quotas(sessions: u64, messages: u64, blobs: u64, bytes: u64) -> Result<(), BackupError> {
    let items = sessions
        .saturating_add(messages)
        .saturating_add(blobs);
    if items > MAX_BACKUP_ITEMS || bytes > MAX_BACKUP_BYTES {
        return Err(BackupError::QuotaExceeded);
    }
    Ok(())
}

trait PipeOk: Sized {
    fn pipe_ok(self) -> Result<Self, BackupError> {
        Ok(self)
    }
}
impl PipeOk for u64 {}

/// Minimal SHA-256 (FIPS 180-4) over std only. Powers both the data hash and
/// the manifest integrity tag; no external digest dependency.
fn sha256(input: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = input.to_vec();
    let bit_len = (input.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for block in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, slot) in w.iter_mut().enumerate().take(16) {
            *slot = u32::from_be_bytes(block[4 * i..4 * i + 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[4 * i..4 * i + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_manifest() {
        let data = b"hello-backup-payload";
        let (manifest, encoded) = BackupV2::create(3, 100, 7, data).unwrap();
        assert_eq!(manifest.schema_version, BACKUP_SCHEMA_VERSION);
        assert_eq!(manifest.total_bytes, data.len() as u64);
        let decoded = BackupV2::decode(&encoded).unwrap();
        assert_eq!(decoded, manifest);
        BackupV2::verify(&manifest, data).unwrap();
        let token = BackupV2::verify_for_restore(&encoded, data).unwrap();
        assert_eq!(token.data(), data);
        assert_eq!(token.manifest(), &manifest);
    }

    #[test]
    fn tampered_manifest_bytes_detected_corrupt() {
        let data = b"pinned-image-bytes";
        let (_, encoded) = BackupV2::create(1, 2, 0, data).unwrap();
        let mut bad = encoded.clone();
        bad[10] ^= 0xFF;
        assert_eq!(BackupV2::decode(&bad), Err(BackupError::Corrupt));
        let mut bad_tag = encoded.clone();
        let n = bad_tag.len();
        bad_tag[n - 1] ^= 0x01;
        assert_eq!(BackupV2::decode(&bad_tag), Err(BackupError::Corrupt));
    }

    #[test]
    fn tampered_data_detected_corrupt() {
        let data = b"payload-one-payload-two";
        let (manifest, _) = BackupV2::create(1, 2, 0, data).unwrap();
        let mut bad = data.to_vec();
        bad[0] ^= 0xFF;
        assert_eq!(BackupV2::verify(&manifest, &bad), Err(BackupError::Corrupt));
    }

    #[test]
    fn over_quota_rejected() {
        let data = b"tiny bytes";
        assert_eq!(
            BackupV2::create(MAX_BACKUP_ITEMS, 1, 1, data),
            Err(BackupError::QuotaExceeded)
        );
        let manifest = BackupManifest {
            schema_version: BACKUP_SCHEMA_VERSION,
            session_count: 2,
            message_count: 3,
            blob_count: 0,
            total_bytes: MAX_BACKUP_BYTES + 1,
            data_hash: [0u8; 32],
        };
        let encoded = BackupV2::encode(&manifest);
        assert_eq!(
            BackupV2::verify(&manifest, data),
            Err(BackupError::QuotaExceeded)
        );
        assert_eq!(
            BackupV2::verify_for_restore(&encoded, data),
            Err(BackupError::QuotaExceeded)
        );
    }

    #[test]
    fn restore_requires_verify() {
        let data = [7u8; 64];
        let (_, encoded) = BackupV2::create(1, 1, 0, &data).unwrap();
        let mut bad = data.to_vec();
        bad.push(123u8);
        assert_eq!(
            BackupV2::verify_for_restore(&encoded, &bad),
            Err(BackupError::Corrupt)
        );
    }

    #[test]
    fn unsupported_version_rejected() {
        let manifest = BackupManifest {
            schema_version: 99,
            session_count: 0,
            message_count: 0,
            blob_count: 0,
            total_bytes: 0,
            data_hash: [0u8; 32],
        };
        let encoded = BackupV2::encode(&manifest);
        assert_eq!(
            BackupV2::decode(&encoded),
            Err(BackupError::UnsupportedVersion(99))
        );
    }

    #[test]
    fn sha256_known_vector() {
        let hex: String = sha256(b"abc").iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(
            hex,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
