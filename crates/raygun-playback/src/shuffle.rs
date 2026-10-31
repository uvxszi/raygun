/// Small seeded PRNG for shuffle order. Not cryptographic; it only needs to look random to a
/// listener and be reproducible in tests.
#[derive(Debug, Clone)]
pub(crate) struct XorShift(u64);

impl XorShift {
    pub(crate) fn new(seed: u64) -> Self {
        // xorshift gets stuck at zero.
        Self(seed.max(1))
    }

    fn next_u64(&mut self) -> u64 {
        let mut value = self.0;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.0 = value;
        value
    }

    /// Uniform-enough value in `0..bound`. `bound` must be at least 1.
    pub(crate) fn below(&mut self, bound: usize) -> usize {
        (self.next_u64() % bound as u64) as usize
    }

    /// Fisher-Yates shuffle.
    pub(crate) fn shuffle<T>(&mut self, items: &mut [T]) {
        for last in (1..items.len()).rev() {
            let pick = self.below(last + 1);
            items.swap(last, pick);
        }
    }
}
