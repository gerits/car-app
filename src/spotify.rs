use rspotify::{
    prelude::*,
    AuthCodeSpotify, Credentials, OAuth,
};
use std::path::PathBuf;

pub struct SpotifyState {
    pub is_playing: bool,
    pub track_name: String,
    pub track_artist: String,
    pub progress: f32,
    pub album_art_url: Option<String>,
    pub album_art_data: Option<Vec<u8>>,
}

pub struct SpotifyClient {
    client: AuthCodeSpotify,
    http_client: reqwest::Client,
}

impl SpotifyClient {
    pub async fn init() -> Option<Self> {
        log::debug!("Checking Spotify environment variables...");
        let client_id = std::env::var("SPOTIFY_CLIENT_ID").ok();
        let client_secret = std::env::var("SPOTIFY_CLIENT_SECRET").ok();
        
        if client_id.is_none() || client_secret.is_none() {
            log::warn!("Missing SPOTIFY_CLIENT_ID or SPOTIFY_CLIENT_SECRET in environment.");
            return None;
        }

        let creds = Credentials {
            id: client_id.unwrap(),
            secret: client_secret,
        };
        log::debug!("Credentials loaded successfully.");
        
        let scopes = vec!["user-read-playback-state", "user-read-currently-playing"]
            .into_iter()
            .map(|s| s.to_string())
            .collect::<std::collections::HashSet<_>>();
            
        let redirect_uri = std::env::var("SPOTIFY_REDIRECT_URI").unwrap_or_else(|_| "http://localhost:8888/callback".to_string());
        log::debug!("Using Redirect URI: {}", redirect_uri);
        
        let oauth = OAuth {
            scopes,
            redirect_uri,
            ..Default::default()
        };
        
        let mut spotify = AuthCodeSpotify::new(creds, oauth);
        
        // Use a persistent token cache
        spotify.config.token_cached = true;
        spotify.config.cache_path = PathBuf::from(".spotify_token_cache.json");

        // Attempt to refresh the token from cache first
        if let Ok(token) = spotify.read_token_cache(true).await {
            if let Some(t) = token {
                log::debug!("Found cached Spotify token.");
                *spotify.get_token().lock().await.unwrap() = Some(t);
            }
        }

        // If still no token, prompt for it
        let is_none = {
            let token_arc = spotify.get_token();
            let token_lock = token_arc.lock().await.unwrap();
            token_lock.is_none()
        };

        if is_none {
            let url = spotify.get_authorize_url(false).ok()?;
            log::info!("--- SPOTIFY AUTHENTICATION REQUIRED ---");
            log::info!("1. Open this URL in your browser: {}", url);
            log::info!("2. Log in and authorize the app.");
            log::info!("3. You will be redirected to a URL (likely localhost:8888).");
            log::info!("4. Copy the FULL URL from your browser's address bar and paste it here:");
            
            match spotify.prompt_for_token(&url).await {
                Ok(_) => log::info!("Spotify authentication successful!"),
                Err(e) => {
                    log::error!("Spotify authentication failed: {}", e);
                    return None;
                }
            }
        }

        Some(Self {
            client: spotify,
            http_client: reqwest::Client::new(),
        })
    }

    pub async fn get_current_playback(&self) -> Option<SpotifyState> {
        // Refresh token if needed
        let _ = self.client.refresh_token().await;
        
        // We fetch the raw JSON and parse it manually to avoid the 'untagged enum' bugs in rspotify's models.
        let token_arc = self.client.get_token();
        let token = token_arc.lock().await.unwrap();
        let access_token = token.as_ref()?.access_token.clone();
        drop(token);

        let url = "https://api.spotify.com/v1/me/player/currently-playing";
        let response = self.http_client
            .get(url)
            .bearer_auth(access_token)
            .send()
            .await
            .ok()?;

        if response.status() == reqwest::StatusCode::NO_CONTENT {
            log::debug!("Spotify returned 204 (nothing playing).");
            return None;
        }

        let json: serde_json::Value = response.json().await.ok()?;
        
        let is_playing = json["is_playing"].as_bool().unwrap_or(false);
        let progress_ms = json["progress_ms"].as_f64().unwrap_or(0.0) as f32;
        
        let item = &json["item"];
        if item.is_null() {
            return None;
        }

        let track_name = item["name"].as_str().unwrap_or("Unknown").to_string();
        let duration_ms = item["duration_ms"].as_f64().unwrap_or(0.0) as f32;
        
        let artists = item["artists"].as_array();
        let track_artist = artists.map(|a| {
            a.iter()
                .filter_map(|artist| artist["name"].as_str())
                .collect::<Vec<_>>()
                .join(", ")
        }).unwrap_or_else(|| "Unknown Artist".to_string());

        let album_art_url = item["album"]["images"]
            .as_array()
            .and_then(|imgs| imgs.first())
            .and_then(|img| img["url"].as_str())
            .map(|s| s.to_string());

        let mut progress = 0.0;
        if duration_ms > 0.0 {
            progress = (progress_ms / duration_ms).clamp(0.0, 1.0);
        }

        Some(SpotifyState {
            is_playing,
            track_name,
            track_artist,
            progress,
            album_art_url,
            album_art_data: None,
        })
    }

    pub async fn fetch_album_art(&self, url: &str) -> Option<Vec<u8>> {
        let response = self.http_client.get(url).send().await.ok()?;
        let bytes = response.bytes().await.ok()?;
        Some(bytes.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_init_missing_env_vars() {
        // Remove env vars to ensure we hit the early return path
        unsafe {
            std::env::remove_var("SPOTIFY_CLIENT_ID");
            std::env::remove_var("SPOTIFY_CLIENT_SECRET");
        }
        
        let client = SpotifyClient::init().await;
        assert!(client.is_none());
    }
    
    #[test]
    fn test_spotify_state_struct() {
        let state = SpotifyState {
            is_playing: true,
            track_name: "Test Track".to_string(),
            track_artist: "Test Artist".to_string(),
            progress: 0.5,
            album_art_url: Some("http://example.com/art.png".to_string()),
            album_art_data: None,
        };
        assert_eq!(state.is_playing, true);
        assert_eq!(state.track_name, "Test Track");
        assert_eq!(state.track_artist, "Test Artist");
        assert_eq!(state.progress, 0.5);
        assert_eq!(state.album_art_url, Some("http://example.com/art.png".to_string()));
    }
}
