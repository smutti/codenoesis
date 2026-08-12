use codenoesis_domain::s7::{ClientPresenceAssumption, SourceExtractionError};
use codenoesis_lang_kotlin::TreeSitterKotlinClientExtractor;
use codenoesis_ports::KotlinClientSourceExtractor;

const STRICT: &[u8] = include_bytes!(
    "../../../tests/fixtures/s7/implementation-aware-api-v1/clients/strict/src/commonMain/kotlin/dev/codenoesis/fixture/StrictUsersClient.kt"
);
const SAFE: &[u8] = include_bytes!(
    "../../../tests/fixtures/s7/implementation-aware-api-v1/clients/safe/src/commonMain/kotlin/dev/codenoesis/fixture/SafeUsersClient.kt"
);
const DECOY: &[u8] = include_bytes!(
    "../../../tests/fixtures/s7/implementation-aware-api-v1/clients/decoy/src/commonMain/kotlin/dev/codenoesis/fixture/DecoyAccountClient.kt"
);

#[test]
fn conf_fr_ext_019_strict_safe_and_decoy_paths() {
    let extractor = TreeSitterKotlinClientExtractor::new();
    let strict = extractor
        .extract_s7_client(STRICT, "decodeStrictUser", "getStrictUser")
        .expect("extract strict client");
    let safe = extractor
        .extract_s7_client(SAFE, "decodeSafeUser", "getSafeUser")
        .expect("extract safe client");
    let decoy = extractor
        .extract_s7_client(DECOY, "decodeAccount", "getAccount")
        .expect("extract decoy client");

    assert_eq!(strict.path_template, "/users/{id}");
    assert_eq!(
        strict.assumptions[1].assumption,
        ClientPresenceAssumption::RequiresPresent
    );
    assert_eq!(
        (
            strict.evidence_span.start_line,
            strict.evidence_span.end_line
        ),
        (6, 15)
    );
    assert_eq!(
        safe.assumptions[1].assumption,
        ClientPresenceAssumption::HandlesAbsent
    );
    assert_eq!(
        (safe.evidence_span.start_line, safe.evidence_span.end_line),
        (7, 16)
    );
    assert_eq!(decoy.path_template, "/accounts/{id}");
    assert_eq!(
        (decoy.evidence_span.start_line, decoy.evidence_span.end_line),
        (6, 15)
    );
}

#[test]
fn sec_fr_ext_019_dynamic_path_and_unguarded_index_fail_closed() {
    let extractor = TreeSitterKotlinClientExtractor::new();
    let dynamic = br#"
data class Dto(val nickname: String)
fun decode(payload: JsonObject): Dto = Dto(payload.getValue("nickname").jsonPrimitive.content)
suspend fun call(id: String): Dto = decode(httpGet(pathFor(id)))
"#;
    let unguarded = br#"
data class Dto(val nickname: String)
fun decode(payload: JsonObject): Dto { val nickname = payload["nickname"].jsonPrimitive.content; return Dto(nickname) }
suspend fun call(id: String): Dto = decode(httpGet("/users/$id"))
"#;
    for source in [dynamic.as_slice(), unguarded.as_slice()] {
        assert_eq!(
            extractor.extract_s7_client(source, "decode", "call"),
            Err(SourceExtractionError::UnsupportedSemantics)
        );
    }
}

#[test]
fn sec_fr_ext_019_throwing_safe_chain_and_indirect_call_fail_closed() {
    let extractor = TreeSitterKotlinClientExtractor::new();
    let throwing = br#"
data class Dto(val nickname: String)
fun decode(payload: JsonObject): Dto { val nickname = payload["nickname"]?.jsonPrimitive?.content ?: error("missing"); return Dto(nickname) }
suspend fun call(id: String): Dto = decode(httpGet("/users/$id"))
"#;
    let indirect = br#"
data class Dto(val nickname: String)
fun decode(payload: JsonObject): Dto = Dto(payload.getValue("nickname").jsonPrimitive.content)
suspend fun call(id: String): Dto = decode(httpGet("/users/$id"), fallback())
"#;
    let wrapped = br#"
data class Dto(val nickname: String)
fun decode(payload: JsonObject): Dto { val nickname = requireNotNull(payload["nickname"]?.jsonPrimitive?.content); return Dto(nickname) }
suspend fun call(id: String): Dto = decode(httpGet("/users/$id"))
"#;
    for source in [throwing.as_slice(), indirect.as_slice(), wrapped.as_slice()] {
        assert_eq!(
            extractor.extract_s7_client(source, "decode", "call"),
            Err(SourceExtractionError::UnsupportedSemantics)
        );
    }
}
