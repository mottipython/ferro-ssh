use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::engine::general_purpose::STANDARD_NO_PAD as BASE64_NO_PAD;
use base64::Engine;
use directories::BaseDirs;
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// A memory-safe container for sensitive strings (passwords, private keys, passphrases)
/// that zeroizes its content when dropped and redacts debug output.
#[derive(Clone, Default, Zeroize, ZeroizeOnDrop)]
pub struct SecureSecret {
    inner: String,
}

impl fmt::Debug for SecureSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SecureSecret([REDACTED])")
    }
}

#[allow(dead_code)]
impl SecureSecret {
    pub fn new(secret: String) -> Self {
        Self { inner: secret }
    }

    pub fn as_str(&self) -> &str {
        &self.inner
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// Host key verification status when connecting to a remote machine.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostKeyStatus {
    /// Host key matches an entry in known_hosts.
    Trusted,
    /// Host is not present in known_hosts; needs user decision.
    Unknown {
        host: String,
        port: u16,
        key_type: String,
        fingerprint: String,
        raw_entry: String,
    },
    /// Host key does not match what is stored in known_hosts! (Possible MITM).
    Mismatch {
        host: String,
        port: u16,
        key_type: String,
        stored_fingerprint: String,
        presented_fingerprint: String,
    },
}

#[allow(dead_code)]
pub struct KnownHostsManager {
    path: PathBuf,
}

#[allow(dead_code)]
impl KnownHostsManager {
    pub fn default_path() -> PathBuf {
        BaseDirs::new()
            .map(|b| b.home_dir().join(".ssh").join("known_hosts"))
            .unwrap_or_else(|| PathBuf::from("known_hosts"))
    }

    pub fn new(path: Option<PathBuf>) -> Self {
        Self {
            path: path.unwrap_or_else(Self::default_path),
        }
    }

    /// Calculate SHA256 fingerprint in standard OpenSSH format: SHA256:<base64-no-pad>
    pub fn calculate_fingerprint(raw_pubkey_base64: &str) -> Option<String> {
        let key_bytes = BASE64_STANDARD.decode(raw_pubkey_base64.trim()).ok()?;
        let mut hasher = Sha256::new();
        hasher.update(&key_bytes);
        let hash = hasher.finalize();
        Some(format!("SHA256:{}", BASE64_NO_PAD.encode(hash)))
    }

    /// Format host patterns matching OpenSSH format:
    /// standard port 22 -> "hostname"
    /// custom port -> "[hostname]:port"
    pub fn format_host_pattern(host: &str, port: u16) -> String {
        if port == 22 {
            host.to_string()
        } else {
            format!("[{}]:{}", host, port)
        }
    }

    /// Checks whether host/port is known and if its key matches.
    pub fn check_host_key(
        &self,
        host: &str,
        port: u16,
        key_type: &str,
        raw_pubkey_base64: &str,
    ) -> HostKeyStatus {
        let target_pattern = Self::format_host_pattern(host, port);
        let presented_fingerprint = Self::calculate_fingerprint(raw_pubkey_base64)
            .unwrap_or_else(|| "UNKNOWN_FINGERPRINT".to_string());

        if !self.path.exists() {
            return HostKeyStatus::Unknown {
                host: host.to_string(),
                port,
                key_type: key_type.to_string(),
                fingerprint: presented_fingerprint,
                raw_entry: format!("{} {} {}\n", target_pattern, key_type, raw_pubkey_base64.trim()),
            };
        }

        let file = match fs::File::open(&self.path) {
            Ok(f) => f,
            Err(_) => {
                return HostKeyStatus::Unknown {
                    host: host.to_string(),
                    port,
                    key_type: key_type.to_string(),
                    fingerprint: presented_fingerprint,
                    raw_entry: format!("{} {} {}\n", target_pattern, key_type, raw_pubkey_base64.trim()),
                };
            }
        };

        let reader = BufReader::new(file);
        let mut found_host = false;
        let mut stored_fingerprint = String::new();

        for line in reader.lines().map_while(Result::ok) {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 3 {
                continue;
            }

            let host_part = parts[0];
            let entry_key_type = parts[1];
            let entry_pubkey = parts[2];

            // Match host pattern (either exact or comma-separated list)
            let matches_host = host_part.split(',').any(|pattern| {
                pattern == target_pattern || pattern == host
            });

            if matches_host && entry_key_type == key_type {
                found_host = true;
                if entry_pubkey.trim() == raw_pubkey_base64.trim() {
                    return HostKeyStatus::Trusted;
                } else {
                    stored_fingerprint = Self::calculate_fingerprint(entry_pubkey)
                        .unwrap_or_else(|| "CORRUPT_KEY".to_string());
                }
            }
        }

        if found_host {
            HostKeyStatus::Mismatch {
                host: host.to_string(),
                port,
                key_type: key_type.to_string(),
                stored_fingerprint,
                presented_fingerprint,
            }
        } else {
            HostKeyStatus::Unknown {
                host: host.to_string(),
                port,
                key_type: key_type.to_string(),
                fingerprint: presented_fingerprint,
                raw_entry: format!("{} {} {}\n", target_pattern, key_type, raw_pubkey_base64.trim()),
            }
        }
    }

    /// Appends a newly accepted host key entry to known_hosts.
    pub fn add_known_host(&self, raw_entry: &str) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;

        let entry = if raw_entry.ends_with('\n') {
            raw_entry.to_string()
        } else {
            format!("{}\n", raw_entry)
        };

        file.write_all(entry.as_bytes())?;
        file.flush()?;
        Ok(())
    }
}

/// Safely escapes an arbitrary string for POSIX shell execution using strict single-quote encapsulation.
/// Any single quote inside the string is closed, escaped, and reopened: '\''
/// In POSIX shells (sh, bash, zsh, dash), no character or variable inside single quotes
/// is expanded, completely eliminating shell metacharacter injection.
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_quote_basic() {
        assert_eq!(shell_quote("/var/log/syslog"), "'/var/log/syslog'");
    }

    #[test]
    fn test_shell_quote_with_spaces() {
        assert_eq!(shell_quote("My Documents/file.txt"), "'My Documents/file.txt'");
    }

    #[test]
    fn test_shell_quote_with_metacharacters() {
        assert_eq!(
            shell_quote("; rm -rf /; $(id) && `whoami` | echo \"$PWNED\""),
            "'; rm -rf /; $(id) && `whoami` | echo \"$PWNED\"'"
        );
    }

    #[test]
    fn test_shell_quote_with_single_quotes() {
        assert_eq!(shell_quote("user's file.txt"), "'user'\\''s file.txt'");
    }

    #[test]
    fn test_secure_secret_redacted_debug() {
        let secret = SecureSecret::new("super_secret_password".to_string());
        let debug_str = format!("{:?}", secret);
        assert!(!debug_str.contains("super_secret_password"));
        assert_eq!(debug_str, "SecureSecret([REDACTED])");
    }

    #[test]
    fn test_host_pattern_formatting() {
        assert_eq!(KnownHostsManager::format_host_pattern("example.com", 22), "example.com");
        assert_eq!(KnownHostsManager::format_host_pattern("example.com", 2222), "[example.com]:2222");
    }
}
