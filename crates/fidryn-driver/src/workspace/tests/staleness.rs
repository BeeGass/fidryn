//! The staleness of bindings. A curated workspace of the prefix
//! `/zz/notes`, in `notes/` of a scratch directory, depends on a generated
//! workspace of the prefix `/zz/demo` in `demo/` beside it, named `demo` at
//! `../demo`. Its bindings are written against the first version of the
//! dependency, `v1`, with the hashes of its models, and checked against a
//! second version, `v2`, which `demo/` holds and the manifest pins. Every
//! text is invented.

use super::*;
use fidryn_provision::hash::{hash_provision, layer_hashes_of_unit};
use fidryn_provision::{BoundHashes, StaleReason};

/// The unit whose provisions most bindings here bind.
const S8: &str = "/zz/demo/t1/s8";

/// A subsection `value` of the provision at `parent` that says `words`.
fn child(parent: &str, value: &str, words: &str) -> Item {
    Item::Child(Provision {
        address: address(&format!("{parent}/{value}")),
        kind: "subsection".to_owned(),
        num: num(value, &format!("({value})")),
        heading: None,
        status: None,
        items: vec![Item::Part {
            role: PartRole::Content,
            blocks: vec![Block::Para(plain(words))],
        }],
    })
}

/// A note `name` of the module at `module`, in `layer`, that says `words`.
fn note_of(module: &str, name: &str, layer: Layer, words: &str) -> Note {
    Note {
        address: address(&format!("{module}/note/demoNotes/nolaw/{name}")),
        layer,
        group: "demoNotes".to_owned(),
        topics: Vec::new(),
        heading: Some(plain("An Invented Note")),
        blocks: vec![Block::Para(plain(words))],
    }
}

/// The first version of the dependency: the demo models, the unit `s3`
/// with the alias `/zz/demo/t1/s4`, the chapter with the alias
/// `/zz/demo/t1/chI`, and the unit `s8` of the chapter, with the
/// subsections `a` and `b`, an enacted note, an editorial note, a source
/// credit and a footnote whose marker stands in its heading.
fn v1() -> Vec<Module> {
    let mut models = demo_models();
    let chapter = container_at(&mut models, 1);
    chapter.members.push(address(S8));
    chapter.aliases.push(address("/zz/demo/t1/chI"));
    unit_at(&mut models, 4)
        .aliases
        .push(address("/zz/demo/t1/s4"));
    let mut s8 = unit(
        S8,
        Some(CHAPTER),
        "8",
        "Parts",
        "A widget has invented parts.",
    );
    s8.root.heading = Some(Text::from_runs(vec![
        Run::Words("Parts".to_owned()),
        Run::Mark {
            shown: "1".to_owned(),
            footnote: 1,
        },
    ]));
    s8.root
        .items
        .push(child(S8, "a", "An invented subsection."));
    s8.root
        .items
        .push(child(S8, "b", "Another invented subsection."));
    s8.credit = Some(plain("(Added by an invented act.)"));
    s8.notes = vec![
        note_of(
            S8,
            "effective",
            Layer::Enacted,
            "An invented act took effect on an invented day.",
        ),
        note_of(
            S8,
            "codification",
            Layer::Editorial,
            "An invented editor wrote this note.",
        ),
    ];
    s8.footnotes = vec![Footnote {
        ordinal: 1,
        shown: "1".to_owned(),
        blocks: vec![Block::Para(plain("An invented footnote."))],
    }];
    models.push(Module::Unit(s8));
    models
}

/// The unit `s8` of `models`, which [`v1`] gives.
fn s8_of(models: &mut [Module]) -> &mut UnitModule {
    unit_at(models, 5)
}

/// The provision at `at` in the tree of `provision`.
fn find<'a>(provision: &'a Provision, at: &str) -> Option<&'a Provision> {
    if provision.address.as_str() == at {
        return Some(provision);
    }
    provision.items.iter().find_map(|item| match item {
        Item::Child(child) => find(child, at),
        Item::Part { .. } => None,
    })
}

/// The provision at `at` in the tree of `provision`, to change.
fn find_mut<'a>(provision: &'a mut Provision, at: &str) -> Option<&'a mut Provision> {
    if provision.address.as_str() == at {
        return Some(provision);
    }
    provision.items.iter_mut().find_map(|item| match item {
        Item::Child(child) => find_mut(child, at),
        Item::Part { .. } => None,
    })
}

/// Replaces `from` with `to` in the words of the provision at `at` of
/// `models`, which must hold them.
fn reword(models: &mut [Module], at: &str, from: &str, to: &str) {
    let mut changed = false;
    for model in models.iter_mut() {
        let Module::Unit(unit) = model else {
            continue;
        };
        let Some(provision) = find_mut(&mut unit.root, at) else {
            continue;
        };
        for item in &mut provision.items {
            let Item::Part { blocks, .. } = item else {
                continue;
            };
            for block in blocks {
                if let Block::Para(text) = block
                    && text.plain().contains(from)
                {
                    *text = plain(&text.plain().replacen(from, to, 1));
                    changed = true;
                }
            }
        }
    }
    assert!(changed, "no provision at {at} says {from:?}");
}

/// The hashes that a binding to `at` records when it is written against
/// `models`, worked out from the models: the tree hash of the provision at
/// `at`, or of the top provision of the unit whose alias it is, and the
/// enacted layer of its unit. `None` when `at` is neither.
fn hashes_of(models: &[Module], at: &str) -> Option<BoundHashes> {
    models.iter().find_map(|model| {
        let Module::Unit(unit) = model else {
            return None;
        };
        let provision = if unit.aliases.iter().any(|alias| alias.as_str() == at) {
            Some(&unit.root)
        } else {
            find(&unit.root, at)
        }?;
        Some(BoundHashes {
            provision_hash: hash_provision(provision),
            notes_hash: layer_hashes_of_unit(unit).enacted,
        })
    })
}

/// A `binding` named `name` whose `binds` names `binds`, written against the
/// release `release` with `hashes`.
fn binding_text(name: &str, binds: &str, release: &str, hashes: BoundHashes) -> String {
    format!(
        "binding {name} {{\n  binds {binds}\n  release \"{release}\"\n  provision_hash \"{}\"\n  \
         notes_hash \"{}\"\n  pending\n}}",
        hashes.provision_hash, hashes.notes_hash
    )
}

/// A `binding` named `name` whose `binds` names `binds`, written against
/// `models` of the release `release`: with the hashes of the provision at
/// `at`, which `models` holds.
pub(super) fn written_against(
    name: &str,
    binds: &str,
    release: &str,
    models: &[Module],
    at: &str,
) -> String {
    let hashes = hashes_of(models, at).unwrap_or_else(|| panic!("no provision is at {at}"));
    binding_text(name, binds, release, hashes)
}

/// The bindings that most tests here check: of the unit `s8`, of each of
/// its subsections, of the unit `s1`, and of the unit `s3` by its alias.
const BOUND: [(&str, &str); 5] = [
    ("section", S8),
    ("first", "/zz/demo/t1/s8/a"),
    ("second", "/zz/demo/t1/s8/b"),
    ("elsewhere", "/zz/demo/t1/s1"),
    ("aliased", "/zz/demo/t1/s4"),
];

/// The module `/zz/notes/a`, which binds each `(name, address)` of `bound`
/// by its address, written against [`v1`] of the release `v1`. A binding of
/// what is no provision there records the hashes of `s8`.
fn binding_module(bound: &[(&str, &str)]) -> String {
    let models = v1();
    let decls: Vec<String> = bound
        .iter()
        .map(|(name, at)| {
            let hashes = hashes_of(&models, at)
                .or_else(|| hashes_of(&models, S8))
                .expect("the unit s8");
            binding_text(name, &format!("\"{at}\""), "v1", hashes)
        })
        .collect();
    let decls: Vec<&str> = decls.iter().map(String::as_str).collect();
    note_module("a", &decls)
}

/// A scratch directory with the dependency in `demo/`, of the release
/// `release` and the models `models`, and in `notes/` the curated workspace
/// of `modules`, which depends on it, pinned to its lock.
fn against(tag: &str, release: &str, models: &[Module], modules: &[(&str, &str)]) -> Scratch {
    let scratch = Scratch::new(tag);
    let digest = write_release(&scratch.join("demo"), release, models);
    depending(
        &scratch.join("notes"),
        vec![dependency("demo", "../demo", release, digest)],
        modules,
    );
    scratch
}

/// Each stale binding of `report`, in their order: the name that the span
/// of its `W750` covers in `text`, the source of the module `a`, in whose
/// file it is, and the one reason its message gives.
fn stale_in(report: &WorkspaceReport, text: &str) -> Vec<(String, StaleReason)> {
    report
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == DiagnosticCode::W750)
        .map(|diagnostic| {
            assert_eq!(
                diagnostic.source_file.as_deref(),
                Some("zz/notes/a.fr"),
                "{diagnostic:#?}"
            );
            let span = diagnostic.primary_span.expect("a W750 has a span");
            let name = text[span.start as usize..span.end as usize].to_owned();
            let reasons: Vec<StaleReason> = [
                StaleReason::Removed,
                StaleReason::TextChanged,
                StaleReason::EnactedNotesChanged,
            ]
            .into_iter()
            .filter(|reason| diagnostic.message.contains(&format!("({reason})")))
            .collect();
            match reasons.as_slice() {
                [reason] => (name, *reason),
                _ => panic!("no one reason in {:?}", diagnostic.message),
            }
        })
        .collect()
}

/// `stale` as [`stale_in`] gives it.
fn named(stale: &[(&str, StaleReason)]) -> Vec<(String, StaleReason)> {
    stale
        .iter()
        .map(|(name, reason)| ((*name).to_owned(), *reason))
        .collect()
}

/// The message of the `W750` of a binding named `name` of `at` that is
/// stale for `reason` in the release `v2` of the dependency `demo`, written
/// against the release `v1`.
fn message_of(name: &str, at: &str, reason: StaleReason) -> String {
    let action = match reason {
        StaleReason::Removed => {
            "find where the provision now stands, read it again, and bind its address with the \
             hashes that `fidryn text` prints for it with `--hashes`"
        }
        StaleReason::TextChanged => {
            "read the provision again, and write into the binding the hashes that `fidryn text` \
             prints for it with `--hashes`"
        }
        StaleReason::EnactedNotesChanged => {
            "read the provision and the enacted notes of its unit again, and write into the \
             binding the hashes that `fidryn text` prints for it with `--hashes`"
        }
    };
    format!(
        "the binding `{name}` of `{at}` is stale ({reason}) in the release `v2` of the \
         dependency `demo`, and was written against the release `v1`; {action}"
    )
}

#[test]
fn a_binding_to_a_changed_provision_is_stale() {
    let mut v2 = v1();
    reword(
        &mut v2,
        "/zz/demo/t1/s8/a",
        "An invented subsection.",
        "An invented clause.",
    );
    let a = binding_module(&BOUND);
    let scratch = against("stale-text", "v2", &v2, &[("a", &a)]);
    let report = check(&scratch.join("notes"));
    // The subsection and the section that holds it; not its sibling, nor
    // another unit.
    assert_eq!(
        stale_in(&report, &a),
        named(&[
            ("section", StaleReason::TextChanged),
            ("first", StaleReason::TextChanged),
        ])
    );
    assert_eq!(
        (report.diagnostics.len(), report.errors(), report.warnings()),
        (2, 0, 2),
        "{report:#?}"
    );
    let first = &report.diagnostics[1];
    assert_eq!(
        first.message,
        message_of("first", "/zz/demo/t1/s8/a", StaleReason::TextChanged)
    );
    assert_eq!(first.primary_span, Some(span_of(&a, "binding ", "first")));

    // The same from the interfaces in memory, with the dependency as phase
    // two knows it.
    let dependency = DependencyInterfaces::Loaded {
        name: "demo".to_owned(),
        release: "v2".to_owned(),
        prefix: address("/zz/demo"),
        interfaces: v2.iter().map(interface_in_memory).collect(),
    };
    let phase_two = check_interfaces(
        &[interface_of_text(&a)],
        &[dependency],
        &notes_manifest(),
        TOOLCHAIN,
    );
    assert_eq!(phase_two.diagnostics, report.diagnostics);
}

#[test]
fn a_binding_in_a_unit_whose_enacted_notes_changed_is_stale() {
    let mut v2 = v1();
    s8_of(&mut v2).notes.push(note_of(
        S8,
        "amendment",
        Layer::Enacted,
        "An invented act amended this section.",
    ));
    // A note under a container enters no unit's enacted layer.
    container_at(&mut v2, 1).notes.push(note_of(
        CHAPTER,
        "amendment",
        Layer::Enacted,
        "An invented act amended this chapter.",
    ));
    let a = binding_module(&BOUND);
    let scratch = against("stale-notes", "v2", &v2, &[("a", &a)]);
    let report = check(&scratch.join("notes"));
    assert_eq!(
        stale_in(&report, &a),
        named(&[
            ("section", StaleReason::EnactedNotesChanged),
            ("first", StaleReason::EnactedNotesChanged),
            ("second", StaleReason::EnactedNotesChanged),
        ])
    );
    assert_eq!(report.diagnostics.len(), 3, "{report:#?}");
    assert_eq!(
        report.diagnostics[2].message,
        message_of(
            "second",
            "/zz/demo/t1/s8/b",
            StaleReason::EnactedNotesChanged
        )
    );
}

#[test]
fn editorial_changes_make_nothing_stale() {
    let mut v2 = v1();
    let s8 = s8_of(&mut v2);
    s8.credit = Some(plain("(Added by another invented act.)"));
    s8.notes[1].blocks = vec![Block::Para(plain(
        "Another invented editor wrote this note.",
    ))];
    s8.footnotes[0].blocks = vec![Block::Para(plain("A revised invented footnote."))];
    // Only the editorial layer of the unit changed.
    let mut first = v1();
    let before = layer_hashes_of_unit(s8_of(&mut first));
    let after = layer_hashes_of_unit(s8_of(&mut v2));
    assert_ne!(before.editorial, after.editorial);
    assert_eq!(
        (before.operative, before.enacted),
        (after.operative, after.enacted)
    );
    let a = binding_module(&BOUND);
    let scratch = against("stale-editorial", "v2", &v2, &[("a", &a)]);
    let report = check(&scratch.join("notes"));
    assert_eq!(report.diagnostics, []);
}

#[test]
fn a_binding_to_a_provision_that_is_gone_is_stale() {
    // `s8/b` moves to `s8/c`.
    let mut v2 = v1();
    let moved = find_mut(&mut s8_of(&mut v2).root, "/zz/demo/t1/s8/b").expect("s8/b");
    moved.address = address("/zz/demo/t1/s8/c");
    moved.num = num("c", "(c)");
    // Besides: a note, a container, an alias of a container, nothing, the
    // workspace's own module, and an address below no prefix at all.
    let mut bound = BOUND.to_vec();
    bound.extend([
        ("a_note", "/zz/demo/t1/s8/note/demoNotes/nolaw/effective"),
        ("a_container", CHAPTER),
        ("an_alias_of_a_container", "/zz/demo/t1/chI"),
        ("nothing", "/zz/demo/t1/s9"),
        ("own", "/zz/notes/a"),
        ("outside", "/zz/other/x"),
    ]);
    let a = binding_module(&bound);
    let scratch = against("stale-gone", "v2", &v2, &[("a", &a)]);
    let report = check(&scratch.join("notes"));
    // The section that holds the moved subsection, whose number changed,
    // has another text.
    let mut expected = vec![("section", StaleReason::TextChanged)];
    expected.extend(
        [
            "second",
            "a_note",
            "a_container",
            "an_alias_of_a_container",
            "nothing",
            "own",
            "outside",
        ]
        .into_iter()
        .map(|name| (name, StaleReason::Removed)),
    );
    assert_eq!(stale_in(&report, &a), named(&expected));
    assert_eq!(
        (report.diagnostics.len(), report.errors()),
        (8, 0),
        "{report:#?}"
    );
    assert_eq!(
        report.diagnostics[1].message,
        message_of("second", "/zz/demo/t1/s8/b", StaleReason::Removed)
    );
    assert_eq!(
        report.diagnostics[6].message,
        "the binding `own` of `/zz/notes/a` is stale (provision removed or moved): the address \
         lies below no dependency of the workspace, and the binding was written against the \
         release `v1`; bind a provision of a dependency, with the hashes that `fidryn text` \
         prints for it with `--hashes`"
    );

    // Without a dependency, every binding binds an address below none.
    let alone = Scratch::new("stale-alone");
    notes_workspace(alone.path(), &[("a", &a)]);
    let report = check(alone.path());
    let every: Vec<(&str, StaleReason)> = bound
        .iter()
        .map(|(name, _)| (*name, StaleReason::Removed))
        .collect();
    assert_eq!(stale_in(&report, &a), named(&every));
    assert!(
        report
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.message.contains("lies below no dependency")),
        "{report:#?}"
    );
}

#[test]
fn nothing_is_stale_when_nothing_changed() {
    // The release a binding records is compared with nothing: against the
    // same models, of its own release or of another, no binding is stale.
    let a = binding_module(&BOUND);
    for release in ["v1", "v2"] {
        let scratch = against(
            &format!("stale-none-{release}"),
            release,
            &v1(),
            &[("a", &a)],
        );
        let report = check(&scratch.join("notes"));
        assert_eq!(report.diagnostics, [], "{release}");
        assert_eq!(
            report.summary(),
            "modules: 1; from cache: 0; errors: 0; warnings: 0"
        );
    }
}

#[test]
fn a_binding_by_name_is_checked_like_one_by_address() {
    // `v2` changes a word of `s8/a` and of `s1`, and moves `s8/b` to `s8/c`.
    let mut v2 = v1();
    reword(
        &mut v2,
        "/zz/demo/t1/s8/a",
        "An invented subsection.",
        "An invented clause.",
    );
    reword(&mut v2, "/zz/demo/t1/s1", "a small device", "a tiny device");
    let moved = find_mut(&mut s8_of(&mut v2).root, "/zz/demo/t1/s8/b").expect("s8/b");
    moved.address = address("/zz/demo/t1/s8/c");
    let models = v1();
    let bind = |name: &str, binds: &str, at: &str| written_against(name, binds, "v1", &models, at);
    let b = note_module("b", &["citation parts -> \"/zz/demo/t1/s8/b\""]);
    let c = note_module("c", &["citation gadget -> \"/zz/demo/t1/s1\""]);
    // Each target by a name, then by its address: a local citation, a name
    // that an import brings into scope, and `alias.name`; and an unchanged
    // provision by a name.
    let pairs = [
        ("widget", "\"/zz/demo/t1/s8/a\"", "/zz/demo/t1/s8/a"),
        ("gadget", "\"/zz/demo/t1/s1\"", "/zz/demo/t1/s1"),
        ("b.parts", "\"/zz/demo/t1/s8/b\"", "/zz/demo/t1/s8/b"),
        ("colors", "\"/zz/demo/t1/s3\"", "/zz/demo/t1/s3"),
    ];
    let mut decls = vec![
        "import \"/zz/notes/b\" as b".to_owned(),
        "import \"/zz/notes/c\" { gadget }".to_owned(),
        "citation widget -> \"/zz/demo/t1/s8/a\"".to_owned(),
        "citation colors -> \"/zz/demo/t1/s3\"".to_owned(),
    ];
    for (n, (name, address, at)) in pairs.iter().enumerate() {
        decls.push(bind(&format!("by_name_{n}"), name, at));
        decls.push(bind(&format!("by_address_{n}"), address, at));
    }
    let decls: Vec<&str> = decls.iter().map(String::as_str).collect();
    let a = note_module("a", &decls);
    let scratch = against(
        "stale-by-name",
        "v2",
        &v2,
        &[("a", &a), ("b", &b), ("c", &c)],
    );
    let report = check(&scratch.join("notes"));
    assert_eq!(
        stale_in(&report, &a),
        named(&[
            ("by_name_0", StaleReason::TextChanged),
            ("by_address_0", StaleReason::TextChanged),
            ("by_name_1", StaleReason::TextChanged),
            ("by_address_1", StaleReason::TextChanged),
            ("by_name_2", StaleReason::Removed),
            ("by_address_2", StaleReason::Removed),
        ])
    );
    assert_eq!(report.diagnostics.len(), 6, "{report:#?}");
    // A binding by a name says what one by its address says, but its name.
    for pair in report.diagnostics.chunks(2) {
        let [by_name, by_address] = pair else {
            panic!("the warnings come in pairs: {pair:#?}");
        };
        let n = by_name
            .message
            .split('`')
            .nth(1)
            .and_then(|name| name.strip_prefix("by_name_"))
            .expect("a binding by a name");
        assert_eq!(
            by_name
                .message
                .replacen(&format!("by_name_{n}"), &format!("by_address_{n}"), 1),
            by_address.message
        );
    }
}

#[test]
fn a_dependency_that_failed_makes_no_binding_stale() {
    // The text of `s8/a` changed, and the pin names another release: the
    // one diagnostic of the dependency is the report.
    let mut v2 = v1();
    reword(
        &mut v2,
        "/zz/demo/t1/s8/a",
        "An invented subsection.",
        "An invented clause.",
    );
    let a = binding_module(&BOUND);
    let scratch = Scratch::new("stale-failed");
    let digest = write_release(&scratch.join("demo"), "v2", &v2);
    let notes = scratch.join("notes");
    depending(
        &notes,
        vec![dependency("demo", "../demo", "v9", digest)],
        &[("a", &a)],
    );
    let report = check(&notes);
    about_the_manifest(&report, DiagnosticCode::E740, &["`v9`", "`v2`"]);

    // And from the interfaces, whether the prefix of the dependency that
    // failed is known or not.
    for prefix in [Some(address("/zz/demo")), None] {
        let phase_two = check_interfaces(
            &[interface_of_text(&a)],
            &[DependencyInterfaces::Failed { prefix }],
            &notes_manifest(),
            TOOLCHAIN,
        );
        assert_eq!(phase_two.diagnostics, []);
    }
}
