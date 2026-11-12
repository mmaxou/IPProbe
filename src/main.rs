//! IPProbe 入口:配置 eframe 窗口并启动 egui 应用。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod netinfo;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
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
            setup_cjk_font(&cc.egui_ctx);
            Ok(Box::new(app::IPProbeApp::default()))
        }),
    )
}

/// 加载系统中文字体,保证中文界面正常显示;找不到时回退 egui 默认字体。
fn setup_cjk_font(ctx: &egui::Context) {
    const CANDIDATES: &[&str] = &[
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\simhei.ttf",
        "C:\\Windows\\Fonts\\Deng.ttf",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
    ];

    for path in CANDIDATES {
        if let Ok(bytes) = std::fs::read(path) {
            let mut fonts = egui::FontDefinitions::default();
            fonts
                .font_data
                .insert("cjk".to_owned(), egui::FontData::from_owned(bytes).into());
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts.families.entry(family).or_default().push("cjk".to_owned());
            }
            ctx.set_fonts(fonts);
            return;
        }
    }
}
