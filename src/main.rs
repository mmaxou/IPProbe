//! IPProbe 入口:配置 eframe 窗口并启动 egui 应用。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod autostart;
mod bark;
mod netinfo;
mod settings;
mod tray;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        // 界面使用显式加载的 CJK 字体,关闭系统字体自动收集(可省数百 MB 内存),
        // 同时关闭系统字体回退,避免 eframe 的 MissingSystemFontsWarning 提示。
        system_font_fallback: false,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([720.0, 520.0])
            .with_min_inner_size([520.0, 360.0])
            .with_title(app::APP_TITLE),
        ..Default::default()
    };

    eframe::run_native(
        app::APP_TITLE,
        options,
        Box::new(|cc| {
            // 跟随 Windows 系统深浅色模式(浅色/深色自动切换)
            cc.egui_ctx.set_theme(egui::ThemePreference::System);
            setup_cjk_font(&cc.egui_ctx);
            Ok(Box::new(app::IPProbeApp::default()))
        }),
    )
}

/// 加载系统中文字体,保证中文界面正常显示;找不到时回退 egui 默认字体。
/// 优先选择体积最小的可用字体文件,减少常驻内存占用。
fn setup_cjk_font(ctx: &egui::Context) {
    const CANDIDATES: &[&str] = &[
        "C:\\Windows\\Fonts\\Deng.ttf",
        "C:\\Windows\\Fonts\\simhei.ttf",
        "C:\\Windows\\Fonts\\msyh.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
    ];

    // 找出磁盘上存在且字节数最小的字体文件
    let mut best: Option<(std::path::PathBuf, u64)> = None;
    for path in CANDIDATES {
        if let Ok(meta) = std::fs::metadata(path) {
            let len = meta.len();
            if best.as_ref().map_or(true, |(_, n)| len < *n) {
                best = Some((std::path::PathBuf::from(path), len));
            }
        }
    }

    if let Some((path, _)) = best {
        if let Ok(bytes) = std::fs::read(&path) {
            let mut fonts = egui::FontDefinitions::default();
            fonts
                .font_data
                .insert("cjk".to_owned(), egui::FontData::from_owned(bytes).into());
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts.families.entry(family).or_default().push("cjk".to_owned());
            }
            ctx.set_fonts(fonts);
        }
    }
}
