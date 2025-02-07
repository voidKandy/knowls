use derived_deref::{Deref, DerefMut};
use knowls::{other_err, rpc::RpcMessage, MainResult};
use seraphic::packet::{PacketRead, TcpPacket};
use std::{
    collections::VecDeque,
    net::SocketAddr,
    sync::{atomic::AtomicBool, Arc},
    time::{Duration, Instant},
};
use tokio::net::TcpListener;
use tokio::{
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
    sync::RwLock,
    task::JoinHandle,
};

#[derive(Debug, Deref, DerefMut)]
pub struct RpcListener(pub TcpListener);

impl From<TcpListener> for RpcListener {
    fn from(value: TcpListener) -> Self {
        Self(value)
    }
}

impl RpcListener {
    /// Awaits an incoming connection and converts it to a `ConnectionInfo`
    /// This will spawn the thread that handles the individual connection
    pub async fn accept(&mut self) -> MainResult<ConnectionInfo> {
        match self.0.accept().await {
            Ok((stream, addr)) => {
                tracing::warn!("connected: {addr:#?}");
                Ok(ConnectionThreadState::spawn_handle(stream))
            }
            Err(e) => {
                let msg = format!("couldn't accept connection: {e:?}");
                tracing::warn!(msg);
                return Err(other_err!("{msg}"));
            }
        }
    }
}

type SharedMessageQueue = Arc<RwLock<VecDeque<RpcMessage>>>;
#[derive(Debug)]
/// Information of connection stored on connection handler thread
pub struct ConnectionThreadState {
    read: OwnedReadHalf,
    write: OwnedWriteHalf,
    incoming: SharedMessageQueue,
    incoming_pending: Arc<AtomicBool>,
    outbound: SharedMessageQueue,
    outbound_pending: Arc<AtomicBool>,
}

#[derive(Debug)]
/// Information of connection stored on application main thread
pub(super) struct ConnectionInfo {
    pub handle: JoinHandle<()>,
    pub established: Instant,
    pub incoming: SharedMessageQueue,
    pub incoming_pending: Arc<AtomicBool>,
    pub outbound: SharedMessageQueue,
    pub outbound_pending: Arc<AtomicBool>,
}

impl ConnectionInfo {
    pub async fn push_outbound(&mut self, message: RpcMessage) {
        self.outbound.write().await.push_back(message);
        if !self
            .outbound_pending
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            self.outbound_pending
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

impl ConnectionThreadState {
    const THREAD_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

    fn new(
        stream: TcpStream,
        incoming: SharedMessageQueue,
        outbound: SharedMessageQueue,
        incoming_pending: Arc<AtomicBool>,
        outbound_pending: Arc<AtomicBool>,
    ) -> Self {
        let (read, write) = stream.into_split();
        Self {
            read,
            write,
            incoming,
            incoming_pending,
            outbound,
            outbound_pending,
        }
    }

    async fn push_incoming(&mut self, message: RpcMessage) {
        self.incoming.write().await.push_back(message);
        if !self
            .incoming_pending
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            self.incoming_pending
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// Spins up handle and returns connection info
    pub fn spawn_handle(stream: TcpStream) -> ConnectionInfo {
        let established = Instant::now();
        let incoming = Arc::new(RwLock::new(VecDeque::new()));
        let outbound = Arc::new(RwLock::new(VecDeque::new()));
        let incoming_pending = Arc::new(AtomicBool::new(false));
        let outbound_pending = Arc::new(AtomicBool::new(false));

        let thread_incoming = Arc::clone(&incoming);
        let thread_outbound = Arc::clone(&outbound);
        let thread_incoming_pending = Arc::clone(&incoming_pending);
        let thread_outbound_pending = Arc::clone(&outbound_pending);

        let handle = tokio::spawn(async move {
            tracing::warn!("spawned connection handle");
            let mut thread_state = ConnectionThreadState::new(
                stream,
                thread_incoming,
                thread_outbound,
                thread_incoming_pending,
                thread_outbound_pending,
            );

            loop {
                tokio::select! {
                    Ok(_) = thread_state.read.readable() => {
                        match TcpPacket::async_read(&mut thread_state.read).await {
                            Err(err) => {
                                panic!("connection thread encountered error when reading: {err:#?}");
                            },
                            Ok(PacketRead::Empty) =>{},
                            Ok(PacketRead::Disconnected) =>{
                                tracing::warn!("connection with client closed");
                                break;
                            },
                            Ok(PacketRead::Message(msg)) =>{
                                thread_state.push_incoming(msg).await;
                            },
                        }
                    },
                    Ok(_) = thread_state.write.writable() => {
                        if thread_state.outbound_pending.load(std::sync::atomic::Ordering::Relaxed) {
                            let mut w = thread_state.outbound.write().await;
                            while let Some(msg) = w.pop_front() {
                                TcpPacket::async_write(&mut thread_state.write, &msg).await.expect("failed to write msg");
                            }
                            thread_state.outbound_pending.store(false, std::sync::atomic::Ordering::Relaxed);
                        }
                    },

                }
            }
        });

        ConnectionInfo {
            incoming,
            established,
            outbound,
            handle,
            outbound_pending,
            incoming_pending,
        }
    }
}
