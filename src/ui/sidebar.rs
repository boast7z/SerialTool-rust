use iced::widget;

use crate::backend::history::History;
use crate::backend::quick_commands::QuickCommands;
use crate::types::Message;

use super::{icons, styles};

/// 渲染右侧边栏：快捷命令列表（支持内联编辑）+ 发送历史列表（点击复用）。
pub fn view_sidebar<'a>(
    is_dark_mode: bool,
    history: &'a History,
    quick_commands: &'a QuickCommands,
    right_width: f32,
    editing: Option<&'a (usize, String, String, bool)>,
) -> widget::Column<'a, Message> {
    // 历史记录行的预览字符数：根据面板实际宽度动态计算，避免文本截断或溢出。
    // right_width - 44.0（按钮+内边距占用） - 66.0（"[HH:MM:SS] "时间前缀宽度）= 可用文本宽度
    // ÷ 6.5（等宽字体单字符平均宽度，像素估算）= 可容纳字符数，最少保留 6 个字符
    let preview_chars = ((right_width - 44.0 - 66.0) / 6.5).max(6.0) as usize;

    let icon_size = 12.0;
    let text_size = 12;

    // ── 快捷命令区 ─────────────────────────────────────
    // has_any：决定快捷命令区是显示列表还是"暂无快捷命令"占位文本。
    // editing.is_some() 也算"有内容"，确保编辑表单能正常展示（否则编辑中空槽会消失）。
    let has_any = quick_commands.items().iter().any(|c| !c.text.trim().is_empty())
        || editing.is_some();

    let quick_cmd_body: widget::Column<'a, Message> = if has_any {
        let mut col = widget::column![].spacing(4);
        for (i, cmd) in quick_commands.items().iter().enumerate() {
            let is_editing_this = editing.map(|(idx, ..)| *idx) == Some(i);

            // 内联编辑表单
            if is_editing_this {
                let (_, name, text, is_hex) = editing.unwrap();
                col = col.push(
                    widget::column![
                        widget::text_input("名称...", name)
                            .on_input(Message::QuickCommandEditName)
                            .size(11).width(iced::Fill),
                        widget::text_input("内容...", text)
                            .on_input(Message::QuickCommandEditText)
                            .size(11).width(iced::Fill),
                        widget::row![
                            widget::checkbox(*is_hex)
                                .label("HEX")
                                .on_toggle(Message::QuickCommandEditHex)
                                .text_size(10),
                            widget::space::Space::new().width(iced::Fill),
                            widget::button(widget::text("保存").size(10))
                                .on_press(Message::QuickCommandSaveEdit)
                                .style(styles::button_primary).padding(3),
                            widget::button(widget::text("取消").size(10))
                                .on_press(Message::QuickCommandCancelEdit)
                                .style(styles::button_subtle).padding(3),
                        ]
                        .spacing(4).align_y(iced::Alignment::Center),
                    ]
                    .spacing(3)
                    .padding(4),
                );
                continue;
            }

            // 空槽位且非编辑中：跳过
            if cmd.text.trim().is_empty() {
                continue;
            }

            let label = if cmd.name.trim().is_empty() {
                format!("命令 {}", i + 1)
            } else {
                cmd.name.clone()
            };

            col = col.push(
                widget::row![
                    widget::button(widget::text(label).size(text_size))
                        .on_press(Message::QuickCommandSend(i))
                        .style(styles::button_subtle)
                        .width(iced::Fill)
                        .padding(4),
                    widget::button(widget::text("✎").size(10))
                        .on_press(Message::QuickCommandStartEdit(i))
                        .style(styles::button_subtle).padding(3),
                    widget::button(widget::text("✕").size(10))
                        .on_press(Message::QuickCommandDelete(i))
                        .style(styles::button_subtle).padding(3),
                ]
                .spacing(2).align_y(iced::Alignment::Center),
            );
        }
        col
    } else {
        widget::column![
            widget::text("暂无快捷命令").size(text_size)
                .color(styles::COLOR_DIM)
        ]
    };

    // ── 历史记录区 ─────────────────────────────────────
    let history_body: widget::Column<'a, Message> = if !history.items().is_empty() {
        let mut col = widget::column![].spacing(2);
        for (i, item) in history.items().iter().enumerate() {
            // 用 chars() 而非字节索引，避免在多字节 Unicode（如中文）的字节边界处切断导致 panic
            let preview: String = item.text.chars().take(preview_chars).collect();
            let preview = if item.text.chars().count() > preview_chars {
                format!("{}…", preview)
            } else {
                preview
            };
            let label = format!("[{}] {}", item.time, preview);
            col = col.push(
                widget::button(widget::text(label).size(10))
                    .on_press(Message::HistoryReuse(i))
                    .style(styles::button_subtle)
                    .width(iced::Fill)
                    .padding(2),
            );
        }
        col
    } else {
        widget::column![
            widget::text("暂无历史记录").size(text_size)
                .color(styles::COLOR_DIM)
        ]
    };

    widget::column![
        // 标题栏
        widget::row![
            widget::button(widget::text("▶").size(11))
                .on_press(Message::ToggleRightPanel)
                .style(styles::button_subtle)
                .padding(3),
            icons::timer(icon_size, is_dark_mode),
            widget::text("快捷操作").size(14),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center),
        widget::rule::horizontal(1).style(styles::rule_style),

        // 快捷命令标题行
        widget::row![
            widget::text("快捷命令").size(11)
                .color(styles::COLOR_SECTION_LABEL),
            widget::space::Space::new().width(iced::Fill),
            widget::button(
                widget::row![icons::trash(icon_size, is_dark_mode), widget::text("清空").size(10)].spacing(3)
            )
            .on_press(Message::ClearQuickCommands)
            .style(styles::button_subtle)
            .padding(2),
        ]
        .align_y(iced::Alignment::Center),
        quick_cmd_body,
        widget::rule::horizontal(1).style(styles::rule_style),

        // 历史记录
        widget::row![
            widget::text("发送历史").size(11)
                .color(styles::COLOR_SECTION_LABEL),
            widget::space::Space::new().width(iced::Fill),
            widget::button(
                widget::row![icons::trash(icon_size, is_dark_mode), widget::text("清空").size(10)].spacing(3)
            )
            .on_press(Message::ClearHistory)
            .style(styles::button_subtle)
            .padding(2),
        ]
        .align_y(iced::Alignment::Center),

        widget::scrollable(history_body).height(iced::Fill),
    ]
    .padding(10)
    .spacing(8)
}
