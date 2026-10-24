use crate::security::shell_quote;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub permissions: String,
    pub modified: String,
}

pub struct SftpBrowser {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub current_path: String,
    pub entries: Vec<RemoteFileEntry>,
    pub status_message: Option<String>,
    pub is_loading: bool,
}

impl SftpBrowser {
    pub fn new(host: String, port: u16, username: String) -> Self {
        Self {
            host,
            port,
            username,
            current_path: "/home".to_string(),
            entries: Vec::new(),
            status_message: None,
            is_loading: false,
        }
    }

    /// Refresh file list in `current_path` using ssh with safe vector arguments
    pub fn refresh_directory(&mut self) -> Result<(), String> {
        self.is_loading = true;

        let mut cmd = Command::new("ssh");
        cmd.arg("-p").arg(self.port.to_string());
        cmd.arg("--");
        cmd.arg(format!("{}@{}", self.username, self.host));
        // Safe argument: non-interactive ls command with POSIX single-quoted path
        // to prevent remote shell command breakout or metacharacter injection.
        cmd.arg(format!("ls -la {}", shell_quote(&self.current_path)));

        let output = cmd.output().map_err(|e| format!("SSH command failed: {}", e))?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            self.is_loading = false;
            return Err(format!("Failed to list remote directory: {}", err));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        self.entries = parse_ls_output(&stdout);
        self.is_loading = false;
        self.status_message = Some(format!("Loaded {} items in {}", self.entries.len(), self.current_path));
        Ok(())
    }

    pub fn navigate(&mut self, folder: &str) {
        if folder == ".." {
            let parent = Path::new(&self.current_path)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "/".to_string());
            self.current_path = if parent.is_empty() { "/".to_string() } else { parent };
        } else if self.current_path == "/" {
            self.current_path = format!("/{}", folder);
        } else {
            self.current_path = format!("{}/{}", self.current_path.trim_end_matches('/'), folder);
        }
        let _ = self.refresh_directory();
    }

    /// Download remote file to local destination using scp
    pub fn download_file(&self, remote_file: &str, local_dest: &Path) -> Result<(), String> {
        let full_remote = format!("{}/{}", self.current_path.trim_end_matches('/'), remote_file);
        let remote_spec = format!("{}@{}:{}", self.username, self.host, full_remote);

        let mut cmd = Command::new("scp");
        cmd.arg("-P").arg(self.port.to_string());
        cmd.arg("--");
        cmd.arg(remote_spec);
        cmd.arg(local_dest.to_string_lossy().to_string());

        let output = cmd.output().map_err(|e| format!("Failed to run scp: {}", e))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    /// Upload local file to remote destination using scp
    pub fn upload_file(&self, local_src: &Path, remote_dest_name: &str) -> Result<(), String> {
        let full_remote = format!("{}/{}", self.current_path.trim_end_matches('/'), remote_dest_name);
        let remote_spec = format!("{}@{}:{}", self.username, self.host, full_remote);

        let mut cmd = Command::new("scp");
        cmd.arg("-P").arg(self.port.to_string());
        cmd.arg("--");
        cmd.arg(local_src.to_string_lossy().to_string());
        cmd.arg(remote_spec);

        let output = cmd.output().map_err(|e| format!("Failed to run scp: {}", e))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }
}

/// Parses the output of non-interactive `ls -la` safely into structured entries.
/// Resilient against malformed lines, irregular spacing, unicode filenames, and missing fields.
pub fn parse_ls_output(stdout: &str) -> Vec<RemoteFileEntry> {
    let mut entries = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        // Standard ls -la line: drwxr-xr-x 2 user group 4096 Oct 23 12:00 dirname
        if parts.len() >= 9 {
            let perms = parts[0];
            let is_dir = perms.starts_with('d');
            let size: u64 = parts[4].parse().unwrap_or(0);
            let modified = format!("{} {} {}", parts[5], parts[6], parts[7]);
            let name = parts[8..].join(" ");

            if name == "." {
                continue;
            }

            entries.push(RemoteFileEntry {
                name,
                is_dir,
                size,
                permissions: perms.to_string(),
                modified,
            });
        }
    }

    // Sort: directories strictly precede non-directories, with parent ".." first among dirs
    entries.sort_by(|a, b| {
        if a.name == b.name && a.is_dir == b.is_dir {
            return std::cmp::Ordering::Equal;
        }
        // Directories always precede non-directories
        if a.is_dir != b.is_dir {
            return if a.is_dir {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }
        // Among directories, parent ".." always comes first
        if a.is_dir {
            if a.name == ".." {
                return std::cmp::Ordering::Less;
            }
            if b.name == ".." {
                return std::cmp::Ordering::Greater;
            }
        }
        // Alphabetical sort within the same entry type
        a.name.cmp(&b.name)
    });

    entries
}
