//! A line-based TCP chat client on a Tokio worker shard.
use eventful_rs::*;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpStream, tcp::OwnedWriteHalf},
    sync::Mutex,
};

declare_shard!(pub Net, runtime = tokio);
use_shard!(shard = Net);

/// What happens on the connection.
#[events]
pub trait ChatEvents {
    /// The connection is up.
    fn connected(&self, peer: String) {}
    /// The peer sent a line.
    fn received(&self, line: String) {}
    /// A line was written to the peer.
    fn sent(&self, line: String) {}
    /// The connection failed or closed.
    fn disconnected(&self, reason: String) {}
}

/// The socket lives on the Tokio shard; the UI talks to it through events and
/// actions.
#[eventful(ChatEvents)]
pub struct Client {
    /// Write half, once connected. A Tokio mutex, because writes await.
    writer: Mutex<Option<OwnedWriteHalf>>,
}

impl Client {
    pub async fn new() -> Result<ShardRcHandle<Self>, InvokeError> {
        Self::spawn(|| Self {
            writer: Mutex::new(None),
            events: Default::default(),
        })
        .await
    }

    /// Read lines until the peer closes, as a task owned by the connection.
    fn read(this: &ShardRc<Self>, reader: tokio::net::tcp::OwnedReadHalf) {
        let task = ShardRc::spawn_owned(this, |weak| async move {
            let mut lines = BufReader::new(reader).lines();
            loop {
                let next = lines.next_line().await;
                let Some(this) = weak.upgrade() else { return };
                match next {
                    Ok(Some(line)) => this.events.received().emit(line),
                    Ok(None) => return this.events.disconnected().emit("closed by peer".into()),
                    Err(err) => return this.events.disconnected().emit(err.to_string()),
                }
            }
        });
        if let Err(err) = task {
            this.events.disconnected().emit(err.to_string());
        }
    }
}

#[asynchronize(pub)]
impl Client {
    /// Connect to `addr` and start reading.
    #[action]
    pub async fn open(&self, addr: String) {
        let stream = match TcpStream::connect(&addr).await {
            Ok(stream) => stream,
            Err(err) => return self.events.disconnected().emit(format!("{addr}: {err}")),
        };
        let (reader, writer) = stream.into_split();
        *self.writer.lock().await = Some(writer);
        match ShardRc::try_from_ref(self) {
            Ok(this) => Self::read(&this, reader),
            Err(err) => return self.events.disconnected().emit(err.to_string()),
        }
        self.events.connected().emit(addr);
    }

    /// Send one line to the peer.
    #[action]
    pub async fn send(&self, line: String) {
        let mut writer = self.writer.lock().await;
        let Some(stream) = writer.as_mut() else {
            return self.events.disconnected().emit("not connected".into());
        };
        let written = async {
            stream.write_all(line.as_bytes()).await?;
            stream.write_all(b"\n").await
        };
        match written.await {
            Ok(()) => self.events.sent().emit(line),
            Err(err) => {
                *writer = None;
                self.events.disconnected().emit(err.to_string());
            }
        }
    }
}
