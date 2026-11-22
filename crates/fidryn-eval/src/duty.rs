//! Duty status machine. Illegal or unauthorized transitions commit nothing.

use fidryn_core::ir::CoreDuty;
use fidryn_core::time::{Bound, CalendarKind, Instant};
use fidryn_core::value::{Term, Value};
use fidryn_core::{
    CaseRecord, DutyState, DutyStatus, EngineError, EvidenceItem, FrozenCaseView, Interval,
    LedgerEvent, RunContext,
};
use std::collections::BTreeMap;

pub const DUTY_KEY_PREFIX: &str = "duty:";
pub const DEFAULT_DUTY_INSTANCE: &str = "default";

pub fn duty_fact_key(name: &str) -> String {
    duty_instance_key(name, DEFAULT_DUTY_INSTANCE)
}

pub fn duty_instance_key(def: &str, instance: &str) -> String {
    format!("{DUTY_KEY_PREFIX}{def}:{instance}")
}

pub fn status_name(status: DutyStatus) -> &'static str {
    match status {
        DutyStatus::Attached => "Attached",
        DutyStatus::Performed => "Performed",
        DutyStatus::Breached => "Breached",
        DutyStatus::Cured => "Cured",
        DutyStatus::Discharged => "Discharged",
        DutyStatus::Unresolved => "Unresolved",
        DutyStatus::NotAttached => "NotAttached",
    }
}

pub fn status_value(status: DutyStatus) -> Value {
    Value::Ctor {
        name: status_name(status).to_owned(),
        fields: BTreeMap::new(),
    }
}

pub fn duty_state_value(state: &DutyState) -> Value {
    let instance = if state.instance.is_empty() {
        DEFAULT_DUTY_INSTANCE
    } else {
        state.instance.as_str()
    };
    let mut fields = BTreeMap::from([
        ("name".into(), Value::String(state.name.clone())),
        (
            "status".into(),
            Value::String(status_name(state.status).into()),
        ),
        ("breached".into(), Value::Bool(state.breached)),
        ("bearer".into(), Value::String(state.bearer.clone())),
        ("instance".into(), Value::String(instance.into())),
    ]);
    if let Some(claimant) = &state.claimant {
        fields.insert("claimant".into(), Value::String(claimant.clone()));
    }
    Value::Map(fields)
}

pub fn parse_duty_state(value: &Value, name: &str) -> Option<DutyState> {
    let fields = match value {
        Value::Map(fields) | Value::Ctor { fields, .. } => fields,
        _ => return None,
    };
    let status = fields.get("status").and_then(parse_status_value)?;
    let breached = match fields.get("breached") {
        Some(Value::Bool(flag)) => *flag,
        None => false,
        _ => return None,
    };
    let bearer = match fields.get("bearer") {
        Some(Value::String(s) | Value::Entity(s)) => s.clone(),
        Some(Value::Unit) | None => String::new(),
        _ => return None,
    };
    let stored_name = match fields.get("name") {
        Some(Value::String(s) | Value::Entity(s)) => s.clone(),
        _ => name.to_owned(),
    };
    let claimant = match fields.get("claimant") {
        Some(Value::String(s) | Value::Entity(s)) => Some(s.clone()),
        Some(Value::Option(Some(inner))) => match inner.as_ref() {
            Value::String(s) | Value::Entity(s) => Some(s.clone()),
            _ => None,
        },
        _ => None,
    };
    let instance = match fields.get("instance").and_then(value_as_name) {
        Some(name) if !name.is_empty() => name,
        _ => DEFAULT_DUTY_INSTANCE.to_owned(),
    };
    Some(DutyState {
        name: stored_name,
        status,
        breached,
        bearer,
        claimant,
        instance,
    })
}

fn parse_status_value(value: &Value) -> Option<DutyStatus> {
    match value {
        Value::String(name) | Value::Entity(name) => parse_status_name(name),
        Value::Ctor { name, .. } => parse_status_name(name),
        _ => None,
    }
}

fn parse_status_name(name: &str) -> Option<DutyStatus> {
    if name.eq_ignore_ascii_case("Attached") {
        Some(DutyStatus::Attached)
    } else if name.eq_ignore_ascii_case("Performed") {
        Some(DutyStatus::Performed)
    } else if name.eq_ignore_ascii_case("Breached") {
        Some(DutyStatus::Breached)
    } else if name.eq_ignore_ascii_case("Cured") {
        Some(DutyStatus::Cured)
    } else if name.eq_ignore_ascii_case("Discharged") {
        Some(DutyStatus::Discharged)
    } else if name.eq_ignore_ascii_case("Unresolved") {
        Some(DutyStatus::Unresolved)
    } else if name.eq_ignore_ascii_case("NotAttached") {
        Some(DutyStatus::NotAttached)
    } else {
        None
    }
}

pub fn apply_duty_action(
    current: Option<&DutyState>,
    name: &str,
    action: &str,
    person: Option<String>,
) -> Result<DutyState, EngineError> {
    let action = parse_duty_action(action)
        .ok_or_else(|| EngineError::InvalidInput(format!("unknown duty action `{action}`")))?;
    let bearer = match (person, current) {
        (Some(person), _) => person,
        (None, Some(state)) => state.bearer.clone(),
        (None, None) => String::new(),
    };
    let claimant = current.and_then(|state| state.claimant.clone());
    let from = current.map(|state| state.status);
    let breached = current.map(|state| state.breached).unwrap_or(false);
    let instance = current
        .map(|state| state.instance.clone())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| DEFAULT_DUTY_INSTANCE.to_owned());
    let next = match (from, action) {
        (None | Some(DutyStatus::Unresolved | DutyStatus::NotAttached), DutyAction::Attach) => {
            DutyState {
                name: name.to_owned(),
                status: DutyStatus::Attached,
                breached: false,
                bearer,
                claimant,
                instance,
            }
        }
        (Some(DutyStatus::Attached), DutyAction::Perform) => DutyState {
            name: name.to_owned(),
            status: DutyStatus::Performed,
            breached: false,
            bearer,
            claimant,
            instance,
        },
        (Some(DutyStatus::Attached), DutyAction::Breach) => DutyState {
            name: name.to_owned(),
            status: DutyStatus::Breached,
            breached: true,
            bearer,
            claimant,
            instance,
        },
        (Some(DutyStatus::Breached), DutyAction::Perform) => DutyState {
            name: name.to_owned(),
            status: DutyStatus::Performed,
            breached: true,
            bearer,
            claimant,
            instance,
        },
        (Some(DutyStatus::Breached), DutyAction::Cure) => DutyState {
            name: name.to_owned(),
            status: DutyStatus::Cured,
            breached: true,
            bearer,
            claimant,
            instance,
        },
        (
            Some(DutyStatus::Performed | DutyStatus::Cured | DutyStatus::Breached),
            DutyAction::Discharge,
        ) => DutyState {
            name: name.to_owned(),
            status: DutyStatus::Discharged,
            breached,
            bearer,
            claimant,
            instance,
        },
        (from, action) => {
            let from = from.map_or("unattached", status_name);
            return Err(EngineError::InvalidInput(format!(
                "illegal duty transition: {} from {from}",
                action.as_str()
            )));
        }
    };
    Ok(next)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DutyAction {
    Attach,
    Perform,
    Breach,
    Cure,
    Discharge,
}

impl DutyAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::Attach => "attach",
            Self::Perform => "perform",
            Self::Breach => "breach",
            Self::Cure => "cure",
            Self::Discharge => "discharge",
        }
    }
}

fn parse_duty_action(name: &str) -> Option<DutyAction> {
    if name.eq_ignore_ascii_case("attach") {
        Some(DutyAction::Attach)
    } else if name.eq_ignore_ascii_case("perform") {
        Some(DutyAction::Perform)
    } else if name.eq_ignore_ascii_case("breach") {
        Some(DutyAction::Breach)
    } else if name.eq_ignore_ascii_case("cure") {
        Some(DutyAction::Cure)
    } else if name.eq_ignore_ascii_case("discharge") {
        Some(DutyAction::Discharge)
    } else {
        None
    }
}

pub fn has_authority_constraint(case: &CaseRecord) -> bool {
    case.facts.contains_key("authority_grants")
        || case
            .evidence
            .iter()
            .any(|item| item.schema.eq_ignore_ascii_case("AuthorityGrant"))
}

pub fn action_is_granted(case: &CaseRecord, action: &str, record_time: Instant) -> bool {
    action_is_granted_for(case, action, None, None, record_time)
}

pub fn action_is_granted_for(
    case: &CaseRecord,
    action: &str,
    scope: Option<&str>,
    context: Option<&str>,
    record_time: Instant,
) -> bool {
    let mut grants = Vec::new();
    for value in FrozenCaseView::new(case, record_time, record_time).grants() {
        collect_grants(value, &mut grants);
    }
    grants.iter().any(|grant| {
        grant_covers(
            grant,
            &grants,
            action,
            scope,
            context,
            record_time,
            &mut Vec::new(),
        )
    })
}

fn collect_grants(value: &Value, grants: &mut Vec<fidryn_core::state::AuthorityGrant>) {
    match value {
        Value::Set(values) => values
            .iter()
            .for_each(|value| collect_grants(value, grants)),
        Value::Map(fields) | Value::Ctor { fields, .. } => {
            if let Some(grant) = parse_grant(fields) {
                grants.push(grant);
            }
        }
        _ => {}
    }
}

fn parse_grant(fields: &BTreeMap<String, Value>) -> Option<fidryn_core::state::AuthorityGrant> {
    let action = fields.get("action").and_then(value_as_name)?;
    let scope = fields.get("scope").and_then(value_as_name)?;
    let context = fields.get("context").and_then(value_as_name)?;
    let source = fields.get("source").and_then(value_as_name)?;
    if action.is_empty() || source.is_empty() {
        return None;
    }
    let valid_time = value_as_interval(
        fields
            .get("validTime")
            .or_else(|| fields.get("valid_time"))?,
    )?;
    let revoked = match fields.get("revoked") {
        None => false,
        Some(Value::Bool(revoked)) => *revoked,
        _ => return None,
    };
    let revoked_at = match fields.get("revokedAt").or_else(|| fields.get("revoked_at")) {
        None | Some(Value::Unit) => None,
        Some(value) => Some(value_as_instant(value)?),
    };
    Some(fidryn_core::state::AuthorityGrant {
        action,
        scope,
        context,
        valid_time,
        source,
        revoked,
        revoked_at,
        principal: fields.get("principal").and_then(value_as_name),
        delegate_of: fields
            .get("delegateOf")
            .or_else(|| fields.get("delegate_of"))
            .and_then(value_as_name),
    })
}

pub(crate) fn value_as_interval(value: &Value) -> Option<Interval> {
    let fields = match value {
        Value::Map(fields) | Value::Ctor { fields, .. } => fields,
        _ => return None,
    };
    let start = value_as_bound(fields.get("start")?)?;
    let end = value_as_bound(fields.get("end")?)?;
    let interval = Interval { start, end };
    match (start, end) {
        (Bound::PosInf, _) | (_, Bound::NegInf) => None,
        (Bound::Inclusive(a) | Bound::Exclusive(a), Bound::Inclusive(b) | Bound::Exclusive(b))
            if a > b =>
        {
            None
        }
        _ => Some(interval),
    }
}

fn value_as_bound(value: &Value) -> Option<Bound> {
    match value {
        Value::String(name) | Value::Entity(name) if name == "NegInf" => Some(Bound::NegInf),
        Value::String(name) | Value::Entity(name) if name == "PosInf" => Some(Bound::PosInf),
        Value::Map(fields) if fields.len() == 1 => {
            if let Some(value) = fields.get("Inclusive") {
                Some(Bound::Inclusive(value_as_instant(value)?))
            } else {
                Some(Bound::Exclusive(value_as_instant(
                    fields.get("Exclusive")?,
                )?))
            }
        }
        Value::Ctor { name, fields } => match name.as_str() {
            "NegInf" => Some(Bound::NegInf),
            "PosInf" => Some(Bound::PosInf),
            "Inclusive" => Some(Bound::Inclusive(value_as_instant(fields.values().next()?)?)),
            "Exclusive" => Some(Bound::Exclusive(value_as_instant(fields.values().next()?)?)),
            _ => None,
        },
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn grant_covers(
    grant: &fidryn_core::state::AuthorityGrant,
    grants: &[fidryn_core::state::AuthorityGrant],
    action: &str,
    scope: Option<&str>,
    context: Option<&str>,
    at: Instant,
    chain: &mut Vec<String>,
) -> bool {
    if !grant.covers(action, at)
        || scope.is_some_and(|scope| grant.scope != "*" && grant.scope != scope)
        || context.is_some_and(|context| grant.context != "*" && grant.context != context)
    {
        return false;
    }
    let Some(grantor) = &grant.delegate_of else {
        return true;
    };
    if chain.len() >= 16 || chain.contains(grantor) {
        return false;
    }
    chain.push(grantor.clone());
    let authorized = grants.iter().any(|parent| {
        parent.principal.as_ref() == Some(grantor)
            && grant_covers(
                parent,
                grants,
                action,
                Some(&grant.scope),
                Some(&grant.context),
                at,
                chain,
            )
    });
    chain.pop();
    authorized
}

/// Admission keys off payload semantics and authority, not only kind strings.
///
/// `duty` / `authority` / institutional kinds, and any Performed/Attached/…
/// duty-transition payload, need a covering grant. Assumption events are never
/// operative. Relabeling a Performed payload as `correction` does not admit it.
pub fn event_is_admitted(case: &CaseRecord, event: &LedgerEvent, record_time: Instant) -> bool {
    if !FrozenCaseView::event_is_known_at(event, record_time) {
        return false;
    }
    if event.kind.eq_ignore_ascii_case("assumption") {
        return false;
    }
    if is_gated_event_kind(&event.kind) || payload_is_gated_duty_transition(&event.payload) {
        let fields = match &event.payload {
            Value::Map(fields) | Value::Ctor { fields, .. } => Some(fields),
            _ => None,
        };
        let scope = fields.and_then(duty_name_from_fields);
        let context = fields
            .and_then(|fields| fields.get("context"))
            .and_then(value_as_name);
        return event_action(&event.payload).is_some_and(|action| {
            action_is_granted_for(
                case,
                &action,
                scope.as_deref(),
                context.as_deref(),
                record_time,
            )
        });
    }
    true
}

fn is_gated_event_kind(kind: &str) -> bool {
    kind.eq_ignore_ascii_case("duty")
        || kind.eq_ignore_ascii_case("authority")
        || kind.eq_ignore_ascii_case("institutional")
}

fn payload_is_gated_duty_transition(payload: &Value) -> bool {
    event_action(payload).is_some_and(|action| is_gated_duty_action(&action))
}

fn is_gated_duty_action(action: &str) -> bool {
    action.eq_ignore_ascii_case("perform")
        || action.eq_ignore_ascii_case("attach")
        || action.eq_ignore_ascii_case("breach")
        || action.eq_ignore_ascii_case("cure")
        || action.eq_ignore_ascii_case("discharge")
}

fn event_action(payload: &Value) -> Option<String> {
    match payload {
        Value::String(name) | Value::Entity(name) => Some(status_to_action(name)),
        Value::Ctor { name, fields } => fields
            .get("action")
            .and_then(value_action_name)
            .or_else(|| Some(status_to_action(name))),
        Value::Map(fields) => fields
            .get("action")
            .and_then(value_action_name)
            .or_else(|| fields.get("status").and_then(value_action_name))
            .or_else(|| match fields.get("performed") {
                Some(Value::Bool(true)) => Some("perform".to_owned()),
                _ => None,
            }),
        _ => None,
    }
}

fn value_action_name(value: &Value) -> Option<String> {
    match value {
        Value::String(name) | Value::Entity(name) => Some(status_to_action(name)),
        Value::Ctor { name, .. } => Some(status_to_action(name)),
        _ => None,
    }
}

fn status_to_action(name: &str) -> String {
    if name.eq_ignore_ascii_case("Performed") {
        "perform".into()
    } else if name.eq_ignore_ascii_case("Attached") {
        "attach".into()
    } else if name.eq_ignore_ascii_case("Breached") {
        "breach".into()
    } else if name.eq_ignore_ascii_case("Cured") {
        "cure".into()
    } else if name.eq_ignore_ascii_case("Discharged") {
        "discharge".into()
    } else {
        name.to_ascii_lowercase()
    }
}

/// Surface `duty_status(Name)` / `duty_status(Name, Instance)` from a [`CoreDuty`].
pub fn surface_duty_state(
    duty: &CoreDuty,
    case: &CaseRecord,
    ctx: &RunContext,
    attaches_held: bool,
    attaches_denied: bool,
    performed_held: bool,
    instance: &str,
) -> DutyState {
    let bearer = party_name(&duty.bearer);
    let claimant = duty
        .claimant
        .as_ref()
        .map(party_name)
        .filter(|s| !s.is_empty());
    let instance = if instance.is_empty() {
        DEFAULT_DUTY_INSTANCE.to_owned()
    } else {
        instance.to_owned()
    };
    if !attaches_held {
        let status = if attaches_denied {
            DutyStatus::NotAttached
        } else {
            DutyStatus::Unresolved
        };
        return DutyState {
            name: duty.name.clone(),
            status,
            breached: false,
            bearer,
            claimant,
            instance,
        };
    }
    let named = is_named_instance(&instance);
    // Named instances ignore definition-level `performed` / `<duty>_performed`
    // shortcuts from the derived world and case facts.
    let _ = performed_held;
    let performed = performance_exists(duty, &instance, case, ctx);
    let deadline_passed = deadline_has_passed(&duty.name, &instance, &duty.content, case, ctx);
    let already_breached = stored_duty_breached(&duty.name, &instance, case)
        || (!named && fact_flag(case, &format!("{}_breached", duty.name)).unwrap_or(false));
    if performed {
        // Historical breach is occurrence vs deadline, not "the query is after
        // the deadline". An on-time payment stays unbreached when asked later.
        let late = already_breached || performance_is_after_deadline(duty, &instance, case, ctx);
        return DutyState {
            name: duty.name.clone(),
            status: DutyStatus::Performed,
            breached: late,
            bearer,
            claimant,
            instance,
        };
    }
    if deadline_passed {
        return DutyState {
            name: duty.name.clone(),
            status: DutyStatus::Breached,
            breached: true,
            bearer,
            claimant,
            instance,
        };
    }
    DutyState {
        name: duty.name.clone(),
        status: DutyStatus::Attached,
        breached: false,
        bearer,
        claimant,
        instance,
    }
}

fn party_name(term: &Term) -> String {
    match term {
        Term::Ident(name) | Term::String(name) | Term::Binder(name) => name.clone(),
        Term::Apply { ctor, args } if args.is_empty() => ctor.clone(),
        Term::Call { callee, args } if args.is_empty() => callee.clone(),
        _ => String::new(),
    }
}

fn is_named_instance(instance: &str) -> bool {
    !instance.is_empty() && !instance.eq_ignore_ascii_case(DEFAULT_DUTY_INSTANCE)
}

/// True when a payload does not name a non-default duty instance.
pub(crate) fn payload_applies_to_default_instance(payload: &Value) -> bool {
    match payload {
        Value::Map(fields) | Value::Ctor { fields, .. } => match instance_from_fields(fields) {
            Some(named) => named.eq_ignore_ascii_case(DEFAULT_DUTY_INSTANCE),
            None => true,
        },
        _ => true,
    }
}

fn performance_exists(
    duty: &CoreDuty,
    instance: &str,
    case: &CaseRecord,
    ctx: &RunContext,
) -> bool {
    if stored_duty_performed(&duty.name, instance, case) {
        return true;
    }
    let view = FrozenCaseView::from_context(case, ctx);
    if view
        .evidence()
        .any(|item| payment_matches_duty(item, duty, instance, ctx))
    {
        return true;
    }
    view.events().any(|event| {
        event_is_admitted(case, event, ctx.record_time)
            && event_marks_performed(event, &duty.name, instance)
            && event_occurred_at(event).is_some_and(|at| at <= ctx.valid_time)
    })
}

fn payment_matches_duty(
    item: &EvidenceItem,
    duty: &CoreDuty,
    instance: &str,
    ctx: &RunContext,
) -> bool {
    if !payment_matches_instance(item, &duty.name, instance)
        || !payment_occurred_at(item).is_some_and(|at| at <= ctx.valid_time)
    {
        return false;
    }
    let fields = match &item.value {
        Value::Map(fields) | Value::Ctor { fields, .. } => fields,
        _ => return false,
    };
    if fields.get("payer").and_then(value_as_name).as_deref()
        != Some(party_name(&duty.bearer).as_str())
    {
        return false;
    }
    if let Some(claimant) = &duty.claimant
        && fields.get("payee").and_then(value_as_name).as_deref()
            != Some(party_name(claimant).as_str())
    {
        return false;
    }
    let Some((currency, required)) = duty.content.iter().find_map(monetary_content) else {
        return false;
    };
    if fields.get("currency").and_then(value_as_name).as_deref() != Some(currency.as_str()) {
        return false;
    }
    let paid = match fields.get("amount") {
        Some(Value::Decimal(amount)) => Some(*amount),
        Some(Value::Int(amount)) => Some(rust_decimal::Decimal::from(*amount)),
        _ => None,
    };
    paid.is_some_and(|amount| amount >= required)
}

fn monetary_content(term: &Term) -> Option<(String, rust_decimal::Decimal)> {
    match term {
        Term::Apply { ctor, args } | Term::Call { callee: ctor, args }
            if ctor.len() == 3 && ctor.bytes().all(|byte| byte.is_ascii_uppercase()) =>
        {
            match args.as_slice() {
                [Term::Decimal(amount)] => Some((ctor.clone(), *amount)),
                [Term::Int(amount)] => Some((ctor.clone(), rust_decimal::Decimal::from(*amount))),
                _ => None,
            }
        }
        _ => None,
    }
}

fn payment_matches_instance(item: &EvidenceItem, duty_name: &str, instance: &str) -> bool {
    if !item.schema.eq_ignore_ascii_case("PaymentRecord") {
        return false;
    }
    match &item.value {
        Value::String(_) | Value::Entity(_) => false,
        Value::Map(fields) | Value::Ctor { fields, .. } => {
            if duty_name_from_fields(fields).as_deref() != Some(duty_name) {
                return false;
            }
            match instance_from_fields(fields) {
                Some(named) => named.eq_ignore_ascii_case(instance),
                None => instance.eq_ignore_ascii_case(DEFAULT_DUTY_INSTANCE),
            }
        }
        _ => false,
    }
}

fn event_marks_performed(event: &LedgerEvent, duty_name: &str, instance: &str) -> bool {
    payload_marks_performed(&event.payload, duty_name, instance)
}

fn payload_marks_performed(payload: &Value, duty_name: &str, instance: &str) -> bool {
    match payload {
        Value::String(name) | Value::Entity(name) => {
            instance.eq_ignore_ascii_case(DEFAULT_DUTY_INSTANCE) && is_performed_name(name)
        }
        Value::Ctor { name, fields } => {
            if duty_field_mismatch(fields, duty_name) || instance_field_mismatch(fields, instance) {
                return false;
            }
            is_performed_name(name) || fields.get("status").is_some_and(value_is_performed)
        }
        Value::Map(fields) => {
            if duty_field_mismatch(fields, duty_name) || instance_field_mismatch(fields, instance) {
                return false;
            }
            fields.get("status").is_some_and(value_is_performed)
                || matches!(fields.get("performed"), Some(Value::Bool(true)))
        }
        _ => false,
    }
}

fn duty_field_mismatch(fields: &BTreeMap<String, Value>, duty_name: &str) -> bool {
    fields
        .get("name")
        .or_else(|| fields.get("duty"))
        .is_some_and(|named| !value_names_duty(named, duty_name))
}

fn instance_field_mismatch(fields: &BTreeMap<String, Value>, instance: &str) -> bool {
    match instance_from_fields(fields) {
        Some(named) => !named.eq_ignore_ascii_case(instance),
        None => !instance.eq_ignore_ascii_case(DEFAULT_DUTY_INSTANCE),
    }
}

fn instance_from_fields(fields: &BTreeMap<String, Value>) -> Option<String> {
    fields.get("instance").and_then(value_as_name)
}

fn duty_name_from_fields(fields: &BTreeMap<String, Value>) -> Option<String> {
    fields
        .get("name")
        .or_else(|| fields.get("duty"))
        .and_then(value_as_name)
}

fn value_as_name(value: &Value) -> Option<String> {
    match value {
        Value::String(name) | Value::Entity(name) => Some(name.clone()),
        Value::Ctor { name, .. } => Some(name.clone()),
        _ => None,
    }
}

fn value_names_duty(value: &Value, duty_name: &str) -> bool {
    match value {
        Value::String(name) | Value::Entity(name) => name.eq_ignore_ascii_case(duty_name),
        Value::Ctor { name, .. } => name.eq_ignore_ascii_case(duty_name),
        _ => false,
    }
}

fn value_is_performed(value: &Value) -> bool {
    match value {
        Value::Bool(flag) => *flag,
        Value::String(name) | Value::Entity(name) => is_performed_name(name),
        Value::Ctor { name, .. } => is_performed_name(name),
        _ => false,
    }
}

fn is_performed_name(name: &str) -> bool {
    name.eq_ignore_ascii_case("Performed") || name.eq_ignore_ascii_case("perform")
}

fn stored_duty_state(name: &str, instance: &str, case: &CaseRecord) -> Option<DutyState> {
    case.facts
        .get(&duty_instance_key(name, instance))
        .and_then(|value| parse_duty_state(value, name))
}

fn stored_duty_performed(name: &str, instance: &str, case: &CaseRecord) -> bool {
    stored_duty_state(name, instance, case)
        .is_some_and(|state| state.status == DutyStatus::Performed)
}

fn stored_duty_breached(name: &str, instance: &str, case: &CaseRecord) -> bool {
    stored_duty_state(name, instance, case).is_some_and(|state| state.breached)
}

fn deadline_has_passed(
    name: &str,
    instance: &str,
    content: &[Term],
    case: &CaseRecord,
    ctx: &RunContext,
) -> bool {
    if !is_named_instance(instance)
        && fact_flag(case, &format!("{name}_deadline_passed")).unwrap_or(false)
    {
        return true;
    }
    match due_deadline_instant(content, case) {
        Some(deadline) => ctx.valid_time > deadline,
        None => {
            !is_named_instance(instance)
                && fact_flag(case, &format!("{name}_late")).unwrap_or(false)
        }
    }
}

fn performance_is_after_deadline(
    duty: &CoreDuty,
    instance: &str,
    case: &CaseRecord,
    ctx: &RunContext,
) -> bool {
    let Some(deadline) = due_deadline_instant(&duty.content, case) else {
        return false;
    };
    performance_instant(duty, instance, case, ctx).is_some_and(|at| at > deadline)
}

pub fn validate_deadline(content: &[Term], case: &CaseRecord) -> Result<(), EngineError> {
    if due_duration(content).is_some() {
        checked_deadline(content, case)?;
    }
    Ok(())
}

fn due_deadline_instant(content: &[Term], case: &CaseRecord) -> Option<Instant> {
    checked_deadline(content, case).ok().flatten()
}

fn checked_deadline(content: &[Term], case: &CaseRecord) -> Result<Option<Instant>, EngineError> {
    let Some(duration) = due_duration(content) else {
        return Ok(None);
    };
    let start = due_anchor_instant(content, case)
        .ok_or_else(|| EngineError::InvalidInput("deadline anchor is missing or invalid".into()))?;
    if duration.kind == CalendarKind::WorkingDays {
        let closed = case
            .closures
            .iter()
            .any(|closure| closure.domain == "working_days" && closure.closed);
        let holidays = match case.facts.get("working_days_holidays") {
            Some(Value::Set(values)) if closed => values,
            _ => return Err(EngineError::Unsupported(
                "working_days requires a closed working_days calendar and working_days_holidays"
                    .into(),
            )),
        };
        if duration.amount.unsigned_abs() > 100_000 {
            return Err(EngineError::FuelExhausted { remaining: 0 });
        }
        let dates: Option<Vec<_>> = holidays
            .iter()
            .map(value_as_instant)
            .map(|date| date.map(|date| date.as_offset().date()))
            .collect();
        let dates = dates
            .ok_or_else(|| EngineError::InvalidInput("invalid working-day holiday date".into()))?;
        let mut at = start.as_offset();
        let mut remaining = duration.amount.unsigned_abs();
        let step = if duration.amount < 0 { -1 } else { 1 };
        while remaining > 0 {
            at = at
                .checked_add(time::Duration::seconds(step * 86_400))
                .ok_or_else(|| EngineError::InvalidInput("deadline overflow".into()))?;
            if !matches!(
                at.weekday(),
                time::Weekday::Saturday | time::Weekday::Sunday
            ) && !dates.contains(&at.date())
            {
                remaining -= 1;
            }
        }
        return Ok(Some(Instant::from_offset(at)));
    }
    let seconds_per_unit = match duration.kind {
        CalendarKind::Days | CalendarKind::CalendarDays | CalendarKind::CountedDays => 86_400,
        CalendarKind::Hours => 3_600,
        CalendarKind::Minutes => 60,
        CalendarKind::Seconds => 1,
        CalendarKind::WorkingDays => unreachable!(),
    };
    let seconds = duration
        .amount
        .checked_mul(seconds_per_unit)
        .ok_or_else(|| EngineError::InvalidInput("deadline duration overflow".into()))?;
    start
        .as_offset()
        .checked_add(time::Duration::seconds(seconds))
        .map(Instant::from_offset)
        .map(Some)
        .ok_or_else(|| EngineError::InvalidInput("deadline overflow".into()))
}

fn due_anchor_instant(content: &[Term], case: &CaseRecord) -> Option<Instant> {
    fn after(term: &Term) -> Option<&Term> {
        match term {
            Term::Apply { ctor, args } | Term::Call { callee: ctor, args } if ctor == "after" => {
                args.get(1)
            }
            Term::Apply { args, .. } | Term::Call { args, .. } | Term::Set(args) => {
                args.iter().find_map(after)
            }
            Term::Binary { left, right, .. } => after(left).or_else(|| after(right)),
            _ => None,
        }
    }
    if let Some(anchor) = content.iter().find_map(after) {
        return match anchor {
            Term::Instant(at) => Some(*at),
            Term::Ident(name) | Term::String(name) => {
                fact_instant(case, name).or_else(|| Instant::parse(name).ok())
            }
            _ => None,
        };
    }
    fact_instant(case, "invoice_date").or_else(|| fact_instant(case, "due_at"))
}

fn due_duration(content: &[Term]) -> Option<fidryn_core::time::FidrynDuration> {
    content.iter().find_map(find_duration)
}

fn find_duration(term: &Term) -> Option<fidryn_core::time::FidrynDuration> {
    use fidryn_core::time::FidrynDuration;
    match term {
        Term::Duration(duration) => Some(*duration),
        Term::Apply { ctor, args } | Term::Call { callee: ctor, args } => {
            let kind = match ctor.as_str() {
                "days" => Some(CalendarKind::Days),
                "calendar_days" => Some(CalendarKind::CalendarDays),
                "counted_days" => Some(CalendarKind::CountedDays),
                "working_days" => Some(CalendarKind::WorkingDays),
                "hours" => Some(CalendarKind::Hours),
                "minutes" => Some(CalendarKind::Minutes),
                "seconds" => Some(CalendarKind::Seconds),
                _ => None,
            };
            if let (Some(kind), Some(Term::Int(amount))) = (kind, args.first()) {
                Some(FidrynDuration {
                    amount: *amount,
                    kind,
                })
            } else {
                args.iter().find_map(find_duration)
            }
        }
        Term::Binary { left, right, .. } => find_duration(left).or_else(|| find_duration(right)),
        Term::Set(values) => values.iter().find_map(find_duration),
        _ => None,
    }
}

fn performance_instant(
    duty: &CoreDuty,
    instance: &str,
    case: &CaseRecord,
    ctx: &RunContext,
) -> Option<Instant> {
    let view = FrozenCaseView::from_context(case, ctx);
    let from_evidence = view
        .evidence()
        .filter(|item| payment_matches_duty(item, duty, instance, ctx))
        .filter_map(payment_occurred_at)
        .min();
    if from_evidence.is_some() {
        return from_evidence;
    }
    view.events()
        .filter(|event| {
            event_is_admitted(case, event, ctx.record_time)
                && event_marks_performed(event, &duty.name, instance)
                && event_occurred_at(event).is_some_and(|at| at <= ctx.valid_time)
        })
        .filter_map(event_occurred_at)
        .min()
}

/// Occurrence time for a payment. Knowledge uses `observed_at`; performance
/// prefers payload `occurred_at` and falls back to `observed_at` when absent.
fn payment_occurred_at(item: &EvidenceItem) -> Option<Instant> {
    occurrence_from_value(&item.value).or(Some(item.observed_at))
}

fn event_occurred_at(event: &LedgerEvent) -> Option<Instant> {
    occurrence_from_value(&event.payload).or_else(|| interval_start(event.valid_time))
}

fn occurrence_from_value(value: &Value) -> Option<Instant> {
    match value {
        Value::Map(fields) | Value::Ctor { fields, .. } => fields
            .get("occurred_at")
            .or_else(|| fields.get("occurredAt"))
            .or_else(|| fields.get("paid_at"))
            .or_else(|| fields.get("paidAt"))
            .and_then(value_as_instant),
        _ => None,
    }
}

fn interval_start(interval: Interval) -> Option<Instant> {
    match interval.start {
        Bound::Inclusive(instant) | Bound::Exclusive(instant) => Some(instant),
        Bound::NegInf | Bound::PosInf => None,
    }
}

fn fact_flag(case: &CaseRecord, key: &str) -> Option<bool> {
    match case.facts.get(key) {
        Some(Value::Bool(flag)) => Some(*flag),
        Some(Value::String(name) | Value::Entity(name)) if name.eq_ignore_ascii_case("true") => {
            Some(true)
        }
        Some(Value::String(name) | Value::Entity(name)) if name.eq_ignore_ascii_case("false") => {
            Some(false)
        }
        _ => None,
    }
}

fn fact_instant(case: &CaseRecord, key: &str) -> Option<Instant> {
    case.facts.get(key).and_then(value_as_instant)
}

fn value_as_instant(value: &Value) -> Option<Instant> {
    match value {
        Value::Instant(instant) => Some(*instant),
        Value::String(text) => Instant::parse(text)
            .ok()
            .or_else(|| Instant::parse(&format!("{text}T00:00:00Z")).ok()),
        _ => None,
    }
}

/// Clone `case` and apply assumption payloads to scenario-only facts.
/// Never appends to `events`.
pub fn scenario_overlay_case(case: &CaseRecord) -> CaseRecord {
    let mut overlay = case.clone();
    for assumption in &case.assumptions {
        apply_assumption_payload(&mut overlay, &assumption.payload);
    }
    overlay
}

pub fn scenario_hypotheses(case: &CaseRecord, ctx: &RunContext) -> Vec<fidryn_core::Assumption> {
    let mut hypotheses = case.assumptions.clone();
    for (index, event) in case.events.iter().enumerate() {
        if event.kind == "assumption"
            && event.record_time <= ctx.record_time
            && event.valid_time.contains(ctx.valid_time)
        {
            hypotheses.push(fidryn_core::Assumption {
                id: format!("event:{index}:{}", event.record_time),
                payload: event.payload.clone(),
            });
        }
    }
    hypotheses
}

pub fn scenario_overlay_at(case: &CaseRecord, ctx: &RunContext) -> CaseRecord {
    let mut overlay = case.clone();
    overlay.assumptions = scenario_hypotheses(case, ctx);
    let hypotheses = overlay.assumptions.clone();
    for assumption in &hypotheses {
        apply_assumption_payload(&mut overlay, &assumption.payload);
    }
    overlay
}

/// Merge an assumption payload into scenario facts. Does not push events.
pub fn apply_assumption_payload(case: &mut CaseRecord, payload: &Value) {
    match payload {
        Value::Map(fields) => {
            for (key, value) in fields {
                case.facts.insert(key.clone(), value.clone());
            }
            overlay_performed_from_fields(case, fields, false);
        }
        Value::Ctor { name, fields } if is_performed_name(name) => {
            overlay_performed_from_fields(case, fields, true);
        }
        Value::String(name) | Value::Entity(name) if is_performed_name(name) => {
            case.facts.insert("performed".into(), Value::Bool(true));
        }
        _ => {}
    }
}

fn overlay_performed_from_fields(
    case: &mut CaseRecord,
    fields: &BTreeMap<String, Value>,
    ctor_performed: bool,
) {
    let performed = ctor_performed
        || fields.get("status").is_some_and(value_is_performed)
        || matches!(fields.get("performed"), Some(Value::Bool(true)));
    if !performed {
        return;
    }
    let instance = instance_from_fields(fields).unwrap_or_else(|| DEFAULT_DUTY_INSTANCE.to_owned());
    if let Some(name) = duty_name_from_fields(fields) {
        insert_performed_duty(case, &name, &instance);
    } else {
        case.facts.insert("performed".into(), Value::Bool(true));
    }
}

fn insert_performed_duty(case: &mut CaseRecord, name: &str, instance: &str) {
    let state = DutyState {
        name: name.to_owned(),
        status: DutyStatus::Performed,
        breached: false,
        bearer: String::new(),
        claimant: None,
        instance: instance.to_owned(),
    };
    case.facts
        .insert(duty_instance_key(name, instance), duty_state_value(&state));
}

#[cfg(test)]
mod tests {
    use super::*;
    use fidryn_core::ids::{JurisdictionId, NodeId, OriginId};
    use fidryn_core::{Assumption, EvidenceItem, Guard, Interval, NodeMeta};

    fn attached() -> DutyState {
        DutyState {
            name: "pay".into(),
            status: DutyStatus::Attached,
            breached: false,
            bearer: String::new(),
            claimant: None,
            instance: DEFAULT_DUTY_INSTANCE.into(),
        }
    }

    #[test]
    fn attach_from_absent_is_attached() {
        let got = apply_duty_action(None, "pay", "attach", None).unwrap();
        assert_eq!(got.status, DutyStatus::Attached);
        assert!(!got.breached);
        assert_eq!(got.name, "pay");
        assert_eq!(got.instance, DEFAULT_DUTY_INSTANCE);
    }

    #[test]
    fn attach_from_not_attached_is_attached() {
        let current = DutyState {
            name: "pay".into(),
            status: DutyStatus::NotAttached,
            breached: false,
            bearer: String::new(),
            claimant: None,
            instance: "invoice_a".into(),
        };
        let got = apply_duty_action(Some(&current), "pay", "attach", None).unwrap();
        assert_eq!(got.status, DutyStatus::Attached);
        assert_eq!(got.instance, "invoice_a");
    }

    #[test]
    fn late_perform_from_breached_keeps_breached() {
        let breached = DutyState {
            name: "pay".into(),
            status: DutyStatus::Breached,
            breached: true,
            bearer: "Alice".into(),
            claimant: None,
            instance: DEFAULT_DUTY_INSTANCE.into(),
        };
        let got = apply_duty_action(Some(&breached), "pay", "perform", None).unwrap();
        assert_eq!(got.status, DutyStatus::Performed);
        assert!(got.breached);
        assert_eq!(got.bearer, "Alice");
    }

    #[test]
    fn discharge_from_attached_is_illegal() {
        let err = apply_duty_action(Some(&attached()), "pay", "discharge", None).unwrap_err();
        assert!(
            matches!(err, EngineError::InvalidInput(ref msg) if msg.contains("discharge")),
            "{err:?}"
        );
    }

    #[test]
    fn grant_set_and_authority_evidence_match_action() {
        let t = Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let mut case = CaseRecord::default();
        case.facts.insert(
            "authority_grants".into(),
            Value::Set(vec![grant_map("attach", false, None)]),
        );
        assert!(action_is_granted(&case, "attach", t));
        assert!(!action_is_granted(&case, "perform", t));

        let mut evidence_only = CaseRecord::default();
        evidence_only.evidence.push(EvidenceItem {
            schema: "AuthorityGrant".into(),
            value: grant_map("perform", false, None),
            observed_at: t,
        });
        assert!(action_is_granted(&evidence_only, "perform", t));
        assert!(!action_is_granted(&evidence_only, "attach", t));
        assert!(has_authority_constraint(&evidence_only));
    }

    fn grant_map(action: &str, revoked: bool, delegate_of: Option<&str>) -> Value {
        let mut fields = BTreeMap::from([
            ("action".into(), Value::String(action.into())),
            ("scope".into(), Value::String("*".into())),
            ("context".into(), Value::String("*".into())),
            ("source".into(), Value::String("test-grant".into())),
            (
                "validTime".into(),
                Value::Map(BTreeMap::from([
                    ("start".into(), Value::String("NegInf".into())),
                    ("end".into(), Value::String("PosInf".into())),
                ])),
            ),
            ("revoked".into(), Value::Bool(revoked)),
        ]);
        if let Some(grantor) = delegate_of {
            fields.insert("delegate_of".into(), Value::String(grantor.into()));
            fields.insert("principal".into(), Value::String("delegate".into()));
        }
        Value::Map(fields)
    }

    #[test]
    fn revoked_grant_map_does_not_grant() {
        let t = Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let mut case = CaseRecord::default();
        case.facts.insert(
            "authority_grants".into(),
            Value::Set(vec![grant_map("attach", true, None)]),
        );
        assert!(!action_is_granted(&case, "attach", t));

        let mut ctor_case = CaseRecord::default();
        ctor_case.evidence.push(EvidenceItem {
            schema: "AuthorityGrant".into(),
            value: Value::Ctor {
                name: "AuthorityGrant".into(),
                fields: BTreeMap::from([
                    ("action".into(), Value::String("perform".into())),
                    ("revoked".into(), Value::Bool(true)),
                ]),
            },
            observed_at: t,
        });
        assert!(!action_is_granted(&ctor_case, "perform", t));
        assert!(has_authority_constraint(&ctor_case));
    }

    #[test]
    fn false_permission_entry_does_not_grant_authority() {
        let t = Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let mut case = CaseRecord::default();
        case.facts.insert(
            "authority_grants".into(),
            Value::Map(BTreeMap::from([("perform".into(), Value::Bool(false))])),
        );
        assert!(
            !action_is_granted(&case, "perform", t),
            "{{\"perform\": false}} is a denial, not a grant"
        );
        case.facts.insert(
            "authority_grants".into(),
            Value::Map(BTreeMap::from([("perform".into(), Value::Bool(true))])),
        );
        assert!(
            !action_is_granted(&case, "perform", t),
            "a Boolean entry has no grant scope, provenance, or validity interval"
        );
        assert!(!action_is_granted(&case, "attach", t));
    }

    #[test]
    fn unrevoked_delegated_grant_map_grants() {
        let t = Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let mut case = CaseRecord::default();
        let Value::Map(mut parent) = grant_map("attach", false, None) else {
            unreachable!()
        };
        parent.insert("principal".into(), Value::String("grantor".into()));
        case.facts.insert(
            "authority_grants".into(),
            Value::Set(vec![
                Value::Map(parent),
                grant_map("attach", false, Some("grantor")),
            ]),
        );
        assert!(action_is_granted(&case, "attach", t));
        assert!(!action_is_granted(&case, "perform", t));
    }

    #[test]
    fn duty_event_without_grant_is_not_admitted() {
        let t = Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let event = LedgerEvent {
            kind: "duty".into(),
            valid_time: Interval::always(),
            record_time: t,
            payload: Value::Ctor {
                name: "Performed".into(),
                fields: BTreeMap::new(),
            },
        };
        let case = CaseRecord::default();
        assert!(!event_is_admitted(&case, &event, t));
    }

    #[test]
    fn assumption_event_is_not_admitted_without_grant() {
        let t = Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let event = LedgerEvent {
            kind: "assumption".into(),
            valid_time: Interval::always(),
            record_time: t,
            payload: Value::Ctor {
                name: "Performed".into(),
                fields: BTreeMap::new(),
            },
        };
        let case = CaseRecord::default();
        assert!(!event_is_admitted(&case, &event, t));
    }

    #[test]
    fn correction_performed_payload_is_not_admitted_without_grant() {
        let t = Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let event = LedgerEvent {
            kind: "correction".into(),
            valid_time: Interval::always(),
            record_time: t,
            payload: Value::Ctor {
                name: "Performed".into(),
                fields: BTreeMap::new(),
            },
        };
        let case = CaseRecord::default();
        assert!(
            !event_is_admitted(&case, &event, t),
            "relabeling a performed payload as correction must not authorize it"
        );
    }

    #[test]
    fn assumption_kind_never_operatively_admits_even_with_grant() {
        let t = Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let mut case = CaseRecord::default();
        case.facts.insert(
            "authority_grants".into(),
            Value::Set(vec![Value::String("perform".into())]),
        );
        let event = LedgerEvent {
            kind: "assumption".into(),
            valid_time: Interval::always(),
            record_time: t,
            payload: Value::Ctor {
                name: "Performed".into(),
                fields: BTreeMap::new(),
            },
        };
        assert!(!event_is_admitted(&case, &event, t));
    }

    fn pay_invoice_duty_after(days: i64, after: &str) -> CoreDuty {
        CoreDuty {
            id: NodeId::of(b"PayInvoice"),
            name: "PayInvoice".into(),
            binders: Vec::new(),
            bearer: Term::Ident("Payer".into()),
            claimant: Some(Term::Ident("Payee".into())),
            attaches: Guard::Satisfied,
            content: vec![
                Term::Apply {
                    ctor: "USD".into(),
                    args: vec![Term::Int(100)],
                },
                Term::Apply {
                    ctor: "due".into(),
                    args: vec![Term::Apply {
                        ctor: "after".into(),
                        args: vec![
                            Term::Apply {
                                ctor: "counted_days".into(),
                                args: vec![Term::Int(days)],
                            },
                            Term::Ident(after.into()),
                        ],
                    }],
                },
            ],
            meta: NodeMeta {
                span: None,
                source: None,
                jurisdiction: JurisdictionId::of(b"j"),
                valid_time: Interval::always(),
                record_time: Interval::always(),
                origin: OriginId::Direct(NodeId::of(b"PayInvoice")),
            },
        }
    }

    fn instant(text: &str) -> Instant {
        Instant::parse(text).unwrap()
    }

    #[test]
    fn on_time_payment_remains_unbreached_after_its_deadline() {
        let invoice = instant("2033-01-01T00:00:00Z");
        let paid = instant("2033-01-10T00:00:00Z");
        let later = instant("2033-02-15T00:00:00Z");
        let mut case = CaseRecord::default();
        case.facts
            .insert("invoice_date".into(), Value::Instant(invoice));
        case.evidence.push(EvidenceItem {
            schema: "PaymentRecord".into(),
            observed_at: paid,
            value: Value::Map(BTreeMap::from([
                ("name".into(), Value::String("PayInvoice".into())),
                ("instance".into(), Value::String("invoice_a".into())),
                ("payer".into(), Value::String("Payer".into())),
                ("payee".into(), Value::String("Payee".into())),
                ("amount".into(), Value::Int(100)),
                ("currency".into(), Value::String("USD".into())),
            ])),
        });
        let duty = pay_invoice_duty_after(30, "invoice_date");
        let state = surface_duty_state(
            &duty,
            &case,
            &RunContext::new(later, later),
            true,
            false,
            false,
            "invoice_a",
        );
        assert_eq!(state.status, DutyStatus::Performed);
        assert!(
            !state.breached,
            "querying later must not turn timely performance into breach"
        );
    }

    #[test]
    fn relabeling_a_performed_payload_as_correction_does_not_authorize_it() {
        let t = instant("2033-01-01T00:00:00Z");
        let mut case = CaseRecord::default();
        case.facts.insert("invoice_date".into(), Value::Instant(t));
        case.events.push(LedgerEvent {
            kind: "correction".into(),
            valid_time: Interval::always(),
            record_time: t,
            payload: Value::Ctor {
                name: "Performed".into(),
                fields: BTreeMap::new(),
            },
        });
        let duty = pay_invoice_duty_after(30, "invoice_date");
        let state = surface_duty_state(
            &duty,
            &case,
            &RunContext::new(t, t),
            true,
            false,
            true,
            "invoice_a",
        );
        assert_ne!(
            state.status,
            DutyStatus::Performed,
            "an ungated event label must not admit a performed payload"
        );
    }

    #[test]
    fn named_instance_ignores_global_performed_shortcuts() {
        let t = instant("2033-01-01T00:00:00Z");
        let mut case = CaseRecord::default();
        case.facts.insert("invoice_date".into(), Value::Instant(t));
        case.facts.insert("performed".into(), Value::Bool(true));
        case.facts
            .insert("PayInvoice_performed".into(), Value::Bool(true));
        let duty = pay_invoice_duty_after(30, "invoice_date");
        let named = surface_duty_state(
            &duty,
            &case,
            &RunContext::new(t, t),
            true,
            false,
            true,
            "invoice_a",
        );
        assert_ne!(named.status, DutyStatus::Performed);
        let default = surface_duty_state(
            &duty,
            &case,
            &RunContext::new(t, t),
            true,
            false,
            true,
            DEFAULT_DUTY_INSTANCE,
        );
        assert_ne!(default.status, DutyStatus::Performed);
    }

    #[test]
    fn unpaid_deadline_uses_valid_time_not_record_time() {
        let invoice = instant("2033-01-01T00:00:00Z");
        let valid = instant("2033-01-15T00:00:00Z");
        let known = instant("2033-03-01T00:00:00Z");
        let mut case = CaseRecord::default();
        case.facts
            .insert("invoice_date".into(), Value::Instant(invoice));
        let duty = pay_invoice_duty_after(30, "invoice_date");
        let state = surface_duty_state(
            &duty,
            &case,
            &RunContext::new(valid, known),
            true,
            false,
            false,
            DEFAULT_DUTY_INSTANCE,
        );
        assert_eq!(state.status, DutyStatus::Attached);
        assert!(!state.breached);
    }

    #[test]
    fn due_after_ident_is_the_deadline_anchor() {
        let issued = instant("2033-02-01T00:00:00Z");
        let invoice = instant("2033-01-01T00:00:00Z");
        let at = instant("2033-02-15T00:00:00Z");
        let mut case = CaseRecord::default();
        case.facts
            .insert("invoice_date".into(), Value::Instant(invoice));
        case.facts
            .insert("issued_on".into(), Value::Instant(issued));
        let duty = pay_invoice_duty_after(30, "issued_on");
        let state = surface_duty_state(
            &duty,
            &case,
            &RunContext::new(at, at),
            true,
            false,
            false,
            DEFAULT_DUTY_INSTANCE,
        );
        assert_eq!(
            state.status,
            DutyStatus::Attached,
            "deadline must follow `after issued_on`, not hardcoded invoice_date"
        );
    }

    #[test]
    fn occurred_at_not_observed_at_decides_lateness() {
        let invoice = instant("2033-01-01T00:00:00Z");
        let occurred = instant("2033-01-10T00:00:00Z");
        let observed = instant("2033-02-20T00:00:00Z");
        let query = instant("2033-02-20T00:00:00Z");
        let mut case = CaseRecord::default();
        case.facts
            .insert("invoice_date".into(), Value::Instant(invoice));
        case.evidence.push(EvidenceItem {
            schema: "PaymentRecord".into(),
            observed_at: observed,
            value: Value::Map(BTreeMap::from([
                ("name".into(), Value::String("PayInvoice".into())),
                ("instance".into(), Value::String("invoice_a".into())),
                ("occurred_at".into(), Value::Instant(occurred)),
                ("payer".into(), Value::String("Payer".into())),
                ("payee".into(), Value::String("Payee".into())),
                ("amount".into(), Value::Int(100)),
                ("currency".into(), Value::String("USD".into())),
            ])),
        });
        let duty = pay_invoice_duty_after(30, "invoice_date");
        let state = surface_duty_state(
            &duty,
            &case,
            &RunContext::new(query, query),
            true,
            false,
            false,
            "invoice_a",
        );
        assert_eq!(state.status, DutyStatus::Performed);
        assert!(!state.breached);
    }

    #[test]
    fn missing_occurrence_time_is_not_silently_late() {
        let invoice = instant("2033-01-01T00:00:00Z");
        let later = instant("2033-02-15T00:00:00Z");
        let mut case = CaseRecord::default();
        case.facts
            .insert("invoice_date".into(), Value::Instant(invoice));
        case.facts.insert(
            duty_instance_key("PayInvoice", DEFAULT_DUTY_INSTANCE),
            duty_state_value(&DutyState {
                name: "PayInvoice".into(),
                status: DutyStatus::Performed,
                breached: false,
                bearer: "Payer".into(),
                claimant: None,
                instance: DEFAULT_DUTY_INSTANCE.into(),
            }),
        );
        let duty = pay_invoice_duty_after(30, "invoice_date");
        let state = surface_duty_state(
            &duty,
            &case,
            &RunContext::new(later, later),
            true,
            false,
            false,
            DEFAULT_DUTY_INSTANCE,
        );
        assert_eq!(state.status, DutyStatus::Performed);
        assert!(
            !state.breached,
            "unknown occurrence time must not be converted into lateness"
        );
    }

    #[test]
    fn scenario_overlay_keeps_assumption_ids_and_does_not_append_events() {
        let t = Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let payload = Value::Ctor {
            name: "Performed".into(),
            fields: BTreeMap::new(),
        };
        let mut case = CaseRecord::default();
        case.assumptions.push(Assumption {
            id: "hyp-performed".into(),
            payload: payload.clone(),
        });
        case.events.push(LedgerEvent {
            kind: "assumption".into(),
            valid_time: Interval::always(),
            record_time: t,
            payload,
        });
        let events_before = case.events.clone();
        let assumptions_before = case.assumptions.clone();
        let overlay = scenario_overlay_case(&case);
        assert_eq!(case.events, events_before);
        assert_eq!(overlay.events, events_before);
        assert_eq!(overlay.assumptions, assumptions_before);
        assert_eq!(overlay.assumptions[0].id, "hyp-performed");
        assert_eq!(overlay.facts.get("performed"), Some(&Value::Bool(true)));
        assert!(!case.facts.contains_key("performed"));
    }
}
