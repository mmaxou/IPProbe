//! 网络探测:枚举本机全部网络接口(IPv4 + IPv6),并计算默认出口地址。
use std::net::{IpAddr, ToSocketAddrs, UdpSocket};
use std::collections::BTreeMap;

use if_addrs::IfOperStatus;

/// 地址作用域分类(用于界面展示)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddrScope {
    Loopback,
    LinkLocal,
    UniqueLocal,
    Private,
    Public,
}

impl AddrScope {
    pub fn label(self) -> &'static str {
        match self {
            AddrScope::Loopback => "回环",
            AddrScope::LinkLocal => "链路本地",
            AddrScope::UniqueLocal => "唯一本地",
            AddrScope::Private => "私网",
            AddrScope::Public => "公网",
        }
    }
}

/// 接口上的一个地址条目
#[derive(Debug, Clone)]
pub struct IfaceAddr {
    pub addr: IpAddr,
    pub scope: AddrScope,
}

/// 单个网络接口信息
#[derive(Debug, Clone)]
pub struct IfaceInfo {
    pub name: String,
    pub index: u32,
    pub is_up: bool,
    pub addrs: Vec<IfaceAddr>,
}

impl IfaceInfo {
    pub fn ipv4(&self) -> impl Iterator<Item = &IfaceAddr> {
        self.addrs.iter().filter(|a| a.addr.is_ipv4())
    }

    pub fn ipv6(&self) -> impl Iterator<Item = &IfaceAddr> {
        self.addrs.iter().filter(|a| a.addr.is_ipv6())
    }
}

/// 扫描结果统计,用于状态栏
#[derive(Debug, Clone, Default)]
pub struct ScanSummary {
    pub iface_count: usize,
    pub up_count: usize,
    pub ipv4_count: usize,
    pub ipv6_count: usize,
}

impl ScanSummary {
    pub fn compute(ifaces: &[IfaceInfo]) -> Self {
        let mut s = ScanSummary::default();
        s.iface_count = ifaces.len();
        for intf in ifaces {
            if intf.is_up {
                s.up_count += 1;
            }
            s.ipv4_count += intf.ipv4().count();
            s.ipv6_count += intf.ipv6().count();
        }
        s
    }
}

/// 枚举本机所有网络接口,按 (索引, 名称) 聚合地址。
pub fn scan_interfaces() -> Result<Vec<IfaceInfo>, String> {
    let list = if_addrs::get_if_addrs().map_err(|e| e.to_string())?;

    let mut map: BTreeMap<(u32, String), IfaceInfo> = BTreeMap::new();
    for intf in list {
        let entry = map
            .entry((intf.index.unwrap_or(0), intf.name.clone()))
            .or_insert_with(|| IfaceInfo {
                name: intf.name.clone(),
                index: intf.index.unwrap_or(0),
                is_up: intf.oper_status == IfOperStatus::Up,
                addrs: Vec::new(),
            });
        entry.addrs.push(IfaceAddr {
            addr: intf.addr.ip(),
            scope: classify(intf.addr.ip()),
        });
    }
    Ok(map.into_values().collect())
}

/// 默认出口 IPv4 地址(UDP connect 技巧,不发送实际流量)。
/// 依次尝试多个目标(阿里 DNS、114DNS、Cloudflare、Google),国内网络屏蔽
/// 部分境外 IP 时也能拿到本地源地址。
pub fn outbound_ipv4() -> Option<IpAddr> {
    outbound_from_targets(
        "0.0.0.0:0",
        &[
            "223.5.5.5:53",      // 阿里 DNS
            "114.114.114.114:53", // 114DNS
            "1.1.1.1:53",        // Cloudflare
            "8.8.8.8:53",        // Google
        ],
    )
}

/// 默认出口 IPv6 地址;本机无 IPv6 外网连接时返回 None
pub fn outbound_ipv6() -> Option<IpAddr> {
    outbound_from_targets(
        "[::]:0",
        &[
            "[2400:3200::1]:53",       // 阿里 IPv6 DNS
            "[240c::6666]:53",         // 电信 IPv6 DNS
            "[2606:4700:4700::1111]:53", // Cloudflare
            "[2001:4860:4860::8888]:53", // Google
        ],
    )
}

/// UDP connect 到候选目标,取第一个成功者的本地源地址。
fn outbound_from_targets(bind_addr: &str, targets: &[&str]) -> Option<IpAddr> {
    for target in targets {
        let Ok(sock) = UdpSocket::bind(bind_addr) else {
            continue;
        };
        if sock.connect(target).is_ok() {
            return sock.local_addr().ok().map(|a| a.ip());
        }
    }
    None
}
/// 通过公网 HTTP API 获取本机真实出口 IPv4(纯文本响应)。
/// 返回文本,失败返回 None。
pub fn public_outbound_ipv4() -> Option<String> {
    const APIS: &[&str] = &[
        "4.ipw.cn",
        "api.ipify.org",
        "members.3322.org/dyndns/getip",
    ];
    for host_path in APIS {
        if let Some(ip) = http_get_text(host_path, 80) {
            return Some(ip);
        }
    }
    None
}

/// 发一个极小明文 HTTP GET,返回响应体的第一行(期望为 IP 文本)
fn http_get_text(host_path: &str, port: u16) -> Option<String> {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    let (host, path) = match host_path.split_once('/') {
        Some((h, p)) => (h, format!("/{p}")),
        None => (host_path, "/".to_owned()),
    };
    let addr = (host, port).to_socket_addrs().ok()?.next()?;
    let mut sock = TcpStream::connect_timeout(&addr, Duration::from_secs(3)).ok()?;
    sock.set_read_timeout(Some(Duration::from_secs(3))).ok()?;
    sock.set_write_timeout(Some(Duration::from_secs(3))).ok()?;

    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: IPProbe/0.1\r\nConnection: close\r\n\r\n"
    );
    sock.write_all(req.as_bytes()).ok()?;

    let mut buf = String::new();
    sock.read_to_string(&mut buf).ok()?;
    let body = buf.split("\r\n\r\n").nth(1)?;
    let line = body.lines().find(|l| !l.trim().is_empty())?;
    let text = line.trim().to_string();
    if text.parse::<IpAddr>().is_ok() {
        Some(text)
    } else {
        None
    }
}
fn classify(ip: IpAddr) -> AddrScope {
    match ip {
        IpAddr::V4(v4) => {
            if v4.is_loopback() {
                AddrScope::Loopback
            } else if v4.is_link_local() {
                AddrScope::LinkLocal
            } else if is_private_v4(v4) {
                AddrScope::Private
            } else {
                AddrScope::Public
            }
        }
        IpAddr::V6(v6) => {
            if v6.is_loopback() {
                AddrScope::Loopback
            } else if v6.is_unicast_link_local() {
                AddrScope::LinkLocal
            } else if is_unique_local_v6(v6) {
                AddrScope::UniqueLocal
            } else {
                AddrScope::Public
            }
        }
    }
}

/// RFC 1918 私网 + 100.64/10 CGNAT
fn is_private_v4(v4: std::net::Ipv4Addr) -> bool {
    let o = v4.octets();
    match o {
        [10, _, _, _] => true,
        [172, b, _, _] => (16..=31).contains(&b),
        [192, 168, _, _] => true,
        [100, b, _, _] => (64..=127).contains(&b),
        _ => false,
    }
}

/// fc00::/7 唯一本地地址
fn is_unique_local_v6(v6: std::net::Ipv6Addr) -> bool {
    v6.segments()[0] & 0xfe00 == 0xfc00
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn classify_known_addresses() {
        assert_eq!(classify(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))), AddrScope::Loopback);
        assert_eq!(classify(IpAddr::V4(Ipv4Addr::new(169, 254, 1, 1))), AddrScope::LinkLocal);
        assert_eq!(classify(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2))), AddrScope::Private);
        assert_eq!(classify(IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))), AddrScope::Private);
        assert_eq!(classify(IpAddr::V4(Ipv4Addr::new(172, 32, 0, 1))), AddrScope::Public);
        assert_eq!(classify(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))), AddrScope::Private);
        assert_eq!(classify(IpAddr::V4(Ipv4Addr::new(100, 66, 0, 1))), AddrScope::Private);
        assert_eq!(classify(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))), AddrScope::Public);

        assert_eq!(classify(IpAddr::V6(Ipv6Addr::LOCALHOST)), AddrScope::Loopback);
        assert_eq!(classify(IpAddr::V6("fe80::1".parse().unwrap())), AddrScope::LinkLocal);
        assert_eq!(classify(IpAddr::V6("fd12:3456::1".parse().unwrap())), AddrScope::UniqueLocal);
        assert_eq!(classify(IpAddr::V6("2001:4860:4860::8888".parse().unwrap())), AddrScope::Public);
    }

    #[test]
    fn scan_runs_without_panic() {
        let _ = scan_interfaces();
        let _ = outbound_ipv4();
        let _ = outbound_ipv6();
    }
}
