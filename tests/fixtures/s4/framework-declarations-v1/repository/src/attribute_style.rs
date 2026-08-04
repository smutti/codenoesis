#[route("/candidate")]
pub fn candidate_route() {}

#[component]
pub struct CandidateComponent;

#[service]
pub fn candidate_service() {}

#[configuration]
pub fn candidate_configuration() {}

#[command]
pub fn candidate_command() {}

#[runtime::entry]
pub fn candidate_runtime() {}

#[bridge]
pub fn candidate_bridge() {}

#[cfg(feature = "candidate")]
#[route("/cfg-candidate")]
pub fn conditional_route() {}

#[cfg_attr(feature = "candidate", route("/cfg-attr-candidate"))]
pub fn transformed_route() {}

#[derive(Component)]
pub struct DerivedCandidate;

#[framework::endpoint("/candidate-endpoint")]
pub fn endpoint_candidate() {}

declare_routes! {
    GET "/generated" => generated_handler
}
