//! 系统托盘图标与右键菜单。
//! 使用 tray-icon(Windows/macOS/Linux)与 muda 菜单库。
//! 非 Windows 平台为降级空实现,便于跨平台编译。

use eframe::egui;

/// 托盘右键菜单的菜单项 ID。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    /// 显示主窗口。
    Show,
    /// 完全退出程序。
    Quit,
}

/// 托盘模块的公开接口(平台无关)。
pub struct TrayController {
    /// Windows 平台持有真实的托盘对象(仅用于保持存活),其他平台为 None。
    #[cfg(windows)]
    #[allow(dead_code)]
    inner: Option<tray_icon::TrayIcon>,
    #[cfg(not(windows))]
    inner: (),
}

#[cfg(windows)]
fn build_menu() -> tray_icon::menu::Menu {
    use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};

    let menu = Menu::new();
    let show = MenuItem::with_id("show", "显示主窗口", true, None);
    let quit = MenuItem::with_id("quit", "退出", true, None);
    let separator = PredefinedMenuItem::separator();
    let _ = menu.append_items(&[&show, &separator, &quit]);
    menu
}

/// 生成一个简单的应用图标(RGBA)。
/// 先尝试从系统可选路径读取 ico,失败则退回纯色方块。
#[cfg(windows)]
fn make_icon() -> Option<tray_icon::Icon> {
    for path in [
        "C:\\Windows\\System32\\shell32.dll",
        "C:\\Windows\\System32\\imageres.dll",
    ] {
        if let Ok(icon) = tray_icon::Icon::from_path(path, Some((32, 32))) {
            return Some(icon);
        }
    }

    // 纯色 16x16 RGBA(蓝色方块,含 alpha)
    let size = 16usize;
    let mut rgba = Vec::with_capacity(size * size * 4);
    for y in 0..size {
        for x in 0..size {
            let edge = x == 0 || y == 0 || x == size - 1 || y == size - 1;
            if edge {
                rgba.extend_from_slice(&[255, 255, 255, 255]); // 白色边框
            } else {
                rgba.extend_from_slice(&[46, 108, 204, 255]); // 主题蓝
            }
        }
    }
    tray_icon::Icon::from_rgba(rgba, size as u32, size as u32).ok()
}

impl TrayController {
    /// 创建托盘控制器(Windows 上才真正创建托盘图标)。
    pub fn new() -> Self {
        #[cfg(windows)]
        {
            let tray = tray_icon::TrayIconBuilder::new()
                .with_tooltip("IPProbe - 网络地址探测器")
                .with_menu(Box::new(build_menu()))
                .with_icon(make_icon().unwrap_or_else(|| {
                    // from_path 一般不会失败,但作为兜底再试一次纯色
                    make_icon().expect("无法创建托盘图标")
                }))
                .build()
                .ok();
            Self { inner: tray }
        }

        #[cfg(not(windows))]
        {
            Self { inner: () }
        }
    }

    /// 处理托盘事件(菜单点击、左键点击等)。
    /// 调用时应传入当前应用状态,返回需要执行的动作。
    pub fn poll(&self) -> Option<TrayAction> {
        #[cfg(windows)]
        {
            // 菜单事件(仅统计;菜单项点击会进入 MenuEvent 通道)
            if let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
                return match event.id.as_ref() {
                    "show" => Some(TrayAction::Show),
                    "quit" => Some(TrayAction::Quit),
                    _ => None,
                };
            }
            // 托盘点击事件(左键单击默认显示窗口)
            if let Ok(event) = tray_icon::TrayIconEvent::receiver().try_recv() {
                if let tray_icon::TrayIconEvent::Click {
                    button: tray_icon::MouseButton::Left,
                    ..
                } = event
                {
                    return Some(TrayAction::Show);
                }
            }
            None
        }

        #[cfg(not(windows))]
        {
            let _ = &self.inner;
            None
        }
    }
}

impl Default for TrayController {
    fn default() -> Self {
        Self::new()
    }
}

/// 在 egui 中请求显示窗口。
pub fn show_main_window(ctx: &egui::Context) {
    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
}

/// 在 egui 中隐藏窗口。
pub fn hide_main_window(ctx: &egui::Context) {
    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
}

/// 在 egui 中退出程序。
/// 保留为公共 API;实际退出流程由托盘菜单触发。
#[allow(dead_code)]
pub fn quit_app(ctx: &egui::Context) {
    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
}
