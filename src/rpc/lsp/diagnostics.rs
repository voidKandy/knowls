use crate::{
    interact::{Interact, InteractWrapper},
    knowledge::{uri_to_surreal_id, Knowledge},
    server::ServerState,
    MainResult,
};
use lsp_types::{PublishDiagnosticsParams, Uri};
use tokio::sync::RwLockReadGuard;

#[derive(Debug, Clone)]
pub enum LspDiagnostic {
    ClearDiagnostics(Uri),
    Publish(PublishDiagnosticsParams),
}

impl LspDiagnostic {
    #[tracing::instrument(name = "diagnosing document", skip_all)]
    pub fn diagnose_document<'i>(
        uri: Uri,
        r: &'i RwLockReadGuard<'_, ServerState>,
    ) -> MainResult<LspDiagnostic> {
        let mut all_diagnostics = vec![];
        if let Some(Knowledge::Document { interacts, .. }) =
            r.knowledge.get(&uri_to_surreal_id(&uri))
        {
            for interact in interacts.values() {
                tracing::warn!("interact: {interact:#?}");
                match interact {
                    InteractWrapper::Agent(i) => {
                        let ctx = i.get_read_context(r)?;
                        all_diagnostics.append(&mut i.diagnostics(ctx));
                    }
                    InteractWrapper::DB(i) => {
                        let ctx = i.get_read_context(r)?;
                        all_diagnostics.append(&mut i.diagnostics(ctx));
                    }
                }
            }
        }

        if all_diagnostics.is_empty() {
            return Ok(LspDiagnostic::ClearDiagnostics(uri));
        }

        Ok(LspDiagnostic::Publish(PublishDiagnosticsParams {
            uri,
            diagnostics: all_diagnostics,
            version: None,
        }))
    }
}
