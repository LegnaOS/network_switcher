use std::process::Command;

use crate::config::NetworkConfig;

/// 获取当前连接的 WiFi SSID
pub fn get_current_ssid() -> Option<String> {
    // 方法1: 使用 ioreg (最可靠，不会被隐私保护遮蔽)
    if let Some(ssid) = get_ssid_via_ioreg() {
        return Some(ssid);
    }

    // 方法2: 使用 networksetup
    if let Some(ssid) = get_ssid_via_networksetup() {
        return Some(ssid);
    }

    // 方法3: 使用 system_profiler
    if let Some(ssid) = get_ssid_via_system_profiler() {
        return Some(ssid);
    }

    None
}

/// 通过 ioreg 获取 SSID (不受隐私保护影响)
fn get_ssid_via_ioreg() -> Option<String> {
    // 使用 shell 管道直接 grep，比读取全部输出更快
    let output = Command::new("sh")
        .args(["-c", "ioreg -l | grep 'IO80211SSID' | head -1"])
        .output()
        .ok()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let line = stdout.trim();
        if line.contains("IO80211SSID") {
            // 格式: "IO80211SSID" = "NetworkName"
            if let Some(start) = line.find("= \"") {
                let rest = &line[start + 3..];
                if let Some(end) = rest.find('"') {
                    let ssid = &rest[..end];
                    if !ssid.is_empty() {
                        return Some(ssid.to_string());
                    }
                }
            }
        }
    }
    None
}

fn get_ssid_via_networksetup() -> Option<String> {
    let output = Command::new("networksetup")
        .args(["-getairportnetwork", "en0"])
        .output()
        .ok()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(ssid) = stdout.strip_prefix("Current Wi-Fi Network: ") {
            let ssid = ssid.trim();
            if !ssid.is_empty() && !stdout.contains("not associated") {
                return Some(ssid.to_string());
            }
        }
    }
    None
}

fn get_ssid_via_system_profiler() -> Option<String> {
    let output = Command::new("system_profiler")
        .args(["SPAirPortDataType"])
        .output()
        .ok()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut in_current_network = false;

        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed == "Current Network Information:" {
                in_current_network = true;
                continue;
            }
            if in_current_network {
                // SSID 是下一行，格式是 "SSID_NAME:"
                if trimmed.ends_with(':') && !trimmed.contains("Network") {
                    let ssid = trimmed.trim_end_matches(':');
                    if !ssid.is_empty() {
                        return Some(ssid.to_string());
                    }
                }
                // 如果遇到其他信息就停止
                if trimmed.starts_with("PHY Mode") || trimmed.starts_with("Other") {
                    break;
                }
            }
        }
    }
    None
}

/// 获取所有网络服务
pub fn get_network_services() -> Vec<String> {
    let output = Command::new("networksetup")
        .args(["-listallnetworkservices"])
        .output()
        .ok();

    match output {
        Some(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .skip(1) // 跳过第一行提示
                .filter(|line| !line.starts_with('*')) // 跳过禁用的服务
                .map(|s| s.to_string())
                .collect()
        }
        _ => vec!["Wi-Fi".to_string()],
    }
}

/// 检测有线网络连接状态
/// 返回连接的以太网接口名称，如 "Ethernet" 或 "USB 10/100/1000 LAN"
pub fn get_ethernet_status() -> Option<String> {
    // 获取所有硬件端口
    let output = Command::new("networksetup")
        .args(["-listallhardwareports"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut current_service: Option<String> = None;
    let mut current_device: Option<String> = None;

    for line in stdout.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix("Hardware Port: ") {
            current_service = Some(name.to_string());
        } else if let Some(dev) = line.strip_prefix("Device: ") {
            current_device = Some(dev.to_string());
        }

        // 当我们有了服务名和设备名，检查是否是以太网且已连接
        if let (Some(service), Some(device)) = (&current_service, &current_device) {
            // 检查是否是以太网类型（排除 Wi-Fi 和 Bluetooth）
            let is_ethernet = !service.to_lowercase().contains("wi-fi")
                && !service.to_lowercase().contains("bluetooth")
                && !service.to_lowercase().contains("thunderbolt bridge")
                && (service.to_lowercase().contains("ethernet")
                    || service.to_lowercase().contains("lan")
                    || service.to_lowercase().contains("usb")
                    || device.starts_with("en"));

            if is_ethernet {
                // 检查接口是否有 IP（即已连接）
                if let Ok(info_output) = Command::new("networksetup")
                    .args(["-getinfo", service])
                    .output()
                {
                    let info = String::from_utf8_lossy(&info_output.stdout);
                    // 如果有 IP 地址，说明已连接
                    for info_line in info.lines() {
                        if info_line.starts_with("IP address:") {
                            let ip = info_line.replace("IP address:", "").trim().to_string();
                            if !ip.is_empty() && ip != "none" {
                                return Some(service.clone());
                            }
                        }
                    }
                }
            }

            current_service = None;
            current_device = None;
        }
    }

    None
}

/// 获取路由器 MAC 地址作为网络的唯一标识
pub fn get_router_mac() -> Option<String> {
    // 1. 先获取默认路由器 IP
    let output = Command::new("sh")
        .args(["-c", "netstat -rn | grep default | awk '{print $2}' | head -1"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let router_ip = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if router_ip.is_empty() {
        return None;
    }

    // 2. 通过 ARP 获取路由器 MAC
    let arp_output = Command::new("arp")
        .args(["-n", &router_ip])
        .output()
        .ok()?;

    if arp_output.status.success() {
        let stdout = String::from_utf8_lossy(&arp_output.stdout);
        // 格式: ? (192.168.1.1) at aa:bb:cc:dd:ee:ff on en0 ifscope [ethernet]
        if let Some(at_pos) = stdout.find(" at ") {
            let rest = &stdout[at_pos + 4..];
            if let Some(on_pos) = rest.find(" on ") {
                let mac = rest[..on_pos].trim();
                if !mac.is_empty() && mac != "(incomplete)" {
                    return Some(mac.to_lowercase());
                }
            }
        }
    }

    None
}

/// 获取当前网络的完整标识信息
#[derive(Debug, Clone, Default)]
pub struct NetworkIdentity {
    pub ssid: Option<String>,           // WiFi SSID
    pub router_mac: Option<String>,     // 路由器 MAC 地址
    pub is_wired: bool,                 // 是否有线
    pub service_name: Option<String>,   // 有线网络服务名
}



/// 获取当前网络的完整标识
pub fn get_network_identity() -> NetworkIdentity {
    let router_mac = get_router_mac();

    // 优先检查 WiFi
    if let Some(ssid) = get_current_ssid() {
        return NetworkIdentity {
            ssid: Some(ssid),
            router_mac,
            is_wired: false,
            service_name: None,
        };
    }

    // 检查有线网络
    if let Some(ethernet) = get_ethernet_status() {
        return NetworkIdentity {
            ssid: None,
            router_mac,
            is_wired: true,
            service_name: Some(ethernet),
        };
    }

    NetworkIdentity::default()
}

/// 获取当前网络配置
pub fn get_current_config(service: &str) -> NetworkConfig {
    let mut config = NetworkConfig::default();

    // 获取 IP 信息
    if let Ok(output) = Command::new("networksetup")
        .args(["-getinfo", service])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some(ip) = line.strip_prefix("IP address: ") {
                config.ip_address = Some(ip.trim().to_string());
            } else if let Some(mask) = line.strip_prefix("Subnet mask: ") {
                config.subnet_mask = Some(mask.trim().to_string());
            } else if let Some(router) = line.strip_prefix("Router: ") {
                config.router = Some(router.trim().to_string());
            }
        }
        config.use_dhcp = stdout.contains("DHCP Configuration");
    }

    // 获取 DNS (先尝试 networksetup，再尝试 scutil)
    config.dns_servers = get_dns_servers(service);

    config
}

/// 获取 DNS 服务器
fn get_dns_servers(service: &str) -> Vec<String> {
    // 方法1: 从 networksetup 获取该服务配置的 DNS
    if let Ok(output) = Command::new("networksetup")
        .args(["-getdnsservers", service])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.contains("There aren't any DNS Servers") {
            let servers: Vec<String> = stdout
                .lines()
                .filter(|line| !line.is_empty() && !line.contains("Error"))
                .map(|s| s.trim().to_string())
                .collect();
            if !servers.is_empty() {
                return servers;
            }
        }
    }

    // 方法2: 从 scutil --dns 获取实际使用的 DNS
    if let Ok(output) = Command::new("scutil")
        .args(["--dns"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut servers = Vec::new();

        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("nameserver") {
                // 格式: "nameserver[0] : 8.8.8.8"
                if let Some(dns) = trimmed.split(':').nth(1) {
                    let dns = dns.trim();
                    // 过滤掉 VPN 的 DNS (198.18.x.x)
                    if !dns.starts_with("198.18.") && !servers.contains(&dns.to_string()) {
                        servers.push(dns.to_string());
                    }
                }
            }
        }

        if !servers.is_empty() {
            return servers;
        }
    }

    Vec::new()
}

/// 应用网络配置
pub fn apply_config(service: &str, config: &NetworkConfig) -> Result<(), String> {
    if config.use_dhcp {
        // 使用 DHCP
        run_command("networksetup", &["-setdhcp", service])?;
    } else {
        // 使用静态 IP
        let ip = config.ip_address.as_deref().unwrap_or("192.168.1.100");
        let mask = config.subnet_mask.as_deref().unwrap_or("255.255.255.0");
        let router = config.router.as_deref().unwrap_or("192.168.1.1");
        
        run_command("networksetup", &["-setmanual", service, ip, mask, router])?;
    }

    // 设置 DNS
    if config.dns_servers.is_empty() {
        run_command("networksetup", &["-setdnsservers", service, "Empty"])?;
    } else {
        let mut args = vec!["-setdnsservers", service];
        for dns in &config.dns_servers {
            args.push(dns.as_str());
        }
        run_command("networksetup", &args)?;
    }

    Ok(())
}

fn run_command(cmd: &str, args: &[&str]) -> Result<(), String> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

