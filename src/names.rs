//! Profile-name rules. Identical to MultiSignal.sh, so both tools agree.

pub const MAX_LEN: usize = 64;

/// Letters, digits, `.`, `_`, `-`, starting with a letter or digit. This rules
/// out empty names, `.`/`..`, `/`, spaces, quotes and leading dashes.
pub fn is_valid(name: &str) -> bool {
    let mut chars = name.chars();
    let first_ok = chars.next().is_some_and(|c| c.is_ascii_alphanumeric());
    first_ok
        && name.len() <= MAX_LEN
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Turns a rejected name such as "My Work!" into "My-Work", or `None` if
/// nothing usable is left.
pub fn suggest(input: &str) -> Option<String> {
    let mut out = String::new();
    for c in input.trim().chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '.' | '_') {
            out.push(c);
        } else if (c.is_whitespace() || c == '-') && !out.ends_with('-') {
            out.push('-');
        }
    }
    let start = out.find(|c: char| c.is_ascii_alphanumeric())?;
    let fixed: String = out[start..].chars().take(MAX_LEN).collect();
    let fixed = fixed.trim_end_matches('-').to_string();
    is_valid(&fixed).then_some(fixed)
}

/// "A, B, C" for messages.
pub fn join<S: AsRef<str>>(names: &[S]) -> String {
    names
        .iter()
        .map(AsRef::as_ref)
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_names() {
        for n in ["Work", "UKR", "Family_2", "a.b-c", "9lives"] {
            assert!(is_valid(n), "{n}");
        }
    }

    #[test]
    fn rejects_unsafe_names() {
        let long = "a".repeat(65);
        for n in [
            "",
            ".",
            "..",
            "../x",
            "a/b",
            "My Profile",
            "it's",
            "a|b",
            "-rf",
            ".hidden",
            " ",
            long.as_str(),
        ] {
            assert!(!is_valid(n), "{n:?}");
        }
    }

    #[test]
    fn suggests_fixed_names() {
        assert_eq!(suggest("My Work").as_deref(), Some("My-Work"));
        assert_eq!(suggest("  My   Work! ").as_deref(), Some("My-Work"));
        assert_eq!(suggest("../evil").as_deref(), Some("evil"));
        assert_eq!(suggest("!!!"), None);
    }

    #[test]
    fn joins_names_for_messages() {
        assert_eq!(join(&["A"]), "A");
        assert_eq!(join(&["A", "B", "C"]), "A, B, C");
    }
}
