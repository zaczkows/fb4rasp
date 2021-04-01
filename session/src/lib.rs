pub mod ssh_session;
pub mod ws_session;

pub use ssh_session::SshSession;
pub use ws_session::WsSession;
pub type Error = ssh2::Error;
