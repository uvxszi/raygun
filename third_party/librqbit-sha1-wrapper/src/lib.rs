// Adapted from librqbit-sha1-wrapper 9.1.0 (https://github.com/ikatson/rqbit, Apache-2.0).
// Raygun's copy hashes with `ring` whatever feature is chosen; see the workspace's
// `third_party/README.md`.

// Wrapper for sha1/sha256 libraries to be able to swap them easily,
// e.g. to measure performance, or change implementations depending on platform.
//
// Sha1 computation is the majority of CPU usage of librqbit.

pub trait ISha1 {
    fn new() -> Self;
    fn update(&mut self, buf: &[u8]);
    fn finish(self) -> [u8; 20];
}

/// SHA-256 hash trait for BEP 52 (BitTorrent v2) support.
pub trait ISha256 {
    fn new() -> Self;
    fn update(&mut self, buf: &[u8]);
    fn finish(self) -> [u8; 32];

    fn finish_id32(self) -> [u8; 32]
    where
        Self: Sized,
    {
        self.finish()
    }
}

mod ring_impl {
    use super::{ISha1, ISha256};

    use ring::digest::{Context, SHA1_FOR_LEGACY_USE_ONLY as SHA1, SHA256};

    pub struct Sha1Ring {
        ctx: Context,
    }

    impl ISha1 for Sha1Ring {
        fn new() -> Self {
            Self {
                ctx: Context::new(&SHA1),
            }
        }

        fn update(&mut self, buf: &[u8]) {
            self.ctx.update(buf);
        }

        fn finish(self) -> [u8; 20] {
            let result = self.ctx.finish();
            debug_assert_eq!(result.as_ref().len(), 20);
            let mut result_arr = [0u8; 20];
            result_arr.copy_from_slice(result.as_ref());
            result_arr
        }
    }

    pub struct Sha256Ring {
        ctx: Context,
    }

    impl ISha256 for Sha256Ring {
        fn new() -> Self {
            Self {
                ctx: Context::new(&SHA256),
            }
        }

        fn update(&mut self, buf: &[u8]) {
            self.ctx.update(buf);
        }

        fn finish(self) -> [u8; 32] {
            let result = self.ctx.finish();
            debug_assert_eq!(result.as_ref().len(), 32);
            let mut result_arr = [0u8; 32];
            result_arr.copy_from_slice(result.as_ref());
            result_arr
        }
    }
}

pub type Sha1 = ring_impl::Sha1Ring;

pub type Sha256 = ring_impl::Sha256Ring;

#[cfg(test)]
mod tests {
    use super::{ISha1, ISha256, Sha1, Sha256};

    #[test]
    fn test_sha256_known_vector_empty() {
        let mut h = Sha256::new();
        h.update(b"");
        let got = h.finish();
        let expected: [u8; 32] = [
            0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
            0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
            0x78, 0x52, 0xb8, 0x55,
        ];
        assert_eq!(got, expected);
    }

    #[test]
    fn test_sha1_known_vector_abc() {
        let mut h = Sha1::new();
        h.update(b"abc");
        let got = h.finish();
        let expected: [u8; 20] = [
            0xa9, 0x99, 0x3e, 0x36, 0x47, 0x06, 0x81, 0x6a, 0xba, 0x3e, 0x25, 0x71, 0x78, 0x50,
            0xc2, 0x6c, 0x9c, 0xd0, 0xd8, 0x9d,
        ];
        assert_eq!(got, expected);
    }
}
