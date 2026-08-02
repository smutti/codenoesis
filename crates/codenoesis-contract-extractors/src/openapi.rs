use std::collections::{BTreeMap, BTreeSet};

use codenoesis_domain::s6::{
    ContractError, CoverageGap, FederationEvidence, FederationLimit, HttpMethod, JsonSchemaType,
    LimitExceeded, OpenApiContractInput, OperationField, ProviderBinding, ProviderContract,
    ProviderOperation, ResourceCounter, SourceFormat, contract_gap_id, field_id, operation_id,
    schema_id, service_id,
};
use codenoesis_ports::OpenApiContractExtractor;
use serde_json::{Map, Number, Value};
use yaml_rust2::parser::{Event, MarkedEventReceiver, Parser};
use yaml_rust2::scanner::{Marker, TScalarStyle};

#[derive(Clone, Copy, Debug, Default)]
pub struct OpenApi31HttpJsonExtractor;

impl OpenApi31HttpJsonExtractor {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl OpenApiContractExtractor for OpenApi31HttpJsonExtractor {
    fn extract(&self, input: OpenApiContractInput<'_>) -> Result<ProviderContract, ContractError> {
        extract_contract(input)
    }
}

struct ParsedDocument {
    value: Value,
    spans: BTreeMap<String, Span>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Span {
    start_line: u64,
    end_line: u64,
}

fn extract_contract(input: OpenApiContractInput<'_>) -> Result<ProviderContract, ContractError> {
    let path = input.binding.contract_path.clone();
    let mut counter = ResourceCounter::new();
    charge_contract(&mut counter, FederationLimit::ContractDocuments, 1, &path)?;
    charge_contract(
        &mut counter,
        FederationLimit::ContractBytesPerDocument,
        u64::try_from(input.bytes.len()).unwrap_or(u64::MAX),
        &path,
    )?;
    let source = std::str::from_utf8(input.bytes)
        .map_err(|_| ContractError::InvalidEncoding { path: path.clone() })?;
    let parsed = match input.binding.source_format {
        SourceFormat::Json => ParsedDocument {
            value: serde_json::from_str(source)
                .map_err(|_| ContractError::InvalidOperation { path: path.clone() })?,
            spans: BTreeMap::new(),
        },
        SourceFormat::Yaml => parse_restricted_yaml(source, &path)?,
    };
    normalize_openapi(&parsed, input.binding, &mut counter)
}

#[allow(clippy::too_many_lines)]
fn normalize_openapi(
    parsed: &ParsedDocument,
    binding: ProviderBinding,
    counter: &mut ResourceCounter,
) -> Result<ProviderContract, ContractError> {
    let path = binding.contract_path.clone();
    let root = parsed
        .value
        .as_object()
        .ok_or_else(|| ContractError::InvalidOperation { path: path.clone() })?;
    if root.get("openapi").and_then(Value::as_str) != Some("3.1.0") {
        return Err(ContractError::UnsupportedOpenApiVersion { path });
    }
    let info = root
        .get("info")
        .and_then(Value::as_object)
        .ok_or_else(|| ContractError::InvalidOperation { path: path.clone() })?;
    let title = required_string(info, "title", 256, &path)?;
    validate_all_references(&parsed.value, &path)?;

    let servers = root
        .get("servers")
        .and_then(Value::as_array)
        .ok_or_else(|| ContractError::InvalidServiceAuthority { path: path.clone() })?;
    let mut authorities = BTreeSet::new();
    let mut matching_server = None;
    for (index, server) in servers.iter().enumerate() {
        let server = server
            .as_object()
            .ok_or_else(|| ContractError::InvalidServiceAuthority { path: path.clone() })?;
        let authority = server
            .get("url")
            .and_then(Value::as_str)
            .filter(|value| valid_service_authority(value))
            .ok_or_else(|| ContractError::InvalidServiceAuthority { path: path.clone() })?;
        authorities.insert(authority);
        if authority == binding.service_authority {
            matching_server = Some(index);
        }
    }
    if authorities.len() != 1 || matching_server.is_none() {
        return Err(ContractError::InvalidServiceAuthority { path });
    }
    let server_index = matching_server.expect("matching server checked");
    let service_id = service_id(&binding.service_authority);
    let server_pointer = format!("/servers/{server_index}/url");
    let server_evidence = evidence_for(&binding, &parsed.spans, &server_pointer)?;
    let server_evidence_id = server_evidence.evidence_id.clone();

    let components = root.get("components").and_then(Value::as_object);
    let schemas = components
        .and_then(|value| value.get("schemas"))
        .and_then(Value::as_object);
    charge_contract(
        counter,
        FederationLimit::Schemas,
        schemas.map_or(0, Map::len).try_into().unwrap_or(u64::MAX),
        &binding.contract_path,
    )?;
    let paths = root
        .get("paths")
        .and_then(Value::as_object)
        .ok_or_else(|| ContractError::InvalidOperation {
            path: binding.contract_path.clone(),
        })?;
    charge_contract(
        counter,
        FederationLimit::PathItems,
        u64::try_from(paths.len()).unwrap_or(u64::MAX),
        &binding.contract_path,
    )?;

    let mut evidence_by_id = BTreeMap::new();
    insert_evidence(counter, &mut evidence_by_id, server_evidence, &path)?;
    let mut provider_evidence_ids = vec![server_evidence_id];
    let mut operations = Vec::new();
    let mut coverage_gaps = Vec::new();

    add_service_gaps(
        root,
        &binding,
        &parsed.spans,
        &service_id,
        counter,
        &mut evidence_by_id,
        &mut coverage_gaps,
    )?;
    if let Some(servers) = root.get("servers").and_then(Value::as_array) {
        for (index, server) in servers.iter().enumerate() {
            if server
                .as_object()
                .is_some_and(|object| object.contains_key("variables"))
            {
                add_contract_gap(
                    &binding,
                    &parsed.spans,
                    &service_id,
                    "unsupported_server_variables",
                    &format!("/servers/{index}/variables"),
                    counter,
                    &mut evidence_by_id,
                    &mut coverage_gaps,
                )?;
            }
        }
    }

    for (path_template, path_item) in paths {
        if !valid_path_template(path_template) {
            return Err(ContractError::InvalidOperation {
                path: binding.contract_path.clone(),
            });
        }
        let path_item = path_item
            .as_object()
            .ok_or_else(|| ContractError::InvalidOperation {
                path: binding.contract_path.clone(),
            })?;
        for (method_name, method) in supported_methods() {
            let Some(operation_value) = path_item.get(method_name) else {
                continue;
            };
            charge_contract(
                counter,
                FederationLimit::Operations,
                1,
                &binding.contract_path,
            )?;
            let operation_object =
                operation_value
                    .as_object()
                    .ok_or_else(|| ContractError::InvalidOperation {
                        path: binding.contract_path.clone(),
                    })?;
            let explicit_operation_id =
                required_string(operation_object, "operationId", 256, &binding.contract_path)?;
            if !valid_operation_name(&explicit_operation_id) {
                return Err(ContractError::InvalidOperation {
                    path: binding.contract_path.clone(),
                });
            }
            let operation_id =
                operation_id(&service_id, method, path_template, &explicit_operation_id);
            let operation_pointer = format!(
                "/paths/{}/{method_name}",
                escape_pointer_segment(path_template)
            );
            let operation_evidence = evidence_for(&binding, &parsed.spans, &operation_pointer)?;
            let operation_evidence_id = operation_evidence.evidence_id.clone();
            insert_evidence(
                counter,
                &mut evidence_by_id,
                operation_evidence,
                &binding.contract_path,
            )?;
            add_operation_gaps(
                operation_object,
                &binding,
                &parsed.spans,
                &operation_id,
                &operation_pointer,
                counter,
                &mut evidence_by_id,
                &mut coverage_gaps,
            )?;
            let response = select_json_response(
                operation_object,
                &binding,
                &parsed.spans,
                &operation_id,
                &operation_pointer,
                counter,
                &mut evidence_by_id,
                &mut coverage_gaps,
            )?;
            let schema_pointer = response.schema_pointer;
            let (resolved_schema, component_pointer) = resolve_schema(
                &parsed.value,
                response.schema,
                &schema_pointer,
                &binding.contract_path,
                0,
                &mut Vec::new(),
            )?;
            let schema_evidence = evidence_for(&binding, &parsed.spans, &component_pointer)?;
            let schema_evidence_id = schema_evidence.evidence_id.clone();
            insert_evidence(
                counter,
                &mut evidence_by_id,
                schema_evidence,
                &binding.contract_path,
            )?;
            let schema_id = schema_id(
                &operation_id,
                "response",
                &response.status,
                &format!("#{component_pointer}"),
            );
            let mut fields = Vec::new();
            let mut field_counter = ResourceCounter::new();
            collect_fields(
                &parsed.value,
                resolved_schema,
                "",
                &operation_id,
                &response.status,
                &schema_evidence_id,
                &binding.contract_path,
                &mut field_counter,
                &mut fields,
            )?;
            fields.sort_by(|left, right| left.field_id.cmp(&right.field_id));
            let mut evidence_ids = vec![operation_evidence_id.clone(), schema_evidence_id];
            evidence_ids.sort();
            evidence_ids.dedup();
            provider_evidence_ids.extend(evidence_ids.iter().cloned());
            operations.push(ProviderOperation {
                operation_id,
                service_id: service_id.clone(),
                method,
                path_template: path_template.clone(),
                explicit_operation_id,
                response_status: response.status,
                schema_id,
                fields,
                evidence_ids,
                primary_evidence_id: operation_evidence_id,
            });
        }
    }
    operations.sort_by(|left, right| left.operation_id.cmp(&right.operation_id));
    provider_evidence_ids.sort();
    provider_evidence_ids.dedup();
    coverage_gaps.sort_by(|left, right| left.coverage_gap_id.cmp(&right.coverage_gap_id));

    Ok(ProviderContract {
        binding,
        service_id,
        title,
        operations,
        evidence: evidence_by_id.into_values().collect(),
        evidence_ids: provider_evidence_ids,
        coverage_gaps,
    })
}

struct JsonResponse<'a> {
    status: String,
    schema: &'a Value,
    schema_pointer: String,
}

#[allow(clippy::too_many_arguments)]
fn select_json_response<'a>(
    operation: &'a Map<String, Value>,
    binding: &ProviderBinding,
    spans: &BTreeMap<String, Span>,
    operation_id: &str,
    operation_pointer: &str,
    counter: &mut ResourceCounter,
    evidence: &mut BTreeMap<String, FederationEvidence>,
    gaps: &mut Vec<CoverageGap>,
) -> Result<JsonResponse<'a>, ContractError> {
    let responses = operation
        .get("responses")
        .and_then(Value::as_object)
        .ok_or_else(|| ContractError::InvalidOperation {
            path: binding.contract_path.clone(),
        })?;
    let mut selected = Vec::new();
    for (status, response) in responses {
        if !valid_response_status(status) {
            continue;
        }
        let response = response
            .as_object()
            .ok_or_else(|| ContractError::InvalidOperation {
                path: binding.contract_path.clone(),
            })?;
        let response_pointer = format!(
            "{operation_pointer}/responses/{}",
            escape_pointer_segment(status)
        );
        if response.contains_key("links") {
            add_contract_gap(
                binding,
                spans,
                operation_id,
                "unsupported_links",
                &format!("{response_pointer}/links"),
                counter,
                evidence,
                gaps,
            )?;
        }
        let Some(content) = response.get("content").and_then(Value::as_object) else {
            continue;
        };
        for media_type in content
            .keys()
            .filter(|key| key.as_str() != "application/json")
        {
            add_contract_gap(
                binding,
                spans,
                operation_id,
                "unsupported_media_type",
                &format!(
                    "{response_pointer}/content/{}",
                    escape_pointer_segment(media_type)
                ),
                counter,
                evidence,
                gaps,
            )?;
        }
        if let Some(json_media) = content.get("application/json").and_then(Value::as_object) {
            let schema =
                json_media
                    .get("schema")
                    .ok_or_else(|| ContractError::InvalidOperation {
                        path: binding.contract_path.clone(),
                    })?;
            selected.push(JsonResponse {
                status: status.clone(),
                schema,
                schema_pointer: format!("{response_pointer}/content/application~1json/schema"),
            });
        }
    }
    match selected.len() {
        1 => Ok(selected.remove(0)),
        _ => Err(ContractError::InvalidOperation {
            path: binding.contract_path.clone(),
        }),
    }
}

fn add_service_gaps(
    root: &Map<String, Value>,
    binding: &ProviderBinding,
    spans: &BTreeMap<String, Span>,
    service_id: &str,
    counter: &mut ResourceCounter,
    evidence: &mut BTreeMap<String, FederationEvidence>,
    gaps: &mut Vec<CoverageGap>,
) -> Result<(), ContractError> {
    for (key, reason) in [
        ("webhooks", "unsupported_webhooks"),
        ("security", "unsupported_security_semantics"),
    ] {
        if root.contains_key(key) {
            add_contract_gap(
                binding,
                spans,
                service_id,
                reason,
                &format!("/{key}"),
                counter,
                evidence,
                gaps,
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn add_operation_gaps(
    operation: &Map<String, Value>,
    binding: &ProviderBinding,
    spans: &BTreeMap<String, Span>,
    operation_id: &str,
    operation_pointer: &str,
    counter: &mut ResourceCounter,
    evidence: &mut BTreeMap<String, FederationEvidence>,
    gaps: &mut Vec<CoverageGap>,
) -> Result<(), ContractError> {
    for (key, reason) in [
        ("callbacks", "unsupported_callbacks"),
        ("security", "unsupported_security_semantics"),
    ] {
        if operation.contains_key(key) {
            add_contract_gap(
                binding,
                spans,
                operation_id,
                reason,
                &format!("{operation_pointer}/{key}"),
                counter,
                evidence,
                gaps,
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn add_contract_gap(
    binding: &ProviderBinding,
    spans: &BTreeMap<String, Span>,
    subject_id: &str,
    reason_code: &str,
    pointer: &str,
    counter: &mut ResourceCounter,
    evidence: &mut BTreeMap<String, FederationEvidence>,
    gaps: &mut Vec<CoverageGap>,
) -> Result<(), ContractError> {
    charge_contract(
        counter,
        FederationLimit::CoverageGaps,
        1,
        &binding.contract_path,
    )?;
    let source = evidence_for(binding, spans, pointer)?;
    let evidence_id = source.evidence_id.clone();
    insert_evidence(counter, evidence, source, &binding.contract_path)?;
    let location = format!("#{pointer}");
    gaps.push(CoverageGap {
        coverage_gap_id: contract_gap_id(subject_id, reason_code, &location),
        subject_id: subject_id.to_owned(),
        reason_code: reason_code.to_owned(),
        evidence_ids: vec![evidence_id],
    });
    Ok(())
}

fn evidence_for(
    binding: &ProviderBinding,
    spans: &BTreeMap<String, Span>,
    pointer: &str,
) -> Result<FederationEvidence, ContractError> {
    match binding.source_format {
        SourceFormat::Json => Ok(FederationEvidence::openapi_json(binding, pointer)),
        SourceFormat::Yaml => {
            let span = spans
                .get(pointer)
                .ok_or_else(|| ContractError::InvalidYaml {
                    path: binding.contract_path.clone(),
                })?;
            Ok(FederationEvidence::openapi_yaml(
                binding,
                &format!("#{pointer}"),
                span.start_line,
                span.end_line,
            ))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_fields(
    document: &Value,
    schema: &Value,
    base_pointer: &str,
    operation_id: &str,
    status: &str,
    evidence_id: &str,
    path: &str,
    counter: &mut ResourceCounter,
    fields: &mut Vec<OperationField>,
) -> Result<(), ContractError> {
    let (schema, _) = resolve_schema(document, schema, "", path, 0, &mut Vec::new())?;
    let object = schema
        .as_object()
        .ok_or_else(|| ContractError::InvalidOperation {
            path: path.to_owned(),
        })?;
    if object.get("type").and_then(Value::as_str) != Some("object") {
        return Ok(());
    }
    let required = object
        .get("required")
        .map(|value| {
            let values = value
                .as_array()
                .ok_or_else(|| ContractError::InvalidOperation {
                    path: path.to_owned(),
                })?;
            let required = values
                .iter()
                .map(|item| {
                    item.as_str().map(str::to_owned).ok_or_else(|| {
                        ContractError::InvalidOperation {
                            path: path.to_owned(),
                        }
                    })
                })
                .collect::<Result<BTreeSet<_>, _>>()?;
            if required.len() != values.len() {
                return Err(ContractError::InvalidOperation {
                    path: path.to_owned(),
                });
            }
            Ok(required)
        })
        .transpose()?
        .unwrap_or_default();
    let properties = object
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| ContractError::InvalidOperation {
            path: path.to_owned(),
        })?;
    for (name, property) in properties {
        let pointer = format!("{base_pointer}/{}", escape_pointer_segment(name));
        let (resolved, _) = resolve_schema(document, property, "", path, 0, &mut Vec::new())?;
        let schema_type = resolved
            .as_object()
            .and_then(|value| value.get("type"))
            .and_then(Value::as_str)
            .and_then(parse_schema_type)
            .ok_or_else(|| ContractError::InvalidOperation {
                path: path.to_owned(),
            })?;
        charge_contract(counter, FederationLimit::FieldsPerOperation, 1, path)?;
        fields.push(OperationField {
            field_id: field_id(operation_id, "response", status, &pointer),
            json_pointer: pointer.clone(),
            required: required.contains(name),
            schema_type,
            evidence_ids: vec![evidence_id.to_owned()],
        });
        if schema_type == JsonSchemaType::Object {
            collect_fields(
                document,
                resolved,
                &pointer,
                operation_id,
                status,
                evidence_id,
                path,
                counter,
                fields,
            )?;
        }
    }
    Ok(())
}

fn insert_evidence(
    counter: &mut ResourceCounter,
    evidence: &mut BTreeMap<String, FederationEvidence>,
    source: FederationEvidence,
    path: &str,
) -> Result<(), ContractError> {
    if !evidence.contains_key(&source.evidence_id) {
        charge_contract(counter, FederationLimit::EvidenceItems, 1, path)?;
    }
    evidence.insert(source.evidence_id.clone(), source);
    Ok(())
}

fn resolve_schema<'a>(
    document: &'a Value,
    schema: &'a Value,
    current_pointer: &str,
    path: &str,
    depth: u64,
    stack: &mut Vec<String>,
) -> Result<(&'a Value, String), ContractError> {
    let Some(reference) = schema
        .as_object()
        .and_then(|object| object.get("$ref"))
        .and_then(Value::as_str)
    else {
        return Ok((schema, current_pointer.to_owned()));
    };
    let pointer = local_schema_pointer(reference, path)?;
    if stack.contains(&pointer) {
        return Err(ContractError::ReferenceCycle {
            path: path.to_owned(),
        });
    }
    let observed = depth.saturating_add(1);
    if observed > FederationLimit::LocalRefDepth.maximum() {
        return Err(ContractError::LimitExceeded {
            path: path.to_owned(),
            error: LimitExceeded {
                limit: FederationLimit::LocalRefDepth,
                maximum: FederationLimit::LocalRefDepth.maximum(),
                observed,
            },
        });
    }
    let target = document
        .pointer(&pointer)
        .ok_or_else(|| ContractError::InvalidOperation {
            path: path.to_owned(),
        })?;
    stack.push(pointer.clone());
    let resolved = resolve_schema(document, target, &pointer, path, observed, stack);
    stack.pop();
    resolved
}

fn validate_all_references(document: &Value, path: &str) -> Result<(), ContractError> {
    fn visit(document: &Value, value: &Value, path: &str) -> Result<(), ContractError> {
        match value {
            Value::Object(object) => {
                if object.contains_key("$ref") {
                    let _ = resolve_schema(document, value, "", path, 0, &mut Vec::new())?;
                }
                for child in object.values() {
                    visit(document, child, path)?;
                }
            }
            Value::Array(values) => {
                for child in values {
                    visit(document, child, path)?;
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
        Ok(())
    }
    visit(document, document, path)
}

fn local_schema_pointer(reference: &str, path: &str) -> Result<String, ContractError> {
    let Some(pointer) = reference.strip_prefix('#') else {
        return Err(ContractError::RemoteReferenceForbidden {
            path: path.to_owned(),
        });
    };
    if !pointer.starts_with("/components/schemas/") || !valid_json_pointer(pointer) {
        return Err(ContractError::InvalidOperation {
            path: path.to_owned(),
        });
    }
    Ok(pointer.to_owned())
}

fn parse_restricted_yaml(source: &str, path: &str) -> Result<ParsedDocument, ContractError> {
    if source.lines().any(|line| {
        line.starts_with('\t')
            || line
                .trim_start()
                .strip_prefix('%')
                .is_some_and(|_| !line.trim_start().starts_with("%YAML 1.2"))
    }) {
        return Err(ContractError::UnsupportedYamlFeature {
            path: path.to_owned(),
        });
    }
    if source
        .lines()
        .any(|line| line.trim_start().starts_with('%'))
    {
        return Err(ContractError::UnsupportedYamlFeature {
            path: path.to_owned(),
        });
    }
    let mut receiver = EventCollector::default();
    Parser::new_from_str(source)
        .load(&mut receiver, true)
        .map_err(|_| ContractError::InvalidYaml {
            path: path.to_owned(),
        })?;
    validate_yaml_events(&receiver.events, path)?;
    let node_index = receiver
        .events
        .iter()
        .position(|record| matches!(record.event, Event::DocumentStart))
        .and_then(|index| index.checked_add(1))
        .ok_or_else(|| ContractError::InvalidYaml {
            path: path.to_owned(),
        })?;
    let mut spans = BTreeMap::new();
    let mut index = node_index;
    let value = parse_yaml_node(&receiver.events, &mut index, "", &mut spans, path)?;
    if !matches!(
        receiver.events.get(index).map(|record| &record.event),
        Some(Event::DocumentEnd)
    ) {
        return Err(ContractError::InvalidYaml {
            path: path.to_owned(),
        });
    }
    Ok(ParsedDocument { value, spans })
}

#[derive(Default)]
struct EventCollector {
    events: Vec<MarkedEvent>,
}

struct MarkedEvent {
    event: Event,
    marker: Marker,
}

impl MarkedEventReceiver for EventCollector {
    fn on_event(&mut self, event: Event, marker: Marker) {
        self.events.push(MarkedEvent { event, marker });
    }
}

fn validate_yaml_events(events: &[MarkedEvent], path: &str) -> Result<(), ContractError> {
    let documents = events
        .iter()
        .filter(|record| matches!(record.event, Event::DocumentStart))
        .count();
    if documents != 1 {
        return Err(ContractError::UnsupportedYamlFeature {
            path: path.to_owned(),
        });
    }
    let mut depth = 0_u64;
    for record in events {
        match &record.event {
            Event::Alias(_) => {
                return Err(ContractError::UnsupportedYamlFeature {
                    path: path.to_owned(),
                });
            }
            Event::Scalar(_, style, anchor, tag) => {
                if *anchor != 0
                    || tag.is_some()
                    || matches!(style, TScalarStyle::Literal | TScalarStyle::Folded)
                {
                    return Err(ContractError::UnsupportedYamlFeature {
                        path: path.to_owned(),
                    });
                }
            }
            Event::SequenceStart(anchor, tag) | Event::MappingStart(anchor, tag) => {
                if *anchor != 0 || tag.is_some() {
                    return Err(ContractError::UnsupportedYamlFeature {
                        path: path.to_owned(),
                    });
                }
                depth = depth.saturating_add(1);
                if depth > FederationLimit::YamlNestingDepth.maximum() {
                    return Err(ContractError::LimitExceeded {
                        path: path.to_owned(),
                        error: LimitExceeded {
                            limit: FederationLimit::YamlNestingDepth,
                            maximum: FederationLimit::YamlNestingDepth.maximum(),
                            observed: depth,
                        },
                    });
                }
            }
            Event::SequenceEnd | Event::MappingEnd => depth = depth.saturating_sub(1),
            Event::Nothing
            | Event::StreamStart
            | Event::StreamEnd
            | Event::DocumentStart
            | Event::DocumentEnd => {}
        }
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn parse_yaml_node(
    events: &[MarkedEvent],
    index: &mut usize,
    pointer: &str,
    spans: &mut BTreeMap<String, Span>,
    path: &str,
) -> Result<Value, ContractError> {
    let record = events
        .get(*index)
        .ok_or_else(|| ContractError::InvalidYaml {
            path: path.to_owned(),
        })?;
    match &record.event {
        Event::Scalar(value, style, _, _) => {
            *index += 1;
            spans.insert(
                pointer.to_owned(),
                Span {
                    start_line: u64::try_from(record.marker.line()).unwrap_or(u64::MAX),
                    end_line: u64::try_from(record.marker.line()).unwrap_or(u64::MAX),
                },
            );
            scalar_value(value, *style, path)
        }
        Event::MappingStart(_, _) => {
            let marker = record.marker;
            let mut end_line = marker.line();
            *index += 1;
            let mut object = Map::new();
            while !matches!(
                events.get(*index).map(|item| &item.event),
                Some(Event::MappingEnd)
            ) {
                let key_record = events
                    .get(*index)
                    .ok_or_else(|| ContractError::InvalidYaml {
                        path: path.to_owned(),
                    })?;
                let Event::Scalar(key, _, _, _) = &key_record.event else {
                    return Err(ContractError::UnsupportedYamlFeature {
                        path: path.to_owned(),
                    });
                };
                if key == "<<" {
                    return Err(ContractError::UnsupportedYamlFeature {
                        path: path.to_owned(),
                    });
                }
                if object.contains_key(key) {
                    return Err(ContractError::DuplicateKey {
                        path: path.to_owned(),
                    });
                }
                let key = key.clone();
                let key_marker = key_record.marker;
                *index += 1;
                let child_pointer = format!("{pointer}/{}", escape_pointer_segment(&key));
                let child = parse_yaml_node(events, index, &child_pointer, spans, path)?;
                let child_end_line = spans.get(&child_pointer).map_or(key_marker.line(), |span| {
                    usize::try_from(span.end_line).unwrap_or(usize::MAX)
                });
                end_line = end_line.max(child_end_line);
                spans.insert(
                    child_pointer,
                    Span {
                        start_line: u64::try_from(key_marker.line()).unwrap_or(u64::MAX),
                        end_line: u64::try_from(child_end_line).unwrap_or(u64::MAX),
                    },
                );
                object.insert(key, child);
            }
            *index += 1;
            spans.insert(
                pointer.to_owned(),
                Span {
                    start_line: u64::try_from(marker.line()).unwrap_or(u64::MAX),
                    end_line: u64::try_from(end_line).unwrap_or(u64::MAX),
                },
            );
            Ok(Value::Object(object))
        }
        Event::SequenceStart(_, _) => {
            let marker = record.marker;
            let mut end_line = marker.line();
            *index += 1;
            let mut values = Vec::new();
            while !matches!(
                events.get(*index).map(|item| &item.event),
                Some(Event::SequenceEnd)
            ) {
                let child_pointer = format!("{pointer}/{}", values.len());
                values.push(parse_yaml_node(events, index, &child_pointer, spans, path)?);
                end_line = end_line.max(spans.get(&child_pointer).map_or(marker.line(), |span| {
                    usize::try_from(span.end_line).unwrap_or(usize::MAX)
                }));
            }
            *index += 1;
            spans.insert(
                pointer.to_owned(),
                Span {
                    start_line: u64::try_from(marker.line()).unwrap_or(u64::MAX),
                    end_line: u64::try_from(end_line).unwrap_or(u64::MAX),
                },
            );
            Ok(Value::Array(values))
        }
        Event::Alias(_)
        | Event::Nothing
        | Event::StreamStart
        | Event::StreamEnd
        | Event::DocumentStart
        | Event::DocumentEnd
        | Event::SequenceEnd
        | Event::MappingEnd => Err(ContractError::InvalidYaml {
            path: path.to_owned(),
        }),
    }
}

fn scalar_value(value: &str, style: TScalarStyle, path: &str) -> Result<Value, ContractError> {
    if value.contains(['\r', '\n']) {
        return Err(ContractError::UnsupportedYamlFeature {
            path: path.to_owned(),
        });
    }
    if style != TScalarStyle::Plain {
        return Ok(Value::String(value.to_owned()));
    }
    match value {
        "" | "~" | "null" | "Null" | "NULL" => Ok(Value::Null),
        "true" | "True" | "TRUE" => Ok(Value::Bool(true)),
        "false" | "False" | "FALSE" => Ok(Value::Bool(false)),
        _ => {
            if let Ok(integer) = value.parse::<i64>() {
                return Ok(Value::Number(Number::from(integer)));
            }
            if let Ok(unsigned) = value.parse::<u64>() {
                return Ok(Value::Number(Number::from(unsigned)));
            }
            if looks_numeric(value) {
                let number = value
                    .parse::<f64>()
                    .ok()
                    .and_then(Number::from_f64)
                    .ok_or_else(|| ContractError::UnsupportedYamlFeature {
                        path: path.to_owned(),
                    })?;
                return Ok(Value::Number(number));
            }
            Ok(Value::String(value.to_owned()))
        }
    }
}

fn looks_numeric(value: &str) -> bool {
    value.contains(['.', 'e', 'E'])
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'+' | b'-' | b'.' | b'e' | b'E'))
        && value.parse::<f64>().is_ok()
}

fn charge_contract(
    counter: &mut ResourceCounter,
    limit: FederationLimit,
    amount: u64,
    path: &str,
) -> Result<(), ContractError> {
    counter
        .charge(limit, amount)
        .map(|_| ())
        .map_err(|error| ContractError::LimitExceeded {
            path: path.to_owned(),
            error,
        })
}

fn required_string(
    object: &Map<String, Value>,
    key: &str,
    maximum: usize,
    path: &str,
) -> Result<String, ContractError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= maximum)
        .map(str::to_owned)
        .ok_or_else(|| ContractError::InvalidOperation {
            path: path.to_owned(),
        })
}

fn supported_methods() -> [(&'static str, HttpMethod); 5] {
    [
        ("delete", HttpMethod::Delete),
        ("get", HttpMethod::Get),
        ("patch", HttpMethod::Patch),
        ("post", HttpMethod::Post),
        ("put", HttpMethod::Put),
    ]
}

fn parse_schema_type(value: &str) -> Option<JsonSchemaType> {
    match value {
        "array" => Some(JsonSchemaType::Array),
        "boolean" => Some(JsonSchemaType::Boolean),
        "integer" => Some(JsonSchemaType::Integer),
        "number" => Some(JsonSchemaType::Number),
        "object" => Some(JsonSchemaType::Object),
        "string" => Some(JsonSchemaType::String),
        _ => None,
    }
}

fn valid_service_authority(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("https://") else {
        return false;
    };
    !rest.is_empty()
        && value.len() <= 2048
        && !rest.contains(['@', '?', '#', '{', '}'])
        && !rest.bytes().any(|byte| byte.is_ascii_uppercase())
        && rest.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'.' | b'_'
                        | b'~'
                        | b'!'
                        | b'$'
                        | b'&'
                        | b'\''
                        | b'('
                        | b')'
                        | b'*'
                        | b'+'
                        | b','
                        | b';'
                        | b'='
                        | b':'
                        | b'@'
                        | b'%'
                        | b'/'
                        | b'-'
                )
        })
}

fn valid_path_template(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 2048
        && value.starts_with('/')
        && (value == "/" || !value[1..].split('/').any(str::is_empty))
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'.' | b'_'
                        | b'~'
                        | b'!'
                        | b'$'
                        | b'&'
                        | b'\''
                        | b'('
                        | b')'
                        | b'*'
                        | b'+'
                        | b','
                        | b';'
                        | b'='
                        | b':'
                        | b'@'
                        | b'%'
                        | b'{'
                        | b'}'
                        | b'/'
                        | b'-'
                )
        })
}

fn valid_operation_name(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes.next().is_some_and(|byte| byte.is_ascii_alphabetic())
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn valid_response_status(value: &str) -> bool {
    value.len() == 3
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_digit() && (index != 0 || (b'1'..=b'5').contains(&byte))
        })
}

fn valid_json_pointer(pointer: &str) -> bool {
    if !pointer.starts_with('/') || pointer.contains(['\0', '\r', '\n']) {
        return false;
    }
    let bytes = pointer.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'~' {
            if bytes
                .get(index + 1)
                .is_none_or(|byte| !matches!(byte, b'0' | b'1'))
            {
                return false;
            }
            index += 1;
        }
        index += 1;
    }
    true
}

fn escape_pointer_segment(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}
