use serde::{Deserialize, Serialize};
use crate::host::host::Host;

#[derive(Serialize, Deserialize, Debug)]
pub struct Settings {
    pub fade_in_duration: u64,
    pub fade_out_duration: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            fade_in_duration: 1000,
            fade_out_duration: 1000,
        }
    }
}

pub fn save(host: &Host) -> Result<(), confy::ConfyError> {
    let settings = Settings {
        fade_in_duration: host.fade_in_duration,
        fade_out_duration: host.fade_out_duration,
    };
    confy::store("multiplayer", None, &settings)
}