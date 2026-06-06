# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```powershell
cargo run --bin serial-debugger          # 开发运行（debug 构建，保留控制台）
cargo check                              # 快速类型检查，不产生二进制
cargo build --release --bin serial-debugger  # release 构建

# 打包（Windows，需已安装 NSIS）
cargo packager --release --formats nsis

# 打包（Linux）
cargo packager --release --formats deb,appimage,pacman
cargo build --release --bin serial-debugger && cargo generate-rpm  # RPM 单独处理
```

详细打包说明见 `PACKAGING.md`。

## 架构

### iced 应用模式

项目遵循 iced 0.14 的 `application()` builder 模式，所有状态和消息集中管理：

```
main.rs
  └─ iced::application(AppState::new, ui::update, ui::view)
       ├─ theme      → ui::theme::get_theme(&state.monitor)
       ├─ subscription → ui::subscription()
       └─ window     → icon 从 icons/128x128.png 编译时嵌入
```

### 状态与消息

- **`src/types.rs`** — 所有类型的单一来源：`SerialMonitor`（主状态结构体，含所有 UI 字段）、`Message` 枚举（覆盖所有可能的用户动作和事件）、串口参数枚举（`DataBits`、`Parity` 等）
- **`src/ui/mod.rs`** — `AppState`（包装 `SerialMonitor` + `History` + `QuickCommands` + 串口句柄）、`update()`（状态机入口）、`view()`（组装三列布局）、`subscription()`（注册异步事件）

### 异步事件来源（subscription）

三条订阅并行运行：
1. `serial_stream` — 每 20ms 轮询串口，产生 `Message::DataReceived`
2. `drag_subscription`（`ui/splitter.rs`）— 监听鼠标事件，产生 `DragMoved` / `DragEnded`
3. `time::every`（仅定时发送开启时）— 产生 `Message::SendMessage`

### UI 层（三列布局）

| 模块 | 对应面板 | 职责 |
|------|----------|------|
| `ui/settings.rs` | 左列 | 端口/波特率/参数配置、DTR/RTS、打开/关闭 |
| `ui/terminal.rs` | 中列 | 接收区（滚动文本）+ 发送区（编辑器） |
| `ui/sidebar.rs`  | 右列 | 快捷命令列表 + 发送历史列表 |

列宽由 `ui/splitter.rs` 的拖拽逻辑控制，状态存储在 `SerialMonitor.left_width` 和 `right_width` 字段中（单位 px）。

### 后端层

- **`backend/mod.rs`** — 串口 I/O 核心：`open_port()`、`read_data()`（非阻塞）、`send_data()`（支持 ASCII/HEX 模式和行结束符追加）
- **`backend/no_reset.rs`** — Windows 平台特定：通过写注册表 `DisableModemOutHandShake=1` 防止 USB 串口打开时复位设备；首次设置后返回 `NeedReconnect` 错误要求用户重新插拔
- **`backend/history.rs`** — 最近 50 条发送记录，内存维护
- **`backend/quick_commands.rs`** — 20 个槽位的快捷命令，持久化到 `~/.config/iced_serialtool/quick_commands.json`

### 串口句柄生命周期

`PortHandle` 是 `Arc<Mutex<Box<dyn SerialPort>>>` 的类型别名，存储在 `AppState.port`（`Option<PortHandle>`）。打开时赋值，关闭或出错时置 `None`，`subscription` 中检查 `None` 来停止轮询。

### Windows 特殊处理

- `build.rs` 通过 `winresource` 在编译时将 `icons/icon.ico` 和 ProductName/CompanyName 嵌入 EXE
- `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` 使 release 构建不弹出控制台窗口
- `[target.'cfg(windows)'.dependencies]` 中的 `winreg` 仅供 `no_reset.rs` 使用
