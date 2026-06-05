// xorshift64 PRNG.

pub struct SeedRng {
    state: u64,
}

impl SeedRng {
    pub fn new(seed: u64) -> Self {
        let state = if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        };
        Self { state }
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    pub fn gen_range(&mut self, lo: usize, hi: usize) -> usize {
        if hi <= lo {
            return lo;
        }
        lo + (self.next_u64() as usize) % (hi - lo)
    }

    /// Pick an element from a slice, returning a reference to the original.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.gen_range(0, items.len())]
    }

    /// Pick a `&str` from a `&[&str]` slice. This is a tiny convenience
    /// helper to keep call sites short.
    pub fn pick_str<'a>(&mut self, items: &'a [&'a str]) -> &'a str {
        items[self.gen_range(0, items.len())]
    }

    pub fn bool(&mut self) -> bool {
        (self.next_u64() & 1) == 0
    }
}
