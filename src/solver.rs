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
}
