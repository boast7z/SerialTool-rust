/// 申请串口访问权限的平台入口。
///
/// Windows：通过 PowerShell `Start-Process -Verb RunAs` 触发 UAC，
///           以管理员身份重新启动程序；批准后当前（低权限）进程自动退出。
///
/// Linux  ：通过 pkexec 将 udev 规则写入 /etc/udev/rules.d/，
///           让目标设备对所有用户可读写；规则通过 `udevadm trigger` 立即生效，
///           无需注销重新登录。
///
/// 函数立即返回一条提示字符串供显示在接收区，实际提权操作在后台进行。
pub fn elevate(port_name: &str) -> String {
    platform::elevate(port_name)
}

// ── Windows ───────────────────────────────────────────────────────────────────
#[cfg(windows)]
mod platform {
    pub fn elevate(_port_name: &str) -> String {
        let exe = match std::env::current_exe() {
            Ok(p) => p,
            Err(e) => return format!("[错误] 获取程序路径失败: {}", e),
        };

        // Start-Process -Verb RunAs 触发 UAC 对话框。
        // 使用 spawn()（非阻塞），让 UAC 弹出后当前进程仍可响应渲染；
        // 随后在后台线程稍作等待再 exit(0)，让 PowerShell 有时间弹出 UAC。
        match std::process::Command::new("powershell")
            .args([
                "-WindowStyle",
                "Hidden",
                "-NonInteractive",
                "-Command",
                &format!("Start-Process '{}' -Verb RunAs", exe.to_string_lossy()),
            ])
            .spawn()
        {
            Ok(_) => {
                std::thread::spawn(|| {
                    std::thread::sleep(std::time::Duration::from_millis(800));
                    std::process::exit(0);
                });
                "[提权] UAC 对话框已弹出，批准后将以管理员身份重新启动。".to_string()
            }
            Err(e) => format!("[错误] 无法启动提权进程: {}", e),
        }
    }
}

// ── Linux / macOS ─────────────────────────────────────────────────────────────
#[cfg(not(windows))]
mod platform {
    /// 阻塞执行（应在 spawn_blocking 中调用）：
    /// 1. 写入永久 udev 规则
    /// 2. chmod 立即修复已连接设备的权限节点
    /// 3. reload + trigger 确保后续插入的设备也生效
    pub fn elevate(port_name: &str) -> String {
        let rule = concat!(
            "# 由串口调试助手自动写入，可安全删除\n",
            "KERNEL==\"ttyUSB[0-9]*\",      MODE=\"0666\"\n",
            "KERNEL==\"ttyACM[0-9]*\",      MODE=\"0666\"\n",
            "KERNEL==\"ttyS[0-9]*\",        MODE=\"0666\"\n",
            "KERNEL==\"ttyCH341USB[0-9]*\", MODE=\"0666\"\n",  // WCH 官方驱动节点
        );
        let tmp  = "/tmp/50-serial-debugger.rules";
        let dest = "/etc/udev/rules.d/50-serial-debugger.rules";

        if let Err(e) = std::fs::write(tmp, rule) {
            return format!("[错误] 写入临时规则文件失败: {}", e);
        }

        // chmod 先于 udevadm trigger，确保已连接设备立即可用；
        // 2>/dev/null 忽略无匹配设备时的报错
        let cmd = format!(
            "mv {tmp} {dest} \
             && chmod o+rw /dev/ttyUSB* /dev/ttyACM* /dev/ttyS* /dev/ttyCH341USB* 2>/dev/null; \
             udevadm control --reload-rules \
             && udevadm trigger --action=change --subsystem-match=tty",
            tmp = tmp,
            dest = dest,
        );

        match std::process::Command::new("pkexec")
            .args(["sh", "-c", &cmd])
            .output()  // 阻塞等待完成，不再 fire-and-forget
        {
            Ok(o) if o.status.success() => format!(
                "[提权] 成功：设备 {} 的访问权限已更新，规则文件已写入 {}。\n\
                 现在可以直接点击「打开串口」。",
                port_name, dest
            ),
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr).trim().to_string();
                format!(
                    "[提权] pkexec 执行失败（可能已取消）。\n\
                     如需手动修复：sudo sh -c '{}'\n{}",
                    cmd,
                    if stderr.is_empty() { String::new() } else { format!("错误详情: {}", stderr) }
                )
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => format!(
                "[提权] 未找到 pkexec，请手动执行：\n  sudo sh -c '{}'",
                cmd
            ),
            Err(e) => format!("[错误] 启动 pkexec 失败: {}", e),
        }
    }
}
