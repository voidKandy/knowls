use std::time::Duration;

use crate::handle::buffer_operations::BufferOpChannelError;

use super::{BufferOpChannelResult, BufferOperation};
use futures::{self, Stream};
use lsp_types::{
    WorkDoneProgress, WorkDoneProgressBegin, WorkDoneProgressEnd, WorkDoneProgressReport,
};
use tracing::{debug, warn};

pub type BufferOpChannel = Box<dyn Stream<Item = BufferOpChannelResult<BufferOpChannelStatus>>>;

pub type BufferOpChannelReceiver =
    tokio::sync::mpsc::Receiver<BufferOpChannelResult<BufferOpChannelStatus>>;
#[derive(Clone, Debug)]
pub struct BufferOpChannelSender(
    tokio::sync::mpsc::Sender<BufferOpChannelResult<BufferOpChannelStatus>>,
);

#[derive(Debug)]
pub struct BufferOpChannelHandler {
    pub sender: BufferOpChannelSender,
    pub receiver: BufferOpChannelReceiver,
}

#[derive(Debug)]
pub enum BufferOpChannelStatus {
    Working(BufferOperation),
    Finished,
}

impl From<BufferOperation> for BufferOpChannelStatus {
    fn from(value: BufferOperation) -> Self {
        Self::Working(value)
    }
}

impl BufferOpChannelHandler {
    pub fn new() -> Self {
        let channel =
            tokio::sync::mpsc::channel::<BufferOpChannelResult<BufferOpChannelStatus>>(55);
        Self {
            sender: BufferOpChannelSender(channel.0),
            receiver: channel.1,
        }
    }
}

impl BufferOpChannelSender {
    pub async fn send_operation(&mut self, op: BufferOperation) -> BufferOpChannelResult<()> {
        debug!("sending buffer operation to client: {:?}", op);
        let result =
            tokio::time::timeout(Duration::from_millis(1000), self.0.send(Ok(op.into()))).await;

        match result {
            Ok(send_result) => send_result.map_err(|err| err.into()),
            Err(_) => Err(BufferOpChannelError::Timeout),
        }
    }

    pub async fn send_finish(&self) -> BufferOpChannelResult<()> {
        let result = tokio::time::timeout(
            Duration::from_millis(1000),
            self.0.send(Ok(BufferOpChannelStatus::Finished)),
        )
        .await;

        match result {
            Ok(send_result) => send_result.map_err(|err| err.into()),
            Err(_) => Err(BufferOpChannelError::Timeout),
        }
    }

    pub async fn start_work_done(&mut self, message: Option<&str>) -> BufferOpChannelResult<()> {
        let work_done = WorkDoneProgressBegin {
            message: message.and_then(|s| Some(s.to_string())),
            ..Default::default()
        };
        self.send_operation(BufferOperation::WorkDone(WorkDoneProgress::Begin(
            work_done,
        )))
        .await?;
        Ok(())
    }

    #[tracing::instrument(name = "send work done report", skip_all)]
    pub async fn send_work_done_report(
        &mut self,
        message: Option<&str>,
        percentage: Option<u32>,
    ) -> BufferOpChannelResult<()> {
        let work_done = WorkDoneProgressReport {
            message: message.and_then(|s| Some(s.to_string())),
            percentage,
            ..Default::default()
        };

        warn!("report: {work_done:#?}");

        self.send_operation(BufferOperation::WorkDone(WorkDoneProgress::Report(
            work_done,
        )))
        .await?;

        Ok(())
    }

    pub async fn send_work_done_end(&mut self, message: Option<&str>) -> BufferOpChannelResult<()> {
        let work_done = WorkDoneProgressEnd {
            message: message.and_then(|s| Some(s.to_string())),
            ..Default::default()
        };
        self.send_operation(BufferOperation::WorkDone(WorkDoneProgress::End(work_done)))
            .await?;
        Ok(())
    }
}

mod tests {
    #[allow(unused)]
    use super::*;
    #[tokio::test]
    async fn buffer_op_stream_works() {
        let mut ops_stream_handler = BufferOpChannelHandler::new();
        let s_clone = ops_stream_handler.sender.clone();

        let _: tokio::task::JoinHandle<BufferOpChannelResult<()>> = tokio::spawn(async move {
            for _ in 0..5 {
                s_clone
                    .clone()
                    .send_operation(BufferOperation::ShowMessage(lsp_types::ShowMessageParams {
                        typ: lsp_types::MessageType::INFO,
                        message: "".to_owned(),
                    }))
                    .await
                    .unwrap();
            }
            ops_stream_handler.sender.send_finish().await.unwrap();
            Ok(())
        });

        let mut counter = 0;
        while let Ok(BufferOpChannelStatus::Working(buffer_op)) =
            ops_stream_handler.receiver.recv().await.unwrap()
        {
            counter += 1;
            println!("{:?}", buffer_op)
        }
        assert_eq!(5, counter);
    }
}
