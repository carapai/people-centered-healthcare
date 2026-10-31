//! A fixed PRNG, so a seed gives the same data forever: `rand` does not promise value stability
//! across versions, and reproducibility is this tool's contract.

/// xoshiro256** seeded through SplitMix64 (Blackman and Vigna).
#[derive(Debug, Clone)]
pub struct Rng {
    state: [u64; 4],
}

impl Rng {
    /// The generator for `seed`. Different `stream`s from one seed are independent sequences.
    pub fn new(seed: u64, stream: u64) -> Self {
        let mut x = seed ^ stream.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let mut next = || {
            x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = x;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        };
        Self {
            state: [next(), next(), next(), next()],
        }
    }

    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        let [s0, s1, s2, s3] = &mut self.state;
        let result = s1.wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = *s1 << 17;
        *s2 ^= *s0;
        *s3 ^= *s1;
        *s1 ^= *s2;
        *s0 ^= *s3;
        *s2 ^= t;
        *s3 = s3.rotate_left(45);
        result
    }

    /// Uniform in `[0, 1)`.
    pub fn unit(&mut self) -> f64 {
        // 53 random bits; the conversions are exact.
        #[allow(clippy::cast_precision_loss)]
        let value = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        value
    }

    /// Uniform in `0..n` (`n > 0`), without modulo bias.
    pub fn below(&mut self, n: u64) -> u64 {
        let zone = u64::MAX - u64::MAX % n;
        loop {
            let value = self.next_u64();
            if value < zone {
                return value % n;
            }
        }
    }

    /// Uniform in `low..=high`.
    pub fn between(&mut self, low: u32, high: u32) -> u32 {
        let span = u64::from(high - low) + 1;
        low + u32::try_from(self.below(span)).unwrap_or(0)
    }

    /// True with probability `p`.
    pub fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }

    /// A uniformly chosen element of a non-empty slice.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        let len = u64::try_from(items.len()).unwrap_or(u64::MAX);
        &items[usize::try_from(self.below(len)).unwrap_or(0)]
    }

    /// Poisson with mean `lambda` (Knuth; small means only).
    pub fn poisson(&mut self, lambda: f64) -> u32 {
        let limit = (-lambda).exp();
        let mut k = 0;
        let mut p = self.unit();
        while p > limit {
            k += 1;
            p *= self.unit();
        }
        k
    }

    /// Exponential with mean `mean`.
    pub fn exponential(&mut self, mean: f64) -> f64 {
        -mean * (1.0 - self.unit()).ln()
    }
}

/// A weighted choice over `0..n`, sampled by binary search of the cumulative weights.
#[derive(Debug, Clone)]
pub struct Weighted {
    cumulative: Vec<f64>,
}

impl Weighted {
    /// Weights in order; they need not sum to 1.
    pub fn new(weights: impl IntoIterator<Item = f64>) -> Self {
        let mut total = 0.0;
        let cumulative = weights
            .into_iter()
            .map(|w| {
                total += w;
                total
            })
            .collect();
        Self { cumulative }
    }

    /// Zipf–Mandelbrot over `n` ranks: weight of rank k is `1 / (k + q)^s`.
    pub fn zipf(n: usize, s: f64, q: f64) -> Self {
        #[allow(clippy::cast_precision_loss)]
        Self::new((1..=n).map(|k| 1.0 / (k as f64 + q).powf(s)))
    }

    /// A rank in `0..n`.
    pub fn sample(&self, rng: &mut Rng) -> usize {
        let total = self.cumulative.last().copied().unwrap_or(0.0);
        let target = rng.unit() * total;
        self.cumulative
            .partition_point(|&c| c <= target)
            .min(self.cumulative.len() - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_seed_gives_a_fixed_sequence() {
        let mut rng = Rng::new(42, 0);
        let first: Vec<u64> = (0..3).map(|_| rng.next_u64()).collect();
        let mut again = Rng::new(42, 0);
        assert_eq!(first, (0..3).map(|_| again.next_u64()).collect::<Vec<_>>());
        assert_ne!(first[0], Rng::new(42, 1).next_u64());
        assert_ne!(first[0], Rng::new(43, 0).next_u64());
    }

    #[test]
    fn below_stays_in_range_and_covers_it() {
        let mut rng = Rng::new(1, 0);
        let mut seen = [false; 7];
        for _ in 0..1_000 {
            let value = rng.below(7);
            seen[usize::try_from(value).unwrap_or(0)] = true;
        }
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn zipf_favours_low_ranks() {
        let mut rng = Rng::new(7, 0);
        let zipf = Weighted::zipf(100, 1.0, 2.7);
        let mut counts = [0u32; 100];
        for _ in 0..20_000 {
            counts[zipf.sample(&mut rng)] += 1;
        }
        assert!(counts[0] > counts[9] && counts[9] > counts[99]);
    }
}
