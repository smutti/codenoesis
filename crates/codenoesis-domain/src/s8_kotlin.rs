//! Source-only Kotlin/KMP facts. No compiler or effective Gradle claims.
use crate::s7::SourceSpan;

pub const PROFILE: &str = "kotlin-kmp-declarations-v1";
pub const SNAPSHOT_VERSION: &str = "codenoesis.repository-snapshot/v19";
pub const ONTOLOGY_VERSION: &str = "codenoesis.ontology/kotlin-kmp/v1";
pub const GRAPH_VERSION: &str = "codenoesis.kotlin-graph/v1";
pub const SNAPSHOT_HASH_DOMAIN: &str = "codenoesis.repository-snapshot.semantic.v19";
pub const GRAPH_HASH_DOMAIN: &str = "codenoesis.kotlin-graph.semantic.v1";
pub const EXTRACTION_HASH_DOMAIN: &str = "codenoesis.kotlin-extraction.semantic.v1";
pub const MAX_SOURCE_BYTES: usize = 4_194_304;
pub const MAX_SYNTAX_NODES: usize = 250_000;
pub const MAX_SYNTAX_DEPTH: usize = 128;
pub const MAX_ENTITIES: usize = 100_000;
pub const MAX_OUTPUT_BYTES: usize = 33_554_432;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DeclarationKind {
    Class,
    Interface,
    Object,
    Function,
    Property,
    TypeAlias,
}
impl DeclarationKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Class => "KotlinClass",
            Self::Interface => "KotlinInterface",
            Self::Object => "KotlinObject",
            Self::Function => "KotlinFunction",
            Self::Property => "KotlinProperty",
            Self::TypeAlias => "KotlinTypeAlias",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Declaration {
    pub name: String,
    pub owner: String,
    pub kind: DeclarationKind,
    pub signature: String,
    pub is_expect: bool,
    pub is_actual: bool,
    pub span: SourceSpan,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Import {
    pub text: String,
    pub span: SourceSpan,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SourceExtraction {
    pub package: String,
    pub declarations: Vec<Declaration>,
    pub imports: Vec<Import>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KotlinError {
    InvalidUtf8,
    InvalidSyntax,
    LimitExceeded(&'static str),
    NoKotlinSources,
    InvalidContract,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KotlinSource {
    pub path: String,
    pub blob_oid: String,
    pub byte_length: usize,
    pub module: Option<String>,
    pub source_set: Option<String>,
    pub extraction: Option<SourceExtraction>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GradleFile {
    pub path: String,
    pub blob_oid: String,
    pub byte_length: usize,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct KotlinWorkspace {
    pub sources: Vec<KotlinSource>,
    pub gradle_files: Vec<GradleFile>,
    pub java_files: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeclarationLocation {
    pub source: usize,
    pub declaration: usize,
}

impl KotlinWorkspace {
    /// Finds a unique syntactic expect candidate for one actual declaration.
    /// This never establishes compiler actualization or source-set visibility.
    #[must_use]
    pub fn expect_candidate(&self, actual: DeclarationLocation) -> Option<DeclarationLocation> {
        let actual_source = self.sources.get(actual.source)?;
        let actual_extraction = actual_source.extraction.as_ref()?;
        let actual_decl = actual_extraction.declarations.get(actual.declaration)?;
        if !actual_decl.is_actual
            || actual_decl.is_expect
            || !actual_decl.owner.is_empty()
            || actual_decl.kind == DeclarationKind::TypeAlias
            || actual_source.module.is_none()
            || actual_source.source_set.is_none()
        {
            return None;
        }
        let mut candidates = Vec::new();
        for (source_index, source) in self.sources.iter().enumerate() {
            let Some(extraction) = &source.extraction else {
                continue;
            };
            if source.module != actual_source.module
                || extraction.package != actual_extraction.package
            {
                continue;
            }
            for (declaration_index, declaration) in extraction.declarations.iter().enumerate() {
                if declaration.name != actual_decl.name
                    || declaration.kind != actual_decl.kind
                    || !declaration.owner.is_empty()
                {
                    continue;
                }
                if source.source_set == actual_source.source_set {
                    if source_index != actual.source || declaration_index != actual.declaration {
                        return None;
                    }
                } else if declaration.is_expect {
                    // Multiple overloads remain ambiguous even if one signature happens to match.
                    candidates.push((
                        DeclarationLocation {
                            source: source_index,
                            declaration: declaration_index,
                        },
                        declaration,
                    ));
                }
            }
        }
        if candidates.len() != 1 {
            return None;
        }
        let (location, candidate) = candidates[0];
        if candidate.is_actual {
            return None;
        }
        if matches!(
            actual_decl.kind,
            DeclarationKind::Function | DeclarationKind::Property
        ) && actual_decl.signature != candidate.signature
        {
            return None;
        }
        Some(location)
    }
}
