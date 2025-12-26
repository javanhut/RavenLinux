// WiFi backend - IWD and wpa_supplicant support

use std::process::Command;
use std::path::Path;
use std::fs;
use regex::Regex;

/// WiFi network information
#[derive(Debug, Clone)]
pub struct Network {
    pub ssid: String,
    pub signal: i32,
    pub security: String,
    pub connected: bool,
}

/// WiFi backend type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Iwd,
    WpaSupplicant,
    None,
}

/// WiFi manager for network operations
pub struct WiFiManager {
    backend: Backend,
    interface: String,
}

impl WiFiManager {
    pub fn new() -> Self {
        let backend = Self::detect_backend();
        let interface = Self::detect_interface();
        Self { backend, interface }
    }

    /// Detect available WiFi backend
    fn detect_backend() -> Backend {
        // Check for IWD
        if Command::new("which").arg("iwctl").output()
            .map(|o| o.status.success()).unwrap_or(false)
        {
            // Check if iwd daemon is running
            if Command::new("pgrep").args(["-x", "iwd"]).output()
                .map(|o| o.status.success()).unwrap_or(false)
                || Path::new("/run/iwd").exists()
            {
                return Backend::Iwd;
            }
        }

        // Check for wpa_supplicant
        if Command::new("which").arg("wpa_cli").output()
            .map(|o| o.status.success()).unwrap_or(false)
        {
            return Backend::WpaSupplicant;
        }

        Backend::None
    }

    /// Detect wireless interface
    fn detect_interface() -> String {
        // Common interface names
        for name in &["wlan0", "wlp2s0", "wlp3s0", "wifi0"] {
            let path = format!("/sys/class/net/{}/wireless", name);
            if Path::new(&path).exists() {
                return name.to_string();
            }
        }

        // Search in /sys/class/net
        if let Ok(entries) = fs::read_dir("/sys/class/net") {
            for entry in entries.filter_map(|e| e.ok()) {
                let name = entry.file_name().to_string_lossy().to_string();
                let wireless_path = format!("/sys/class/net/{}/wireless", name);
                if Path::new(&wireless_path).exists() {
                    return name;
                }
            }
        }

        "wlan0".to_string()
    }

    pub fn backend(&self) -> Backend {
        self.backend
    }

    pub fn interface(&self) -> &str {
        &self.interface
    }

    /// Scan for available networks
    pub fn scan(&self) -> Vec<Network> {
        match self.backend {
            Backend::Iwd => self.scan_iwd(),
            Backend::WpaSupplicant => self.scan_wpa(),
            Backend::None => self.scan_iw(),
        }
    }

    fn scan_iwd(&self) -> Vec<Network> {
        // Trigger scan
        let _ = Command::new("iwctl")
            .args(["station", &self.interface, "scan"])
            .output();

        std::thread::sleep(std::time::Duration::from_secs(2));

        // Get networks
        let output = Command::new("iwctl")
            .args(["station", &self.interface, "get-networks"])
            .output();

        let output = match output {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
            _ => return Vec::new(),
        };

        let current_ssid = self.get_current_ssid();
        let mut networks = Vec::new();

        // Remove ANSI codes
        let ansi_re = Regex::new(r"\x1b\[[0-9;]*m").unwrap();
        let clean_output = ansi_re.replace_all(&output, "");

        for line in clean_output.lines().skip(4) {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Parse IWD output format
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let connected = line.starts_with('>') || line.starts_with('*');
                let ssid = if connected {
                    parts.get(1).unwrap_or(&"").to_string()
                } else {
                    parts.first().unwrap_or(&"").to_string()
                };

                if ssid.is_empty() || ssid.starts_with('-') {
                    continue;
                }

                // Count asterisks for signal strength
                let signal = line.matches('*').count() as i32 * 25;

                // Get security type
                let security = if line.contains("psk") || line.contains("wpa") {
                    "WPA2".to_string()
                } else if line.contains("open") {
                    "Open".to_string()
                } else {
                    "Unknown".to_string()
                };

                let is_connected = connected || current_ssid.as_ref() == Some(&ssid);

                networks.push(Network {
                    ssid,
                    signal: signal.min(100),
                    security,
                    connected: is_connected,
                });
            }
        }

        networks
    }

    fn scan_wpa(&self) -> Vec<Network> {
        // Trigger scan
        let _ = Command::new("wpa_cli")
            .args(["-i", &self.interface, "scan"])
            .output();

        std::thread::sleep(std::time::Duration::from_secs(2));

        // Get results
        let output = Command::new("wpa_cli")
            .args(["-i", &self.interface, "scan_results"])
            .output();

        let output = match output {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
            _ => return Vec::new(),
        };

        let current_ssid = self.get_current_ssid();
        let mut networks = Vec::new();

        for line in output.lines().skip(1) {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 5 {
                let signal_dbm: i32 = parts[2].parse().unwrap_or(-90);
                let signal = Self::dbm_to_percent(signal_dbm);
                let flags = parts[3];
                let ssid = parts[4].to_string();

                if ssid.is_empty() {
                    continue;
                }

                let security = if flags.contains("WPA3") {
                    "WPA3"
                } else if flags.contains("WPA2") {
                    "WPA2"
                } else if flags.contains("WPA") {
                    "WPA"
                } else if flags.contains("WEP") {
                    "WEP"
                } else {
                    "Open"
                }.to_string();

                let connected = current_ssid.as_ref() == Some(&ssid);

                networks.push(Network {
                    ssid,
                    signal,
                    security,
                    connected,
                });
            }
        }

        networks
    }

    fn scan_iw(&self) -> Vec<Network> {
        let output = Command::new("iw")
            .args(["dev", &self.interface, "scan"])
            .output();

        let output = match output {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
            _ => return Vec::new(),
        };

        let current_ssid = self.get_current_ssid();
        let mut networks = Vec::new();
        let mut current_network: Option<Network> = None;

        for line in output.lines() {
            let line = line.trim();

            if line.starts_with("BSS ") {
                if let Some(net) = current_network.take() {
                    if !net.ssid.is_empty() {
                        networks.push(net);
                    }
                }
                current_network = Some(Network {
                    ssid: String::new(),
                    signal: 0,
                    security: "Open".to_string(),
                    connected: false,
                });
            } else if let Some(ref mut net) = current_network {
                if line.starts_with("SSID:") {
                    net.ssid = line.trim_start_matches("SSID:").trim().to_string();
                    net.connected = current_ssid.as_ref() == Some(&net.ssid);
                } else if line.starts_with("signal:") {
                    if let Some(dbm_str) = line.split_whitespace().nth(1) {
                        if let Ok(dbm) = dbm_str.parse::<f32>() {
                            net.signal = Self::dbm_to_percent(dbm as i32);
                        }
                    }
                } else if line.contains("WPA") || line.contains("RSN") {
                    net.security = "WPA2".to_string();
                } else if line.contains("WEP") {
                    net.security = "WEP".to_string();
                }
            }
        }

        if let Some(net) = current_network {
            if !net.ssid.is_empty() {
                networks.push(net);
            }
        }

        networks
    }

    fn dbm_to_percent(dbm: i32) -> i32 {
        // -30 dBm = 100%, -90 dBm = 0%
        let percent = ((dbm + 90) * 100) / 60;
        percent.clamp(0, 100)
    }

    /// Connect to a network
    pub fn connect(&self, ssid: &str, password: Option<&str>) -> Result<(), String> {
        match self.backend {
            Backend::Iwd => self.connect_iwd(ssid, password),
            Backend::WpaSupplicant => self.connect_wpa(ssid, password),
            Backend::None => Err("No WiFi backend available".to_string()),
        }
    }

    fn connect_iwd(&self, ssid: &str, password: Option<&str>) -> Result<(), String> {
        // Pre-save passphrase if provided
        if let Some(pass) = password {
            let psk_path = format!("/var/lib/iwd/{}.psk", ssid);
            let content = format!("[Security]\nPassphrase={}\n", pass);
            fs::write(&psk_path, content).map_err(|e| e.to_string())?;
        }

        // Connect
        let output = Command::new("iwctl")
            .args(["station", &self.interface, "connect", ssid])
            .output()
            .map_err(|e| e.to_string())?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Connection failed: {}", err));
        }

        // Wait for connection
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Request DHCP
        self.request_dhcp();

        Ok(())
    }

    fn connect_wpa(&self, ssid: &str, password: Option<&str>) -> Result<(), String> {
        let config_path = "/etc/wpa_supplicant/wpa_supplicant.conf";

        let network_config = if let Some(pass) = password {
            // Generate PSK config
            let output = Command::new("wpa_passphrase")
                .args([ssid, pass])
                .output()
                .map_err(|e| e.to_string())?;

            if !output.status.success() {
                return Err("Failed to generate WPA config".to_string());
            }

            String::from_utf8_lossy(&output.stdout).to_string()
        } else {
            format!("network={{\n\tssid=\"{}\"\n\tkey_mgmt=NONE\n}}\n", ssid)
        };

        let full_config = format!(
            "ctrl_interface=/run/wpa_supplicant\nupdate_config=1\n{}",
            network_config
        );

        fs::write(config_path, full_config).map_err(|e| e.to_string())?;

        // Reconfigure
        let output = Command::new("wpa_cli")
            .args(["-i", &self.interface, "reconfigure"])
            .output()
            .map_err(|e| e.to_string())?;

        if !output.status.success() {
            return Err("Failed to reconfigure wpa_supplicant".to_string());
        }

        std::thread::sleep(std::time::Duration::from_secs(3));
        self.request_dhcp();

        Ok(())
    }

    fn request_dhcp(&self) {
        // Try various DHCP clients
        for cmd in &["dhcpcd", "dhclient", "udhcpc"] {
            let args = match *cmd {
                "dhcpcd" => vec!["-n", &self.interface],
                "dhclient" => vec![&self.interface as &str],
                "udhcpc" => vec!["-i", &self.interface, "-n", "-q"],
                _ => continue,
            };

            if Command::new(cmd).args(&args).output().is_ok() {
                break;
            }
        }
    }

    /// Disconnect from current network
    pub fn disconnect(&self) -> Result<(), String> {
        let result = match self.backend {
            Backend::Iwd => {
                Command::new("iwctl")
                    .args(["station", &self.interface, "disconnect"])
                    .output()
            }
            Backend::WpaSupplicant => {
                Command::new("wpa_cli")
                    .args(["-i", &self.interface, "disconnect"])
                    .output()
            }
            Backend::None => {
                Command::new("ip")
                    .args(["link", "set", &self.interface, "down"])
                    .output()
            }
        };

        result
            .map_err(|e| e.to_string())
            .and_then(|o| {
                if o.status.success() {
                    Ok(())
                } else {
                    Err("Disconnect failed".to_string())
                }
            })
    }

    /// Get connection status
    pub fn get_status(&self) -> (Option<String>, Option<String>) {
        let ssid = self.get_current_ssid();
        let ip = self.get_current_ip();
        (ssid, ip)
    }

    fn get_current_ssid(&self) -> Option<String> {
        let output = Command::new("iw")
            .args(["dev", &self.interface, "link"])
            .output()
            .ok()?;

        let output_str = String::from_utf8_lossy(&output.stdout);

        if output_str.contains("Not connected") {
            return None;
        }

        for line in output_str.lines() {
            let line = line.trim();
            if line.starts_with("SSID:") {
                return Some(line.trim_start_matches("SSID:").trim().to_string());
            }
        }

        None
    }

    fn get_current_ip(&self) -> Option<String> {
        let output = Command::new("ip")
            .args(["-4", "addr", "show", &self.interface])
            .output()
            .ok()?;

        let output_str = String::from_utf8_lossy(&output.stdout);
        let re = Regex::new(r"inet (\d+\.\d+\.\d+\.\d+)").ok()?;

        re.captures(&output_str)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
    }

    /// Get saved networks
    pub fn get_saved_networks(&self) -> Vec<String> {
        match self.backend {
            Backend::Iwd => self.get_saved_iwd(),
            Backend::WpaSupplicant => self.get_saved_wpa(),
            Backend::None => Vec::new(),
        }
    }

    fn get_saved_iwd(&self) -> Vec<String> {
        let mut networks = Vec::new();

        for pattern in &["/var/lib/iwd/*.psk", "/var/lib/iwd/*.open"] {
            if let Ok(entries) = glob::glob(pattern) {
                for entry in entries.filter_map(|e| e.ok()) {
                    if let Some(name) = entry.file_stem() {
                        networks.push(name.to_string_lossy().to_string());
                    }
                }
            }
        }

        networks
    }

    fn get_saved_wpa(&self) -> Vec<String> {
        let config_path = "/etc/wpa_supplicant/wpa_supplicant.conf";
        let content = fs::read_to_string(config_path).unwrap_or_default();

        let re = Regex::new(r#"ssid="([^"]+)""#).unwrap();
        re.captures_iter(&content)
            .filter_map(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .collect()
    }

    /// Forget a saved network
    pub fn forget_network(&self, ssid: &str) -> Result<(), String> {
        match self.backend {
            Backend::Iwd => {
                let psk_path = format!("/var/lib/iwd/{}.psk", ssid);
                let open_path = format!("/var/lib/iwd/{}.open", ssid);
                let _ = fs::remove_file(&psk_path);
                let _ = fs::remove_file(&open_path);
                Ok(())
            }
            Backend::WpaSupplicant => {
                let config_path = "/etc/wpa_supplicant/wpa_supplicant.conf";
                let content = fs::read_to_string(config_path).map_err(|e| e.to_string())?;

                let pattern = format!(r#"network=\{{[^}}]*ssid="{}"\s*[^}}]*\}}"#, regex::escape(ssid));
                let re = Regex::new(&pattern).map_err(|e| e.to_string())?;

                let new_content = re.replace_all(&content, "").to_string();
                fs::write(config_path, new_content).map_err(|e| e.to_string())?;

                Ok(())
            }
            Backend::None => Err("No backend available".to_string()),
        }
    }

    /// Check if a network is known/saved
    pub fn is_known_network(&self, ssid: &str) -> bool {
        self.get_saved_networks().contains(&ssid.to_string())
    }
}

impl Default for WiFiManager {
    fn default() -> Self {
        Self::new()
    }
}
