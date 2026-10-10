//! One HTTP/1 request, with the attachment fence ending at the last socket write.
//! Waiting for response headers is not dispatch: command routes send them only
//! after execution. The connection is polled by the fenced writer until every
//! request byte is written, then moved to the response collector. No background
//! writer can outlive a failed or timed-out dispatch.
use std::future::{poll_fn, Future};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

use anyhow::{anyhow, ensure, Result};
use axum::body::Body;
use axum::response::Response;
use bytes::Bytes;
use http_body_util::Full;
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;

pub(super) async fn send(
    url: reqwest::Url,
    directory: String,
    payload: Vec<u8>,
) -> Result<impl Future<Output = Result<Response>> + Send> {
    ensure!(url.scheme() == "http", "OpenCode connection requires HTTP");
    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("Missing server host"))?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| anyhow!("Missing server port"))?;
    let stream = TcpStream::connect((host, port)).await?;
    let sent = Arc::new(AtomicBool::new(false));
    let io = RequestSocket {
        stream,
        sent: sent.clone(),
        delimiter: 0,
        head_finished: false,
        remaining: payload.len(),
    };
    let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(io)).await?;
    let request = hyper::Request::post(url.as_str())
        .header("host", format!("{host}:{port}"))
        .header("x-opencode-directory", directory)
        .header("content-type", "application/json")
        .header("content-length", payload.len())
        .body(Full::new(Bytes::from(payload)))?;
    let mut response = Box::pin(sender.send_request(request));
    let mut connection = Box::pin(connection);
    let mut received = None;
    let mut connection_finished = false;
    poll_fn(|cx| {
        // Queue the request before driving its socket. Neither future is spawned
        // until dispatch has completed under the caller's attachment fence.
        if received.is_none() {
            if let Poll::Ready(result) = response.as_mut().poll(cx) {
                received = Some(result);
            }
        }
        let state = connection.as_mut().poll(cx);
        connection_finished = state.is_ready();
        if sent.load(Ordering::Acquire) {
            return Poll::Ready(Ok(()));
        }
        match state {
            Poll::Ready(Err(error)) => Poll::Ready(Err(anyhow!(error))),
            Poll::Ready(Ok(())) => {
                Poll::Ready(Err(anyhow!("OpenCode closed before dispatch completed")))
            }
            Poll::Pending => Poll::Pending,
        }
    })
    .await?;
    Ok(async move {
        if !connection_finished {
            tokio::spawn(async move {
                if let Err(error) = connection.await {
                    tracing::debug!(%error, "OpenCode response connection closed");
                }
            });
        }
        let response = match received {
            Some(result) => result?,
            None => response.await?,
        };
        let (parts, body) = response.into_parts();
        Ok(Response::from_parts(parts, Body::new(body)))
    })
}

/// Count the actual fixed-length HTTP request bytes, not body production or a
/// flush (HTTP may flush headers before writing the body).
struct RequestSocket {
    stream: TcpStream,
    sent: Arc<AtomicBool>,
    delimiter: u32,
    head_finished: bool,
    remaining: usize,
}

impl AsyncRead for RequestSocket {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_read(cx, buffer)
    }
}

impl AsyncWrite for RequestSocket {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let count = std::task::ready!(Pin::new(&mut self.stream).poll_write(cx, buffer))?;
        let mut body_start = 0;
        if !self.head_finished {
            body_start = count;
            for (index, byte) in buffer[..count].iter().enumerate() {
                self.delimiter = (self.delimiter << 8) | u32::from(*byte);
                if self.delimiter == u32::from_be_bytes(*b"\r\n\r\n") {
                    self.head_finished = true;
                    body_start = index + 1;
                    break;
                }
            }
        }
        self.remaining -= count - body_start;
        if self.head_finished && self.remaining == 0 {
            self.sent.store(true, Ordering::Release);
        }
        Poll::Ready(Ok(count))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use super::send;

    #[tokio::test]
    async fn response_can_close_immediately_after_dispatch() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/abort", listener.local_addr().unwrap());
        let provider = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            loop {
                request.push(socket.read_u8().await.unwrap());
                if request.ends_with(b"\r\n\r\n") {
                    break;
                }
            }
            socket
                .write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n")
                .await
                .unwrap();
        });
        let response = send(url.parse().unwrap(), "/fixture".into(), Vec::new())
            .await
            .unwrap()
            .await
            .unwrap();
        assert_eq!(response.status(), 204);
        provider.await.unwrap();
    }

    #[tokio::test]
    async fn cancelled_partial_dispatch_never_finishes_in_the_background() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/command", listener.local_addr().unwrap());
        let (drain, ready) = tokio::sync::oneshot::channel();
        let provider = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            // Withhold reads until the dispatch future has been dropped.
            ready.await.unwrap();
            let mut received = Vec::new();
            socket.read_to_end(&mut received).await.unwrap();
            received
        });
        let length = 16 * 1024 * 1024;
        let result = tokio::time::timeout(
            Duration::from_millis(100),
            send(url.parse().unwrap(), "/fixture".into(), vec![b'x'; length]),
        )
        .await;
        assert!(result.is_err(), "unread socket accepted the whole request");
        drain.send(()).unwrap();
        let received = tokio::time::timeout(Duration::from_secs(2), provider)
            .await
            .unwrap()
            .unwrap();
        assert!(received.windows(4).any(|bytes| bytes == b"\r\n\r\n"));
        assert!(
            received.len() < length,
            "cancelled writer continued dispatch"
        );
    }
}
