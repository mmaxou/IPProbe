//! IPProbe 入口:配置 Iced 应用并启动。
//!
//! 渲染采用 Iced 的 tiny-skia 软件渲染器,无需 GPU 上下文,内存占用极低。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod autostart;
mod bark;
mod netinfo;
mod settings;
mod tray;

use iced::{Font, Size};

fn main() -> iced::Result {
    // 加载系统中文字体(体积最小者),保证中文界面正常显示。
    let cjk = find_cjk_font();

    let mut program = iced::application(app::APP_TITLE, app::State::update, app::State::view)
        .theme(app::State::theme)
        .subscription(app::State::subscription)
        .window(iced::window::Settings {
            size: Size::new(720.0, 520.0),
            min_size: Some(Size::new(520.0, 360.0)),
            // 关闭请求由应用层处理(缩到托盘),避免窗口直接被销毁。
            exit_on_close_request: false,
            ..Default::default()
        });

    if let Some((bytes, family)) = cjk {
        let family: &'static str = Box::leak(family.into_boxed_str());
        program = program.font(bytes).default_font(Font::with_name(family));
    }

    program.run_with(app::State::new)
}

/// 查找系统中体积最小的可用 CJK 字体文件,返回 (字体字节, 家族名)。
///
/// 优先 Windows 自带字体;Linux 开发环境回退到常见开源 CJK 字体。
fn find_cjk_font() -> Option<(Vec<u8>, String)> {
    const CANDIDATES: &[&str] = &[
        "C:\\Windows\\Fonts\\Deng.ttf",
        "C:\\Windows\\Fonts\\simhei.ttf",
        "C:\\Windows\\Fonts\\msyh.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
    ];

    let mut best: Option<(std::path::PathBuf, u64)> = None;
    for path in CANDIDATES {
        if let Ok(meta) = std::fs::metadata(path) {
            let len = meta.len();
            if best.as_ref().map_or(true, |(_, n)| len < *n) {
                best = Some((std::path::PathBuf::from(path), len));
            }
        }
    }

    let (path, _) = best?;
    let bytes = std::fs::read(&path).ok()?;
    let family = font_family_name(&bytes)?;
    Some((bytes, family))
}

/// 从 TTF/TTC 字体文件中提取家族名(优先 Windows 平台英文 locale)。
fn font_family_name(data: &[u8]) -> Option<String> {
    use ttf_parser::Face;
    for index in 0..16 {
        let Ok(face) = Face::parse(data, index) else {
            break;
        };
        for name in face.names() {
            if name.name_id == ttf_parser::name_id::FAMILY
                && name.platform_id == ttf_parser::PlatformId::Windows
                && name.language_id == 0x0409
            {
                if let Some(s) = name.to_string() {
                    if !s.is_empty() {
                        return Some(s);
                    }
                }
            }
        }
    }
    None
}
