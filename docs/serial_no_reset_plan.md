# 跨平台串口打开防复位方案（历史设计草稿）

> **注意**：本文档为早期设计阶段的记录，部分内容已被实际实现超越或修改。
> 当前实现细节请以 `src/backend/no_reset.rs` 源码和 `docs/API.md` 为准。
> 此文件仅作历史参考，不代表当前行为。

## 背景

STM32 开发板板载 CH340 USB 转串口芯片，DTR 引脚连接到 NRST，
上位机软件打开串口时会触发 STM32 意外复位。

---

## 根本原因

### Windows
`CreateFile()` 进入内核后，CH340 驱动向芯片发送 USB 控制传输拉高 DTR，
此行为发生在内核态，用户态代码无法在此之前介入。

### Linux
tty 层在 `open()` 期间调用 `dtr_rts()`，通过 ch341 内核驱动发送 USB 控制传输拉高 DTR。
若设置 `CLOCAL`，tty 层会跳过调制解调器控制线操作（**待 Fedora 实测验证**）。

---

## 解决方案

### Windows

**核心发现：** CH340 驱动 3.9.2024.9（2024/9/16，通过 Windows Update 推送）
在高级设置中新增了 **"禁用 Modem 流控"** 选项，
对应注册表键值：

```
HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Enum\USB\
  VID_1A86&PID_7523\<实例ID>\Device Parameters
    DisableModemOutHandShake = REG_DWORD
      0 = 未禁用（默认，打开串口会复位）
      1 = 禁用（打开串口不再操作 DTR/RTS）
```

**注意：修改后需重新插拔 USB 才能生效**（驱动在设备枚举时读取一次并缓存）。

**流程：**
```
检查 DisableModemOutHandShake
  ├─ 已经是 1 → 直接开串口 ✅
  └─ 是 0
        ↓
    写入 1（需要管理员权限）
        ↓
    弹窗提示用户重新插拔 USB
        ↓
    等待 COM 口重新枚举
        ↓
    正常开串口 ✅（之后永久生效，无需再次操作）
```

### Linux

**方案：** 通过 termios 在 `open()` 后尽快设置 `CLOCAL` + 清除 `HUPCL`，
并用 `TIOCMBIC` ioctl 强制清除 DTR/RTS。

```
open() 带 O_NOCTTY | O_NONBLOCK
    ↓
tcgetattr / 设置 CLOCAL | 清除 HUPCL / tcsetattr(TCSANOW)
    ↓
ioctl(TIOCMBIC, DTR | RTS)
    ↓
恢复阻塞模式（fcntl 清除 O_NONBLOCK）
    ↓
fd 直接移交 TTYPort（不经历第二次 open）
```

**待验证：** `O_NONBLOCK` + `CLOCAL` 是否足以抑制 tty 层的 DTR assert。
→ **Fedora 实测结论填在此处**

---

## 用户配置项

UI 上提供一个开关，默认关闭：

```
☐ 打开串口时禁用 Modem 流控
  （如果打开串口时设备意外重启，请勾选此项）
```

用户只需在遇到问题时手动开启，Windows 下首次开启后弹窗提示插拔一次，之后永久生效。

---

## Cargo.toml

```toml
[dependencies]
serialport = "4"

[target.'cfg(unix)'.dependencies]
libc = "0.2"

[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.59", features = [
    "Win32_Devices_Communication",
    "Win32_Storage_FileSystem",
    "Win32_Foundation",
]}
winreg = "0.52"
```

---

## 数据结构

```rust
/// 串口打开配置
#[derive(Debug, Clone)]
pub struct PortOpenConfig {
    pub baud: u32,
    pub timeout: std::time::Duration,
    /// 打开串口时禁用 Modem 流控（防止 STM32 等设备意外复位）
    /// 对应 UI："打开串口时禁用 Modem 流控"
    pub disable_modem_handshake: bool,
}

impl Default for PortOpenConfig {
    fn default() -> Self {
        Self {
            baud: 115200,
            timeout: std::time::Duration::from_millis(100),
            disable_modem_handshake: false,
        }
    }
}

/// 需要重新插拔 USB 的错误，UI 层弹窗提示
#[derive(Debug)]
pub struct NeedReconnectError(pub String);

impl std::fmt::Display for NeedReconnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "已完成配置，请重新插拔 USB 线后重试（端口：{}）", self.0)
    }
}
impl std::error::Error for NeedReconnectError {}
```

---

## 统一入口

```rust
pub fn open_no_reset(
    port_name: &str,
    config: &PortOpenConfig,
) -> Result<Box<dyn serialport::SerialPort>, Box<dyn std::error::Error>> {
    if config.disable_modem_handshake {
        platform::prepare(port_name)?;
    }
    platform::open(port_name, config)
}
```

---

## Windows 实现

```rust
#[cfg(windows)]
mod platform {
    use super::*;
    use serialport::COMPort;
    use std::ffi::OsStr;
    use std::os::windows::{ffi::OsStrExt, io::FromRawHandle};
    use winreg::enums::*;
    use winreg::RegKey;
    use windows_sys::Win32::Devices::Communication::*;
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::Storage::FileSystem::*;

    /// 检查并写入注册表，返回 true 表示需要重新插拔
    pub fn prepare(port_name: &str)
        -> Result<(), Box<dyn std::error::Error>>
    {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let enum_usb = hklm.open_subkey(
            "SYSTEM\\CurrentControlSet\\Enum\\USB"
        )?;

        for vid_pid in enum_usb.enum_keys().flatten() {
            let vid_pid_key = enum_usb.open_subkey(&vid_pid)?;
            for instance in vid_pid_key.enum_keys().flatten() {
                let instance_key = vid_pid_key.open_subkey(&instance)?;
                let params = instance_key.open_subkey_with_flags(
                    "Device Parameters",
                    KEY_READ | KEY_WRITE,
                );
                let Ok(params) = params else { continue };

                let port_val: Result<String, _> = params.get_value("PortName");
                let Ok(name) = port_val else { continue };
                if !name.eq_ignore_ascii_case(port_name) { continue }

                let current: u32 = params
                    .get_value("DisableModemOutHandShake")
                    .unwrap_or(0u32);

                if current == 1 {
                    return Ok(());  // 已配置，无需插拔
                }

                params.set_value("DisableModemOutHandShake", &1u32)?;
                return Err(Box::new(NeedReconnectError(port_name.to_string())));
            }
        }

        // 未找到注册表项（旧版驱动/非 CH340），静默跳过
        Ok(())
    }

    pub fn open(
        port_name: &str,
        config: &PortOpenConfig,
    ) -> Result<Box<dyn serialport::SerialPort>, Box<dyn std::error::Error>> {
        let path = if port_name.starts_with("\\\\.\\") {
            port_name.to_string()
        } else {
            format!("\\\\.\\{}", port_name)
        };
        let wide: Vec<u16> = OsStr::new(&path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                std::ptr::null_mut(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                0,
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error().into());
        }

        let result = unsafe {
            (|| -> Result<(), Box<dyn std::error::Error>> {
                let mut dcb: DCB = std::mem::zeroed();
                dcb.DCBlength = std::mem::size_of::<DCB>() as u32;
                if GetCommState(handle, &mut dcb) == 0 {
                    return Err(std::io::Error::last_os_error().into());
                }
                // fDtrControl bits[4:5] = 0 (DISABLE)
                // fRtsControl bits[12:13] = 0 (DISABLE)
                dcb._bitfield &= !(0b11 << 4);
                dcb._bitfield &= !(0b11 << 12);
                dcb.BaudRate = config.baud;
                if SetCommState(handle, &dcb) == 0 {
                    return Err(std::io::Error::last_os_error().into());
                }
                EscapeCommFunction(handle, CLRDTR);
                EscapeCommFunction(handle, CLRRTS);
                Ok(())
            })()
        };

        if let Err(e) = result {
            unsafe { CloseHandle(handle) };
            return Err(e);
        }

        let mut port = unsafe { COMPort::from_raw_handle(handle as *mut _) };
        port.set_timeout(config.timeout)?;
        Ok(Box::new(port))
    }
}
```

---

## Linux 实现

```rust
#[cfg(unix)]
mod platform {
    use super::*;
    use libc;
    use serialport::TTYPort;
    use std::ffi::CString;
    use std::os::unix::io::FromRawFd;

    /// Linux 无需预处理，直接返回
    pub fn prepare(_port_name: &str)
        -> Result<(), Box<dyn std::error::Error>>
    {
        Ok(())
    }

    pub fn open(
        port_name: &str,
        config: &PortOpenConfig,
    ) -> Result<Box<dyn serialport::SerialPort>, Box<dyn std::error::Error>> {
        let cpath = CString::new(port_name)?;

        let fd = unsafe {
            libc::open(
                cpath.as_ptr(),
                libc::O_RDWR | libc::O_NOCTTY | libc::O_NONBLOCK,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().into());
        }

        let result: Result<(), Box<dyn std::error::Error>> = unsafe {
            (|| {
                let mut termios: libc::termios = std::mem::zeroed();
                if libc::tcgetattr(fd, &mut termios) != 0 {
                    return Err(std::io::Error::last_os_error().into());
                }
                // CLOCAL : 忽略调制解调器控制线，tty 层不 assert DTR
                // !HUPCL : close 时不拉低 DTR
                termios.c_cflag |= libc::CLOCAL;
                termios.c_cflag &= !libc::HUPCL;
                libc::tcsetattr(fd, libc::TCSANOW, &termios);

                // 双保险：ioctl 强制清除 DTR + RTS
                let bits: libc::c_int = libc::TIOCM_DTR | libc::TIOCM_RTS;
                libc::ioctl(fd, libc::TIOCMBIC as _, &bits);

                // 恢复阻塞模式
                let flags = libc::fcntl(fd, libc::F_GETFL);
                libc::fcntl(fd, libc::F_SETFL, flags & !libc::O_NONBLOCK);

                Ok(())
            })()
        };

        if let Err(e) = result {
            unsafe { libc::close(fd) };
            return Err(e);
        }

        // fd 直接移交，不经历第二次 open()
        let mut port = unsafe { TTYPort::from_raw_fd(fd) };
        port.set_baud_rate(config.baud)?;
        port.set_timeout(config.timeout)?;
        Ok(Box::new(port))
    }
}
```

---

## UI 层调用示例

```rust
match open_no_reset("COM7", &config) {
    Ok(port) => {
        // 正常使用
    }
    Err(e) if e.downcast_ref::<NeedReconnectError>().is_some() => {
        // 弹窗提示
        show_dialog(
            "需要重新插拔设备",
            "已完成配置，请重新插拔 USB 线后点击重试。\n\
             此操作只需进行一次，之后将永久生效。"
        );
    }
    Err(e) => {
        show_error(&e.to_string());
    }
}
```

---

## 各平台效果汇总

| 平台 | 设备 | 方案 | 效果 | 备注 |
|------|------|------|------|------|
| Windows | CH340（新驱动） | 注册表 DisableModemOutHandShake=1 | ✅ 完全消除 | 首次需插拔一次 |
| Windows | 其他芯片/旧驱动 | DCB DTR_CONTROL_DISABLE + CLRDTR | ⚠️ 最佳努力 | 软件层无法根本解决 |
| Linux | CH340 | CLOCAL + HUPCL + TIOCMBIC | 🔲 待 Fedora 实测 | |
| Linux | 其他串口 | 同上 | 🔲 待实测 | |
| macOS | CH340 | 同 Linux | 🔲 待实测 | |

---

## 待确认事项

- [ ] Fedora 实测：Linux termios 方案是否能阻止 STM32 复位
- [ ] 如果 Linux 方案无效：考虑 rusb detach_kernel_driver 路径
- [ ] Windows 旧版驱动（无 DisableModemOutHandShake 键）的回退策略是否需要提示用户升级驱动
