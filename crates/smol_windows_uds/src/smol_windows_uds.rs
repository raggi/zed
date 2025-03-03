use smol::{prelude::*, Async};
use std::fmt;
use std::io;
use std::ops::Deref;
use std::os::windows::io::{AsRawSocket, AsSocket, BorrowedSocket};
use std::path::Path;
use std::pin::Pin;
use std::task::{Context, Poll};
use uds_windows::SocketAddr;

struct UnixListenerSocket(uds_windows::UnixListener);

impl AsSocket for UnixListenerSocket {
    fn as_socket(&self) -> BorrowedSocket<'_> {
        unsafe { BorrowedSocket::borrow_raw(self.0.as_raw_socket()) }
    }
}

pub struct UnixListener(Async<UnixListenerSocket>);

impl TryFrom<uds_windows::UnixListener> for UnixListener {
    type Error = io::Error;

    fn try_from(listener: uds_windows::UnixListener) -> Result<Self, Self::Error> {
        Ok(Self(Async::new(UnixListenerSocket(listener))?))
    }
}

impl Drop for UnixListener {
    fn drop(&mut self) {
        match self.0.get_ref().0.local_addr() {
            Ok(addr) => {
                if let Some(path) = addr.as_pathname() {
                    let _ = std::fs::remove_file(&path);
                }
            }
            Err(_) => {}
        }
    }
}

struct UnixStreamSocket(uds_windows::UnixStream);

unsafe impl async_io::IoSafe for UnixStreamSocket {}

impl AsSocket for UnixStreamSocket {
    fn as_socket(&self) -> BorrowedSocket<'_> {
        unsafe { BorrowedSocket::borrow_raw(self.0.as_raw_socket()) }
    }
}

impl std::io::Read for UnixStreamSocket {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl std::io::Write for UnixStreamSocket {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

/// A Unix domain socket stream.
///
/// A socket stream is a bidirectional connection that can be used to send and receive data.
pub struct UnixStream(Async<UnixStreamSocket>);

impl TryFrom<uds_windows::UnixStream> for UnixStream {
    type Error = io::Error;

    fn try_from(stream: uds_windows::UnixStream) -> Result<Self, Self::Error> {
        Ok(Self(Async::new(UnixStreamSocket(stream))?))
    }
}

impl Deref for UnixStream {
    type Target = uds_windows::UnixStream;

    fn deref(&self) -> &uds_windows::UnixStream {
        &self.0.get_ref().0
    }
}

/// A Unix domain socket listener.
///
/// A Unix domain socket listener is a socket that can be used to accept incoming connections.
/// It is used by binding a socket to a path and then calling the `accept` method.
impl UnixListener {
    /// Creates a new `UnixListener` bound to the specified path.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use smol_windows_uds::UnixListener;
    ///
    /// # fn main() -> std::io::Result<()> {
    /// # smol::block_on(async {
    /// let listener = UnixListener::bind("socket")?;
    /// # std::io::Result::Ok(()) })}
    /// ```
    pub fn bind<P: AsRef<Path>>(path: P) -> io::Result<UnixListener> {
        uds_windows::UnixListener::bind(path)?.try_into()
    }

    /// Accepts a new incoming connection.
    ///
    /// Returns a future that resolves to a stream and the remote peer's address.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use smol_windows_uds::UnixListener;
    ///
    /// # fn main() -> std::io::Result<()> {
    /// # smol::block_on(async {
    /// let listener = UnixListener::bind("socket")?;
    /// let (stream, addr) = listener.accept().await?;
    /// # std::io::Result::Ok(()) })}
    /// ```
    pub async fn accept(&self) -> io::Result<(UnixStream, SocketAddr)> {
        let (sock, addr) = self.0.read_with(|io| io.0.accept()).await?;
        Ok((sock.try_into()?, addr))
    }

    /// Returns the local socket address of this listener.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use smol_windows_uds::UnixListener;
    ///
    /// # fn main() -> std::io::Result<()> {
    /// # smol::block_on(async {
    /// let listener = UnixListener::bind("socket")?;
    /// let local_addr = listener.local_addr()?;
    /// # std::io::Result::Ok(()) })}
    /// ```
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.0.get_ref().0.local_addr()
    }

    /// Returns an iterator that accepts incoming connections.
    ///
    /// The iterator is infinite and will accept connections until the listener is dropped.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use smol_windows_uds::UnixListener;
    /// use futures_lite::StreamExt;
    ///
    /// # fn main() -> std::io::Result<()> {
    /// # smol::block_on(async {
    /// let listener = UnixListener::bind("socket")?;
    ///
    /// let mut incoming = listener.incoming();
    /// while let Some(stream) = incoming.next().await {
    ///     let stream = stream?;
    ///     // Handle the connection...
    /// }
    /// # std::io::Result::Ok(()) })}
    /// ```
    pub fn incoming(&self) -> Incoming<'_> {
        Incoming { listener: self }
    }
}

impl fmt::Debug for UnixListener {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut builder = f.debug_struct("UnixListener");
        if let Ok(addr) = self.local_addr() {
            builder.field("local_addr", &addr);
        }
        builder.finish()
    }
}

/// An iterator that accepts incoming Unix stream connections.
#[derive(Debug)]
pub struct Incoming<'a> {
    listener: &'a UnixListener,
}

impl<'a> futures_lite::Stream for Incoming<'a> {
    type Item = io::Result<UnixStream>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let listener = self.listener;

        let inner = &listener.0;

        match inner.poll_readable(cx) {
            Poll::Ready(Ok(())) => match inner.get_ref().0.accept() {
                Ok((stream, _)) => match UnixStream::try_from(stream) {
                    Ok(stream) => Poll::Ready(Some(Ok(stream))),
                    Err(e) => Poll::Ready(Some(Err(e))),
                },
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
                Err(e) => Poll::Ready(Some(Err(e))),
            },
            Poll::Ready(Err(e)) => Poll::Ready(Some(Err(e))),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl UnixStream {
    /// Connects to the socket at the specified path.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use smol_windows_uds::UnixStream;
    ///
    /// # fn main() -> std::io::Result<()> {
    /// # smol::block_on(async {
    /// let stream = UnixStream::connect("socket").await?;
    /// # std::io::Result::Ok(()) })}
    /// ```
    pub async fn connect<P: AsRef<Path>>(path: P) -> io::Result<UnixStream> {
        uds_windows::UnixStream::connect(path)?.try_into()
    }

    /// Returns the socket address of the local half of this connection.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use smol_windows_uds::UnixStream;
    ///
    /// # fn main() -> std::io::Result<()> {
    /// # smol::block_on(async {
    /// let stream = UnixStream::connect("socket").await?;
    /// let local_addr = stream.local_addr()?;
    /// # std::io::Result::Ok(()) })}
    /// ```
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.0.get_ref().0.local_addr()
    }

    /// Returns the socket address of the remote half of this connection.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use smol_windows_uds::UnixStream;
    ///
    /// # fn main() -> std::io::Result<()> {
    /// # smol::block_on(async {
    /// let stream = UnixStream::connect("socket").await?;
    /// let remote_addr = stream.peer_addr()?;
    /// # std::io::Result::Ok(()) })}
    /// ```
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.0.get_ref().0.peer_addr()
    }
}

impl fmt::Debug for UnixStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut builder = f.debug_struct("UnixStream");

        if let Ok(addr) = self.local_addr() {
            builder.field("local_addr", &addr);
        }

        if let Ok(addr) = self.peer_addr() {
            builder.field("peer_addr", &addr);
        }

        builder.finish()
    }
}

impl AsyncRead for UnixStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl AsyncWrite for UnixStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_close(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_lite::io;
    use smol::block_on;
    use smol::io::{AsyncReadExt, AsyncWriteExt};
    use tempfile::tempdir;

    #[test]
    fn test_unix_stream_connect() {
        block_on(async {
            let dir = tempdir().unwrap();
            let socket_path = dir.path().join("socket");

            let listener = UnixListener::bind(&socket_path).unwrap();
            // Spawn a server task
            let server = smol::spawn(async move {
                let (mut server_stream, _) = listener.accept().await.unwrap();

                // Read client message
                let mut buf = [0u8; 11];
                server_stream.read_exact(&mut buf).await.unwrap();
                assert_eq!(&buf, b"hello world");

                // Send response
                server_stream.write_all(b"goodbye").await.unwrap();
            });

            // Connect as client
            let mut client_stream = UnixStream::connect(&socket_path).await.unwrap();

            // Send message
            client_stream.write_all(b"hello world").await.unwrap();

            // Read response
            let mut buf = [0u8; 7];
            client_stream.read_exact(&mut buf).await.unwrap();
            assert_eq!(&buf, b"goodbye");

            server.await;

            Ok::<_, io::Error>(())
        })
        .unwrap();
    }

    #[test]
    fn test_unix_listener_incoming() {
        block_on(async {
            let dir = tempdir().unwrap();
            let socket_path = dir.path().join("socket");

            // Create listener
            let listener = UnixListener::bind(&socket_path).unwrap();
            let local_addr = listener.local_addr().unwrap();
            assert_eq!(local_addr.as_pathname(), Some(socket_path.as_path()));

            // Spawn server using incoming stream
            let server = smol::spawn(async move {
                let mut count = 0;
                let mut incoming = listener.incoming();

                while let Some(stream_result) = incoming.next().await {
                    let mut stream = stream_result.unwrap();

                    // Echo back the data
                    let mut buf = [0u8; 10];
                    let n = stream.read(&mut buf).await.unwrap();
                    stream.write_all(&buf[..n]).await.unwrap();

                    count += 1;
                    if count >= 2 {
                        break;
                    }
                }
            });

            // Connect two clients
            for i in 0..2 {
                let mut client = UnixStream::connect(&socket_path).await.unwrap();
                let msg = format!("client{}", i);

                client.write_all(msg.as_bytes()).await.unwrap();

                let mut buf = vec![0u8; msg.len()];
                client.read_exact(&mut buf).await.unwrap();
                assert_eq!(buf, msg.as_bytes());
            }

            server.await;

            Ok::<_, io::Error>(())
        })
        .unwrap();
    }
}
