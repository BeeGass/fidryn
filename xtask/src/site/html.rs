//! HTML escaping and asset fingerprints.

/// Escape text for HTML element content and double-quoted attributes.
pub fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Eight lowercase hex digits identifying `bytes` (FNV-1a, 64-bit, low 32 bits),
/// used as `?v=` on asset URLs so they can be cached for a year.
pub fn asset_version(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:08x}", hash & 0xffff_ffff)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn esc_escapes_markup_and_both_quotes() {
        assert_eq!(
            esc(r#"<a href="x">Tom & 'Jerry'</a>"#),
            "&lt;a href=&quot;x&quot;&gt;Tom &amp; &#39;Jerry&#39;&lt;/a&gt;"
        );
    }

    #[test]
    fn esc_leaves_other_text_alone() {
        assert_eq!(esc("§ 6 — café {{slot}}"), "§ 6 — café {{slot}}");
        assert_eq!(esc(""), "");
    }

    #[test]
    fn asset_version_is_pinned() {
        assert_eq!(asset_version(b""), "84222325");
        assert_eq!(asset_version(b"fidryn"), "388c70c5");
    }

    #[test]
    fn asset_version_is_eight_hex_digits_that_follow_the_content() {
        let v = asset_version(b"body { color: red }");
        assert_eq!(v.len(), 8);
        assert!(
            v.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')),
            "{v}"
        );
        assert_eq!(v, asset_version(b"body { color: red }"));
        assert_ne!(v, asset_version(b"body { color: blue }"));
    }
}
