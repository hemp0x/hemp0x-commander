#![allow(dead_code)]

// Deterministic seeded PRNG (xorshift64) and helper combinators used by the
// corpus generator. We avoid pulling in `rand` here so the generator has no
// hidden state and is fully reproducible.

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

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.gen_range(0, items.len())]
    }

    pub fn pick_str<'a>(&mut self, items: &'a [&'a str]) -> &'a str {
        items[self.gen_range(0, items.len())]
    }

    /// Convenience: pick a `&str` from a `&[&str; N]` array (coerces).
    pub fn pick_str_arr<'a, const N: usize>(&mut self, items: &'a [&'a str; N]) -> &'a str {
        let slice: &'a [&'a str] = items.as_slice();
        self.pick_str(slice)
    }

    pub fn shuffle<T: Copy>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.gen_range(0, i + 1);
            items.swap(i, j);
        }
    }

    pub fn bool(&mut self) -> bool {
        (self.next_u64() & 1) == 0
    }

    pub fn maybe_punct(&mut self) -> &'static str {
        const PUNCT: &[&str] = &["", "!", ".", "?", "!!", "...", "!?", "?!", "."];
        let picked: &&str = self.pick(PUNCT);
        *picked
    }

    pub fn cap_first(s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            None => String::new(),
        }
    }

    pub fn join_words(parts: &[String]) -> String {
        parts.join(" ")
    }
}
