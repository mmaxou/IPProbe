//! IPProbe 的 egui 应用:界面布局、自动刷新与状态展示。

use std::time::{Duration, Instant};

use eframe::egui;

use crate::netinfo::{self, AddrScope, IfaceInfo, ScanSummary};

pub const APP_TITLE: &str = "IPProbe - 网络地址探测器";

const INTERVAL_CHOICES: [u64; 4] = [3, 5, 10, 30];

pub struct IPProbeApp {
    ifaces: Vec<IfaceInfo>,
    summary: ScanSummary,
    outbound_v4: Option<std::net::IpAddr>,
    outbound_v6: Option<std::net::IpAddr>,
    last_scan: Option<Instant>,
    auto_refresh: bool,
    interval_secs: u64,
    scan_error: Option<String>,
}

impl Default for IPProbeApp {
    fn default() -> Self {
        let mut app = Self {
            ifaces: Vec::new(),
            summary: ScanSummary::default(),
            outbound_v4: None,
            outbound_v6: None,
            last_scan: None,
            auto_refresh: true,
            interval_secs: 5,
            scan_error: None,
        };
        app.do_scan();
        app
    }
}

impl IPProbeApp {
    fn do_scan(&mut self) {
        match netinfo::scan_interfaces() {
            Ok(ifaces) => {
                self.summary = ScanSummary::compute(&ifaces);
                self.ifaces = ifaces;
                self.scan_error = None;
            }
            Err(e) => self.scan_error = Some(e),
        }
        self.outbound_v4 = netinfo::outbound_ipv4();
        self.outbound_v6 = netinfo::outbound_ipv6();
        self.last_scan = Some(Instant::now());
    }

    fn last_scan_text(&self) -> String {
        match self.last_scan {
            Some(t) => {
                let secs = t.elapsed().as_secs();
                if secs < 3 {
                    "刚刚".to_owned()
                } else {
                    format!("{secs} 秒前")
                }
            }
            None => "-".to_owned(),
        }
    }
}

impl eframe::App for IPProbeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // 自动刷新
        if self.auto_refresh
            && self
                .last_scan
                .is_some_and(|t| t.elapsed() >= Duration::from_secs(self.interval_secs))
        {
            self.do_scan();
        }

        // 顶部工具条
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading("IPProbe");
                ui.label("自动探测本机 IPv4 / IPv6 地址");
                ui.separator();
                if ui.button("刷新").clicked() {
                    self.do_scan();
                }
                ui.checkbox(&mut self.auto_refresh, "自动刷新");
                egui::ComboBox::from_id_salt("interval")
                    .selected_text(format!("{}s", self.interval_secs))
                    .show_ui(ui, |ui| {
                        for c in INTERVAL_CHOICES {
                            ui.selectable_value(&mut self.interval_secs, c, format!("{c}s"));
                        }
                    });
            });
            ui.add_space(4.0);
        });

        // 底部状态栏
        egui::Panel::bottom("statusbar").show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(format!(
                    "接口 {} | 启动 {} | IPv4 {} | IPv6 {}",
                    self.summary.iface_count,
                    self.summary.up_count,
                    self.summary.ipv4_count,
                    self.summary.ipv6_count
                ));
                ui.separator();
                let v4 = self
                    .outbound_v4
                    .map(|a| a.to_string())
                    .unwrap_or_else(|| "无".to_owned());
                let v6 = self
                    .outbound_v6
                    .map(|a| a.to_string())
                    .unwrap_or_else(|| "无".to_owned());
                ui.label(format!("默认出口: IPv4 {v4} · IPv6 {v6}"));
                ui.separator();
                ui.label(format!("上次扫描 {}", self.last_scan_text()));
            });
            if let Some(e) = &self.scan_error {
                ui.colored_label(egui::Color32::LIGHT_RED, format!("扫描出错: {e}"));
            }
            ui.add_space(4.0);
        });

        // 中央接口列表
        egui::CentralPanel::default().show(ui, |ui| {
            if self.ifaces.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label("未检测到任何网络接口");
                });
                return;
            }

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let mut ifaces = std::mem::take(&mut self.ifaces);
                    // 启动中的接口排前面,再按接口名排序
                    ifaces.sort_by(|a, b| b.is_up.cmp(&a.is_up).then(a.name.cmp(&b.name)));
                    for intf in &ifaces {
                        ui.group(|ui| {
                            let (up_color, up_text) = if intf.is_up {
                                (egui::Color32::from_rgb(0x2e, 0xcc, 0x71), "UP")
                            } else {
                                (egui::Color32::GRAY, "DOWN")
                            };
                            ui.horizontal(|ui| {
                                ui.strong(&intf.name);
                                ui.monospace(format!("#{}", intf.index));
                                ui.colored_label(up_color, up_text);
                                ui.separator();
                                ui.label(format!(
                                    "IPv4 x{}/IPv6 x{}",
                                    intf.ipv4().count(),
                                    intf.ipv6().count()
                                ));
                            });
                            for a in &intf.addrs {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(a.addr.to_string()).monospace());
                                    ui.colored_label(scope_color(a.scope), a.scope.label());
                                });
                            }
                        });
                        ui.add_space(6.0);
                    }
                    self.ifaces = ifaces;
                });
        });
    }
}

fn scope_color(scope: AddrScope) -> egui::Color32 {
    match scope {
        AddrScope::Loopback => egui::Color32::from_rgb(0x84, 0x9a, 0xb0),
        AddrScope::LinkLocal => egui::Color32::from_rgb(0x1a, 0xbc, 0x9c),
        AddrScope::UniqueLocal => egui::Color32::from_rgb(0x9b, 0x59, 0xb6),
        AddrScope::Private => egui::Color32::from_rgb(0xe6, 0x7e, 0x22),
        AddrScope::Public => egui::Color32::from_rgb(0x27, 0xae, 0x60),
    }
}
