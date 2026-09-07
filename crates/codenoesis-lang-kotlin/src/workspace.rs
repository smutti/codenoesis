//! Bounded declaration extraction; target Gradle scripts are never executed.
use codenoesis_domain::RepositoryInventory;
use codenoesis_domain::s7::SourceSpan;
use codenoesis_domain::s8_kotlin::{
    Declaration, DeclarationKind, GradleFile, Import, KotlinError, KotlinSource, KotlinWorkspace,
    MAX_ENTITIES, MAX_SOURCE_BYTES, MAX_SYNTAX_DEPTH, MAX_SYNTAX_NODES, SourceExtraction,
};
use codenoesis_ports::KotlinWorkspaceExtractor;
use std::collections::BTreeSet;
use tree_sitter::{Node, Parser};

pub struct TreeSitterKotlinWorkspaceExtractor;

impl KotlinWorkspaceExtractor for TreeSitterKotlinWorkspaceExtractor {
    #[allow(
        clippy::case_sensitive_file_extension_comparisons,
        reason = "Committed Git paths follow exact Kotlin and Java suffix spelling"
    )]
    fn extract_kotlin_workspace(
        &self,
        inventory: &RepositoryInventory,
    ) -> Result<KotlinWorkspace, KotlinError> {
        let mut workspace = KotlinWorkspace::default();
        let modules: BTreeSet<_> = inventory
            .files()
            .iter()
            .filter_map(|file| {
                let path = file.path();
                let (parent, name) = path.rsplit_once('/').unwrap_or(("", path));
                matches!(name, "build.gradle.kts" | "build.gradle").then(|| parent.to_owned())
            })
            .collect();
        let mut count = 0;
        for file in inventory.files() {
            let path = file.path();
            if path.ends_with(".gradle.kts") || path.ends_with(".gradle") {
                workspace.gradle_files.push(GradleFile {
                    path: path.to_owned(),
                    blob_oid: file.blob_oid().to_string(),
                    byte_length: file.bytes().len(),
                });
            } else if path.ends_with(".java") {
                workspace.java_files.push(path.to_owned());
            } else if path.ends_with(".kt") {
                let extraction = match extract_source(file.bytes()) {
                    Ok(extraction) => Some(extraction),
                    Err(KotlinError::InvalidSyntax) => None,
                    Err(error) => return Err(error),
                };
                count += extraction
                    .as_ref()
                    .map_or(0, |e| e.declarations.len() + e.imports.len());
                if count > MAX_ENTITIES {
                    return Err(KotlinError::LimitExceeded("entities"));
                }
                let membership = conventional_membership(path, &modules);
                workspace.sources.push(KotlinSource {
                    path: path.to_owned(),
                    blob_oid: file.blob_oid().to_string(),
                    byte_length: file.bytes().len(),
                    module: membership.as_ref().map(|m| m.0.clone()),
                    source_set: membership.map(|m| m.1),
                    extraction,
                });
            }
        }
        if workspace.sources.is_empty() {
            return Err(KotlinError::NoKotlinSources);
        }
        Ok(workspace)
    }
}

fn conventional_membership(path: &str, modules: &BTreeSet<String>) -> Option<(String, String)> {
    path.match_indices("src/")
        .filter_map(|(offset, _)| {
            if offset > 0 && path.as_bytes()[offset - 1] != b'/' {
                return None;
            }
            let module = if offset == 0 { "" } else { &path[..offset - 1] };
            if !modules.contains(module) {
                return None;
            }
            let (set, relative) = path[offset + 4..].split_once('/')?;
            (!set.is_empty() && relative.starts_with("kotlin/")).then_some((module, set))
        })
        .max_by_key(|(module, _)| module.len())
        .map(|(module, set)| (module.to_owned(), set.to_owned()))
}

/// Extracts declarations and exact byte/line locations from one UTF-8 Kotlin file.
/// # Errors
/// Returns invalid syntax/encoding or a bounded parser resource failure.
pub fn extract_source(bytes: &[u8]) -> Result<SourceExtraction, KotlinError> {
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err(KotlinError::LimitExceeded("source_bytes"));
    }
    let source = std::str::from_utf8(bytes).map_err(|_| KotlinError::InvalidUtf8)?;
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_kotlin_ng::LANGUAGE.into())
        .map_err(|_| KotlinError::InvalidContract)?;
    let tree = parser
        .parse(source, None)
        .ok_or(KotlinError::InvalidSyntax)?;
    let root = tree.root_node();
    let mut stack = vec![(root, 0)];
    let mut count = 0;
    while let Some((node, depth)) = stack.pop() {
        count += 1;
        if count > MAX_SYNTAX_NODES {
            return Err(KotlinError::LimitExceeded("syntax_nodes"));
        }
        if depth > MAX_SYNTAX_DEPTH {
            return Err(KotlinError::LimitExceeded("syntax_depth"));
        }
        let mut cursor = node.walk();
        stack.extend(node.children(&mut cursor).map(|n| (n, depth + 1)));
    }
    if root.has_error() {
        return Err(KotlinError::InvalidSyntax);
    }
    let mut result = SourceExtraction::default();
    let mut cursor = root.walk();
    for node in root.named_children(&mut cursor) {
        match node.kind() {
            "package_header" => {
                result.package = named_child(node, "qualified_identifier")
                    .map_or_else(String::new, |n| text(n, source).to_owned());
            }
            "import" => result.imports.push(Import {
                text: text(node, source).to_owned(),
                span: span(node),
            }),
            _ => declaration(node, source, "", &mut result.declarations),
        }
    }
    Ok(result)
}

fn declaration(node: Node<'_>, source: &str, owner: &str, result: &mut Vec<Declaration>) {
    let kind = match node.kind() {
        "class_declaration" if has_token(node, "interface") => DeclarationKind::Interface,
        "class_declaration" => DeclarationKind::Class,
        "object_declaration" | "companion_object" => DeclarationKind::Object,
        "function_declaration" => DeclarationKind::Function,
        "property_declaration" | "class_parameter" => DeclarationKind::Property,
        "type_alias" => DeclarationKind::TypeAlias,
        _ => return,
    };
    if node.kind() == "class_parameter" && !has_token(node, "val") && !has_token(node, "var") {
        return;
    }
    let name = node
        .child_by_field_name("name")
        .or_else(|| node.child_by_field_name("type"))
        .or_else(|| {
            named_child(node, "variable_declaration").and_then(|n| named_child(n, "identifier"))
        })
        .or_else(|| named_child(node, "identifier"));
    let Some(name) = name else {
        return;
    };
    let name = text(name, source).to_owned();
    let modifiers = named_child(node, "modifiers");
    let is_expect = modifiers.is_some_and(|m| has_platform_modifier(m, source, "expect"));
    let is_actual = modifiers.is_some_and(|m| has_platform_modifier(m, source, "actual"));
    let signature = signature(node, source);
    result.push(Declaration {
        name: name.clone(),
        owner: owner.to_owned(),
        kind,
        signature,
        is_expect,
        is_actual,
        span: span(node),
    });
    if matches!(
        kind,
        DeclarationKind::Class | DeclarationKind::Interface | DeclarationKind::Object
    ) {
        let nested_owner = if owner.is_empty() {
            name
        } else {
            format!("{owner}.{name}")
        };
        if let Some(constructor) = named_child(node, "primary_constructor")
            && let Some(parameters) = named_child(constructor, "class_parameters")
        {
            let mut cursor = parameters.walk();
            for child in parameters.named_children(&mut cursor) {
                declaration(child, source, &nested_owner, result);
            }
        }
        if let Some(body) =
            named_child(node, "class_body").or_else(|| named_child(node, "enum_class_body"))
        {
            let mut cursor = body.walk();
            for child in body.named_children(&mut cursor) {
                declaration(child, source, &nested_owner, result);
            }
        }
    }
}

fn signature(node: Node<'_>, source: &str) -> String {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .filter(|n| {
            !matches!(
                n.kind(),
                "modifiers"
                    | "class_body"
                    | "enum_class_body"
                    | "function_body"
                    | "getter"
                    | "setter"
            )
        })
        .take_while(|n| !matches!(n.kind(), "=" | "by"))
        .map(|n| text(n, source))
        .collect::<Vec<_>>()
        .join(" ")
}
fn has_platform_modifier(node: Node<'_>, source: &str, modifier: &str) -> bool {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .any(|n| n.kind() == "platform_modifier" && text(n, source) == modifier)
}
fn has_token(node: Node<'_>, token: &str) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor).any(|n| n.kind() == token)
}
fn named_child<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).find(|n| n.kind() == kind)
}
fn text<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}
fn span(node: Node<'_>) -> SourceSpan {
    SourceSpan {
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        start_line: node.start_position().row as u64 + 1,
        end_line: node.end_position().row as u64 + 1,
    }
}

#[cfg(test)]
mod tests {
    use super::conventional_membership;
    use std::collections::BTreeSet;
    #[test]
    fn fr_ext_025_membership_uses_components_and_nearest_committed_build_root() {
        let modules = BTreeSet::from([
            String::new(),
            "shared".to_owned(),
            "shared/src/commonMain/kotlin/nested".to_owned(),
        ]);
        assert_eq!(
            conventional_membership("src/main/kotlin/Main.kt", &modules),
            Some((String::new(), "main".to_owned()))
        );
        assert_eq!(
            conventional_membership(
                "shared/src/commonMain/kotlin/nested/src/jvmMain/kotlin/Inner.kt",
                &modules
            ),
            Some((
                "shared/src/commonMain/kotlin/nested".to_owned(),
                "jvmMain".to_owned()
            ))
        );
        assert_eq!(
            conventional_membership("shared/not-src/commonMain/kotlin/Fake.kt", &modules),
            None
        );
    }
}
