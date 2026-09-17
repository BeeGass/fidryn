//! Institutional covering fragment: `duty_status` and `require_authority`.
//!
//! Independent of [`fidryn_eval::evaluate`] and CaseFile handlers. Status is
//! computed from [`CoreDuty`] plus [`FrozenCaseView`] /
//! [`fidryn_eval::duty::surface_duty_state`] on the restricted overlay case
//! (admitted slots only, no fact overwrite). Event admission is the eval
//! helper [`fidryn_eval::duty::event_is_admitted`], used inside surface
//! status. `require_authority` without a grant is not determinate, matching
//! `require false`.

use fidryn_core::ir::{CoreDecl, CoreDuty, CoreModule};
use fidryn_core::{CaseRecord, FrozenCaseView, RunContext, Term, Value};
use fidryn_eval::DerivedWorld;
use fidryn_eval::duty::{
    DEFAULT_DUTY_INSTANCE, action_is_granted, duty_state_value, surface_duty_state,
};
use std::collections::BTreeMap;

/// Whether `term` is a covering-checkable institutional call.
///
/// [`Term::Apply`] / [`Term::Call`] of `duty_status(Name)` or
/// `duty_status(Name, Instance)`, or `require_authority(action)`, with
/// literal names. `duty_step`, Observe, UniqueOccupant, and `seq` wrapping
/// those remain outside this fragment.
pub fn is_institutional_fragment(term: &Term) -> bool {
    match term {
        Term::Apply { ctor, args } | Term::Call { callee: ctor, args } => {
            if is_duty_status(ctor) {
                duty_status_args_ok(args)
            } else if is_require_authority(ctor) {
                require_authority_args_ok(args)
            } else {
                false
            }
        }
        _ => false,
    }
}

/// Evaluate an institutional covering term against the overlay case.
///
/// Does not call [`fidryn_eval::evaluate`] and does not construct handlers.
/// Missing `require_authority` grants fail like `require false`.
pub fn eval_institutional(
    module: &CoreModule,
    term: &Term,
    case: &CaseRecord,
    ctx: &RunContext,
    args: &BTreeMap<String, Value>,
) -> Result<Value, String> {
    if !is_institutional_fragment(term) {
        return Err("term is not in the institutional covering fragment".into());
    }
    match term {
        Term::Apply { ctor, args: terms }
        | Term::Call {
            callee: ctor,
            args: terms,
        } => {
            if is_duty_status(ctor) {
                eval_duty_status(module, terms, case, ctx, args)
            } else if is_require_authority(ctor) {
                eval_require_authority(terms, case, ctx)
            } else {
                Err("term is not in the institutional covering fragment".into())
            }
        }
        _ => Err("term is not in the institutional covering fragment".into()),
    }
}

fn eval_duty_status(
    module: &CoreModule,
    terms: &[Term],
    case: &CaseRecord,
    ctx: &RunContext,
    args: &BTreeMap<String, Value>,
) -> Result<Value, String> {
    let name = literal_name(&terms[0])?;
    let instance = if terms.len() >= 2 {
        literal_name(&terms[1])?
    } else {
        DEFAULT_DUTY_INSTANCE.to_owned()
    };
    let Some(duty) = find_duty(module, &name) else {
        return Err(format!("unknown duty `{name}`"));
    };
    let view = FrozenCaseView::from_context(case, ctx);
    let overlay = view.case();
    let derived =
        DerivedWorld::compute(module, overlay, ctx, args).map_err(|err| err.to_string())?;
    let attaches_held = derived.is_guard_held(&duty.attaches, overlay, ctx);
    let attaches_denied = derived.is_guard_denied(&duty.attaches, overlay, ctx);
    let performed_held =
        derived.holds_named("performed") || derived.holds_named(&format!("{name}_performed"));
    let state = surface_duty_state(
        duty,
        overlay,
        ctx,
        attaches_held,
        attaches_denied,
        performed_held,
        &instance,
    );
    Ok(duty_state_value(&state))
}

fn eval_require_authority(
    terms: &[Term],
    case: &CaseRecord,
    ctx: &RunContext,
) -> Result<Value, String> {
    let action = literal_name(&terms[0])?;
    let view = FrozenCaseView::from_context(case, ctx);
    if action_is_granted(view.case(), &action, view.known_at()) {
        Ok(Value::Unit)
    } else {
        Err("requirement failed".into())
    }
}

fn find_duty<'m>(module: &'m CoreModule, name: &str) -> Option<&'m CoreDuty> {
    module.declarations.iter().find_map(|decl| match decl {
        CoreDecl::Duty(duty) if duty.name == name || duty.name.eq_ignore_ascii_case(name) => {
            Some(duty)
        }
        _ => None,
    })
}

fn duty_status_args_ok(args: &[Term]) -> bool {
    (args.len() == 1 || args.len() == 2) && args.iter().all(is_literal_name)
}

fn require_authority_args_ok(args: &[Term]) -> bool {
    args.len() == 1 && is_literal_name(&args[0])
}

fn is_literal_name(term: &Term) -> bool {
    literal_name(term).is_ok()
}

fn literal_name(term: &Term) -> Result<String, String> {
    match term {
        Term::Ident(name) | Term::String(name) | Term::Binder(name) => Ok(name.clone()),
        Term::Apply { ctor, args } if args.is_empty() => Ok(ctor.clone()),
        Term::Call { callee, args } if args.is_empty() => Ok(callee.clone()),
        other => Err(format!("expected name, got `{other:?}`")),
    }
}

fn is_duty_status(ctor: &str) -> bool {
    ctor.eq_ignore_ascii_case("duty_status")
}

fn is_require_authority(ctor: &str) -> bool {
    ctor.eq_ignore_ascii_case("require_authority")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn duty_status(name: &str) -> Term {
        Term::Apply {
            ctor: "duty_status".into(),
            args: vec![Term::Ident(name.into())],
        }
    }

    fn seq(args: Vec<Term>) -> Term {
        Term::Apply {
            ctor: "seq".into(),
            args,
        }
    }

    #[test]
    fn duty_status_and_require_authority_are_institutional() {
        assert!(is_institutional_fragment(&duty_status("PayInvoice")));
        assert!(is_institutional_fragment(&Term::Call {
            callee: "duty_status".into(),
            args: vec![
                Term::Ident("PayInvoice".into()),
                Term::Ident("invoice_a".into()),
            ],
        }));
        assert!(is_institutional_fragment(&Term::Apply {
            ctor: "require_authority".into(),
            args: vec![Term::Ident("release".into())],
        }));
        assert!(!is_institutional_fragment(&Term::Apply {
            ctor: "duty_step".into(),
            args: vec![Term::Ident("pay".into()), Term::Ident("attach".into())],
        }));
        assert!(!is_institutional_fragment(&Term::Apply {
            ctor: "duty_status".into(),
            args: vec![Term::Bool(true)],
        }));
        assert!(!is_institutional_fragment(&Term::Bool(true)));
        assert!(!is_institutional_fragment(&seq(vec![
            Term::Apply {
                ctor: "observed".into(),
                args: vec![Term::String("Filing".into())],
            },
            Term::Bool(true),
        ])));
        assert!(!is_institutional_fragment(&seq(vec![
            duty_status("PayInvoice"),
            Term::Bool(true),
        ])));
    }

    #[test]
    fn institutional_fragment_does_not_call_evaluate_or_handlers() {
        let impl_src = include_str!("duty.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("impl");
        assert!(
            !impl_src.contains("evaluate("),
            "institutional covering must not call the trusted evaluator"
        );
        assert!(
            !impl_src.contains("fidryn_handlers"),
            "institutional covering must not import handlers"
        );
        assert!(
            !impl_src.contains("CaseFile::") && !impl_src.contains("use fidryn_handlers"),
            "institutional covering must not use CaseFile"
        );
        assert!(
            !impl_src.contains("IsolatedReplay"),
            "institutional covering must not construct IsolatedReplay"
        );
        assert!(
            !impl_src.contains("fidryn_adapt"),
            "institutional covering must not import adapt"
        );
    }
}
