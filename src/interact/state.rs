use super::{
    logic::{InteractArg, InteractVar, IntoInteractVar, LspMessageInteract},
    parsing::{comments::ParsedComment, tokens::TokenVec},
};
use crate::{handle::error::HandleResult, state::LspState};
use lsp_server::RequestId;
use tokio::sync::RwLockWriteGuard;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StateInteract {
    Database,
}

impl StateInteract {
    const DATABASE: char = '%';
}

impl<'i> IntoInteractVar<'i, ()> for StateInteract {
    fn into_interact_var(&self) -> InteractVar {
        InteractVar::State(*self)
    }
    fn n_expected_args(&self) -> usize {
        1
    }
    fn get_execution_args(
        &self,
        w: &'i mut RwLockWriteGuard<'_, LspState<'static>>,
        interact_comment: &'i ParsedComment<'_>,
        doc_tokens: &'i TokenVec,
        my_pos_in_tokens: usize,
        args: &Vec<InteractArg>,
    ) -> Option<()> {
        Some(())
    }
}

impl TryFrom<char> for StateInteract {
    type Error = anyhow::Error;
    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            Self::DATABASE => Ok(Self::Database),
            _ => Err(anyhow::anyhow!(
                "could not create agent interact from {value}"
            )),
        }
    }
}

impl LspMessageInteract<()> for StateInteract {
    async fn execute_notification(
        &self,
        args: (),
        noti: impl Into<super::InteractLspNotification>,
        sender: &mut crate::handle::buffer_operations::BufferOpChannelSender,
    ) -> HandleResult<()> {
        Ok(())
    }

    async fn execute_request(
        &self,
        args: (),
        rq_id: RequestId,
        params: impl Into<super::InteractLspRequest>,
        sender: &mut crate::handle::buffer_operations::BufferOpChannelSender,
    ) -> HandleResult<()> {
        Ok(())
    }
}
