use super::*;

#[derive(Clone)]
pub struct AppState {
    pub node: NodeState,
    pub protocol: std::sync::Arc<ProtocolEngine>,
    pub auth_token: String,
    pub allow_identity_export: bool,
    pub version: String,
}
