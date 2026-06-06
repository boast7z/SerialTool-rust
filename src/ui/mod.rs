pub mod icons;
pub mod settings;
pub mod sidebar;
pub mod splitter;
pub mod styles;
pub mod terminal;
pub mod theme;

use iced::widget;
use iced::{Element, Event, Subscription};

use crate::backend;
use crate::backend::app_settings::AppSettings;
use crate::backend::history::History;
use crate::backend::quick_commands::QuickCommands;
use crate::types::*;

use futures::SinkExt;
use futures::stream::BoxStream;
use std::collections::VecDeque;
use std::time::Duration;

/// iced 订阅系统要求每个 Subscription 有唯一的哈希 ID。
/// 用裸结构体包装 PortHandle，以便为其实现自定义 Hash：
/// 对 Arc 的指针地址取哈希，确保"同一个句柄 = 同一个订阅 ID"。
/// 若直接对 Arc<Mutex<...>> 取哈希，哈希值会随内容变化；
/// 用指针地址则只要句柄未重新创建，ID 就保持不变，不会触发订阅重建。
#[derive(Clone)]
struct PortSub(backend::PortHandle);

impl std::hash::Hash for PortSub {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (std::sync::Arc::as_ptr(&self.0) as usize).hash(state);
    }
}

/// 串口数据读取流，在 iced 订阅系统内持续运行，直到 port_handle 被置 None。
/// 每次迭代在 spawn_blocking 线程里调用 backend::read_data（底层有 100ms 超时），
/// 再 sleep 20ms 后进入下一次轮询。
/// 20ms 间隔的选择：低于此值会空转消耗 CPU；高于此值会增加数据显示延迟。
/// 100ms 后端超时 + 20ms 间隔 = 最差情况下约 120ms 延迟，对串口调试场景可接受。
fn serial_stream(port: &PortSub) -> BoxStream<'static, Message> {
    let handle = port.0.clone();
    // channel 容量 100：消息队列深度。串口数据突发时允许最多积压 100 条消息，
    // 超出则 send().await 会背压阻塞发送方，避免无限内存增长。正常情况远不会触及上限。
    Box::pin(iced::stream::channel(100, async move |mut output| {
        loop {
            tokio::time::sleep(Duration::from_millis(20)).await;
            let handle2 = handle.clone();
            let result = tokio::task::spawn_blocking(move || backend::read_data(&handle2))
                .await
                .unwrap_or_else(|e| Err(format!("读取线程崩溃: {}", e)));
            match result {
                Ok(Some(data)) => {
                    let len = data.len();
                    let ascii = backend::bytes_to_display(&data);
                    let hex = backend::bytes_to_hex(&data);
                    output.send(Message::DataReceived(ascii, hex, len)).await.ok();
                }
                Ok(None) => {}
                Err(e) => {
                    output.send(Message::SerialError(e)).await.ok();
                }
            }
        }
    }))
}

/// 面板可拖拽的最小宽度（像素）；低于此值操作按钮会被截断无法点击
const MIN_PANEL_WIDTH: f32 = 100.0;

/// 按当前设置格式化当前时刻为时间戳字符串，用于在接收/发送行前插入时间前缀
fn fmt_timestamp(fmt: crate::types::TimestampFormat) -> String {
    use crate::types::TimestampFormat::*;
    match fmt {
        TimeOnly => chrono::Local::now().format("%H:%M:%S").to_string(),
        TimeMsec => chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
        DateTime => chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    }
}

/// 接收区最大保留行数。超出后从头部丢弃最旧的行，防止内存无限增长。
/// 2000 行约对应 ~200KB 文本，在典型串口速率下可覆盖数秒到数分钟的数据。
const MAX_LINES: usize = 2000;

/// 追加一条系统消息（连接状态、错误、提权、发送结果等），并立即同步到接收区。
/// 无论 auto_scroll 是否开启都强制滚到末尾，确保用户不会错过系统提示。
/// 串口数据流（DataReceived）不走此函数，由调用方直接操作 received_lines 以控制刷新时机。
fn push_line(monitor: &mut SerialMonitor, line: String) {
    monitor.terminal.received_lines.push_back(line);
    if monitor.terminal.received_lines.len() > MAX_LINES {
        monitor.terminal.received_lines.pop_front();
    }
    rebuild_receive_content(monitor);
    // 系统消息始终滚到末尾，让用户看到（即使 auto_scroll 关闭）
    monitor.terminal.receive_content.perform(
        widget::text_editor::Action::Move(widget::text_editor::Motion::DocumentEnd),
    );
}

/// 将 received_lines（按搜索词过滤后）同步到 receive_content。
/// auto_scroll=true 时将光标移到文档末尾（触发滚动到底部）；
/// auto_scroll=false 时不移动光标，保留用户当前的阅读位置。
fn rebuild_receive_content(monitor: &mut SerialMonitor) {
    let text: String = if monitor.terminal.search_text.is_empty() {
        monitor.terminal.received_lines.iter()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        monitor.terminal.received_lines.iter()
            .filter(|l| l.contains(&monitor.terminal.search_text))
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    };
    monitor.terminal.receive_content = widget::text_editor::Content::with_text(&text);
    if monitor.terminal.auto_scroll {
        monitor.terminal.receive_content.perform(
            widget::text_editor::Action::Move(widget::text_editor::Motion::DocumentEnd),
        );
    }
}

pub struct AppState {
    pub monitor: SerialMonitor,
    pub history: History,
    pub quick_commands: QuickCommands,
    pub port_handle: Option<backend::PortHandle>,
    pub app_settings: AppSettings,
    /// 快捷命令内联编辑状态：(槽位索引, 编辑中的名称, 编辑中的内容, is_hex)。
    /// 用匿名元组而非具名结构体，是因为此状态完全局限于 sidebar 的单次编辑流程，
    /// 无需跨模块传递，独立结构体反而增加噪声。
    pub quick_cmd_editing: Option<(usize, String, String, bool)>,
    pub reset_confirm_pending: bool,
    /// Ctrl+滚轮调字号时置 true，松开 Ctrl 后统一保存，避免每个 tick 都写磁盘
    pub font_size_dirty: bool,
}

impl AppState {
    pub fn new() -> (Self, iced::Task<Message>) {
        let state = boot();
        (state, iced::Task::done(Message::RefreshPorts))
    }
}

pub fn boot() -> AppState {
    let quick_commands = QuickCommands::new();
    let app_settings   = AppSettings::new();

    AppState {
        monitor: SerialMonitor {
            port: PortConfig {
                selected_port: app_settings.last_port.clone(),
                selected_baud: app_settings.last_baud.clone(),
                baud_combo_state: widget::combo_box::State::new(
                    settings::BAUD_PRESETS.iter().map(|s| s.to_string()).collect(),
                ),
                selected_data_bits: Some(DataBits::Eight),
                selected_stop_bits: Some(StopBits::One),
                selected_parity: Some(Parity::None),
                selected_flow_control: Some(FlowControl::None),
                dtr_enabled: false,
                rts_enabled: false,
                disable_modem_handshake: false,
                port_list: Vec::new(),
            },
            terminal: TerminalState {
                received_lines: VecDeque::new(),
                scroll_offset_y: 0.0,
                viewport_height: 400.0,
                search_text: String::new(),
                hex_display: false,
                show_timestamp: false,
                auto_scroll: true,
                pause_receive: false,
                send_content: widget::text_editor::Content::new(),
                receive_content: widget::text_editor::Content::new(),
                hex_send: false,
                line_ending: LineEnding::CrLf,
                timed_send: false,
                timed_send_interval: "1000".to_string(),
                rx_count: 0,
                tx_count: 0,
                history_nav_index: None,
            },
            layout: LayoutState {
                left_width: app_settings.left_width,
                right_width: app_settings.right_width,
                left_collapsed: false,
                right_collapsed: false,
                dragging: None,
                last_mouse_x: None,
                hover_handle: None,
                ctrl_held: false,
            },
            is_connected: false,
            is_dark_mode: false,
            left_panel_view: LeftPanelView::PortConfig,
            show_elevate_hint: false,
            ch341_fix_needed: {
                #[cfg(target_os = "linux")]
                { backend::ch341_fix::check_dtr_fix_needed() }
                #[cfg(not(target_os = "linux"))]
                { false }
            },
            ch341_fix_installing: false,
            ch341_install_elapsed: 0,
        },
        history: History::new(),
        quick_commands,
        port_handle: None,
        app_settings,
        quick_cmd_editing: None,
        reset_confirm_pending: false,
        font_size_dirty: false,
    }
}

pub fn update(state: &mut AppState, message: Message) -> iced::Task<Message> {
    let mut task = iced::Task::none();
    match message {
        // ── UI 配置 ────────────────────────────────────
        Message::PortSelected(port) => {
            state.monitor.port.selected_port = Some(port);
        }
        Message::BaudSelected(baud) => {
            state.monitor.port.selected_baud = baud;
        }
        Message::BaudInputChanged(text) => {
            state.monitor.port.selected_baud = text;
        }
        Message::DataBitsSelected(v) => {
            state.monitor.port.selected_data_bits = Some(v);
        }
        Message::StopBitsSelected(v) => {
            state.monitor.port.selected_stop_bits = Some(v);
        }
        Message::ParitySelected(v) => {
            state.monitor.port.selected_parity = Some(v);
        }
        Message::FlowControlSelected(v) => {
            state.monitor.port.selected_flow_control = Some(v);
        }
        Message::DtrToggled(enabled) => {
            state.monitor.port.dtr_enabled = enabled;
            if let Some(ref port) = state.port_handle {
                if let Ok(mut p) = port.lock() {
                    p.write_data_terminal_ready(enabled).ok();
                }
            }
        }
        Message::RtsToggled(enabled) => {
            state.monitor.port.rts_enabled = enabled;
            if let Some(ref port) = state.port_handle {
                if let Ok(mut p) = port.lock() {
                    p.write_request_to_send(enabled).ok();
                }
            }
        }
        Message::DisableModemHandshakeToggled(v) => {
            state.monitor.port.disable_modem_handshake = v;
        }
        Message::HexDisplayToggled(v) => {
            state.monitor.terminal.hex_display = v;
        }
        Message::TimestampToggled(v) => {
            state.monitor.terminal.show_timestamp = v;
        }
        Message::AutoScrollToggled(v) => {
            state.monitor.terminal.auto_scroll = v;
            if v {
                // 重新开启自动滚动时同步到最新内容
                rebuild_receive_content(&mut state.monitor);
            }
        }
        Message::PauseReceiveToggled(v) => {
            state.monitor.terminal.pause_receive = v;
        }
        Message::ClearReceived => {
            state.monitor.terminal.received_lines.clear();
            state.monitor.terminal.receive_content = widget::text_editor::Content::new();
            state.monitor.terminal.rx_count = 0;
        }
        Message::EditReceive(action) => {
            // 接收区只读：过滤掉所有编辑操作，仅允许光标移动、选中、复制
            match action {
                widget::text_editor::Action::Edit(_) => {}
                other => state.monitor.terminal.receive_content.perform(other),
            }
        }
        Message::EditSend(action) => {
            // ── 历史导航状态机 ──────────────────────────────────────────────────
            // 发送框支持用 Up/Down 键在发送历史中浏览，类似终端 shell 的行为。
            // history_nav_index 追踪当前游标：None=正常编辑，Some(n)=正在浏览第n条。
            //
            // Up 键触发条件（二选一满足即可）：
            //   1. 已处于导航模式（history_nav_index.is_some()）——继续向上翻
            //   2. 发送框是单行内容——此时 Up 不需要在框内移动光标，可安全进入导航
            // 多行内容时 Up 键保留其移动光标的原始语义，不拦截。
            if let widget::text_editor::Action::Move(widget::text_editor::Motion::Up) = &action {
                let items = state.history.items();
                let in_nav = state.monitor.terminal.history_nav_index.is_some();
                let single_line = !state.monitor.terminal.send_content.text()
                    .trim_end_matches('\n').contains('\n');
                if !items.is_empty() && (in_nav || single_line) {
                    // 已在末尾则钳位，不越界
                    let next = state.monitor.terminal.history_nav_index
                        .map(|i| (i + 1).min(items.len().saturating_sub(1)))
                        .unwrap_or(0);
                    state.monitor.terminal.send_content =
                        widget::text_editor::Content::with_text(&items[next].text);
                    state.monitor.terminal.history_nav_index = Some(next);
                    return task;
                }
            }
            // Down 键：仅在导航模式下拦截（正常编辑时不干预）。
            // nav_idx == 0 时再按 Down 退出导航，清空发送框（回到"空白新行"状态）。
            if let widget::text_editor::Action::Move(widget::text_editor::Motion::Down) = &action {
                if let Some(nav_idx) = state.monitor.terminal.history_nav_index {
                    if nav_idx == 0 {
                        state.monitor.terminal.send_content = widget::text_editor::Content::new();
                        state.monitor.terminal.history_nav_index = None;
                    } else {
                        let prev = nav_idx - 1;
                        state.monitor.terminal.send_content =
                            widget::text_editor::Content::with_text(
                                &state.history.items()[prev].text);
                        state.monitor.terminal.history_nav_index = Some(prev);
                    }
                    return task;
                }
            }
            // 任何打字/删除等编辑操作都会让发送框内容偏离历史记录，退出导航模式
            if matches!(&action, widget::text_editor::Action::Edit(_)) {
                state.monitor.terminal.history_nav_index = None;
            }

            // ── Enter 键：发送 or 换行 ──────────────────────────────────────────
            // 根据 send_mode 设置决定 Enter/Ctrl+Enter 各自的行为：
            //   EnterSend    模式：Enter=发送，Ctrl+Enter=换行（默认，适合快速发送）
            //   EnterNewline 模式：Enter=换行，Ctrl+Enter=发送（适合编写多行内容）
            if let widget::text_editor::Action::Edit(widget::text_editor::Edit::Enter) = &action {
                let ctrl = state.monitor.layout.ctrl_held;
                let should_send = match state.app_settings.send_mode {
                    SendMode::EnterSend    => !ctrl,
                    SendMode::EnterNewline => ctrl,
                };
                if should_send {
                    let _ = update(state, Message::SendMessage);
                    return task;
                }
                // should_send=false：fall-through 到 perform(action)，执行换行
            }

            state.monitor.terminal.send_content.perform(action);
        }
        Message::HexSendToggled(v) => {
            state.monitor.terminal.hex_send = v;
        }
        Message::LineEndingSelected(le) => {
            state.monitor.terminal.line_ending = le;
        }
        Message::TimedSendToggled(v) => {
            state.monitor.terminal.timed_send = v;
        }
        Message::TimedSendIntervalChanged(s) => {
            state.monitor.terminal.timed_send_interval = s;
        }

        // ── 串口操作 ────────────────────────────────────
        Message::RefreshPorts => {
            state.monitor.port.port_list = backend::refresh_ports();
        }
        Message::PollPorts => {
            let new_ports = backend::refresh_ports();
            if new_ports != state.monitor.port.port_list {
                state.monitor.port.port_list = new_ports;
            }
        }
        Message::TogglePort => {
            if state.monitor.is_connected {
                state.port_handle = None;
                state.monitor.is_connected = false;
                state.monitor.terminal.rx_count = 0;
                state.monitor.terminal.tx_count = 0;
            } else {
                let port_name = match &state.monitor.port.selected_port {
                    Some(p) => p.clone(),
                    None => {
                        push_line(&mut state.monitor, "[错误] 未选择端口".to_string());
                        return task;
                    }
                };
                let baud: u32 = match state.monitor.port.selected_baud.parse() {
                    Ok(b) => b,
                    Err(_) => {
                        push_line(&mut state.monitor, "[错误] 波特率格式错误".to_string());
                        return task;
                    }
                };
                let data_bits = state.monitor.port.selected_data_bits.clone().unwrap_or(DataBits::Eight);
                let parity    = state.monitor.port.selected_parity.clone().unwrap_or(Parity::None);
                let stop_bits = state.monitor.port.selected_stop_bits.clone().unwrap_or(StopBits::One);
                let flow_ctrl = state.monitor.port.selected_flow_control.clone().unwrap_or(FlowControl::None);
                let no_reset  = state.monitor.port.disable_modem_handshake;

                match backend::open_port(&port_name, baud, data_bits, parity, stop_bits, flow_ctrl, no_reset) {
                    Ok(handle) => {
                        // 将物理 DTR/RTS 同步到 UI 开关的当前状态。
                        // 驱动补丁（no_dtr_on_open=1）抑制了打开时的自动拉高，
                        // 因此此处是决定初始信号电平的唯一时机：
                        //   开关 ON  → 拉高 → 会触发目标设备复位（用户主动选择）
                        //   开关 OFF → 保持低电平 → 不复位
                        if let Ok(mut p) = handle.lock() {
                            p.write_data_terminal_ready(state.monitor.port.dtr_enabled).ok();
                            p.write_request_to_send(state.monitor.port.rts_enabled).ok();
                        }
                        state.port_handle = Some(handle);
                        state.monitor.is_connected = true;
                        state.monitor.show_elevate_hint = false;
                        push_line(&mut state.monitor,
                            format!("[已连接] {} @ {}", port_name, baud));
                        // 记忆上次使用的端口和波特率
                        state.app_settings.last_port = Some(port_name);
                        state.app_settings.last_baud = state.monitor.port.selected_baud.clone();
                        state.app_settings.save();
                    }
                    Err(backend::no_reset::OpenError::NeedReconnect(msg)) => {
                        push_line(&mut state.monitor, format!("[⚠ 配置完成] {}", msg));
                        push_line(&mut state.monitor,
                            "此配置只需进行一次，之后将永久生效。".to_string());
                    }
                    Err(backend::no_reset::OpenError::PermissionDenied(e)) => {
                        push_line(&mut state.monitor,
                            format!("[错误] 串口访问被拒绝（权限不足）: {}", e));
                        state.monitor.show_elevate_hint = true;
                    }
                    Err(backend::no_reset::OpenError::Other(e)) => {
                        push_line(&mut state.monitor, format!("[错误] {}", e));
                    }
                }
            }
        }
        Message::SendMessage => {
            if let Some(ref port) = state.port_handle {
                let text = state.monitor.terminal.send_content.text();
                let text = text.trim_end_matches('\n').to_string();
                if text.trim().is_empty() {
                    return task;
                }
                let hex_mode    = state.monitor.terminal.hex_send;
                let line_ending = state.monitor.terminal.line_ending.clone();

                match backend::send_data(port, &text, hex_mode, &line_ending) {
                    Ok(bytes) => {
                        state.monitor.terminal.tx_count += bytes;
                        state.history.add(text.clone(), hex_mode);
                        state.monitor.terminal.history_nav_index = None;

                        let label = if hex_mode { "[TX HEX]" } else { "[TX]" };
                        let line = if state.monitor.terminal.show_timestamp {
                            format!("[{}] {} {}", fmt_timestamp(state.app_settings.timestamp_format), label, text.trim())
                        } else {
                            format!("{} {}", label, text.trim())
                        };
                        push_line(&mut state.monitor, line);
                    }
                    Err(e) => {
                        push_line(&mut state.monitor, format!("[错误] {}", e));
                    }
                }
            } else {
                push_line(&mut state.monitor, "[错误] 串口未打开".to_string());
            }
        }
        Message::ClearSend => {
            state.monitor.terminal.send_content = widget::text_editor::Content::new();
            state.monitor.terminal.history_nav_index = None;
        }

        // ── 数据接收 ────────────────────────────────────
        Message::DataReceived(ascii, hex, len) => {
            state.monitor.terminal.rx_count += len;
            if state.monitor.terminal.pause_receive {
                return task;
            }
            let display = if state.monitor.terminal.hex_display { hex } else { ascii };
            let prefix = if state.monitor.terminal.show_timestamp {
                format!("[{}] [RX] ", fmt_timestamp(state.app_settings.timestamp_format))
            } else {
                "[RX] ".to_string()
            };
            // 直接操作 VecDeque，不走 push_line（避免每行都 rebuild）
            for line in display.split('\n').filter(|l| !l.is_empty()) {
                let l = format!("{}{}", prefix, line);
                state.monitor.terminal.received_lines.push_back(l);
                if state.monitor.terminal.received_lines.len() > MAX_LINES {
                    state.monitor.terminal.received_lines.pop_front();
                }
            }
            // auto_scroll=true → 实时刷新并滚到底
            // search 激活时 → 即使 auto_scroll=off 也需要刷新搜索结果（Bug 2 修复）
            // 其他情况（auto_scroll=off 且无搜索）→ 冻结视图，让用户安心选文本
            if state.monitor.terminal.auto_scroll
                || !state.monitor.terminal.search_text.is_empty()
            {
                rebuild_receive_content(&mut state.monitor);
            }
        }
        Message::SerialError(msg) => {
            // push_line 内部已经 rebuild + 滚到末尾，无需额外调用
            push_line(&mut state.monitor, format!("[错误] {}", msg));
            state.port_handle = None;
            state.monitor.is_connected = false;
            state.monitor.terminal.rx_count = 0;
            state.monitor.terminal.tx_count = 0;
        }

        // ── 接收区 ──────────────────────────────────────
        Message::TerminalScrolled { offset_y, viewport_height } => {
            state.monitor.terminal.scroll_offset_y = offset_y;
            state.monitor.terminal.viewport_height = viewport_height;
        }
        Message::SearchTextChanged(text) => {
            state.monitor.terminal.search_text = text;
            rebuild_receive_content(&mut state.monitor);
        }

        // ── 字号 ────────────────────────────────────────
        Message::FontSizeChanged(size) => {
            state.app_settings.font_size = size.clamp(8.0, 28.0);
            state.app_settings.save();
        }
        Message::FontSizeReset => {
            state.app_settings.font_size = 13.0;
            state.app_settings.save();
        }

        // ── 全局事件 ─────────────────────────────────────
        Message::ModifiersChanged(ctrl) => {
            state.monitor.layout.ctrl_held = ctrl;
            // Ctrl 松开时统一保存字号，避免滚动期间每 tick 写一次磁盘
            if !ctrl && state.font_size_dirty {
                state.app_settings.save();
                state.font_size_dirty = false;
            }
        }
        Message::WheelScrolled(delta) => {
            if state.monitor.layout.ctrl_held {
                state.app_settings.font_size = (state.app_settings.font_size + delta).clamp(8.0, 28.0);
                state.font_size_dirty = true;
            }
        }

        // ── 历史记录 ────────────────────────────────────
        Message::ClearHistory => {
            state.history.clear();
            state.monitor.terminal.history_nav_index = None;
        }
        Message::HistoryReuse(index) => {
            if let Some(item) = state.history.items().get(index) {
                let text   = item.text.clone();
                let is_hex = item.is_hex;
                state.monitor.terminal.send_content = widget::text_editor::Content::with_text(&text);
                state.monitor.terminal.hex_send = is_hex;
                state.monitor.terminal.history_nav_index = None;
            }
        }

        // ── 快捷命令 ────────────────────────────────────
        Message::QuickCommandSend(index) => {
            if let Some(cmd) = state.quick_commands.send_at(index) {
                if let Some(ref port) = state.port_handle {
                    let line_ending = state.monitor.terminal.line_ending.clone();
                    match backend::send_data(port, &cmd.text, cmd.is_hex, &line_ending) {
                        Ok(bytes) => { state.monitor.terminal.tx_count += bytes; }
                        Err(e) => {
                            push_line(&mut state.monitor, format!("[错误] {}", e));
                        }
                    }
                } else {
                    push_line(&mut state.monitor, "[错误] 串口未打开".to_string());
                }
            }
        }
        Message::QuickCommandStartEdit(index) => {
            if let Some(cmd) = state.quick_commands.items().get(index) {
                state.quick_cmd_editing = Some((index, cmd.name.clone(), cmd.text.clone(), cmd.is_hex));
            }
        }
        Message::QuickCommandEditName(s) => {
            if let Some((_, ref mut name, _, _)) = state.quick_cmd_editing {
                *name = s;
            }
        }
        Message::QuickCommandEditText(s) => {
            if let Some((_, _, ref mut text, _)) = state.quick_cmd_editing {
                *text = s;
            }
        }
        Message::QuickCommandEditHex(v) => {
            if let Some((_, _, _, ref mut is_hex)) = state.quick_cmd_editing {
                *is_hex = v;
            }
        }
        Message::QuickCommandSaveEdit => {
            if let Some((index, name, text, is_hex)) = state.quick_cmd_editing.take() {
                state.quick_commands.edit(index, name, text, is_hex);
            }
        }
        Message::QuickCommandCancelEdit => {
            state.quick_cmd_editing = None;
        }
        Message::QuickCommandDelete(index) => {
            if state.quick_cmd_editing.as_ref().map(|(i, ..)| *i) == Some(index) {
                state.quick_cmd_editing = None;
            }
            state.quick_commands.delete(index);
        }
        Message::ClearQuickCommands => {
            state.quick_commands.clear_all();
        }
        Message::SaveAsQuickCommand => {
            let text = state.monitor.terminal.send_content.text();
            let text = text.trim_end_matches('\n').to_string();
            if text.trim().is_empty() {
                push_line(&mut state.monitor, "[提示] 发送内容为空".to_string());
            } else {
                let is_hex = state.monitor.terminal.hex_send;
                match state.quick_commands.add_to_first_empty(&text, is_hex) {
                    Some(index) => {
                        push_line(&mut state.monitor,
                            format!("[快捷命令] 已保存到槽位 {}", index + 1));
                    }
                    None => {
                        push_line(&mut state.monitor,
                            "[快捷命令] 所有槽位已满".to_string());
                    }
                }
            }
        }

        // ── 日志 ────────────────────────────────────────
        Message::SaveLog => {
            let filename = chrono::Local::now().format("serial_log_%Y%m%d_%H%M%S.txt").to_string();
            let dir = state.app_settings.effective_log_dir();
            let path = dir.join(&filename);
            let content = state.monitor.terminal.received_lines.iter()
                .cloned().collect::<Vec<_>>().join("\n");
            match std::fs::write(&path, &content) {
                Ok(_) => push_line(&mut state.monitor,
                    format!("[日志已保存] {}", path.display())),
                Err(e) => push_line(&mut state.monitor,
                    format!("[错误] 日志保存失败: {} → {}", path.display(), e)),
            }
        }
        Message::LogDirCustomToggled(v) => {
            state.app_settings.log_dir_custom = v;
            state.app_settings.save();
        }
        Message::LogDirChanged(s) => {
            state.app_settings.log_dir = std::path::PathBuf::from(s);
        }
        Message::LogDirSaved => {
            state.app_settings.log_dir_custom = true;
            state.app_settings.save();
            push_line(&mut state.monitor,
                format!("[设置] 日志目录已保存：{}", state.app_settings.log_dir.display()));
        }
        Message::LogDirReset => {
            let default = dirs::document_dir()
                .or_else(|| dirs::home_dir())
                .unwrap_or_else(|| std::path::PathBuf::from("."));
            state.app_settings.log_dir = default;
            state.app_settings.log_dir_custom = false;
            state.app_settings.save();
        }

        // ── 提权 ─────────────────────────────────────────
        Message::ElevatePrivileges => {
            let port_name = state.monitor.port.selected_port
                .clone()
                .unwrap_or_else(|| "未知端口".to_string());
            push_line(&mut state.monitor,
                "[提权] 正在弹出授权对话框，请在系统弹窗中确认…".to_string());
            // 隐藏按钮避免重复点击，等结果回来再决定是否重新显示
            state.monitor.show_elevate_hint = false;
            task = iced::Task::perform(
                tokio::task::spawn_blocking(move || backend::privileges::elevate(&port_name)),
                |result| {
                    let msg = result.unwrap_or_else(|_| "[错误] 提权线程崩溃".to_string());
                    Message::ElevateResult(msg)
                },
            );
        }
        Message::ElevateResult(msg) => {
            push_line(&mut state.monitor, msg);
        }

        // ── CH341 DTR 修复（Linux）────────────────────────
        Message::FixCh341Driver => {
            #[cfg(target_os = "linux")]
            {
                let env = backend::ch341_fix::check_build_env();
                if !env.missing.is_empty() {
                    push_line(
                        &mut state.monitor,
                        format!("[提示] 缺少编译依赖，请先安装：{}", env.install_hint),
                    );
                } else {
                    state.monitor.ch341_fix_installing = true;
                    state.monitor.ch341_install_elapsed = 0;
                    task = iced::Task::perform(
                        tokio::task::spawn_blocking(backend::ch341_fix::run_install),
                        |result| {
                            let r = result.unwrap_or_else(|_| backend::ch341_fix::InstallResult {
                                success: false,
                                message: "安装线程崩溃".into(),
                            });
                            Message::Ch341FixResult { success: r.success, message: r.message }
                        },
                    );
                }
            }
        }
        Message::Ch341FixResult { success, message } => {
            state.monitor.ch341_fix_installing = false;
            state.monitor.ch341_fix_needed = !success;
            push_line(
                &mut state.monitor,
                if success { format!("[成功] {}", message) }
                else { format!("[错误] CH341 驱动安装失败: {}", message) },
            );
        }
        Message::Ch341InstallTick => {
            state.monitor.ch341_install_elapsed += 1;
        }


        // ── 应用设置 ─────────────────────────────────────
        Message::SendModeChanged(mode) => {
            state.app_settings.send_mode = mode;
            state.app_settings.save();
        }
        Message::TimestampFormatChanged(fmt) => {
            state.app_settings.timestamp_format = fmt;
            state.app_settings.save();
        }
        Message::ShowPortConfig => {
            state.monitor.left_panel_view = LeftPanelView::PortConfig;
        }
        Message::ShowAppSettings => {
            state.monitor.left_panel_view = LeftPanelView::AppSettings;
        }
        Message::OpenConfigFile => {
            let path = AppSettings::config_path();
            #[cfg(target_os = "windows")]
            std::process::Command::new("explorer")
                .arg(format!("/select,{}", path.display()))
                .spawn()
                .ok();
            #[cfg(not(target_os = "windows"))]
            if let Some(dir) = path.parent() {
                std::process::Command::new("xdg-open").arg(dir).spawn().ok();
            }
        }
        Message::RequestResetAllSettings => {
            state.reset_confirm_pending = true;
        }
        Message::CancelResetSettings => {
            state.reset_confirm_pending = false;
        }
        Message::ResetAllSettings => {
            state.app_settings.reset_to_defaults();
            state.monitor.layout.left_width  = state.app_settings.left_width;
            state.monitor.layout.right_width = state.app_settings.right_width;
            state.reset_confirm_pending = false;
        }

        // ── 拖拽 ────────────────────────────────────────
        Message::DragStarted(target) => {
            state.monitor.layout.dragging = Some(target);
            state.monitor.layout.last_mouse_x = None;
        }
        Message::DragMoved(x) => {
            if let Some(ref target) = state.monitor.layout.dragging {
                match state.monitor.layout.last_mouse_x {
                    None => state.monitor.layout.last_mouse_x = Some(x),
                    Some(prev) => {
                        let delta = x - prev;
                        match target {
                            // 左分隔条向右拖（delta > 0）→ 左面板变宽，符合直觉
                            DragTarget::Left => {
                                state.monitor.layout.left_width =
                                    (state.monitor.layout.left_width + delta).clamp(MIN_PANEL_WIDTH, 600.0);
                            }
                            // 右分隔条向右拖（delta > 0）→ 右面板变窄（从右向左量）
                            DragTarget::Right => {
                                state.monitor.layout.right_width =
                                    (state.monitor.layout.right_width - delta).clamp(MIN_PANEL_WIDTH, 400.0);
                            }
                        }
                        state.monitor.layout.last_mouse_x = Some(x);
                    }
                }
            }
        }
        Message::DragEnded => {
            state.monitor.layout.dragging = None;
            // 面板宽度持久化
            state.app_settings.left_width  = state.monitor.layout.left_width;
            state.app_settings.right_width = state.monitor.layout.right_width;
            state.app_settings.save();
        }
        Message::HandleHoverChanged(handle) => {
            state.monitor.layout.hover_handle = handle;
        }
        Message::ToggleTheme => {
            state.monitor.is_dark_mode = !state.monitor.is_dark_mode;
        }
        Message::ToggleLeftPanel => {
            state.monitor.layout.left_collapsed = !state.monitor.layout.left_collapsed;
        }
        Message::ToggleRightPanel => {
            state.monitor.layout.right_collapsed = !state.monitor.layout.right_collapsed;
        }
    }
    task
}

pub fn subscription(state: &AppState) -> Subscription<Message> {
    let drag_sub = splitter::drag_subscription(state.monitor.layout.dragging.clone());
    let mut subs: Vec<Subscription<Message>> = vec![drag_sub];

    if let Some(ref handle) = state.port_handle {
        subs.push(Subscription::run_with(PortSub(handle.clone()), serial_stream));
    }

    if state.monitor.terminal.timed_send && state.monitor.is_connected {
        let interval_ms = state.monitor.terminal.timed_send_interval
            .parse::<u64>().unwrap_or(1000)
            .max(100); // 下限 100ms：低于此值容易造成串口写溢出和 UI 卡顿
        subs.push(iced::time::every(Duration::from_millis(interval_ms)).map(|_| Message::SendMessage));
    }

    subs.push(iced::time::every(Duration::from_secs(2)).map(|_| Message::PollPorts));
    if state.monitor.ch341_fix_installing {
        subs.push(iced::time::every(Duration::from_secs(1)).map(|_| Message::Ch341InstallTick));
    }


    // Ctrl 键状态 + 鼠标滚轮（用于字号缩放）
    subs.push(iced::event::listen_with(|event, _status, _id| match event {
        Event::Keyboard(iced::keyboard::Event::ModifiersChanged(mods)) => {
            Some(Message::ModifiersChanged(mods.control()))
        }
        Event::Mouse(iced::mouse::Event::WheelScrolled { delta }) => {
            let y = match delta {
                iced::mouse::ScrollDelta::Lines  { y, .. } => y,
                // Pixels 模式下将像素量转换为"行数"当量：20px ≈ 1 行，与 Lines 模式量纲统一
                iced::mouse::ScrollDelta::Pixels { y, .. } => y / 20.0,
            };
            Some(Message::WheelScrolled(y))
        }
        _ => None,
    }));

    Subscription::batch(subs)
}

pub fn view(state: &AppState) -> Element<'_, Message> {
    let m = &state.monitor;
    // 折叠后面板的宽度：刚好容纳展开按钮（约 24px 按钮 + 4px 左右 padding）
    const COLLAPSED_W: f32 = 32.0;

    let mut children: Vec<Element<'_, Message>> = Vec::with_capacity(5);

    // ── 左面板 ──────────────────────────────────────────
    if m.layout.left_collapsed {
        children.push(
            widget::container(
                widget::column![
                    widget::Space::new().height(iced::Fill),
                    widget::button(widget::text("▶").size(12))
                        .on_press(Message::ToggleLeftPanel)
                        .style(styles::button_subtle)
                        .padding(6),
                    widget::Space::new().height(iced::Fill),
                ]
                .align_x(iced::Alignment::Center),
            )
            .width(COLLAPSED_W)
            .height(iced::Fill)
            .style(styles::panel_container)
            .into(),
        );
    } else {
        let left_content: Element<'_, Message> = match m.left_panel_view {
            LeftPanelView::PortConfig =>
                settings::view_port_config(
                    &m.port,
                    m.is_connected,
                    m.is_dark_mode,
                    m.show_elevate_hint,
                    m.ch341_fix_needed,
                    m.ch341_fix_installing,
                    m.ch341_install_elapsed,
                ).into(),
            LeftPanelView::AppSettings =>
                settings::view_app_settings(&state.app_settings, m.is_dark_mode, state.reset_confirm_pending).into(),
        };
        children.push(
            widget::container(left_content)
                .width(m.layout.left_width)
                .padding(12)
                .style(styles::panel_container)
                .into(),
        );
        children.push(
            splitter::split_handle(
                DragTarget::Left,
                m.layout.dragging == Some(DragTarget::Left),
                m.layout.hover_handle == Some(DragTarget::Left),
            )
            .into(),
        );
    }

    // ── 中间终端（始终显示）────────────────────────────
    children.push(
        widget::container(terminal::view_terminal(
            &m.terminal,
            m.is_dark_mode,
            state.app_settings.font_size,
        ))
        .width(iced::Fill)
        .padding(12)
        .style(styles::panel_container)
        .into(),
    );

    // ── 右面板 ──────────────────────────────────────────
    if m.layout.right_collapsed {
        children.push(
            widget::container(
                widget::column![
                    widget::Space::new().height(iced::Fill),
                    widget::button(widget::text("◀").size(12))
                        .on_press(Message::ToggleRightPanel)
                        .style(styles::button_subtle)
                        .padding(6),
                    widget::Space::new().height(iced::Fill),
                ]
                .align_x(iced::Alignment::Center),
            )
            .width(COLLAPSED_W)
            .height(iced::Fill)
            .style(styles::panel_container)
            .into(),
        );
    } else {
        children.push(
            splitter::split_handle(
                DragTarget::Right,
                m.layout.dragging == Some(DragTarget::Right),
                m.layout.hover_handle == Some(DragTarget::Right),
            )
            .into(),
        );
        children.push(
            widget::container(sidebar::view_sidebar(
                m.is_dark_mode,
                &state.history,
                &state.quick_commands,
                m.layout.right_width,
                state.quick_cmd_editing.as_ref(),
            ))
            .width(m.layout.right_width)
            .padding(12)
            .style(styles::panel_container)
            .into(),
        );
    }

    widget::row(children)
        .spacing(0)
        .padding(12)
        .into()
}
