//! 应用设置:内存状态 + JSON 持久化。
//! 设置随每次变更落盘,模块独立,便于后续扩展更多选项。

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// 存档目录名(放在用户主目录下)。
pub const APP_DIR: &str = ".ipprobe";

/// 存档文件名。
pub const SETTINGS_FILE: &str = "settings.json";

/// Cloudflare 动态解析(DDNS)设置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudflareSettings {
    /// 是否启用 Cloudflare 动态解析。
    #[serde(default)]
    pub enabled: bool,
    /// Cloudflare API Token(Bearer 认证)。
    #[serde(default)]
    pub token: String,
    /// 主域名(Zone),如 example.com。
    #[serde(default)]
    pub zone: String,
    /// DNS 记录名称,如 nas.example.com(或 @ 表示主域)。
    #[serde(default)]
    pub record_name: String,
    /// 是否同步 IPv4(A 记录)。
    #[serde(default)]
    pub ipv4_enabled: bool,
    /// 是否同步 IPv6(AAAA 记录)。
    #[serde(default = "default_true")]
    pub ipv6_enabled: bool,
    /// 是否开启橙色云代理(proxied;免费套餐 AAAA 记录不支持代理)。
    #[serde(default)]
    pub proxied: bool,
    /// TTL(秒),1 = Cloudflare 自动。
    #[serde(default = "default_ttl")]
    pub ttl: u64,
    /// 同步间隔(秒)。
    #[serde(default = "default_cf_interval")]
    pub interval_secs: u64,
}

impl Default for CloudflareSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            token: String::new(),
            zone: String::new(),
            record_name: String::new(),
            ipv4_enabled: false,
            ipv6_enabled: true,
            proxied: false,
            ttl: 1,
            interval_secs: 300,
        }
    }
}

/// IPProbe 可持久化设置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// 开机自启动。
    pub autostart: bool,
    /// 关闭窗口时最小化到托盘,而不是退出。
    pub close_to_tray: bool,
    /// 最小化时隐藏到托盘。
    pub minimize_to_tray: bool,
    /// 后台静默探测。
    pub silent_probe: bool,
    /// 自动刷新(每 interval_secs 秒)。
    pub auto_refresh: bool,
    /// 自动刷新间隔(秒)。
    pub interval_secs: u64,

    /// 是否启用 Bark 通知。
    #[serde(default)]
    pub bark_enabled: bool,
    /// Bark 服务地址(形如 https://api.day.app,不带尾部斜杠)。
    #[serde(default = "default_bark_url")]
    pub bark_url: String,
    #[serde(default)]
    pub bark_token: String,
    /// 每日通知发送小时(0-23),与 bark_minute 组合。
    #[serde(default)]
    pub bark_hour: u32,
    /// 每日通知发送分钟(0-59)。
    #[serde(default)]
    pub bark_minute: u32,
    /// Cloudflare 动态解析设置。
    #[serde(default)]
    pub cloudflare: CloudflareSettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            autostart: false,
            close_to_tray: true,
            minimize_to_tray: true,
            silent_probe: true,
            auto_refresh: true,
            interval_secs: 5,
            bark_enabled: false,
            bark_url: "https://api.day.app".to_owned(),
            bark_token: String::new(),
            bark_hour: 8,
            bark_minute: 0,
            cloudflare: CloudflareSettings::default(),
        }
    }
}

/// Bark 服务地址默认值(公共服务器)。
fn default_bark_url() -> String {
    "https://api.day.app".to_owned()
}

fn default_true() -> bool {
    true
}

fn default_ttl() -> u64 {
    1
}

fn default_cf_interval() -> u64 {
    300
}

/// 从磁盘加载设置;文件不存在或解析失败时回退默认值。
pub fn data_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .map(|home| home.join(APP_DIR))
}

/// 配置文件完整路径。
pub fn settings_path() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join(SETTINGS_FILE))
}

/// 从磁盘加载设置;文件不存在或解析失败时回退默认值。
pub fn load() -> AppSettings {
    let Some(path) = settings_path() else {
        return AppSettings::default();
    };
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => AppSettings::default(),
    }
}

/// 保存设置到磁盘;失败时静默忽略(不影响运行)。
pub fn save(settings: &AppSettings) {
    let Some(path) = settings_path() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    match serde_json::to_string_pretty(settings) {
        Ok(text) => {
            let _ = fs::write(&path, text);
        }
        Err(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_roundtrip() {
        let s = AppSettings {
            autostart: true,
            close_to_tray: false,
            minimize_to_tray: true,
            silent_probe: false,
            auto_refresh: true,
            interval_secs: 10,
            bark_enabled: true,
            bark_url: "https://bark.example.com".to_owned(),
            bark_token: "test-key".to_owned(),
            bark_hour: 9,
    bark_minute: 30,
    cloudflare: CloudflareSettings {
        enabled: true,
        token: "cf-token".to_owned(),
        zone: "example.com".to_owned(),
        record_name: "nas.example.com".to_owned(),
        ipv4_enabled: true,
        ipv6_enabled: true,
        proxied: false,
        ttl: 120,
        interval_secs: 600,
    },
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(s.autostart, back.autostart);
        assert_eq!(s.close_to_tray, back.close_to_tray);
        assert_eq!(s.minimize_to_tray, back.minimize_to_tray);
        assert_eq!(s.silent_probe, back.silent_probe);
        assert_eq!(s.auto_refresh, back.auto_refresh);
        assert_eq!(s.interval_secs, back.interval_secs);
        assert_eq!(s.bark_enabled, back.bark_enabled);
        assert_eq!(s.bark_url, back.bark_url);
        assert_eq!(s.bark_token, back.bark_token);
        assert_eq!(s.bark_hour, back.bark_hour);
        assert_eq!(s.bark_minute, back.bark_minute);
        assert_eq!(s.cloudflare.enabled, back.cloudflare.enabled);
        assert_eq!(s.cloudflare.token, back.cloudflare.token);
        assert_eq!(s.cloudflare.zone, back.cloudflare.zone);
        assert_eq!(s.cloudflare.record_name, back.cloudflare.record_name);
        assert_eq!(s.cloudflare.ipv4_enabled, back.cloudflare.ipv4_enabled);
        assert_eq!(s.cloudflare.ipv6_enabled, back.cloudflare.ipv6_enabled);
        assert_eq!(s.cloudflare.proxied, back.cloudflare.proxied);
        assert_eq!(s.cloudflare.ttl, back.cloudflare.ttl);
        assert_eq!(s.cloudflare.interval_secs, back.cloudflare.interval_secs);
    }

    #[test]
    fn settings_defaults_parseable() {
        let s = AppSettings::default();
        let json = serde_json::to_string(&s).unwrap();
        let back: AppSettings = serde_json::from_str(&json).unwrap();
        assert!(back.close_to_tray);
        assert!(back.minimize_to_tray);
        assert!(back.silent_probe);
        assert!(back.auto_refresh);
        assert!(!back.bark_enabled);
        assert_eq!(back.bark_hour, 8);
        assert_eq!(back.bark_minute, 0);
    }

    #[test]
    fn old_json_without_bark_fields_still_loads() {
        // 兼容旧版 settings.json(没有 bark_* 字段)
        let old = r#"{"autostart":false,"close_to_tray":true,"minimize_to_tray":true,"silent_probe":true,"auto_refresh":true,"interval_secs":5}"#;
        let s: AppSettings = serde_json::from_str(old).unwrap();
        assert!(!s.bark_enabled);
        assert_eq!(s.bark_url, "https://api.day.app");
        assert!(s.bark_token.is_empty());
    }

    #[test]
    fn cloudflare_settings_defaults() {
        let cf = CloudflareSettings::default();
        assert!(!cf.enabled);
        assert!(cf.token.is_empty());
        assert!(cf.zone.is_empty());
        assert!(cf.record_name.is_empty());
        assert!(!cf.ipv4_enabled);
        assert!(cf.ipv6_enabled);
        assert!(!cf.proxied);
        assert_eq!(cf.ttl, 1);
        assert_eq!(cf.interval_secs, 300);

        // 旧版 settings.json(没有 cloudflare 字段)仍能加载,并回退默认值。
        let old = r#"{"autostart":false,"close_to_tray":true,"minimize_to_tray":true,"silent_probe":true,"auto_refresh":true,"interval_secs":5}"#;
        let s: AppSettings = serde_json::from_str(old).unwrap();
        assert!(!s.cloudflare.enabled);
        assert_eq!(s.cloudflare.interval_secs, 300);
    }
}
