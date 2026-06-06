pub mod app_settings;
#[cfg(target_os = "linux")]
pub mod ch341_fix;
pub mod history;
pub mod no_reset;
pub mod privileges;
pub mod quick_commands;

use crate::types::{DataBits, FlowControl, LineEnding, Parity, StopBits};
use serialport::SerialPort;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 线程安全的串口句柄。
/// - `Box<dyn SerialPort>`：串口的所有权包装，支持动态分发（serialport 返回的具体类型在编译期未知）
/// - `Mutex`：serial_stream 订阅线程和 UI 线程（DTR/RTS 切换、发送）会并发访问同一句柄
/// - `Arc`：句柄需在 AppState、serial_stream 闭包、subscription ID 之间共享所有权
pub type PortHandle = Arc<Mutex<Box<dyn SerialPort>>>;

/// 返回系统当前所有可用串口的名称列表（如 ["/dev/ttyUSB0", "COM3"] ）
pub fn refresh_ports() -> Vec<String> {
    serialport::available_ports()
        .unwrap_or_default()
        .iter()
        .map(|p| p.port_name.clone())
        .collect()
}

/// 打开指定串口并返回线程安全的句柄。
///
/// - `disable_modem_handshake`：true 时启用防复位流程：
///   - Windows：写注册表 DisableModemOutHandShake，首次需重插 USB（`OpenError::NeedReconnect`）
///   - Linux/macOS：pre_open 方案——先用守护 fd 打开设备并立即清除 DTR/RTS，
///     再让 serialport 以第二个 fd 打开（驱动 open_count 已为 1，跳过 DTR 初始化），
///     serialport 打开完成后关闭守护 fd，并再次 post_open 保底清零。
///     全程 DTR 在极短脉冲（≤几十微秒）后即恢复低电平，不足以触发目标设备复位。
pub fn open_port(
    port_name: &str,
    baud: u32,
    data_bits: DataBits,
    parity: Parity,
    stop_bits: StopBits,
    flow_control: FlowControl,
    disable_modem_handshake: bool,
) -> Result<PortHandle, no_reset::OpenError> {
    // 平台预处理（Windows: 检查/写注册表）
    if disable_modem_handshake {
        no_reset::prepare(port_name).map_err(no_reset::OpenError::from)?;
    }

    // Linux/macOS：pre_open 守护 fd。
    // 必须在 serialport::open() 之前创建、之后才销毁，
    // 使驱动在 serialport 的 open 调用时看到 open_count > 0，跳过 DTR 初始化。
    // 失败时降级为仅 post_open 方案，不影响串口正常打开。
    #[cfg(not(windows))]
    let _guard = if disable_modem_handshake {
        no_reset::pre_open(port_name)
    } else {
        None
    };

    // 将 UI 层枚举转换为 serialport crate 的枚举。
    // types.rs 中定义了镜像枚举，使 UI 层不直接依赖 serialport，方便将来替换底层库。
    let data_bits = match data_bits {
        DataBits::Five => serialport::DataBits::Five,
        DataBits::Six => serialport::DataBits::Six,
        DataBits::Seven => serialport::DataBits::Seven,
        DataBits::Eight => serialport::DataBits::Eight,
    };

    let parity = match parity {
        Parity::None => serialport::Parity::None,
        Parity::Odd => serialport::Parity::Odd,
        Parity::Even => serialport::Parity::Even,
    };

    let stop_bits = match stop_bits {
        StopBits::One => serialport::StopBits::One,
        StopBits::Two => serialport::StopBits::Two,
    };

    let flow_control = match flow_control {
        FlowControl::None => serialport::FlowControl::None,
        FlowControl::Hardware => serialport::FlowControl::Hardware,
        FlowControl::Software => serialport::FlowControl::Software,
    };

    // 100ms 读超时：read_data 在阻塞线程中调用，超时后返回 TimedOut 而不是永久阻塞。
    // 值不宜过小（频繁超时增加 CPU 开销），也不宜过大（会延迟 SerialError 的发现）。
    let mut port = serialport::new(port_name, baud)
        .data_bits(data_bits)
        .parity(parity)
        .stop_bits(stop_bits)
        .flow_control(flow_control)
        .timeout(Duration::from_millis(100))
        .open()
        .map_err(|e| {
            let msg = e.to_string();
            // 权限错误单独分类，以便 UI 层显示提权按钮
            if msg.contains("Permission denied")
                || msg.contains("Access is denied")
                || msg.contains("os error 13")
            {
                no_reset::OpenError::PermissionDenied(msg)
            } else {
                no_reset::OpenError::Other(format!("打开失败: {}", msg))
            }
        })?;

    // post_open 保底：serialport 完成配置后再清一次 DTR/RTS，
    // 应对 pre_open 失败或个别驱动在 tcsetattr 时重置 modem 线的情况。
    if disable_modem_handshake {
        no_reset::post_open(port.as_mut());
    }

    // _guard 在此处 drop：守护 fd 关闭，open_count 归 1，serialport 独占设备。
    // DTR/RTS 已由 post_open 确保为低电平，不受 HUPCL=0 的影响。

    Ok(Arc::new(Mutex::new(port)))
}

/// 向串口发送数据，返回实际发送的字节数。
///
/// - `hex_mode = true`：将 `text` 解析为十六进制字节序列（如 "41 42" → [0x41, 0x42]），
///   此模式下 `line_ending` 参数被忽略——HEX 数据通常是精确的字节流，不应追加额外字节。
/// - `hex_mode = false`：将 `text` 按 UTF-8 字节直接发送，并在末尾追加 `line_ending`。
pub fn send_data(
    port: &PortHandle,
    text: &str,
    hex_mode: bool,
    line_ending: &LineEnding,
) -> Result<usize, String> {
    let mut data = if hex_mode {
        parse_hex(text)?
    } else {
        text.as_bytes().to_vec()
    };

    // HEX 模式不追加结束符：HEX 数据是精确字节流，调用方自己编码行结束
    if !hex_mode {
        match line_ending {
            LineEnding::None => {}
            LineEnding::CrLf => data.extend_from_slice(b"\r\n"),
            LineEnding::Lf => data.extend_from_slice(b"\n"),
            LineEnding::Cr => data.extend_from_slice(b"\r"),
        }
    }

    let mut port = port.lock().map_err(|e| format!("锁失败: {}", e))?;
    port.write_all(&data)
        .map_err(|e| format!("发送失败: {}", e))?;
    port.flush().map_err(|e| format!("flush 失败: {}", e))?;

    Ok(data.len())
}

/// Ok(Some(data)) — 收到数据
/// Ok(None)       — 超时，无数据（正常）
/// Err(msg)       — 串口断开或致命错误
pub fn read_data(port: &PortHandle) -> Result<Option<Vec<u8>>, String> {
    let mut port = port.lock().map_err(|e| format!("锁失败: {}", e))?;
    let mut buf = [0u8; 4096];
    let mut result = Vec::new();

    loop {
        match port.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => result.extend_from_slice(&buf[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) => return Err(format!("读取失败: {}", e)),
        }
    }

    Ok(if result.is_empty() { None } else { Some(result) })
}

/// 将十六进制文本解析为字节序列。
/// 接受空格/换行分隔的格式（如 "41 42\n43"）以及紧凑格式（"414243"），两者可混用。
/// 遇到非法字符、奇数长度时返回带提示的 Err。
fn parse_hex(text: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = text.split_whitespace().collect();
    if !cleaned.is_ascii() {
        return Err("HEX 格式错误：只允许十六进制字符（0-9 A-F a-f）".to_string());
    }
    if cleaned.len() % 2 != 0 {
        return Err("HEX 格式错误：字符数必须为偶数".to_string());
    }

    let mut bytes = Vec::new();
    for i in (0..cleaned.len()).step_by(2) {
        let byte = u8::from_str_radix(&cleaned[i..i + 2], 16)
            .map_err(|_| format!("HEX 格式错误：包含非法字符 '{}'", &cleaned[i..i + 2]))?;
        bytes.push(byte);
    }

    Ok(bytes)
}

/// 将字节数组转为大写十六进制字符串，字节间以空格分隔（如 [0x41, 0x0D] → "41 0D"）
pub fn bytes_to_hex(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn bytes_to_display(data: &[u8]) -> String {
    // from_utf8_lossy：合法 UTF-8 序列（含中文）原样保留，非法字节替换为 U+FFFD
    // 先规范化换行，再把 U+FFFD 和其他控制字符替换为 '.'
    String::from_utf8_lossy(data)
        .replace("\r\n", "\n")  // CRLF → LF
        .replace('\r', "\n")    // 单独 CR → LF
        .chars()
        .map(|c| match c {
            '\u{FFFD}' => '.',                                    // 非法 UTF-8 字节
            c if c.is_control() && c != '\n' && c != '\t' => '.', // 其他控制字符
            c => c,
        })
        .collect()
}
