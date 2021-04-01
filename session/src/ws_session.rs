use futures_util::{stream::SplitSink, stream::SplitStream, SinkExt, StreamExt};
// use tokio::io::AsyncWriteExt;
use tokio_tungstenite::{tungstenite::protocol::Message, MaybeTlsStream, WebSocketStream};

/// Websocket Session
pub struct WsSession {
    // wss: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
    reader: SplitStream<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>>,
    writer: SplitSink<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>, Message>,
}

impl WsSession {
    pub async fn new(address: &str) -> Option<Self> {
        let r = tokio_tungstenite::connect_async(address).await;
        match r {
            Ok((wss, resp)) => {
                log::debug!("Connection to {} successful {:?}", address, &resp);
                let (writer, reader) = wss.split();
                Some(WsSession { reader, writer })
            }
            Err(e) => {
                log::error!(
                    "Failed to establish websocket connection with {}, due to {}",
                    address,
                    &e
                );
                None
            }
        }
    }

    pub async fn ping(&mut self) -> bool {
        match self.writer.send(tungstenite::Message::Ping(vec![])).await {
            Ok(()) => true,
            Err(e) => {
                log::debug!("Failed to send text message to websocket: {:?}", &e);
                false
            }
        }
    }

    pub async fn send_text(&mut self, what: &str) -> bool {
        match self
            .writer
            .send(tungstenite::Message::Text(what.to_string()).into())
            .await
        {
            Ok(()) => true,
            Err(e) => {
                log::debug!("Failed to send text message to websocket: {:?}", &e);
                false
            }
        }
    }

    pub async fn read_text(&mut self) -> Option<String> {
        match self.reader.next().await {
            Some(m) => match m {
                Ok(m) => match m {
                    Message::Text(t) => Some(t),
                    _ => None,
                },
                Err(e) => {
                    log::error!("Error {} while trying to read websocket message", &e);
                    None
                }
            },
            None => None,
        }
    }
}
