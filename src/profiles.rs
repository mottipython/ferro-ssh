use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthMethod {
    Password,
    KeyFile,
    Agent,
}

impl Default for AuthMethod {
    fn default() -> Self {
        Self::Password
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelConfig {
    pub name: String,
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionProfile {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: AuthMethod,
    pub key_path: Option<PathBuf>,
    pub group: String,
    pub badge_color: [u8; 3],
    pub notes: String,
    pub tunnels: Vec<TunnelConfig>,
    pub last_connected: Option<String>,
}

impl Default for ConnectionProfile {
    fn default() -> Self {
        Self {
            id: uuid_simple(),
            name: "New Connection".to_string(),
            host: "127.0.0.1".to_string(),
            port: 22,
            username: "user".to_string(),
            auth_method: AuthMethod::Password,
            key_path: None,
            group: "General".to_string(),
            badge_color: [52, 152, 219], // Default soft blue
            notes: String::new(),
            tunnels: Vec::new(),
            last_connected: None,
        }
    }
}

pub struct ProfileStore {
    config_path: PathBuf,
    pub profiles: Vec<ConnectionProfile>,
}

impl ProfileStore {
    pub fn config_file_path() -> PathBuf {
        BaseDirs::new()
            .map(|b| b.config_dir().join("ferro-ssh").join("profiles.json"))
            .unwrap_or_else(|| PathBuf::from("profiles.json"))
    }

    pub fn load() -> Self {
        let path = Self::config_file_path();
        let profiles = if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_else(|_| Self::default_profiles()),
                Err(_) => Self::default_profiles(),
            }
        } else {
            Self::default_profiles()
        };

        Self {
            config_path: path,
            profiles,
        }
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.profiles)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        fs::write(&self.config_path, json)
    }

    fn default_profiles() -> Vec<ConnectionProfile> {
        Vec::new()
    }

    pub fn add_or_update(&mut self, profile: ConnectionProfile) {
        if let Some(existing) = self.profiles.iter_mut().find(|p| p.id == profile.id) {
            *existing = profile;
        } else {
            self.profiles.push(profile);
        }
        let _ = self.save();
    }

    pub fn delete(&mut self, id: &str) {
        self.profiles.retain(|p| p.id != id);
        let _ = self.save();
    }
}

/// Discovers existing SSH keys in ~/.ssh/
pub fn discover_ssh_keys() -> Vec<PathBuf> {
    let mut found = Vec::new();
    if let Some(base) = BaseDirs::new() {
        let ssh_dir = base.home_dir().join(".ssh");
        if ssh_dir.exists() {
            let candidate_names = [
                "id_ed25519",
                "id_rsa",
                "id_ecdsa",
                "id_dsa",
                "id_ed25519_sk",
                "id_ecdsa_sk",
            ];
            for name in candidate_names {
                let candidate = ssh_dir.join(name);
                if candidate.exists() && candidate.is_file() {
                    found.push(candidate);
                }
            }
        }
    }
    found
}

fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    format!("{:x}", now)
}
