//! Input descriptions derived from the checked Core declarations.

use fidryn_core::ir::{CoreConstructor, CoreDecl, CoreModule, TypeDefinition};
use fidryn_core::types::{PrimitiveType, Sort, Type, TypeEnvironment, is_subtype, resolve_aliases};
use fidryn_core::{Diagnostic, Span};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

/// JSON Schema for a query's argument object, with evidence and source metadata.
/// Unsupported input representations return an error instead of an unconstrained schema.
pub fn query_input_schema(module: &CoreModule, query: &str) -> Result<Value, String> {
    let query = module
        .query(query)
        .ok_or_else(|| format!("unknown query `{query}`"))?;
    let environment = module
        .declarations
        .iter()
        .filter_map(|declaration| match declaration {
            CoreDecl::Type(declaration) => Some((
                declaration.name.clone(),
                (
                    declaration.parameters.clone(),
                    declaration.definition.clone(),
                ),
            )),
            _ => None,
        })
        .collect();
    let mut builder = SchemaBuilder {
        module,
        environment,
        definitions: Map::new(),
        steps: 0,
    };
    let mut properties = Map::new();
    for (name, ty) in &query.binders {
        if properties
            .insert(name.clone(), builder.schema(ty, false)?)
            .is_some()
        {
            return Err(format!("duplicate query binder `{name}`"));
        }
    }
    let mut schema = closed_object(properties);
    schema["$schema"] = json!("https://json-schema.org/draft/2020-12/schema");
    schema["title"] = json!(format!("{}::{} arguments", module.name, query.name));
    schema["x-fidryn-schema"] = json!("fidryn.query-input-schema/v0.1");
    schema["x-fidryn-module"] = json!(module.name);
    schema["x-fidryn-query"] = json!(query.name);
    schema["x-fidryn-program-digest"] = json!(module.program_digest()?.to_string());
    schema["x-fidryn-result-type"] = json!(type_name(&query.result_type));
    schema["x-fidryn-effects"] = json!(
        query
            .effects
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    );
    let mut observations = Vec::new();
    for declaration in &module.declarations {
        if let CoreDecl::Observation(observation) = declaration {
            let (record_schema, schema_error) = match &observation.record_type {
                Some(ty) => match builder.schema(ty, false) {
                    Ok(schema) => (Some(schema), None),
                    Err(error) => (None, Some(error)),
                },
                None => (None, Some("observation has no declared record type".into())),
            };
            observations.push(json!({
                "name": observation.name, "binder": observation.record_binder,
                "record_type": observation.record_type.as_ref().map(type_name),
                "record_schema": record_schema, "schema_error": schema_error,
                "validators": observation.validator_terms, "legacy_validators": observation.validators,
                "establishes": observation.establishes, "source": observation.source,
                "origin": observation.meta, "unsupported_clauses": observation.unsupported_clauses,
            }));
        }
    }
    schema["x-fidryn-observations"] = json!(observations);
    schema["x-fidryn-record-types"] = json!(module.declarations.iter().filter_map(|declaration| match declaration {
        CoreDecl::RecordType(record) => Some(json!({
            "name":record.name,"evidence":record.is_evidence,
            "fields":record.fields.iter().map(|(name,ty)|json!({"name":name,"type":type_name(ty)})).collect::<Vec<_>>(),
            "origin":record.meta,
        })), _=>None,
    }).collect::<Vec<_>>());
    schema["x-fidryn-effect-requests"] = json!(module.declarations.iter().filter_map(|declaration| match declaration {
        CoreDecl::EffectDecl(effect) => Some(json!({
            "name":effect.name,
            "operations":effect.operations.iter().map(|operation|json!({
                "name":operation.name,
                "parameters":operation.params.iter().map(|(name,ty)|json!({"name":name,"type":type_name(ty)})).collect::<Vec<_>>(),
                "result_type":type_name(&operation.result),
            })).collect::<Vec<_>>(),"origin":effect.meta,
        })), _=>None,
    }).collect::<Vec<_>>());
    schema["$defs"] = Value::Object(builder.definitions);
    Ok(schema)
}

struct SchemaBuilder<'a> {
    module: &'a CoreModule,
    environment: TypeEnvironment,
    definitions: Map<String, Value>,
    steps: usize,
}

impl SchemaBuilder<'_> {
    fn schema(&mut self, ty: &Type, canonical: bool) -> Result<Value, String> {
        self.steps += 1;
        if self.steps > 10_000 {
            return Err("input schema expansion exceeds 10000 nodes".into());
        }
        let resolved = resolve_aliases(ty, &self.environment)?;
        let mut schema = match &resolved {
            Type::Primitive(primitive) => self.primitive(primitive, canonical)?,
            Type::Applied {ctor, args} if ctor == "Union" => json!({"anyOf":args.iter().map(|ty|self.schema(ty,canonical)).collect::<Result<Vec<_>,_>>()?}),
            Type::Sort(Sort::Nominal(name)) => self.nominal(name, &[], canonical)?,
            Type::Applied {ctor, args} => self.nominal(ctor, args, canonical)?,
            Type::Sort(Sort::Prop) => return Err("Prop input requires a resolved proposition expression; no generic input editor is available".into()),
            Type::Sort(Sort::ClauseRef) => {
                if canonical { return Err("canonical ClauseRef input requires a bound clause digest; use a declared clause name".into()); }
                let names = self.module.declarations.iter().filter_map(|declaration| match declaration {
                    CoreDecl::Clause(clause) if clause.binders.is_empty()=>Some(clause.name.clone()),_=>None,
                }).collect::<Vec<_>>();
                choices(names, "no parameter-free clauses are declared")
            }
            Type::Sort(_) => self.entities(&resolved, canonical)?,
        };
        if let Some(object) = schema.as_object_mut() {
            object.insert("x-fidryn-type".into(), json!(type_name(ty)));
        }
        Ok(schema)
    }

    fn primitive(&mut self, primitive: &PrimitiveType, canonical: bool) -> Result<Value, String> {
        use PrimitiveType as P;
        let scalar = |kind, data: Value| {
            if canonical {
                tagged(kind, data)
            } else {
                json!({"anyOf":[data.clone(),tagged(kind,data)]})
            }
        };
        Ok(match primitive {
            P::Bool => scalar("bool", json!({"type":"boolean"})),
            P::Int => scalar(
                "int",
                json!({"type":"integer","minimum":i64::MIN,"maximum":i64::MAX}),
            ),
            P::Decimal => {
                let tagged = tagged(
                    "decimal",
                    json!({"type":"string","pattern":"^-?[0-9]+(\\.[0-9]+)?$"}),
                );
                if canonical {
                    tagged
                } else {
                    json!({"anyOf":[tagged,{"type":"number"}]})
                }
            }
            P::String => scalar("string", json!({"type":"string"})),
            P::Time | P::Date => scalar("instant", json!({"type":"string","format":"date-time"})),
            P::Money { currency } => {
                let decimal = self.schema(&Type::Primitive(P::Decimal), true)?;
                let amount = json!({"anyOf":[decimal,self.schema(&Type::Primitive(P::Int),true)?]});
                let ctor = constructor(
                    currency,
                    closed_object(Map::from_iter([("_0".into(), amount)])),
                );
                if canonical {
                    ctor
                } else {
                    json!({"anyOf":[ctor,{"type":"number"}]})
                }
            }
            P::Duration { calendar } => tagged(
                "duration",
                closed_object(Map::from_iter([
                    (
                        "amount".into(),
                        json!({"type":"integer","minimum":i64::MIN,"maximum":i64::MAX}),
                    ),
                    ("kind".into(), json!({"const":calendar})),
                ])),
            ),
            P::FiniteSet { inner } | P::NonEmptySet { inner } => {
                let minimum = usize::from(matches!(primitive, P::NonEmptySet { .. }));
                let tagged = tagged(
                    "set",
                    json!({"type":"array","items":self.schema(inner,true)?,"uniqueItems":true,"minItems":minimum}),
                );
                if canonical {
                    tagged
                } else {
                    json!({"anyOf":[{"type":"array","items":self.schema(inner,false)?,"uniqueItems":true,"minItems":minimum},tagged]})
                }
            }
            P::Option { inner } => {
                let tagged = tagged(
                    "option",
                    json!({"anyOf":[{"type":"null"},self.schema(inner,true)?]}),
                );
                if canonical {
                    tagged
                } else {
                    json!({"anyOf":[{"type":"null"},tagged]})
                }
            }
            P::Map { key, value } => {
                if resolve_aliases(key, &self.environment)? != Type::Primitive(P::String) {
                    return Err(format!(
                        "map input keys must resolve to String, found {}",
                        type_name(key)
                    ));
                }
                let tagged = tagged(
                    "map",
                    json!({"type":"object","additionalProperties":self.schema(value,true)?}),
                );
                if canonical {
                    tagged
                } else {
                    json!({"anyOf":[{"type":"object","additionalProperties":self.schema(value,false)?,"not":reserved_kind()},tagged]})
                }
            }
            P::Interval { .. } => {
                return Err("Interval input has no runtime value representation".into());
            }
        })
    }

    fn nominal(
        &mut self,
        name: &str,
        arguments: &[Type],
        canonical: bool,
    ) -> Result<Value, String> {
        let key = format!(
            "{}:{}:{}",
            if canonical { "value" } else { "input" },
            name,
            arguments
                .iter()
                .map(type_name)
                .collect::<Vec<_>>()
                .join(",")
        );
        let reference =
            json!({"$ref":format!("#/$defs/{}",key.replace('~',"~0").replace('/',"~1"))});
        if self.definitions.contains_key(&key) {
            return Ok(reference);
        }
        if self.definitions.len() >= 512 {
            return Err("input schema exceeds 512 named definitions".into());
        }
        self.definitions.insert(key.clone(), Value::Bool(false));
        let schema = self.nominal_definition(name, arguments, canonical);
        match schema {
            Ok(schema) => {
                self.definitions.insert(key, schema);
                Ok(reference)
            }
            Err(error) => {
                self.definitions.remove(&key);
                Err(error)
            }
        }
    }

    fn nominal_definition(
        &mut self,
        name: &str,
        arguments: &[Type],
        canonical: bool,
    ) -> Result<Value, String> {
        if name == "Unit" {
            return Ok(if canonical {
                json!({"type":"object","properties":{"kind":{"const":"unit"}},"required":["kind"],"additionalProperties":false})
            } else {
                json!({"type":"null"})
            });
        }
        let record = self
            .module
            .declarations
            .iter()
            .find_map(|declaration| match declaration {
                CoreDecl::RecordType(record) if record.name == name => Some(record.clone()),
                _ => None,
            });
        if let Some(record) = record {
            if !arguments.is_empty() {
                return Err(format!(
                    "record `{name}` does not declare generic parameters"
                ));
            }
            let fields = record
                .fields
                .iter()
                .map(|(name, ty)| Ok((name.clone(), self.schema(ty, canonical)?)))
                .collect::<Result<Map<_, _>, String>>()?;
            let mut record_schema = closed_object(fields);
            if canonical {
                return Ok(tagged("map", record_schema));
            }
            record_schema["not"] = reserved_kind();
            let escaped = tagged(
                "record",
                closed_object(
                    record
                        .fields
                        .iter()
                        .map(|(name, ty)| Ok((name.clone(), self.schema(ty, false)?)))
                        .collect::<Result<Map<_, _>, String>>()?,
                ),
            );
            return Ok(json!({"anyOf":[record_schema,escaped]}));
        }
        if let Some((parameters, TypeDefinition::Sum(variants))) =
            self.environment.get(name).cloned()
        {
            if parameters.len() != arguments.len() {
                return Err(format!(
                    "type `{name}` requires {} arguments",
                    parameters.len()
                ));
            }
            let mut schemas = Vec::new();
            for variant in variants {
                schemas.push(self.variant(&variant, &parameters, arguments, canonical)?);
            }
            return Ok(json!({"anyOf":schemas}));
        }
        let singleton = self
            .environment
            .values()
            .find_map(|(_, definition)| match definition {
                TypeDefinition::Sum(variants) => variants
                    .iter()
                    .find(|variant| variant.name == name)
                    .cloned(),
                _ => None,
            });
        if let Some(singleton) = singleton {
            return self.variant(&singleton, &[], &[], canonical);
        }
        let ty = Type::Sort(Sort::Nominal(name.into()));
        if self.module.declarations.iter().any(|declaration|matches!(declaration,CoreDecl::Entity(entity) if resolve_aliases(&entity.ty,&self.environment).ok().as_ref()==Some(&ty))) {return self.entities(&ty,canonical);}
        Err(format!(
            "unsupported input type `{name}`: no record, sum, alias, or declared entity supplies its values"
        ))
    }

    fn variant(
        &mut self,
        variant: &CoreConstructor,
        parameters: &[String],
        arguments: &[Type],
        canonical: bool,
    ) -> Result<Value, String> {
        let fields = if variant.fields.is_empty() {
            variant
                .arguments
                .iter()
                .enumerate()
                .map(|(index, ty)| (format!("_{index}"), ty.clone()))
                .collect()
        } else {
            variant.fields.clone()
        };
        let fields = fields
            .iter()
            .map(|(name, ty)| {
                Ok((
                    name.clone(),
                    self.schema(
                        &fidryn_check::substitute_type(ty.clone(), parameters, arguments),
                        true,
                    )?,
                ))
            })
            .collect::<Result<Map<_, _>, String>>()?;
        let empty = fields.is_empty();
        let tagged = constructor(&variant.name, closed_object(fields));
        Ok(if !canonical && empty {
            json!({"anyOf":[{"const":variant.name},tagged]})
        } else {
            tagged
        })
    }

    fn entities(&self, ty: &Type, canonical: bool) -> Result<Value, String> {
        let mut names = Vec::new();
        for declaration in &self.module.declarations {
            if let CoreDecl::Entity(entity) = declaration
                && is_subtype(&resolve_aliases(&entity.ty, &self.environment)?, ty)
            {
                names.push(entity.name.clone());
            }
        }
        let data = choices(names, "no entities of this declared sort are available");
        Ok(if canonical {
            tagged("entity", data)
        } else {
            json!({"anyOf":[data.clone(),tagged("entity",data)]})
        })
    }
}

fn choices(values: Vec<String>, unavailable: &str) -> Value {
    if values.is_empty() {
        json!({"allOf":[false],"x-fidryn-unavailable":unavailable})
    } else {
        json!({"type":"string","enum":values})
    }
}

fn closed_object(properties: Map<String, Value>) -> Value {
    let required = properties.keys().cloned().collect::<Vec<_>>();
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}

fn tagged(kind: &str, data: Value) -> Value {
    closed_object(Map::from_iter([
        ("kind".into(), json!({"const":kind})),
        ("data".into(), data),
    ]))
}

fn constructor(name: &str, fields: Value) -> Value {
    tagged(
        "ctor",
        closed_object(Map::from_iter([
            ("name".into(), json!({"const":name})),
            ("fields".into(), fields),
        ])),
    )
}

fn reserved_kind() -> Value {
    json!({"properties":{"kind":{"enum":["unit","bool","int","decimal","string","instant","duration","prop","entity","ctor","set","map","option","clauseRef","record"]}},"required":["kind"]})
}

fn type_name(ty: &Type) -> String {
    match ty {
        Type::Sort(Sort::Nominal(name)) => name.clone(),
        Type::Sort(sort) => format!("{sort:?}"),
        Type::Applied { ctor, args } => format!(
            "{ctor}<{}>",
            args.iter().map(type_name).collect::<Vec<_>>().join(", ")
        ),
        Type::Primitive(PrimitiveType::Money { currency }) => format!("Money<{currency}>"),
        Type::Primitive(PrimitiveType::Duration { calendar }) => format!("Duration<{calendar}>"),
        Type::Primitive(PrimitiveType::Option { inner }) => format!("Option<{}>", type_name(inner)),
        Type::Primitive(PrimitiveType::FiniteSet { inner }) => {
            format!("FiniteSet<{}>", type_name(inner))
        }
        Type::Primitive(PrimitiveType::NonEmptySet { inner }) => {
            format!("NonEmptySet<{}>", type_name(inner))
        }
        Type::Primitive(PrimitiveType::Interval { inner }) => {
            format!("Interval<{}>", type_name(inner))
        }
        Type::Primitive(PrimitiveType::Map { key, value }) => {
            format!("Map<{}, {}>", type_name(key), type_name(value))
        }
        Type::Primitive(primitive) => format!("{primitive:?}"),
    }
}

/// Source ranges use zero-based lines and UTF-16 columns for editor protocols.
/// Missing source files and invalid byte spans remain explicitly unavailable.
pub fn diagnostics_data(diagnostics: &[Diagnostic], sources: &BTreeMap<String, String>) -> Value {
    let maximum = 1000;
    let locations=diagnostics.iter().take(maximum).map(|diagnostic|{
        let source=diagnostic.source_file.as_ref().and_then(|name|sources.get(name));
        json!({
            "code":format!("{:?}",diagnostic.code),"message":diagnostic.message,
            "severity":format!("{:?}",diagnostic.code.severity()).to_ascii_lowercase(),
            "source_file":diagnostic.source_file,"suggestion":diagnostic.suggestion,
            "primary":diagnostic.primary_span.map(|span|span_data(span,source.map(String::as_str))),
            "related":diagnostic.related_spans.iter().map(|span|span_data(*span,source.map(String::as_str))).collect::<Vec<_>>(),
        })
    }).collect::<Vec<_>>();
    json!({"schema":"fidryn.editor-diagnostics/v0.1","diagnostics":locations,"omitted":diagnostics.len().saturating_sub(maximum)})
}

fn span_data(span: Span, source: Option<&str>) -> Value {
    let position = |offset: u32| -> Option<Value> {
        let source = source?;
        let offset = offset as usize;
        if offset > source.len() || !source.is_char_boundary(offset) {
            return None;
        }
        let prefix = &source[..offset];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count();
        let start = prefix.rfind('\n').map_or(0, |index| index + 1);
        Some(json!({"line":line,"character":source[start..offset].encode_utf16().count()}))
    };
    let range = if span.start <= span.end {
        position(span.start)
            .zip(position(span.end))
            .map(|(start, end)| json!({"start":start,"end":end}))
    } else {
        None
    };
    json!({"byte_span":span,"range":range,"location_available":range.is_some()})
}
