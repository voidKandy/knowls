use crate::{
    interact::{
        id::{human_readable_int, InteractID},
        lexer::Token,
    },
    state::LspState,
};
use anyhow::Ok;
use lsp_types::{Diagnostic, DiagnosticSeverity, PublishDiagnosticsParams, Uri};

#[derive(Debug, Clone)]
pub enum LspDiagnostic {
    ClearDiagnostics(Uri),
    Publish(PublishDiagnosticsParams),
}

impl LspDiagnostic {
    #[tracing::instrument(name = "diagnosing document", skip_all)]
    pub fn diagnose_document(uri: Uri, store: &mut LspState) -> anyhow::Result<LspDiagnostic> {
        let mut all_diagnostics = vec![];
        let tokens = store.documents.get(&uri).unwrap();
        // if let Some(burns) = store.burns.read_burns_on_doc(&uri) {
        //     debug!("got burns on doc: {:?}", burns);
        //     for burn in burns {
        //         all_diagnostics.append(&mut Self::burn_diagnostics(&burn, &text)?);
        //     }
        // }

        //  is.is_empty() {
        //     debug!("clearing diagnostics");
        //     return Ok(Self::ClearDiagnostics(uri));
        // } else {
        //     debug!("publishing diagnostics: {:?}", all_diagnostics);
        //     return Ok(Self::Publish(PublishDiagnosticsParams {
        //         uri,
        //         diagnostics: all_diagnostics,
        //         version: None,
        //     }));
        //
        // }

        let severity = Some(DiagnosticSeverity::HINT);
        for token in tokens.as_ref() {
            if let Token::Comment(comment) = token {
                if let Some(int) = comment.try_get_interact_integer().ok() {
                    all_diagnostics.push(Diagnostic {
                        range: comment.range,
                        severity,
                        message: format!("{}", human_readable_int(int)),
                        ..Default::default()
                    });
                }
            }
        }

        let lsp_diagnostic = match all_diagnostics.is_empty() {
            true => LspDiagnostic::ClearDiagnostics(uri),
            false => {
                let params = PublishDiagnosticsParams {
                    uri,
                    diagnostics: all_diagnostics,
                    version: None,
                };
                LspDiagnostic::Publish(params)
            }
        };

        Ok(lsp_diagnostic)

        // #[tracing::instrument(name = "checking for diagnostics for burn", skip(text))]
        // fn burn_diagnostics(burn: &Burn, text: &str) -> anyhow::Result<Vec<Diagnostic>> {
        //     let severity = Some(DiagnosticSeverity::HINT);
        // let mut all_diagnostics = vec![];

        // if let Some(message) = burn.activation.trigger_diagnostic() {
        //     debug!("burn has trigger diagnostic: {}", message);
        //     match burn.activation.range() {
        //         OneOf::Left(range) => {
        //             all_diagnostics.push(Diagnostic {
        //                 range: range.as_ref().to_owned(),
        //                 severity,
        //                 message,
        //                 ..Default::default()
        //             });
        //         }
        //         OneOf::Right((start_range, end_range)) => {
        //             all_diagnostics.push(Diagnostic {
        //                 range: start_range.as_ref().to_owned(),
        //                 severity,
        //                 message: message.clone(),
        //                 ..Default::default()
        //             });
        //             all_diagnostics.push(Diagnostic {
        //                 range: end_range.as_ref().to_owned(),
        //                 severity,
        //                 message,
        //                 ..Default::default()
        //             });
        //         }
        //     }
        // }
        //
        // if let Some(message) = burn.activation.user_input_diagnostic() {
        //     debug!("burn has user input diagnostic: {}", message);
        //     if let Activation::Single(single) = &burn.activation {
        //         if let Some(slices) = parsing::slices_after_pattern(text, &single.trigger_pattern())
        //         {
        //             for slice in slices {
        //                 all_diagnostics.push(Diagnostic {
        //                     range: slice.range,
        //                     severity,
        //                     message: message.clone(),
        //                     ..Default::default()
        //                 });
        //             }
        //         }
        //     }
        // }

        // Ok(all_diagnostics)
    }
}
