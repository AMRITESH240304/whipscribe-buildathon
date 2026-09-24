use std::time::{Duration, Instant};

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tokio::sync::Mutex;
use url::Url;

use crate::{err, net_err, oauth::Loopback, secret, Result, NOT_CONNECTED};

const CLIENT_ID: &str = env!("GOOGLE_CLIENT_ID");
const CLIENT_SECRET: &str = env!("GOOGLE_CLIENT_SECRET");
const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const REVOKE_URL: &str = "https://oauth2.googleapis.com/revoke";
const EVENTS_URL: &str = "https://www.googleapis.com/calendar/v3/calendars/primary/events";
const SCOPE: &str = "https://www.googleapis.com/auth/calendar.events.readonly";
const KEYRING_USER: &str = "google-refresh-token";

pub struct GoogleState {
    http: reqwest::Client,
    access: Mutex<Option<(String, Instant)>>,
}

impl GoogleState {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(20))
                .build()
                .expect("http client"),
            access: Mutex::new(None),
        }
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
    refresh_token: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEvent {
    id: String,
    title: String,
    start: String,
    end: String,
    all_day: bool,
    meeting_url: Option<String>,
    attendees: usize,
}

#[derive(Deserialize)]
struct EventList {
    #[serde(default)]
    items: Vec<GoogleEvent>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleEvent {
    id: String,
    summary: Option<String>,
    start: GoogleTime,
    end: GoogleTime,
    hangout_link: Option<String>,
    location: Option<String>,
    conference_data: Option<ConferenceData>,
    #[serde(default)]
    attendees: Vec<Attendee>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleTime {
    date_time: Option<String>,
    date: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConferenceData {
    #[serde(default)]
    entry_points: Vec<EntryPoint>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntryPoint {
    entry_point_type: String,
    uri: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Attendee {
    #[serde(rename = "self", default)]
    is_self: bool,
    response_status: Option<String>,
}

impl GoogleEvent {
    fn declined_by_me(&self) -> bool {
        self.attendees
            .iter()
            .any(|a| a.is_self && a.response_status.as_deref() == Some("declined"))
    }

    fn meeting_url(&self) -> Option<String> {
        self.conference_data
            .iter()
            .flat_map(|c| &c.entry_points)
            .find(|e| e.entry_point_type == "video")
            .map(|e| e.uri.clone())
            .or_else(|| self.hangout_link.clone())
            .or_else(|| self.location.clone().filter(|l| l.starts_with("https://")))
    }

    fn into_event(self) -> CalendarEvent {
        let all_day = self.start.date_time.is_none();
        CalendarEvent {
            meeting_url: self.meeting_url(),
            attendees: self.attendees.len(),
            title: self.summary.unwrap_or_else(|| "(No title)".into()),
            start: self.start.date_time.or(self.start.date).unwrap_or_default(),
            end: self.end.date_time.or(self.end.date).unwrap_or_default(),
            all_day,
            id: self.id,
        }
    }
}

fn keyring() -> Result<keyring::Entry> {
    secret(KEYRING_USER)
}

fn stored_refresh_token() -> Option<String> {
    keyring().ok()?.get_password().ok()
}

fn cache(slot: &mut Option<(String, Instant)>, token: &TokenResponse) -> String {
    let expires = Instant::now() + Duration::from_secs(token.expires_in.saturating_sub(60));
    *slot = Some((token.access_token.clone(), expires));
    token.access_token.clone()
}

async fn access_token(state: &GoogleState) -> Result<String> {
    let mut access = state.access.lock().await;
    if let Some((token, expires)) = access.as_ref() {
        if Instant::now() < *expires {
            return Ok(token.clone());
        }
    }

    let refresh = stored_refresh_token().ok_or(NOT_CONNECTED)?;
    let res = state
        .http
        .post(TOKEN_URL)
        .form(&[
            ("client_id", CLIENT_ID),
            ("client_secret", CLIENT_SECRET),
            ("refresh_token", &refresh),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await
        .map_err(net_err)?;

    // Revoked or expired grant: forget it so the UI asks to reconnect.
    if matches!(
        res.status(),
        StatusCode::BAD_REQUEST | StatusCode::UNAUTHORIZED
    ) {
        let _ = keyring()?.delete_credential();
        return Err(NOT_CONNECTED.into());
    }
    let token: TokenResponse = res
        .error_for_status()
        .map_err(err)?
        .json()
        .await
        .map_err(err)?;
    Ok(cache(&mut access, &token))
}

#[tauri::command]
pub fn google_status() -> bool {
    stored_refresh_token().is_some()
}

#[tauri::command]
pub async fn google_connect(app: AppHandle, state: State<'_, GoogleState>) -> Result<()> {
    let sign_in = Loopback::bind().await?;
    let auth_url = Url::parse_with_params(
        AUTH_URL,
        &[
            ("client_id", CLIENT_ID),
            ("redirect_uri", &sign_in.redirect_uri),
            ("response_type", "code"),
            ("scope", SCOPE),
            ("code_challenge", &sign_in.challenge),
            ("code_challenge_method", "S256"),
            ("state", &sign_in.state),
            ("access_type", "offline"),
            ("prompt", "consent"),
        ],
    )
    .map_err(err)?;
    let code = sign_in.authorize(&app, &auth_url).await?;

    let token: TokenResponse = state
        .http
        .post(TOKEN_URL)
        .form(&[
            ("client_id", CLIENT_ID),
            ("client_secret", CLIENT_SECRET),
            ("code", &code),
            ("code_verifier", &sign_in.verifier),
            ("grant_type", "authorization_code"),
            ("redirect_uri", &sign_in.redirect_uri),
        ])
        .send()
        .await
        .map_err(net_err)?
        .error_for_status()
        .map_err(err)?
        .json()
        .await
        .map_err(err)?;

    let refresh = token
        .refresh_token
        .as_deref()
        .ok_or("Google did not return a refresh token.")?;
    keyring()?.set_password(refresh).map_err(err)?;
    cache(&mut *state.access.lock().await, &token);
    Ok(())
}

#[tauri::command]
pub async fn google_disconnect(state: State<'_, GoogleState>) -> Result<()> {
    if let Some(token) = stored_refresh_token() {
        let _ = state
            .http
            .post(REVOKE_URL)
            .form(&[("token", token.as_str())])
            .send()
            .await;
    }
    *state.access.lock().await = None;
    match keyring()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(err(e)),
    }
}

#[tauri::command]
pub async fn list_events(
    state: State<'_, GoogleState>,
    time_min: String,
    time_max: String,
) -> Result<Vec<CalendarEvent>> {
    let token = access_token(&state).await?;
    let list: EventList = state
        .http
        .get(EVENTS_URL)
        .bearer_auth(token)
        .query(&[
            ("timeMin", time_min.as_str()),
            ("timeMax", time_max.as_str()),
            ("singleEvents", "true"),
            ("orderBy", "startTime"),
            ("maxResults", "50"),
        ])
        .send()
        .await
        .map_err(net_err)?
        .error_for_status()
        .map_err(err)?
        .json()
        .await
        .map_err(err)?;

    Ok(list
        .items
        .into_iter()
        .filter(|e| !e.declined_by_me())
        .map(GoogleEvent::into_event)
        .collect())
}
