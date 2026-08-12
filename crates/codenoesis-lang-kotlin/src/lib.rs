use codenoesis_domain::s7::{
    ClientFieldExtraction, ClientPresenceAssumption, ClientSourceExtraction, S7Limit,
    SourceExtractionError, SourceSpan,
};
use codenoesis_ports::KotlinClientSourceExtractor;
use tree_sitter::{Node, Parser};

pub const CLIENT_CAPABILITY: &str = "kotlin-direct-json-access/v1";

#[derive(Clone, Copy, Debug, Default)]
pub struct TreeSitterKotlinClientExtractor;

impl TreeSitterKotlinClientExtractor {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl KotlinClientSourceExtractor for TreeSitterKotlinClientExtractor {
    fn extract_s7_client(
        &self,
        source: &[u8],
        decoder_symbol: &str,
        call_symbol: &str,
    ) -> Result<ClientSourceExtraction, SourceExtractionError> {
        extract_client(source, decoder_symbol, call_symbol)
    }
}

fn extract_client(
    source_bytes: &[u8],
    decoder_symbol: &str,
    call_symbol: &str,
) -> Result<ClientSourceExtraction, SourceExtractionError> {
    let source =
        std::str::from_utf8(source_bytes).map_err(|_| SourceExtractionError::InvalidUtf8)?;
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_kotlin_ng::LANGUAGE.into())
        .map_err(|_| SourceExtractionError::UnsupportedSemantics)?;
    let tree = parser
        .parse(source, None)
        .ok_or(SourceExtractionError::InvalidSyntax)?;
    let root = tree.root_node();
    if root.has_error() {
        return Err(SourceExtractionError::InvalidSyntax);
    }
    enforce_tree_limits(root, source)?;
    let decoder = selected_function(root, source, decoder_symbol)?;
    let call = selected_function(root, source, call_symbol)?;
    reject_unsupported_decoder_control(decoder)?;
    reject_unsupported_call_shape(call)?;
    let payload = first_parameter_name(decoder, source)?;
    let assumptions = extract_assumptions(decoder, source, &payload)?;
    let path_template = extract_call_path(call, source, decoder_symbol)?;
    let data_class = decoder_result_class(root, decoder, source)?;
    if data_class.start_byte() >= decoder.start_byte() || decoder.start_byte() >= call.start_byte()
    {
        return Err(SourceExtractionError::UnsupportedSemantics);
    }
    Ok(ClientSourceExtraction {
        path_template,
        assumptions,
        evidence_span: SourceSpan {
            start_byte: data_class.start_byte(),
            end_byte: call.end_byte(),
            start_line: data_class.start_position().row as u64 + 1,
            end_line: call.end_position().row as u64 + 1,
        },
    })
}

fn selected_function<'tree>(
    root: Node<'tree>,
    source: &str,
    symbol: &str,
) -> Result<Node<'tree>, SourceExtractionError> {
    let mut matches = Vec::new();
    visit_named(root, &mut |node| {
        if node.kind() == "function_declaration"
            && node
                .child_by_field_name("name")
                .is_some_and(|name| node_text(name, source).ok() == Some(symbol))
        {
            matches.push(node);
        }
        Ok(())
    })?;
    match matches.as_slice() {
        [] => Err(SourceExtractionError::CallableMissing),
        [function] => Ok(*function),
        _ => Err(SourceExtractionError::CallableAmbiguous),
    }
}

fn first_parameter_name(function: Node<'_>, source: &str) -> Result<String, SourceExtractionError> {
    let parameters = direct_named_child(function, "function_value_parameters")
        .ok_or(SourceExtractionError::UnsupportedSemantics)?;
    let mut cursor = parameters.walk();
    let parameters = parameters
        .named_children(&mut cursor)
        .filter(|node| node.kind() == "parameter")
        .collect::<Vec<_>>();
    let [parameter] = parameters.as_slice() else {
        return Err(SourceExtractionError::UnsupportedSemantics);
    };
    let mut cursor = parameter.walk();
    parameter
        .named_children(&mut cursor)
        .find(|node| node.kind() == "identifier")
        .map(|node| node_text(node, source).map(str::to_owned))
        .transpose()?
        .ok_or(SourceExtractionError::UnsupportedSemantics)
}

fn extract_assumptions(
    decoder: Node<'_>,
    source: &str,
    payload: &str,
) -> Result<Vec<ClientFieldExtraction>, SourceExtractionError> {
    let mut assumptions = std::collections::BTreeMap::new();
    visit_named(decoder, &mut |node| {
        if node.kind() == "call_expression" {
            let mut cursor = node.walk();
            let children = node.named_children(&mut cursor).collect::<Vec<_>>();
            let Some(function) = children.first() else {
                return Ok(());
            };
            if function.kind() == "navigation_expression"
                && compact(node_text(*function, source)?) == format!("{payload}.getValue")
            {
                let field = single_literal_argument(node, source)?;
                insert_assumption(
                    &mut assumptions,
                    field,
                    ClientPresenceAssumption::RequiresPresent,
                )?;
            }
        } else if node.kind() == "index_expression" {
            let mut cursor = node.walk();
            let children = node.named_children(&mut cursor).collect::<Vec<_>>();
            if children.len() == 2 && node_text(children[0], source)? == payload {
                let field = simple_kotlin_literal(children[1], source)?;
                let expression = enclosing_property_expression(node, decoder);
                let expression = compact(node_text(expression, source)?);
                if !is_direct_safe_navigation(&expression, payload, &field) {
                    return Err(SourceExtractionError::UnsupportedSemantics);
                }
                insert_assumption(
                    &mut assumptions,
                    field,
                    ClientPresenceAssumption::HandlesAbsent,
                )?;
            }
        }
        Ok(())
    })?;
    if assumptions.is_empty() {
        return Err(SourceExtractionError::UnsupportedSemantics);
    }
    Ok(assumptions.into_values().collect())
}

fn insert_assumption(
    assumptions: &mut std::collections::BTreeMap<String, ClientFieldExtraction>,
    field_name: String,
    assumption: ClientPresenceAssumption,
) -> Result<(), SourceExtractionError> {
    if assumptions
        .insert(
            field_name.clone(),
            ClientFieldExtraction {
                field_name,
                assumption,
            },
        )
        .is_some()
    {
        return Err(SourceExtractionError::UnsupportedSemantics);
    }
    Ok(())
}

fn extract_call_path(
    call: Node<'_>,
    source: &str,
    decoder_symbol: &str,
) -> Result<String, SourceExtractionError> {
    let mut decoder_calls = Vec::new();
    let mut http_calls = Vec::new();
    visit_named(call, &mut |node| {
        if node.kind() != "call_expression" {
            return Ok(());
        }
        let mut cursor = node.walk();
        let children = node.named_children(&mut cursor).collect::<Vec<_>>();
        let Some(function) = children.first() else {
            return Ok(());
        };
        let function = node_text(*function, source)?;
        if function == decoder_symbol {
            decoder_calls.push(node);
        } else if function == "httpGet" {
            http_calls.push(node);
        }
        Ok(())
    })?;
    let [decoder_call] = decoder_calls.as_slice() else {
        return Err(SourceExtractionError::UnsupportedSemantics);
    };
    let [http_call] = http_calls.as_slice() else {
        return Err(SourceExtractionError::UnsupportedSemantics);
    };
    if !is_descendant_of(*http_call, *decoder_call) {
        return Err(SourceExtractionError::UnsupportedSemantics);
    }
    if single_argument_expression(*decoder_call)? != *http_call {
        return Err(SourceExtractionError::UnsupportedSemantics);
    }
    let body = direct_named_child(call, "function_body")
        .ok_or(SourceExtractionError::UnsupportedSemantics)?;
    let mut cursor = body.walk();
    let direct = body.named_children(&mut cursor).collect::<Vec<_>>();
    if direct.as_slice() != [*decoder_call] {
        return Err(SourceExtractionError::UnsupportedSemantics);
    }
    let literal = single_literal_node(*http_call)?;
    canonical_http_path(node_text(literal, source)?)
}

fn decoder_result_class<'tree>(
    root: Node<'tree>,
    decoder: Node<'tree>,
    source: &str,
) -> Result<Node<'tree>, SourceExtractionError> {
    let parameters = direct_named_child(decoder, "function_value_parameters")
        .ok_or(SourceExtractionError::UnsupportedSemantics)?;
    let mut cursor = decoder.walk();
    let return_type = decoder
        .named_children(&mut cursor)
        .find(|child| {
            child.start_byte() > parameters.end_byte()
                && matches!(child.kind(), "user_type" | "nullable_type")
        })
        .ok_or(SourceExtractionError::UnsupportedSemantics)?;
    let result_name = compact(node_text(return_type, source)?);
    let mut matches = Vec::new();
    visit_named(root, &mut |node| {
        if node.kind() == "class_declaration"
            && node
                .child_by_field_name("name")
                .is_some_and(|name| node_text(name, source).ok() == Some(result_name.as_str()))
        {
            matches.push(node);
        }
        Ok(())
    })?;
    match matches.as_slice() {
        [class] => Ok(*class),
        _ => Err(SourceExtractionError::UnsupportedSemantics),
    }
}

fn reject_unsupported_decoder_control(decoder: Node<'_>) -> Result<(), SourceExtractionError> {
    let mut unsupported = false;
    visit_named(decoder, &mut |node| {
        if matches!(
            node.kind(),
            "for_statement"
                | "if_expression"
                | "lambda_literal"
                | "object_literal"
                | "try_expression"
                | "when_expression"
                | "while_statement"
                | "function_declaration"
        ) {
            unsupported |= node != decoder;
        }
        Ok(())
    })?;
    if unsupported {
        return Err(SourceExtractionError::UnsupportedSemantics);
    }
    Ok(())
}

fn reject_unsupported_call_shape(call: Node<'_>) -> Result<(), SourceExtractionError> {
    if direct_named_child(call, "function_body").is_none() {
        return Err(SourceExtractionError::UnsupportedSemantics);
    }
    let mut unsupported = false;
    visit_named(call, &mut |node| {
        if matches!(
            node.kind(),
            "block"
                | "for_statement"
                | "if_expression"
                | "lambda_literal"
                | "try_expression"
                | "when_expression"
                | "while_statement"
                | "function_declaration"
        ) {
            unsupported |= node != call;
        }
        Ok(())
    })?;
    if unsupported {
        return Err(SourceExtractionError::UnsupportedSemantics);
    }
    Ok(())
}

fn single_literal_argument(call: Node<'_>, source: &str) -> Result<String, SourceExtractionError> {
    let literal = single_literal_node(call)?;
    simple_kotlin_literal(literal, source)
}

fn single_literal_node(call: Node<'_>) -> Result<Node<'_>, SourceExtractionError> {
    let expression = single_argument_expression(call)?;
    if expression.kind() == "string_literal" {
        Ok(expression)
    } else {
        Err(SourceExtractionError::UnsupportedSemantics)
    }
}

fn single_argument_expression(call: Node<'_>) -> Result<Node<'_>, SourceExtractionError> {
    let arguments = direct_named_child(call, "value_arguments")
        .ok_or(SourceExtractionError::UnsupportedSemantics)?;
    let mut argument_cursor = arguments.walk();
    let arguments = arguments
        .named_children(&mut argument_cursor)
        .collect::<Vec<_>>();
    let [argument] = arguments.as_slice() else {
        return Err(SourceExtractionError::UnsupportedSemantics);
    };
    unwrap_expression(*argument)
}

fn unwrap_expression(mut node: Node<'_>) -> Result<Node<'_>, SourceExtractionError> {
    while matches!(
        node.kind(),
        "expression" | "parenthesized_expression" | "value_argument"
    ) {
        let mut cursor = node.walk();
        let children = node.named_children(&mut cursor).collect::<Vec<_>>();
        match children.as_slice() {
            [child] => node = *child,
            _ => return Err(SourceExtractionError::UnsupportedSemantics),
        }
    }
    Ok(node)
}

fn simple_kotlin_literal(node: Node<'_>, source: &str) -> Result<String, SourceExtractionError> {
    let literal = node_text(node, source)?;
    literal
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .filter(|value| {
            !value.is_empty()
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        })
        .map(str::to_owned)
        .ok_or(SourceExtractionError::UnsupportedSemantics)
}

fn canonical_http_path(literal: &str) -> Result<String, SourceExtractionError> {
    let value = literal
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .filter(|value| value.starts_with('/') && !value.contains(['\\', '{', '}', '\n', '\r']))
        .ok_or(SourceExtractionError::UnsupportedSemantics)?;
    let bytes = value.as_bytes();
    let mut index = 0;
    let mut path = String::with_capacity(value.len() + 2);
    while index < bytes.len() {
        if bytes[index] == b'$' {
            index += 1;
            let start = index;
            while index < bytes.len()
                && (bytes[index] == b'_' || bytes[index].is_ascii_alphanumeric())
            {
                index += 1;
            }
            if start == index || !bytes[start].is_ascii_alphabetic() && bytes[start] != b'_' {
                return Err(SourceExtractionError::UnsupportedSemantics);
            }
            path.push('{');
            path.push_str(&value[start..index]);
            path.push('}');
        } else if bytes[index].is_ascii_alphanumeric()
            || matches!(bytes[index], b'/' | b'-' | b'_' | b'.')
        {
            path.push(char::from(bytes[index]));
            index += 1;
        } else {
            return Err(SourceExtractionError::UnsupportedSemantics);
        }
    }
    if path.split('/').skip(1).any(str::is_empty) {
        return Err(SourceExtractionError::UnsupportedSemantics);
    }
    Ok(path)
}

fn is_direct_safe_navigation(expression: &str, payload: &str, field: &str) -> bool {
    let prefix = format!("{payload}[\"{field}\"]?.");
    expression.strip_prefix(&prefix).is_some_and(|navigation| {
        !navigation.is_empty() && navigation.split("?.").all(valid_identifier)
    })
}

fn direct_named_child<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == kind)
}

fn enclosing_property_expression<'tree>(
    mut node: Node<'tree>,
    boundary: Node<'tree>,
) -> Node<'tree> {
    while let Some(parent) = node.parent() {
        if parent == boundary || parent.kind() == "property_declaration" {
            return node;
        }
        node = parent;
    }
    node
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

fn node_text<'a>(node: Node<'_>, source: &'a str) -> Result<&'a str, SourceExtractionError> {
    source
        .get(node.byte_range())
        .ok_or(SourceExtractionError::InvalidSyntax)
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

fn enforce_tree_limits(root: Node<'_>, source: &str) -> Result<(), SourceExtractionError> {
    let mut stack = vec![(root, 1_u64)];
    let mut nodes = 0_u64;
    while let Some((node, depth)) = stack.pop() {
        nodes = nodes.saturating_add(1);
        enforce_limit(S7Limit::SyntaxNodesPerSource, nodes)?;
        enforce_limit(S7Limit::SourceNestingDepth, depth)?;
        if node.kind().contains("string") {
            let observed = node
                .utf8_text(source.as_bytes())
                .map_err(|_| SourceExtractionError::InvalidSyntax)?
                .len() as u64;
            enforce_limit(S7Limit::StringLiteralBytes, observed)?;
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
