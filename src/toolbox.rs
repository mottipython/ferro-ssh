use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolboxCategory {
    All,
    System,
    Disk,
    Network,
    Docker,
    Services,
    Security,
    Custom,
}

impl ToolboxCategory {
    pub const ALL: [ToolboxCategory; 8] = [
        ToolboxCategory::All,
        ToolboxCategory::System,
        ToolboxCategory::Disk,
        ToolboxCategory::Network,
        ToolboxCategory::Docker,
        ToolboxCategory::Services,
        ToolboxCategory::Security,
        ToolboxCategory::Custom,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            ToolboxCategory::All => "All",
            ToolboxCategory::System => "System",
            ToolboxCategory::Disk => "Disk",
            ToolboxCategory::Network => "Network",
            ToolboxCategory::Docker => "Docker",
            ToolboxCategory::Services => "Services",
            ToolboxCategory::Security => "Security",
            ToolboxCategory::Custom => "Custom",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            ToolboxCategory::All => "📂",
            ToolboxCategory::System => "💻",
            ToolboxCategory::Disk => "💾",
            ToolboxCategory::Network => "🌐",
            ToolboxCategory::Docker => "🐳",
            ToolboxCategory::Services => "⚙️",
            ToolboxCategory::Security => "🛡️",
            ToolboxCategory::Custom => "⭐",
        }
    }

    pub fn matches(&self, cat_str: &str) -> bool {
        match self {
            ToolboxCategory::All => true,
            _ => self.as_str().eq_ignore_ascii_case(cat_str),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolboxCommand {
    pub id: String,
    pub title: String,
    pub command: String,
    pub description: String,
    pub category: String,
    pub is_custom: bool,
    pub needs_args: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CustomCmdDraft {
    pub title: String,
    pub command: String,
    pub description: String,
    pub category: String,
}

impl ToolboxCommand {
    pub fn new(
        title: &str,
        command: &str,
        description: &str,
        category: &str,
        needs_args: bool,
    ) -> Self {
        Self {
            id: format!("{}-{}", category.to_lowercase(), title.to_lowercase().replace(' ', "-")),
            title: title.to_string(),
            command: command.to_string(),
            description: description.to_string(),
            category: category.to_string(),
            is_custom: false,
            needs_args,
        }
    }
}

pub struct ToolboxStore {
    pub builtins: Vec<ToolboxCommand>,
    pub custom_commands: Vec<ToolboxCommand>,
    config_path: PathBuf,
}

impl ToolboxStore {
    pub fn load() -> Self {
        let config_path = Self::get_config_path();
        let builtins = Self::default_builtins();
        let custom_commands = Self::load_custom_commands(&config_path);

        Self {
            builtins,
            custom_commands,
            config_path,
        }
    }

    fn get_config_path() -> PathBuf {
        if let Some(base_dirs) = directories::BaseDirs::new() {
            let dir = base_dirs.config_dir().join("ferro-ssh");
            let _ = fs::create_dir_all(&dir);
            dir.join("toolbox.json")
        } else {
            PathBuf::from("toolbox.json")
        }
    }

    fn load_custom_commands(path: &PathBuf) -> Vec<ToolboxCommand> {
        if let Ok(data) = fs::read_to_string(path) {
            if let Ok(cmds) = serde_json::from_str::<Vec<ToolboxCommand>>(&data) {
                return cmds;
            }
        }
        Vec::new()
    }

    pub fn save_custom_commands(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.custom_commands) {
            let _ = fs::write(&self.config_path, json);
        }
    }

    pub fn add_custom(&mut self, title: String, command: String, description: String, category: String) {
        let id = format!("custom-{}", uuid_simple());
        let cmd = ToolboxCommand {
            id,
            title,
            command,
            description,
            category,
            is_custom: true,
            needs_args: false,
        };
        self.custom_commands.push(cmd);
        self.save_custom_commands();
    }

    pub fn delete_custom(&mut self, id: &str) {
        self.custom_commands.retain(|c| c.id != id);
        self.save_custom_commands();
    }

    pub fn all_commands<'a>(&'a self) -> impl Iterator<Item = &'a ToolboxCommand> {
        self.builtins.iter().chain(self.custom_commands.iter())
    }

    fn default_builtins() -> Vec<ToolboxCommand> {
        vec![
            // 💻 System & Performance
            ToolboxCommand::new(
                "Interactive Monitor (htop)",
                "htop",
                "Full interactive curses process & resource viewer",
                "System",
                false,
            ),
            ToolboxCommand::new(
                "Top CPU Processes",
                "ps aux --sort=-%cpu | head -15",
                "Display top 15 CPU-intensive processes",
                "System",
                false,
            ),
            ToolboxCommand::new(
                "Top Memory Processes",
                "ps aux --sort=-%mem | head -15",
                "Display top 15 RAM-consuming processes",
                "System",
                false,
            ),
            ToolboxCommand::new(
                "System Uptime & Load",
                "uptime -p",
                "Pretty format system uptime and average load",
                "System",
                false,
            ),
            ToolboxCommand::new(
                "Memory & Swap Usage",
                "free -h",
                "Human-readable RAM and swap statistics",
                "System",
                false,
            ),
            ToolboxCommand::new(
                "CPU Architecture & Cores",
                "lscpu",
                "Detailed CPU architecture, sockets, and cache topology",
                "System",
                false,
            ),
            ToolboxCommand::new(
                "OS & Kernel Info",
                "cat /etc/os-release && uname -a",
                "Distribution name, version ID, and kernel release",
                "System",
                false,
            ),
            ToolboxCommand::new(
                "Virtual Memory Stats",
                "vmstat 1 5",
                "5 samples of system virtual memory, paging, and CPU ticks",
                "System",
                false,
            ),

            // 💾 Disk & Storage
            ToolboxCommand::new(
                "Disk Free Space",
                "df -h -x tmpfs -x devtmpfs",
                "Human-readable filesystem free and used space",
                "Disk",
                false,
            ),
            ToolboxCommand::new(
                "Block Devices Layout",
                "lsblk -f",
                "Block storage layout, partition types, and filesystem UUIDs",
                "Disk",
                false,
            ),
            ToolboxCommand::new(
                "Top 10 Largest Items",
                "du -sh * 2>/dev/null | sort -hr | head -10",
                "Find 10 largest files/directories in current path",
                "Disk",
                false,
            ),
            ToolboxCommand::new(
                "Disk I/O Statistics",
                "iostat -xz 1 3",
                "3 samples of extended disk throughput and latency",
                "Disk",
                false,
            ),
            ToolboxCommand::new(
                "Find Large Files (>100M)",
                "find . -type f -size +100M -exec ls -lh {} + 2>/dev/null",
                "Search recursively for files larger than 100 megabytes",
                "Disk",
                false,
            ),
            ToolboxCommand::new(
                "Inode Utilization",
                "df -i",
                "Check filesystem inode usage and saturation",
                "Disk",
                false,
            ),

            // 🌐 Network & Ports
            ToolboxCommand::new(
                "Listening Ports & Services",
                "ss -tulpn",
                "All listening TCP and UDP sockets with owning process PID/name",
                "Network",
                false,
            ),
            ToolboxCommand::new(
                "Active Network Connections",
                "ss -tunp",
                "Established TCP connections and foreign peer sockets",
                "Network",
                false,
            ),
            ToolboxCommand::new(
                "IP Addresses (Brief)",
                "ip -brief address",
                "Summary of network interfaces, statuses, and assigned IPs",
                "Network",
                false,
            ),
            ToolboxCommand::new(
                "IP Routing Table",
                "ip route",
                "Default gateway and active routing paths",
                "Network",
                false,
            ),
            ToolboxCommand::new(
                "Ping External Gateway",
                "ping -c 4 1.1.1.1",
                "Test packet latency and drop against Cloudflare DNS",
                "Network",
                false,
            ),
            ToolboxCommand::new(
                "HTTP Header Probe",
                "curl -I https://cloudflare.com",
                "Verify outbound HTTPS connectivity and response status",
                "Network",
                false,
            ),
            ToolboxCommand::new(
                "Firewall Rules (iptables)",
                "sudo iptables -L -n -v --line-numbers",
                "Inspect incoming and outgoing firewall filter tables",
                "Network",
                false,
            ),

            // 🐳 Docker & Containers
            ToolboxCommand::new(
                "Running Containers",
                "docker ps --format \"table {{.ID}}\\t{{.Names}}\\t{{.Status}}\\t{{.Ports}}\"",
                "Formatted overview of active Docker containers",
                "Docker",
                false,
            ),
            ToolboxCommand::new(
                "All Containers (Inc. Stopped)",
                "docker ps -a",
                "List all containers including exited and failed",
                "Docker",
                false,
            ),
            ToolboxCommand::new(
                "Container Resource Usage",
                "docker stats --no-stream",
                "Snapshot of CPU, Memory, Net I/O, and Block I/O per container",
                "Docker",
                false,
            ),
            ToolboxCommand::new(
                "Docker Images",
                "docker images",
                "List locally cached container images and image sizes",
                "Docker",
                false,
            ),
            ToolboxCommand::new(
                "Docker Storage Usage",
                "docker system df",
                "Breakdown of images, containers, and build cache storage",
                "Docker",
                false,
            ),
            ToolboxCommand::new(
                "Tail Container Logs",
                "docker logs --tail 50 -f ",
                "Stream the last 50 lines of container output (type name & press Enter)",
                "Docker",
                true,
            ),
            ToolboxCommand::new(
                "Podman Containers",
                "podman ps -a",
                "List all rootless / system Podman containers",
                "Docker",
                false,
            ),
            ToolboxCommand::new(
                "Prune Unused Images",
                "docker image prune -f",
                "Remove dangling and unused Docker images safely",
                "Docker",
                false,
            ),

            // ⚙️ Services & Logs (systemd)
            ToolboxCommand::new(
                "Recent System Errors",
                "journalctl -p 3 -xb --no-pager | tail -40",
                "View last 40 critical error and alert log entries",
                "Services",
                false,
            ),
            ToolboxCommand::new(
                "Failed systemd Units",
                "systemctl --failed",
                "List failed or degraded background system units",
                "Services",
                false,
            ),
            ToolboxCommand::new(
                "Running Services",
                "systemctl list-units --type=service --state=running",
                "List all active daemon services managed by systemd",
                "Services",
                false,
            ),
            ToolboxCommand::new(
                "Follow Service Logs",
                "journalctl -u  -f",
                "Follow systemd logs for a service in real time",
                "Services",
                true,
            ),
            ToolboxCommand::new(
                "Restart Service",
                "sudo systemctl restart ",
                "Restart a target system service daemon",
                "Services",
                true,
            ),
            ToolboxCommand::new(
                "Kernel Ring Buffer (dmesg)",
                "dmesg -T | tail -30",
                "Print last 30 kernel hardware and boot diagnostics",
                "Services",
                false,
            ),

            // 🛡️ Security & Users
            ToolboxCommand::new(
                "Logged-In Users Activity",
                "w",
                "Show who is logged on and what commands they are executing",
                "Security",
                false,
            ),
            ToolboxCommand::new(
                "Recent User Logins",
                "last -n 15",
                "Show last 15 successful sessions and system reboots",
                "Security",
                false,
            ),
            ToolboxCommand::new(
                "Current User Privileges",
                "sudo -l",
                "List allowed sudo commands for the current user",
                "Security",
                false,
            ),
            ToolboxCommand::new(
                "Active SSH Sessions",
                "who -u",
                "Show active SSH connections and their client IP / TTY",
                "Security",
                false,
            ),
            ToolboxCommand::new(
                "Active SSHD Configuration",
                "sshd -T | head -30",
                "Dump running OpenSSH server effective parameters",
                "Security",
                false,
            ),
            ToolboxCommand::new(
                "Regular User Accounts",
                "awk -F: '$3 >= 1000 && $3 < 65534 {print $1, \"(UID: \" $3 \")\", \"Shell: \" $7}' /etc/passwd",
                "List non-system user accounts on the remote host",
                "Security",
                false,
            ),
        ]
    }
}

fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    format!("{:x}{:x}", now.as_secs(), now.subsec_nanos())
}
