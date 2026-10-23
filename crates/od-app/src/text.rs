//! Small wording helpers shared by messages.

/// "1 object", "3 objects".
pub fn plural(count: usize, noun: &str) -> String {
    format!("{count} {noun}{}", if count == 1 { "" } else { "s" })
}

/// How long ago something was, in words: "just now", "5 minutes ago".
pub fn age_text(age: std::time::Duration) -> String {
    let minutes = age.as_secs() / 60;
    let ago = |n: u64, unit: &str| format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" });
    match minutes {
        0 => "just now".to_owned(),
        1..=59 => ago(minutes, "minute"),
        60..=1439 => ago(minutes / 60, "hour"),
        _ => ago(minutes / 1440, "day"),
    }
}

#[cfg(test)]
mod tests {
    use super::{age_text, plural};

    #[test]
    fn ages_read_naturally() {
        let secs = std::time::Duration::from_secs;
        assert_eq!(age_text(secs(20)), "just now");
        assert_eq!(age_text(secs(60)), "1 minute ago");
        assert_eq!(age_text(secs(5 * 60)), "5 minutes ago");
        assert_eq!(age_text(secs(60 * 60)), "1 hour ago");
        assert_eq!(age_text(secs(3 * 3600 + 100)), "3 hours ago");
        assert_eq!(age_text(secs(2 * 86400)), "2 days ago");
    }

    #[test]
    fn plural_adds_s_except_for_one() {
        assert_eq!(plural(0, "object"), "0 objects");
        assert_eq!(plural(1, "object"), "1 object");
        assert_eq!(plural(2, "object"), "2 objects");
    }
}
