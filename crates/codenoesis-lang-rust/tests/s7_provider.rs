use codenoesis_domain::s7::{ProviderPresence, SourceExtractionError};
use codenoesis_lang_rust::TreeSitterRustProviderExtractor;
use codenoesis_ports::RustProviderSourceExtractor;

const BASELINE: &[u8] = include_bytes!(
    "../../../tests/fixtures/s7/implementation-aware-api-v1/provider/revision-a/src/user_response.rs"
);
const TARGET: &[u8] = include_bytes!(
    "../../../tests/fixtures/s7/implementation-aware-api-v1/provider/revision-b/src/user_response.rs"
);

#[test]
fn conf_fr_ext_018_direct_map_presence_and_custom_gap() {
    let extractor = TreeSitterRustProviderExtractor::new();
    let baseline = extractor
        .extract_s7_provider(BASELINE, "user_response")
        .expect("extract baseline provider");
    let target = extractor
        .extract_s7_provider(TARGET, "user_response")
        .expect("extract target provider");

    assert_eq!(baseline.fields[1].field_name, "nickname");
    assert_eq!(
        baseline.fields[1].presence,
        ProviderPresence::GuaranteedPresent
    );
    assert_eq!(
        (
            baseline.fields[1].span.start_line,
            baseline.fields[1].span.end_line
        ),
        (6, 6)
    );
    assert_eq!(target.fields[1].presence, ProviderPresence::MayBeAbsent);
    assert_eq!(
        (
            target.fields[1].span.start_line,
            target.fields[1].span.end_line
        ),
        (6, 8)
    );
    assert_eq!(
        (
            baseline.custom_mapping_spans[0].start_line,
            baseline.custom_mapping_spans[0].end_line
        ),
        (7, 7)
    );
    assert_eq!(
        (
            target.custom_mapping_spans[0].start_line,
            target.custom_mapping_spans[0].end_line
        ),
        (9, 9)
    );
}

#[test]
fn sec_fr_ext_018_dynamic_key_and_loop_fail_closed() {
    let extractor = TreeSitterRustProviderExtractor::new();
    for source in [
        br"fn selected(key: &str) -> Value { let mut body = Map::new(); body.insert(key.into(), Value::Null); body.extend(extra()); Value::Object(body) }".as_slice(),
        br"fn selected() -> Value { let mut body = Map::new(); for key in keys() { body.insert(key, Value::Null); } body.extend(extra()); Value::Object(body) }".as_slice(),
    ] {
        assert_eq!(
            extractor.extract_s7_provider(source, "selected"),
            Err(SourceExtractionError::UnsupportedSemantics)
        );
    }
}

#[test]
fn sec_fr_ext_018_helper_mutation_and_non_terminal_publication_fail_closed() {
    let extractor = TreeSitterRustProviderExtractor::new();
    for source in [
        br#"fn selected() -> Value { let mut body = Map::new(); body.insert("id".into(), Value::Null); mutate(&mut body); body.extend(extra()); Value::Object(body) }"#.as_slice(),
        br#"fn selected() -> Value { let mut body = Map::new(); body.insert("id".into(), Value::Null); drop(Value::Object(body)); Value::Null }"#.as_slice(),
        br#"fn selected() -> Value { let mut body = Map::new(); fn hidden() { body.insert("nickname".into(), Value::Null); } body.insert("id".into(), Value::Null); body.extend(extra()); Value::Object(body) }"#.as_slice(),
    ] {
        assert_eq!(
            extractor.extract_s7_provider(source, "selected"),
            Err(SourceExtractionError::UnsupportedSemantics)
        );
    }
}
