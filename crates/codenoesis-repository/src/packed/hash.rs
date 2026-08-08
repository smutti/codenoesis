use sha1collisiondetection::{Builder, Sha1CD};

pub(super) struct CollisionHasher {
    inner: Sha1CD,
}

impl CollisionHasher {
    pub(super) fn new() -> Self {
        Self {
            inner: Builder::default().safe_hash(false).build(),
        }
    }

    pub(super) fn update(&mut self, bytes: &[u8]) {
        self.inner.update(bytes);
    }

    pub(super) fn finalize(self) -> Result<[u8; 20], ()> {
        let digest = self.inner.finalize_cd().map_err(|_| ())?;
        let mut bytes = [0_u8; 20];
        bytes.copy_from_slice(&digest);
        Ok(bytes)
    }
}

pub(super) fn collision_detecting_sha1(bytes: &[u8]) -> Result<[u8; 20], ()> {
    let mut hasher = CollisionHasher::new();
    hasher.update(bytes);
    hasher.finalize()
}

#[cfg(test)]
pub(super) fn reviewed_collision_vector() -> Vec<u8> {
    let hex = include_str!("../../../noesis/tests/evidence/s1/packed/vectors/sha-mbles-1.hex");
    let digits = hex
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();
    assert_eq!(digits.len(), 1_280);
    digits
        .chunks_exact(2)
        .map(|pair| {
            let high = decode_hex(pair[0]);
            let low = decode_hex(pair[1]);
            high << 4 | low
        })
        .collect()
}

#[cfg(test)]
fn decode_hex(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => panic!("reviewed collision vector is lowercase hexadecimal"),
    }
}

#[cfg(test)]
mod tests {
    use super::collision_detecting_sha1;
    use super::reviewed_collision_vector;

    #[test]
    fn conf_fr_acq_004_reviewed_sha1_collision_vector() {
        let vector = reviewed_collision_vector();
        assert_eq!(vector.len(), 640);
        assert!(collision_detecting_sha1(&vector).is_err());
    }
}
