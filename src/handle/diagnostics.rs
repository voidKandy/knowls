use crate::{state::LspState, MainResult};
use lsp_types::{PublishDiagnosticsParams, Uri};
use tokio::sync::RwLockWriteGuard;

#[derive(Debug, Clone)]
pub enum LspDiagnostic {
    ClearDiagnostics(Uri),
    Publish(PublishDiagnosticsParams),
}

impl LspDiagnostic {
    #[tracing::instrument(name = "diagnosing document", skip_all)]
    pub fn diagnose_document<'i>(
        uri: Uri,
        w: &'i mut RwLockWriteGuard<'_, LspState<'static>>,
    ) -> MainResult<LspDiagnostic> {
        let mut all_diagnostics = vec![];
        let tokens = w.documents.get(&uri).cloned().unwrap();

        for (my_pos, parsed_comment) in tokens.into_iter() {
            let doc_info = crate::interact::execution::InteractDocumentInfo {
                tokens: &tokens,
                my_pos,
                uri: &uri,
            };
            all_diagnostics.append(&mut parsed_comment.get_diagnostics(w, doc_info));
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
