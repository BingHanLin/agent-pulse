use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookEvent {
    pub session_id: String,
    pub hook_event_name: String,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub tool_name: Option<String>,
    #[serde(default)]
    pub notification_type: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
}

pub struct WebhookServer {
    port: u16,
}

const HTTP_200: &str = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nOK";
const HTTP_400: &str = "HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nBad Request";
const HTTP_405: &str = "HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 18\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nMethod Not Allowed";

impl WebhookServer {
    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn start(sender: mpsc::UnboundedSender<HookEvent>) -> Result<Arc<Self>, String> {
        let mut bound_port = 0u16;
        let mut listener_opt = None;

        for port in 19280..=19289 {
            match TcpListener::bind(format!("127.0.0.1:{}", port)).await {
                Ok(l) => {
                    bound_port = port;
                    listener_opt = Some(l);
                    break;
                }
                Err(_) => continue,
            }
        }

        let listener = listener_opt.ok_or_else(|| {
            "Failed to bind to any port in range 19280-19289".to_string()
        })?;

        println!("Webhook server listening on 127.0.0.1:{}", bound_port);

        let server = Arc::new(WebhookServer { port: bound_port });

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((mut stream, _)) => {
                        let sender = sender.clone();
                        tokio::spawn(async move {
                            let mut buf = vec![0u8; 65536];
                            let n = match tokio::time::timeout(
                                std::time::Duration::from_secs(5),
                                stream.read(&mut buf),
                            )
                            .await
                            {
                                Ok(Ok(n)) => n,
                                _ => return,
                            };

                            let request = String::from_utf8_lossy(&buf[..n]);

                            // Check if it's a POST request
                            if !request.starts_with("POST") {
                                let _ = stream.write_all(HTTP_405.as_bytes()).await;
                                return;
                            }

                            // Extract body (after \r\n\r\n)
                            let body = match request.find("\r\n\r\n") {
                                Some(pos) => &request[pos + 4..],
                                None => {
                                    let _ = stream.write_all(HTTP_400.as_bytes()).await;
                                    return;
                                }
                            };

                            // If we have Content-Length but body is incomplete, read more
                            let content_length = request
                                .lines()
                                .find(|l| l.to_lowercase().starts_with("content-length:"))
                                .and_then(|l| l.split(':').nth(1))
                                .and_then(|v| v.trim().parse::<usize>().ok())
                                .unwrap_or(0);

                            let full_body = if body.len() < content_length {
                                let mut body_buf = body.as_bytes().to_vec();
                                let remaining = content_length - body.len();
                                let mut remaining_buf = vec![0u8; remaining];
                                match tokio::time::timeout(
                                    std::time::Duration::from_secs(5),
                                    stream.read_exact(&mut remaining_buf),
                                )
                                .await
                                {
                                    Ok(Ok(_)) => {
                                        body_buf.extend_from_slice(&remaining_buf);
                                        String::from_utf8_lossy(&body_buf).to_string()
                                    }
                                    _ => {
                                        let _ = stream.write_all(HTTP_400.as_bytes()).await;
                                        return;
                                    }
                                }
                            } else {
                                body.to_string()
                            };

                            match serde_json::from_str::<HookEvent>(&full_body) {
                                Ok(event) => {
                                    let _ = sender.send(event);
                                    let _ = stream.write_all(HTTP_200.as_bytes()).await;
                                }
                                Err(e) => {
                                    eprintln!("Failed to parse hook event: {}", e);
                                    let _ = stream.write_all(HTTP_400.as_bytes()).await;
                                }
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("Failed to accept connection: {}", e);
                    }
                }
            }
        });

        Ok(server)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_full_event() {
        let json = r#"{
            "session_id": "test-123",
            "hook_event_name": "SessionStart",
            "cwd": "/home/user/project",
            "tool_name": "Read"
        }"#;
        let event: HookEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.session_id, "test-123");
        assert_eq!(event.hook_event_name, "SessionStart");
        assert_eq!(event.cwd, Some("/home/user/project".to_string()));
        assert_eq!(event.tool_name, Some("Read".to_string()));
    }

    #[test]
    fn test_deserialize_minimal_event() {
        let json = r#"{
            "session_id": "test-456",
            "hook_event_name": "Stop"
        }"#;
        let event: HookEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.session_id, "test-456");
        assert_eq!(event.hook_event_name, "Stop");
        assert_eq!(event.cwd, None);
        assert_eq!(event.tool_name, None);
    }
}
