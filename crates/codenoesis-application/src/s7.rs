use codenoesis_domain::s7::{
    ImpactAnalysisError, ImpactAnalysisInput, ImpactClassifier, SemanticCompatibilityReport,
};

pub struct ImpactService;

impl ImpactService {
    /// Applies the immutable S7 rule catalog to already validated facts.
    ///
    /// # Errors
    ///
    /// Returns a closed authority, capability, or resource-limit failure.
    pub fn analyze(
        input: ImpactAnalysisInput,
    ) -> Result<SemanticCompatibilityReport, ImpactAnalysisError> {
        ImpactClassifier::analyze(input)
    }
}
