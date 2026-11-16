use crate::base::sentence::Sentence;
use crate::say;

const UNITS: [&str; 3] = ["KB", "MB", "GB"];

/// Both sizes in the one unit the larger needs, so a small one never rounds to 0.
pub fn together(a_mb: f64, b_mb: f64) -> (Sentence, Sentence) {
    let step = if a_mb.max(b_mb) >= 1024.0 {
        2
    } else if a_mb.max(b_mb) >= 1.0 {
        1
    } else {
        0
    };
    let scale = 1024f64.powi(1 - step as i32);
    (in_unit(a_mb * scale, step), in_unit(b_mb * scale, step))
}

pub fn of_bytes(bytes: u64) -> Sentence {
    together(bytes as f64 / 1_048_576.0, 0.0).0
}

fn in_unit(n: f64, step: usize) -> Sentence {
    let number = if n <= 0.0 {
        "0".to_string()
    } else if n >= 10.0 {
        format!("{n:.0}")
    } else if n >= 1.0 {
        format!("{n:.1}")
    } else if n >= 0.01 {
        format!("{n:.2}")
    } else {
        "<0.01".to_string()
    };
    match UNITS[step] {
        "KB" => say!("{} KB", number),
        "MB" => say!("{} MB", number),
        _ => say!("{} GB", number),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(a: f64, b: f64) -> (String, String) {
        let (a, b) = together(a, b);
        (a.to_string(), b.to_string())
    }

    #[test]
    fn the_larger_picks_the_unit_and_a_small_size_is_never_zero() {
        assert_eq!(pair(0.001, 0.0004), ("1.0 KB".into(), "0.41 KB".into()));
        assert_eq!(pair(120.0, 0.03), ("120 MB".into(), "0.03 MB".into()));
        assert_eq!(pair(2048.0, 3.0), ("2.0 GB".into(), "<0.01 GB".into()));
        assert_eq!(pair(5.0, 0.0), ("5.0 MB".into(), "0 MB".into()));
        assert_eq!(of_bytes(3 * 1_048_576).to_string(), "3.0 MB");
        assert_eq!(of_bytes(2048).to_string(), "2.0 KB");
    }
}
