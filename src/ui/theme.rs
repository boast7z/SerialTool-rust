use iced::Theme;

use crate::types::SerialMonitor;

/// 根据深色模式状态返回对应的 iced 主题。
/// 亮色：CatppuccinLatte（暖白色调，对比度柔和）
/// 深色：TokyoNight（蓝紫色调，适合长时间使用）
pub fn get_theme(state: &SerialMonitor) -> Theme {
    if state.is_dark_mode {
        Theme::TokyoNight
    } else {
        Theme::CatppuccinLatte
    }
}
