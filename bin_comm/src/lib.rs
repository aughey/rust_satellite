use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct RemoteConfig {
    pub kind: String
}

#[derive(Serialize, Deserialize, Debug)]
pub enum RemoteCommands {
    Config(RemoteConfig)
}