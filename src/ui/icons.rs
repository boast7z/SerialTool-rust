use iced::widget::{self, svg};

use super::styles;

/// SVG 图标构建辅助：从编译时嵌入的 SVG 字节生成着色图标。
/// `is_dark` 决定图标颜色（深色模式用浅色，亮色模式用深色），颜色值由 styles::icon_color 统一管理。
fn icon(svg_data: &'static str, size: f32, is_dark: bool) -> widget::Svg<'static> {
    let handle = svg::Handle::from_memory(svg_data.as_bytes());
    let color = styles::icon_color(is_dark);
    widget::svg(handle)
        .width(size)
        .height(size)
        .style(move |_theme, _status| svg::Style {
            color: Some(color),
        })
}

/// USB 图标（当前 UI 未使用，保留备用）
#[allow(dead_code)]
pub fn usb(size: f32, is_dark: bool) -> widget::Svg<'static> {
    icon(include_str!("../../assets/icons/usb.svg"), size, is_dark)
}

pub fn settings(size: f32, is_dark: bool) -> widget::Svg<'static> {
    icon(include_str!("../../assets/icons/settings.svg"), size, is_dark)
}

pub fn plug(size: f32, is_dark: bool) -> widget::Svg<'static> {
    icon(include_str!("../../assets/icons/plug.svg"), size, is_dark)
}

pub fn download(size: f32, is_dark: bool) -> widget::Svg<'static> {
    icon(include_str!("../../assets/icons/download.svg"), size, is_dark)
}

pub fn upload(size: f32, is_dark: bool) -> widget::Svg<'static> {
    icon(include_str!("../../assets/icons/upload.svg"), size, is_dark)
}

pub fn trash(size: f32, is_dark: bool) -> widget::Svg<'static> {
    icon(include_str!("../../assets/icons/trash-2.svg"), size, is_dark)
}

pub fn eraser(size: f32, is_dark: bool) -> widget::Svg<'static> {
    icon(include_str!("../../assets/icons/eraser.svg"), size, is_dark)
}

pub fn send(size: f32, is_dark: bool) -> widget::Svg<'static> {
    icon(include_str!("../../assets/icons/send.svg"), size, is_dark)
}

pub fn arrow_down_to_line(size: f32, is_dark: bool) -> widget::Svg<'static> {
    icon(
        include_str!("../../assets/icons/arrow-down-to-line.svg"),
        size,
        is_dark,
    )
}

/// 时钟图标（当前 UI 未使用，保留备用；计时发送场景可能用到）
#[allow(dead_code)]
pub fn clock(size: f32, is_dark: bool) -> widget::Svg<'static> {
    icon(include_str!("../../assets/icons/clock.svg"), size, is_dark)
}

pub fn timer(size: f32, is_dark: bool) -> widget::Svg<'static> {
    icon(include_str!("../../assets/icons/timer.svg"), size, is_dark)
}
