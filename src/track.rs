use std::collections::HashMap;
use std::sync::Arc;

pub struct Track {
    pub path: String,
    pub data: Arc<Vec<u8>>,
}

pub struct Playlist {
    tracks: HashMap<u8, Track>,
}