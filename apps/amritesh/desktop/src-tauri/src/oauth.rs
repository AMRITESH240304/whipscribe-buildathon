use std::time::Duration;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use url::Url;

use crate::{err, Result};

const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(300);

// One browser sign-in: a loopback redirect with PKCE and a CSRF state.
pub struct Loopback {
    listener: TcpListener,
    pub redirect_uri: String,
    pub verifier: String,
    pub challenge: String,
    pub state: String,
}

fn random_token() -> String {
    URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>())
}

impl Loopback {
    pub async fn bind() -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await.map_err(err)?;
        let port = listener.local_addr().map_err(err)?.port();
        let verifier = random_token();
        Ok(Self {
            listener,
            redirect_uri: format!("http://127.0.0.1:{port}"),
            challenge: URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())),
            verifier,
            state: random_token(),
        })
    }

    // Opens the browser and returns the authorization code.
    pub async fn authorize(&self, app: &AppHandle, auth_url: &Url) -> Result<String> {
        app.opener()
            .open_url(auth_url.as_str(), None::<&str>)
            .map_err(err)?;
        let code = tokio::time::timeout(SIGN_IN_TIMEOUT, self.wait_for_code())
            .await
            .map_err(|_| "Sign-in timed out. Try again.".to_string())?;
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.set_focus();
        }
        code
    }

    async fn wait_for_code(&self) -> Result<String> {
        loop {
            let (mut stream, _) = self.listener.accept().await.map_err(err)?;
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).await.map_err(err)?;
            let request = String::from_utf8_lossy(&buf[..n]);
            let Some(path) = request.split_whitespace().nth(1) else {
                continue;
            };
            let url = Url::parse(&format!("http://127.0.0.1{path}")).map_err(err)?;
            let param = |key: &str| {
                url.query_pairs()
                    .find(|(k, _)| k == key)
                    .map(|(_, v)| v.into_owned())
            };

            let result = match (param("code"), param("error")) {
                (Some(code), _) if param("state").as_deref() == Some(&self.state) => Ok(code),
                (_, Some(e)) if e == "access_denied" => Err("Sign-in was cancelled.".to_string()),
                (_, Some(e)) => Err(format!("Sign-in failed ({e}).")),
                _ => continue,
            };

            let message = if result.is_ok() {
                "You're signed in. You can close this tab and go back to WhipScribe Recorder."
            } else {
                "Sign-in didn't finish. You can close this tab and try again from the app."
            };
            let body = format!(
                "<!doctype html><meta charset=utf-8><title>WhipScribe Recorder</title>\
                 <body style=\"font:16px system-ui;padding:48px\">{message}</body>"
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes()).await;
            return result;
        }
    }
}
