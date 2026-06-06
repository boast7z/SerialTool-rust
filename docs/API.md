# 函数与类型参考

本文档覆盖项目所有公开模块的函数、类型与关键内部实现，包含签名、参数说明和行为描述。

> **测试平台**：Windows x64、Linux x64。其他平台理论可编译但未验证，文档中不对未测试平台作兼容性承诺。

---

## 目录

- [构建脚本 (`build.rs`)](#构建脚本-buildrs)
- [类型层 (`src/types.rs`)](#类型层-srctypesrs)
  - [串口参数枚举](#串口参数枚举)
  - [应用级枚举](#应用级枚举)
  - [数据结构体](#数据结构体)
  - [状态子结构体](#状态子结构体)
  - [Message 枚举](#message-枚举)
- [后端层 (`src/backend/`)](#后端层-srcbackend)
  - [串口 I/O (`mod.rs`)](#串口-io-modrs)
  - [防复位 (`no_reset.rs`)](#防复位-no_resetrs)
  - [应用设置 (`app_settings.rs`)](#应用设置-app_settingsrs)
  - [发送历史 (`history.rs`)](#发送历史-historyrs)
  - [快捷命令 (`quick_commands.rs`)](#快捷命令-quick_commandsrs)
  - [提权 (`privileges.rs`)](#提权-privilegesrs)
  - [CH341 修复 (`ch341_fix.rs`)](#ch341-修复-ch341_fixrs)
- [UI 层 (`src/ui/`)](#ui-层-srcui)
  - [应用状态机 (`mod.rs`)](#应用状态机-modrs)
  - [分隔条 (`splitter.rs`)](#分隔条-splitterrs)
  - [终端面板 (`terminal.rs`)](#终端面板-terminalrs)
  - [设置面板 (`settings.rs`)](#设置面板-settingsrs)
  - [侧边栏 (`sidebar.rs`)](#侧边栏-sidebarrs)
  - [主题 (`theme.rs`)](#主题-themers)
  - [样式 (`styles.rs`)](#样式-stylesrs)
  - [图标 (`icons.rs`)](#图标-iconsrs)

---

## 构建脚本 (`build.rs`)

Cargo 在编译前自动执行，仅在目标平台为 Windows 时生效。

```rust
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("icons/icon.ico");
        res.set("ProductName", "串口调试助手");
        res.set("FileDescription", "串口调试助手");
        res.set("CompanyName", "boast");
        res.compile().expect("Failed to compile Windows resources");
    }
}
```

**作用**：通过 `winresource` 将以下内容编译时嵌入 EXE：

| 资源 | 值 |
|------|----|
| 应用图标 | `icons/icon.ico`（资源管理器、任务栏显示） |
| ProductName | `串口调试助手` |
| FileDescription | `串口调试助手` |
| CompanyName | `boast` |

非 Windows 平台（Linux）此脚本无任何操作，`winresource` 依赖也不会被编译（`[build-dependencies]` 在非 Windows 上被条件排除）。

---

## 类型层 (`src/types.rs`)

所有枚举、结构体和 `Message` 的单一来源，UI 层与后端层共同依赖此模块，不直接依赖 `serialport` crate。

### 串口参数枚举

#### `DataBits`

```rust
pub enum DataBits { Five, Six, Seven, Eight }
```

每帧数据位数。标准串口通信中绝大多数设备使用 `Eight`（8 位）。

| 变体 | 含义 |
|------|------|
| `Five` | 5 数据位（teletype 遗留格式） |
| `Six` | 6 数据位 |
| `Seven` | 7 数据位（ASCII 通信） |
| `Eight` | 8 数据位（最常用，默认值） |

实现了 `Display`（显示为 `"5"/"6"/"7"/"8"`）、`Debug`、`Clone`、`PartialEq`、`Eq`。

---

#### `StopBits`

```rust
pub enum StopBits { One, Two }
```

停止位数。`One` 是现代设备默认值；`Two` 用于某些旧设备或低速总线。

实现了 `Display`（显示为 `"1"/"2"`）。

---

#### `Parity`

```rust
pub enum Parity { None, Odd, Even }
```

奇偶校验模式。

| 变体 | 含义 |
|------|------|
| `None` | 无校验（最常见） |
| `Odd` | 奇校验 |
| `Even` | 偶校验 |

---

#### `FlowControl`

```rust
pub enum FlowControl { None, Hardware, Software }
```

流量控制方式。

| 变体 | 含义 |
|------|------|
| `None` | 无流控（最常见） |
| `Hardware` | RTS/CTS 硬件握手 |
| `Software` | XON/XOFF 软件控制 |

---

#### `LineEnding`

```rust
pub enum LineEnding { None, CrLf, Lf, Cr }
```

ASCII 模式发送时追加的行结束符（HEX 模式下此设置被忽略）。

| 变体 | 字节序列 | 显示 |
|------|----------|------|
| `None` | 无 | `无` |
| `CrLf` | `\r\n` (0x0D 0x0A) | `\r\n` |
| `Lf` | `\n` (0x0A) | `\n` |
| `Cr` | `\r` (0x0D) | `\r` |

---

#### `DragTarget`

```rust
pub enum DragTarget { Left, Right }
```

三列布局中正在被拖拽的分隔条：`Left` = 左列/中列之间，`Right` = 中列/右列之间。

---

### 应用级枚举

#### `SendMode`

```rust
#[derive(Default, Serialize, Deserialize)]
pub enum SendMode {
    #[default]
    EnterSend,     // Enter = 发送，Ctrl+Enter = 换行
    EnterNewline,  // Enter = 换行，Ctrl+Enter = 发送
}
```

发送框 Enter 键行为模式，持久化到 `settings.json`。

---

#### `TimestampFormat`

```rust
#[derive(Default, Serialize, Deserialize)]
pub enum TimestampFormat {
    #[default]
    TimeOnly,   // HH:MM:SS
    TimeMsec,   // HH:MM:SS.mmm（含毫秒）
    DateTime,   // YYYY-MM-DD HH:MM:SS
}
```

时间戳显示格式，持久化到 `settings.json`。

---

#### `LeftPanelView`

```rust
pub enum LeftPanelView { PortConfig, AppSettings }
```

左侧面板当前显示的内容页。`PortConfig` = 串口配置页（默认）；`AppSettings` = 应用设置页。

---

### 数据结构体

#### `HistoryItem`

```rust
#[derive(Serialize, Deserialize)]
pub struct HistoryItem {
    pub text: String,   // 发送的原始内容
    pub is_hex: bool,   // 是否为 HEX 模式发送
    pub time: String,   // 格式化时间字符串 "%H:%M:%S"，仅用于 UI 显示
}
```

单条发送历史记录，持久化到 `history.json`。

---

#### `QuickCommand`

```rust
#[derive(Serialize, Deserialize)]
pub struct QuickCommand {
    pub name: String,   // 按钮显示名称，为空时回退到 "命令 N"
    pub text: String,   // 命令内容；空字符串表示空槽
    pub is_hex: bool,   // 是否以 HEX 模式发送
}
```

快捷命令槽位，持久化到 `quick_commands.json`。`Default` 实现返回所有字段为空/false 的空槽。

---

### 状态子结构体

#### `PortConfig`

串口连接参数和端口列表，存储在 `SerialMonitor.port`。

| 字段 | 类型 | 说明 |
|------|------|------|
| `selected_port` | `Option<String>` | 当前选中端口名（如 `"COM3"`、`"/dev/ttyUSB0"`），`None` 表示未选择 |
| `selected_baud` | `String` | 波特率字符串，存 `String` 而非 `u32` 是为了支持用户手动输入中间状态 |
| `baud_combo_state` | `combo_box::State<String>` | iced combo_box 内部状态，持有预设波特率列表 |
| `selected_data_bits` | `Option<DataBits>` | 当前选中数据位 |
| `selected_stop_bits` | `Option<StopBits>` | 当前选中停止位 |
| `selected_parity` | `Option<Parity>` | 当前选中校验位 |
| `selected_flow_control` | `Option<FlowControl>` | 当前选中流控 |
| `dtr_enabled` | `bool` | DTR 信号当前电平（连接中可实时切换） |
| `rts_enabled` | `bool` | RTS 信号当前电平（连接中可实时切换） |
| `disable_modem_handshake` | `bool` | 是否启用防复位流程（写注册表 / pre_open 守护 fd） |
| `port_list` | `Vec<String>` | 系统当前可用串口列表，每 2 秒自动刷新 |

---

#### `TerminalState`

接收显示、发送输入和统计计数器，存储在 `SerialMonitor.terminal`。

| 字段 | 类型 | 说明 |
|------|------|------|
| `received_lines` | `VecDeque<String>` | 接收行缓冲，上限 2000 行；超出时从头部丢弃最旧的行 |
| `scroll_offset_y` | `f32` | 接收区当前滚动偏移（px），`auto_scroll=false` 时有效 |
| `viewport_height` | `f32` | 接收区可见高度（px），由 `TerminalScrolled` 事件更新 |
| `search_text` | `String` | 关键字过滤输入内容；非空时仅显示匹配行 |
| `hex_display` | `bool` | `true` = HEX 格式显示接收内容 |
| `show_timestamp` | `bool` | 是否在每行前插入时间戳前缀 |
| `auto_scroll` | `bool` | 是否自动滚动到接收区底部 |
| `pause_receive` | `bool` | `true` = 暂停将数据推入 `received_lines`（数据仍计入 `rx_count`） |
| `send_content` | `text_editor::Content` | 发送框编辑器内容 |
| `receive_content` | `text_editor::Content` | 接收区显示内容（只读，由 `rebuild_receive_content` 同步） |
| `hex_send` | `bool` | `true` = HEX 模式发送 |
| `line_ending` | `LineEnding` | 发送时追加的行结束符 |
| `timed_send` | `bool` | 是否启用定时自动发送 |
| `timed_send_interval` | `String` | 定时发送间隔（毫秒），存 `String` 允许输入中间状态 |
| `rx_count` | `usize` | 累计接收字节数（含暂停期间） |
| `tx_count` | `usize` | 累计发送字节数 |
| `history_nav_index` | `Option<usize>` | 键盘历史导航游标：`None` = 正常编辑；`Some(0)` = 最新条目；`Some(n)` = 第 n 条（越大越旧） |

---

#### `LayoutState`

三列布局拖拽和折叠状态，存储在 `SerialMonitor.layout`。

| 字段 | 类型 | 说明 |
|------|------|------|
| `left_width` | `f32` | 左面板像素宽度，范围 `[100, 600]` |
| `right_width` | `f32` | 右面板像素宽度，范围 `[100, 400]` |
| `left_collapsed` | `bool` | 左面板是否折叠（折叠后宽度固定为 32 px） |
| `right_collapsed` | `bool` | 右面板是否折叠 |
| `dragging` | `Option<DragTarget>` | 当前正在拖拽的分隔条；`None` 表示无拖拽 |
| `last_mouse_x` | `Option<f32>` | 上一帧鼠标 X 坐标，用于计算拖拽增量；`None` 表示拖拽第一帧 |
| `hover_handle` | `Option<DragTarget>` | 当前鼠标悬停的分隔条，用于高亮显示 |
| `ctrl_held` | `bool` | Ctrl 键当前是否按下（区分 Ctrl+滚轮调字号 vs 普通滚动） |

---

#### `SerialMonitor`

顶层 UI 状态容器。

| 字段 | 类型 | 说明 |
|------|------|------|
| `port` | `PortConfig` | 串口参数 |
| `terminal` | `TerminalState` | 接收/发送状态 |
| `layout` | `LayoutState` | 布局状态 |
| `is_connected` | `bool` | 串口当前是否打开 |
| `is_dark_mode` | `bool` | 是否使用深色主题 |
| `left_panel_view` | `LeftPanelView` | 左面板当前显示页 |
| `show_elevate_hint` | `bool` | 上次打开串口时出现权限错误，显示提权按钮 |
| `ch341_fix_needed` | `bool` | Linux：检测到 ch341 模块缺少 `no_dtr_on_open` 参数 |
| `ch341_fix_installing` | `bool` | Linux：驱动安装脚本正在执行 |
| `ch341_install_elapsed` | `u32` | Linux：安装进行中的秒数，用于 UI 防卡死提示 |

---

### Message 枚举

`Message` 是 iced Elm 架构的消息总线，所有用户交互和异步事件都通过它路由到 `update()`。

```rust
pub enum Message { ... }
```

**UI 配置类**

| 变体 | 触发时机 | 携带数据 |
|------|----------|----------|
| `PortSelected(String)` | 用户选择端口 | 端口名 |
| `BaudSelected(String)` | 用户在下拉框选择波特率 | 波特率字符串 |
| `BaudInputChanged(String)` | 用户手动输入波特率 | 输入中间状态 |
| `DataBitsSelected(DataBits)` | 用户选择数据位 | — |
| `StopBitsSelected(StopBits)` | 用户选择停止位 | — |
| `ParitySelected(Parity)` | 用户选择校验位 | — |
| `FlowControlSelected(FlowControl)` | 用户选择流控 | — |
| `DtrToggled(bool)` | 用户切换 DTR 开关 | 新状态 |
| `RtsToggled(bool)` | 用户切换 RTS 开关 | 新状态 |
| `DisableModemHandshakeToggled(bool)` | 用户切换防复位开关 | — |
| `TogglePort` | 用户点击打开/关闭串口按钮 | — |
| `HexDisplayToggled(bool)` | 用户切换 HEX 接收显示 | — |
| `TimestampToggled(bool)` | 用户切换时间戳 | — |
| `AutoScrollToggled(bool)` | 用户切换自动滚动 | — |
| `PauseReceiveToggled(bool)` | 用户切换暂停接收 | — |
| `ClearReceived` | 用户点击清空接收区 | — |
| `EditSend(text_editor::Action)` | 发送框编辑器任意操作 | iced 编辑器动作 |
| `EditReceive(text_editor::Action)` | 接收区选中/复制操作 | （Edit 类动作被过滤） |
| `HexSendToggled(bool)` | 用户切换 HEX 发送 | — |
| `LineEndingSelected(LineEnding)` | 用户选择行结束符 | — |
| `TimedSendToggled(bool)` | 用户启停定时发送 | — |
| `TimedSendIntervalChanged(String)` | 用户修改定时间隔 | 间隔毫秒字符串 |
| `SendMessage` | 用户点击发送 / 按 Enter / 定时触发 | — |
| `ClearSend` | 用户点击清空发送框 | — |

**串口后端类**

| 变体 | 触发时机 | 携带数据 |
|------|----------|----------|
| `RefreshPorts` | 启动时立即刷新 | — |
| `PollPorts` | 每 2 秒定时轮询 | — |
| `DataReceived(String, String, usize)` | 串口收到新数据 | ASCII 字符串, HEX 字符串, 原始字节数 |
| `SerialError(String)` | 串口断开或读取出错 | 错误描述 |

**接收区类**

| 变体 | 携带数据 |
|------|----------|
| `TerminalScrolled { offset_y, viewport_height }` | 滚动偏移和视口高度（px） |
| `SearchTextChanged(String)` | 新的过滤关键字 |

**字号类**

| 变体 | 携带数据 |
|------|----------|
| `FontSizeChanged(f32)` | 新字号（8–28 px） |
| `FontSizeReset` | — |

**全局事件类**

| 变体 | 携带数据 |
|------|----------|
| `ModifiersChanged(bool)` | Ctrl 键当前是否按下 |
| `WheelScrolled(f32)` | 滚轮 delta Y（向上为正，Lines 单位） |

**历史记录类**

| 变体 | 携带数据 |
|------|----------|
| `ClearHistory` | — |
| `HistoryReuse(usize)` | 条目索引 |

**快捷命令类**

| 变体 | 携带数据 |
|------|----------|
| `QuickCommandSend(usize)` | 槽位索引 |
| `QuickCommandStartEdit(usize)` | 槽位索引 |
| `QuickCommandEditName(String)` | 编辑中的名称 |
| `QuickCommandEditText(String)` | 编辑中的内容 |
| `QuickCommandEditHex(bool)` | 编辑中的 HEX 标志 |
| `QuickCommandSaveEdit` | — |
| `QuickCommandCancelEdit` | — |
| `QuickCommandDelete(usize)` | 槽位索引 |
| `ClearQuickCommands` | — |
| `SaveAsQuickCommand` | — |

**日志类**

| 变体 | 携带数据 |
|------|----------|
| `SaveLog` | — |
| `LogDirCustomToggled(bool)` | 是否使用自定义目录 |
| `LogDirChanged(String)` | 新路径字符串（编辑中间状态） |
| `LogDirSaved` | — |
| `LogDirReset` | — |

**提权类**

| 变体 | 携带数据 |
|------|----------|
| `ElevatePrivileges` | — |
| `ElevateResult(String)` | 提权结果消息 |

**CH341 修复类**

| 变体 | 携带数据 |
|------|----------|
| `FixCh341Driver` | — |
| `Ch341FixResult { success: bool, message: String }` | 安装结果 |
| `Ch341InstallTick` | — |

**应用设置类**

| 变体 | 携带数据 |
|------|----------|
| `SendModeChanged(SendMode)` | 新发送模式 |
| `TimestampFormatChanged(TimestampFormat)` | 新时间戳格式 |
| `ShowPortConfig` / `ShowAppSettings` | — |
| `OpenConfigFile` | — |
| `RequestResetAllSettings` / `CancelResetSettings` / `ResetAllSettings` | — |

---

## 后端层 (`src/backend/`)

### 串口 I/O (`mod.rs`)

#### `PortHandle` 类型别名

```rust
pub type PortHandle = Arc<Mutex<Box<dyn SerialPort>>>;
```

线程安全的串口句柄。

- `Box<dyn SerialPort>`：动态分发，`serialport` 返回的具体类型在编译期未知
- `Mutex`：`serial_stream` 订阅线程与 UI 线程（DTR/RTS 切换、发送）并发访问同一句柄
- `Arc`：句柄在 `AppState`、`serial_stream` 闭包、订阅 ID 之间共享所有权

---

#### `refresh_ports`

```rust
pub fn refresh_ports() -> Vec<String>
```

枚举系统当前所有可用串口，返回端口名列表（如 `["/dev/ttyUSB0", "COM3"]`）。

底层调用 `serialport::available_ports()`，失败时返回空列表（不 panic）。

---

#### `open_port`

```rust
pub fn open_port(
    port_name: &str,               // 端口名，如 "COM3" 或 "/dev/ttyUSB0"
    baud: u32,                     // 波特率，如 115200
    data_bits: DataBits,           // 数据位
    parity: Parity,                // 校验位
    stop_bits: StopBits,           // 停止位
    flow_control: FlowControl,     // 流量控制
    disable_modem_handshake: bool, // 是否启用防复位流程
) -> Result<PortHandle, no_reset::OpenError>
```

打开串口，成功返回线程安全句柄。

**防复位流程（`disable_modem_handshake = true`）**

- **Windows**：调用 `no_reset::prepare()` 写注册表 `DisableModemOutHandShake=1`。首次生效需重插 USB，此时返回 `Err(OpenError::NeedReconnect)`；已配置则直接继续。
- **Linux / macOS**：调用 `no_reset::pre_open()` 创建守护 fd，打开完成后调用 `no_reset::post_open()` 保底清零 DTR/RTS。

**错误分类**

| `OpenError` 变体 | 含义 | UI 响应 |
|------------------|------|---------|
| `NeedReconnect(String)` | 注册表已写入，需重插 USB | 接收区显示提示 |
| `PermissionDenied(String)` | 权限不足（Linux 不在 `dialout` 组等） | 显示提权按钮 |
| `Other(String)` | 端口不存在、被占用、参数错误等 | 接收区显示错误 |

**串口超时**：读超时设为 100 ms，防止 `read_data` 永久阻塞。

---

#### `send_data`

```rust
pub fn send_data(
    port: &PortHandle,          // 串口句柄
    text: &str,                 // 发送内容
    hex_mode: bool,             // true = HEX 模式；false = ASCII 模式
    line_ending: &LineEnding,   // 行结束符（HEX 模式下忽略）
) -> Result<usize, String>      // 返回实际发送的字节数
```

向串口发送数据。

- `hex_mode = true`：调用 `parse_hex(text)` 将十六进制字符串转为字节序列，不追加行结束符
- `hex_mode = false`：将 UTF-8 文本直接发送，末尾追加 `line_ending` 对应的字节
- 底层调用 `write_all()` + `flush()`，确保数据完整写入驱动缓冲区

---

#### `read_data`

```rust
pub fn read_data(port: &PortHandle) -> Result<Option<Vec<u8>>, String>
```

非阻塞读取串口数据（底层超时 100 ms）。

| 返回值 | 含义 |
|--------|------|
| `Ok(Some(data))` | 读到新数据，`data` 为原始字节序列 |
| `Ok(None)` | 超时或 `WouldBlock`，无数据（正常，不视为错误） |
| `Err(msg)` | 串口断开或致命 I/O 错误 |

内部用 4096 字节缓冲区循环读取，直到超时或 `WouldBlock`，将所有读到的数据拼接后返回。

---

#### `parse_hex`（私有）

```rust
fn parse_hex(text: &str) -> Result<Vec<u8>, String>
```

将十六进制文本解析为字节序列。

- 接受空格/换行分隔格式（`"41 42 0D 0A"`）和紧凑格式（`"41420D0A"`），两者可混用
- 遇到非 ASCII 字符、非法十六进制字符或奇数长度时返回 `Err`，错误消息包含具体原因

---

#### `bytes_to_hex`

```rust
pub fn bytes_to_hex(data: &[u8]) -> String
```

将字节数组转为大写十六进制字符串，字节间以空格分隔。

示例：`[0x41, 0x0D, 0x0A]` → `"41 0D 0A"`

---

#### `bytes_to_display`

```rust
pub fn bytes_to_display(data: &[u8]) -> String
```

将字节数组转为可读 ASCII 字符串用于终端显示。

- 合法 UTF-8 序列（含中文）原样保留
- 非法 UTF-8 字节（`from_utf8_lossy` 替换为 U+FFFD）→ 显示为 `'.'`
- CRLF (`\r\n`) 和单独 CR (`\r`) 规范化为 LF (`\n`)
- 其他控制字符（除 `\n`、`\t` 外）→ 显示为 `'.'`

---

### 防复位 (`no_reset.rs`)

#### `OpenError` 枚举

```rust
pub enum OpenError {
    NeedReconnect(String),    // 注册表已写，需重插 USB
    PermissionDenied(String), // 权限拒绝
    Other(String),            // 其他错误
}
```

`open_port` 的统一错误类型，由 UI 层匹配后分别处理。

---

#### `prepare`

```rust
pub fn prepare(port_name: &str) -> Result<(), NeedReconnect>
```

打开串口前的平台预处理。

- **Windows**：遍历 `HKLM\SYSTEM\CurrentControlSet\Enum\USB` 找到匹配 `port_name` 的设备实例，检查 `Device Parameters\DisableModemOutHandShake`。已为 `1` 则直接返回 `Ok`；否则写入 `1` 并返回 `Err(NeedReconnect)`（需重插生效）。对于旧版驱动或非 CH340 设备（无此注册表键），静默返回 `Ok`。
- **其他平台**：空操作，直接返回 `Ok`。

---

#### `pre_open`（非 Windows）

```rust
#[cfg(not(windows))]
pub fn pre_open(port_name: &str) -> Option<std::fs::File>
```

Linux/macOS 防复位预处理：创建守护 fd，保持 `open_count > 0` 使驱动跳过 DTR 初始化。

**调用方须在 `serialport::open()` 完成后再 drop 返回值。**

内部步骤：
1. 以 `O_NOCTTY | O_NONBLOCK` 打开设备（不成为控制终端，不等待 DCD）
2. `tcgetattr` 读取当前 termios
3. 设置 `CLOCAL`（忽略 modem 线）、清除 `HUPCL`（关闭时不自动拉低 DTR）
4. `tcsetattr` 应用新 termios
5. `TIOCMBIC` ioctl 立即拉低 DTR 和 RTS
6. 返回 `Some(file)` 持有守护 fd；失败时返回 `None`（降级为仅 `post_open`）

---

#### `post_open`

```rust
pub fn post_open(port: &mut dyn SerialPort)
```

串口打开后的保底后处理。

- **Linux/macOS**：调用 `write_data_terminal_ready(false)` 和 `write_request_to_send(false)`，应对 `pre_open` 失败或个别驱动在 `tcsetattr` 时重置 modem 线的情况。
- **Windows**：空操作（驱动侧已通过注册表处理）。

---

### 应用设置 (`app_settings.rs`)

#### `AppSettings` 结构体

```rust
pub struct AppSettings {
    pub log_dir: PathBuf,           // 自定义日志保存目录
    pub log_dir_custom: bool,       // 是否启用自定义目录
    pub font_size: f32,             // 接收区字号（8–28 px）
    pub left_width: f32,            // 左面板宽度（px）
    pub right_width: f32,           // 右面板宽度（px）
    pub last_port: Option<String>,  // 上次成功连接的端口名
    pub last_baud: String,          // 上次成功连接的波特率
    pub send_mode: SendMode,        // 发送框 Enter 键模式
    pub timestamp_format: TimestampFormat, // 时间戳格式
}
```

---

#### `AppSettings::new`

```rust
pub fn new() -> Self
```

构造实例并立即调用 `load()` 从配置文件恢复设置。默认值：

| 字段 | 默认值 |
|------|--------|
| `font_size` | `13.0` |
| `left_width` | `220.0` |
| `right_width` | `150.0` |
| `last_baud` | `"115200"` |
| `log_dir` | 系统文档目录（`dirs::document_dir()`），不存在则用 home 目录 |

---

#### `AppSettings::save`

```rust
pub fn save(&self)
```

将设置序列化为 JSON 并写入配置文件。失败时静默忽略，不影响程序运行。

配置文件路径：`{config_dir}/iced_serialtool/settings.json`

> **注意**：iced 控件相关的不可序列化字段（如 `combo_box::State`）通过独立的 `Persisted` 中间结构体隔离，仅持久化纯数据字段。

---

#### `AppSettings::load`（私有）

```rust
fn load(&mut self)
```

从配置文件读取并覆盖当前设置。文件不存在或 JSON 解析失败时保留默认值（静默忽略）。使用 `#[serde(default)]` 保证新增字段向后兼容旧版配置文件。

---

#### `AppSettings::config_path`

```rust
pub fn config_path() -> PathBuf
```

返回配置文件的绝对路径，并自动创建父目录（`iced_serialtool/`）。

---

#### `AppSettings::effective_log_dir`

```rust
pub fn effective_log_dir(&self) -> PathBuf
```

返回实际使用的日志保存目录：

- `log_dir_custom == false`：exe 所在目录（安装版为安装目录，开发时为当前目录）
- `log_dir_custom == true`：用户自定义的 `log_dir`

---

#### `AppSettings::log_dir_valid`

```rust
pub fn log_dir_valid(&self) -> bool
```

检查 `log_dir` 指向的路径是否存在（用于在设置面板显示目录不存在警告）。

---

#### `AppSettings::reset_to_defaults`

```rust
pub fn reset_to_defaults(&mut self)
```

将所有用户偏好恢复为默认值并立即保存。`last_port` 和 `last_baud` 不重置（复位设置通常是调整 UI 偏好，不应让用户重新选设备）。

---

### 发送历史 (`history.rs`)

#### `History` 结构体

内存中的发送历史记录，持久化到 `{config_dir}/iced_serialtool/history.json`。始终按时间倒序存储（`index 0 = 最新`），方便键盘导航（`↑` 键直接取 `[0]`）。

最多保留 50 条（`MAX_ITEMS = 50`）。

---

#### `History::new`

```rust
pub fn new() -> Self
```

构造实例并调用 `load()` 从文件恢复历史记录。

---

#### `History::add`

```rust
pub fn add(&mut self, text: String, is_hex: bool)
```

添加一条记录到列表头部（最新位置），超出 50 条时丢弃最旧记录，并立即持久化。空白内容（`text.trim().is_empty()`）不添加。

**参数**

| 参数 | 说明 |
|------|------|
| `text` | 发送的原始内容 |
| `is_hex` | 是否为 HEX 模式 |

---

#### `History::clear`

```rust
pub fn clear(&mut self)
```

清空所有历史记录并立即持久化（写入空数组 JSON）。

---

#### `History::items`

```rust
pub fn items(&self) -> &[HistoryItem]
```

返回所有历史记录的只读切片，按倒序排列（`[0]` = 最新）。

---

### 快捷命令 (`quick_commands.rs`)

#### `QuickCommands` 结构体

固定 20 个槽位（`MAX_ITEMS = 20`）的快捷命令集合，持久化到 `{config_dir}/iced_serialtool/quick_commands.json`。

固定槽位设计使 UI 位置稳定，用户可建立"位置 = 功能"的肌肉记忆，增删不改变其他命令的位置。

---

#### `QuickCommands::new`

```rust
pub fn new() -> Self
```

构造实例并调用 `load()`。始终保证内存中有完整的 20 个槽位（文件中不足时补充默认空槽）。

---

#### `QuickCommands::items`

```rust
pub fn items(&self) -> &[QuickCommand]
```

返回所有 20 个槽位的只读切片，包含空槽。

---

#### `QuickCommands::send_at`

```rust
pub fn send_at(&self, index: usize) -> Option<&QuickCommand>
```

返回指定槽位的命令引用。空槽（`text.trim().is_empty()`）返回 `None`，防止意外发送空内容。

**参数**：`index` — 槽位索引（0–19）

---

#### `QuickCommands::edit`

```rust
pub fn edit(&mut self, index: usize, name: String, text: String, is_hex: bool)
```

更新指定槽位的名称、内容和 HEX 标志，并立即持久化。

---

#### `QuickCommands::add_to_first_empty`

```rust
pub fn add_to_first_empty(&mut self, text: &str, is_hex: bool) -> Option<usize>
```

将内容写入第一个空槽位，返回写入的槽位索引；所有槽位已满时返回 `None`。

---

#### `QuickCommands::delete`

```rust
pub fn delete(&mut self, index: usize)
```

逻辑删除：将指定槽位重置为默认空槽（名称恢复为 `"命令 N"`，内容清空，HEX 标志置 `false`），不物理移除数组元素。

---

#### `QuickCommands::clear_all`

```rust
pub fn clear_all(&mut self)
```

清空所有槽位（全部逻辑删除），并立即持久化。

---

### 提权 (`privileges.rs`)

#### `elevate`

```rust
pub fn elevate(port_name: &str) -> String
```

申请串口访问权限，返回结果消息字符串（用于显示在接收区）。**应在 `tokio::task::spawn_blocking` 中调用。**

**Windows 实现**

通过 PowerShell `Start-Process -Verb RunAs` 弹出 UAC 对话框，批准后以管理员身份重启程序，当前（低权限）进程约 800 ms 后自动退出。

**Linux 实现（阻塞执行）**

1. 生成 udev 规则内容（覆盖 `ttyUSB*`、`ttyACM*`、`ttyS*`、`ttyCH341USB*`，`MODE="0666"`）
2. 通过 `pkexec sh -c "<cmd>"` 执行：`mv` 规则文件 → `chmod` 修复已连接设备节点 → `udevadm control --reload-rules` → `udevadm trigger`
3. `pkexec` 不存在时返回手动执行命令的提示

---

### CH341 修复 (`ch341_fix.rs`)（Linux only）

#### `check_dtr_fix_needed`

```rust
pub fn check_dtr_fix_needed() -> bool
```

检测当前 ch341 内核模块是否缺少 `no_dtr_on_open` 参数（即打开串口时会拉高 DTR 导致目标复位）。

检测逻辑：
- 若 `/sys/module/ch341/parameters/no_dtr_on_open` 存在 → 检查其值是否为 `"Y"`
- 若模块未加载 → 检查 `/etc/modprobe.d/ch341.conf` 是否含 `no_dtr_on_open=1`
- 两者均不满足 → 返回 `true`（需要修复）

---

#### `check_build_env`

```rust
pub fn check_build_env() -> BuildEnvReport
```

检查编译驱动所需的环境依赖：`gcc`、`make`、`git` 和当前内核对应的头文件（`/lib/modules/{kernel}/build`）。

返回 `BuildEnvReport`：
- `missing: Vec<String>` — 缺少的工具或头文件名
- `install_hint: String` — 针对当前发行版（Arch/Debian/Fedora）生成的安装命令

---

#### `run_install`

```rust
pub fn run_install() -> InstallResult
```

编译安装含 `no_dtr_on_open` 补丁的 ch341 内核模块。**应在 `tokio::task::spawn_blocking` 中调用，会阻塞数十秒。**

流程：
1. 将编译时内嵌的驱动源码（`ch341.c`、`ch341.h`、`Makefile`）写入临时目录
2. 将安装脚本（`install.sh`）写入临时文件并赋予执行权限
3. 通过 `pkexec bash install.sh <src_dir>` 以 root 权限编译和加载模块
4. 检查 stdout 是否含 `"SUCCESS"` 判断结果

返回 `InstallResult { success: bool, message: String }`。

---

## UI 层 (`src/ui/`)

### 应用状态机 (`mod.rs`)

#### `AppState` 结构体

```rust
pub struct AppState {
    pub monitor: SerialMonitor,                              // UI 层状态
    pub history: History,                                    // 发送历史
    pub quick_commands: QuickCommands,                       // 快捷命令
    pub port_handle: Option<PortHandle>,                     // 串口句柄
    pub app_settings: AppSettings,                           // 应用设置
    pub quick_cmd_editing: Option<(usize, String, String, bool)>, // 内联编辑状态
    pub reset_confirm_pending: bool,                         // 重置确认对话框状态
    pub font_size_dirty: bool,                               // Ctrl+滚轮调字号时置 true
}
```

`quick_cmd_editing` 用匿名元组而非具名结构体，因为此状态完全局限于 sidebar 的单次编辑流程，无需跨模块传递。

---

#### `AppState::new`

```rust
pub fn new() -> (Self, iced::Task<Message>)
```

iced `application()` builder 要求的构造函数签名，调用 `boot()` 初始化状态，并返回启动任务 `Task::done(Message::RefreshPorts)`（立即刷新端口列表）。

---

#### `boot`

```rust
pub fn boot() -> AppState
```

完整初始化 `AppState`：加载 `QuickCommands`、`AppSettings`（含历史上次端口/波特率回填）、`History`；初始化所有子结构体为默认值；Linux 下检测 ch341 修复需求。

---

#### `update`

```rust
pub fn update(state: &mut AppState, message: Message) -> iced::Task<Message>
```

iced 状态机入口，处理所有 `Message` 变体。大多数消息同步处理返回 `Task::none()`；需要异步操作的（`TogglePort`、`ElevatePrivileges`、`FixCh341Driver`）返回 `Task::perform()`。

**关键路径**

`TogglePort`（打开串口）：
1. 解析端口名和波特率（失败时 `push_line` 错误消息并返回）
2. 调用 `backend::open_port()`
3. 成功：同步 DTR/RTS 信号到 UI 开关状态；`push_line` 连接消息；保存 `last_port/last_baud`
4. 失败：按 `OpenError` 变体分别处理（NeedReconnect/PermissionDenied/Other）

`EditSend`（发送框操作）：
1. 检测 `↑`/`↓` 键，实现历史导航状态机（`history_nav_index`）
2. 检测 `Edit::Enter`，根据 `send_mode` 和 `ctrl_held` 决定发送还是换行
3. 其他操作透传给 `send_content.perform(action)`

`DataReceived`：
1. 累加 `rx_count`
2. 若 `pause_receive`，丢弃显示（但已计数）
3. 分行追加到 `received_lines`，超过 2000 行时丢弃头部
4. `auto_scroll=true` 或有搜索关键字时调用 `rebuild_receive_content()`

---

#### `push_line`（私有）

```rust
fn push_line(monitor: &mut SerialMonitor, line: String)
```

追加一条系统消息（连接状态、错误提示等）到接收区，并立即将光标移到文档末尾（无论 `auto_scroll` 状态）。串口数据流不走此函数，由 `DataReceived` 处理路径直接操作 `received_lines`。

---

#### `rebuild_receive_content`（私有）

```rust
fn rebuild_receive_content(monitor: &mut SerialMonitor)
```

将 `received_lines`（按 `search_text` 过滤后）同步到 `receive_content`（iced text_editor 内容）。

- `auto_scroll=true`：将光标移到文档末尾触发滚动
- `auto_scroll=false`：不移动光标，保留用户阅读位置

---

#### `fmt_timestamp`（私有）

```rust
fn fmt_timestamp(fmt: TimestampFormat) -> String
```

按当前设置格式化当前时刻为时间戳字符串，用于在行前插入时间前缀。

| 格式 | 输出示例 |
|------|----------|
| `TimeOnly` | `12:34:56` |
| `TimeMsec` | `12:34:56.789` |
| `DateTime` | `2026-06-06 12:34:56` |

---

#### `subscription`

```rust
pub fn subscription(state: &AppState) -> Subscription<Message>
```

注册所有异步事件源，通过 `Subscription::batch()` 合并后返回。

| 订阅 | 条件 | 产生消息 |
|------|------|----------|
| `drag_subscription` | 始终 | `DragMoved` / `DragEnded` |
| `serial_stream` | `port_handle.is_some()` | `DataReceived` / `SerialError` |
| `time::every(interval)` | `timed_send && is_connected` | `SendMessage` |
| `time::every(2s)` | 始终 | `PollPorts` |
| `time::every(1s)` | `ch341_fix_installing` | `Ch341InstallTick` |
| 全局键盘/鼠标事件 | 始终 | `ModifiersChanged` / `WheelScrolled` |

---

#### `serial_stream`（私有）

```rust
fn serial_stream(port: &PortSub) -> BoxStream<'static, Message>
```

串口数据读取流，在 iced 订阅系统内持续运行，直到 `port_handle` 被置 `None`（订阅 ID 变化触发重建）。

每次迭代：
1. `tokio::time::sleep(20ms)` — 避免空转消耗 CPU
2. `tokio::task::spawn_blocking` — 在阻塞线程调用 `backend::read_data`（底层有 100ms 超时）
3. 有数据：调用 `bytes_to_display` 和 `bytes_to_hex` 转换后发送 `DataReceived`
4. 无数据：跳过
5. 出错：发送 `SerialError`

消息队列容量 100（背压保护，突发时最多积压 100 条）。

---

#### `view`

```rust
pub fn view(state: &AppState) -> Element<'_, Message>
```

将状态渲染为 iced Element 树。三列布局：

```
[ 左面板 (left_width px) ] [ 分隔条 ] [ 终端 (Fill) ] [ 分隔条 ] [ 右面板 (right_width px) ]
```

- 折叠状态下，左/右面板替换为宽 32 px 的展开按钮条
- 左面板根据 `left_panel_view` 切换串口配置页或应用设置页

---

### 分隔条 (`splitter.rs`)

#### `split_handle`

```rust
pub fn split_handle<'a>(
    target: DragTarget,  // 标识是左分隔条还是右分隔条
    is_dragging: bool,   // 当前是否正在被拖拽（高亮显示）
    is_hovering: bool,   // 当前鼠标是否悬停（高亮显示）
) -> Element<'a, Message>
```

渲染一个可拖拽的分隔条（4 px 宽）。

- 悬停或拖拽时显示蓝色高亮背景（`rgba(0.3, 0.55, 0.95, 0.6)`）
- 鼠标样式切换为 `ResizingHorizontally`（双向水平箭头）
- 产生消息：`DragStarted`（按下）、`HandleHoverChanged`（悬停进出）

---

#### `drag_subscription`

```rust
pub fn drag_subscription(dragging: Option<DragTarget>) -> Subscription<Message>
```

拖拽期间的全局鼠标事件订阅。`dragging.is_none()` 时返回 `Subscription::none()` 节省资源。

使用全局事件而非控件事件，是因为用户可能将鼠标拖出分隔条区域，仍需捕获移动和释放。

- 鼠标左键释放 → `Message::DragEnded`
- 鼠标移动 → `Message::DragMoved(position.x)`

---

### 终端面板 (`terminal.rs`)

#### `view_terminal`

```rust
pub fn view_terminal<'a>(
    state: &'a TerminalState,  // 终端状态
    is_dark_mode: bool,        // 主题（影响图标颜色）
    font_size: f32,            // 接收区字号（8–28 px）
) -> widget::Column<'a, Message>
```

渲染中间终端面板，包含：

1. **接收区工具栏**：下载图标、`RX: N B` 计数、关键字过滤输入框 + 匹配条数、HEX 显示开关、时间戳开关、自动滚动开关、暂停按钮、字号显示（点击重置）、清空按钮、保存日志按钮
2. **接收区**：`text_editor`（只读，支持文本选中和 Ctrl+C 复制），等宽字体，可配置字号
3. **发送区工具栏**：上传图标、`TX: N B` 计数、HEX 发送开关、行结束符选择、定时发送（开关 + 间隔输入）、存为快捷命令按钮、清空发送框按钮、发送按钮
4. **发送区**：多行 `text_editor`

---

### 设置面板 (`settings.rs`)

#### `view_port_config`

```rust
pub fn view_port_config<'a>(
    config: &'a PortConfig,
    is_connected: bool,
    is_dark_mode: bool,
    show_elevate_hint: bool,
    ch341_fix_needed: bool,
    ch341_fix_installing: bool,
    ch341_install_elapsed: u32,
) -> widget::Column<'a, Message>
```

渲染左侧串口配置面板：

- 顶部工具栏：应用名称 + 齿轮图标（切换到应用设置页）+ 主题切换按钮
- 端口下拉框（每 2 秒自动刷新列表）
- 波特率 combo_box（预设 + 自定义输入）
- 数据位 / 停止位 / 校验位 / 流控 下拉框（各 4 个或更少选项）
- DTR / RTS 切换开关（连接中可实时生效）
- 防复位开关（`disable_modem_handshake`）
- 权限错误提示 + 提权按钮（`show_elevate_hint = true` 时显示）
- CH341 修复按钮 + 安装进度（Linux，`ch341_fix_needed = true` 时显示）
- 打开 / 关闭串口按钮

**预设波特率列表**：`BAUD_PRESETS` 常量，包含 1200 到 6000000 等常见值。

---

#### `view_app_settings`

```rust
pub fn view_app_settings<'a>(
    settings: &'a AppSettings,
    is_dark_mode: bool,
    reset_confirm_pending: bool,
) -> widget::Column<'a, Message>
```

渲染应用设置页（左侧面板，由齿轮图标进入）：

- 发送模式选择（`EnterSend` / `EnterNewline`）
- 时间戳格式选择
- 日志保存目录（默认 / 自定义切换 + 路径输入 + 保存/重置按钮）
- 打开配置文件目录
- 重置所有设置（带二次确认对话框）

---

### 侧边栏 (`sidebar.rs`)

#### `view_sidebar`

```rust
pub fn view_sidebar<'a>(
    is_dark_mode: bool,
    history: &'a History,
    quick_commands: &'a QuickCommands,
    panel_width: f32,
    editing: Option<&'a (usize, String, String, bool)>,
) -> widget::Column<'a, Message>
```

渲染右侧侧边栏，包含两个可折叠区域：

**快捷命令区**（上方）：
- 20 个槽位，每行显示命令名称（或默认 `"命令 N"`）
- 点击按钮发送（`QuickCommandSend(i)`）
- 铅笔图标进入内联编辑（名称 + 内容 + HEX 开关 + 保存/取消/删除）
- 底部「清空所有命令」按钮

**发送历史区**（下方）：
- 最近 50 条记录，显示时间和内容摘要
- 点击条目发送 `HistoryReuse(i)`
- 顶部「清空历史」按钮

`editing`（内联编辑状态）：`Some((index, name, text, is_hex))` 时将对应槽位替换为编辑表单。

---

### 主题 (`theme.rs`)

#### `get_theme`

```rust
pub fn get_theme(monitor: &SerialMonitor) -> iced::Theme
```

根据 `monitor.is_dark_mode` 返回对应的 iced 主题：

- `true` → `Theme::TokyoNight`（深色）
- `false` → `Theme::CatppuccinLatte`（浅色）

---

### 样式 (`styles.rs`)

控件外观样式的统一出口，所有颜色常量和样式函数集中于此，UI 各模块直接引用，不在各处硬编码颜色值。

#### 颜色常量

| 常量 | RGB 近似值 | 用途 |
|------|-----------|------|
| `COLOR_SUCCESS` | `(0.45, 0.75, 0.45)` 绿色 | RX 字节计数、连接成功提示 |
| `COLOR_ACCENT_TX` | `(0.45, 0.55, 0.85)` 蓝色 | TX 字节计数 |
| `COLOR_DIM` | `(0.55, 0.55, 0.55)` 灰色 | 连接中只读参数、辅助说明文字 |
| `COLOR_WARNING` | `(0.90, 0.60, 0.10)` 橙色 | 目录不存在警告、重置确认提示 |
| `COLOR_SECTION_LABEL` | `(0.45, 0.45, 0.55)` 蓝灰色 | 设置面板各分组标签 |
| `COLOR_CONNECTED` | `(0.20, 0.80, 0.20)` 亮绿色 | `● 已连接` 状态指示点 |

---

#### `icon_color`

```rust
pub fn icon_color(is_dark: bool) -> iced::Color
```

根据主题返回 SVG 图标的着色颜色：

- 深色模式：`rgb(0.85, 0.85, 0.88)`（近白色，在深色背景上清晰）
- 浅色模式：`rgb(0.35, 0.35, 0.40)`（深灰色，在浅色背景上清晰）

---

#### `panel_container`

```rust
pub fn panel_container(theme: &iced::Theme) -> widget::container::Style
```

三列面板的背景容器样式，含圆角（8 px）和细边框。

| 状态 | 背景色 | 边框色 |
|------|--------|--------|
| 深色 | `rgb(0.15, 0.15, 0.18)` | `rgba(0.25, 0.25, 0.30, 0.5)` |
| 浅色 | `rgb(0.97, 0.97, 0.98)` | `rgba(0.75, 0.75, 0.80, 0.5)` |

---

#### `button_primary`

```rust
pub fn button_primary(theme: &iced::Theme, status: widget::button::Status) -> widget::button::Style
```

主操作按钮样式（蓝色填充，白色文字），用于「打开串口」「发送」等主要动作。响应 `Active`、`Hovered`、`Pressed`、`Disabled` 四种状态，`Disabled` 时透明度降为 50%。

---

#### `button_subtle`

```rust
pub fn button_subtle(theme: &iced::Theme, status: widget::button::Status) -> widget::button::Style
```

低视觉权重按钮样式（透明背景，跟随主题文字色），用于工具栏图标按钮、折叠展开按钮等辅助操作。悬停时显示极淡的半透明遮罩。

---

#### `rx_editor`

```rust
pub fn rx_editor(theme: &iced::Theme, _status: widget::text_editor::Status) -> widget::text_editor::Style
```

接收区只读 `text_editor` 的外观样式。背景与 `panel_container` 内容区配色一致，无输入框视觉效果（无高亮边框），选中文字使用蓝色半透明高亮（`rgba(0.25, 0.55, 0.95, 0.35)`）。

---

#### `rule_style`

```rust
pub fn rule_style(theme: &iced::Theme) -> widget::rule::Style
```

设置面板各分组之间的分隔线样式。深色模式用 `rgba(0.3, 0.3, 0.35, 0.4)`，浅色模式用 `rgba(0.7, 0.7, 0.75, 0.4)`，全宽绘制（`FillMode::Full`）。

---

### 图标 (`icons.rs`)

所有 SVG 图标通过 `include_str!` 在编译时嵌入二进制，运行时无需外部文件。

#### `icon`（私有辅助函数）

```rust
fn icon(svg_data: &'static str, size: f32, is_dark: bool) -> widget::Svg<'static>
```

从编译时内嵌的 SVG 字符串创建着色图标控件。颜色由 `styles::icon_color(is_dark)` 统一管理，深色/浅色模式自动切换。

**参数**

| 参数 | 说明 |
|------|------|
| `svg_data` | 编译时嵌入的 SVG 字符串（`include_str!` 结果） |
| `size` | 图标宽高（px），正方形 |
| `is_dark` | 当前是否深色主题 |

---

#### 公开图标函数

所有图标函数签名形如 `pub fn name(size: f32, is_dark: bool) -> widget::Svg<'static>`，对应图标文件均在 `assets/icons/` 目录下：

| 函数 | 图标文件 | 使用位置 |
|------|----------|----------|
| `settings` | `settings.svg` | 左面板工具栏：进入应用设置页 |
| `plug` | `plug.svg` | 串口配置面板：连接状态指示 |
| `download` | `download.svg` | 终端面板接收区工具栏 |
| `upload` | `upload.svg` | 终端面板发送区工具栏 |
| `trash` | `trash-2.svg` | 清空历史、清空快捷命令按钮 |
| `eraser` | `eraser.svg` | 清空接收区、清空发送框按钮 |
| `send` | `send.svg` | 发送按钮 |
| `arrow_down_to_line` | `arrow-down-to-line.svg` | 保存日志按钮 |
| `timer` | `timer.svg` | 定时发送开关旁图标 |
| `usb` | `usb.svg` | 暂未使用，保留备用 |
| `clock` | `clock.svg` | 暂未使用，保留备用 |
