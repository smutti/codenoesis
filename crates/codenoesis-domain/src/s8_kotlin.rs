//! Source-only Kotlin/KMP facts. No compiler or effective Gradle claims.
use crate::s7::SourceSpan;
use std::collections::BTreeMap;

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

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DeclarationLocation {
    pub source: usize,
    pub declaration: usize,
}

type CandidateGroup<'a> = Vec<(DeclarationLocation, &'a KotlinSource, &'a Declaration)>;

impl KotlinWorkspace {
    /// Indexes unique syntactic expect candidates once for the complete workspace.
    /// This never establishes compiler actualization or source-set visibility.
    #[must_use]
    pub fn expect_candidates(&self) -> BTreeMap<DeclarationLocation, DeclarationLocation> {
        let mut groups: BTreeMap<(&str, &str, &str, DeclarationKind), CandidateGroup<'_>> =
            BTreeMap::new();
        for (source_index, source) in self.sources.iter().enumerate() {
            let (Some(module), Some(_), Some(extraction)) =
                (&source.module, &source.source_set, &source.extraction)
            else {
                continue;
            };
            for (declaration_index, declaration) in extraction.declarations.iter().enumerate() {
                if !declaration.owner.is_empty() || declaration.kind == DeclarationKind::TypeAlias {
                    continue;
                }
                groups
                    .entry((
                        module,
                        &extraction.package,
                        &declaration.name,
                        declaration.kind,
                    ))
                    .or_default()
                    .push((
                        DeclarationLocation {
                            source: source_index,
                            declaration: declaration_index,
                        },
                        source,
                        declaration,
                    ));
            }
        }
        let mut matches = BTreeMap::new();
        for group in groups.into_values() {
            match_group(&group, &mut matches);
        }
        matches
    }
}

fn match_group(
    group: &CandidateGroup<'_>,
    matches: &mut BTreeMap<DeclarationLocation, DeclarationLocation>,
) {
    let mut expected = group
        .iter()
        .filter(|(_, _, declaration)| declaration.is_expect);
    let Some((expect_location, expect_source, expect_declaration)) = expected.next() else {
        return;
    };
    // Multiple overloads remain ambiguous even if one signature happens to match.
    if expected.next().is_some() || expect_declaration.is_actual {
        return;
    }
    let mut counts: BTreeMap<Option<&str>, usize> = BTreeMap::new();
    for (_, source, _) in group {
        *counts.entry(source.source_set.as_deref()).or_default() += 1;
    }
    for (location, source, declaration) in group {
        if !declaration.is_actual
            || declaration.is_expect
            || source.source_set == expect_source.source_set
            || counts[&source.source_set.as_deref()] != 1
        {
            continue;
        }
        if matches!(
            declaration.kind,
            DeclarationKind::Function | DeclarationKind::Property
        ) && declaration.signature != expect_declaration.signature
        {
            continue;
        }
        matches.insert(*location, *expect_location);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source(set: &str, expect: bool, count: usize) -> KotlinSource {
        let declarations = (0..count)
            .map(|index| Declaration {
                name: format!("name{index}"),
                owner: String::new(),
                kind: DeclarationKind::Function,
                signature: format!("fun name{index} ()"),
                is_expect: expect,
                is_actual: !expect,
                span: SourceSpan {
                    start_byte: 0,
                    end_byte: 1,
                    start_line: 1,
                    end_line: 1,
                },
            })
            .collect();
        KotlinSource {
            path: format!("{set}.kt"),
            blob_oid: String::new(),
            byte_length: 1,
            module: Some("shared".to_owned()),
            source_set: Some(set.to_owned()),
            extraction: Some(SourceExtraction {
                declarations,
                ..SourceExtraction::default()
            }),
        }
    }
    #[test]
    fn fr_ext_025_many_names_and_duplicate_actuals_keep_candidate_boundaries() {
        let mut workspace = KotlinWorkspace {
            sources: vec![
                source("commonMain", true, 6000),
                source("jvmMain", false, 6000),
                source("iosMain", false, 6000),
            ],
            ..KotlinWorkspace::default()
        };
        let initial = workspace.expect_candidates();
        assert_eq!(initial.len(), 12_000);
        let duplicate = workspace.sources[1]
            .extraction
            .as_ref()
            .unwrap()
            .declarations[0]
            .clone();
        workspace.sources[1]
            .extraction
            .as_mut()
            .unwrap()
            .declarations
            .push(duplicate);
        let candidates = workspace.expect_candidates();
        assert_eq!(candidates.len(), 11_999);
        assert!(!candidates.contains_key(&DeclarationLocation {
            source: 1,
            declaration: 0
        }));
        assert!(candidates.contains_key(&DeclarationLocation {
            source: 2,
            declaration: 0
        }));
    }
}
