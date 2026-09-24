use std::time::Duration;

use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, State};
use url::Url;

use crate::{err, net_err, oauth::Loopback, secret, Result, NOT_CONNECTED};

const BASE: &str = "https://whipscribe.com";
const RESOURCE: &str = "https://whipscribe.com/mcp";
const KEYRING_USER: &str = "whipscribe-mcp-token";

pub struct Mcp {
    http: reqwest::Client,
}

#[derive(Deserialize)]
struct Registered {
    client_id: String,
}

#[derive(Deserialize)]
struct Token {
    access_token: String,
}

fn token() -> Result<String> {
    secret(KEYRING_USER)?
        .get_password()
        .map_err(|_| NOT_CONNECTED.to_string())
}

impl Mcp {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .expect("http client"),
        }
    }

    // Calls one MCP tool and returns its JSON result. The server is stateless,
    // so no initialize handshake or session id is needed per call.
    async fn call(&self, tool: &str, arguments: Value) -> Result<Value> {
        let res = self
            .http
            .post(RESOURCE)
            .bearer_auth(token()?)
            .header("Accept", "application/json, text/event-stream")
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": { "name": tool, "arguments": arguments },
            }))
            .send()
            .await
            .map_err(net_err)?;
        if res.status() == StatusCode::UNAUTHORIZED {
            let _ = secret(KEYRING_USER)?.delete_credential();
            return Err(NOT_CONNECTED.into());
        }
        let body = res
            .error_for_status()
            .map_err(err)?
            .text()
            .await
            .map_err(err)?;

        // Streamable HTTP replies with a single SSE `data:` line, or plain JSON.
        let message = body
            .lines()
            .find_map(|line| line.strip_prefix("data: "))
            .unwrap_or(&body);
        let rpc: Value = serde_json::from_str(message).map_err(err)?;
        if let Some(e) = rpc.get("error") {
            return Err(e["message"].as_str().unwrap_or("WhipScribe error.").into());
        }
        let text = rpc["result"]["content"][0]["text"]
            .as_str()
            .ok_or("Unexpected reply from WhipScribe.")?;
        let result: Value = serde_json::from_str(text).map_err(err)?;
        if result["ok"] == false {
            let message = result["error"]["message"].as_str();
            return Err(message.unwrap_or("WhipScribe couldn't do that.").into());
        }
        Ok(result)
    }
}

#[tauri::command]
pub fn whipscribe_signed_in() -> bool {
    token().is_ok()
}

// OAuth 2.1 as the MCP spec describes it: dynamic client registration,
// then an authorization-code flow with PKCE through the browser.
#[tauri::command]
pub async fn whipscribe_sign_in(app: AppHandle, state: State<'_, Mcp>) -> Result<()> {
    let sign_in = Loopback::bind().await?;
    let client: Registered = state
        .http
        .post(format!("{BASE}/oauth/register"))
        .json(&json!({
            "client_name": "WhipScribe Recorder",
            "redirect_uris": [sign_in.redirect_uri],
            "grant_types": ["authorization_code"],
            "response_types": ["code"],
            "token_endpoint_auth_method": "none",
            "scope": "mcp",
        }))
        .send()
        .await
        .map_err(net_err)?
        .error_for_status()
        .map_err(err)?
        .json()
        .await
        .map_err(err)?;

    let auth_url = Url::parse_with_params(
        &format!("{BASE}/oauth/authorize"),
        &[
            ("client_id", client.client_id.as_str()),
            ("redirect_uri", &sign_in.redirect_uri),
            ("response_type", "code"),
            ("scope", "mcp"),
            ("code_challenge", &sign_in.challenge),
            ("code_challenge_method", "S256"),
            ("state", &sign_in.state),
            ("resource", RESOURCE),
        ],
    )
    .map_err(err)?;
    let code = sign_in.authorize(&app, &auth_url).await?;

    let token: Value = state
        .http
        .post(format!("{BASE}/oauth/token"))
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", &sign_in.redirect_uri),
            ("client_id", &client.client_id),
            ("code_verifier", &sign_in.verifier),
            ("resource", RESOURCE),
        ])
        .send()
        .await
        .map_err(net_err)?
        .error_for_status()
        .map_err(err)?
        .json()
        .await
        .map_err(err)?;
    let token: Token = serde_json::from_value(token).map_err(err)?;
    secret(KEYRING_USER)?
        .set_password(&token.access_token)
        .map_err(err)
}

// The only tools the UI may call; the rest of the server stays out of reach.
const LIBRARY_TOOLS: &[&str] = &[
    "library_list_folders",
    "library_create_folder",
    "library_get_folder",
    "library_add_item",
    "library_trash_item",
];

#[tauri::command]
pub async fn library(state: State<'_, Mcp>, tool: String, arguments: Value) -> Result<Value> {
    if !LIBRARY_TOOLS.contains(&tool.as_str()) {
        return Err(format!("{tool} isn't available."));
    }
    state.call(&tool, arguments).await
}
