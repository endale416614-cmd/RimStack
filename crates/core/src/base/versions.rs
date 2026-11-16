pub fn numbers(version: &str) -> Vec<u64> {
    let version = version.trim_start_matches('v');
    let core = version.split(['-', '+']).next().unwrap_or_default();
    core.split('.')
        .map(|part| part.parse().unwrap_or(0))
        .collect()
}

/// The highest of `versions` by their numbers, not the last written.
pub fn latest(versions: &[String]) -> Option<&str> {
    versions.iter().map(|v| v.trim()).max_by_key(|v| numbers(v))
}

/// Compared number by number, not as text: 0.10.0 is later than 0.9.3.
/// What is not a number counts as 0.
pub fn newer(current: &str, latest: &str) -> bool {
    let (mut now, mut then) = (numbers(current), numbers(latest));
    let parts = now.len().max(then.len());
    now.resize(parts, 0);
    then.resize(parts, 0);
    then > now
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_later_version_is_told_from_an_earlier_one_by_its_numbers() {
        assert!(newer("0.9.3", "0.10.0") && newer("1.2", "v1.2.1") && newer("0.0.0", "0.1.0-beta"));
        assert!(!newer("1.2.0", "1.2") && !newer("2.0.0", "1.9.9") && !newer("1.0.0", "1.0.0"));
    }

    #[test]
    fn the_latest_version_is_the_highest_listed_not_the_last() {
        let listed = ["1.10".to_string(), " 1.6 ".into(), "1.9".into()];
        assert_eq!(latest(&listed), Some("1.10"));
        assert_eq!(latest(&[]), None);
    }
}
