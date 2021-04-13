use std::future::Future;

use futures_util::{stream::SplitSink, stream::SplitStream, SinkExt, StreamExt};
use tokio_tungstenite::{tungstenite::protocol::Message, MaybeTlsStream, WebSocketStream};

#[derive(Debug)]
pub enum WsSessionError {
    Closed,
    Fatal(String),
    InvalidFormat,
    TooMuchData,
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

/// Websocket Session
pub struct WsSession {
    reader: SplitStream<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>>,
    writer: SplitSink<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>, Message>,
}

impl WsSession {
    pub async fn new(address: &str) -> Result<Self, WsSessionError> {
        let (wss, resp) = tokio_tungstenite::connect_async(address).await?;
        log::debug!("Connection to {} successful {:?}", address, &resp);
        let (writer, reader) = wss.split();
        Ok(WsSession { reader, writer })
    }

    pub async fn ping(&mut self, what: &[u8]) -> Result<(), WsSessionError> {
        if what.len() > 125 {
            return Err(WsSessionError::TooMuchData);
        }

        self.writer
            .send(tungstenite::Message::Ping(what.to_vec()))
            .await?;
        Ok(())
    }

    pub async fn send_text(&mut self, what: &str) -> Result<(), WsSessionError> {
        self.writer
            .send(tungstenite::Message::Text(what.to_string()))
            .await?;
        Ok(())
    }

    pub async fn send_binary(&mut self, what: &[u8]) -> Result<(), WsSessionError> {
        self.writer
            .send(tungstenite::Message::Binary(what.to_vec()))
            .await?;
        Ok(())
    }

    pub async fn read_text(&mut self) -> Result<Option<String>, WsSessionError> {
        match self.reader.next().await {
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
