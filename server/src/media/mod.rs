use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use zbus::zvariant::Value;

#[derive(Serialize, Clone)]
pub struct PlayerInfo {
    pub id: String,
    pub name: String,
}

#[derive(Serialize, Clone)]
pub struct MediaState {
    pub status: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub art_url: Option<String>,
    pub position_ms: Option<i64>,
    pub duration_ms: Option<i64>,
    pub volume: Option<f64>,
    pub shuffle: Option<bool>,
    pub repeat: Option<String>,
    pub player_id: Option<String>,
}

#[derive(Clone)]
pub struct MediaController {
    connection: zbus::Connection,
    selected_player: Arc<RwLock<Option<String>>>,
}

impl MediaController {
    pub async fn new() -> Result<Self, zbus::Error> {
        let connection = zbus::Connection::session().await?;
        Ok(Self {
            connection,
            selected_player: Arc::new(RwLock::new(None)),
        })
    }

    pub async fn list_players(&self) -> Result<Vec<PlayerInfo>, zbus::Error> {
        let dbus_proxy = zbus::fdo::DBusProxy::new(&self.connection).await?;
        let names = dbus_proxy.list_names().await?;

        let mut players = Vec::new();

        // Check if the KDE browser integration host is running (provides richer
        // MPRIS data than Firefox/Zen's built-in MPRIS). If so, skip the
        // browser's built-in entries which have ".instance_" in the name.
        let has_browser_integration = names.iter().any(|n| {
            let s = n.as_str();
            s == "org.mpris.MediaPlayer2.telemax"
                || s == "org.mpris.MediaPlayer2.plasma-browser-integration"
        });

        for name in names {
            let name_str = name.as_str();
            if !name_str.starts_with("org.mpris.MediaPlayer2.") {
                continue;
            }
            // Drop browser built-in MPRIS when the integration host is active
            if has_browser_integration && name_str.contains(".instance_") {
                continue;
            }
            let display_name = self.get_player_display_name(name_str).await;
            players.push(PlayerInfo {
                id: name_str.to_string(),
                name: display_name,
            });
        }

        Ok(players)
    }

    /// Build a display name from the player's current media title or identity.
    /// Prefers "Title — App" (e.g., "House | Disney+ — Zen") when media is playing,
    /// falls back to Identity (e.g., "Spotify"), then the bus name suffix.
    async fn get_player_display_name(&self, bus_name: &str) -> String {
        let fallback = bus_name
            .strip_prefix("org.mpris.MediaPlayer2.")
            .unwrap_or(bus_name)
            .to_string();

        let builder = (|| -> Result<_, zbus::Error> {
            let b = zbus::proxy::Builder::<'_, zbus::Proxy>::new(&self.connection)
                .destination(bus_name)?
                .path("/org/mpris/MediaPlayer2")?
                .interface("org.mpris.MediaPlayer2.Player")?;
            Ok(b)
        })();
        let proxy = match builder {
            Ok(b) => match b.build().await {
                Ok(p) => p,
                Err(_) => return fallback,
            },
            Err(_) => return fallback,
        };

        // Try to get the current media title
        let title: Option<String> = proxy
            .get_property::<HashMap<String, Value>>("Metadata")
            .await
            .ok()
            .and_then(|meta| extract_string(&meta, "xesam:title"))
            .filter(|t| !t.is_empty());

        // Get app identity from the root interface
        let identity: Option<String> = {
            let root_proxy = (|| -> Result<_, zbus::Error> {
                let b = zbus::proxy::Builder::<'_, zbus::Proxy>::new(&self.connection)
                    .destination(bus_name)?
                    .path("/org/mpris/MediaPlayer2")?
                    .interface("org.mpris.MediaPlayer2")?;
                Ok(b)
            })();
            match root_proxy {
                Ok(b) => match b.build().await {
                    Ok(p) => p.get_property::<String>("Identity").await.ok(),
                    Err(_) => None,
                },
                Err(_) => None,
            }
        };

        match (title, identity) {
            (Some(t), Some(id)) => format!("{t} — {id}"),
            (Some(t), None) => t,
            (None, Some(id)) => id,
            (None, None) => fallback,
        }
    }

    pub async fn select_player(&self, id: &str) {
        let mut selected = self.selected_player.write().await;
        *selected = Some(id.to_string());
    }

    async fn get_selected_player(&self) -> Result<String, String> {
        let selected = self.selected_player.read().await;
        selected
            .clone()
            .ok_or_else(|| "No player selected".to_string())
    }

    async fn player_proxy(&self) -> Result<zbus::Proxy<'_>, String> {
        let player_name = self.get_selected_player().await?;
        zbus::proxy::Builder::new(&self.connection)
            .destination(player_name)
            .map_err(|e| format!("Invalid destination: {e}"))?
            .path("/org/mpris/MediaPlayer2")
            .map_err(|e| format!("Invalid path: {e}"))?
            .interface("org.mpris.MediaPlayer2.Player")
            .map_err(|e| format!("Invalid interface: {e}"))?
            .build()
            .await
            .map_err(|e| format!("Failed to create proxy: {e}"))
    }

    pub async fn get_state(&self) -> Result<MediaState, String> {
        // Auto-select the first available player if none is selected
        {
            let selected = self.selected_player.read().await;
            if selected.is_none() {
                drop(selected);
                if let Ok(players) = self.list_players().await {
                    if let Some(first) = players.first() {
                        self.select_player(&first.id).await;
                    }
                }
            }
        }

        let proxy = self.player_proxy().await?;
        let player_id = self.get_selected_player().await.ok();

        let status: String = proxy
            .get_property("PlaybackStatus")
            .await
            .map_err(|e| format!("Failed to get PlaybackStatus: {e}"))?;

        let metadata: HashMap<String, Value> = proxy
            .get_property("Metadata")
            .await
            .unwrap_or_default();

        let title = extract_string(&metadata, "xesam:title");
        let artist = extract_string_list(&metadata, "xesam:artist");
        let album = extract_string(&metadata, "xesam:album");
        let art_url_raw = extract_string(&metadata, "mpris:artUrl");
        let length_us = extract_i64(&metadata, "mpris:length");

        let art_url = art_url_raw.as_ref().map(|_| "/api/media/art".to_string());

        let position_ms: Option<i64> = proxy
            .get_property::<i64>("Position")
            .await
            .ok()
            .map(|us| us / 1000);

        let duration_ms = length_us.map(|us| us / 1000);

        let volume: Option<f64> = proxy.get_property("Volume").await.ok();

        let shuffle: Option<bool> = proxy.get_property("Shuffle").await.ok();

        let repeat: Option<String> = proxy.get_property("LoopStatus").await.ok();

        Ok(MediaState {
            status,
            title,
            artist,
            album,
            art_url,
            position_ms,
            duration_ms,
            volume,
            shuffle,
            repeat,
            player_id,
        })
    }

    pub async fn play(&self) -> Result<(), String> {
        let proxy = self.player_proxy().await?;
        proxy
            .call_method("Play", &())
            .await
            .map_err(|e| format!("Play failed: {e}"))?;
        Ok(())
    }

    pub async fn pause(&self) -> Result<(), String> {
        let proxy = self.player_proxy().await?;
        proxy
            .call_method("Pause", &())
            .await
            .map_err(|e| format!("Pause failed: {e}"))?;
        Ok(())
    }

    pub async fn next(&self) -> Result<(), String> {
        let proxy = self.player_proxy().await?;
        proxy
            .call_method("Next", &())
            .await
            .map_err(|e| format!("Next failed: {e}"))?;
        Ok(())
    }

    pub async fn previous(&self) -> Result<(), String> {
        let proxy = self.player_proxy().await?;
        proxy
            .call_method("Previous", &())
            .await
            .map_err(|e| format!("Previous failed: {e}"))?;
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), String> {
        let proxy = self.player_proxy().await?;
        proxy
            .call_method("Stop", &())
            .await
            .map_err(|e| format!("Stop failed: {e}"))?;
        Ok(())
    }

    pub async fn seek(&self, position_ms: i64) -> Result<(), String> {
        let proxy = self.player_proxy().await?;
        let position_us = position_ms * 1000;

        // Get the track ID from metadata for SetPosition
        let metadata: HashMap<String, Value> = proxy
            .get_property("Metadata")
            .await
            .unwrap_or_default();

        let track_id = extract_string(&metadata, "mpris:trackid")
            .unwrap_or_else(|| "/org/mpris/MediaPlayer2/TrackList/NoTrack".to_string());

        let track_path = zbus::zvariant::ObjectPath::try_from(track_id.as_str())
            .map_err(|e| format!("Invalid track path: {e}"))?;

        proxy
            .call_method("SetPosition", &(track_path, position_us))
            .await
            .map_err(|e| format!("SetPosition failed: {e}"))?;
        Ok(())
    }

    pub async fn set_volume(&self, volume: f64) -> Result<(), String> {
        let proxy = self.player_proxy().await?;
        proxy
            .set_property("Volume", volume)
            .await
            .map_err(|e| format!("Set volume failed: {e}"))?;
        Ok(())
    }

    pub async fn toggle_shuffle(&self) -> Result<(), String> {
        let proxy = self.player_proxy().await?;
        let current: bool = proxy
            .get_property("Shuffle")
            .await
            .map_err(|e| format!("Get shuffle failed: {e}"))?;
        proxy
            .set_property("Shuffle", !current)
            .await
            .map_err(|e| format!("Set shuffle failed: {e}"))?;
        Ok(())
    }

    pub async fn cycle_repeat(&self) -> Result<(), String> {
        let proxy = self.player_proxy().await?;
        let current: String = proxy
            .get_property("LoopStatus")
            .await
            .map_err(|e| format!("Get loop status failed: {e}"))?;
        let next = match current.as_str() {
            "None" => "Track",
            "Track" => "Playlist",
            "Playlist" => "None",
            _ => "None",
        };
        proxy
            .set_property("LoopStatus", next)
            .await
            .map_err(|e| format!("Set loop status failed: {e}"))?;
        Ok(())
    }

    pub async fn get_art_bytes(&self) -> Result<Option<Vec<u8>>, String> {
        let proxy = self.player_proxy().await?;
        let metadata: HashMap<String, Value> = proxy
            .get_property("Metadata")
            .await
            .unwrap_or_default();

        let art_url = match extract_string(&metadata, "mpris:artUrl") {
            Some(url) => url,
            None => return Ok(None),
        };

        if let Some(path) = art_url.strip_prefix("file://") {
            let bytes = tokio::fs::read(path)
                .await
                .map_err(|e| format!("Failed to read art file: {e}"))?;
            Ok(Some(bytes))
        } else {
            Ok(None)
        }
    }
}

fn extract_string(metadata: &HashMap<String, Value>, key: &str) -> Option<String> {
    let value = metadata.get(key)?;
    match value {
        Value::Str(s) => Some(s.to_string()),
        _ => {
            // Try to downcast through nested Value types
            if let Ok(s) = <&str>::try_from(value) {
                Some(s.to_string())
            } else {
                None
            }
        }
    }
}

fn extract_string_list(metadata: &HashMap<String, Value>, key: &str) -> Option<String> {
    let value = metadata.get(key)?;
    match value {
        Value::Array(arr) => {
            let strings: Vec<String> = arr
                .iter()
                .filter_map(|v| match v {
                    Value::Str(s) => Some(s.to_string()),
                    _ => <&str>::try_from(v).ok().map(|s| s.to_string()),
                })
                .collect();
            if strings.is_empty() {
                None
            } else {
                Some(strings.join(", "))
            }
        }
        _ => extract_string(metadata, key),
    }
}

fn extract_i64(metadata: &HashMap<String, Value>, key: &str) -> Option<i64> {
    let value = metadata.get(key)?;
    match value {
        Value::I64(n) => Some(*n),
        Value::U64(n) => Some(*n as i64),
        Value::I32(n) => Some(*n as i64),
        Value::U32(n) => Some(*n as i64),
        _ => i64::try_from(value).ok(),
    }
}
