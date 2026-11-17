//! Cloudflare 动态解析(DDNS):利用 API Token 自动创建/更新 A 与 AAAA 记录,
//! 实现“域名转发到本机”的效果(参考 ddns-go 的 Cloudflare 实现)。
//!
//! Token 需要在 Cloudflare 控制台创建,并授予 `Zone → DNS → Edit` 权限。
//! API 文档:https://developers.cloudflare.com/api/resources/dns/subresources/records/

use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::settings::CloudflareSettings;

const API_BASE: &str = "https://api.cloudflare.com/client/v4";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Cloudflare API 统一响应信封。
#[derive(Debug, Clone, Deserialize)]
struct Envelope<T> {
    success: bool,
    #[serde(default)]
    errors: Vec<ApiError>,
    messages: Vec<String>,
    result: Option<T>,
}

#[derive(Debug, Clone, Deserialize)]
struct ApiError {
    code: i64,
    message: String,
}

/// Zone 信息。
#[derive(Debug, Clone)]
pub struct Zone {
    pub id: String,
    pub name: String,
}

/// DNS 记录(只保留同步所需字段)。
#[derive(Debug, Clone)]
pub struct Record {
    pub id: String,
    pub content: String,
    pub proxied: bool,
}

/// 创建/更新记录时发送的请求体。
#[derive(Debug, Clone, Serialize)]
struct RecordPayload {
    name: String,
    #[serde(rename = "type")]
    record_type: String,
    content: String,
    proxied: bool,
    ttl: u64,
}

/// 同步结果摘要。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncReport {
    /// 每行形如“更新 AAAA nas.example.com → 240e::…”或“无变化 AAAA nas.example.com”。
    pub lines: Vec<String>,
    /// 是否有任一请求失败(失败信息会写入 lines)。
    pub failed: bool,
}

impl SyncReport {
    pub fn render(&self) -> String {
        if self.lines.is_empty() {
            "已同步,无需变更".to_owned()
        } else {
            self.lines.join("; ")
        }
    }
}

/// 发起一次 Cloudflare API 请求,返回 (HTTP 状态码, 原始响应体)。
fn http_request(
    method: &str,
    path: &str,
    token: &str,
    body: Option<&str>,
) -> Result<(u16, String), String> {
    let url = format!("{API_BASE}{path}");
    let agent = ureq::AgentBuilder::new()
        .timeout(REQUEST_TIMEOUT)
        .build();
    let mut req = match method {
        "GET" => agent.get(&url),
        "POST" => agent.post(&url),
        "PUT" => agent.put(&url),
        _ => return Err(format!("不支持的请求方法: {method}")),
    };
    req = req.set("Authorization", &format!("Bearer {token}"));
    if body.is_some() {
        req = req.set("Content-Type", "application/json");
    }
    let resp = match body {
        Some(b) => req.send_string(b),
        None => req.call(),
    };
    match resp {
        Ok(r) => {
            let status = r.status();
            let text = r.into_string().map_err(|e| format!("读取响应失败: {e}"))?;
            Ok((status, text))
        }
        Err(ureq::Error::Status(code, r)) => {
            let text = r.into_string().unwrap_or_default();
            Ok((code, text))
        }
        Err(e) => Err(format!("网络请求失败: {e}")),
    }
}

/// 解析信封并提取 result;HTTP 非 2xx 或 success=false 时返回带 Cloudflare 错误码的详情。
fn parse_envelope<T: DeserializeOwned>(status: u16, body: &str, action: &str) -> Result<T, String> {
    let env: Envelope<T> = serde_json::from_str(body)
        .map_err(|e| format!("{action}: 响应解析失败: {e}(HTTP {status})"))?;
    if !env.success || !(200..300).contains(&status) {
        let detail = env
            .errors
            .iter()
            .map(|e| format!("[{}] {}", e.code, e.message))
            .chain(env.messages.iter().cloned())
            .collect::<Vec<_>>()
            .join(", ");
        let detail = if detail.is_empty() {
            "未知错误".to_owned()
        } else {
            detail
        };
        return Err(format!("{action} 失败: {detail}"));
    }
    env.result
        .ok_or_else(|| format!("{action}: 响应缺少 result 字段"))
}

/// RFC 3986 非保留字符以外的字符做百分号编码(用于查询参数与路径段)。
fn pct_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// 根据用户输入的记录名与主域名,解析出完整记录名(FQDN)。
/// “@”表示主域本身;无点号的纯子域自动拼接主域;已含点号按原样使用。
fn resolve_fqdn(record: &str, zone: &str) -> String {
    let record = record.trim().trim_end_matches('.');
    if record.is_empty() {
        return String::new();
    }
    if record == "@" {
        return zone.to_owned();
    }
    if record.contains('.') {
        return record.to_owned();
    }
    format!("{record}.{zone}")
}

/// 规范化 IP 文本(用于与 Cloudflare 记录内容比较,避免格式差异导致误更新)。
fn normalize_ip(raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    if let Ok(v4) = raw.parse::<std::net::Ipv4Addr>() {
        return Ok(v4.to_string());
    }
    if let Ok(v6) = raw.parse::<std::net::Ipv6Addr>() {
        return Ok(v6.to_string());
    }
    Err(format!("无法解析 IP 地址: {raw}"))
}

/// 查询 Zone 列表(按主域名过滤)。
pub fn list_zones(token: &str, zone: &str) -> Result<Vec<Zone>, String> {
    let query = format!("/zones?name={}&status=active&per_page=50", pct_encode(zone));
    let (status, body) = http_request("GET", &query, token, None)?;
    let zones: Vec<ZoneEnvelope> = parse_envelope(status, &body, "查询 Zone")?;
    Ok(zones
        .into_iter()
        .map(|z| Zone { id: z.id, name: z.name })
        .collect())
}

#[derive(Debug, Clone, Deserialize)]
struct ZoneEnvelope {
    id: String,
    name: String,
}

/// 查询指定 Zone 下的 DNS 记录(type = A / AAAA)。
pub fn list_records(
    token: &str,
    zone_id: &str,
    fqdn: &str,
    record_type: &str,
) -> Result<Vec<Record>, String> {
    let query = format!(
        "/zones/{}/dns_records?type={}&name={}&per_page=50",
        pct_encode(zone_id),
        record_type,
        pct_encode(fqdn)
    );
    let (status, body) = http_request("GET", &query, token, None)?;
    let recs: Vec<RecordEnvelope> = parse_envelope(status, &body, "查询 DNS 记录")?;
    Ok(recs
        .into_iter()
        .map(|r| Record {
            id: r.id,
            content: r.content,
            proxied: r.proxied,
        })
        .collect())
}

#[derive(Debug, Clone, Deserialize)]
struct RecordEnvelope {
    id: String,
    content: String,
    #[serde(default)]
    proxied: bool,
}

/// 新增 DNS 记录。
fn create_record(token: &str, zone_id: &str, payload: &RecordPayload) -> Result<(), String> {
    let path = format!("/zones/{}/dns_records", pct_encode(zone_id));
    let body = serde_json::to_string(payload).map_err(|e| e.to_string())?;
    let (status, text) = http_request("POST", &path, token, Some(&body))?;
    parse_envelope::<serde_json::Value>(status, &text, "新增 DNS 记录").map(|_| ())
}

/// 更新 DNS 记录。
fn update_record(
    token: &str,
    zone_id: &str,
    record_id: &str,
    payload: &RecordPayload,
) -> Result<(), String> {
    let path = format!(
        "/zones/{}/dns_records/{}",
        pct_encode(zone_id),
        pct_encode(record_id)
    );
    let body = serde_json::to_string(payload).map_err(|e| e.to_string())?;
    let (status, text) = http_request("PUT", &path, token, Some(&body))?;
    parse_envelope::<serde_json::Value>(status, &text, "更新 DNS 记录").map(|_| ())
}

/// 同步单个地址族(A/AAAA):记录不存在则创建,内容/代理状态有差异则更新。
fn sync_family(
    token: &str,
    zone_id: &str,
    fqdn: &str,
    record_type: &str,
    ip: &str,
    cfg: &CloudflareSettings,
) -> Result<String, String> {
    let ip = normalize_ip(ip)?;
    let existing = list_records(token, zone_id, fqdn, record_type)?;
    let payload = RecordPayload {
        name: fqdn.to_owned(),
        record_type: record_type.to_owned(),
        content: ip.clone(),
        proxied: cfg.proxied,
        ttl: cfg.ttl,
    };

    if existing.is_empty() {
        create_record(token, zone_id, &payload)?;
        return Ok(format!("新增 {record_type} {fqdn} → {ip}"));
    }

    let mut lines = Vec::new();
    for rec in existing {
        let ip_changed = rec.content != ip;
        let proxied_changed = rec.proxied != cfg.proxied;
        if !ip_changed && !proxied_changed {
            lines.push(format!("无变化 {record_type} {fqdn}"));
            continue;
        }
        update_record(token, zone_id, &rec.id, &payload)?;
        lines.push(format!("更新 {record_type} {fqdn} → {ip}"));
    }
    Ok(lines.join("; "))
}

/// 执行一次完整同步。`ipv4` / `ipv6` 为当前检测到的公网地址
/// (None 表示该族无可用地址,将跳过)。
pub fn sync(
    cfg: &CloudflareSettings,
    ipv4: Option<String>,
    ipv6: Option<String>,
) -> Result<SyncReport, String> {
    let mut report = SyncReport::default();

    if !cfg.enabled {
        return Ok(report);
    }

    let token = cfg.token.trim();
    if token.is_empty() {
        return Err("未配置 Cloudflare API Token".to_owned());
    }
    let zone = cfg.zone.trim().trim_end_matches('.');
    if zone.is_empty() {
        return Err("未配置主域名(Zone)".to_owned());
    }
    let fqdn = resolve_fqdn(&cfg.record_name, zone);
    if fqdn.is_empty() {
        return Err("未配置记录名称".to_owned());
    }

    let zones = list_zones(token, zone)?;
    let Some(z) = zones
        .into_iter()
        .find(|z| z.name.eq_ignore_ascii_case(zone))
    else {
        return Err(format!(
            "在 Cloudflare 账户中未找到主域名 {zone}(请检查 Token 权限与域名归属)"
        ));
    };

    if cfg.ipv4_enabled {
        match ipv4 {
            Some(ip) => match sync_family(token, &z.id, &fqdn, "A", &ip, cfg) {
                Ok(line) => report.lines.push(line),
                Err(e) => {
                    report.failed = true;
                    report.lines.push(e);
                }
            },
            None => report.lines.push("未检测到公网 IPv4,跳过 A 记录".to_owned()),
        }
    }

    if cfg.ipv6_enabled {
        match ipv6 {
            Some(ip) => match sync_family(token, &z.id, &fqdn, "AAAA", &ip, cfg) {
                Ok(line) => report.lines.push(line),
                Err(e) => {
                    report.failed = true;
                    report.lines.push(e);
                }
            },
            None => report.lines.push("未检测到公网 IPv6,跳过 AAAA 记录".to_owned()),
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_fqdn_cases() {
        assert_eq!(resolve_fqdn("nas", "example.com"), "nas.example.com");
        assert_eq!(resolve_fqdn("nas.example.com", "example.com"), "nas.example.com");
        assert_eq!(resolve_fqdn("nas.example.com.", "example.com"), "nas.example.com");
        assert_eq!(resolve_fqdn("@", "example.com"), "example.com");
        assert_eq!(resolve_fqdn("", "example.com"), "");
        assert_eq!(resolve_fqdn("  ", "example.com"), "");
    }

    #[test]
    fn normalize_ip_cases() {
        assert_eq!(normalize_ip(" 8.8.8.8 ").unwrap(), "8.8.8.8");
        assert_eq!(normalize_ip("240E:3A4::1").unwrap().to_lowercase(), "240e:3a4::1");
        assert!(normalize_ip("not-an-ip").is_err());
    }

    #[test]
    fn pct_encode_keeps_unreserved() {
        assert_eq!(pct_encode("example.com"), "example.com");
        assert_eq!(pct_encode("a b/c"), "a%20b%2Fc");
    }

    #[test]
    fn envelope_parse_success() {
        let body = r#"{"success":true,"errors":[],"messages":[],"result":[{"id":"z1","name":"example.com"}]}"#;
        let zones: Vec<ZoneEnvelope> = parse_envelope(200, body, "查询 Zone").unwrap();
        assert_eq!(zones.len(), 1);
        assert_eq!(zones[0].id, "z1");
    }

    #[test]
    fn envelope_parse_error_surfaces_code() {
        let body = r#"{"success":false,"errors":[{"code":9109,"message":"This Token is not valid"}],"messages":[],"result":null}"#;
        let err = parse_envelope::<Vec<ZoneEnvelope>>(403, body, "查询 Zone").unwrap_err();
        assert!(err.contains("9109"), "err = {err}");
        assert!(err.contains("not valid"), "err = {err}");
    }

    #[test]
    fn record_payload_json_shape() {
        let payload = RecordPayload {
            name: "nas.example.com".to_owned(),
            record_type: "AAAA".to_owned(),
            content: "240e:3a4::1".to_owned(),
            proxied: false,
            ttl: 1,
        };
        let v = serde_json::to_value(&payload).unwrap();
        assert_eq!(v["name"], "nas.example.com");
        assert_eq!(v["type"], "AAAA");
        assert_eq!(v["content"], "240e:3a4::1");
        assert_eq!(v["proxied"], false);
        assert_eq!(v["ttl"], 1);
    }

    #[test]
    fn sync_report_render() {
        let mut r = SyncReport::default();
        assert_eq!(r.render(), "已同步,无需变更");
        r.lines.push("更新 AAAA nas.example.com → 240e::1".to_owned());
        assert!(r.render().contains("更新 AAAA"));
    }
}
