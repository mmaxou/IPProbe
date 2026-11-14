//! IPProbe 的 Iced 应用:界面布局、自动刷新与状态展示,以及设置/托盘/后台静默探测。
//!
//! 采用 Iced 的 Elm 架构:State(状态) / Message(消息) / update / view / subscription。

use std::time::{Duration, Instant};

use iced::widget::{button, checkbox, column, container, pick_list, row, scrollable, text, text_input, Column, Space};
use iced::{Element, Length, Subscription, Task, Theme};

use crate::netinfo::{self, AddrScope, IfaceInfo, ScanSummary};
use crate::settings::{self, AppSettings};
use crate::tray::{TrayAction, TrayController};

pub const APP_TITLE: &str = "IPProbe - 网络地址探测器";

const INTERVAL_CHOICES: [u64; 4] = [3, 5, 10, 30];
const BARK_HOURS: [u32; 24] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
];
const BARK_MINUTES: [u32; 12] = [0, 5, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55];

/// 应用消息。
#[derive(Debug, Clone)]
pub enum Message {
    /// 周期心跳(用于自动刷新、静默探测、Bark 定时与系统主题检测)。
    Tick,
    /// 窗口事件(由订阅监听)。
    WindowEvents(iced::window::Id, iced::window::Event),
    /// 托盘事件轮询(高频,用于及时响应托盘菜单/点击与最小化隐藏)。
    TrayPoll,
    /// 立即刷新。
    Refresh,
    /// 自动刷新开关。
    ToggleAutoRefresh(bool),
    /// 刷新间隔变化。
    IntervalChanged(u64),
    /// 打开设置页。
    OpenSettings,
    /// 放弃修改并返回主界面。
    CloseSettings,
    /// 保存设置并返回主界面。
    SaveSettings,
    /// 开机自启动开关。
    ToggleAutostart(bool),
    /// 后台静默探测开关。
    ToggleSilentProbe(bool),
    /// 关闭时缩到托盘。
    ToggleCloseToTray(bool),
    /// 最小化时隐藏到托盘。
    ToggleMinimizeToTray(bool),
    /// Bark 每日通知开关。
    ToggleBarkEnabled(bool),
    /// Bark 服务地址输入。
    BarkUrlChanged(String),
    /// Bark 设备 Key 输入。
    BarkTokenChanged(String),
    /// Bark 发送小时。
    BarkHourChanged(u32),
    /// Bark 发送分钟。
    BarkMinuteChanged(u32),
    /// 发送 Bark 测试通知。
    BarkTestSend,
}

/// 应用状态。
pub struct State {
    /// 已注册的窗口 id(单窗口应用,通常只有一个)。
    window_ids: Vec<iced::window::Id>,
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
    /// 托盘控制器(保持存活以提供系统托盘;事件经 TrayPoll 轮询)。
    #[allow(dead_code)]
    tray: TrayController,
    /// 是否正在显示设置页。
    settings_open: bool,
    /// 用户是否通过托盘“退出”。
    requested_quit: bool,
    /// 主窗口当前是否隐藏(托盘/关闭时)。
    window_hidden: bool,
    /// 上一个静默探测时间。
    last_silent_scan: Option<Instant>,
    /// Bark 上次成功发送的日期键(yyyy-mm-dd)。
    last_bark_date: Option<String>,
    /// Bark 最近一次发送/测试结果。
    bark_feedback: Option<String>,
    /// 当前主题(跟随系统深浅色)。
    theme: Theme,
}

impl State {
    /// 创建应用并执行首次探测。
    pub fn new() -> (Self, Task<Message>) {
        let loaded = settings::load();
        let mut app = Self {
            window_ids: Vec::new(),
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
            window_hidden: false,
            last_silent_scan: None,
            last_bark_date: None,
            bark_feedback: None,
            theme: Theme::default(),
        };
        app.sync_autostart_on_startup();
        app.do_scan();
        app.refresh_public_ipv4();
        (app, Task::none())
    }

    /// 主窗口 id(首个注册的窗口)。
    fn main_window_id(&self) -> Option<iced::window::Id> {
        self.window_ids.first().copied()
    }

    /// 启动时同步自启动状态。
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
        let fresh = self
            .last_pub_fetch
            .is_some_and(|t| t.elapsed() < Duration::from_secs(60));
        if !fresh {
            self.public_v4 = netinfo::public_outbound_ipv4();
            self.last_pub_fetch = Some(Instant::now());
        }
    }

    /// “默认出口 IPv4”展示文本:优先公网出口 IP;失败回退本地接口地址并标注“(本地)”。
    fn egress_v4_label(&self) -> String {
        if let Some(pub_ip) = self.public_v4.as_deref() {
            pub_ip.to_string()
        } else if let Some(local) = self.outbound_v4 {
            format!("{local} (本地)")
        } else {
            "无".to_owned()
        }
    }

    /// 上次扫描时间文本。
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

    /// 检测当前系统深浅色(仅 Windows;其他平台返回 None)。
    fn system_dark() -> Option<bool> {
        #[cfg(windows)]
        {
            use winreg::enums::HKEY_CURRENT_USER;
            use winreg::RegKey;
            let hkcu = RegKey::predef(HKEY_CURRENT_USER);
            match hkcu.open_subkey(
                r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
            ) {
                Ok(key) => match key.get_value::<u32, _>("AppsUseLightTheme") {
                    Ok(v) => Some(v == 0),
                    Err(_) => None,
                },
                Err(_) => None,
            }
        }
        #[cfg(not(windows))]
        {
            None
        }
    }

    /// 同步系统主题。
    fn sync_theme(&mut self) {
        if let Some(dark) = Self::system_dark() {
            self.theme = if dark { Theme::Dark } else { Theme::Light };
        }
    }

    /// 处理托盘动作,返回需要执行的任务(若有)。
    fn handle_tray_action(&mut self, action: TrayAction) -> Option<Task<Message>> {
        match action {
            TrayAction::Show => {
                self.window_hidden = false;
                self.main_window_id()
                    .map(|id| iced::window::change_mode(id, iced::window::Mode::Windowed))
            }
            TrayAction::Quit => {
                self.requested_quit = true;
                self.main_window_id().map(|id| iced::window::close(id))
            }
        }
    }

    /// 最小化时隐藏到托盘:通过 Win32 检测主窗口是否处于最小化状态。
    /// 注意:窗口隐藏后不再处于最小化状态,不会重复触发。
    fn maybe_minimize_to_tray(&mut self) -> Task<Message> {
        if !self.settings.minimize_to_tray || self.window_hidden {
            return Task::none();
        }
        let Some(id) = self.main_window_id() else {
            return Task::none();
        };
        if Self::main_window_is_minimized() {
            self.window_hidden = true;
            return iced::window::change_mode(id, iced::window::Mode::Hidden);
        }
        Task::none()
    }

    /// 主窗口是否处于最小化且可见状态(仅 Windows;其他平台恒为 false)。
    #[cfg(windows)]
    fn main_window_is_minimized() -> bool {
        use std::sync::atomic::{AtomicIsize, Ordering};
        use windows_sys::Win32::Foundation::HWND;
        use windows_sys::Win32::System::Threading::GetCurrentProcessId;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            EnumWindows, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
        };

        static FOUND: AtomicIsize = AtomicIsize::new(0);

        // windows-sys 中 BOOL 即 i32,LPARAM 即 isize。
        unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: isize) -> i32 {
            let target_pid = lparam as u32;
            let mut win_pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, &mut win_pid);
            if win_pid == target_pid {
                FOUND.store(hwnd as isize, Ordering::SeqCst);
                return 0; // 停止枚举
            }
            1
        }

        let pid = unsafe { GetCurrentProcessId() };
        let _ = unsafe { EnumWindows(Some(enum_proc), pid as isize) };
        let found = FOUND.load(Ordering::SeqCst);
        if found == 0 {
            return false;
        }
        let hwnd = found as HWND;
        unsafe { IsIconic(hwnd) != 0 && IsWindowVisible(hwnd) != 0 }
    }

    /// 主窗口是否处于最小化状态(非 Windows 平台)。
    #[cfg(not(windows))]
    fn main_window_is_minimized() -> bool {
        false
    }
    /// 后台静默探测:窗口隐藏期间仍按间隔探测。
    /// 由于订阅心跳始终运行,隐藏时同样会收到 Tick。
    fn maybe_silent_probe(&mut self) {
        if !self.settings.auto_refresh {
            return;
        }
        let interval = Duration::from_secs(self.settings.interval_secs.max(1));
        let due = self
            .last_silent_scan
            .is_some_and(|t| t.elapsed() >= interval);
        if due {
            self.do_scan();
            self.refresh_public_ipv4();
        }
    }

    /// Bark 每日定时通知:到达设定时间且当日未发送时发送。
    fn maybe_bark_report(&mut self) {
        if !self.settings.bark_enabled {
            return;
        }
        let Some(now) = crate::bark::local_now() else {
            return;
        };
        if now.hour != self.settings.bark_hour || now.minute != self.settings.bark_minute {
            return;
        }
        if self.last_bark_date.as_deref() == Some(now.date_key.as_str()) {
            return; // 当日已发送
        }
        // 发送前确保数据最新
        self.do_scan();
        self.refresh_public_ipv4();
        let ips = self.collect_ips_text();
        match crate::bark::send(
            &self.settings.bark_url,
            &self.settings.bark_token,
            "IPProbe 网络探测",
            &ips,
        ) {
            Ok(()) => {
                self.last_bark_date = Some(now.date_key.clone());
                self.bark_feedback = Some(format!(
                    "已于 {}:{:02} 发送成功",
                    now.hour, now.minute
                ));
            }
            Err(e) => {
                self.bark_feedback = Some(format!("Bark 发送失败: {e}"));
            }
        }
    }

    /// 汇总探测到的所有 IP 地址。
    fn collect_ips_text(&self) -> String {
        let mut lines = Vec::new();
        for iface in &self.ifaces {
            for a in &iface.addrs {
                lines.push(format!("{} [{}]: {}", iface.name, a.scope.label(), a.addr));
            }
        }
        if let Some(public) = &self.public_v4 {
            lines.push(format!("公网出口 IPv4: {public}"));
        }
        if let Some(a) = self.outbound_v4 {
            lines.push(format!("默认出口(接口) IPv4: {a}"));
        }
        if let Some(a) = self.outbound_v6 {
            lines.push(format!("默认出口 IPv6: {a}"));
        }
        if lines.is_empty() {
            "未检测到 IP 地址".to_owned()
        } else {
            lines.join("\n")
        }
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

    /// 更新逻辑。
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {
                self.sync_theme();
                self.maybe_silent_probe();
                self.maybe_bark_report();
                Task::none()
            }

            Message::WindowEvents(id, event) => {
                if !self.window_ids.contains(&id) {
                    self.window_ids.push(id);
                }
                match event {
                    iced::window::Event::CloseRequested => {
                        if self.settings.close_to_tray && !self.requested_quit {
                            self.window_hidden = true;
                            return iced::window::change_mode(
                                id,
                                iced::window::Mode::Hidden,
                            );
                        }
                        self.requested_quit = true;
                        return iced::window::close(id);
                    }
                    _ => {}
                }
                Task::none()
            }

            Message::TrayPoll => {
                if let Some(action) = crate::tray::poll_action() {
                    if let Some(task) = self.handle_tray_action(action) {
                        return task;
                    }
                }
                self.maybe_minimize_to_tray()
            }

            Message::Refresh => {
                self.do_scan();
                self.refresh_public_ipv4();
                Task::none()
            }

            Message::ToggleAutoRefresh(v) => {
                self.settings.auto_refresh = v;
                Task::none()
            }
            Message::IntervalChanged(v) => {
                self.settings.interval_secs = v;
                Task::none()
            }

            Message::OpenSettings => {
                self.settings_open = true;
                Task::none()
            }
            Message::CloseSettings => {
                self.settings_open = false;
                Task::none()
            }
            Message::SaveSettings => {
                self.apply_settings();
                self.settings_open = false;
                Task::none()
            }

            Message::ToggleAutostart(v) => {
                self.settings.autostart = v;
                Task::none()
            }
            Message::ToggleSilentProbe(v) => {
                self.settings.silent_probe = v;
                Task::none()
            }
            Message::ToggleCloseToTray(v) => {
                self.settings.close_to_tray = v;
                Task::none()
            }
            Message::ToggleMinimizeToTray(v) => {
                self.settings.minimize_to_tray = v;
                Task::none()
            }

            Message::ToggleBarkEnabled(v) => {
                self.settings.bark_enabled = v;
                Task::none()
            }
            Message::BarkUrlChanged(v) => {
                self.settings.bark_url = v;
                Task::none()
            }
            Message::BarkTokenChanged(v) => {
                self.settings.bark_token = v;
                Task::none()
            }
            Message::BarkHourChanged(v) => {
                self.settings.bark_hour = v;
                Task::none()
            }
            Message::BarkMinuteChanged(v) => {
                self.settings.bark_minute = v;
                Task::none()
            }

            Message::BarkTestSend => {
                let (base, token) = (
                    self.settings.bark_url.clone(),
                    self.settings.bark_token.clone(),
                );
                self.bark_feedback = Some(
                    match crate::bark::send(
                        &base,
                        &token,
                        "IPProbe 测试",
                        "这是一条来自 IPProbe 的测试通知",
                    ) {
                        Ok(()) => "测试通知发送成功".to_owned(),
                        Err(e) => format!("发送失败: {e}"),
                    },
                );
                Task::none()
            }
        }
    }

    /// 订阅列表:周期心跳(5s)、托盘轮询(80ms)、窗口事件。。
    pub fn subscription(&self) -> Subscription<Message> {
        let tick = iced::time::every(Duration::from_secs(5)).map(|_| Message::Tick);
        let tray_poll = iced::time::every(Duration::from_millis(80)).map(|_| Message::TrayPoll);
        let win = iced::window::events().map(|(id, event)| Message::WindowEvents(id, event));
        Subscription::batch([tick, tray_poll, win])
    }

    /// 当前主题(跟随系统深浅色)。
    pub fn theme(&self) -> Theme {
        self.theme.clone()
    }

    /// 主界面。
    fn main_view(&self) -> Element<'_, Message> {
        let top = row![
            text("IPProbe").size(20),
            button("刷新").on_press(Message::Refresh),
            checkbox("自动刷新", self.settings.auto_refresh)
                .on_toggle(Message::ToggleAutoRefresh),
            pick_list(
                INTERVAL_CHOICES,
                Some(self.settings.interval_secs),
                Message::IntervalChanged
            ),
            Space::with_width(Length::Fill),
            button("设置").on_press(Message::OpenSettings),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .padding(8);

        let v4 = self.egress_v4_label();
        let v6 = self
            .outbound_v6
            .map(|a| a.to_string())
            .unwrap_or_else(|| "无".to_owned());

        let bottom_text = if let Some(e) = &self.scan_error {
            format!(
                "接口 {} | 启动 {} | IPv4 {} | IPv6 {} | 默认出口: IPv4 {v4} · IPv6 {v6} | 上次扫描 {} | 扫描出错: {e}",
                self.summary.iface_count,
                self.summary.up_count,
                self.summary.ipv4_count,
                self.summary.ipv6_count,
                self.last_scan_text()
            )
        } else {
            format!(
                "接口 {} | 启动 {} | IPv4 {} | IPv6 {} | 默认出口: IPv4 {v4} · IPv6 {v6} | 上次扫描 {}",
                self.summary.iface_count,
                self.summary.up_count,
                self.summary.ipv4_count,
                self.summary.ipv6_count,
                self.last_scan_text()
            )
        };
        let bottom = container(text(bottom_text).size(12))
            .width(Length::Fill)
            .padding(8);

        let list = scrollable(self.interface_list()).height(Length::Fill);

        let content = column![top, list, bottom];

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// 接口列表。
    fn interface_list(&self) -> Element<'_, Message> {
        if self.ifaces.is_empty() {
            return text("未检测到任何网络接口").into();
        }

        // 按“启动状态优先、名称升序”排列索引,元素借用 self,避免局部克隆导致借用逃逸。
        let mut order: Vec<usize> = (0..self.ifaces.len()).collect();
        order.sort_by(|&a, &b| {
            let ia = &self.ifaces[a];
            let ib = &self.ifaces[b];
            ib.is_up.cmp(&ia.is_up).then(ia.name.cmp(&ib.name))
        });

        let mut col = Column::new().spacing(6).padding(8);

        for idx in order {
            let intf = &self.ifaces[idx];
            let (up_text, up_color) = if intf.is_up {
                ("UP", iced::Color::from_rgb(0.18, 0.80, 0.44))
            } else {
                ("DOWN", iced::Color::from_rgb(0.60, 0.60, 0.60))
            };

            let mut card = Column::new().spacing(4);

            let header = row![
                text(&intf.name).size(16),
                text(format!("#{}", intf.index)).size(12),
                text(up_text).color(up_color),
                Space::with_width(Length::Fill),
                text(format!(
                    "IPv4 x{}/IPv6 x{}",
                    intf.ipv4().count(),
                    intf.ipv6().count()
                ))
                .size(12),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center);
            card = card.push(header);

            for a in &intf.addrs {
                card = card.push(row![
                    text(a.addr.to_string()).size(14),
                    text(a.scope.label()).color(scope_color(a.scope)),
                ]
                .spacing(8));
            }

            col = col.push(container(card).padding(6).style(iced::widget::container::rounded_box));
        }

        col.into()
    }

    /// 设置页。
    fn settings_view(&self) -> Element<'_, Message> {
        let mut content = Column::new().spacing(12).padding(16);

        content = content.push(text("设置").size(24));

        content = content.push(text("常规").size(18));
        content = content.push(
            row![
                checkbox("自动刷新", self.settings.auto_refresh)
                    .on_toggle(Message::ToggleAutoRefresh),
                text("刷新间隔"),
                pick_list(
                    INTERVAL_CHOICES,
                    Some(self.settings.interval_secs),
                    Message::IntervalChanged
                ),
            ]
            .spacing(8),
        );

        content = content.push(text("启动与后台").size(18));
        content = content.push(
            row![
                checkbox("开机自启动", self.settings.autostart)
                    .on_toggle(Message::ToggleAutostart),
                checkbox("后台静默探测", self.settings.silent_probe)
                    .on_toggle(Message::ToggleSilentProbe),
            ]
            .spacing(16),
        );

        content = content.push(text("窗口行为").size(18));
        content = content.push(
            row![
                checkbox("关闭时缩小到托盘", self.settings.close_to_tray)
                    .on_toggle(Message::ToggleCloseToTray),
                checkbox("最小化时隐藏到托盘", self.settings.minimize_to_tray)
                    .on_toggle(Message::ToggleMinimizeToTray),
            ]
            .spacing(16),
        );

        content = content.push(text("Bark 通知(iOS)").size(18));
        content = content.push(
            checkbox("启用每日 IP 通知", self.settings.bark_enabled)
                .on_toggle(Message::ToggleBarkEnabled),
        );
        content = content.push(
            row![text("服务地址"), text_input("https://api.day.app", &self.settings.bark_url)
                .on_input(Message::BarkUrlChanged),]
            .spacing(8),
        );
        content = content.push(
            row![text("设备 Key"), text_input("Bark 设备 Key", &self.settings.bark_token)
                .on_input(Message::BarkTokenChanged),]
            .spacing(8),
        );
        content = content.push(
            row![
                text("每天"),
                pick_list(BARK_HOURS, Some(self.settings.bark_hour), Message::BarkHourChanged),
                text("点"),
                pick_list(
                    BARK_MINUTES,
                    Some(self.settings.bark_minute),
                    Message::BarkMinuteChanged
                ),
                text("分"),
                button("发送测试通知").on_press(Message::BarkTestSend),
            ]
            .spacing(8),
        );
        if let Some(feedback) = &self.bark_feedback {
            content = content.push(text(feedback).size(13));
        }

        content = content.push(Space::with_height(8));

        let actions = row![
            button("保存设置").on_press(Message::SaveSettings),
            button("放弃并返回").on_press(Message::CloseSettings),
        ]
        .spacing(12);

        content = content.push(actions);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// 视图。
    pub fn view(&self) -> Element<'_, Message> {
        if self.settings_open {
            self.settings_view()
        } else {
            self.main_view()
        }
    }
}

fn scope_color(scope: AddrScope) -> iced::Color {
    match scope {
        AddrScope::Loopback => iced::Color::from_rgb(0.52, 0.60, 0.69),
        AddrScope::LinkLocal => iced::Color::from_rgb(0.10, 0.74, 0.61),
        AddrScope::UniqueLocal => iced::Color::from_rgb(0.61, 0.35, 0.71),
        AddrScope::Private => iced::Color::from_rgb(0.90, 0.41, 0.13),
        AddrScope::Public => iced::Color::from_rgb(0.15, 0.68, 0.38),
    }
}
