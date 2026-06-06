use iced::widget::text_editor;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

// ── 串口参数枚举 ──────────────────────────────────────────────────────────────
// 每个枚举都是对 serialport crate 同名类型的镜像，解耦 UI 层与底层库的依赖。
// 转换在 backend::open_port 中集中进行，UI 只感知这些类型，不直接依赖 serialport。

/// 每帧数据位数（标准串口协议中最常用的是 8 位）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataBits { Five, Six, Seven, Eight }

impl std::fmt::Display for DataBits {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            DataBits::Five  => write!(f, "5"),
            DataBits::Six   => write!(f, "6"),
            DataBits::Seven => write!(f, "7"),
            DataBits::Eight => write!(f, "8"),
        }
    }
}

/// 停止位数（绝大多数设备使用 1 位；2 位用于某些旧设备或低速总线）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopBits { One, Two }

impl std::fmt::Display for StopBits {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            StopBits::One => write!(f, "1"),
            StopBits::Two => write!(f, "2"),
        }
    }
}

/// 奇偶校验模式（现代设备多用 None；Odd/Even 用于对可靠性有要求的工业协议）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Parity { None, Odd, Even }

impl std::fmt::Display for Parity {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Parity::None => write!(f, "None"),
            Parity::Odd  => write!(f, "Odd"),
            Parity::Even => write!(f, "Even"),
        }
    }
}

/// 流量控制方式（Hardware = RTS/CTS 硬件握手；Software = XON/XOFF 软件控制；None 最常见）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowControl { None, Hardware, Software }

impl std::fmt::Display for FlowControl {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            FlowControl::None     => write!(f, "无"),
            FlowControl::Hardware => write!(f, "硬件"),
            FlowControl::Software => write!(f, "软件"),
        }
    }
}

/// 发送时追加的行结束符（仅 ASCII 模式生效；HEX 模式下忽略此设置）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineEnding { None, CrLf, Lf, Cr }

impl std::fmt::Display for LineEnding {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            LineEnding::None => write!(f, "无"),
            LineEnding::CrLf => write!(f, "\\r\\n"),
            LineEnding::Lf   => write!(f, "\\n"),
            LineEnding::Cr   => write!(f, "\\r"),
        }
    }
}

/// 三列布局中正在被拖拽的分隔条：Left = 左列/中列之间，Right = 中列/右列之间
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DragTarget { Left, Right }

// ── 应用级枚举 ────────────────────────────────────────────────────────────────

/// 发送框 Enter 键行为模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SendMode {
    #[default]
    EnterSend,     // Enter = 发送，Ctrl+Enter = 换行
    EnterNewline,  // Enter = 换行，Ctrl+Enter = 发送
}

impl std::fmt::Display for SendMode {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            SendMode::EnterSend    => write!(f, "Enter 发送 / Ctrl+Enter 换行"),
            SendMode::EnterNewline => write!(f, "Enter 换行 / Ctrl+Enter 发送"),
        }
    }
}

/// 时间戳格式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TimestampFormat {
    #[default]
    TimeOnly,   // HH:MM:SS
    TimeMsec,   // HH:MM:SS.mmm
    DateTime,   // YYYY-MM-DD HH:MM:SS
}

impl std::fmt::Display for TimestampFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            TimestampFormat::TimeOnly => write!(f, "HH:MM:SS"),
            TimestampFormat::TimeMsec => write!(f, "HH:MM:SS.mmm"),
            TimestampFormat::DateTime => write!(f, "YYYY-MM-DD HH:MM:SS"),
        }
    }
}

/// 左侧面板当前显示的内容
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeftPanelView { PortConfig, AppSettings }

// ── 数据类型 ──────────────────────────────────────────────────────────────────

/// 单条发送历史记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryItem {
    pub text: String,
    pub is_hex: bool,
    /// 发送时刻的格式化时间字符串（"%H:%M:%S"），仅用于 UI 显示，不参与逻辑运算
    pub time: String,
}

/// 快捷命令槽位（持久化到 quick_commands.json；固定 20 个槽，空槽 text 为空字符串）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickCommand {
    /// 按钮显示名称，为空时回退到"命令 N"
    pub name: String,
    pub text: String,
    pub is_hex: bool,
}

impl Default for QuickCommand {
    fn default() -> Self {
        Self { name: String::new(), text: String::new(), is_hex: false }
    }
}

// ── SerialMonitor 子结构体 ────────────────────────────────────────────────────

/// 串口连接参数 + 端口列表
pub struct PortConfig {
    /// 当前选中的端口名（如 "COM3" / "/dev/ttyUSB0"），None 表示未选择
    pub selected_port: Option<String>,
    /// 当前波特率字符串；存 String 而非 u32 是因为支持手动输入，解析在 TogglePort 时进行
    pub selected_baud: String,
    /// combo_box 控件的内部状态，持有预设波特率列表；iced 要求将其绑定到父结构体
    pub baud_combo_state: iced::widget::combo_box::State<String>,
    pub selected_data_bits: Option<DataBits>,
    pub selected_stop_bits: Option<StopBits>,
    pub selected_parity: Option<Parity>,
    pub selected_flow_control: Option<FlowControl>,
    /// DTR（数据终端就绪）信号电平，连接中可实时切换
    pub dtr_enabled: bool,
    /// RTS（请求发送）信号电平，连接中可实时切换
    pub rts_enabled: bool,
    /// 打开串口前是否写注册表禁用 Modem 握手信号（防止 CH340 等芯片在 open() 时复位目标设备）
    pub disable_modem_handshake: bool,
    /// 系统当前可用串口列表，每 2 秒刷新一次
    pub port_list: Vec<String>,
}

/// 接收显示 + 发送输入 + 计数器
pub struct TerminalState {
    /// 接收区行缓冲，上限 MAX_LINES；超出后从头部弹出最旧的行
    pub received_lines: VecDeque<String>,
    /// 接收区当前滚动偏移（像素），仅在 auto_scroll=false 时有效
    pub scroll_offset_y: f32,
    /// 接收区可见高度（像素），由 TerminalScrolled 事件更新
    pub viewport_height: f32,
    pub search_text: String,
    /// true = 以 HEX 格式显示接收内容，false = ASCII 可显示字符串
    pub hex_display: bool,
    pub show_timestamp: bool,
    pub auto_scroll: bool,
    /// true = 暂停显示新数据（数据仍在 rx_count 累计，只是不推入 received_lines）
    pub pause_receive: bool,
    pub send_content: text_editor::Content,
    /// 接收区显示内容，随 received_lines 同步更新，支持文本选中复制
    pub receive_content: text_editor::Content,
    pub hex_send: bool,
    pub line_ending: LineEnding,
    pub timed_send: bool,
    /// 定时发送间隔，单位毫秒，存 String 以允许用户输入中间状态（如空字符串）
    pub timed_send_interval: String,
    /// 累计接收字节数（含暂停期间），用于状态栏显示
    pub rx_count: usize,
    /// 累计发送字节数
    pub tx_count: usize,
    /// 历史记录键盘导航游标：
    /// - None      — 未处于导航模式，发送框内容与历史无关
    /// - Some(0)   — 正在浏览最近一条历史（Up 键进入此状态）
    /// - Some(n)   — 正在浏览第 n 条（n 越大越旧；Up 增大，Down 减小至 0 后退出）
    /// 任何 Edit 类型的 Action（打字、删除等）都会将此值重置为 None
    pub history_nav_index: Option<usize>,
}

/// 三列布局拖拽状态
pub struct LayoutState {
    /// 左面板像素宽度，范围 [MIN_PANEL_WIDTH, 600]
    pub left_width: f32,
    /// 右面板像素宽度，范围 [MIN_PANEL_WIDTH, 400]
    pub right_width: f32,
    pub left_collapsed: bool,
    pub right_collapsed: bool,
    /// 当前正在拖拽的分隔条目标；None 表示无拖拽进行中
    pub dragging: Option<DragTarget>,
    /// 上一帧的鼠标 X 坐标，用于计算每帧拖拽增量（delta = current_x - last_mouse_x）。
    /// None 表示拖拽的第一帧——此时没有"上一帧"，跳过计算，仅记录当前位置作为基准
    pub last_mouse_x: Option<f32>,
    /// 当前鼠标悬停的分隔条；用于高亮显示，None 表示未悬停任何分隔条
    pub hover_handle: Option<DragTarget>,
    /// Ctrl 键当前是否按下；用于区分"Ctrl+滚轮调字号"和"普通滚轮滚动"
    pub ctrl_held: bool,
}

/// 顶层状态容器
pub struct SerialMonitor {
    pub port: PortConfig,
    pub terminal: TerminalState,
    pub layout: LayoutState,
    pub is_connected: bool,
    pub is_dark_mode: bool,
    pub left_panel_view: LeftPanelView,
    /// 上次打开串口时发生权限错误，为 true 时在设置面板显示提权按钮
    pub show_elevate_hint: bool,
    /// Linux: ch341 模块缺少 no_dtr_on_open 参数，需要用户手动安装修复驱动
    pub ch341_fix_needed: bool,
    /// Linux: 正在执行驱动安装（pkexec 运行中），用于 UI 显示进度状态
    pub ch341_fix_installing: bool,
    /// Linux: 安装已经过的秒数，每秒由 subscription 递增，用于 UI 防卡死提示
    pub ch341_install_elapsed: u32,
}

// ── 消息 ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Message {
    // UI 配置
    PortSelected(String),
    BaudSelected(String),
    BaudInputChanged(String),
    DataBitsSelected(DataBits),
    StopBitsSelected(StopBits),
    ParitySelected(Parity),
    FlowControlSelected(FlowControl),
    DtrToggled(bool),
    RtsToggled(bool),
    DisableModemHandshakeToggled(bool),
    TogglePort,
    HexDisplayToggled(bool),
    TimestampToggled(bool),
    AutoScrollToggled(bool),
    PauseReceiveToggled(bool),
    ClearReceived,
    EditSend(text_editor::Action),
    HexSendToggled(bool),
    LineEndingSelected(LineEnding),
    TimedSendToggled(bool),
    TimedSendIntervalChanged(String),
    SendMessage,
    ClearSend,
    /// 接收区交互（选中/移动光标/复制），Edit 类 action 在 update 中被过滤
    EditReceive(text_editor::Action),
    DragStarted(DragTarget),
    DragMoved(f32),
    DragEnded,
    HandleHoverChanged(Option<DragTarget>),
    ToggleTheme,
    ToggleLeftPanel,
    ToggleRightPanel,

    // 串口后端
    RefreshPorts,
    PollPorts,
    /// 串口收到新数据。字段：
    /// 0 ascii — bytes_to_display 转换后的可打印字符串（非法字节替换为 '.'）
    /// 1 hex   — bytes_to_hex 转换后的大写十六进制字符串（如 "41 42 0D 0A"）
    /// 2 len   — 原始字节数，用于累加 rx_count
    DataReceived(String, String, usize),
    SerialError(String),

    // 接收区
    #[allow(dead_code)]
    TerminalScrolled { offset_y: f32, viewport_height: f32 },
    SearchTextChanged(String),

    // 字号
    FontSizeChanged(f32),
    FontSizeReset,

    // 全局事件
    ModifiersChanged(bool),  // Ctrl 键状态
    WheelScrolled(f32),      // 滚轮 delta Y

    // 历史记录
    ClearHistory,
    HistoryReuse(usize),

    // 快捷命令
    QuickCommandSend(usize),
    QuickCommandStartEdit(usize),
    QuickCommandEditName(String),
    QuickCommandEditText(String),
    QuickCommandEditHex(bool),
    QuickCommandSaveEdit,
    QuickCommandCancelEdit,
    QuickCommandDelete(usize),
    ClearQuickCommands,
    SaveAsQuickCommand,

    // 日志
    SaveLog,
    LogDirCustomToggled(bool),
    LogDirChanged(String),
    LogDirSaved,
    LogDirReset,

    // 提权
    /// 触发平台专属的权限申请流程（Windows: UAC 重启；Linux: pkexec udev 规则）
    ElevatePrivileges,
    /// Linux: pkexec 执行完毕，携带结果消息用于终端显示
    ElevateResult(String),

    // CH341 DTR 修复（Linux）
    /// 用户主动触发：检查编译环境并启动驱动安装流程
    FixCh341Driver,
    /// 安装脚本执行完毕后返回结果
    Ch341FixResult { success: bool, message: String },
    /// 安装进行中每秒触发一次，用于更新 UI 计时显示
    Ch341InstallTick,

    // 应用设置
    SendModeChanged(SendMode),
    TimestampFormatChanged(TimestampFormat),
    ShowPortConfig,
    ShowAppSettings,
    OpenConfigFile,
    RequestResetAllSettings,
    CancelResetSettings,
    ResetAllSettings,
}
