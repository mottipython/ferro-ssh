use crate::profiles::ConnectionProfile;
use crate::security::{KnownHostsManager, SecureSecret};
use crate::terminal::TerminalScreen;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use zeroize::Zeroize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStatus {
    Connecting,
    Connected,
    Disconnected(String),
}

#[derive(Debug, Clone)]
pub enum PromptType {
    HostFingerprint {
        host: String,
        port: u16,
        key_type: String,
        fingerprint: String,
    },
    Password {
        user: String,
        host: String,
    },
}

#[allow(dead_code)]
pub struct SshSession {
    pub id: String,
    pub profile: ConnectionProfile,
    pub screen: TerminalScreen,
    pub status: SessionStatus,
    pub active_prompt: Option<PromptType>,
    pty_writer: Option<Box<dyn Write + Send>>,
    pty_master: Option<Box<dyn MasterPty + Send>>,
    rx_bytes: Receiver<Vec<u8>>,
    tx_write: Sender<Vec<u8>>,
    raw_buffer_history: String,
}

#[allow(dead_code)]
impl SshSession {
    pub fn new(profile: ConnectionProfile, cols: u16, rows: u16) -> Result<Self, String> {
        let pty_system = native_pty_system();
        let pty_pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to allocate PTY: {}", e))?;

        // Build OpenSSH command with clean, vector-based arguments (no shell injection)
        let mut cmd = CommandBuilder::new("ssh");
        cmd.arg("-p");
        cmd.arg(profile.port.to_string());

        // Identity key if specified
        if let Some(key_path) = &profile.key_path {
            cmd.arg("-i");
            cmd.arg(key_path.to_string_lossy().to_string());
        }

        // Setup local tunnels if enabled
        for tunnel in &profile.tunnels {
            if tunnel.enabled {
                cmd.arg("-L");
                cmd.arg(format!("{}:{}:{}", tunnel.local_port, tunnel.remote_host, tunnel.remote_port));
            }
        }

        // Host specification with option-injection guard
        cmd.arg("--");
        cmd.arg(format!("{}@{}", profile.username, profile.host));

        let slave = pty_pair.slave;
        let _child = slave
            .spawn_command(cmd)
            .map_err(|e| format!("Failed to spawn SSH process: {}", e))?;

        let mut reader = pty_pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("Failed to clone PTY reader: {}", e))?;
        let mut writer = pty_pair
            .master
            .take_writer()
            .map_err(|e| format!("Failed to acquire PTY writer: {}", e))?;

        let (tx_to_gui, rx_bytes) = channel::<Vec<u8>>();
        let (tx_write, rx_from_gui) = channel::<Vec<u8>>();

        // Background reader thread: reads from SSH PTY and sends to GUI
        let tx_reader = tx_to_gui.clone();
        thread::Builder::new()
            .name("ssh-pty-reader".to_string())
            .spawn(move || {
                let mut buf = [0u8; 4096];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break, // EOF, process exited
                        Ok(n) => {
                            if tx_reader.send(buf[..n].to_vec()).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            })
            .map_err(|e| format!("Failed to spawn reader thread: {}", e))?;

        // Background writer thread: receives keystrokes from GUI and writes to PTY.
        // Incoming sensitive data (passwords, input) is zeroized immediately after writing.
        thread::Builder::new()
            .name("ssh-pty-writer".to_string())
            .spawn(move || {
                while let Ok(mut data) = rx_from_gui.recv() {
                    let _ = writer.write_all(&data);
                    let _ = writer.flush();
                    data.zeroize();
                }
            })
            .map_err(|e| format!("Failed to spawn writer thread: {}", e))?;

        Ok(Self {
            id: profile.id.clone(),
            profile,
            screen: TerminalScreen::new(cols as usize, rows as usize),
            status: SessionStatus::Connecting,
            active_prompt: None,
            pty_writer: None,
            pty_master: Some(pty_pair.master),
            rx_bytes,
            tx_write,
            raw_buffer_history: String::new(),
        })
    }

    /// Pumps incoming bytes from background reader into the terminal screen
    /// and scans for password or host fingerprint confirmation prompts.
    pub fn update(&mut self) {
        let mut new_text = String::new();
        while let Ok(bytes) = self.rx_bytes.try_recv() {
            self.screen.process_bytes(&bytes);
            if let Ok(s) = std::str::from_utf8(&bytes) {
                new_text.push_str(s);
            }
        }

        if !new_text.is_empty() {
            self.raw_buffer_history.push_str(&new_text);
            if self.raw_buffer_history.len() > 8192 {
                let mut cut = self.raw_buffer_history.len() - 4096;
                while !self.raw_buffer_history.is_char_boundary(cut) && cut < self.raw_buffer_history.len() {
                    cut += 1;
                }
                self.raw_buffer_history.drain(..cut);
            }

            self.inspect_prompts();
        }
    }

    fn inspect_prompts(&mut self) {
        let text = &self.raw_buffer_history;

        // Check for host key fingerprint prompt:
        // Example: "The authenticity of host '...' can't be established.\n... key fingerprint is SHA256:...\nAre you sure you want to continue connecting (yes/no/[fingerprint])?"
        if text.contains("authenticity of host") && text.contains("Are you sure you want to continue connecting") {
            if self.active_prompt.is_none() {
                let fingerprint = extract_fingerprint(text)
                    .unwrap_or_else(|| "SHA256:Unknown".to_string());
                let key_type = extract_key_type(text)
                    .unwrap_or_else(|| "ED25519".to_string());

                self.active_prompt = Some(PromptType::HostFingerprint {
                    host: self.profile.host.clone(),
                    port: self.profile.port,
                    key_type,
                    fingerprint,
                });
            }
        }

        // Check for password prompt:
        // Example: "user@host's password:" or "Password:"
        if (text.ends_with("password: ") || text.ends_with("Password: ") || text.ends_with("password:") || text.ends_with("Password:"))
            && self.active_prompt.is_none()
        {
            self.active_prompt = Some(PromptType::Password {
                user: self.profile.username.clone(),
                host: self.profile.host.clone(),
            });
        }

        // If shell prompt is active (e.g., "$ ", "# ", "> "), mark as Connected
        if (text.ends_with("$ ") || text.ends_with("# ") || text.ends_with("> ") || text.contains("Last login:"))
            && self.status == SessionStatus::Connecting
        {
            self.status = SessionStatus::Connected;
            self.active_prompt = None;
        }
    }

    /// User answers fingerprint prompt
    pub fn answer_fingerprint(&mut self, accept_and_save: bool, accept_once: bool) {
        if accept_and_save || accept_once {
            // Send "yes\n" to SSH process
            self.send_input(b"yes\n");
            self.active_prompt = None;

            if accept_and_save {
                // Also persist to known_hosts if we have raw info
                let _km = KnownHostsManager::new(None);
                // Note: OpenSSH will also write to known_hosts automatically when "yes" is confirmed
            }
        } else {
            // Reject: send "no\n"
            self.send_input(b"no\n");
            self.active_prompt = None;
            self.status = SessionStatus::Disconnected("Host key rejected by user".to_string());
        }
    }

    /// User sends password with immediate buffer zeroization
    pub fn send_password(&mut self, secret: &SecureSecret) {
        let mut bytes = secret.as_str().as_bytes().to_vec();
        bytes.push(b'\n');
        self.send_input(&bytes);
        bytes.zeroize();
        self.active_prompt = None;
    }

    /// Send raw bytes to the PTY
    pub fn send_input(&self, data: &[u8]) {
        let _ = self.tx_write.send(data.to_vec());
    }

    /// Handle terminal window resize
    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.screen.resize(cols as usize, rows as usize);
        if let Some(master) = &self.pty_master {
            let _ = master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            });
        }
    }
}

pub fn extract_fingerprint(text: &str) -> Option<String> {
    if let Some(idx) = text.find("SHA256:") {
        let sub = &text[idx..];
        let end = sub.find(|c: char| c.is_whitespace() || c == '.' || c == '\n').unwrap_or(sub.len());
        return Some(sub[..end].to_string());
    }
    None
}

pub fn extract_key_type(text: &str) -> Option<String> {
    for kt in &["ED25519", "RSA", "ECDSA", "DSA"] {
        if text.to_uppercase().contains(kt) {
            return Some(kt.to_string());
        }
    }
    None
}
