//! Deterministic untrusted contract-ingest adapters.

mod openapi;

use codenoesis_domain::s6::{ContractError, EvidenceSelector, OpenApiContractInput};
use codenoesis_domain::s7::{ContractFieldProjection, ContractProjection, SourceSpan};
use codenoesis_ports::{OpenApiContractExtractor, S7OpenApiContractProjector};

pub use openapi::OpenApi31HttpJsonExtractor;

impl S7OpenApiContractProjector for OpenApi31HttpJsonExtractor {
    fn project_s7(
        &self,
        input: OpenApiContractInput<'_>,
        expected_operation_id: &str,
    ) -> Result<ContractProjection, ContractError> {
        let contract = OpenApiContractExtractor::extract(self, input)?;
        if !contract.coverage_gaps.is_empty() {
            return Err(ContractError::UnsupportedCapability {
                path: contract.binding.contract_path,
            });
        }
        let operation = contract
            .operations
            .iter()
            .find(|operation| operation.operation_id == expected_operation_id)
            .ok_or_else(|| ContractError::InvalidOperation {
                path: contract.binding.contract_path.clone(),
            })?;
        let schema_evidence_id = operation
            .fields
            .first()
            .and_then(|field| field.evidence_ids.first())
            .ok_or_else(|| ContractError::InvalidOperation {
                path: contract.binding.contract_path.clone(),
            })?;
        if operation
            .fields
            .iter()
            .any(|field| field.evidence_ids.as_slice() != [schema_evidence_id.as_str()])
        {
            return Err(ContractError::UnsupportedCapability {
                path: contract.binding.contract_path,
            });
        }
        let evidence = contract
            .evidence
            .iter()
            .find(|evidence| &evidence.evidence_id == schema_evidence_id)
            .ok_or_else(|| ContractError::InvalidOperation {
                path: contract.binding.contract_path.clone(),
            })?;
        let EvidenceSelector::OpenApiLocationSpan {
            start_line,
            end_line,
            ..
        } = evidence.selector
        else {
            return Err(ContractError::UnsupportedCapability {
                path: contract.binding.contract_path,
            });
        };
        let mut fields = operation
            .fields
            .iter()
            .map(|field| ContractFieldProjection {
                field_id: field.field_id.clone(),
                json_pointer: field.json_pointer.clone(),
                required: field.required,
            })
            .collect::<Vec<_>>();
        fields.sort_by(|left, right| left.field_id.cmp(&right.field_id));
        Ok(ContractProjection {
            service_id: operation.service_id.clone(),
            operation_id: operation.operation_id.clone(),
            method: operation.method.as_str().to_owned(),
            path_template: operation.path_template.clone(),
            explicit_operation_id: operation.explicit_operation_id.clone(),
            response_status: operation.response_status.clone(),
            fields,
            evidence_span: SourceSpan {
                start_byte: 0,
                end_byte: 1,
                start_line,
                end_line,
            },
        })
    }
}
