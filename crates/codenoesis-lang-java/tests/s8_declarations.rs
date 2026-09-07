use codenoesis_domain::s8_java::{DeclarationKind, JavaError, MAX_SOURCE_BYTES, SourceGap};
use codenoesis_lang_java::workspace::extract_source;

#[test]
fn fr_ext_026_named_declarations_preserve_overloads_and_skip_local_bodies() {
    let bytes = include_bytes!(
        "../../../tests/fixtures/s8/java-declarations-v1/repository/src/main/java/example/Library.java"
    );
    let result = extract_source(bytes).unwrap();
    assert_eq!(result.package.as_ref().unwrap().name, "example");
    assert_eq!(result.declarations.len(), 22);
    assert_eq!(result.imports.len(), 2);
    assert!(!result.imports[0].is_static);
    assert!(result.imports[1].is_static);
    assert!(
        result
            .declarations
            .iter()
            .all(|d| !d.name.contains("Decoy") && d.name != "toString")
    );
    let overloads: Vec<_> = result
        .declarations
        .iter()
        .filter(|d| d.name == "overloaded")
        .collect();
    assert_eq!(overloads.len(), 2);
    assert_ne!(overloads[0].span, overloads[1].span);
    assert_eq!(overloads[0].signature, "public void overloaded(int value)");
    assert_eq!(
        overloads[1].signature,
        "public void overloaded(String value)"
    );
    for declaration in &result.declarations {
        if let Some(parent) = declaration.parent {
            assert!(
                parent
                    < result
                        .declarations
                        .iter()
                        .position(|d| std::ptr::eq(d, declaration))
                        .unwrap()
            );
            assert!(result.declarations[parent].kind.is_type());
        }
    }
    assert_eq!(
        result
            .declarations
            .iter()
            .find(|d| d.name == "second")
            .unwrap()
            .signature,
        "int second[]"
    );
    assert_eq!(
        result
            .declarations
            .iter()
            .filter(|d| d.kind == DeclarationKind::RecordComponent)
            .count(),
        2
    );
    assert_eq!(
        result
            .declarations
            .iter()
            .filter(|d| d.kind == DeclarationKind::Constructor)
            .count(),
        2
    );
}

#[test]
fn fr_ext_026_utf8_byte_spans_and_package_annotations_are_exact() {
    let source = "@Deprecated\npackage caffè;\nclass Caffè { String nome; }\n";
    let result = extract_source(source.as_bytes()).unwrap();
    assert_eq!(result.package.as_ref().unwrap().name, "caffè");
    let declaration = &result.declarations[0];
    assert_eq!(declaration.name, "Caffè");
    assert_eq!(
        declaration.span.start_byte,
        source.find("class Caffè").unwrap()
    );
    assert_eq!(declaration.span.start_line, 3);
    assert_eq!(
        &source[declaration.span.start_byte..declaration.span.end_byte],
        "class Caffè { String nome; }"
    );
}

#[test]
fn fr_ext_026_source_failures_do_not_synthesize_declarations() {
    assert_eq!(extract_source(b"class {"), Err(JavaError::InvalidSyntax));
    assert_eq!(extract_source(&[0xff]), Err(JavaError::InvalidUtf8));
    assert_eq!(
        extract_source(&vec![b' '; MAX_SOURCE_BYTES + 1]),
        Err(JavaError::LimitExceeded("source_bytes"))
    );
    assert_eq!(
        extract_source(br"class \u0041 {}"),
        Err(JavaError::SourceBoundary(SourceGap::UnicodeEscape))
    );
    assert_eq!(
        extract_source(b"module example { exports example; }"),
        Err(JavaError::SourceBoundary(SourceGap::ModuleDescriptor))
    );
    assert_eq!(
        extract_source(b"void main() {}"),
        Err(JavaError::SourceBoundary(SourceGap::CompilationUnit))
    );
}

#[test]
fn fr_ext_026_record_varargs_constants_and_empty_compilation_units() {
    let result = extract_source(b"import java.util.*; record Names(String... values) {} interface Constants { int A = 1, B = 2; }").unwrap();
    assert!(result.imports[0].is_wildcard);
    let values = result
        .declarations
        .iter()
        .find(|d| d.name == "values")
        .unwrap();
    assert_eq!(values.kind, DeclarationKind::RecordComponent);
    assert!(values.signature.contains("String... values"));
    assert_eq!(
        result
            .declarations
            .iter()
            .filter(|d| d.kind == DeclarationKind::Field)
            .count(),
        2
    );
    assert!(
        extract_source(b"// no declarations\n")
            .unwrap()
            .declarations
            .is_empty()
    );
}

#[test]
fn fr_ext_026_syntax_node_and_depth_limits_are_typed() {
    let wide = format!("class Wide {{ {} }}", "int x;".repeat(60_000));
    assert_eq!(
        extract_source(wide.as_bytes()),
        Err(JavaError::LimitExceeded("syntax_nodes"))
    );
    let deep = format!("{}{}", "class Nested {".repeat(150), "}".repeat(150));
    assert_eq!(
        extract_source(deep.as_bytes()),
        Err(JavaError::LimitExceeded("syntax_depth"))
    );
}
