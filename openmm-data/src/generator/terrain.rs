//! Deterministic heightmap generator — no external dependencies.
//!
//! Produces 128×128 `u8` heightmaps using layered sine waves.
//! The same `seed` always yields the same terrain, making outputs reproducible
//! across runs (CI-safe).

/// Generate heightmaps for ODM outdoor maps.
pub struct TerrainGen {
    pub width: usize,
    pub height: usize,
}

impl TerrainGen {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height }
    }

    /// Generate a `[0..255]` heightmap using layered sine waves.
    ///
    /// `seed` controls the phase offsets — identical seeds always produce
    /// identical output.
    pub fn generate(&self, seed: u64) -> Vec<u8> {
        let w = self.width;
        let h = self.height;
        let mut map = vec![0u8; w * h];

        // Three sine layers at different frequencies / amplitudes
        let layers: &[(f64, f64, f64, f64)] = &[
            // (freq_x, freq_y, amplitude, phase_shift)
            (2.0, 3.0, 60.0, 0.0),
            (5.0, 7.0, 25.0, 1.0),
            (11.0, 13.0, 10.0, 2.0),
        ];

        let seed_f = seed as f64 * 0.1;

        for row in 0..h {
            for col in 0..w {
                let fx = col as f64 / w as f64;
                let fy = row as f64 / h as f64;
                let mut val = 128.0_f64;

                for &(fq_x, fq_y, amp, ph) in layers {
                    val += amp
                        * (std::f64::consts::TAU * fq_x * fx + seed_f + ph).sin()
                        * (std::f64::consts::TAU * fq_y * fy + seed_f + ph * 0.7).sin();
                }

                map[row * w + col] = val.clamp(0.0, 255.0) as u8;
            }
        }

        map
    }

    /// Flatten a rectangular region to a fixed `height` value.
    /// Useful for creating flat building pads or spawn platforms.
    pub fn flatten_rect(&self, map: &mut Vec<u8>, x: usize, y: usize, w: usize, h: usize, value: u8) {
        let map_w = self.width;
        let map_h = self.height;
        for row in y..((y + h).min(map_h)) {
            for col in x..((x + w).min(map_w)) {
                map[row * map_w + col] = value;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_is_deterministic() {
        let tgen = TerrainGen::new(128, 128);
        let a = tgen.generate(42);
        let b = tgen.generate(42);
        assert_eq!(a, b, "same seed must produce identical heightmap");
        assert_eq!(a.len(), 128 * 128);
    }

    #[test]
    fn different_seeds_differ() {
        let tgen = TerrainGen::new(128, 128);
        let a = tgen.generate(1);
        let b = tgen.generate(2);
        assert_ne!(a, b);
    }

    #[test]
    fn flatten_rect_works() {
        let tgen = TerrainGen::new(128, 128);
        let mut map = tgen.generate(0);
        tgen.flatten_rect(&mut map, 60, 60, 10, 10, 80);
        for row in 60..70 {
            for col in 60..70 {
                assert_eq!(map[row * 128 + col], 80);
            }
        }
    }
}
