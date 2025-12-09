use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// 配置类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum ConfigType {
    #[default]
    Wifi,       // 基于 WiFi SSID 触发
    Service,    // 基于网络服务名触发（有线等）
}

/// 单个网络配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkConfig {
    /// 配置名称（用户自定义）
    pub name: String,
    /// 匹配的 WiFi SSID（可选，用于自动匹配）
    #[serde(default)]
    pub ssid: String,
    /// 配置类型
    #[serde(default)]
    pub config_type: ConfigType,
    /// 路由器 MAC 地址（用于唯一标识网络）
    #[serde(default)]
    pub router_mac: Option<String>,
    /// 是否自动应用此配置
    #[serde(default)]
    pub auto_apply: bool,
    /// 应用到哪个网络服务 (如 "Wi-Fi", "Thunderbolt Ethernet")
    pub target_service: Option<String>,
    pub use_dhcp: bool,
    pub ip_address: Option<String>,
    pub subnet_mask: Option<String>,
    pub router: Option<String>,
    pub dns_servers: Vec<String>,
}

/// 应用配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub configs: HashMap<String, NetworkConfig>,
    pub auto_switch: bool,
    pub network_service: String,
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("network-switcher")
            .join("config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
                Err(_) => Self::default(),
            }
        } else {
            Self::default()
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let content = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(&path, content).map_err(|e| e.to_string())
    }

    pub fn add_config(&mut self, config: NetworkConfig) {
        // 使用唯一键存储
        let key = config.config_key();
        self.configs.insert(key, config);
    }

    pub fn remove_config(&mut self, key: &str) {
        self.configs.remove(key);
    }

    /// 根据 SSID 和 MAC 地址查找自动应用的配置
    pub fn find_auto_apply_config(&self, ssid: &str, router_mac: Option<&str>) -> Option<&NetworkConfig> {
        // 只查找标记为自动应用的配置
        // 优先精确匹配（SSID + MAC）
        for config in self.configs.values() {
            if config.auto_apply && config.matches_network(ssid, router_mac) {
                return Some(config);
            }
        }

        // 如果没有精确匹配，尝试仅匹配 SSID（兼容旧配置）
        for config in self.configs.values() {
            if config.auto_apply && config.ssid == ssid && config.router_mac.is_none() {
                return Some(config);
            }
        }

        None
    }
}

impl NetworkConfig {
    pub fn new(name: String, ssid: String, target_service: Option<String>, config_type: ConfigType, router_mac: Option<String>) -> Self {
        Self {
            name,
            ssid,
            config_type,
            router_mac,
            auto_apply: false,
            target_service,
            use_dhcp: true,
            ip_address: None,
            subnet_mask: None,
            router: None,
            dns_servers: Vec::new(),
        }
    }

    /// 生成配置的唯一键（使用配置名称）
    pub fn config_key(&self) -> String {
        // 使用配置名称作为唯一键
        self.name.clone()
    }

    /// 匹配网络标识（检查 SSID 和可选的 MAC 地址）
    pub fn matches_network(&self, ssid: &str, router_mac: Option<&str>) -> bool {
        // SSID 为空表示不限制
        if self.ssid.is_empty() {
            return true;
        }

        // SSID 必须匹配
        if self.ssid != ssid {
            return false;
        }

        // 如果配置有 MAC，则需要 MAC 也匹配
        if let Some(config_mac) = &self.router_mac {
            if let Some(current_mac) = router_mac {
                return config_mac == current_mac;
            }
            // 配置有 MAC 但当前无法获取 MAC，不匹配
            return false;
        }

        // 配置无 MAC，仅匹配 SSID
        true
    }

    /// 显示名称（给用户看的）
    pub fn display_name(&self) -> String {
        let icon = match self.config_type {
            ConfigType::Wifi => "📶",
            ConfigType::Service => "🔌",
        };
        let auto_icon = if self.auto_apply { "🔄" } else { "" };

        if let Some(mac) = &self.router_mac {
            // 只显示 MAC 后 8 位
            let short_mac = &mac[mac.len().saturating_sub(8)..];
            format!("{}{} {} [{}] ({})", auto_icon, icon, self.name, self.ssid, short_mac)
        } else if !self.ssid.is_empty() {
            format!("{}{} {} [{}]", auto_icon, icon, self.name, self.ssid)
        } else {
            format!("{}{} {}", auto_icon, icon, self.name)
        }
    }
}

