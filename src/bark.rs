//! Bark 推送通知客户端(Bark: iOS 轻量推送工具)。
//! API 规范见 https://bark.day.app :
//!   `GET {server}/{key}/{title}/{body}?group=...` 或 POST 同名参数。
//! 本模块负责:构建请求 URL、percent 编码、发送、以及本地时钟(供每日定时)。

use std::time::Duration;

/// 本地时钟快照(用于判断每日定时发送)。
#[derive(Debug, Clone)]
pub struct LocalTime {
    /// 小时 0-23。
    pub hour: u32,
    /// 分钟 0-59。
    pub minute: u32,
    /// 日期键,形如 `2026-11-13`,用于避免同一天重复发送。
    pub date_key: String,
}

/// 获取当前本地时间。
/// 非 Windows 平台返回 None(不支持本地时钟,定时发送自动停用)。
#[cfg(windows)]
pub fn local_now() -> Option<LocalTime> {
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;

    let mut st = SYSTEMTIME::default();
    unsafe { GetLocalTime(&mut st) };
    Some(LocalTime {
        hour: st.wHour as u32,
        minute: st.wMinute as u32,
        date_key: format!("{:04}-{:02}-{:02}", st.wYear, st.wMonth, st.wDay),
    })
}

#[cfg(not(windows))]
pub fn local_now() -> Option<LocalTime> {
    None
}

/// 发送 Bark 通知。
///
/// - `base`: 服务地址,如 `https://api.day.app`(可带端口,如 `http://192.168.1.10:8080`)。
/// - `token`: Bark 设备 Key(URL 路径第一段)。
/// - `title` / `body`: 通知标题与正文(正文支持换行)。
pub fn send(base: &str, token: &str, title: &str, body: &str) -> Result<(), String> {
    let base = base.trim().trim_end_matches('/').to_string();
    if base.is_empty() {
        return Err("Bark 服务地址为空".to_owned());
    }
    let token = token.trim();
    if token.is_empty() {
        return Err("Bark 设备 Key 为空".to_owned());
    }

    let url = format!(
        "{base}/{}/{}/{}?group=IPProbe",
        encode_component(token),
        encode_component(title),
        encode_component(body),
    );

    let resp = ureq::get(&url)
        .timeout(Duration::from_secs(5))
        .call()
        .map_err(|e| format!("Bark 请求失败: {e}"))?;

    if resp.status() == 200 {
        Ok(())
    } else {
        Err(format!("Bark 服务器返回状态码 {}", resp.status()))
    }
}

/// RFC 3986 未保留字符之外的字节按 UTF-8 percent 编码。
fn encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'~' => out.push(*b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_encode_works() {
        assert_eq!(encode_component("ab 12"), "ab%2012");
        assert_eq!(encode_component("中文"), "%E4%B8%AD%E6%96%87");
        assert_eq!(encode_component("a-b_c.d~"), "a-b_c.d~");
    }

    #[test]
    fn send_rejects_empty_config() {
        assert!(send("", "k", "t", "b").is_err());
        assert!(send("https://api.day.app", " ", "t", "b").is_err());
        assert!(send("https://api.day.app", "k", "t", "b").is_ok_or_err_if_network());
    }

    #[test]
    fn local_time_shape() {
        if let Some(t) = local_now() {
            assert!((0..24).contains(&t.hour));
            assert!((0..60).contains(&t.minute));
            assert_eq!(t.date_key.len(), 10);
        }
    }

    trait IsOkOrErrIfNetwork {
        fn is_ok_or_err_if_network(self) -> bool;
    }
    impl IsOkOrErrIfNetwork for Result<(), String> {
        fn is_ok_or_err_if_network(self) -> bool {
            // 离线或网络不可达时允许 Err(但 Err 消息不应是配置错误)
            match self {
                Ok(()) => true,
                Err(e) => !e.contains("服务地址") && !e.contains("Key 为空"),
            }
        }
    }
}
