use super::agent::AgentInteract;
use lsp_server::RequestId;
use lsp_types::{
    DidChangeTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    DocumentDiagnosticParams, GotoDefinitionParams, HoverParams,
};

#[derive(Debug, Clone)]
pub enum InteractLspMessage {
    Req {
        body: InteractLspRequest,
        id: RequestId,
    },
    Noti(InteractLspNotification),
}
impl From<(InteractLspRequest, RequestId)> for InteractLspMessage {
    fn from((body, id): (InteractLspRequest, RequestId)) -> Self {
        Self::Req { body, id }
    }
}
impl From<InteractLspNotification> for InteractLspMessage {
    fn from(value: InteractLspNotification) -> Self {
        Self::Noti(value)
    }
}

#[derive(Debug, Clone)]
pub enum InteractLspRequest {
    GotoDef(GotoDefinitionParams),
    Hover(HoverParams),
    Diagnostic(DocumentDiagnosticParams),
}

impl Into<InteractLspRequest> for GotoDefinitionParams {
    fn into(self) -> InteractLspRequest {
        InteractLspRequest::GotoDef(self)
    }
}
impl Into<InteractLspRequest> for HoverParams {
    fn into(self) -> InteractLspRequest {
        InteractLspRequest::Hover(self)
    }
}
impl Into<InteractLspRequest> for DocumentDiagnosticParams {
    fn into(self) -> InteractLspRequest {
        InteractLspRequest::Diagnostic(self)
    }
}

#[derive(Debug, Clone)]
pub enum InteractLspNotification {
    Save(DidSaveTextDocumentParams),
    Change(DidChangeTextDocumentParams),
    Open(DidOpenTextDocumentParams),
}
impl Into<InteractLspNotification> for DidSaveTextDocumentParams {
    fn into(self) -> InteractLspNotification {
        InteractLspNotification::Save(self)
    }
}
impl Into<InteractLspNotification> for DidChangeTextDocumentParams {
    fn into(self) -> InteractLspNotification {
        InteractLspNotification::Change(self)
    }
}
impl Into<InteractLspNotification> for DidOpenTextDocumentParams {
    fn into(self) -> InteractLspNotification {
        InteractLspNotification::Open(self)
    }
}
