use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

use crate::audio::AudioLevel;
use crate::input::VirtualInput;
use crate::media::MediaController;
use crate::settings::Settings;

#[derive(Clone)]
pub struct AppState {
    pub settings: Arc<RwLock<Settings>>,
    pub input: Arc<VirtualInput>,
    pub media: Arc<MediaController>,
    pub audio_tx: broadcast::Sender<AudioLevel>,
}
