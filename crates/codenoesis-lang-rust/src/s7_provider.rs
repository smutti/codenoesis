use std::collections::BTreeMap;

use codenoesis_domain::s7::{
    ProviderFieldExtraction, ProviderPresence, ProviderSourceExtraction, S7Limit,
    SourceExtractionError, SourceSpan,
};
use codenoesis_ports::RustProviderSourceExtractor;
use tree_sitter::{Node, Parser};

pub const PROVIDER_CAPABILITY: &str = "rust-direct-json-map/v1";

#[derive(Clone, Copy, Debug, Default)]
pub struct TreeSitterRustProviderExtractor;

impl TreeSitterRustProviderExtractor {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl RustProviderSourceExtractor for TreeSitterRustProviderExtractor {
    fn extract_s7_provider(
        &self,
        source: &[u8],
        callable_symbol: &str,
    ) -> Result<ProviderSourceExtraction, SourceExtractionError> {
        extract_provider(source, callable_symbol)
    }
}

fn extract_provider(
    source_bytes: &[u8],
    callable_symbol: &str,
) -> Result<ProviderSourceExtraction, SourceExtractionError> {
    let source =
        std::str::from_utf8(source_bytes).map_err(|_| SourceExtractionError::InvalidUtf8)?;
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .map_err(|_| SourceExtractionError::UnsupportedSemantics)?;
    let tree = parser
        .parse(source, None)
        .ok_or(SourceExtractionError::InvalidSyntax)?;
    let root = tree.root_node();
    if root.has_error() {
        return Err(SourceExtractionError::InvalidSyntax);
    }
    enforce_tree_limits(root, source)?;
    let callable = selected_callable(root, source, callable_symbol)?;
    reject_unsupported_control(callable)?;
    let body = callable
        .child_by_field_name("body")
        .ok_or(SourceExtractionError::UnsupportedSemantics)?;
    let selected_map = selected_map(body, source)?;
    require_closed_body(body, source, &selected_map)?;
    let map_name = selected_map.name;

    let mut fields = BTreeMap::new();
    let mut custom_mapping_spans = Vec::new();
    visit_named(body, &mut |node| {
        if node.kind() != "call_expression" {
            return Ok(());
        }
        let Some(function) = node.child_by_field_name("function") else {
            return Ok(());
        };
        let function_text = compact(node_text(function, source)?);
        if function_text == format!("{map_name}.insert") {
            let arguments = node
                .child_by_field_name("arguments")
                .ok_or(SourceExtractionError::UnsupportedSemantics)?;
            let mut cursor = arguments.walk();
            let arguments = arguments.named_children(&mut cursor).collect::<Vec<_>>();
            if arguments.len() != 2 {
                return Err(SourceExtractionError::UnsupportedSemantics);
            }
            let field_name = direct_literal_argument(arguments[0], source)?;
            let enclosing_if = ancestor_before(node, body, "if_expression");
            let (presence, evidence_node) = if let Some(if_expression) = enclosing_if {
                if if_expression.child_by_field_name("alternative").is_some() {
                    return Err(SourceExtractionError::UnsupportedSemantics);
                }
                let consequence = if_expression
                    .child_by_field_name("consequence")
                    .ok_or(SourceExtractionError::UnsupportedSemantics)?;
                if !is_descendant_of(node, consequence)
                    || if_expression
                        .child_by_field_name("condition")
                        .is_some_and(contains_effectful_condition)
                {
                    return Err(SourceExtractionError::UnsupportedSemantics);
                }
                (ProviderPresence::MayBeAbsent, if_expression)
            } else {
                (
                    ProviderPresence::GuaranteedPresent,
                    ancestor_before(node, body, "expression_statement").unwrap_or(node),
                )
            };
            let extraction = ProviderFieldExtraction {
                field_name: field_name.clone(),
                presence,
                span: source_span(evidence_node),
            };
            if fields.insert(field_name, extraction).is_some() {
                return Err(SourceExtractionError::UnsupportedSemantics);
            }
        } else if function_text == format!("{map_name}.extend") {
            let statement = ancestor_before(node, body, "expression_statement").unwrap_or(node);
            custom_mapping_spans.push(source_span(statement));
        } else if function_text.starts_with(&format!("{map_name}.")) {
            return Err(SourceExtractionError::UnsupportedSemantics);
        }
        Ok(())
    })?;

    if fields.is_empty() {
        return Err(SourceExtractionError::UnsupportedSemantics);
    }
    custom_mapping_spans.sort();
    custom_mapping_spans.dedup();
    Ok(ProviderSourceExtraction {
        fields: fields.into_values().collect(),
        custom_mapping_spans,
    })
}

fn selected_callable<'tree>(
    root: Node<'tree>,
    source: &str,
    callable_symbol: &str,
) -> Result<Node<'tree>, SourceExtractionError> {
    let mut matches = Vec::new();
    visit_named(root, &mut |node| {
        if node.kind() == "function_item"
            && node
                .child_by_field_name("name")
                .is_some_and(|name| node_text(name, source).ok() == Some(callable_symbol))
        {
            matches.push(node);
        }
        Ok(())
    })?;
    match matches.as_slice() {
        [] => Err(SourceExtractionError::CallableMissing),
        [callable] => Ok(*callable),
        _ => Err(SourceExtractionError::CallableAmbiguous),
    }
}

struct SelectedMap<'tree> {
    name: String,
    declaration: Node<'tree>,
}

fn selected_map<'tree>(
    body: Node<'tree>,
    source: &str,
) -> Result<SelectedMap<'tree>, SourceExtractionError> {
    let mut maps = Vec::new();
    let mut cursor = body.walk();
    for node in body.named_children(&mut cursor) {
        if node.kind() != "let_declaration" {
            continue;
        }
        let Some(pattern) = node.child_by_field_name("pattern") else {
            continue;
        };
        let Some(value) = node.child_by_field_name("value") else {
            continue;
        };
        let value = compact(node_text(value, source)?);
        if matches!(value.as_str(), "Map::new()" | "serde_json::Map::new()") {
            let pattern = node_text(pattern, source)?.trim();
            let name = pattern.strip_prefix("mut ").unwrap_or(pattern);
            if !valid_identifier(name) {
                return Err(SourceExtractionError::UnsupportedSemantics);
            }
            maps.push(SelectedMap {
                name: name.to_owned(),
                declaration: node,
            });
        }
    }
    match maps.pop() {
        Some(map) if maps.is_empty() => Ok(map),
        _ => Err(SourceExtractionError::UnsupportedSemantics),
    }
}

fn require_closed_body(
    body: Node<'_>,
    source: &str,
    selected_map: &SelectedMap<'_>,
) -> Result<(), SourceExtractionError> {
    let expected = [
        format!("Value::Object({})", selected_map.name),
        format!("serde_json::Value::Object({})", selected_map.name),
    ];
    let mut cursor = body.walk();
    let children = body.named_children(&mut cursor).collect::<Vec<_>>();
    let publication = children
        .last()
        .filter(|node| node.kind() == "call_expression")
        .ok_or(SourceExtractionError::UnsupportedSemantics)?;
    if !expected.contains(&compact(node_text(*publication, source)?)) {
        return Err(SourceExtractionError::UnsupportedSemantics);
    }
    for child in &children[..children.len() - 1] {
        if *child == selected_map.declaration {
            continue;
        }
        let expression = statement_expression(*child)?;
        if expression.kind() == "if_expression" {
            validate_direct_if(expression, source, &selected_map.name)?;
        } else if !is_direct_map_call(expression, source, &selected_map.name, "insert")?
            && !is_direct_map_call(expression, source, &selected_map.name, "extend")?
        {
            return Err(SourceExtractionError::UnsupportedSemantics);
        }
    }
    Ok(())
}

fn validate_direct_if(
    expression: Node<'_>,
    source: &str,
    map_name: &str,
) -> Result<(), SourceExtractionError> {
    if expression.child_by_field_name("alternative").is_some()
        || expression
            .child_by_field_name("condition")
            .is_some_and(contains_effectful_condition)
    {
        return Err(SourceExtractionError::UnsupportedSemantics);
    }
    let consequence = expression
        .child_by_field_name("consequence")
        .ok_or(SourceExtractionError::UnsupportedSemantics)?;
    let mut cursor = consequence.walk();
    let statements = consequence.named_children(&mut cursor).collect::<Vec<_>>();
    if statements.is_empty() {
        return Err(SourceExtractionError::UnsupportedSemantics);
    }
    for statement in statements {
        let call = statement_expression(statement)?;
        if !is_direct_map_call(call, source, map_name, "insert")? {
            return Err(SourceExtractionError::UnsupportedSemantics);
        }
    }
    Ok(())
}

fn statement_expression(statement: Node<'_>) -> Result<Node<'_>, SourceExtractionError> {
    if statement.kind() != "expression_statement" {
        return Ok(statement);
    }
    let mut cursor = statement.walk();
    let children = statement.named_children(&mut cursor).collect::<Vec<_>>();
    match children.as_slice() {
        [expression] => Ok(*expression),
        _ => Err(SourceExtractionError::UnsupportedSemantics),
    }
}

fn is_direct_map_call(
    expression: Node<'_>,
    source: &str,
    map_name: &str,
    method: &str,
) -> Result<bool, SourceExtractionError> {
    if expression.kind() != "call_expression" {
        return Ok(false);
    }
    let function = expression
        .child_by_field_name("function")
        .ok_or(SourceExtractionError::UnsupportedSemantics)?;
    Ok(compact(node_text(function, source)?) == format!("{map_name}.{method}"))
}

fn reject_unsupported_control(callable: Node<'_>) -> Result<(), SourceExtractionError> {
    let mut unsupported = false;
    visit_named(callable, &mut |node| {
        if matches!(
            node.kind(),
            "for_expression"
                | "loop_expression"
                | "macro_invocation"
                | "match_expression"
                | "assignment_expression"
                | "async_block"
                | "break_expression"
                | "closure_expression"
                | "continue_expression"
                | "return_expression"
                | "try_expression"
                | "unsafe_block"
                | "while_expression"
                | "function_item"
        ) {
            unsupported |= node != callable;
        }
        Ok(())
    })?;
    if unsupported {
        return Err(SourceExtractionError::UnsupportedSemantics);
    }
    Ok(())
}

fn direct_literal_argument(node: Node<'_>, source: &str) -> Result<String, SourceExtractionError> {
    let mut literals = Vec::new();
    visit_named(node, &mut |candidate| {
        if candidate.kind() == "string_literal" {
            literals.push(node_text(candidate, source)?.to_owned());
        }
        Ok(())
    })?;
    let [literal] = literals.as_slice() else {
        return Err(SourceExtractionError::UnsupportedSemantics);
    };
    let expression = compact(node_text(node, source)?);
    let literal_expression = compact(literal);
    if expression != literal_expression && expression != format!("{literal_expression}.into()") {
        return Err(SourceExtractionError::UnsupportedSemantics);
    }
    let value = literal
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .filter(|value| {
            !value.is_empty()
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        })
        .ok_or(SourceExtractionError::UnsupportedSemantics)?;
    Ok(value.to_owned())
}

fn enforce_tree_limits(root: Node<'_>, source: &str) -> Result<(), SourceExtractionError> {
    let mut stack = vec![(root, 1_u64)];
    let mut nodes = 0_u64;
    while let Some((node, depth)) = stack.pop() {
        nodes = nodes.saturating_add(1);
        enforce_limit(S7Limit::SyntaxNodesPerSource, nodes)?;
        enforce_limit(S7Limit::SourceNestingDepth, depth)?;
        if matches!(node.kind(), "string_literal" | "raw_string_literal") {
            let bytes = node_text(node, source)?.len();
            enforce_limit(S7Limit::StringLiteralBytes, bytes as u64)?;
        }
        let mut cursor = node.walk();
        stack.extend(
            node.named_children(&mut cursor)
                .map(|child| (child, depth.saturating_add(1))),
        );
    }
    Ok(())
}

fn enforce_limit(limit: S7Limit, observed: u64) -> Result<(), SourceExtractionError> {
    limit
        .check(observed)
        .map_err(SourceExtractionError::LimitExceeded)
}

fn visit_named<'tree>(
    root: Node<'tree>,
    visitor: &mut impl FnMut(Node<'tree>) -> Result<(), SourceExtractionError>,
) -> Result<(), SourceExtractionError> {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        visitor(node)?;
        let mut cursor = node.walk();
        stack.extend(node.named_children(&mut cursor));
    }
    Ok(())
}

fn ancestor_before<'tree>(
    mut node: Node<'tree>,
    boundary: Node<'tree>,
    kind: &str,
) -> Option<Node<'tree>> {
    while let Some(parent) = node.parent() {
        if parent == boundary {
            return None;
        }
        if parent.kind() == kind {
            return Some(parent);
        }
        node = parent;
    }
    None
}

fn is_descendant_of(mut node: Node<'_>, ancestor: Node<'_>) -> bool {
    while let Some(parent) = node.parent() {
        if parent == ancestor {
            return true;
        }
        node = parent;
    }
    false
}

fn contains_effectful_condition(root: Node<'_>) -> bool {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if matches!(
            node.kind(),
            "assignment_expression" | "await_expression" | "call_expression" | "macro_invocation"
        ) {
            return true;
        }
        let mut cursor = node.walk();
        stack.extend(node.named_children(&mut cursor));
    }
    false
}

fn node_text<'a>(node: Node<'_>, source: &'a str) -> Result<&'a str, SourceExtractionError> {
    source
        .get(node.byte_range())
        .ok_or(SourceExtractionError::InvalidSyntax)
}

fn source_span(node: Node<'_>) -> SourceSpan {
    SourceSpan {
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        start_line: node.start_position().row as u64 + 1,
        end_line: node.end_position().row as u64 + 1,
    }
}

fn compact(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn valid_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte == b'_' || byte.is_ascii_alphabetic())
        && bytes.all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
}
