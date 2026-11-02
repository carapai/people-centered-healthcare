//! An axum listener that hands over connections once their TLS handshake completes.
//!
//! Handshakes run in their own tasks, each bounded by [`HANDSHAKE`], so a slow or silent
//! client never holds up the next connection. Accept errors (too many open files, a reset) are
//! retried after a pause, as axum's own TCP listener does.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_rustls::TlsAcceptor;
use tokio_rustls::server::TlsStream;

/// The longest a client may take over its handshake.
const HANDSHAKE: Duration = Duration::from_secs(10);

/// TLS connections on a bound TCP listener.
pub(crate) struct TlsListener {
    ready: mpsc::Receiver<(TlsStream<TcpStream>, SocketAddr)>,
    local: SocketAddr,
    accepting: JoinHandle<()>,
}

impl TlsListener {
    /// Accepts on `tcp`, completing each handshake with `config`.
    pub(crate) fn new(tcp: TcpListener, config: rustls::ServerConfig) -> std::io::Result<Self> {
        let local = tcp.local_addr()?;
        let acceptor = TlsAcceptor::from(Arc::new(config));
        let (sender, ready) = mpsc::channel(64);
        let accepting = tokio::spawn(async move {
            loop {
                let (stream, peer) = match tcp.accept().await {
                    Ok(accepted) => accepted,
                    Err(_) => {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        continue;
                    }
                };
                let (acceptor, sender) = (acceptor.clone(), sender.clone());
                tokio::spawn(async move {
                    // A failed or timed-out handshake just drops the connection.
                    if let Ok(Ok(tls)) =
                        tokio::time::timeout(HANDSHAKE, acceptor.accept(stream)).await
                    {
                        let _ = sender.send((tls, peer)).await;
                    }
                });
            }
        });
        Ok(Self {
            ready,
            local,
            accepting,
        })
    }
}

impl Drop for TlsListener {
    /// Stops accepting, so the port is released when serving ends.
    fn drop(&mut self) {
        self.accepting.abort();
    }
}

impl axum::serve::Listener for TlsListener {
    type Io = TlsStream<TcpStream>;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        match self.ready.recv().await {
            Some(connection) => connection,
            // The accept task ended, which it only does when aborted.
            None => std::future::pending().await,
        }
    }

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        Ok(self.local)
    }
}
