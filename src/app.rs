//! IPProbe 的 egui 应用:界面布局、自动刷新与状态展示,以及设置/托盘/后台静默探测。

use std::time::{Duration, Instant};

use eframe::egui;

use crate::netinfo::{self, AddrScope, IfaceInfo, ScanSummary};
use crate::settings::{self, AppSettings};
use crate::tray::{self, TrayAction, TrayController};

pub const APP_TITLE: &str = "IPProbe - 网络地址探测器";

const INTERVAL_CHOICES: [u64; 4] = [3, 5, 10, 30];

pub struct IPProbeApp {
    ifaces: Vec<IfaceInfo>,
    summary: ScanSummary,
    outbound_v4: Option<std::net::IpAddr>,
    outbound_v6: Option<std::net::IpAddr>,
    last_scan: Option<Instant>,
    scan_error: Option<String>,
    public_v4: Option<String>,
    last_pub_fetch: Option<Instant>,

    /// 应用设置(持久化)。
    settings: AppSettings,
    /// 托盘控制器。
    tray: TrayController,
    /// 设置窗口是否打开。
    settings_open: bool,
    /// 用户是否通过托盘“退出”。
    requested_quit: bool,
    /// 上一个扫描时间(与 auto_refresh 共用 last_scan,这里用于静默探测判断)。
    last_silent_scan: Option<Instant>,
}

impl Default for IPProbeApp {
    fn default() -> Self {
        let loaded = settings::load();
        let mut app = Self {
            ifaces: Vec::new(),
            summary: ScanSummary::default(),
            outbound_v4: None,
            outbound_v6: None,
            last_scan: None,
            scan_error: None,
            public_v4: None,
            last_pub_fetch: None,
            settings: loaded,
            tray: TrayController::new(),
            settings_open: false,
            requested_quit: false,
            last_silent_scan: None,
        };
        app.sync_autostart_on_startup();
        app.do_scan();
        app
    }
}

impl IPProbeApp {
    /// 启动时同步自启动状态(若设置开启但注册表缺失则补写)。
    fn sync_autostart_on_startup(&mut self) {
        #[cfg(windows)]
        {
            if self.settings.autostart {
                if let Ok(exe) = std::env::current_exe() {
                    if !crate::autostart::autostart_enabled() {
                        let _ = crate::autostart::set_autostart(
                            &exe.to_string_lossy(),
                            true,
                        );
                    }
                }
            }
        }
        #[cfg(not(windows))]
        {
            let _ = &self;
        }
    }

    /// 执行一次完整探测。
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
        self.last_silent_scan = Some(Instant::now());
    }

    /// 公网出口 IPv4(外部 HTTP 查询),60 秒内复用缓存。
    fn refresh_public_ipv4(&mut self) {
        let fresh = self.last_pub_fetch.is_some_and(|t| t.elapsed() < Duration::from_secs(60));
        if !fresh {
            self.public_v4 = netinfo::public_outbound_ipv4();
            self.last_pub_fetch = Some(Instant::now());
        }
    }

    /// 处理关闭/最小化到托盘等窗口事件。
    /// 返回 `true` 表示应用应正常退出(放行关闭)。
    fn handle_viewport_events(&mut self, ctx: &egui::Context) -> bool {
        // 1. 托盘动作优先处理
        if let Some(action) = self.tray.poll() {
            match action {
                TrayAction::Show => tray::show_main_window(ctx),
                TrayAction::Quit => self.requested_quit = true,
            }
        }

        // 2. 用户请求退出时放行关闭
        if self.requested_quit {
            return true;
        }

        // 3. 系统关闭请求 → 按设置决定是退出还是缩到托盘
        let close_requested = ctx.input(|i| i.viewport().close_requested());
        if close_requested {
            if self.settings.close_to_tray {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                tray::hide_main_window(ctx);
            } else {
                return true;
            }
        }

        // 4. 最小化到托盘
        if self.settings.minimize_to_tray {
            let minimized = ctx.input(|i| i.viewport().minimized);
            if minimized == Some(true) {
                tray::hide_main_window(ctx);
                // 防止再次被评估为最小化
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            }
        }

        false
    }

    /// 后台静默探测:窗口隐藏期间仍按间隔探测。
    /// 通过请求未来重绘来保证隐藏时也能收到帧。
    fn maybe_silent_probe(&mut self, ctx: &egui::Context) {
        let interval = Duration::from_secs(self.settings.interval_secs.max(1));
        if self.settings.silent_probe && self.settings.auto_refresh {
            let due = self
                .last_silent_scan
                .is_some_and(|t| t.elapsed() >= interval);
            if due {
                self.do_scan();
                self.refresh_public_ipv4();
            }
        }
        // 注册下一帧唤醒(隐藏时也保持探测)
        ctx.request_repaint_after(interval);
    }

    /// 保存设置并同步自启动状态。
    fn apply_settings(&mut self) {
        settings::save(&self.settings);
        #[cfg(windows)]
        {
            let autostart_on = self.settings.autostart;
            if let Ok(exe) = std::env::current_exe() {
                let path = exe.to_string_lossy().to_string();
                let enabled = crate::autostart::autostart_enabled();
                if autostart_on && !enabled {
                    let _ = crate::autostart::set_autostart(&path, true);
                } else if !autostart_on && enabled {
                    let _ = crate::autostart::set_autostart(&path, false);
                }
            }
        }
        #[cfg(not(windows))]
        {
            let _ = &self.settings;
        }
    }

    /// 设置窗口(Modal)。
    fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let mut open = self.settings_open;
        egui::Window::new("设置")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(360.0)
            .show(ctx, |ui| {
                ui.heading("常规");
                ui.checkbox(&mut self.settings.auto_refresh, "自动刷新");
                ui.horizontal(|ui| {
                    ui.label("刷新间隔");
                    egui::ComboBox::from_id_salt("settings_interval")
                        .selected_text(format!("{}s", self.settings.interval_secs))
                        .show_ui(ui, |ui| {
                            for c in INTERVAL_CHOICES {
                                ui.selectable_value(
                                    &mut self.settings.interval_secs,
                                    c,
                                    format!("{c}s"),
                                );
                            }
                        });
                });
                ui.add_space(8.0);

                ui.heading("启动与后台");
                ui.checkbox(&mut self.settings.autostart, "开机自启动");
                ui.checkbox(&mut self.settings.silent_probe, "后台静默探测");
                ui.add_space(8.0);

                ui.heading("窗口行为");
                ui.checkbox(&mut self.settings.close_to_tray, "关闭时缩小到托盘");
                ui.checkbox(&mut self.settings.minimize_to_tray, "最小化时隐藏到托盘");
                ui.add_space(12.0);

                if ui.button("保存设置").clicked() {
                    self.apply_settings();
                    self.settings_open = false;
                }
            });
        self.settings_open = open;
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
        let ctx = ui.ctx().clone();

        // —— 窗口/托盘事件 ——
        if self.handle_viewport_events(&ctx) {
            // 用户请求退出:发送关闭命令放行
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        // —— 静默探测(Auto 刷新合并) ——
        self.maybe_silent_probe(&ctx);
        if self.settings.auto_refresh
            && self
                .last_scan
                .is_some_and(|t| t.elapsed() >= Duration::from_secs(self.settings.interval_secs))
        {
            self.do_scan();
        }

        // —— 设置窗口 ——
        self.settings_window(&ctx);

        // —— 顶部工具栏 ——
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading("IPProbe");
                ui.label("自动探测本机 IPv4 / IPv6 地址");
                ui.separator();
                if ui.button("刷新").clicked() {
                    self.do_scan();
                    self.refresh_public_ipv4();
                }
                ui.checkbox(&mut self.settings.auto_refresh, "自动刷新");
                egui::ComboBox::from_id_salt("interval")
                    .selected_text(format!("{}s", self.settings.interval_secs))
                    .show_ui(ui, |ui| {
                        for c in INTERVAL_CHOICES {
                            ui.selectable_value(&mut self.settings.interval_secs, c, format!("{c}s"));
                        }
                    });
                ui.separator();
                if ui.button("设置").clicked() {
                    self.settings_open = true;
                }
            });
            ui.add_space(4.0);
        });

        // —— 底部状态栏 ——
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
                ui.label(format!(
                    "公网出口: {}",
                    self.public_v4.as_deref().unwrap_or("无")
                ));
                ui.separator();
                ui.label(format!("上次扫描 {}", self.last_scan_text()));
            });
            if let Some(e) = &self.scan_error {
                ui.colored_label(egui::Color32::LIGHT_RED, format!("扫描出错: {e}"));
            }
            ui.add_space(4.0);
        });

        // —— 中央接口列表 ——
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
