use tokio::io::{AsyncReadExt, AsyncWriteExt};
pub struct Endpoints;
impl Drop for Endpoints {
    fn drop(&mut self) {
        std::env::remove_var("LF_TEST_CLAUDE_USAGE_URL");
        std::env::remove_var("LF_TEST_CLAUDE_TOKEN_URL");
        std::env::remove_var("LF_TEST_CLAUDE_PROFILE_URL");
    }
}

pub async fn serve(
    responses: Vec<(u16, String)>,
    before_response: impl Fn(usize) + Send + 'static,
) -> (Endpoints, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    std::env::set_var("LF_TEST_CLAUDE_USAGE_URL", &endpoint);
    std::env::set_var("LF_TEST_CLAUDE_TOKEN_URL", &endpoint);
    std::env::set_var("LF_TEST_CLAUDE_PROFILE_URL", &endpoint);
    let task = tokio::spawn(async move {
        for (index, (status, body)) in responses.into_iter().enumerate() {
            let (mut stream, _) =
                tokio::time::timeout(std::time::Duration::from_secs(5), listener.accept())
                    .await
                    .unwrap()
                    .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0; 4096];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let count = stream.read(&mut buffer).await.unwrap();
                assert_ne!(count, 0);
                request.extend_from_slice(&buffer[..count]);
            }
            before_response(index);
            let response = format!("HTTP/1.1 {status} Result\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            stream.write_all(response.as_bytes()).await.unwrap();
        }
    });
    (Endpoints, task)
}
