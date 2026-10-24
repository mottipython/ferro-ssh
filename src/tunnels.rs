use serde::{Deserialize, Serialize};
use std::process::{Child, Command};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TunnelType {
    Local,   // -L local_port:remote_host:remote_port
    Remote,  // -R remote_port:local_host:local_port
    Dynamic, // -D local_socks_port
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortTunnel {
    pub id: String,
    pub name: String,
    pub tunnel_type: TunnelType,
    pub bind_port: u16,
    pub target_host: String,
    pub target_port: u16,
    pub ssh_host: String,
    pub ssh_port: u16,
    pub ssh_user: String,
    #[serde(skip)]
    pub is_active: bool,
}

impl PortTunnel {
    pub fn new(
        name: &str,
        bind_port: u16,
        target_host: &str,
        target_port: u16,
        ssh_user: &str,
        ssh_host: &str,
        ssh_port: u16,
    ) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let id = format!("{:x}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos());
        Self {
            id,
            name: name.to_string(),
            tunnel_type: TunnelType::Local,
            bind_port,
            target_host: target_host.to_string(),
            target_port,
            ssh_host: ssh_host.to_string(),
            ssh_port,
            ssh_user: ssh_user.to_string(),
            is_active: false,
        }
    }
}

pub struct TunnelManager {
    pub tunnels: Vec<PortTunnel>,
    active_children: Vec<(String, Child)>,
}

impl TunnelManager {
    pub fn new() -> Self {
        Self {
            tunnels: Vec::new(),
            active_children: Vec::new(),
        }
    }

    pub fn start_tunnel(&mut self, tunnel_id: &str) -> Result<(), String> {
        let tunnel = self
            .tunnels
            .iter_mut()
            .find(|t| t.id == tunnel_id)
            .ok_or_else(|| "Tunnel not found".to_string())?;

        if tunnel.is_active {
            return Ok(());
        }

        let mut cmd = Command::new("ssh");
        cmd.arg("-N"); // Do not execute a remote command
        cmd.arg("-p").arg(tunnel.ssh_port.to_string());

        match tunnel.tunnel_type {
            TunnelType::Local => {
                cmd.arg("-L").arg(format!(
                    "{}:{}:{}",
                    tunnel.bind_port, tunnel.target_host, tunnel.target_port
                ));
            }
            TunnelType::Remote => {
                cmd.arg("-R").arg(format!(
                    "{}:{}:{}",
                    tunnel.bind_port, tunnel.target_host, tunnel.target_port
                ));
            }
            TunnelType::Dynamic => {
                cmd.arg("-D").arg(tunnel.bind_port.to_string());
            }
        }

        cmd.arg(format!("{}@{}", tunnel.ssh_user, tunnel.ssh_host));

        let child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn SSH tunnel process: {}", e))?;

        tunnel.is_active = true;
        self.active_children.push((tunnel.id.clone(), child));
        Ok(())
    }

    pub fn stop_tunnel(&mut self, tunnel_id: &str) {
        if let Some(pos) = self.active_children.iter().position(|(id, _)| id == tunnel_id) {
            let (_, mut child) = self.active_children.remove(pos);
            let _ = child.kill();
            let _ = child.wait();
        }

        if let Some(tunnel) = self.tunnels.iter_mut().find(|t| t.id == tunnel_id) {
            tunnel.is_active = false;
        }
    }

    pub fn stop_all(&mut self) {
        for (_, mut child) in self.active_children.drain(..) {
            let _ = child.kill();
            let _ = child.wait();
        }
        for tunnel in &mut self.tunnels {
            tunnel.is_active = false;
        }
    }
}

impl Drop for TunnelManager {
    fn drop(&mut self) {
        self.stop_all();
    }
}
