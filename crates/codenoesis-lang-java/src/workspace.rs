use codenoesis_domain::s8_java::{
    BuildFile, Declaration, DeclarationKind, Import, JavaError, JavaSource, JavaWorkspace,
    MAX_ENTITIES, MAX_SOURCE_BYTES, MAX_SYNTAX_DEPTH, MAX_SYNTAX_NODES, Package, SourceExtraction,
    SourceGap,
};
use codenoesis_domain::{RepositoryInventory, s7::SourceSpan};
use codenoesis_ports::JavaWorkspaceExtractor;
use std::collections::BTreeSet;
use tree_sitter::{Node, Parser};

pub struct TreeSitterJavaWorkspaceExtractor;
impl JavaWorkspaceExtractor for TreeSitterJavaWorkspaceExtractor {
    #[allow(
        clippy::case_sensitive_file_extension_comparisons,
        reason = "Exact committed language suffixes define the profile"
    )]
    fn extract_java_workspace(
        &self,
        inventory: &RepositoryInventory,
    ) -> Result<JavaWorkspace, JavaError> {
        let mut workspace = JavaWorkspace::default();
        let modules: BTreeSet<_> = inventory
            .files()
            .iter()
            .filter_map(|file| {
                let (parent, name) = file.path().rsplit_once('/').unwrap_or(("", file.path()));
                matches!(name, "pom.xml" | "build.gradle" | "build.gradle.kts")
                    .then(|| parent.to_owned())
            })
            .collect();
        let mut count = 0;
        for file in inventory.files() {
            let path = file.path();
            let name = path.rsplit('/').next().unwrap_or(path);
            if name == "pom.xml" || path.ends_with(".gradle") || path.ends_with(".gradle.kts") {
                workspace.build_files.push(BuildFile {
                    path: path.to_owned(),
                    system: if name == "pom.xml" { "maven" } else { "gradle" },
                });
            } else if path.ends_with(".kt") || path.ends_with(".kts") {
                workspace.kotlin_files.push(path.to_owned());
            } else if path.ends_with(".java") {
                let extraction = match extract_source(file.bytes()) {
                    Ok(extraction) => Ok(extraction),
                    Err(JavaError::InvalidSyntax) => Err(SourceGap::Syntax),
                    Err(JavaError::SourceBoundary(gap)) => Err(gap),
                    Err(error) => return Err(error),
                };
                count += extraction
                    .as_ref()
                    .map_or(0, |e| e.declarations.len() + e.imports.len());
                if count > MAX_ENTITIES {
                    return Err(JavaError::LimitExceeded("entities"));
                }
                let membership = conventional_membership(path, &modules);
                workspace.sources.push(JavaSource {
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
            return Err(JavaError::NoJavaSources);
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
            (!set.is_empty() && relative.starts_with("java/")).then_some((module, set))
        })
        .max_by_key(|(module, _)| module.len())
        .map(|(m, s)| (m.to_owned(), s.to_owned()))
}

/// Extracts a complete bounded Java compilation unit or an explicit boundary.
/// # Errors
/// Returns encoding, syntax, unsupported preprocessing/form or capacity errors.
pub fn extract_source(bytes: &[u8]) -> Result<SourceExtraction, JavaError> {
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err(JavaError::LimitExceeded("source_bytes"));
    }
    let source = std::str::from_utf8(bytes).map_err(|_| JavaError::InvalidUtf8)?;
    if bytes.windows(2).any(|pair| pair == b"\\u") {
        return Err(JavaError::SourceBoundary(SourceGap::UnicodeEscape));
    }
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_java::LANGUAGE.into())
        .map_err(|_| JavaError::InvalidContract)?;
    let tree = parser.parse(bytes, None).ok_or(JavaError::InvalidSyntax)?;
    let root = tree.root_node();
    let mut stack = vec![(root, 0)];
    let mut count = 0;
    while let Some((node, depth)) = stack.pop() {
        count += 1;
        if count > MAX_SYNTAX_NODES {
            return Err(JavaError::LimitExceeded("syntax_nodes"));
        }
        if depth > MAX_SYNTAX_DEPTH {
            return Err(JavaError::LimitExceeded("syntax_depth"));
        }
        let mut cursor = node.walk();
        stack.extend(node.children(&mut cursor).map(|n| (n, depth + 1)));
    }
    if root.has_error() {
        return Err(JavaError::InvalidSyntax);
    }
    let mut result = SourceExtraction::default();
    let mut cursor = root.walk();
    for node in root.named_children(&mut cursor) {
        match node.kind() {
            "line_comment" | "block_comment" => {}
            "package_declaration" => {
                let mut cursor = node.walk();
                let name = node
                    .named_children(&mut cursor)
                    .find(|n| matches!(n.kind(), "identifier" | "scoped_identifier"))
                    .ok_or(JavaError::InvalidSyntax)?;
                result.package = Some(Package {
                    name: text(name, source).to_owned(),
                    span: span(node),
                });
            }
            "import_declaration" => result.imports.push(Import {
                text: text(node, source).to_owned(),
                is_static: has_token(node, "static"),
                is_wildcard: named_child(node, "asterisk").is_some(),
                span: span(node),
            }),
            "module_declaration" => {
                return Err(JavaError::SourceBoundary(SourceGap::ModuleDescriptor));
            }
            "class_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "record_declaration"
            | "annotation_type_declaration" => {
                declaration(node, source, None, "", &mut result.declarations)?;
            }
            _ => return Err(JavaError::SourceBoundary(SourceGap::CompilationUnit)),
        }
    }
    Ok(result)
}

fn declaration(
    node: Node<'_>,
    source: &str,
    parent: Option<usize>,
    owner: &str,
    result: &mut Vec<Declaration>,
) -> Result<(), JavaError> {
    let kind = match node.kind() {
        "class_declaration" => DeclarationKind::Class,
        "interface_declaration" => DeclarationKind::Interface,
        "enum_declaration" => DeclarationKind::Enum,
        "record_declaration" => DeclarationKind::Record,
        "annotation_type_declaration" => DeclarationKind::AnnotationType,
        "method_declaration" => DeclarationKind::Method,
        "constructor_declaration" | "compact_constructor_declaration" => {
            DeclarationKind::Constructor
        }
        "annotation_type_element_declaration" => DeclarationKind::AnnotationElement,
        "enum_constant" => DeclarationKind::EnumConstant,
        "formal_parameter" | "spread_parameter" => DeclarationKind::RecordComponent,
        "field_declaration" | "constant_declaration" => {
            return fields(node, source, parent, owner, result);
        }
        "enum_body_declarations" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                declaration(child, source, parent, owner, result)?;
            }
            return Ok(());
        }
        _ => return Ok(()),
    };
    let name = node
        .child_by_field_name("name")
        .or_else(|| {
            named_child(node, "variable_declarator").and_then(|n| n.child_by_field_name("name"))
        })
        .ok_or(JavaError::InvalidSyntax)?;
    let name = text(name, source).to_owned();
    let body = node.child_by_field_name("body");
    let signature = if kind == DeclarationKind::EnumConstant {
        name.clone()
    } else {
        source[node.start_byte()..body.map_or(node.end_byte(), |n| n.start_byte())]
            .trim()
            .trim_end_matches(';')
            .trim()
            .to_owned()
    };
    let index = result.len();
    result.push(Declaration {
        name: name.clone(),
        owner: owner.to_owned(),
        parent,
        kind,
        signature,
        span: span(node),
    });
    if kind.is_type() {
        let nested = if owner.is_empty() {
            name
        } else {
            format!("{owner}.{name}")
        };
        if kind == DeclarationKind::Record
            && let Some(parameters) = node.child_by_field_name("parameters")
        {
            let mut cursor = parameters.walk();
            for child in parameters.named_children(&mut cursor) {
                declaration(child, source, Some(index), &nested, result)?;
            }
        }
        if let Some(body) = body {
            let mut cursor = body.walk();
            for child in body.named_children(&mut cursor) {
                declaration(child, source, Some(index), &nested, result)?;
            }
        }
    }
    Ok(())
}
fn fields(
    node: Node<'_>,
    source: &str,
    parent: Option<usize>,
    owner: &str,
    result: &mut Vec<Declaration>,
) -> Result<(), JavaError> {
    let declared_type = node
        .child_by_field_name("type")
        .ok_or(JavaError::InvalidSyntax)?;
    let mut cursor = node.walk();
    for variable in node.children_by_field_name("declarator", &mut cursor) {
        let name = variable
            .child_by_field_name("name")
            .ok_or(JavaError::InvalidSyntax)?;
        let dimensions = variable
            .child_by_field_name("dimensions")
            .map_or("", |n| text(n, source));
        result.push(Declaration {
            name: text(name, source).to_owned(),
            owner: owner.to_owned(),
            parent,
            kind: DeclarationKind::Field,
            signature: format!(
                "{} {}{}",
                text(declared_type, source),
                text(name, source),
                dimensions
            ),
            span: span(node),
        });
    }
    Ok(())
}
fn text<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    &source[node.start_byte()..node.end_byte()]
}
fn named_child<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).find(|n| n.kind() == kind)
}
fn has_token(node: Node<'_>, kind: &str) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor).any(|n| n.kind() == kind)
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
    use super::*;
    #[test]
    fn fr_ext_026_membership_uses_nearest_build_module_and_component_boundaries() {
        let modules = BTreeSet::from([
            String::new(),
            "shared".to_owned(),
            "shared/src/main/java/nested".to_owned(),
        ]);
        assert_eq!(
            conventional_membership("src/main/java/Root.java", &modules),
            Some((String::new(), "main".to_owned()))
        );
        assert_eq!(
            conventional_membership(
                "shared/src/main/java/nested/src/test/java/Inner.java",
                &modules
            ),
            Some(("shared/src/main/java/nested".to_owned(), "test".to_owned()))
        );
        assert_eq!(
            conventional_membership("shared/not-src/main/java/Fake.java", &modules),
            None
        );
    }
}
