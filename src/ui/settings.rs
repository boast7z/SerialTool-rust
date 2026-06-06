use iced::widget;

use crate::backend::app_settings::AppSettings;
use crate::types::{DataBits, FlowControl, Message, Parity, PortConfig, SendMode, StopBits, TimestampFormat};

use super::{icons, styles};

/// 波特率下拉列表的预设值（涵盖低速传感器到高速模块的常用范围）。
/// 用户也可在 combo_box 中直接键入自定义值，不受此列表限制。
pub const BAUD_PRESETS: &[&str] = &[
    "300", "1200", "2400", "4800", "9600", "19200", "38400",
    "57600", "115200", "230400", "460800", "576000", "921600",
];

// ── 串口参数面板 ──────────────────────────────────────────────────────────────

/// 渲染左侧串口配置面板：端口选择、波特率、帧参数、DTR/RTS、防复位开关、连接按钮。
/// 已连接时各参数切换为只读文本（防止修改参数导致与实际串口状态不一致）。
/// `show_elevate_hint` 为 true 时，在连接按钮下方额外显示提权操作区。
pub fn view_port_config<'a>(
    port: &'a PortConfig,
    is_connected: bool,
    is_dark_mode: bool,
    show_elevate_hint: bool,
    ch341_fix_needed: bool,
    ch341_fix_installing: bool,
    ch341_install_elapsed: u32,
) -> widget::Column<'a, Message> {
    let data_bits_list    = vec![DataBits::Five, DataBits::Six, DataBits::Seven, DataBits::Eight];
    let stop_bits_list    = vec![StopBits::One, StopBits::Two];
    let parity_list       = vec![Parity::None, Parity::Odd, Parity::Even];
    let flow_control_list = vec![FlowControl::None, FlowControl::Hardware, FlowControl::Software];

    let icon_size = 12.0;
    let text_size = 12;
    let label_w   = 55;
    let list_w    = 100;
    let gap       = 8;
    let dim       = styles::COLOR_DIM;

    // 连接中：各参数显示为只读文本；未连接：显示交互控件
    let port_selector: iced::Element<'a, Message> = if is_connected {
        widget::text(port.selected_port.as_deref().unwrap_or("-"))
            .size(text_size).width(list_w).color(dim).into()
    } else {
        widget::pick_list(port.port_list.clone(), port.selected_port.clone(), Message::PortSelected)
            .width(list_w).text_size(text_size).into()
    };
    let refresh_btn: iced::Element<'a, Message> = {
        let b = widget::button(widget::text("刷新").size(11)).padding(3);
        if is_connected { b.into() } else { b.on_press(Message::RefreshPorts).into() }
    };

    let baud_selector: iced::Element<'a, Message> = if is_connected {
        widget::text(&port.selected_baud)
            .size(text_size).width(list_w).color(dim).into()
    } else {
        widget::combo_box(
            &port.baud_combo_state, "输入或选择...",
            Some(&port.selected_baud), Message::BaudSelected,
        )
        .on_input(Message::BaudInputChanged)
        .width(list_w).size(text_size).into()
    };

    let data_bits_sel: iced::Element<'a, Message> = if is_connected {
        widget::text(port.selected_data_bits.as_ref().map(|v| v.to_string()).unwrap_or_default())
            .size(text_size).width(list_w).color(dim).into()
    } else {
        widget::pick_list(data_bits_list, port.selected_data_bits.clone(), Message::DataBitsSelected)
            .width(list_w).text_size(text_size).into()
    };

    let stop_bits_sel: iced::Element<'a, Message> = if is_connected {
        widget::text(port.selected_stop_bits.as_ref().map(|v| v.to_string()).unwrap_or_default())
            .size(text_size).width(list_w).color(dim).into()
    } else {
        widget::pick_list(stop_bits_list, port.selected_stop_bits.clone(), Message::StopBitsSelected)
            .width(list_w).text_size(text_size).into()
    };

    let parity_sel: iced::Element<'a, Message> = if is_connected {
        widget::text(port.selected_parity.as_ref().map(|v| v.to_string()).unwrap_or_default())
            .size(text_size).width(list_w).color(dim).into()
    } else {
        widget::pick_list(parity_list, port.selected_parity.clone(), Message::ParitySelected)
            .width(list_w).text_size(text_size).into()
    };

    let flow_sel: iced::Element<'a, Message> = if is_connected {
        widget::text(port.selected_flow_control.as_ref().map(|v| v.to_string()).unwrap_or_default())
            .size(text_size).width(list_w).color(dim).into()
    } else {
        widget::pick_list(flow_control_list, port.selected_flow_control.clone(), Message::FlowControlSelected)
            .width(list_w).text_size(text_size).into()
    };

    let modem_checkbox: iced::Element<'a, Message> = {
        let cb = widget::checkbox(port.disable_modem_handshake)
            .label("打开时禁用 Modem 流控")
            .text_size(11);
        if is_connected { cb.into() } else { cb.on_toggle(Message::DisableModemHandshakeToggled).into() }
    };

    widget::column![
        // 标题栏
        widget::row![
            icons::settings(icon_size, is_dark_mode),
            widget::text("串口配置").size(14),
            widget::space::Space::new().width(iced::Fill),
            widget::toggler(is_dark_mode)
                .label("深色")
                .on_toggle(|_| Message::ToggleTheme)
                .text_size(11),
            widget::button(icons::settings(icon_size, is_dark_mode))
                .on_press(Message::ShowAppSettings)
                .style(styles::button_subtle)
                .padding(3),
            widget::button(widget::text("◀").size(11))
                .on_press(Message::ToggleLeftPanel)
                .style(styles::button_subtle)
                .padding(3),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center),
        widget::rule::horizontal(1).style(styles::rule_style),

        // 端口
        widget::row![
            widget::text("端口:").size(text_size).width(label_w),
            port_selector,
            refresh_btn,
        ]
        .spacing(gap).align_y(iced::Alignment::Center),

        // 波特率
        widget::row![
            widget::text("波特率:").size(text_size).width(label_w),
            baud_selector,
        ]
        .spacing(gap).align_y(iced::Alignment::Center),

        // 数据位
        widget::row![
            widget::text("数据位:").size(text_size).width(label_w),
            data_bits_sel,
        ]
        .spacing(gap).align_y(iced::Alignment::Center),

        // 停止位
        widget::row![
            widget::text("停止位:").size(text_size).width(label_w),
            stop_bits_sel,
        ]
        .spacing(gap).align_y(iced::Alignment::Center),

        // 校验位
        widget::row![
            widget::text("校验位:").size(text_size).width(label_w),
            parity_sel,
        ]
        .spacing(gap).align_y(iced::Alignment::Center),

        // 流控
        widget::column![
            widget::row![
                widget::text("流控:").size(text_size).width(label_w),
                flow_sel,
            ]
            .spacing(gap).align_y(iced::Alignment::Center),
            widget::text(match &port.selected_flow_control {
                Some(FlowControl::Hardware) => "硬件: RTS/CTS",
                Some(FlowControl::Software) => "软件: XON/XOFF",
                _ => "",
            })
            .size(11)
            .color(dim),
        ]
        .spacing(2),

        // DTR / RTS
        // 未连接时：决定打开串口后的初始电平（ON = 打开时拉高，会触发目标复位）
        // 已连接时：实时切换信号电平
        widget::column![
            widget::row![
                widget::text("DTR:").size(text_size).width(28),
                widget::toggler(port.dtr_enabled).on_toggle(Message::DtrToggled),
                widget::space::Space::new().width(12),
                widget::text("RTS:").size(text_size).width(28),
                widget::toggler(port.rts_enabled).on_toggle(Message::RtsToggled),
            ]
            .spacing(gap).align_y(iced::Alignment::Center),
            if !is_connected {
                widget::text("开串口时初始电平由此开关决定")
                    .size(10).color(dim)
            } else {
                widget::text("").size(10)
            },
        ]
        .spacing(2),

        // 防复位
        widget::column![
            modem_checkbox,
            widget::text("如打开串口时设备意外重启，请勾选此项")
                .size(10)
                .color(dim),
        ]
        .spacing(3),

        widget::rule::horizontal(1).style(styles::rule_style),

        // 打开/关闭串口
        widget::row![
            widget::button(
                widget::row![
                    icons::plug(icon_size, is_dark_mode),
                    widget::text(if is_connected { "关闭串口" } else { "打开串口" }).size(text_size),
                ]
                .spacing(4)
            )
            .on_press(Message::TogglePort)
            .style(styles::button_primary)
            .padding(6),
            widget::text(if is_connected { "● 已连接" } else { "○ 未连接" })
                .size(text_size)
                .color(if is_connected {
                    styles::COLOR_CONNECTED
                } else {
                    styles::COLOR_DIM
                }),
        ]
        .spacing(gap).align_y(iced::Alignment::Center),

        // 提权提示区（仅在权限错误后显示）
        if show_elevate_hint {
            // 按钮文字在编译期由目标平台决定
            #[cfg(windows)]
            let label = "以管理员身份重启";
            #[cfg(not(windows))]
            let label = "申请串口访问权限";

            widget::column![
                widget::rule::horizontal(1).style(styles::rule_style),
                widget::text("串口访问被系统拒绝").size(10).color(styles::COLOR_WARNING),
                widget::button(widget::text(label).size(text_size))
                    .on_press(Message::ElevatePrivileges)
                    .style(styles::button_primary)
                    .padding(6),
                widget::text(
                    if cfg!(windows) {
                        "将触发 UAC，批准后以管理员身份重启"
                    } else {
                        "将通过 pkexec 配置 udev 规则，无需重新登录"
                    }
                )
                .size(10)
                .color(styles::COLOR_DIM),
            ]
            .spacing(4)
        } else {
            widget::column![]
        },

        // CH341 DTR 修复提示（Linux 专属，仅在检测到问题时显示）
        {
            #[cfg(target_os = "linux")]
            let card: widget::Column<'a, Message> = if ch341_fix_needed || ch341_fix_installing {
                let (btn_label, progress_text): (String, &str) = if ch341_fix_installing {
                    (
                        format!("构建中... {}s", ch341_install_elapsed),
                        "正在编译内核模块，请勿关闭（约需 1-3 分钟）",
                    )
                } else {
                    (
                        "一键修复".to_string(),
                        "需要 gcc / make / linux-headers（无需网络）",
                    )
                };
                let btn = widget::button(widget::text(btn_label).size(11))
                    .style(styles::button_primary)
                    .padding(6);
                let btn: iced::Element<'a, Message> = if ch341_fix_installing {
                    btn.into()
                } else {
                    btn.on_press(Message::FixCh341Driver).into()
                };
                widget::column![
                    widget::rule::horizontal(1).style(styles::rule_style),
                    widget::text("CH340/CH341 DTR 问题").size(10).color(styles::COLOR_WARNING),
                    widget::text("打开串口时会触发设备复位（需要安装修复驱动）")
                        .size(10)
                        .color(styles::COLOR_DIM),
                    btn,
                    widget::text(progress_text)
                        .size(10)
                        .color(styles::COLOR_DIM),
                ]
                .spacing(4)
            } else {
                widget::column![]
            };

            #[cfg(not(target_os = "linux"))]
            let card: widget::Column<'a, Message> = {
                let _ = (ch341_fix_needed, ch341_fix_installing, ch341_install_elapsed);
                widget::column![]
            };

            card
        },
    ]
    .spacing(12)
}

// ── 应用设置面板 ──────────────────────────────────────────────────────────────

/// 渲染应用设置面板：发送模式、时间戳格式、字号、日志目录、配置文件操作、版本信息。
pub fn view_app_settings<'a>(
    app_settings: &'a AppSettings,
    _is_dark_mode: bool,
    reset_confirm_pending: bool,
) -> widget::Column<'a, Message> {
    let text_size = 12;
    let gap       = 8;

    let log_dir_str = app_settings.log_dir.to_string_lossy().into_owned();

    let log_warning: iced::Element<'a, Message> = if !app_settings.log_dir_valid() {
        widget::text("⚠ 目录不存在")
            .size(10)
            .color(styles::COLOR_WARNING)
            .into()
    } else {
        widget::Space::new().into()
    };

    let reset_btn: iced::Element<'a, Message> = if reset_confirm_pending {
        widget::row![
            widget::text("确认？").size(11).color(styles::COLOR_WARNING),
            widget::button(widget::text("是").size(11))
                .on_press(Message::ResetAllSettings)
                .style(styles::button_primary).padding(4),
            widget::button(widget::text("否").size(11))
                .on_press(Message::CancelResetSettings)
                .style(styles::button_subtle).padding(4),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center)
        .into()
    } else {
        widget::button(widget::text("恢复默认设置").size(11))
            .on_press(Message::RequestResetAllSettings)
            .style(styles::button_subtle)
            .padding(4)
            .into()
    };

    widget::column![
        // 标题栏：返回按钮 + 标题
        widget::row![
            widget::button(widget::text("← 返回").size(text_size))
                .on_press(Message::ShowPortConfig)
                .style(styles::button_subtle)
                .padding(4),
            widget::text("应用设置").size(14),
            widget::space::Space::new().width(iced::Fill),
            widget::button(widget::text("◀").size(11))
                .on_press(Message::ToggleLeftPanel)
                .style(styles::button_subtle)
                .padding(3),
        ]
        .spacing(gap).align_y(iced::Alignment::Center),
        widget::rule::horizontal(1).style(styles::rule_style),

        // 发送模式
        widget::text("发送模式").size(11)
            .color(styles::COLOR_SECTION_LABEL),
        widget::radio(
            "Enter 发送 / Ctrl+Enter 换行",
            SendMode::EnterSend,
            Some(app_settings.send_mode),
            Message::SendModeChanged,
        )
        .text_size(text_size),
        widget::radio(
            "Enter 换行 / Ctrl+Enter 发送",
            SendMode::EnterNewline,
            Some(app_settings.send_mode),
            Message::SendModeChanged,
        )
        .text_size(text_size),

        widget::rule::horizontal(1).style(styles::rule_style),

        // 时间戳格式
        widget::text("时间戳格式").size(11)
            .color(styles::COLOR_SECTION_LABEL),
        widget::radio(
            "HH:MM:SS",
            TimestampFormat::TimeOnly,
            Some(app_settings.timestamp_format),
            Message::TimestampFormatChanged,
        ).text_size(text_size),
        widget::radio(
            "HH:MM:SS.mmm（毫秒）",
            TimestampFormat::TimeMsec,
            Some(app_settings.timestamp_format),
            Message::TimestampFormatChanged,
        ).text_size(text_size),
        widget::radio(
            "YYYY-MM-DD HH:MM:SS",
            TimestampFormat::DateTime,
            Some(app_settings.timestamp_format),
            Message::TimestampFormatChanged,
        ).text_size(text_size),

        widget::rule::horizontal(1).style(styles::rule_style),

        // 接收区字号
        widget::text("接收区字号").size(11)
            .color(styles::COLOR_SECTION_LABEL),
        widget::row![
            widget::button(widget::text("A-").size(text_size))
                .on_press(Message::FontSizeChanged((app_settings.font_size - 1.0).max(8.0)))
                .style(styles::button_subtle).padding(4),
            widget::text(format!("{:.0}px", app_settings.font_size))
                .size(text_size)
                .width(40),
            widget::button(widget::text("A+").size(text_size))
                .on_press(Message::FontSizeChanged((app_settings.font_size + 1.0).min(28.0)))
                .style(styles::button_subtle).padding(4),
            widget::button(widget::text("重置").size(10))
                .on_press(Message::FontSizeReset)
                .style(styles::button_subtle).padding(4),
        ]
        .spacing(6).align_y(iced::Alignment::Center),

        widget::rule::horizontal(1).style(styles::rule_style),

        // 日志保存目录
        widget::row![
            widget::text("日志保存目录").size(11)
                .color(styles::COLOR_SECTION_LABEL),
            widget::space::Space::new().width(iced::Fill),
            widget::toggler(app_settings.log_dir_custom)
                .label("自定义路径")
                .on_toggle(Message::LogDirCustomToggled)
                .text_size(10),
        ]
        .align_y(iced::Alignment::Center),
        if app_settings.log_dir_custom {
            widget::column![
                widget::text_input("日志保存路径...", &log_dir_str)
                    .on_input(Message::LogDirChanged)
                    .size(text_size)
                    .width(iced::Fill),
                log_warning,
                widget::row![
                    widget::button(widget::text("保存").size(11))
                        .on_press(Message::LogDirSaved)
                        .style(styles::button_primary).padding(4),
                    widget::button(widget::text("重置").size(11))
                        .on_press(Message::LogDirReset)
                        .style(styles::button_subtle).padding(4),
                ]
                .spacing(6),
            ]
            .spacing(4)
        } else {
            widget::column![
                widget::text("保存至程序所在目录")
                    .size(text_size)
                    .color(styles::COLOR_DIM),
                widget::text("开启开关可自定义保存路径")
                    .size(10)
                    .color(iced::Color::from_rgba(0.65, 0.65, 0.65, 1.0)),
            ]
            .spacing(2)
        },

        widget::rule::horizontal(1).style(styles::rule_style),

        // 面板宽度（只读展示，自动保存）
        widget::text("面板宽度（拖拽后自动保存）").size(11)
            .color(styles::COLOR_SECTION_LABEL),
        widget::row![
            widget::text(format!("左: {:.0}px", app_settings.left_width)).size(text_size),
            widget::space::Space::new().width(iced::Fill),
            widget::text(format!("右: {:.0}px", app_settings.right_width)).size(text_size),
        ]
        .align_y(iced::Alignment::Center),

        widget::rule::horizontal(1).style(styles::rule_style),

        // 配置文件操作
        widget::row![
            widget::button(widget::text("打开配置文件").size(11))
                .on_press(Message::OpenConfigFile)
                .style(styles::button_subtle).padding(4),
            reset_btn,
        ]
        .spacing(gap),

        widget::rule::horizontal(1).style(styles::rule_style),

        // 版本信息
        widget::row![
            widget::column![
                widget::text(format!("v{}", env!("CARGO_PKG_VERSION"))).size(12),
                widget::text("串口调试助手").size(10)
                    .color(iced::Color::from_rgba(0.55, 0.55, 0.55, 1.0)),
            ]
            .spacing(1),
            widget::space::Space::new().width(iced::Fill),
            // TODO: 接入更新检查逻辑（当前无 on_press，按钮不可用）
            widget::button(widget::text("检查更新").size(11))
                .style(styles::button_subtle).padding(4),
        ]
        .spacing(gap).align_y(iced::Alignment::Center),
    ]
    .spacing(10)
}
