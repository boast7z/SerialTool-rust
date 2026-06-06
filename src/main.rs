// Release 构建隐藏 Windows 控制台窗口（不弹出黑框）；Debug 构建保留控制台以便查看 println!/eprintln! 输出
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod backend;
mod types;
mod ui;

use ui::AppState;

/// 从编译时嵌入的 PNG 字节生成窗口图标。
/// 返回 None 时 iced 会使用系统默认图标，不影响启动。
fn window_icon() -> Option<iced::window::Icon> {
    let bytes = include_bytes!("../icons/128x128.png");
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    iced::window::icon::from_rgba(img.into_raw(), w, h).ok()
}

fn main() -> iced::Result {
    iced::application(AppState::new, ui::update, ui::view)
        .theme(|state: &AppState| ui::theme::get_theme(&state.monitor))
        .subscription(ui::subscription)
        .title("串口调试助手")
        .window(iced::window::Settings {
            icon: window_icon(),
            ..Default::default()
        })
        .run()
}
