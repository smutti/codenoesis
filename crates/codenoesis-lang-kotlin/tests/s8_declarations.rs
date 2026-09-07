use codenoesis_domain::s8_kotlin::DeclarationKind;
use codenoesis_lang_kotlin::workspace::extract_source;

#[test]
fn fr_ext_025_declarations_preserve_source_evidence_and_scopes() {
    let source = "package demo\nimport java.time.Instant\nexpect class Platform\ndata class Greeting(val text: String) {\n fun say(who: String): String = text + who\n}\nobject Clock {\n val now: String = \"fun fake() {}\"\n}\nfun outer() { fun hidden() {} }\nval café: String = \"salut\"\n";
    let extraction = extract_source(source.as_bytes()).unwrap();
    assert_eq!(extraction.package, "demo");
    let names: Vec<_> = extraction
        .declarations
        .iter()
        .map(|d| d.name.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "Platform", "Greeting", "text", "say", "Clock", "now", "outer", "café"
        ]
    );
    assert!(extraction.declarations[0].is_expect);
    assert_eq!(extraction.declarations[2].kind, DeclarationKind::Property);
    assert_eq!(extraction.declarations[2].owner, "Greeting");
    for declaration in extraction.declarations {
        let span = declaration.span;
        assert!(span.is_valid_for(source.len()));
        assert!(source[span.start_byte..span.end_byte].contains(&declaration.name));
    }
}

#[test]
fn fr_ext_025_malformed_file_has_no_partial_facts() {
    assert!(extract_source(b"package demo\nclass Broken { fun (").is_err());
    assert!(extract_source(&[0xff]).is_err());
}

#[test]
fn fr_ext_025_modifiers_are_syntax_not_words_in_annotations() {
    let source = "@Suppress(\"expect actual\")\nfun normal(): String = \"actual\"\nactual fun platformName(): String = \"JVM\"\nactual typealias NativeName = String\n";
    let extraction = extract_source(source.as_bytes()).unwrap();
    assert!(!extraction.declarations[0].is_expect);
    assert!(!extraction.declarations[0].is_actual);
    assert!(extraction.declarations[1].is_actual);
    assert_eq!(extraction.declarations[2].kind, DeclarationKind::TypeAlias);
}

#[test]
fn fr_ext_025_pinned_parser_rejects_single_line_type_members_explicitly() {
    // Valid Kotlin, outside the syntax accepted by tree-sitter-kotlin-ng 1.1.0.
    assert!(extract_source(b"class Greeting { fun say() = 1 }\n").is_err());
}

#[test]
fn fr_ext_025_source_capacity_is_checked_before_parsing() {
    use codenoesis_domain::s8_kotlin::{KotlinError, MAX_SOURCE_BYTES};
    assert_eq!(
        extract_source(&vec![b' '; MAX_SOURCE_BYTES + 1]),
        Err(KotlinError::LimitExceeded("source_bytes"))
    );
}
