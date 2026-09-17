//! Duty status machine. Illegal or unauthorized transitions commit nothing.

use fidryn_core::ir::CoreDuty;
use fidryn_core::time::{CalendarKind, Instant};
use fidryn_core::value::{Term, Value};
use fidryn_core::{
    CaseRecord, DutyState, DutyStatus, EngineError, EvidenceItem, LedgerEvent, RunContext,
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
    if let Some(grants) = case.facts.get("authority_grants")
        && value_grants_action(grants, action)
    {
        return true;
    }
    case.evidence.iter().any(|item| {
        item.schema.eq_ignore_ascii_case("AuthorityGrant")
            && item.observed_at <= record_time
            && value_grants_action(&item.value, action)
    })
}

fn value_grants_action(value: &Value, action: &str) -> bool {
    match value {
        Value::String(name) | Value::Entity(name) => name.eq_ignore_ascii_case(action),
        Value::Ctor { name, fields } => {
            name.eq_ignore_ascii_case(action)
                || (name.eq_ignore_ascii_case("AuthorityGrant")
                    && fields
                        .get("action")
                        .is_some_and(|v| value_grants_action(v, action)))
                || fields.values().any(|v| value_grants_action(v, action))
        }
        Value::Set(items) => items.iter().any(|item| value_grants_action(item, action)),
        Value::Map(fields) => {
            if let Some(named) = fields.get("action") {
                return value_grants_action(named, action);
            }
            fields.iter().any(|(key, val)| match val {
                Value::Bool(true) => key.eq_ignore_ascii_case(action),
                _ => key.eq_ignore_ascii_case(action) || value_grants_action(val, action),
            })
        }
        _ => false,
    }
}

/// `duty` / `authority` / institutional events need a covering grant.
/// Assumption events are not operative duty performance.
pub fn event_is_admitted(case: &CaseRecord, event: &LedgerEvent, record_time: Instant) -> bool {
    if event.record_time > record_time {
        return false;
    }
    if event.kind.eq_ignore_ascii_case("assumption") {
        return false;
    }
    if is_gated_event_kind(&event.kind) {
        return event_action(&event.payload)
            .is_some_and(|action| action_is_granted(case, &action, record_time));
    }
    true
}

fn is_gated_event_kind(kind: &str) -> bool {
    kind.eq_ignore_ascii_case("duty")
        || kind.eq_ignore_ascii_case("authority")
        || kind.eq_ignore_ascii_case("institutional")
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
            .or_else(|| fields.get("status").and_then(value_action_name)),
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
    let performed = performed_held || performance_exists(&duty.name, &instance, case, ctx);
    let deadline_passed = deadline_has_passed(&duty.name, &duty.content, case, ctx);
    let already_breached = fact_flag(case, &format!("{}_breached", duty.name)).unwrap_or(false)
        || stored_duty_breached(&duty.name, &instance, case);
    if performed {
        let late = already_breached
            || deadline_passed
            || performance_is_after_deadline(&duty.name, &instance, &duty.content, case, ctx);
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

fn performance_exists(name: &str, instance: &str, case: &CaseRecord, ctx: &RunContext) -> bool {
    if fact_flag(case, &format!("{name}_performed")).unwrap_or(false)
        || fact_flag(case, "performed").unwrap_or(false)
    {
        return true;
    }
    if stored_duty_performed(name, instance, case) {
        return true;
    }
    if case.evidence.iter().any(|item| {
        item.observed_at <= ctx.record_time && payment_matches_instance(item, name, instance)
    }) {
        return true;
    }
    case.events.iter().any(|event| {
        event_is_admitted(case, event, ctx.record_time)
            && event_marks_performed(event, name, instance)
    })
}

fn payment_matches_instance(item: &EvidenceItem, duty_name: &str, instance: &str) -> bool {
    if !item.schema.eq_ignore_ascii_case("PaymentRecord") {
        return false;
    }
    match &item.value {
        Value::String(_) | Value::Entity(_) => instance.eq_ignore_ascii_case(DEFAULT_DUTY_INSTANCE),
        Value::Map(fields) | Value::Ctor { fields, .. } => {
            if duty_field_mismatch(fields, duty_name) {
                return false;
            }
            match instance_from_fields(fields) {
                Some(named) => named.eq_ignore_ascii_case(instance),
                None => instance.eq_ignore_ascii_case(DEFAULT_DUTY_INSTANCE),
            }
        }
        _ => instance.eq_ignore_ascii_case(DEFAULT_DUTY_INSTANCE),
    }
}

fn event_marks_performed(event: &LedgerEvent, duty_name: &str, instance: &str) -> bool {
    if !event.kind.eq_ignore_ascii_case("duty") {
        return false;
    }
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

fn deadline_has_passed(name: &str, content: &[Term], case: &CaseRecord, ctx: &RunContext) -> bool {
    if fact_flag(case, &format!("{name}_deadline_passed")).unwrap_or(false) {
        return true;
    }
    match due_deadline_instant(content, case) {
        Some(deadline) => ctx.record_time > deadline,
        None => fact_flag(case, &format!("{name}_late")).unwrap_or(false),
    }
}

fn performance_is_after_deadline(
    name: &str,
    instance: &str,
    content: &[Term],
    case: &CaseRecord,
    ctx: &RunContext,
) -> bool {
    let Some(deadline) = due_deadline_instant(content, case) else {
        return fact_flag(case, &format!("{name}_late")).unwrap_or(false)
            || fact_flag(case, &format!("{name}_deadline_passed")).unwrap_or(false);
    };
    match performance_instant(name, instance, case, ctx) {
        Some(at) => at > deadline,
        None => ctx.record_time > deadline,
    }
}

fn due_deadline_instant(content: &[Term], case: &CaseRecord) -> Option<Instant> {
    let days = due_counted_days(content)?;
    let start = fact_instant(case, "invoice_date").or_else(|| fact_instant(case, "due_at"))?;
    add_counted_days(start, days)
}

fn due_counted_days(content: &[Term]) -> Option<i64> {
    content.iter().find_map(find_due_days)
}

fn find_due_days(term: &Term) -> Option<i64> {
    match term {
        Term::Apply { ctor, args } | Term::Call { callee: ctor, args }
            if ctor.eq_ignore_ascii_case("due")
                || ctor.eq_ignore_ascii_case("after")
                || is_counted_ctor(ctor) =>
        {
            extract_days(args).or_else(|| args.iter().find_map(find_due_days))
        }
        Term::Apply { args, .. } | Term::Call { args, .. } | Term::Set(args) => {
            args.iter().find_map(find_due_days)
        }
        Term::Binary { left, right, .. } => find_due_days(left).or_else(|| find_due_days(right)),
        Term::If { cond, then, else_ } => find_due_days(cond)
            .or_else(|| find_due_days(then))
            .or_else(|| find_due_days(else_)),
        Term::Field { base, .. } => find_due_days(base),
        Term::Record(fields) => fields.values().find_map(find_due_days),
        Term::Duration(duration) if is_counted_calendar(duration.kind) => Some(duration.amount),
        _ => None,
    }
}

fn extract_days(args: &[Term]) -> Option<i64> {
    for arg in args {
        match arg {
            Term::Int(n) => return Some(*n),
            Term::Duration(duration) => return Some(duration.amount),
            Term::Apply { ctor, args } | Term::Call { callee: ctor, args }
                if is_counted_ctor(ctor) =>
            {
                if let Some(Term::Int(n)) = args.first() {
                    return Some(*n);
                }
            }
            _ => {}
        }
    }
    args.iter().find_map(find_due_days)
}

fn is_counted_ctor(name: &str) -> bool {
    name.eq_ignore_ascii_case("counted_days")
        || name.eq_ignore_ascii_case("days")
        || name.eq_ignore_ascii_case("calendar_days")
}

fn is_counted_calendar(kind: CalendarKind) -> bool {
    matches!(
        kind,
        CalendarKind::CountedDays | CalendarKind::Days | CalendarKind::CalendarDays
    )
}

fn add_counted_days(start: Instant, days: i64) -> Option<Instant> {
    start
        .as_offset()
        .checked_add(time::Duration::days(days))
        .map(Instant::from_offset)
}

fn performance_instant(
    name: &str,
    instance: &str,
    case: &CaseRecord,
    ctx: &RunContext,
) -> Option<Instant> {
    let from_evidence = case
        .evidence
        .iter()
        .filter(|item| {
            item.observed_at <= ctx.record_time && payment_matches_instance(item, name, instance)
        })
        .map(|item| item.observed_at)
        .min();
    if from_evidence.is_some() {
        return from_evidence;
    }
    case.events
        .iter()
        .filter(|event| {
            event_is_admitted(case, event, ctx.record_time)
                && event_marks_performed(event, name, instance)
        })
        .map(|event| event.record_time)
        .min()
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
    match case.facts.get(key) {
        Some(Value::Instant(instant)) => Some(*instant),
        Some(Value::String(text)) => Instant::parse(text)
            .ok()
            .or_else(|| Instant::parse(&format!("{text}T00:00:00Z")).ok()),
        _ => None,
    }
}

/// Clone `case` and apply assumption payloads to scenario-only facts.
/// Never appends to `events`.
pub fn scenario_overlay_case(case: &CaseRecord) -> CaseRecord {
    let mut overlay = case.clone();
    let assumptions = overlay.assumptions.clone();
    for assumption in &assumptions {
        apply_assumption_payload(&mut overlay, &assumption.payload);
    }
    let event_payloads: Vec<Value> = overlay
        .events
        .iter()
        .filter(|event| event.kind.eq_ignore_ascii_case("assumption"))
        .map(|event| event.payload.clone())
        .collect();
    for payload in &event_payloads {
        apply_assumption_payload(&mut overlay, payload);
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
    use fidryn_core::{Assumption, EvidenceItem};

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
            Value::Set(vec![Value::String("attach".into())]),
        );
        assert!(action_is_granted(&case, "attach", t));
        assert!(!action_is_granted(&case, "perform", t));

        let mut evidence_only = CaseRecord::default();
        evidence_only.evidence.push(EvidenceItem {
            schema: "AuthorityGrant".into(),
            value: Value::Map(BTreeMap::from([(
                "action".into(),
                Value::String("perform".into()),
            )])),
            observed_at: t,
        });
        assert!(action_is_granted(&evidence_only, "perform", t));
        assert!(!action_is_granted(&evidence_only, "attach", t));
        assert!(has_authority_constraint(&evidence_only));
    }

    #[test]
    fn duty_event_without_grant_is_not_admitted() {
        let t = Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let event = LedgerEvent {
            kind: "duty".into(),
            valid_time: fidryn_core::Interval::always(),
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
            valid_time: fidryn_core::Interval::always(),
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
            valid_time: fidryn_core::Interval::always(),
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
