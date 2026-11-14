//! IPProbe 现代 UI 样式:自定义主题与控件样式。
//!
//! 统一采用“圆角 + 半透明面板 + 柔和阴影 + 微妙渐变背景”的设计语言:
//! - 主题通过 `Theme::custom` 自定义调色板(区分深浅色);
//! - 所有自带控件(按钮/输入框/下拉/复选框/滚动条)均提供定制样式;
//! - 所有颜色均带 alpha 通道,叠加在窗口渐变背景上呈现通透感。

use iced::gradient::{Gradient, Linear};
use iced::widget::{button, checkbox, container, pick_list, scrollable, text_input};
use iced::{Background, Border, Color, Shadow, Theme, Vector};
use std::f32::consts::FRAC_PI_2;

// ---------------------------------------------------------------------------
// 主题与调色板
// ---------------------------------------------------------------------------

/// 生成跟随系统深浅色的自定义主题。
pub fn theme(dark: bool) -> Theme {
    if dark {
        Theme::custom(
            "IPProbe Dark".to_string(),
            iced::theme::Palette {
                background: color(0x14171E),
                text: color(0xEAEEF5),
                primary: color(0x6C8CFF),
                success: color(0x2FB47A),
                danger: color(0xF2555A),
            },
        )
    } else {
        Theme::custom(
            "IPProbe Light".to_string(),
            iced::theme::Palette {
                background: color(0xF2F5FB),
                text: color(0x1E293B),
                primary: color(0x4F6EF7),
                success: color(0x129A50),
                danger: color(0xE5484D),
            },
        )
    }
}

/// 单色(不透明)。
fn color(hex: u32) -> Color {
    Color::from_rgb8(
        ((hex >> 16) & 0xFF) as u8,
        ((hex >> 8) & 0xFF) as u8,
        (hex & 0xFF) as u8,
    )
}

/// 带透明度颜色。
fn color_a(hex: u32, a: f32) -> Color {
    Color::from_rgba8(
        ((hex >> 16) & 0xFF) as u8,
        ((hex >> 8) & 0xFF) as u8,
        (hex & 0xFF) as u8,
        a,
    )
}

/// 根据主题背景亮度判断是否深色。
fn is_dark(theme: &Theme) -> bool {
    let b = theme.palette().background;
    (0.299 * b.r + 0.587 * b.g + 0.114 * b.b) < 0.5
}

/// 两套配色(深/浅)的控制颜色。
struct C {
    surface: Color,       // 半透明面板底
    surface_strong: Color, // 更“实”的面板底(卡片)
    border: Color,
    border_focus: Color,
    text_muted: Color,
    scroller: Color,
    rail: Color,
    shadow: Color,
    overlay_bg: Color,
    overlay_selected: Color,
}

fn colors(dark: bool) -> C {
    if dark {
        C {
            surface: color_a(0xFFFFFF, 0.07),
            surface_strong: color_a(0xFFFFFF, 0.11),
            border: color_a(0xFFFFFF, 0.10),
            border_focus: color(0x6C8CFF),
            text_muted: color_a(0xEAEEF5, 0.55),
            scroller: color_a(0xFFFFFF, 0.32),
            rail: color_a(0xFFFFFF, 0.06),
            shadow: color_a(0x000000, 0.30),
            overlay_bg: color_a(0x1E232E, 0.96),
            overlay_selected: color_a(0x6C8CFF, 0.24),
        }
    } else {
        C {
            surface: color_a(0xFFFFFF, 0.55),
            surface_strong: color_a(0xFFFFFF, 0.72),
            border: color_a(0x0B1220, 0.08),
            border_focus: color(0x4F6EF7),
            text_muted: color_a(0x1E293B, 0.52),
            scroller: color_a(0x0B1220, 0.26),
            rail: color_a(0x0B1220, 0.05),
            shadow: color_a(0x0B1220, 0.08),
            overlay_bg: color_a(0xFFFFFF, 0.97),
            overlay_selected: color_a(0x4F6EF7, 0.14),
        }
    }
}

/// 构造圆角边框。
fn border(color: Color, width: f32, radius: f32) -> Border {
    Border {
        color,
        width,
        radius: iced::border::Radius::new(radius),
    }
}

// ---------------------------------------------------------------------------
// 容器样式
// ---------------------------------------------------------------------------

/// 窗口根容器:微妙垂直渐变 + 低透明度,营造现代质感。
pub fn root(theme: &Theme) -> container::Style {
    let dark = is_dark(theme);
    let (top, bottom) = if dark {
        (color_a(0x1B212C, 0.92), color_a(0x10131A, 0.95))
    } else {
        (color_a(0xEAF0FA, 0.90), color_a(0xF7F9FE, 0.95))
    };
    container::Style {
        text_color: None,
        background: Some(Background::Gradient(Gradient::Linear(
            Linear::new(FRAC_PI_2)
                .add_stop(0.0, top)
                .add_stop(1.0, bottom),
        ))),
        border: Border::default(),
        shadow: Shadow::default(),
    }
}

/// 通用半透明圆角面板(顶栏 / 分组设置区 / 底部状态栏)。
pub fn panel(theme: &Theme) -> container::Style {
    let c = colors(is_dark(theme));
    container::Style {
        text_color: None,
        background: Some(Background::Color(c.surface)),
        border: border(c.border, 1.0, 16.0),
        shadow: Shadow {
            color: c.shadow,
            offset: Vector::new(0.0, 6.0),
            blur_radius: 18.0,
        },
    }
}

/// 卡片(接口列表项):更实的半透明底,稍小圆角。
pub fn card(theme: &Theme) -> container::Style {
    let c = colors(is_dark(theme));
    container::Style {
        text_color: None,
        background: Some(Background::Color(c.surface_strong)),
        border: border(c.border, 1.0, 12.0),
        shadow: Shadow {
            color: c.shadow,
            offset: Vector::new(0.0, 3.0),
            blur_radius: 10.0,
        },
    }
}

// ---------------------------------------------------------------------------
// 按钮
// ---------------------------------------------------------------------------

/// 主按钮:实心主题色、大圆角。
pub fn button_primary(theme: &Theme, status: button::Status) -> button::Style {
    let dark = is_dark(theme);
    let c = colors(dark);
    let base = theme.palette().primary;
    let bg = match status {
        button::Status::Pressed => base.scale_alpha(0.85),
        button::Status::Hovered => base.scale_alpha(0.92),
        _ => base,
    };
    let mut style = button::Style {
        background: Some(Background::Color(bg)),
        text_color: if dark { color(0x0D1220) } else { Color::WHITE },
        border: border(Color::TRANSPARENT, 0.0, 11.0),
        shadow: Shadow {
            color: c.shadow,
            offset: Vector::new(0.0, 2.0),
            blur_radius: 8.0,
        },
    };
    if matches!(status, button::Status::Disabled) {
        style.background = Some(Background::Color(c.surface_strong));
        style.text_color = c.text_muted;
        style.shadow = Shadow::default();
    }
    style
}

/// 幽灵按钮:半透明底 + 细边框,悬停强调主题色。
pub fn button_ghost(theme: &Theme, status: button::Status) -> button::Style {
    let c = colors(is_dark(theme));
    let primary = theme.palette().primary;
    let mut style = button::Style {
        background: Some(Background::Color(c.surface)),
        text_color: primary,
        border: border(c.border, 1.0, 11.0),
        shadow: Shadow::default(),
    };
    match status {
        button::Status::Hovered => {
            style.background = Some(Background::Color(c.surface_strong));
            style.border.color = c.border_focus;
        }
        button::Status::Pressed => {
            style.background = Some(Background::Color(c.surface_strong));
        }
        button::Status::Disabled => {
            style.text_color = c.text_muted;
        }
        button::Status::Active => {}
    }
    style
}

// ---------------------------------------------------------------------------
// 输入框 / 下拉 / 复选框
// ---------------------------------------------------------------------------

/// 文本输入框:半透明底、圆角,聚焦时主题色描边。
pub fn input(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let c = colors(is_dark(theme));
    let palette = theme.palette();
    let mut style = text_input::Style {
        background: Background::Color(c.surface),
        border: border(c.border, 1.0, 10.0),
        icon: c.text_muted,
        placeholder: c.text_muted,
        value: palette.text,
        selection: color_a(0x4F6EF7, 0.30),
    };
    match status {
        text_input::Status::Focused => {
            style.background = Background::Color(c.surface_strong);
            style.border.color = c.border_focus;
        }
        text_input::Status::Hovered => {
            style.border.color = c.border_focus;
        }
        text_input::Status::Disabled => {
            style.value = c.text_muted;
        }
        text_input::Status::Active => {}
    }
    style
}

/// 下拉选择框:与输入框一致的设计语言。
pub fn pick(theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    let c = colors(is_dark(theme));
    let palette = theme.palette();
    let mut style = pick_list::Style {
        text_color: palette.text,
        placeholder_color: c.text_muted,
        handle_color: c.text_muted,
        background: Background::Color(c.surface),
        border: border(c.border, 1.0, 10.0),
    };
    match status {
        pick_list::Status::Hovered | pick_list::Status::Opened => {
            style.background = Background::Color(c.surface_strong);
            style.border.color = c.border_focus;
        }
        pick_list::Status::Active => {}
    }
    style
}
/// 下拉菜单:圆角半透明浮层,选中项高亮。
pub fn menu(theme: &Theme) -> iced::widget::overlay::menu::Style {
    let c = colors(is_dark(theme));
    let primary = theme.palette().primary;
    iced::widget::overlay::menu::Style {
        background: Background::Color(c.overlay_bg),
        border: border(c.border, 1.0, 10.0),
        text_color: theme.palette().text,
        selected_text_color: primary,
        selected_background: Background::Color(c.overlay_selected),
    }
}

/// 复选框:圆角方框,选中时主题色填充。
pub fn check(theme: &Theme, status: checkbox::Status) -> checkbox::Style {
    let c = colors(is_dark(theme));
    let palette = theme.palette();
    let checked = matches!(
        status,
        checkbox::Status::Active { is_checked: true }
            | checkbox::Status::Hovered { is_checked: true }
            | checkbox::Status::Disabled { is_checked: true }
    );
    let hovered = matches!(status, checkbox::Status::Hovered { .. });
    let bg = if checked {
        palette.primary
    } else if hovered {
        c.surface_strong
    } else {
        c.surface
    };
    checkbox::Style {
        background: Background::Color(bg),
        icon_color: if is_dark(theme) {
            color(0x0D1220)
        } else {
            Color::WHITE
        },
        border: border(
            if checked { palette.primary } else { c.border },
            1.0,
            5.0,
        ),
        text_color: Some(palette.text),
    }
}

// ---------------------------------------------------------------------------
// 滚动区
// ---------------------------------------------------------------------------

/// 接口列表滚动区:半透明圆角底 + 柔和滚动条。
pub fn scroll(theme: &Theme, _status: scrollable::Status) -> scrollable::Style {
    let c = colors(is_dark(theme));
    let rail = scrollable::Rail {
        background: Some(Background::Color(c.rail)),
        border: Border::default(),
        scroller: scrollable::Scroller {
            color: c.scroller,
            border: border(Color::TRANSPARENT, 0.0, 8.0),
        },
    };
    scrollable::Style {
        container: container::Style {
            text_color: None,
            background: Some(Background::Color(c.surface)),
            border: border(c.border, 1.0, 12.0),
            shadow: Shadow::default(),
        },
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
    }
}
