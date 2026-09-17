//! Compile+run pipeline bench for a tiny determinate module.
//!
//! Correctness is checked before timing. Uses in-memory `compile_source`
//! (empty manifest), the mill paste path.

use criterion::{Criterion, criterion_group, criterion_main};
use fidryn_cli::compile_source;
use fidryn_core::{CaseRecord, Instant, Outcome, QueryName, RunContext, SourceManifest, Value};
use fidryn_eval::evaluate;
use fidryn_handlers::CaseFile;
use std::collections::BTreeMap;
use std::hint::black_box;

const SRC: &str = r#"
module Bench version "0.1.0" {
    query q() -> Bool { return true }
}
"#;

fn compile_and_run() -> Outcome<Value> {
    let module =
        compile_source(SRC, &SourceManifest::default()).expect("compile return-true module");
    let case = CaseRecord::default();
    let t = Instant::parse("2033-01-01T00:00:00Z").expect("fixture timestamp");
    let ctx = RunContext::new(t, t);
    let mut handler = CaseFile {
        record: case.clone(),
        known_at: Some(ctx.record_time),
    };
    evaluate(
        &module,
        &QueryName::from("q"),
        &BTreeMap::new(),
        &case.into_state(),
        &ctx,
        &mut handler,
        &case,
    )
    .expect("evaluate return-true module")
}

fn assert_determinate_true(outcome: &Outcome<Value>) {
    match outcome {
        Outcome::Determinate {
            value: Value::Bool(true),
            ..
        } => {}
        other => panic!("expected Determinate true, got {other:?}"),
    }
}

fn pipeline(c: &mut Criterion) {
    assert_determinate_true(&compile_and_run());

    c.bench_function("compile_and_run_return_true", |b| {
        b.iter(|| {
            let outcome = compile_and_run();
            assert_determinate_true(&outcome);
            black_box(outcome)
        });
    });
}

criterion_group!(benches, pipeline);
criterion_main!(benches);
