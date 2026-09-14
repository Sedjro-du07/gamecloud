//! 6-digit numeric OTP generation.

use rand::Rng;

/// Generate a fresh 6-digit zero-padded OTP code, e.g. `"042973"`.
#[must_use]
pub fn generate() -> String {
    let mut rng = rand::thread_rng();
    let n: u32 = rng.gen_range(0..1_000_000);
    format!("{n:06}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn always_six_digits() {
        for _ in 0..100 {
            let c = generate();
            assert_eq!(c.len(), 6);
            assert!(c.chars().all(|c| c.is_ascii_digit()));
        }
    }
}
