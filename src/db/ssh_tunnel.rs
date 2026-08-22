use std::{
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::Duration,
};

use crate::config::SshTunnelConfig;

/// Owns the SSH process so the local forwarding port remains available for the
/// lifetime of the database connection.
pub struct SshTunnel {
    child: Child,
    local_port: u16,
}

impl SshTunnel {
    pub fn connect(
        config: &SshTunnelConfig,
        default_remote_host: &str,
        default_remote_port: u16,
    ) -> Result<Self, String> {
        let local_port = unused_local_port()?;
        let remote_host = config.remote_host.as_deref().unwrap_or(default_remote_host);
        let remote_port = config.remote_port.unwrap_or(default_remote_port);

        let mut command = Command::new("ssh");
        command
            .arg("-N")
            .arg("-o")
            .arg("ExitOnForwardFailure=yes")
            .arg("-o")
            .arg("BatchMode=yes")
            .arg("-p")
            .arg(config.port.to_string())
            .arg("-L")
            .arg(format!(
                "127.0.0.1:{local_port}:{remote_host}:{remote_port}"
            ));
        if let Some(identity_file) = &config.identity_file {
            command.arg("-i").arg(expand_home(identity_file));
        }
        if let Some(known_hosts) = &config.known_hosts {
            command.arg("-o").arg(format!(
                "UserKnownHostsFile={}",
                expand_home(known_hosts).display()
            ));
        }
        let mut child = command
            .arg(format!("{}@{}", config.user, config.host))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start ssh: {e}"))?;

        // ssh exits immediately if it cannot authenticate or bind the forward.
        thread::sleep(Duration::from_millis(100));
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            let stderr = child
                .stderr
                .take()
                .map(|mut stderr| {
                    use std::io::Read as _;
                    let mut output = String::new();
                    let _ = stderr.read_to_string(&mut output);
                    output
                })
                .unwrap_or_default();
            return Err(format!("SSH tunnel failed ({status}): {}", stderr.trim()));
        }

        Ok(Self { child, local_port })
    }

    pub fn local_port(&self) -> u16 {
        self.local_port
    }
}

impl Drop for SshTunnel {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn unused_local_port() -> Result<u16, String> {
    TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("Failed to allocate local port for SSH tunnel: {e}"))?
        .local_addr()
        .map(|address| address.port())
        .map_err(|e| format!("Failed to read local SSH tunnel port: {e}"))
}

fn expand_home(path: &str) -> PathBuf {
    match path.strip_prefix("~/") {
        Some(relative) => dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(relative),
        None => PathBuf::from(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_home_directory() {
        assert_eq!(expand_home("/tmp/key"), PathBuf::from("/tmp/key"));
        assert_eq!(
            expand_home("~/.ssh/id_ed25519"),
            dirs::home_dir().unwrap().join(".ssh/id_ed25519")
        );
    }

    #[test]
    fn allocates_a_local_port() {
        assert_ne!(unused_local_port().unwrap(), 0);
    }
}
