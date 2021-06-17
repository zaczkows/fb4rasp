use std::future::Future;

use futures_util::{stream::SplitSink, stream::SplitStream, SinkExt, StreamExt};
use tokio::time::error::Elapsed;
use tokio_tungstenite::{tungstenite::protocol::Message, MaybeTlsStream, WebSocketStream};
use tungstenite::client::IntoClientRequest;

#[derive(Debug)]
pub enum WsSessionError {
    Closed,
    Fatal(String),
    InvalidFormat,
    TooMuchData,
    Timeout(String),
}

impl From<tungstenite::error::Error> for WsSessionError {
    fn from(e: tungstenite::error::Error) -> Self {
        match e {
            tungstenite::error::Error::ConnectionClosed
            | tungstenite::error::Error::AlreadyClosed => WsSessionError::Closed,
            _ => WsSessionError::Fatal(format!("{}", e)),
        }
    }
}

impl From<Elapsed> for WsSessionError {
    fn from(e: Elapsed) -> Self {
        WsSessionError::Timeout(format!("{}", e))
    }
}

/// Websocket Session
pub struct WsSession {
    reader: SplitStream<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>>,
    writer: SplitSink<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>, Message>,
    timeout: std::time::Duration,
}

impl WsSession {
    pub async fn new<R>(address: R) -> Result<Self, WsSessionError>
    where
        R: IntoClientRequest + Unpin + std::fmt::Display,
    {
        let addr = format!("{}", &address);
        let (wss, resp) = tokio_tungstenite::connect_async(address).await?;
        log::debug!("Connection to {} successful {:?}", addr, &resp);
        let (writer, reader) = wss.split();
        Ok(WsSession {
            reader,
            writer,
            timeout: std::time::Duration::from_secs(5),
        })
    }

    pub fn set_timeout(&mut self, timeout: std::time::Duration) {
        self.timeout = timeout;
    }

    pub fn get_timeout(&self) -> std::time::Duration {
        self.timeout
    }

    pub async fn ping(&mut self, what: &[u8]) -> Result<(), WsSessionError> {
        if what.len() > 125 {
            return Err(WsSessionError::TooMuchData);
        }

        tokio::time::timeout(
            self.timeout,
            self.writer.send(tungstenite::Message::Ping(what.to_vec())),
        )
        .await??;
        Ok(())
    }

    pub async fn send_text(&mut self, what: &str) -> Result<(), WsSessionError> {
        tokio::time::timeout(
            self.timeout,
            self.writer
                .send(tungstenite::Message::Text(what.to_string())),
        )
        .await??;
        Ok(())
    }

    pub async fn send_binary(&mut self, what: &[u8]) -> Result<(), WsSessionError> {
        tokio::time::timeout(
            self.timeout,
            self.writer
                .send(tungstenite::Message::Binary(what.to_vec())),
        )
        .await??;
        Ok(())
    }

    pub async fn read_text(&mut self) -> Result<Option<String>, WsSessionError> {
        match tokio::time::timeout(self.timeout, self.reader.next()).await? {
            Some(msg) => match msg? {
                Message::Text(t) => Ok(Some(t)),
                _ => Err(WsSessionError::InvalidFormat),
            },
            None => Ok(None),
        }
    }

    pub async fn on_text<Fut, F>(
        self,
        f: F,
    ) -> futures_util::stream::Then<
        SplitStream<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>>,
        Fut,
        F,
    >
    where
        F: FnMut(Result<Message, tungstenite::Error>) -> Fut,
        Fut: Future,
    {
        self.reader.then(f)
    }
}
