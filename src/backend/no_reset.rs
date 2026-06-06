/// 注册表已写入，需要用户重新插拔 USB 才能生效（首次配置时触发）。
/// 重新插拔是因为 Windows 在设备枚举时读取注册表参数，写入后必须重新枚举才能生效。
#[derive(Debug)]
pub struct NeedReconnect(pub String);

impl std::fmt::Display for NeedReconnect {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "已完成配置，请重新插拔 USB 数据线后重试（端口：{}）", self.0)
    }
}
impl std::error::Error for NeedReconnect {}

/// open_port 的统一错误类型
pub enum OpenError {
    /// 配置已写入，需要用户重新插拔 USB
    NeedReconnect(String),
    /// 系统拒绝访问串口（Linux: 不在 dialout/uucp 组；Windows: 需要管理员权限）
    PermissionDenied(String),
    /// 其他错误（端口不存在、被占用、参数错误等）
    Other(String),
}

impl From<NeedReconnect> for OpenError {
    fn from(e: NeedReconnect) -> Self {
        OpenError::NeedReconnect(e.to_string())
    }
}

/// 打开串口前的平台预处理。
/// Windows: 检查并写入 DisableModemOutHandShake 注册表键。
/// 其他平台: 无需处理，直接返回 Ok。
pub fn prepare(port_name: &str) -> Result<(), NeedReconnect> {
    platform::prepare(port_name)
}

/// 预打开：Linux/macOS 专用，返回守护文件对象。
/// 调用方须在 serialport::open() 完成后再 drop 返回值。
/// Windows 上此函数不存在（由 cfg 控制）。
#[cfg(not(windows))]
pub fn pre_open(port_name: &str) -> Option<std::fs::File> {
    platform::pre_open(port_name)
}

/// 打开串口后的保底后处理：清除 DTR/RTS。
/// Windows: 驱动已由注册表配置处理，无操作。
/// Unix: 应对 pre_open 失败或驱动行为不一致的情况。
pub fn post_open(port: &mut dyn serialport::SerialPort) {
    platform::post_open(port);
}

// ── Windows 实现 ─────────────────────────────────────────────────────────────

#[cfg(windows)]
mod platform {
    use super::NeedReconnect;
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    pub fn prepare(port_name: &str) -> Result<(), NeedReconnect> {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

        let enum_usb = match hklm.open_subkey("SYSTEM\\CurrentControlSet\\Enum\\USB") {
            Ok(k) => k,
            Err(_) => return Ok(()), // 无 USB 枚举节点，静默跳过
        };

        // 注册表路径结构：
        //   HKLM\SYSTEM\CurrentControlSet\Enum\USB\
        //     VID_xxxx&PID_xxxx\      ← vid_pid（设备型号）
        //       <实例ID>\             ← instance（同型号多设备时有多个）
        //         Device Parameters\ ← params（存放 PortName、DisableModemOutHandShake 等）
        for vid_pid in enum_usb.enum_keys().flatten() {
            let vid_pid_key = match enum_usb.open_subkey(&vid_pid) {
                Ok(k) => k,
                Err(_) => continue,
            };
            for instance in vid_pid_key.enum_keys().flatten() {
                let instance_key = match vid_pid_key.open_subkey(&instance) {
                    Ok(k) => k,
                    Err(_) => continue,
                };

                // KEY_WRITE：需要写入 DisableModemOutHandShake；无写权限时跳过（非目标设备或权限不足）
                let params = instance_key.open_subkey_with_flags(
                    "Device Parameters",
                    KEY_READ | KEY_WRITE,
                );
                let Ok(params) = params else { continue };

                // 检查此设备实例对应的端口名是否匹配
                let Ok(name): Result<String, _> = params.get_value("PortName") else {
                    continue;
                };
                if !name.eq_ignore_ascii_case(port_name) {
                    continue;
                }

                // 检查 DisableModemOutHandShake（CH340 驱动 ≥ 3.9.2024.9 支持）
                let current: u32 = params
                    .get_value("DisableModemOutHandShake")
                    .unwrap_or(0u32);

                if current == 1 {
                    return Ok(()); // 已配置，直接开串口即可
                }

                // 首次配置：写入 1，需要重新插拔
                params.set_value("DisableModemOutHandShake", &1u32).ok();
                return Err(NeedReconnect(port_name.to_string()));
            }
        }

        // 未找到注册表项（旧版驱动 / 非 CH340），静默跳过，正常开串口
        Ok(())
    }

    pub fn post_open(_port: &mut dyn serialport::SerialPort) {
        // Windows 驱动侧已处理，无需软件后处理
    }
}

// ── 非 Windows 实现（Linux / macOS）──────────────────────────────────────────
//
// 根本问题：Linux tty 驱动在 open() 内部会拉高 DTR/RTS，此时 Rust 代码尚未执行，
// 无法拦截。CH340/CP2102/FT232 等驱动的初始化代码（如 ch341_set_handshake）
// 仅在 open_count==0 时执行——即设备从"未打开"变为"第一次打开"时。
//
// 解决方案（pre_open）：
//   1. 先用我们自己的 fd（guard_fd）打开设备：驱动初始化 → DTR 被拉高
//   2. guard_fd 立即通过 termios 设置 CLOCAL（忽略 modem 线）、清除 HUPCL
//   3. guard_fd 通过 TIOCMBIC ioctl 将 DTR/RTS 拉低
//   4. 保持 guard_fd 打开，让 serialport 打开第二个 fd：
//      此时 open_count 从 1 变为 2，驱动跳过初始化，DTR 保持低电平
//   5. serialport 打开完毕后关闭 guard_fd（open_count 回到 1）
//   6. post_open 再做一次保底清零

#[cfg(not(windows))]
mod platform {
    use super::NeedReconnect;
    use std::fs::OpenOptions;
    use std::os::unix::{fs::OpenOptionsExt, io::AsRawFd};

    pub fn prepare(_port_name: &str) -> Result<(), NeedReconnect> {
        Ok(())
    }

    /// 预打开串口，配置 termios 并清除 DTR/RTS，返回必须持续持有的守护文件。
    /// 调用方须在 serialport::open() 完成后再 drop 返回值，
    /// 以确保 open_count 不归零（从而阻止驱动在 serialport 的 open 调用中重置 DTR）。
    /// 失败时返回 None 并静默降级为旧的 post_open 方案。
    pub fn pre_open(port_name: &str) -> Option<std::fs::File> {
        // O_NOCTTY：不让此 fd 成为进程的控制终端
        // O_NONBLOCK：不阻塞等待 DCD（载波检测），适合大多数 USB 转串口设备
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOCTTY | libc::O_NONBLOCK)
            .open(port_name)
            .ok()?;

        let fd = file.as_raw_fd();

        unsafe {
            // 读取当前 termios
            let mut tios: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(fd, &mut tios) != 0 {
                return None;
            }

            // CLOCAL：让驱动忽略 modem 控制线（DCD/DSR/RI），
            //         open() 不再等待载波，信号变化不再产生 SIGHUP
            tios.c_cflag |= libc::CLOCAL;
            // 清除 HUPCL：最后一个 fd 关闭时不自动拉低 DTR/RTS，
            //             确保 guard_fd close 后 DTR 保持 serialport 设定的状态
            tios.c_cflag &= !libc::HUPCL;

            if libc::tcsetattr(fd, libc::TCSANOW, &tios) != 0 {
                return None;
            }

            // TIOCMBIC：按位清除指定的 modem 控制信号
            // 此 ioctl 在 CH340/CP210x/FT232 驱动上均有效；失败时静默忽略
            let bits: libc::c_int = (libc::TIOCM_DTR | libc::TIOCM_RTS) as libc::c_int;
            libc::ioctl(fd, libc::TIOCMBIC, &bits);
        }

        Some(file)
    }

    /// 保底：serialport 打开后再清一次 DTR/RTS（应对 pre_open 失败或驱动行为不一致的情况）
    pub fn post_open(port: &mut dyn serialport::SerialPort) {
        port.write_data_terminal_ready(false).ok();
        port.write_request_to_send(false).ok();
    }
}
