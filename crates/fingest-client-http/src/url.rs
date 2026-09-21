//! URL construction.
//!
//! Category names and logins go into the path, and both are free text. `format!` would
//! splice a `/` or a `#` straight through and address a different resource — or none — so
//! every interpolated segment is encoded here rather than at each call site.

use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};

/// Everything that is not an unreserved character per RFC 3986, plus `%` itself.
///
/// Deliberately stricter than a path-segment set: over-encoding is always safe to decode,
/// under-encoding changes which resource is addressed.
const SEGMENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'=')
    .add(b'@')
    .add(b'[')
    .add(b']')
    .add(b'\\')
    .add(b'^')
    .add(b'|')
    .add(b'+')
    .add(b'&')
    .add(b'$')
    .add(b',');

pub fn segment(value: &str) -> String {
    utf8_percent_encode(value, SEGMENT).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ordinary_name_is_left_alone() {
        assert_eq!(segment("Food"), "Food");
        assert_eq!(segment("bob_1-2.3~x"), "bob_1-2.3~x");
    }

    /// The case that motivates this module: a slash would otherwise add a path segment.
    #[test]
    fn a_slash_cannot_escape_its_segment() {
        assert_eq!(segment("Food/Drink"), "Food%2FDrink");
    }

    #[test]
    fn a_fragment_or_query_marker_cannot_truncate_the_path() {
        assert_eq!(segment("a#b"), "a%23b");
        assert_eq!(segment("a?b"), "a%3Fb");
    }

    #[test]
    fn spaces_and_percents_are_encoded() {
        assert_eq!(segment("Eating out"), "Eating%20out");
        assert_eq!(segment("100%"), "100%25");
    }

    #[test]
    fn non_ascii_is_utf8_percent_encoded() {
        assert_eq!(segment("Café"), "Caf%C3%A9");
    }

    #[test]
    fn a_traversal_attempt_stays_one_segment() {
        assert_eq!(segment("../../admin"), "..%2F..%2Fadmin");
    }
}
