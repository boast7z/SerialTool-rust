use iced::widget;

use crate::types::{LineEnding, Message, TerminalState};

use super::{icons, styles};

/// 渲染中间终端面板：接收区（带搜索/滚动）+ 发送区（编辑器 + 工具栏）。
/// 接收区在 auto_scroll=true 时使用 anchor_bottom()（iced 内置，始终固定到底部显示最新内容）；
/// 关闭自动滚动后改为普通 Scrollable 并监听 on_scroll 事件以记录当前位置。
pub fn view_terminal<'a>(
    state: &'a TerminalState,
    is_dark_mode: bool,
    font_size: f32,
) -> widget::Column<'a, Message> {
    let line_ending_list = vec![LineEnding::None, LineEnding::CrLf, LineEnding::Lf, LineEnding::Cr];

    let icon_size = 12.0;
    let text_size = 12;
    let gap = 8;

    // 过滤行
    let lines: Vec<&str> = if state.search_text.is_empty() {
        state.received_lines.iter().map(|s| s.as_str()).collect()
    } else {
        state.received_lines
            .iter()
            .filter(|l| l.contains(&state.search_text))
            .map(|s| s.as_str())
            .collect()
    };

    // 搜索计数标签
    let search_count: iced::Element<'a, Message> = if !state.search_text.is_empty() {
        widget::text(format!("{} 条", lines.len())).size(10)
            .color(styles::COLOR_SUCCESS)
            .into()
    } else {
        widget::Space::new().into()
    };

    // 接收区内容：text_editor 只读模式，支持文本选中和 Ctrl+C 复制。
    // 内容由 update() 中的 rebuild_receive_content() 维护；
    // auto_scroll=true 时光标始终在末尾，关闭后视图冻结供用户自由选取。
    let rx_content: iced::Element<'a, Message> =
        widget::text_editor(&state.receive_content)
            .on_action(Message::EditReceive)
            .font(iced::Font::MONOSPACE)
            .size(font_size)
            .height(iced::Fill)
            .padding(8)
            .style(styles::rx_editor)
            .into();

    widget::column![
        // 接收区工具栏
        widget::row![
            icons::download(icon_size, is_dark_mode),
            widget::text("接收").size(text_size),
            widget::text(format!("RX: {} B", state.rx_count))
                .size(10)
                .color(styles::COLOR_SUCCESS),
            widget::space::Space::new().width(iced::Fill),
            widget::checkbox(state.hex_display)
                .label("HEX").on_toggle(Message::HexDisplayToggled).text_size(text_size),
            widget::checkbox(state.show_timestamp)
                .label("时间戳").on_toggle(Message::TimestampToggled).text_size(text_size),
            widget::checkbox(state.auto_scroll)
                .label("自动滚动").on_toggle(Message::AutoScrollToggled).text_size(text_size),
            widget::button(
                widget::text(if state.pause_receive { "▶ 继续" } else { "⏸ 暂停" }).size(text_size)
            )
            .on_press(Message::PauseReceiveToggled(!state.pause_receive))
            .style(if state.pause_receive { styles::button_primary } else { styles::button_subtle })
            .padding(4),
            widget::button(
                widget::row![icons::trash(icon_size, is_dark_mode), widget::text("清空").size(text_size)].spacing(4)
            )
            .on_press(Message::ClearReceived).style(styles::button_subtle).padding(4),
        ]
        .spacing(gap)
        .align_y(iced::Alignment::Center),

        // 搜索栏
        widget::row![
            widget::text_input("搜索...", &state.search_text)
                .on_input(Message::SearchTextChanged).width(iced::Fill).size(text_size),
            search_count,
        ]
        .spacing(gap)
        .align_y(iced::Alignment::Center),

        // 接收区主体（text_editor 自带背景和边框，无需外层 container）
        rx_content,

        // 发送区工具栏（第一行）
        widget::row![
            icons::upload(icon_size, is_dark_mode),
            widget::text("发送").size(text_size),
            widget::text(format!("TX: {} B", state.tx_count))
                .size(10)
                .color(styles::COLOR_ACCENT_TX),
            widget::space::Space::new().width(iced::Fill),
            widget::checkbox(state.hex_send)
                .label("HEX").on_toggle(Message::HexSendToggled).text_size(text_size),
            widget::text("结束符:").size(11),
            widget::pick_list(line_ending_list, Some(state.line_ending.clone()), Message::LineEndingSelected)
                .width(60).text_size(text_size),
        ]
        .spacing(gap)
        .align_y(iced::Alignment::Center),

        // 发送区工具栏（第二行）
        widget::row![
            widget::checkbox(state.timed_send)
                .label("定时发送").on_toggle(Message::TimedSendToggled).text_size(text_size),
            widget::text("间隔(ms):").size(11),
            widget::text_input("1000", &state.timed_send_interval)
                .on_input(Message::TimedSendIntervalChanged).width(60).size(text_size),
        ]
        .spacing(gap)
        .align_y(iced::Alignment::Center),

        // 发送框
        widget::text_editor(&state.send_content)
            .placeholder("输入要发送的数据...")
            .on_action(Message::EditSend)
            .height(90),

        // 底部按钮栏
        widget::row![
            widget::button(
                widget::row![icons::send(icon_size, is_dark_mode), widget::text("发送").size(text_size)].spacing(4)
            )
            .on_press(Message::SendMessage).style(styles::button_primary).padding(6),
            widget::button(
                widget::row![icons::eraser(icon_size, is_dark_mode), widget::text("清空发送").size(text_size)].spacing(4)
            )
            .on_press(Message::ClearSend).style(styles::button_subtle).padding(6),
            widget::space::Space::new().width(iced::Fill),
            widget::button(
                widget::row![icons::timer(icon_size, is_dark_mode), widget::text("存为快捷").size(text_size)].spacing(4)
            )
            .on_press(Message::SaveAsQuickCommand).style(styles::button_subtle).padding(6),
            widget::button(
                widget::row![icons::arrow_down_to_line(icon_size, is_dark_mode), widget::text("保存日志").size(text_size)].spacing(4)
            )
            .on_press(Message::SaveLog).style(styles::button_subtle).padding(6),
        ]
        .spacing(gap)
    ]
    .spacing(6)
}
