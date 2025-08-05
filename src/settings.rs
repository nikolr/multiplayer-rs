use serde::{Deserialize, Serialize};
use crate::host::host::Host;
use crate::State;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub enum LobbyType {
    FriendsOnly,
    Private,
}

impl From<LobbyType> for steamworks::LobbyType {
    fn from(value: LobbyType) -> Self {
        match value {
            LobbyType::FriendsOnly => {
                steamworks::LobbyType::FriendsOnly
            }
            LobbyType::Private => {
                steamworks::LobbyType::Private
            }
        }
    }
}
impl std::fmt::Display for LobbyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::FriendsOnly => "FriendsOnly",
            Self::Private => "Private",
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Settings {
    pub fade_in_duration: u64,
    pub fade_out_duration: u64,
    pub lobby_type: LobbyType,
    pub max_members: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            fade_in_duration: 1000,
            fade_out_duration: 1000,
            lobby_type: LobbyType::FriendsOnly,
            max_members: 4,
        }
    }
}

pub fn save_host_settings(host: &Host) -> Result<(), confy::ConfyError> {
    let settings: Settings = confy::load("multiplayer", None)?;
    let settings = Settings {
        fade_in_duration: host.fade_in_duration,
        fade_out_duration: host.fade_out_duration,
        lobby_type: settings.lobby_type,
        max_members: settings.max_members,
    };
    confy::store("multiplayer", None, &settings)
}

pub fn save_state_settings(state: &State) -> Result<(), confy::ConfyError> {
    let settings: Settings = confy::load("multiplayer", None)?;
    let settings = Settings {
        fade_in_duration: settings.fade_in_duration,
        fade_out_duration: settings.fade_out_duration,
        lobby_type: state.lobby_type,
        max_members: state.max_members,
    };
    confy::store("multiplayer", None, &settings)
}
