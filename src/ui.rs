//! IPProbe 现代 UI 样式:自定义主题与控件样式。
//!
//! 设计语言:圆角 + 分层明暗 + 柔和描边,保持现代通透感。
//!
//! 实现约束(重要):softbuffer 后端采用“损坏区增量重绘”,半透明背景与阴影
//! 会在悬停重绘时反复叠加并拖影,且全窗渐变/大模糊开销高导致卡顿。
//! 因此根背景、面板、卡片与控件一律使用**不透明**叠层,悬停仅做纯色变化,
//! 阴影全部移除。保持圆角与层次感的同时避免渲染伪影。

use iced::widget::{button, checkbox, container, pick_list, scrollable, text_input};
use iced::{Background, Border, Color, Shadow, Theme};

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
                background: color(0xF3F6FB),
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

/// 带透明度颜色(仅用于细描边/选中高亮等一次性绘制)。
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
    panel_bg: Color,        // 面板/输入框等控件底(不透明)
    card_bg: Color,         // 卡片底(不透明)
    border: Color,
    border_focus: Color,
    text_muted: Color,
    primary_hover: Color,
    primary_pressed: Color,
    scroller: Color,
    rail: Color,
    overlay_bg: Color,
    overlay_selected: Color,
}

fn colors(dark: bool) -> C {
    if dark {
        C {
            panel_bg: color(0x1C212B),
            card_bg: color(0x222936),
            border: color_a(0xFFFFFF, 0.10),
            border_focus: color(0x6C8CFF),
            text_muted: color_a(0xEAEEF5, 0.55),
            primary_hover: color(0x7E9AFF),
            primary_pressed: color(0x5B76E8),
            scroller: color_a(0xFFFFFF, 0.30),
            rail: color_a(0xFFFFFF, 0.06),
            overlay_bg: color(0x1E232E),
            overlay_selected: color_a(0x6C8CFF, 0.24),
        }
    } else {
        C {
            panel_bg: color(0xFFFFFF),
            card_bg: color(0xFAFBFE),
            border: color_a(0x0B1220, 0.08),
            border_focus: color(0x4F6EF7),
            text_muted: color_a(0x1E293B, 0.52),
            primary_hover: color(0x425EE0),
            primary_pressed: color(0x3753D0),
            scroller: color_a(0x0B1220, 0.26),
            rail: color_a(0x0B1220, 0.05),
            overlay_bg: color(0xFFFFFF),
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

/// 窗口根容器:不透明主题底色,承载所有分层。
pub fn root(theme: &Theme) -> container::Style {
    let dark = is_dark(theme);
    let bg = if dark {
        color(0x14171E)
    } else {
        color(0xF3F6FB)
    };
    container::Style {
        text_color: None,
        background: Some(Background::Color(bg)),
        border: Border::default(),
        shadow: Shadow::default(),
    }
}

/// 通用圆角面板(顶栏 / 分组设置区 / 底部状态栏)。
pub fn panel(theme: &Theme) -> container::Style {
    let c = colors(is_dark(theme));
    container::Style {
        text_color: None,
        background: Some(Background::Color(c.panel_bg)),
        border: border(c.border, 1.0, 16.0),
        shadow: Shadow::default(),
    }
}

/// 卡片(接口列表项):略微不同的底色以形成层次。
pub fn card(theme: &Theme) -> container::Style {
    let c = colors(is_dark(theme));
    container::Style {
        text_color: None,
        background: Some(Background::Color(c.card_bg)),
        border: border(c.border, 1.0, 12.0),
        shadow: Shadow::default(),
    }
}

// ---------------------------------------------------------------------------
// 按钮
// ---------------------------------------------------------------------------

/// 主按钮:实心主题色、大圆角,悬停/按下做纯色变化。
pub fn button_primary(theme: &Theme, status: button::Status) -> button::Style {
    let dark = is_dark(theme);
    let c = colors(dark);
    let base = theme.palette().primary;
    let bg = match status {
        button::Status::Hovered => c.primary_hover,
        button::Status::Pressed => c.primary_pressed,
        _ => base,
    };
    let mut style = button::Style {
        background: Some(Background::Color(bg)),
        text_color: if dark { color(0x0D1220) } else { Color::WHITE },
        border: border(Color::TRANSPARENT, 0.0, 11.0),
        shadow: Shadow::default(),
    };
    if matches!(status, button::Status::Disabled) {
        style.background = Some(Background::Color(c.panel_bg));
        style.text_color = c.text_muted;
    }
    style
}

/// 幽灵按钮:面板色底 + 细边框,悬停强调主题色。
pub fn button_ghost(theme: &Theme, status: button::Status) -> button::Style {
    let c = colors(is_dark(theme));
    let primary = theme.palette().primary;
    let mut style = button::Style {
        background: Some(Background::Color(c.panel_bg)),
        text_color: primary,
        border: border(c.border, 1.0, 11.0),
        shadow: Shadow::default(),
    };
    match status {
        button::Status::Hovered => {
            style.background = Some(Background::Color(c.card_bg));
            style.border.color = c.border_focus;
        }
        button::Status::Pressed => {
            style.background = Some(Background::Color(c.card_bg));
            style.text_color = c.primary_pressed;
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

/// 文本输入框:面板色底、圆角,聚焦时主题色描边。
pub fn input(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let c = colors(is_dark(theme));
    let palette = theme.palette();
    let mut style = text_input::Style {
        background: Background::Color(c.panel_bg),
        border: border(c.border, 1.0, 10.0),
        icon: c.text_muted,
        placeholder: c.text_muted,
        value: palette.text,
        selection: color_a(0x4F6EF7, 0.30),
    };
    match status {
        text_input::Status::Focused => {
            style.background = Background::Color(c.card_bg);
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
        background: Background::Color(c.panel_bg),
        border: border(c.border, 1.0, 10.0),
    };
    match status {
        pick_list::Status::Hovered | pick_list::Status::Opened => {
            style.background = Background::Color(c.card_bg);
            style.border.color = c.border_focus;
        }
        pick_list::Status::Active => {}
    }
    style
}

/// 下拉菜单:圆角浮层,选中项高亮。
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
        c.card_bg
    } else {
        c.panel_bg
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

/// 接口列表滚动区:圆角底 + 柔和滚动条。
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
            background: Some(Background::Color(c.panel_bg)),
            border: border(c.border, 1.0, 12.0),
            shadow: Shadow::default(),
        },
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
    }
}
