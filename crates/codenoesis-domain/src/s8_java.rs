//! Source-only Java declarations and explicit preprocessing/build boundaries.
use crate::s7::SourceSpan;

pub const PROFILE: &str = "java-declarations-v1";
pub const SNAPSHOT_VERSION: &str = "codenoesis.repository-snapshot/v20";
pub const ONTOLOGY_VERSION: &str = "codenoesis.ontology/java/v1";
pub const GRAPH_VERSION: &str = "codenoesis.java-graph/v1";
pub const SNAPSHOT_HASH_DOMAIN: &str = "codenoesis.repository-snapshot.semantic.v20";
pub const GRAPH_HASH_DOMAIN: &str = "codenoesis.java-graph.semantic.v1";
pub const EXTRACTION_HASH_DOMAIN: &str = "codenoesis.java-extraction.semantic.v1";
pub const MAX_SOURCE_BYTES: usize = 4_194_304;
pub const MAX_SYNTAX_NODES: usize = 250_000;
pub const MAX_SYNTAX_DEPTH: usize = 128;
pub const MAX_ENTITIES: usize = 100_000;
pub const MAX_OUTPUT_BYTES: usize = 33_554_432;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DeclarationKind {
    Class,
    Interface,
    Enum,
    Record,
    AnnotationType,
    Method,
    Constructor,
    Field,
    EnumConstant,
    RecordComponent,
    AnnotationElement,
}
impl DeclarationKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Class => "JavaClass",
            Self::Interface => "JavaInterface",
            Self::Enum => "JavaEnum",
            Self::Record => "JavaRecord",
            Self::AnnotationType => "JavaAnnotationType",
            Self::Method => "JavaMethod",
            Self::Constructor => "JavaConstructor",
            Self::Field => "JavaField",
            Self::EnumConstant => "JavaEnumConstant",
            Self::RecordComponent => "JavaRecordComponent",
            Self::AnnotationElement => "JavaAnnotationElement",
        }
    }
    #[must_use]
    pub const fn is_type(self) -> bool {
        matches!(
            self,
            Self::Class | Self::Interface | Self::Enum | Self::Record | Self::AnnotationType
        )
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Declaration {
    pub name: String,
    pub owner: String,
    pub parent: Option<usize>,
    pub kind: DeclarationKind,
    pub signature: String,
    pub span: SourceSpan,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Package {
    pub name: String,
    pub span: SourceSpan,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Import {
    pub text: String,
    pub is_static: bool,
    pub is_wildcard: bool,
    pub span: SourceSpan,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SourceExtraction {
    pub package: Option<Package>,
    pub declarations: Vec<Declaration>,
    pub imports: Vec<Import>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceGap {
    Syntax,
    UnicodeEscape,
    ModuleDescriptor,
    CompilationUnit,
}
impl SourceGap {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Syntax => "java_syntax_not_accepted_by_pinned_parser",
            Self::UnicodeEscape => "java_unicode_escape_preprocessing_not_supported",
            Self::ModuleDescriptor => "java_module_descriptor_boundary",
            Self::CompilationUnit => "java_compilation_unit_form_not_supported",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JavaError {
    InvalidUtf8,
    InvalidSyntax,
    SourceBoundary(SourceGap),
    LimitExceeded(&'static str),
    NoJavaSources,
    InvalidContract,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JavaSource {
    pub path: String,
    pub blob_oid: String,
    pub byte_length: usize,
    pub module: Option<String>,
    pub source_set: Option<String>,
    pub extraction: Result<SourceExtraction, SourceGap>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildFile {
    pub path: String,
    pub system: &'static str,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct JavaWorkspace {
    pub sources: Vec<JavaSource>,
    pub build_files: Vec<BuildFile>,
    pub kotlin_files: Vec<String>,
}
