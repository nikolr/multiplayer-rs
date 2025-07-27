use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Track {
    pub path: String,
    pub volume: f64,
}

#[derive(Serialize, Deserialize)]
pub struct Playlist {
    pub tracks: Vec<Track>,
}