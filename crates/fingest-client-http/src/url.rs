//! URL construction.
//!
//! Category names and logins go into the path, and both are free text. `format!` would
//! splice a `/` or a `#` straight through and address a different resource — or none — so
//! every interpolated segment is encoded here rather than at each call site.

use fingest_client_ports::ClientError;
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

/// Encodes one path segment, refusing the values no encoding can keep in place.
///
/// URL parsing (the `url` crate natively, `fetch` in the browser) removes `.` and `..`
/// segments and treats `%2e` as a dot, so those would address a different path. An empty
/// value would collapse into `//`.
pub fn segment(value: &str) -> Result<String, ClientError> {
    if matches!(value, "" | "." | "..") {
        return Err(ClientError::BadRequest(format!(
            "\"{value}\" cannot be used as a name"
        )));
    }
    Ok(utf8_percent_encode(value, SEGMENT).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encoded(value: &str) -> String {
        segment(value).unwrap()
    }

    #[test]
    fn an_ordinary_name_is_left_alone() {
        assert_eq!(encoded("Food"), "Food");
        assert_eq!(encoded("bob_1-2.3~x"), "bob_1-2.3~x");
    }

    /// The case that motivates this module: a slash would otherwise add a path segment.
    #[test]
    fn a_slash_cannot_escape_its_segment() {
        assert_eq!(encoded("Food/Drink"), "Food%2FDrink");
    }

    #[test]
    fn a_fragment_or_query_marker_cannot_truncate_the_path() {
        assert_eq!(encoded("a#b"), "a%23b");
        assert_eq!(encoded("a?b"), "a%3Fb");
    }

    #[test]
    fn spaces_and_percents_are_encoded() {
        assert_eq!(encoded("Eating out"), "Eating%20out");
        assert_eq!(encoded("100%"), "100%25");
    }

    #[test]
    fn non_ascii_is_utf8_percent_encoded() {
        assert_eq!(encoded("Café"), "Caf%C3%A9");
    }

    #[test]
    fn a_traversal_attempt_stays_one_segment() {
        assert_eq!(encoded("../../admin"), "..%2F..%2Fadmin");
    }

    /// `..` and `.` are removed by URL parsing whichever way they are spelled.
    #[test]
    fn dot_only_and_empty_segments_are_refused() {
        for value in ["", ".", ".."] {
            assert!(
                matches!(segment(value), Err(ClientError::BadRequest(_))),
                "{value:?} must not reach a URL"
            );
        }
    }

    /// Built with the parser `reqwest` uses, so a normalised-away segment would show here.
    #[test]
    fn names_containing_dots_keep_their_place_in_the_path() {
        for value in ["...", "a.b", ".hidden", "x..", "..%2F"] {
            let url = format!("http://h/resources/users/{}/wallets", encoded(value));
            let parsed = reqwest::Url::parse(&url).unwrap();
            let segments: Vec<_> = parsed.path_segments().unwrap().collect();

            assert_eq!(segments.len(), 4, "{value:?} changed the path: {parsed}");
            assert_eq!(segments[3], "wallets");
        }
    }
}
