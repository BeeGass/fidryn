//! `{{slot}}` templates. A slot with no value is a bug and panics.

/// Replace every `{{name}}` in `template` with its value from `vars`.
pub fn fill(template: &str, vars: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len() + 4096);
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after
            .find("}}")
            .unwrap_or_else(|| panic!("unclosed `{{{{` in template"));
        let key = after[..end].trim();
        let value = vars
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| *v)
            .unwrap_or_else(|| panic!("template slot `{key}` has no value"));
        out.push_str(value);
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_replaces_every_slot() {
        let page = fill(
            "<title>{{title}}</title><p>{{body}}</p>",
            &[("title", "T"), ("body", "B")],
        );
        assert_eq!(page, "<title>T</title><p>B</p>");
    }

    #[test]
    fn fill_repeats_a_slot() {
        assert_eq!(fill("{{x}} and {{x}}", &[("x", "a")]), "a and a");
    }

    #[test]
    fn fill_trims_spaces_inside_the_braces() {
        assert_eq!(fill("[{{ name }}]", &[("name", "v")]), "[v]");
    }

    #[test]
    fn fill_inserts_values_verbatim() {
        // docs/mill.md shows `{{module}}@{{version}}`; a filled body must keep it.
        let page = fill(
            "<main>{{main}}</main>",
            &[("main", "{{module}}@{{version}}")],
        );
        assert_eq!(page, "<main>{{module}}@{{version}}</main>");
    }

    #[test]
    #[should_panic(expected = "template slot `missing` has no value")]
    fn fill_panics_on_a_missing_slot() {
        fill("{{missing}}", &[("other", "x")]);
    }

    #[test]
    #[should_panic(expected = "unclosed `{{` in template")]
    fn fill_panics_on_an_unclosed_slot() {
        fill("{{open", &[]);
    }
}
