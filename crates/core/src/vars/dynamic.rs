//! Dynamic variables — fresh value per occurrence.

use rand::Rng;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

/// Resolve a `$name` token, or None if it's not a dynamic variable.
pub fn resolve(token: &str) -> Option<String> {
    match token {
        "$uuid" => Some(uuid::Uuid::new_v4().to_string()),
        "$timestamp" => Some(OffsetDateTime::now_utc().unix_timestamp().to_string()),
        "$isoTimestamp" => OffsetDateTime::now_utc().format(&Rfc3339).ok(),
        "$randomInt" => Some(rand::rng().random_range(0..=1000).to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_is_fresh_per_call() {
        let a = resolve("$uuid").unwrap();
        let b = resolve("$uuid").unwrap();
        assert_eq!(a.len(), 36);
        assert_ne!(a, b);
    }

    #[test]
    fn timestamps_and_ranges() {
        assert!(resolve("$timestamp").unwrap().parse::<i64>().unwrap() > 1_700_000_000);
        let iso = resolve("$isoTimestamp").unwrap();
        assert!(iso.contains('T') && iso.ends_with('Z'), "{iso}");
        let n: i32 = resolve("$randomInt").unwrap().parse().unwrap();
        assert!((0..=1000).contains(&n));
        assert_eq!(resolve("$nope"), None);
    }
}
