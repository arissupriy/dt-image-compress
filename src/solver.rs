use crate::CompressionError;

pub(crate) struct Candidate {
    pub bytes: Vec<u8>,
    pub quality: u8,
}

pub(crate) struct SearchResult {
    pub best: Option<Candidate>,
    pub smallest: u64,
    pub attempts: u32,
}

pub(crate) fn search_target(
    target: u64,
    budget: u32,
    mut encode: impl FnMut(u8) -> Result<Vec<u8>, CompressionError>,
) -> Result<SearchResult, CompressionError> {
    let mut result = SearchResult {
        best: None,
        smallest: u64::MAX,
        attempts: 0,
    };
    let (mut low, mut high) = (40_i16, 95_i16);
    // The maximum endpoint avoids unnecessary search when the best allowed quality fits.
    let mut next = Some(95_u8);
    while result.attempts < budget {
        let quality = match next.take() {
            Some(q) => q,
            None if low <= high => ((low + high) / 2) as u8,
            None => break,
        };
        let bytes = encode(quality)?;
        result.attempts += 1;
        result.smallest = result.smallest.min(bytes.len() as u64);
        if bytes.len() as u64 <= target {
            if result.best.as_ref().is_none_or(|b| quality > b.quality) {
                result.best = Some(Candidate { bytes, quality });
            }
            if quality == 95 {
                break;
            }
            low = i16::from(quality) + 1;
        } else {
            high = i16::from(quality) - 1;
        }
    }
    Ok(result)
}

pub(crate) fn search_target_log_secant(
    target: u64,
    budget: u32,
    mut encode: impl FnMut(u8) -> Result<Vec<u8>, CompressionError>,
) -> Result<SearchResult, CompressionError> {
    let mut result = SearchResult {
        best: None,
        smallest: u64::MAX,
        attempts: 0,
    };
    let (mut low, mut high) = (None, None);
    let mut quality = 95;
    let mut previous_width = 55;
    let mut slow_steps = 0;
    while result.attempts < budget {
        let bytes = encode(quality)?;
        let size = bytes.len() as u64;
        result.attempts += 1;
        result.smallest = result.smallest.min(size);
        if size <= target {
            if result.best.as_ref().is_none_or(|b| quality > b.quality) {
                result.best = Some(Candidate { bytes, quality });
            }
            low = Some((quality, size));
            if quality == 95 {
                break;
            }
        } else {
            high = Some((quality, size));
            // Like binary search, this assumes approximately monotonic sizes.
            if quality == 40 {
                break;
            }
        }
        let (Some((lo, lo_size)), Some((hi, hi_size))) = (low, high) else {
            quality = 40;
            continue;
        };
        let width = hi - lo;
        if width <= 1 {
            break;
        }
        slow_steps = if width * 4 > previous_width * 3 {
            slow_steps + 1
        } else {
            0
        };
        previous_width = width;
        let denominator = (hi_size.max(1) as f64).ln() - (lo_size.max(1) as f64).ln();
        let estimate = f64::from(lo)
            + ((target.max(1) as f64).ln() - (lo_size.max(1) as f64).ln()) / denominator
                * f64::from(width);
        quality = if slow_steps >= 2 || denominator <= 0.0 || !estimate.is_finite() {
            slow_steps = 0;
            lo + width / 2
        } else {
            (estimate.floor() as u8).clamp(lo + 1, hi - 1)
        };
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_highest_feasible_quality_and_does_not_stop_at_first_fit() {
        let r = search_target(700, 32, |q| Ok(vec![0; usize::from(q) * 10])).unwrap();
        assert_eq!(r.best.unwrap().quality, 70);
        assert!(r.attempts <= 8);
    }

    #[test]
    fn plateau_budget_and_unreachable_are_bounded() {
        let r = search_target(50, 3, |_| Ok(vec![0; 100])).unwrap();
        assert!(r.best.is_none());
        assert_eq!(r.attempts, 3);
        assert_eq!(r.smallest, 100);
    }

    #[test]
    fn nonmonotonic_sizes_never_return_oversized_success() {
        let r = search_target(75, 32, |q| {
            Ok(vec![0; if q % 3 == 0 { 200 } else { usize::from(q) }])
        })
        .unwrap();
        assert!(r.best.unwrap().bytes.len() <= 75);
    }

    #[test]
    fn log_secant_finds_the_same_quality_with_fewer_exponential_probes() {
        let encode = |q: u8| Ok(vec![0; (100.0 * (0.06 * f64::from(q - 40)).exp()) as usize]);
        let binary = search_target(604, 32, encode).unwrap();
        let secant = search_target_log_secant(604, 32, encode).unwrap();
        assert_eq!(secant.best.as_ref().unwrap().quality, 70);
        assert_eq!(
            secant.best.as_ref().unwrap().quality,
            binary.best.unwrap().quality
        );
        assert!(secant.attempts < binary.attempts);
    }

    #[test]
    fn log_secant_keeps_the_hard_cap_and_budget_on_plateaus_and_nonmonotonic_sizes() {
        for budget in [1, 2, 3, 32] {
            let r = search_target_log_secant(75, budget, |q| {
                Ok(vec![0; if q % 3 == 0 { 200 } else { usize::from(q) }])
            })
            .unwrap();
            assert!(r.attempts <= budget);
            if let Some(best) = r.best {
                assert!(best.bytes.len() <= 75);
            }
            let flat = search_target_log_secant(50, budget, |_| Ok(vec![0; 100])).unwrap();
            assert!(flat.best.is_none());
            assert_eq!(flat.smallest, 100);
            assert!(flat.attempts <= budget);
        }
    }
}
