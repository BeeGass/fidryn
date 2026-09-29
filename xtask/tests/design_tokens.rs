//! WCAG AA contrast of the design tokens in `site/assets/fidryn.css`.
//!
//! The stylesheet declares its colors in three token blocks: the light
//! `:root {` block, the explicit `:root[data-theme="dark"] {` block, and the
//! same dark values inside `@media (prefers-color-scheme: dark)`. These tests
//! parse those blocks, hold every pair in `PAIRS` to 4.5:1 in both themes,
//! and check that the dark blocks redefine every light color. Muted, status,
//! and code-comment text never sits on `--paper-3`: in the light theme
//! `--ink-3`, `--sus`, `--tok-com`, and `--tok-punct` fall below 4.5:1 there.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

const LIGHT: &str = ":root {";
const DARK: &str = ":root[data-theme=\"dark\"] {";
const DARK_MEDIA: &str = "@media (prefers-color-scheme: dark) {";
const DARK_SYSTEM: &str = ":root:not([data-theme=\"light\"]) {";

/// WCAG 2 level AA for normal-size text.
const AA: f64 = 4.5;

/// (foreground, background) token pairs that carry text.
const PAIRS: &[(&str, &str)] = &[
    ("--ink", "--paper"),
    ("--ink-2", "--paper"),
    ("--ink-3", "--paper"),
    ("--ink-3", "--paper-2"),
    ("--ink", "--paper-2"),
    ("--ink", "--paper-3"),
    ("--rubric", "--paper"),
    ("--paper", "--ink"),
    ("--det", "--paper"),
    ("--con", "--paper"),
    ("--sus", "--paper"),
    ("--nc", "--paper"),
    ("--oc", "--paper"),
    ("--inc", "--paper"),
    ("--tok-kw", "--paper"),
    ("--tok-type", "--paper"),
    ("--tok-str", "--paper"),
    ("--tok-num", "--paper"),
    ("--tok-com", "--paper"),
    ("--tok-punct", "--paper"),
];

fn stylesheet() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives one directory below the workspace root")
        .join("site/assets/fidryn.css");
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display()))
}

/// Byte range of the declarations between `start` (ending in `{`) and the next `}`.
fn block_range(css: &str, from: usize, start: &str) -> (usize, usize) {
    let at = css[from..]
        .find(start)
        .map(|i| from + i)
        .unwrap_or_else(|| panic!("fidryn.css has no `{start}` block"));
    let body = at + start.len();
    let end = css[body..]
        .find('}')
        .map(|i| body + i)
        .unwrap_or_else(|| panic!("the `{start}` block in fidryn.css is not closed"));
    (body, end)
}

struct Blocks<'a> {
    light: &'a str,
    dark: &'a str,
    dark_system: &'a str,
    /// The stylesheet with the three token blocks cut out.
    rest: String,
}

fn blocks(css: &str) -> Blocks<'_> {
    let light = block_range(css, 0, LIGHT);
    let dark = block_range(css, 0, DARK);
    let media = css
        .find(DARK_MEDIA)
        .unwrap_or_else(|| panic!("fidryn.css has no `{DARK_MEDIA}` block"));
    let dark_system = block_range(css, media, DARK_SYSTEM);
    let mut ranges = [light, dark, dark_system];
    ranges.sort_unstable();
    let mut rest = String::with_capacity(css.len());
    let mut cursor = 0;
    for (start, end) in ranges {
        rest.push_str(&css[cursor..start]);
        cursor = end;
    }
    rest.push_str(&css[cursor..]);
    Blocks {
        light: &css[light.0..light.1],
        dark: &css[dark.0..dark.1],
        dark_system: &css[dark_system.0..dark_system.1],
        rest,
    }
}

/// Custom properties declared in a block: name to value.
fn tokens(block: &str) -> BTreeMap<&str, &str> {
    block
        .split(';')
        .filter_map(|decl| {
            let (name, value) = decl.split_once(':')?;
            let name = name.trim();
            name.starts_with("--").then(|| (name, value.trim()))
        })
        .collect()
}

/// Names referenced with `var(--name` anywhere in `css`.
fn used_properties(css: &str) -> BTreeSet<&str> {
    css.match_indices("var(")
        .filter_map(|(at, _)| {
            let rest = css[at + 4..].trim_start();
            let len = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
                .unwrap_or(rest.len());
            let name = &rest[..len];
            name.starts_with("--").then_some(name)
        })
        .collect()
}

/// Names declared as `--name:` in `css`.
fn declared_properties(css: &str) -> BTreeSet<&str> {
    css.match_indices("--")
        .filter_map(|(at, _)| {
            let rest = &css[at..];
            let len = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
                .unwrap_or(rest.len());
            let preceded_by_name = css[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '(');
            let declares = rest[len..].trim_start().starts_with(':');
            (declares && !preceded_by_name && len > 2).then(|| &rest[..len])
        })
        .collect()
}

fn channel(hex: &str) -> f64 {
    let v = f64::from(u8::from_str_radix(hex, 16).expect("hex channel")) / 255.0;
    if v <= 0.040_45 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// WCAG relative luminance of a `#rrggbb` color.
fn luminance(color: &str) -> f64 {
    let hex = color
        .strip_prefix('#')
        .filter(|h| h.len() == 6 && h.bytes().all(|b| b.is_ascii_hexdigit()))
        .unwrap_or_else(|| panic!("`{color}` is not a #rrggbb color"));
    0.2126 * channel(&hex[0..2]) + 0.7152 * channel(&hex[2..4]) + 0.0722 * channel(&hex[4..6])
}

fn contrast(a: &str, b: &str) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

fn failing_pairs(theme: &str, block: &str) -> Vec<String> {
    let colors = tokens(block);
    let color = |name: &str| {
        *colors
            .get(name)
            .unwrap_or_else(|| panic!("the {theme} token block does not define `{name}`"))
    };
    PAIRS
        .iter()
        .filter_map(|&(fg, bg)| {
            let ratio = contrast(color(fg), color(bg));
            (ratio < AA).then(|| format!("{theme}: {fg} on {bg} is {ratio:.2}:1"))
        })
        .collect()
}

#[test]
fn text_pairs_meet_wcag_aa_in_both_themes() {
    let css = stylesheet();
    let b = blocks(&css);
    let mut failures = failing_pairs("light", b.light);
    failures.extend(failing_pairs("dark", b.dark));
    failures.extend(failing_pairs("system dark", b.dark_system));
    assert!(
        failures.is_empty(),
        "below {AA}:1:\n  {}",
        failures.join("\n  ")
    );
}

#[test]
fn explicit_dark_block_equals_system_dark_block() {
    let css = stylesheet();
    let b = blocks(&css);
    let normalize = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    assert_eq!(
        normalize(b.dark),
        normalize(b.dark_system),
        "`{DARK}` and the `{DARK_SYSTEM}` block inside `{DARK_MEDIA}` must declare the same values"
    );
}

#[test]
fn dark_block_redefines_every_light_color() {
    let css = stylesheet();
    let b = blocks(&css);
    let light = tokens(b.light);
    let dark = tokens(b.dark);
    let missing: Vec<&str> = light
        .iter()
        .filter(|(name, value)| value.starts_with('#') && !dark.contains_key(*name))
        .map(|(name, _)| *name)
        .collect();
    let extra: Vec<&str> = dark
        .keys()
        .copied()
        .filter(|name| !light.contains_key(name))
        .collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "`{DARK}` must redefine every light color and nothing the light block lacks; missing {missing:?}, extra {extra:?}"
    );
}

#[test]
fn every_custom_property_used_is_defined_in_the_light_block() {
    let css = stylesheet();
    let b = blocks(&css);
    let light = tokens(b.light);
    let missing: Vec<&str> = used_properties(&css)
        .into_iter()
        .filter(|name| !light.contains_key(name))
        .collect();
    assert!(
        missing.is_empty(),
        "used but not defined in `{LIGHT}`: {missing:?}"
    );
}

#[test]
fn tokens_are_declared_only_in_the_token_blocks() {
    let css = stylesheet();
    for selector in [LIGHT, DARK, DARK_SYSTEM] {
        assert_eq!(
            css.matches(selector).count(),
            1,
            "`{selector}` must appear exactly once"
        );
    }
    let b = blocks(&css);
    let stray = declared_properties(&b.rest);
    assert!(
        stray.is_empty(),
        "custom properties declared outside the token blocks would bypass the contrast check: {stray:?}"
    );
}
