use anyhow::{Ok, Result};
use rand::{rngs::StdRng, SeedableRng};
use rand_distr::Distribution as statrs_distribution;
use statrs::distribution::Poisson as statrs_poisson;

use RustQuant::statistics::distributions::{Distribution, Poisson};

/// Sample Poisson process.
pub fn sample_poisson(lambda: f64) -> Result<Vec<i32>> {
    let poisson = Poisson::new(lambda);
    let float_samples = poisson.sample(1);
    let int_sample: Vec<i32> = float_samples.iter().map(|&x| x.round() as i32).collect();
    Ok(int_sample)
}

/// Poisson process with seed.
#[derive(Debug, Clone)]
pub struct SeededPoisson {
    /// Poisson distribution.
    pub distribution: statrs_poisson,
    /// Random number generator.
    pub rng: StdRng,
}

/// Sample Poisson process with seed.
impl SeededPoisson {
    /// Create new Poisson process with seed.
    pub fn new(lambda: f64, seed: u64) -> Self {
        let distribution = statrs_poisson::new(lambda).unwrap();
        let rng = StdRng::seed_from_u64(seed);
        Self { distribution, rng }
    }
    /// Sample Poisson process.
    pub fn sample(&mut self) -> usize {
        self.distribution.sample(&mut self.rng) as usize
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn seed_test() {
        let mut test_dist_1 = SeededPoisson::new(10.0, 321);
        let mut test_dist_2 = SeededPoisson::new(10000.0, 123);
        let mut test_dist_3 = SeededPoisson::new(10000.0, 123);

        let result_1 = test_dist_1.sample();
        let result_2 = test_dist_1.sample();
        let result_3 = test_dist_2.sample();
        let result_4 = test_dist_2.sample();
        let result_5 = test_dist_3.sample();
        let result_6 = test_dist_3.sample();

        assert_eq!(result_1, 15);
        assert_eq!(result_2, 12);
        assert_eq!(result_3, 9914);
        assert_eq!(result_4, 10143);
        assert_eq!(result_5, result_3);
        assert_eq!(result_6, result_4);
    }
}
