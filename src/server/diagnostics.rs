use crate::{
    interact::{logic::LspMessageInteract, InteractVar},
    state::LspState,
    MainResult,
};
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
            tracing::warn!("getting diagnostic for {:#?}", parsed_comment.interact);
            let doc_info = crate::interact::execution::InteractDocumentInfo {
                tokens: &tokens,
                my_pos,
                uri: &uri,
            };

            if let Some(interact) = parsed_comment.interact.as_ref() {
                if let Some(ref mut diagnostics) = match interact.variant {
                    InteractVar::Agent(i) => i
                        .get_execution_args(w, &parsed_comment, doc_info, &interact.parsed_args)
                        .and_then(|args| Some(i.diagnostics(args))),
                    InteractVar::DB(i) => i
                        .get_execution_args(w, &parsed_comment, doc_info, &interact.parsed_args)
                        .and_then(|args| Some(i.diagnostics(args))),
                } {
                    tracing::warn!("adding diagnostics: {diagnostics:#?}");
                    all_diagnostics.append(diagnostics);
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
