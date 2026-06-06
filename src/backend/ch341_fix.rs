// Linux-only: detect and fix CH341 DTR-on-open issue by installing a patched
// WCH driver that exposes the no_dtr_on_open kernel module parameter.

use std::path::Path;

/// Returns true when the running ch341 module lacks the no_dtr_on_open
/// parameter (meaning DTR will be asserted on every port open, resetting
/// attached microcontrollers).
pub fn check_dtr_fix_needed() -> bool {
    let param = "/sys/module/ch341/parameters/no_dtr_on_open";
    if Path::new(param).exists() {
        // Module loaded: check whether the parameter is already enabled
        let val = std::fs::read_to_string(param).unwrap_or_default();
        return val.trim() != "Y";
    }
    // Module not loaded: check whether the permanent config was written
    let conf = std::fs::read_to_string("/etc/modprobe.d/ch341.conf")
        .unwrap_or_default();
    !conf.contains("no_dtr_on_open=1")
}

pub struct BuildEnvReport {
    /// Tools or headers that could not be found
    pub missing: Vec<String>,
    /// Human-readable install hint for the detected distro
    pub install_hint: String,
}

/// Checks that all build prerequisites are present: gcc, make, git,
/// and the kernel headers for the running kernel.
pub fn check_build_env() -> BuildEnvReport {
    let kernel = std::process::Command::new("uname")
        .arg("-r")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    let mut missing = Vec::new();

    for tool in ["gcc", "make", "git"] {
        if !has_cmd(tool) {
            missing.push(tool.to_string());
        }
    }
    if !Path::new(&format!("/lib/modules/{}/build", kernel)).exists() {
        missing.push("linux-headers".to_string());
    }

    let hint = build_hint(&missing, &kernel);
    BuildEnvReport { missing, install_hint: hint }
}

fn build_hint(missing: &[String], kernel: &str) -> String {
    if missing.is_empty() {
        return String::new();
    }
    let pkg = |name: &str| -> String {
        if name == "linux-headers" {
            if Path::new("/etc/arch-release").exists() {
                // Detect lts vs. mainline kernel
                if kernel.contains("lts") {
                    return "linux-lts-headers".into();
                }
                return "linux-headers".into();
            }
            if Path::new("/etc/debian_version").exists() {
                return format!("linux-headers-{}", kernel);
            }
            if Path::new("/etc/fedora-release").exists() {
                return "kernel-devel".into();
            }
        }
        name.to_string()
    };

    let pkgs: Vec<String> = missing.iter().map(|m| pkg(m)).collect();
    let joined = pkgs.join(" ");

    if Path::new("/etc/arch-release").exists() {
        format!("sudo pacman -S --needed {}", joined)
    } else if Path::new("/etc/debian_version").exists() {
        format!("sudo apt install {}", joined)
    } else if Path::new("/etc/fedora-release").exists() {
        format!("sudo dnf install {}", joined)
    } else {
        format!("请安装：{}", joined)
    }
}

fn has_cmd(name: &str) -> bool {
    std::process::Command::new(name)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub struct InstallResult {
    pub success: bool,
    pub message: String,
}

/// Extracts the bundled driver source and install.sh to temp locations,
/// then runs the script under pkexec. No network access required.
/// Designed to be called from tokio::task::spawn_blocking.
pub fn run_install() -> InstallResult {
    const SCRIPT: &str = include_str!("assets/install.sh");
    const CH341_C: &[u8]  = include_bytes!("assets/ch341.c");
    const CH341_H: &[u8]  = include_bytes!("assets/ch341.h");
    const MAKEFILE: &[u8] = include_bytes!("assets/Makefile");

    use std::io::Write;

    // Write script to temp file
    let mut script_tmp = match tempfile::Builder::new().suffix(".sh").tempfile() {
        Ok(f) => f,
        Err(e) => return InstallResult {
            success: false,
            message: format!("创建临时文件失败: {}", e),
        },
    };
    if let Err(e) = script_tmp.write_all(SCRIPT.as_bytes()) {
        return InstallResult { success: false, message: format!("写入脚本失败: {}", e) };
    }
    let script_path = script_tmp.path().to_owned();
    std::process::Command::new("chmod")
        .args(["+x", script_path.to_str().unwrap_or("")])
        .output().ok();

    // Write bundled driver source to a temp dir (world-readable so root can access)
    let src_dir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => return InstallResult { success: false, message: format!("创建临时目录失败: {}", e) },
    };
    let write_file = |name: &str, data: &[u8]| -> Result<(), String> {
        std::fs::write(src_dir.path().join(name), data)
            .map_err(|e| format!("写入 {} 失败: {}", name, e))
    };
    if let Err(e) = write_file("ch341.c", CH341_C)
        .and(write_file("ch341.h", CH341_H))
        .and(write_file("Makefile", MAKEFILE))
    {
        return InstallResult { success: false, message: e };
    }

    // Make source dir world-readable so pkexec (root) can access it
    std::process::Command::new("chmod")
        .args(["755", src_dir.path().to_str().unwrap_or("")])
        .output().ok();

    let output = std::process::Command::new("pkexec")
        .arg("bash")
        .arg(&script_path)
        .arg(src_dir.path())
        .output();

    // Keep both tmp handles alive until pkexec returns
    drop(script_tmp);
    drop(src_dir);

    match output {
        Ok(o) if String::from_utf8_lossy(&o.stdout).contains("SUCCESS") => {
            InstallResult {
                success: true,
                message: "CH341 驱动安装成功，重新插拔设备后生效".to_string(),
            }
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr).trim().to_string();
            InstallResult {
                success: false,
                message: if stderr.is_empty() {
                    "安装失败（用户取消或权限不足）".to_string()
                } else {
                    stderr
                },
            }
        }
        Err(e) => InstallResult {
            success: false,
            message: format!("pkexec 执行失败: {}", e),
        },
    }
}
