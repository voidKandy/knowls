use crate::{
    database::models::{Knowledge, KnowledgeId},
    state::{SharedState, StateReadGuard},
};
use color_eyre::owo_colors::OwoColorize;
use knowls::{rpc::LspMessage, MainResult};
use lsp_server::ResponseError;
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse,
    CompletionTriggerKind, DiagnosticSeverity, Hover, HoverParams, Position, Range,
    TextDocumentPositionParams,
};
use std::{collections::HashMap, marker::PhantomData};
use surrealdb::RecordId;

#[derive(Debug)]
pub struct LspHandler {
    /// not ideal that this requires cloning
    // knowledge_context: HashMap<RecordId, Knowledge>,
    pub documents: HashMap<lsp_types::Uri, String>,
    received_shutdown: bool,
}

impl LspHandler {
    pub fn new() -> Self {
        Self {
            // knowledge_context: r.knowledge.
            documents: HashMap::new(),
            received_shutdown: false,
        }
    }

    /// I don't know why this end of the connection would ever receive responses
    pub fn handle_lsp_response(
        &mut self,
        _state: &SharedState,
        _res: lsp_server::Response,
    ) -> MainResult<Option<LspMessage>> {
        Ok(None)
    }

    pub fn handle_lsp_request(
        &mut self,
        state: &SharedState,
        req: lsp_server::Request,
    ) -> MainResult<Option<LspMessage>> {
        tracing::warn!("handle lsp req: {req:#?}");
        if self.received_shutdown {
            let response = lsp_server::Response {
                result: None,
                error: Some(ResponseError {
                    // invalid request error code
                    code: -32600,
                    message: format!("Shutdown request has been received, {req:#?} is invalid"),
                    data: None,
                }),
                id: req.id,
            };
            return Ok(Some(lsp_server::Message::Response(response).into()));
        }

        match req.method.as_str() {
            "textDocument/definition" => {}
            "textDocument/hover" => {
                let r = state.try_read().expect("failed to read lock state");
                let params = serde_json::from_value::<HoverParams>(req.params)?;
                let pos = params.text_document_position_params.position;

                if let Some(content) = self
                    .documents
                    .get(&params.text_document_position_params.text_document.uri)
                {
                    let line = content
                        .lines()
                        .nth(pos.line as usize)
                        .expect("should have gotten line");
                    let (char_before, char_after) = (
                        pos.character
                            .checked_sub(1)
                            .and_then(|i| line.chars().nth(i as usize)),
                        line.chars().nth(pos.character as usize),
                    );

                    // match (char_before, char_after) {
                    //     (Some(bc), Some(ac)) => {
                    //         if bc == '@' {
                    //             if let Some(kn) = r.knowledge.values().find(|k| k.lsp_char == ac) {
                    //                 let hover = Hover {
                    //                     contents: lsp_types::HoverContents::Markup(
                    //                         lsp_types::MarkupContent {
                    //                             kind: lsp_types::MarkupKind::Markdown,
                    //                             value: kn.content.to_owned(),
                    //                         },
                    //                     ),
                    //                     range: None,
                    //                 };

                    //                 let json = serde_json::to_value(hover)
                    //                     .expect("could not serialize hover");
                    //                 let msg = lsp_server::Message::Response(lsp_server::Response {
                    //                     id: req.id,
                    //                     result: Some(json),
                    //                     error: None,
                    //                 });
                    //                 return Ok(Some(msg.into()));
                    //             }
                    //         }
                    //     }
                    //     _ => {}
                    // }
                }
                return Ok(None);
            }
            "textDocument/diagnostic" => {}
            "textDocument/completion" => {
                tracing::error!("GOT COMPLETION REQUEST");
                let completion: CompletionParams = serde_json::from_value(req.params)?;
                match completion.context {
                    Some(CompletionContext {
                        trigger_kind: CompletionTriggerKind::TRIGGER_CHARACTER,
                        ..
                    })
                    | Some(CompletionContext {
                        trigger_kind: CompletionTriggerKind::INVOKED,
                        ..
                    }) => {
                        let r = state.try_read().expect("failed to read lock state");
                        if let Some(items) = self.completions(&r, completion.text_document_position)
                        {
                            let response = CompletionResponse::Array(items);
                            let json =
                                serde_json::to_value(response).expect("could not serialize hover");
                            let msg = lsp_server::Message::Response(lsp_server::Response {
                                id: req.id,
                                result: Some(json),
                                error: None,
                            });
                            return Ok(Some(msg.into()));
                        } else {
                            tracing::warn!("EMPTY RESULTS OF COMPLETION");
                        }
                    }
                    _ => {
                        tracing::error!("unhandled completion context: {:?}", completion.context);
                    }
                }
            }
            "shutdown" => {
                let response = lsp_server::Response {
                    id: req.id,
                    result: None,
                    error: None,
                };
                self.received_shutdown = true;
                return Ok(Some(lsp_server::Message::Response(response).into()));
            }
            m => {
                tracing::warn!("unhandled request method: {m:#?}");
            }
        }
        Ok(None)
    }

    pub fn handle_lsp_notification(
        &mut self,
        state: &SharedState,
        noti: lsp_server::Notification,
    ) -> MainResult<Option<LspMessage>> {
        tracing::warn!("handle lsp noti: {noti:#?}");
        match noti.method.as_str() {
            "textDocument/didChange" => {
                let params =
                    serde_json::from_value::<lsp_types::DidOpenTextDocumentParams>(noti.params)?;
                self.documents
                    .insert(params.text_document.uri, params.text_document.text);
            }
            "textDocument/didSave" => {
                let params =
                    serde_json::from_value::<lsp_types::DidSaveTextDocumentParams>(noti.params)?;
                self.documents.insert(
                    params.text_document.uri.clone(),
                    params.text.clone().unwrap(),
                );
                let diagnostic =
                    self.diagnose_document(params.text_document.uri, params.text.unwrap());
                let params = serde_json::to_value(diagnostic).unwrap();
                let msg = lsp_server::Message::Notification(lsp_server::Notification {
                    method: "textDocument/publishDiagnostics".to_string(),
                    params,
                });
                tracing::warn!("returning: {msg:#?}");
                return Ok(Some(msg.into()));
            }
            "textDocument/didOpen" => {
                let params =
                    serde_json::from_value::<lsp_types::DidOpenTextDocumentParams>(noti.params)?;
                self.documents
                    .insert(params.text_document.uri, params.text_document.text);
            }
            m => {
                tracing::warn!("unhandled notification: {m:#?}");
            }
        }
        Ok(None)
    }

    /// Defines how a completion item is parsed out of a line that has
    /// *already* been marked as viable (as a comment)
    // pub trait CompletionParsingContext {
    //     const PRE: &'static str;
    //     const POST: &'static str;
    //     fn parse_from_line(
    //
    // &mut self,&self, str: &str) -> Option<String> {
    //         let mut buffer = String::new();
    //         if let Some(pre_pos) = str.find(Self::PRE) {}
    //     }
    // }

    fn completions(
        &mut self,
        state: &StateReadGuard,
        // ctx: impl CompletionParsingContext,
        position_params: TextDocumentPositionParams,
    ) -> Option<Vec<CompletionItem>> {
        let all_possible_comp_values = state
            .knowledge
            .values()
            .map(|k| k.kid.to_string())
            .collect::<Vec<String>>();
        // this should be configurable
        let trigger_characters = "@@$";
        let doc = self.documents.get(&position_params.text_document.uri)?;
        let line = doc.lines().nth(position_params.position.line as usize)?;
        if let Some(pos) = line.find(trigger_characters) {
            let first_char_pos = pos + trigger_characters.len();
            let slice_between_trigger_and_cursor = line
                .chars()
                .skip(first_char_pos)
                .take(position_params.position.character as usize - first_char_pos)
                .collect::<String>();

            tracing::warn!("seeing which values start with: {slice_between_trigger_and_cursor}");

            let items = all_possible_comp_values
                .into_iter()
                .filter(|s| s.starts_with(&slice_between_trigger_and_cursor))
                .map(|label| CompletionItem {
                    label,
                    kind: Some(CompletionItemKind::REFERENCE),
                    ..Default::default()
                })
                .collect::<Vec<CompletionItem>>();
            if !items.is_empty() {
                return Some(items);
            }
        }
        None
    }

    /// Currently just puts diagnostic on any 'word' that starts with a @
    fn diagnose_document(
        &mut self,
        uri: lsp_types::Uri,
        str: String,
    ) -> lsp_types::PublishDiagnosticsParams {
        let mut diagnostics = vec![];

        let mut current_range = Option::<Range>::None;
        let mut current_word = Option::<String>::None;
        for (i, line) in str.lines().enumerate() {
            for (k, ch) in line.char_indices() {
                match (current_range, ch) {
                    (None, '@') => {
                        let range = Range {
                            start: Position {
                                line: i as u32,
                                character: k as u32,
                            },
                            end: Position {
                                line: 0,
                                character: 0,
                            },
                        };
                        current_range = Some(range);
                        match current_word {
                            Some(ref mut w) => w.push(ch),
                            None => current_word = Some(ch.to_string()),
                        }
                    }
                    (Some(ref mut r), ch) => {
                        if ch.is_whitespace() {
                            r.end = Position {
                                line: i as u32,
                                character: k as u32,
                            }
                        } else {
                            match current_word {
                                Some(ref mut w) => w.push(ch),
                                None => current_word = Some(ch.to_string()),
                            }
                        }
                    }
                    _ => {}
                }
            }

            if let Some(range) = current_range.take() {
                let diagnostic = lsp_types::Diagnostic {
                    severity: Some(DiagnosticSeverity::INFORMATION),
                    range,
                    message: current_word.take().unwrap_or("no word?".to_string()),
                    ..Default::default()
                };
                diagnostics.push(diagnostic);
            }
        }
        lsp_types::PublishDiagnosticsParams {
            diagnostics,
            uri,
            version: None,
        }
    }
}
