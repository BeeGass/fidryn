//! Change sets. Each test writes two generated workspaces of the prefix
//! `/zz/demo` into a scratch directory, the older of the release `v1` in
//! `old/` and the newer of the release `v2` in `new/`, and compares them.
//! Most start from the models of the tests of staleness ([`v1`]); the
//! expected change set of each is derived from the rules of change sets by
//! hand. Every text is invented.

use super::staleness::{BOUND, S8, binding_module, child, find_mut, note_of, reword, s8_of, v1};
use super::*;
use crate::workspace::changes::{change_set, compare};
use fidryn_provision::changes::{ChangeSet, Moved, Release, StaleBinding};
use fidryn_provision::{Cell, Row, RowGroup, StaleReason, Table};

/// The old and the new workspace of a test, in `old/` and `new/` of a
/// scratch directory.
struct Pair {
    scratch: Scratch,
}

impl Pair {
    fn older(&self) -> PathBuf {
        self.scratch.join("old")
    }

    fn newer(&self) -> PathBuf {
        self.scratch.join("new")
    }
}

/// Writes `old` as the release `v1` and `new` as the release `v2`.
fn pair(tag: &str, old: &[Module], new: &[Module]) -> Pair {
    let scratch = Scratch::new(tag);
    write_release(&scratch.join("old"), "v1", old);
    write_release(&scratch.join("new"), "v2", new);
    Pair { scratch }
}

/// The change set of `pair` with the bindings of the curated workspaces at
/// `bindings`, which one worker, four and eight find alike, to the byte.
fn changes_of(pair: &Pair, bindings: &[PathBuf]) -> ChangeSet {
    let run = |jobs: usize| {
        change_set(&pair.older(), &pair.newer(), bindings, &with_jobs(jobs))
            .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"))
    };
    let one = run(1);
    for jobs in [4, 8] {
        let other = run(jobs);
        assert_eq!(other, one, "{jobs} workers");
        assert_eq!(other.to_json(), one.to_json(), "{jobs} workers");
        assert_eq!(other.to_text(), one.to_text(), "{jobs} workers");
    }
    one
}

/// The pair of [`v1`] and of [`v1`] changed by `change`, and its change set.
fn from_v1(tag: &str, change: impl FnOnce(&mut Vec<Module>)) -> (Pair, ChangeSet) {
    let mut new = v1();
    change(&mut new);
    let pair = pair(tag, &v1(), &new);
    let found = changes_of(&pair, &[]);
    (pair, found)
}

/// The change set of `pair` in which nothing changed: its prefix, and the
/// releases and the digests of the locks of its two sides.
fn nothing(pair: &Pair) -> ChangeSet {
    ChangeSet::empty(
        address("/zz/demo"),
        Release {
            release: "v1".to_owned(),
            lock_digest: lock_digest_of(&pair.older()),
        },
        Release {
            release: "v2".to_owned(),
            lock_digest: lock_digest_of(&pair.newer()),
        },
    )
}

fn at(addresses: &[&str]) -> Vec<Address> {
    addresses.iter().map(|text| address(text)).collect()
}

fn moved(from: &str, to: &str) -> Moved {
    Moved {
        from: address(from),
        to: address(to),
    }
}

/// The address of the note `name` of the group `demoNotes` of the module at
/// `module`, which cites the law `law`.
fn note_at(module: &str, law: &str, name: &str) -> String {
    format!("{module}/note/demoNotes/{law}/{name}")
}

/// The unit of `models` at `at`.
fn unit_named<'a>(models: &'a mut [Module], at: &str) -> &'a mut UnitModule {
    models
        .iter_mut()
        .find_map(|model| match model {
            Module::Unit(unit) if unit.address.as_str() == at => Some(unit),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no unit at {at}"))
}

/// The container of `models` at `at`.
fn container_named<'a>(models: &'a mut [Module], at: &str) -> &'a mut ContainerModule {
    models
        .iter_mut()
        .find_map(|model| match model {
            Module::Container(container) if container.address.as_str() == at => Some(container),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no container at {at}"))
}

/// Takes the module at `at` out of `models`, and out of the members of the
/// container at `within`.
fn take_out(models: &mut Vec<Module>, at: &str, within: &str) {
    container_named(models, within)
        .members
        .retain(|member| member.as_str() != at);
    let before = models.len();
    models.retain(|model| address_of(model).as_str() != at);
    assert_eq!(models.len(), before - 1, "no module at {at}");
}

/// Renumbers `unit` to `value`, at the address `to`: every address of it,
/// of its provisions and of its notes, that begins with its address begins
/// with `to`, and nothing else of it changes.
fn renumber(unit: &mut UnitModule, to: &str, value: &str) {
    fn walk(provision: &mut Provision, moved: &dyn Fn(&Address) -> Address) {
        provision.address = moved(&provision.address);
        for item in &mut provision.items {
            if let Item::Child(child) = item {
                walk(child, moved);
            }
        }
    }
    let from = unit.address.as_str().to_owned();
    let moved = |at: &Address| address(&format!("{to}{}", &at.as_str()[from.len()..]));
    unit.address = moved(&unit.address);
    walk(&mut unit.root, &moved);
    for note in &mut unit.notes {
        note.address = moved(&note.address);
    }
    unit.root.num = num(value, &format!("§ {value}."));
}

/// A paragraph of `words` followed by a footnote marker shown as `shown`
/// that points at the footnote `footnote`.
fn marked(words: &str, shown: &str, footnote: u32) -> Block {
    Block::Para(Text::from_runs(vec![
        Run::Words(words.to_owned()),
        Run::Mark {
            shown: shown.to_owned(),
            footnote,
        },
    ]))
}

#[test]
fn a_footnote_marker_added_to_a_provision_changes_only_its_footnotes() {
    let (pair, found) = from_v1("marker", |models| {
        let s8 = s8_of(models);
        let a = find_mut(&mut s8.root, "/zz/demo/t1/s8/a").expect("s8/a");
        // The words of the subsection, with a marker after them.
        a.items = vec![Item::Part {
            role: PartRole::Content,
            blocks: vec![marked("An invented subsection.", "2", 2)],
        }];
        s8.footnotes.push(Footnote {
            ordinal: 2,
            shown: "2".to_owned(),
            blocks: vec![Block::Para(plain("A second invented footnote."))],
        });
    });
    let mut expected = nothing(&pair);
    expected.editorial.footnotes_changed = at(&[S8]);
    assert_eq!(found, expected);
}

#[test]
fn a_container_heading_changed_is_only_a_changed_container() {
    let (pair, found) = from_v1("container-heading", |models| {
        container_at(models, 1).heading = Some(plain("Gadgets"));
    });
    let mut expected = nothing(&pair);
    expected.containers.changed = at(&[CHAPTER]);
    assert_eq!(found, expected);
}

#[test]
fn a_unit_added_is_added_and_its_container_changed() {
    let (pair, found) = from_v1("unit-added", |models| {
        container_at(models, 1)
            .members
            .push(address("/zz/demo/t1/s9"));
        models.push(Module::Unit(unit(
            "/zz/demo/t1/s9",
            Some(CHAPTER),
            "9",
            "Wheels",
            "A widget may have wheels.",
        )));
    });
    let mut expected = nothing(&pair);
    expected.operative.added = at(&["/zz/demo/t1/s9"]);
    expected.containers.changed = at(&[CHAPTER]);
    assert_eq!(found, expected);
}

#[test]
fn a_unit_removed_is_removed_and_its_container_changed() {
    let (pair, found) = from_v1("unit-removed", |models| {
        take_out(models, "/zz/demo/t1/s2", CHAPTER);
    });
    let mut expected = nothing(&pair);
    expected.operative.removed = at(&["/zz/demo/t1/s2"]);
    expected.containers.changed = at(&[CHAPTER]);
    assert_eq!(found, expected);
}

#[test]
fn a_provision_added_with_children_is_listed_at_the_highest_only() {
    let (pair, found) = from_v1("provision-added", |models| {
        let mut c = child(S8, "c", "A third invented subsection holds two paragraphs:");
        if let Item::Child(c) = &mut c {
            c.items
                .push(child("/zz/demo/t1/s8/c", "1", "the first; and"));
            c.items.push(child("/zz/demo/t1/s8/c", "2", "the second."));
        }
        s8_of(models).root.items.push(c);
    });
    // The section gains a child: its children's addresses are its own.
    let mut expected = nothing(&pair);
    expected.operative.added = at(&["/zz/demo/t1/s8/c"]);
    expected.operative.amended_structure = at(&[S8]);
    assert_eq!(found, expected);
}

#[test]
fn a_word_changed_is_amended_text_of_its_provision_and_of_no_ancestor() {
    let (pair, found) = from_v1("word", |models| {
        reword(
            models,
            "/zz/demo/t1/s8/a",
            "An invented subsection.",
            "An invented clause.",
        );
    });
    let mut expected = nothing(&pair);
    expected.operative.amended_text = at(&["/zz/demo/t1/s8/a"]);
    assert_eq!(found, expected);
}

#[test]
fn a_heading_changed_is_amended_structure() {
    let (pair, found) = from_v1("heading", |models| {
        let b = find_mut(&mut s8_of(models).root, "/zz/demo/t1/s8/b").expect("s8/b");
        b.heading = Some(plain("Other Parts"));
    });
    let mut expected = nothing(&pair);
    expected.operative.amended_structure = at(&["/zz/demo/t1/s8/b"]);
    assert_eq!(found, expected);
}

#[test]
fn a_status_added_is_amended_structure() {
    let (pair, found) = from_v1("status", |models| {
        unit_at(models, 4).root.status = Some("repealed".to_owned());
    });
    let mut expected = nothing(&pair);
    expected.operative.amended_structure = at(&["/zz/demo/t1/s3"]);
    assert_eq!(found, expected);
}

#[test]
fn two_children_swapped_are_amended_structure_of_their_parent() {
    let (pair, found) = from_v1("swapped", |models| {
        s8_of(models).root.items.swap(1, 2);
    });
    let mut expected = nothing(&pair);
    expected.operative.amended_structure = at(&[S8]);
    assert_eq!(found, expected);
}

/// A cell of a table that says `words`, or that holds no block when they
/// are empty.
fn cell(header: bool, words: &str) -> Cell {
    Cell {
        header,
        colspan: 1,
        rowspan: 1,
        blocks: if words.is_empty() {
            Vec::new()
        } else {
            vec![Block::Para(plain(words))]
        },
    }
}

/// [`v1`] with a table in the section `s2`: a row of headers and a row of
/// values, one column for each `(header, value)` of `columns`.
fn with_table(columns: &[(&str, &str)]) -> Vec<Module> {
    let table = Table {
        rows: vec![
            Row {
                group: RowGroup::Head,
                cells: columns.iter().map(|(head, _)| cell(true, head)).collect(),
            },
            Row {
                group: RowGroup::Body,
                cells: columns.iter().map(|(_, body)| cell(false, body)).collect(),
            },
        ],
    };
    let mut models = v1();
    unit_at(&mut models, 3).root.items = vec![Item::Part {
        role: PartRole::Content,
        blocks: vec![
            Block::Para(plain("A widget is small or large.")),
            Block::Table(table),
        ],
    }];
    models
}

#[test]
fn a_column_added_to_a_table_is_amended_structure() {
    let s2 = "/zz/demo/t1/s2";
    let two = [("Size", "small"), ("Weight", "1 gram")];
    // A column of cells that hold nothing: the words of the table are the
    // same, and its shape is not.
    let empty = pair(
        "column-empty",
        &with_table(&two),
        &with_table(&[two[0], two[1], ("", "")]),
    );
    let mut expected = nothing(&empty);
    expected.operative.amended_structure = at(&[s2]);
    assert_eq!(changes_of(&empty, &[]), expected);

    // A column whose cells hold words changes both.
    let worded = pair(
        "column-words",
        &with_table(&two),
        &with_table(&[two[0], two[1], ("Color", "red")]),
    );
    let mut expected = nothing(&worded);
    expected.operative.amended_text = at(&[s2]);
    expected.operative.amended_structure = at(&[s2]);
    assert_eq!(changes_of(&worded, &[]), expected);
}

#[test]
fn a_word_and_a_heading_changed_in_one_provision_are_in_both_lists() {
    let s2 = "/zz/demo/t1/s2";
    let (pair, found) = from_v1("word-and-heading", |models| {
        unit_at(models, 3).root.heading = Some(plain("Measures"));
        reword(models, s2, "small or large", "tiny or large");
    });
    let mut expected = nothing(&pair);
    expected.operative.amended_text = at(&[s2]);
    expected.operative.amended_structure = at(&[s2]);
    assert_eq!(found, expected);
}

#[test]
fn a_section_renumbered_with_its_text_unchanged_has_moved() {
    let s9 = "/zz/demo/t1/s9";
    let (pair, found) = from_v1("renumbered", |models| {
        renumber(s8_of(models), s9, "9");
        for member in &mut container_at(models, 1).members {
            if member.as_str() == S8 {
                *member = address(s9);
            }
        }
    });
    // The section moved with its subsections, which are not listed; its
    // notes moved with it, each paired by its note hash.
    let mut expected = nothing(&pair);
    expected.operative.moved = vec![moved(S8, s9)];
    expected.enacted_notes.moved = vec![moved(
        &note_at(S8, "nolaw", "effective"),
        &note_at(s9, "nolaw", "effective"),
    )];
    expected.editorial.notes.moved = vec![moved(
        &note_at(S8, "nolaw", "codification"),
        &note_at(s9, "nolaw", "codification"),
    )];
    expected.containers.changed = at(&[CHAPTER]);
    assert_eq!(found, expected);
}

#[test]
fn a_note_added_is_added() {
    let (pair, found) = from_v1("note-added", |models| {
        s8_of(models).notes.push(note_of(
            S8,
            "amendment",
            Layer::Enacted,
            "An invented act amended this section.",
        ));
    });
    let mut expected = nothing(&pair);
    expected.enacted_notes.added = at(&[&note_at(S8, "nolaw", "amendment")]);
    assert_eq!(found, expected);
}

#[test]
fn a_note_removed_is_removed() {
    let (pair, found) = from_v1("note-removed", |models| {
        let removed = s8_of(models).notes.remove(1);
        assert_eq!(removed.layer, Layer::Editorial);
    });
    let mut expected = nothing(&pair);
    expected.editorial.notes.removed = at(&[&note_at(S8, "nolaw", "codification")]);
    assert_eq!(found, expected);
}

#[test]
fn a_note_amended_is_amended() {
    let (pair, found) = from_v1("note-amended", |models| {
        s8_of(models).notes[0].blocks = vec![Block::Para(plain(
            "An invented act took effect on another invented day.",
        ))];
    });
    let mut expected = nothing(&pair);
    expected.enacted_notes.amended = at(&[&note_at(S8, "nolaw", "effective")]);
    assert_eq!(found, expected);
}

#[test]
fn a_note_whose_cited_law_changed_and_whose_text_did_not_has_moved() {
    let (pair, found) = from_v1("note-moved", |models| {
        s8_of(models).notes[0].address = address(&note_at(S8, "pl.42.7", "effective"));
    });
    let mut expected = nothing(&pair);
    expected.enacted_notes.moved = vec![moved(
        &note_at(S8, "nolaw", "effective"),
        &note_at(S8, "pl.42.7", "effective"),
    )];
    assert_eq!(found, expected);
}

#[test]
fn a_credit_changed_is_a_changed_credit() {
    let (pair, found) = from_v1("credit", |models| {
        s8_of(models).credit = Some(plain("(Added by another invented act.)"));
    });
    let mut expected = nothing(&pair);
    expected.editorial.credits_changed = at(&[S8]);
    assert_eq!(found, expected);
}

#[test]
fn a_footnote_changed_is_a_changed_footnote() {
    let (pair, found) = from_v1("footnote", |models| {
        s8_of(models).footnotes[0].blocks =
            vec![Block::Para(plain("A revised invented footnote."))];
    });
    let mut expected = nothing(&pair);
    expected.editorial.footnotes_changed = at(&[S8]);
    assert_eq!(found, expected);
}

#[test]
fn a_member_added_to_a_container_changes_the_containers_only() {
    // The section `s3` leaves the chapter for the title; nothing of its own
    // changes.
    let (pair, found) = from_v1("member", |models| {
        container_at(models, 0)
            .members
            .push(address("/zz/demo/t1/s3"));
        container_at(models, 1)
            .members
            .retain(|member| member.as_str() != "/zz/demo/t1/s3");
        unit_at(models, 4).within = Some(address("/zz/demo/t1"));
    });
    let mut expected = nothing(&pair);
    expected.containers.changed = at(&["/zz/demo/t1", CHAPTER]);
    assert_eq!(found, expected);
}

#[test]
fn a_reissued_title_has_an_empty_change_set() {
    let pair = pair("reissue", &v1(), &v1());
    assert_ne!(lock_digest_of(&pair.older()), lock_digest_of(&pair.newer()));
    assert_eq!(changes_of(&pair, &[]), nothing(&pair));
    let compared = compare(&pair.older(), &pair.newer(), &[], &with_jobs(2)).expect("a change set");
    assert_eq!(compared.opened, 0);
}

#[test]
fn only_the_modules_whose_hashes_differ_are_opened() {
    let opened = |tag: &str, change: &dyn Fn(&mut Vec<Module>)| {
        let mut new = v1();
        change(&mut new);
        let pair = pair(tag, &v1(), &new);
        compare(&pair.older(), &pair.newer(), &[], &with_jobs(3))
            .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"))
            .opened
    };
    // A note: the interfaces hold every note's address and hash.
    assert_eq!(
        opened("open-note", &|models| {
            s8_of(models).notes[0].blocks = vec![Block::Para(plain("Another invented day."))];
        }),
        0
    );
    // A container's heading: its hash is in the interfaces.
    assert_eq!(
        opened("open-container", &|models| {
            container_at(models, 1).heading = Some(plain("Gadgets"));
        }),
        0
    );
    // A unit added, with nothing removed that it could have moved from.
    assert_eq!(
        opened("open-added", &|models| {
            container_at(models, 1)
                .members
                .push(address("/zz/demo/t1/s9"));
            models.push(section(9, CHAPTER));
        }),
        0
    );
    // A word of a provision: the unit, on each side.
    assert_eq!(
        opened("open-word", &|models| {
            reword(models, "/zz/demo/t1/s1", "a small device", "a tiny device");
        }),
        2
    );
    // A credit: the unit, on each side.
    assert_eq!(
        opened("open-credit", &|models| {
            s8_of(models).credit = Some(plain("(Another invented act.)"));
        }),
        2
    );
    // A unit removed and one added: each, to compare them with the number
    // left out.
    assert_eq!(
        opened("open-renumbered", &|models| {
            renumber(s8_of(models), "/zz/demo/t1/s9", "9");
            for member in &mut container_at(models, 1).members {
                if member.as_str() == S8 {
                    *member = address("/zz/demo/t1/s9");
                }
            }
        }),
        2
    );
}

/// The pair of [`v1`] and of the second version of the dependency of the
/// tests of staleness, and in `notes/` beside them the curated workspace
/// `notes`, whose module `/zz/notes/a` binds [`BOUND`] and two addresses
/// below another prefix, written against [`v1`]. The second version changes
/// one word of `s8/a`, renumbers `s8/b` to `s8/c`, and adds an enacted note
/// to `s3`, whose alias is `s4`.
fn stale_fixture(tag: &str) -> (Pair, PathBuf) {
    let mut v2 = v1();
    reword(
        &mut v2,
        "/zz/demo/t1/s8/a",
        "An invented subsection.",
        "An invented clause.",
    );
    let b = find_mut(&mut s8_of(&mut v2).root, "/zz/demo/t1/s8/b").expect("s8/b");
    b.address = address("/zz/demo/t1/s8/c");
    b.num = num("c", "(c)");
    unit_at(&mut v2, 4).notes.push(note_of(
        "/zz/demo/t1/s3",
        "amendment",
        Layer::Enacted,
        "An invented act amended this section.",
    ));
    let pair = pair(tag, &v1(), &v2);
    // Bindings written against the first version, and two whose addresses
    // lie below another prefix, which are left out.
    let mut bound = BOUND.to_vec();
    bound.extend([("own", "/zz/notes/a"), ("outside", "/zz/other/x")]);
    let a = binding_module(&bound);
    // Pinned to a release that neither side is, in a directory that is not
    // there: a binding is judged against the new side whatever its
    // workspace is pinned to.
    let notes = pair.scratch.join("notes");
    depending(
        &notes,
        vec![dependency("demo", "../nowhere", "v0", Hash32([0; 32]))],
        &[("a", &a)],
    );
    (pair, notes)
}

/// The stale bindings of the curated workspace of [`stale_fixture`].
fn stale_of_fixture() -> Vec<StaleBinding> {
    [
        (
            "aliased",
            "/zz/demo/t1/s4",
            StaleReason::EnactedNotesChanged,
        ),
        ("first", "/zz/demo/t1/s8/a", StaleReason::TextChanged),
        ("second", "/zz/demo/t1/s8/b", StaleReason::Removed),
        ("section", S8, StaleReason::TextChanged),
    ]
    .into_iter()
    .map(|(binding, bound, reason)| StaleBinding {
        workspace: "notes".to_owned(),
        module: address("/zz/notes/a"),
        binding: binding.to_owned(),
        address: address(bound),
        reason,
    })
    .collect()
}

#[test]
fn stale_bindings_are_listed() {
    let (pair, notes) = stale_fixture("stale");
    let found = changes_of(&pair, std::slice::from_ref(&notes));

    let mut expected = nothing(&pair);
    expected.operative.amended_text = at(&["/zz/demo/t1/s8/a"]);
    expected.operative.amended_structure = at(&[S8]);
    expected.operative.moved = vec![moved("/zz/demo/t1/s8/b", "/zz/demo/t1/s8/c")];
    expected.enacted_notes.added = at(&[&note_at("/zz/demo/t1/s3", "nolaw", "amendment")]);
    expected.stale_bindings = stale_of_fixture();
    assert_eq!(found, expected);
    // The commit message that the section "Change sets" of
    // `docs/law-modules.md` shows.
    assert_eq!(
        found.to_text(),
        "/zz/demo: v1 to v2\n\
         \n\
         operative: 0 added, 0 removed, 1 amended_text, 1 amended_structure, 1 moved\n\
         enacted_notes: 1 added, 0 removed, 0 amended, 0 moved\n\
         editorial: 0 added, 0 removed, 0 amended, 0 moved, 0 credits_changed, \
         0 footnotes_changed\n\
         containers: 0 added, 0 removed, 0 changed\n\
         \n\
         operative.amended_text:\n\
         /zz/demo/t1/s8/a\n\
         \n\
         operative.amended_structure:\n\
         /zz/demo/t1/s8\n\
         \n\
         operative.moved:\n\
         /zz/demo/t1/s8/b to /zz/demo/t1/s8/c\n\
         \n\
         enacted_notes.added:\n\
         /zz/demo/t1/s3/note/demoNotes/nolaw/amendment\n\
         \n\
         stale_bindings:\n\
         /zz/demo/t1/s4: aliased of /zz/notes/a in notes (enacted notes of the unit changed)\n\
         /zz/demo/t1/s8/a: first of /zz/notes/a in notes (provision text changed)\n\
         /zz/demo/t1/s8/b: second of /zz/notes/a in notes (provision removed or moved)\n\
         /zz/demo/t1/s8: section of /zz/notes/a in notes (provision text changed)\n"
    );

    // Without the bindings, the same change set with none.
    expected.stale_bindings.clear();
    assert_eq!(changes_of(&pair, &[]), expected);
}

#[test]
fn a_workspace_of_bindings_named_twice_is_read_once() {
    let (pair, notes) = stale_fixture("stale-twice");
    let once = changes_of(&pair, std::slice::from_ref(&notes));
    assert_eq!(once.stale_bindings, stale_of_fixture());
    // The same directory, as given and by two other paths that lead to it.
    let named = [notes.clone(), notes.join("."), notes.join("../notes")];
    assert_eq!(changes_of(&pair, &named), once);
    // A copy of the workspace in another directory is another workspace of
    // bindings, whose entries are those of the first: each is listed once.
    let copy = pair.scratch.join("copy");
    for rel in ["workspace.json", "zz/notes/a.fr"] {
        put(&copy, rel, &read(&notes, rel));
    }
    let both = changes_of(&pair, &[notes, copy]);
    assert_eq!(both, once);
    let value: serde_json::Value = serde_json::from_str(&both.to_json()).expect("JSON");
    assert_eq!(
        value["stale_bindings"].as_array().map(Vec::len),
        Some(stale_of_fixture().len())
    );
}

/// The address of the unit `s<n>`.
fn s(n: u32) -> String {
    format!("/zz/demo/t1/s{n}")
}

/// The unit `s<n>` of the container at `within`, headed `Invented Rule <n>`,
/// whose words name `n`.
fn section(n: u32, within: &str) -> Module {
    Module::Unit(unit(
        &s(n),
        Some(within),
        &n.to_string(),
        &format!("Invented Rule {n}"),
        &format!("Invented rule {n} applies to every widget."),
    ))
}

/// The chapter at `at` of the title, numbered `n`, that lists `members`.
fn chapter(at: &str, n: &str, members: &[&str]) -> Module {
    let mut chapter = container(
        at,
        Some("/zz/demo/t1"),
        "chapter",
        &format!("Chapter {n}—"),
        &format!("Invented Chapter {n}"),
        members,
    );
    chapter.num = num(n, &format!("Chapter {n}—"));
    Module::Container(chapter)
}

/// The notes `e<n>` (enacted) and `d<n>` (editorial) of the unit `s1` for
/// each `n` of `numbers`, each of its own words.
fn notes_of_s1(numbers: std::ops::RangeInclusive<u32>) -> Vec<Note> {
    numbers
        .flat_map(|n| {
            [
                note_of(
                    &s(1),
                    &format!("e{n}"),
                    Layer::Enacted,
                    &format!("Invented enacted note {n}."),
                ),
                note_of(
                    &s(1),
                    &format!("d{n}"),
                    Layer::Editorial,
                    &format!("Invented editorial note {n}."),
                ),
            ]
        })
        .collect()
}

/// The old side of the pair in which every list of the change set holds
/// more than one entry: [`v1`], with the units `s10` to `s17` in the
/// chapter, the chapters `ch2` and `ch5` holding `s18` and `s19`, a
/// subsection of `s12`, twelve notes and a credit of `s1`.
fn rich_old() -> Vec<Module> {
    let mut models = v1();
    for n in 10..=17 {
        container_at(&mut models, 1).members.push(address(&s(n)));
        models.push(section(n, CHAPTER));
    }
    let s12 = unit_named(&mut models, &s(12));
    s12.root
        .items
        .push(child(&s(12), "a", "An invented subsection of rule 12."));
    for (n, at, member) in [("2", "/zz/demo/t1/ch2", 18), ("5", "/zz/demo/t1/ch5", 19)] {
        container_at(&mut models, 0).members.push(address(at));
        models.push(chapter(at, n, &[&s(member)]));
        models.push(section(member, at));
    }
    let s1 = unit_at(&mut models, 2);
    s1.notes = notes_of_s1(1..=6);
    s1.credit = Some(plain("(Added by an invented act.)"));
    models
}

/// The new side of the pair of [`rich_old`].
fn rich_new() -> Vec<Module> {
    let mut models = rich_old();
    // Removed: two units of the chapter, and two chapters with their units.
    take_out(&mut models, &s(10), CHAPTER);
    take_out(&mut models, &s(11), CHAPTER);
    for (at, member) in [("/zz/demo/t1/ch2", 18), ("/zz/demo/t1/ch5", 19)] {
        take_out(&mut models, &s(member), at);
        take_out(&mut models, at, "/zz/demo/t1");
    }
    // Amended: words of `s12/a` and `s13`, a subsection added to `s12`, the
    // heading of `s14`, the status of `s15`.
    reword(&mut models, &format!("{}/a", s(12)), "subsection", "clause");
    reword(&mut models, &s(13), "every widget", "every gadget");
    unit_named(&mut models, &s(12)).root.items.push(child(
        &s(12),
        "b",
        "A second invented subsection of rule 12.",
    ));
    unit_named(&mut models, &s(14)).root.heading = Some(plain("Invented Rule Fourteen"));
    unit_named(&mut models, &s(15)).root.status = Some("repealed".to_owned());
    // Renumbered: `s16` and `s17` to `s26` and `s27`.
    for (from, to) in [(16, 26), (17, 27)] {
        renumber(unit_named(&mut models, &s(from)), &s(to), &to.to_string());
        for member in &mut container_at(&mut models, 1).members {
            if *member == address(&s(from)) {
                *member = address(&s(to));
            }
        }
    }
    // Added: two units of the chapter, and two chapters with a unit each.
    for n in [20, 21] {
        container_at(&mut models, 1).members.push(address(&s(n)));
        models.push(section(n, CHAPTER));
    }
    for (n, at, member) in [("3", "/zz/demo/t1/ch3", 30), ("4", "/zz/demo/t1/ch4", 31)] {
        container_at(&mut models, 0).members.push(address(at));
        models.push(chapter(at, n, &[&s(member)]));
        models.push(section(member, at));
    }
    // The notes of `s1`: the first two of each layer removed, the next two
    // amended, the two after them moved to another law, and two more added;
    // its credit changed, and a credit given to `s2`.
    let s1 = unit_at(&mut models, 2);
    let mut notes = notes_of_s1(3..=8);
    for note in &mut notes {
        let name = note.address.as_str().rsplit('/').next().expect("a name");
        match &name[1..] {
            "3" | "4" => {
                note.blocks = vec![Block::Para(plain(&format!("An amended note {name}.")))];
            }
            "5" | "6" => note.address = address(&note_at(&s(1), "pl.42.8", name)),
            _ => {}
        }
    }
    s1.notes = notes;
    s1.credit = Some(plain("(Added by another invented act.)"));
    unit_at(&mut models, 3).credit = Some(plain("(Added by an invented act.)"));
    // Footnotes: one of `s8` changed, and one added to `s3` with its marker.
    s8_of(&mut models).footnotes[0].blocks = vec![Block::Para(plain("A revised footnote."))];
    let s3 = unit_at(&mut models, 4);
    s3.root.items = vec![Item::Part {
        role: PartRole::Content,
        blocks: vec![marked("A widget may be of any color.", "1", 1)],
    }];
    s3.footnotes.push(Footnote {
        ordinal: 1,
        shown: "1".to_owned(),
        blocks: vec![Block::Para(plain("An invented footnote of s3."))],
    });
    models
}

/// The curated workspace `atlas` of the prefix `/zz/atlas` in `out`, with
/// no dependency and the one module `/zz/atlas/b`, which declares `decls`.
fn atlas(out: &Path, decls: &[String]) {
    fs::create_dir_all(out).expect("create the workspace's directory");
    let manifest = Manifest {
        name: "atlas".to_owned(),
        prefix: address("/zz/atlas"),
        ..curated_manifest()
    };
    fs::write(out.join("workspace.json"), manifest.to_json()).expect("write");
    let mut text = "edition 2\nmodule \"/zz/atlas/b\" {\n".to_owned();
    for line in decls.iter().flat_map(|decl| decl.lines()) {
        text.push_str("  ");
        text.push_str(line);
        text.push('\n');
    }
    text.push_str("}\n");
    put(out, "zz/atlas/b.fr", &text);
}

/// The pair of [`rich_old`] and [`rich_new`], and two curated workspaces
/// whose bindings are written against the old side, `notes/` and
/// `atlas/`; and its change set.
fn rich() -> (Pair, ChangeSet) {
    let old = rich_old();
    let pair = pair("rich", &old, &rich_new());
    let bind = |name: &str, at: &str| {
        super::staleness::written_against(name, &format!("\"{at}\""), "v1", &old, at)
    };
    let notes = pair.scratch.join("notes");
    let decls = [
        bind("thirteen", &s(13)),
        bind("ten", &s(10)),
        bind("one", &s(1)),
        bind("two", &s(2)),
    ];
    let decls: Vec<&str> = decls.iter().map(String::as_str).collect();
    let a = note_module("a", &decls);
    depending(&notes, Vec::new(), &[("a", &a)]);
    let atlas_dir = pair.scratch.join("atlas");
    atlas(
        &atlas_dir,
        &[
            bind("twelve_a", &format!("{}/a", s(12))),
            bind("fourteen", &s(14)),
        ],
    );
    let found = changes_of(&pair, &[notes, atlas_dir]);
    (pair, found)
}

#[test]
fn every_list_of_a_change_set_holds_what_changed() {
    let (pair, found) = rich();
    let mut expected = nothing(&pair);
    let operative = &mut expected.operative;
    operative.added = at(&[
        "/zz/demo/t1/s12/b",
        "/zz/demo/t1/s20",
        "/zz/demo/t1/s21",
        "/zz/demo/t1/s30",
        "/zz/demo/t1/s31",
    ]);
    operative.removed = at(&[
        "/zz/demo/t1/s10",
        "/zz/demo/t1/s11",
        "/zz/demo/t1/s18",
        "/zz/demo/t1/s19",
    ]);
    operative.amended_text = at(&["/zz/demo/t1/s12/a", "/zz/demo/t1/s13"]);
    operative.amended_structure = at(&["/zz/demo/t1/s12", "/zz/demo/t1/s14", "/zz/demo/t1/s15"]);
    operative.moved = vec![
        moved("/zz/demo/t1/s16", "/zz/demo/t1/s26"),
        moved("/zz/demo/t1/s17", "/zz/demo/t1/s27"),
    ];
    let s1 = s(1);
    for (notes, letter) in [
        (&mut expected.enacted_notes, "e"),
        (&mut expected.editorial.notes, "d"),
    ] {
        let named = |n: u32| note_at(&s1, "nolaw", &format!("{letter}{n}"));
        notes.added = at(&[&named(7), &named(8)]);
        notes.removed = at(&[&named(1), &named(2)]);
        notes.amended = at(&[&named(3), &named(4)]);
        notes.moved = [5, 6]
            .map(|n| moved(&named(n), &note_at(&s1, "pl.42.8", &format!("{letter}{n}"))))
            .to_vec();
    }
    expected.editorial.credits_changed = at(&[&s(1), &s(2)]);
    expected.editorial.footnotes_changed = at(&[&s(3), S8]);
    expected.containers.added = at(&["/zz/demo/t1/ch3", "/zz/demo/t1/ch4"]);
    expected.containers.removed = at(&["/zz/demo/t1/ch2", "/zz/demo/t1/ch5"]);
    expected.containers.changed = at(&["/zz/demo/t1", CHAPTER]);
    let stale = |workspace: &str, module: &str, binding: &str, bound: &str, reason| StaleBinding {
        workspace: workspace.to_owned(),
        module: address(module),
        binding: binding.to_owned(),
        address: address(bound),
        reason,
    };
    expected.stale_bindings = vec![
        stale(
            "atlas",
            "/zz/atlas/b",
            "fourteen",
            &s(14),
            StaleReason::TextChanged,
        ),
        stale(
            "atlas",
            "/zz/atlas/b",
            "twelve_a",
            "/zz/demo/t1/s12/a",
            StaleReason::TextChanged,
        ),
        stale(
            "notes",
            "/zz/notes/a",
            "one",
            &s(1),
            StaleReason::EnactedNotesChanged,
        ),
        stale("notes", "/zz/notes/a", "ten", &s(10), StaleReason::Removed),
        stale(
            "notes",
            "/zz/notes/a",
            "thirteen",
            &s(13),
            StaleReason::TextChanged,
        ),
    ];
    assert_eq!(found, expected);
}

/// Every list of the JSON `value`, by its key path.
fn lists_in(
    value: &serde_json::Value,
    path: &str,
    out: &mut Vec<(String, Vec<serde_json::Value>)>,
) {
    match value {
        serde_json::Value::Object(object) => {
            for (key, value) in object {
                lists_in(value, &format!("{path}.{key}"), out);
            }
        }
        serde_json::Value::Array(items) => out.push((path.to_owned(), items.clone())),
        _ => {}
    }
}

#[test]
fn every_list_is_sorted() {
    let (_pair, found) = rich();
    let value: serde_json::Value = serde_json::from_str(&found.to_json()).expect("JSON");
    let mut lists = Vec::new();
    lists_in(&value, "", &mut lists);
    assert_eq!(lists.len(), 19, "{lists:#?}");
    for (path, items) in lists {
        assert!(items.len() >= 2, "{path} has fewer than two entries");
        // The key each entry is sorted by: an address, a moved entry by the
        // address it moved from and then to, and a stale binding by its
        // workspace, its module and its name.
        let keys: Vec<Vec<String>> = items
            .iter()
            .map(|item| match item {
                serde_json::Value::String(text) => vec![text.clone()],
                serde_json::Value::Object(object) if path.ends_with(".moved") => ["from", "to"]
                    .iter()
                    .map(|key| object[*key].as_str().expect("an address").to_owned())
                    .collect(),
                serde_json::Value::Object(object) => ["workspace", "module", "binding"]
                    .iter()
                    .map(|key| object[*key].as_str().expect("a string").to_owned())
                    .collect(),
                other => panic!("{path}: {other}"),
            })
            .collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted, "{path}");
        sorted.dedup();
        assert_eq!(sorted.len(), keys.len(), "{path} holds an entry twice");
    }
}

/// The commit message of the change set of [`rich`].
const RICH_TEXT: &str = "\
/zz/demo: v1 to v2

operative: 5 added, 4 removed, 2 amended_text, 3 amended_structure, 2 moved
enacted_notes: 2 added, 2 removed, 2 amended, 2 moved
editorial: 2 added, 2 removed, 2 amended, 2 moved, 2 credits_changed, 2 footnotes_changed
containers: 2 added, 2 removed, 2 changed

operative.added:
/zz/demo/t1/s12/b
/zz/demo/t1/s20
/zz/demo/t1/s21
/zz/demo/t1/s30
/zz/demo/t1/s31

operative.removed:
/zz/demo/t1/s10
/zz/demo/t1/s11
/zz/demo/t1/s18
/zz/demo/t1/s19

operative.amended_text:
/zz/demo/t1/s12/a
/zz/demo/t1/s13

operative.amended_structure:
/zz/demo/t1/s12
/zz/demo/t1/s14
/zz/demo/t1/s15

operative.moved:
/zz/demo/t1/s16 to /zz/demo/t1/s26
/zz/demo/t1/s17 to /zz/demo/t1/s27

enacted_notes.added:
/zz/demo/t1/s1/note/demoNotes/nolaw/e7
/zz/demo/t1/s1/note/demoNotes/nolaw/e8

enacted_notes.removed:
/zz/demo/t1/s1/note/demoNotes/nolaw/e1
/zz/demo/t1/s1/note/demoNotes/nolaw/e2

enacted_notes.amended:
/zz/demo/t1/s1/note/demoNotes/nolaw/e3
/zz/demo/t1/s1/note/demoNotes/nolaw/e4

enacted_notes.moved:
/zz/demo/t1/s1/note/demoNotes/nolaw/e5 to /zz/demo/t1/s1/note/demoNotes/pl.42.8/e5
/zz/demo/t1/s1/note/demoNotes/nolaw/e6 to /zz/demo/t1/s1/note/demoNotes/pl.42.8/e6

editorial.added:
/zz/demo/t1/s1/note/demoNotes/nolaw/d7
/zz/demo/t1/s1/note/demoNotes/nolaw/d8

editorial.removed:
/zz/demo/t1/s1/note/demoNotes/nolaw/d1
/zz/demo/t1/s1/note/demoNotes/nolaw/d2

editorial.amended:
/zz/demo/t1/s1/note/demoNotes/nolaw/d3
/zz/demo/t1/s1/note/demoNotes/nolaw/d4

editorial.moved:
/zz/demo/t1/s1/note/demoNotes/nolaw/d5 to /zz/demo/t1/s1/note/demoNotes/pl.42.8/d5
/zz/demo/t1/s1/note/demoNotes/nolaw/d6 to /zz/demo/t1/s1/note/demoNotes/pl.42.8/d6

editorial.credits_changed:
/zz/demo/t1/s1
/zz/demo/t1/s2

editorial.footnotes_changed:
/zz/demo/t1/s3
/zz/demo/t1/s8

containers.added:
/zz/demo/t1/ch3
/zz/demo/t1/ch4

containers.removed:
/zz/demo/t1/ch2
/zz/demo/t1/ch5

containers.changed:
/zz/demo/t1
/zz/demo/t1/ch1

stale_bindings:
/zz/demo/t1/s14: fourteen of /zz/atlas/b in atlas (provision text changed)
/zz/demo/t1/s12/a: twelve_a of /zz/atlas/b in atlas (provision text changed)
/zz/demo/t1/s1: one of /zz/notes/a in notes (enacted notes of the unit changed)
/zz/demo/t1/s10: ten of /zz/notes/a in notes (provision removed or moved)
/zz/demo/t1/s13: thirteen of /zz/notes/a in notes (provision text changed)
";

#[test]
fn the_text_form_is_the_golden_commit_message() {
    let (_rich, found) = rich();
    let text = found.to_text();
    assert_eq!(text, RICH_TEXT);
    // A commit message: no line is a comment of git, none ends in a space,
    // and the text ends with one line end.
    for line in text.lines() {
        assert!(!line.starts_with('#'), "{line:?}");
        assert!(!line.ends_with(' '), "{line:?}");
    }
    assert!(text.ends_with('\n') && !text.ends_with("\n\n"));

    // When nothing changed: the first line, a blank line and the counts.
    let reissue = pair("reissue-text", &v1(), &v1());
    assert_eq!(
        changes_of(&reissue, &[]).to_text(),
        "/zz/demo: v1 to v2\n\n\
         operative: 0 added, 0 removed, 0 amended_text, 0 amended_structure, 0 moved\n\
         enacted_notes: 0 added, 0 removed, 0 amended, 0 moved\n\
         editorial: 0 added, 0 removed, 0 amended, 0 moved, 0 credits_changed, \
         0 footnotes_changed\n\
         containers: 0 added, 0 removed, 0 changed\n"
    );
}

#[test]
fn the_committed_change_set_is_what_the_comparison_writes() {
    let (_pair, found) = rich();
    let mut fresh = Scratch::new("change-set-fresh");
    put(fresh.path(), "change-set.json", &found.to_json());
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/fixtures/change-set");
    if !fixture.is_dir() || tree(fresh.path()) != tree(&fixture) {
        fresh.keep = true;
        panic!(
            "conformance/fixtures/change-set is not what the comparison writes of the rich pair \
             of the tests of change sets; what it writes is in {}",
            fresh.path().display()
        );
    }
}
