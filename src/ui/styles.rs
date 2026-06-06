use iced::widget;
use iced::theme::Base;

pub const COLOR_SUCCESS: iced::Color = iced::Color { r: 0.45, g: 0.75, b: 0.45, a: 1.0 };      // 绿色：RX 字节计数、连接成功提示
pub const COLOR_ACCENT_TX: iced::Color = iced::Color { r: 0.45, g: 0.55, b: 0.85, a: 1.0 };   // 蓝色：TX 字节计数
pub const COLOR_DIM: iced::Color = iced::Color { r: 0.55, g: 0.55, b: 0.55, a: 1.0 };         // 灰色：连接中只读参数、辅助说明文字
pub const COLOR_WARNING: iced::Color = iced::Color { r: 0.90, g: 0.60, b: 0.10, a: 1.0 };     // 橙色：目录不存在警告、重置确认提示
pub const COLOR_SECTION_LABEL: iced::Color = iced::Color { r: 0.45, g: 0.45, b: 0.55, a: 1.0 }; // 蓝灰色：设置面板各分组标签
pub const COLOR_CONNECTED: iced::Color = iced::Color { r: 0.20, g: 0.80, b: 0.20, a: 1.0 };   // 亮绿色："● 已连接"状态指示

fn is_dark(theme: &iced::Theme) -> bool {
    matches!(theme.mode(), iced::theme::Mode::Dark)
}

pub fn icon_color(is_dark: bool) -> iced::Color {
    if is_dark {
        iced::Color::from_rgb(0.85, 0.85, 0.88)
    } else {
        iced::Color::from_rgb(0.35, 0.35, 0.40)
    }
}

pub fn panel_container(theme: &iced::Theme) -> widget::container::Style {
    widget::container::Style {
        background: Some(iced::Background::Color(if is_dark(theme) {
            iced::Color::from_rgb(0.15, 0.15, 0.18)
        } else {
            iced::Color::from_rgb(0.97, 0.97, 0.98)
        })),
        border: iced::Border {
            radius: 8.0.into(),
            width: 1.0,
            color: if is_dark(theme) {
                iced::Color::from_rgba(0.25, 0.25, 0.30, 0.5)
            } else {
                iced::Color::from_rgba(0.75, 0.75, 0.80, 0.5)
            },
        },
        ..Default::default()
    }
}

#[allow(dead_code)]
pub fn content_area(theme: &iced::Theme) -> widget::container::Style {
    if is_dark(theme) {
        widget::container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgb(
                0.08, 0.08, 0.10,
            ))),
            border: iced::Border {
                radius: 6.0.into(),
                width: 1.0,
                color: iced::Color::from_rgba(0.2, 0.2, 0.25, 0.3),
            },
            ..Default::default()
        }
    } else {
        widget::container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgb(
                0.92, 0.92, 0.94,
            ))),
            border: iced::Border {
                radius: 6.0.into(),
                width: 1.0,
                color: iced::Color::from_rgba(0.7, 0.7, 0.75, 0.5),
            },
            ..Default::default()
        }
    }
}

pub fn button_primary(
    _theme: &iced::Theme,
    status: widget::button::Status,
) -> widget::button::Style {
    match status {
        widget::button::Status::Active => widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgb(
                0.25, 0.55, 0.95,
            ))),
            text_color: iced::Color::WHITE,
            border: iced::Border {
                radius: 6.0.into(),
                width: 1.0,
                color: iced::Color::from_rgb(0.20, 0.45, 0.85),
            },
            ..Default::default()
        },
        widget::button::Status::Hovered => widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgb(
                0.30, 0.60, 0.98,
            ))),
            text_color: iced::Color::WHITE,
            border: iced::Border {
                radius: 6.0.into(),
                width: 1.0,
                color: iced::Color::from_rgb(0.25, 0.50, 0.90),
            },
            ..Default::default()
        },
        widget::button::Status::Pressed => widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgb(
                0.20, 0.50, 0.90,
            ))),
            text_color: iced::Color::WHITE,
            border: iced::Border {
                radius: 6.0.into(),
                width: 1.0,
                color: iced::Color::from_rgb(0.15, 0.40, 0.80),
            },
            ..Default::default()
        },
        widget::button::Status::Disabled => widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                0.25, 0.55, 0.95, 0.5,
            ))),
            text_color: iced::Color::from_rgba(1.0, 1.0, 1.0, 0.5),
            border: iced::Border {
                radius: 6.0.into(),
                width: 1.0,
                color: iced::Color::from_rgba(0.20, 0.45, 0.85, 0.5),
            },
            ..Default::default()
        },
    }
}

pub fn button_subtle(
    theme: &iced::Theme,
    status: widget::button::Status,
) -> widget::button::Style {
    let palette = theme.palette();
    match status {
        widget::button::Status::Active => widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
            text_color: palette.text,
            border: iced::Border {
                radius: 6.0.into(),
                width: 1.0,
                color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.1),
            },
            ..Default::default()
        },
        widget::button::Status::Hovered => widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                0.0, 0.0, 0.0, 0.05,
            ))),
            text_color: palette.text,
            border: iced::Border {
                radius: 6.0.into(),
                width: 1.0,
                color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.15),
            },
            ..Default::default()
        },
        widget::button::Status::Pressed => widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                0.0, 0.0, 0.0, 0.1,
            ))),
            text_color: palette.text,
            border: iced::Border {
                radius: 6.0.into(),
                width: 1.0,
                color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.2),
            },
            ..Default::default()
        },
        widget::button::Status::Disabled => widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
            text_color: iced::Color::from_rgba(
                palette.text.r,
                palette.text.g,
                palette.text.b,
                0.3,
            ),
            border: iced::Border {
                radius: 6.0.into(),
                width: 1.0,
                color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.05),
            },
            ..Default::default()
        },
    }
}

/// 接收区只读 text_editor 的外观：与 content_area 容器配色一致，无输入框视觉效果
pub fn rx_editor(theme: &iced::Theme, _status: widget::text_editor::Status) -> widget::text_editor::Style {
    let (bg, border_color) = if is_dark(theme) {
        (
            iced::Color::from_rgb(0.08, 0.08, 0.10),
            iced::Color::from_rgba(0.2, 0.2, 0.25, 0.3),
        )
    } else {
        (
            iced::Color::from_rgb(0.92, 0.92, 0.94),
            iced::Color::from_rgba(0.7, 0.7, 0.75, 0.5),
        )
    };
    widget::text_editor::Style {
        background: iced::Background::Color(bg),
        border: iced::Border { radius: 6.0.into(), width: 1.0, color: border_color },
        placeholder: iced::Color::from_rgba(
            theme.palette().text.r,
            theme.palette().text.g,
            theme.palette().text.b,
            0.4,
        ),
        value: theme.palette().text,
        selection: iced::Color::from_rgba(0.25, 0.55, 0.95, 0.35),
    }
}

pub fn rule_style(theme: &iced::Theme) -> widget::rule::Style {
    if is_dark(theme) {
        widget::rule::Style {
            color: iced::Color::from_rgba(0.3, 0.3, 0.35, 0.4),
            radius: 0.5.into(),
            fill_mode: widget::rule::FillMode::Full,
            snap: true,
        }
    } else {
        widget::rule::Style {
            color: iced::Color::from_rgba(0.7, 0.7, 0.75, 0.4),
            radius: 0.5.into(),
            fill_mode: widget::rule::FillMode::Full,
            snap: true,
        }
    }
}
