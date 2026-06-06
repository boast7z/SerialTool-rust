use crate::types::{SendMode, TimestampFormat};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

/// 运行时应用设置，持久化到 `~/.config/iced_serialtool/settings.json`。
pub struct AppSettings {
    /// 用户自定义的日志保存目录（仅 log_dir_custom=true 时生效）
    pub log_dir: PathBuf,
    /// false = 保存到 exe 所在目录；true = 使用 log_dir
    pub log_dir_custom: bool,
    /// 接收区字号（像素），范围 [8, 28]，Ctrl+滚轮调整
    pub font_size: f32,
    /// 左面板宽度（像素），拖拽后自动保存
    pub left_width: f32,
    /// 右面板宽度（像素），拖拽后自动保存
    pub right_width: f32,
    /// 上次成功连接的端口名，启动时自动回填到下拉框
    pub last_port: Option<String>,
    /// 上次成功连接的波特率，启动时自动回填
    pub last_baud: String,
    pub send_mode: SendMode,
    pub timestamp_format: TimestampFormat,
}

/// JSON 序列化/反序列化专用的镜像结构体，与 AppSettings 字段一一对应。
///
/// 为什么不直接在 AppSettings 上派生 Serialize/Deserialize？
/// serde 的 `#[serde(default = "fn")]` 要求字段类型实现 Default 或提供具名函数，
/// 而 AppSettings 持有 iced 的 combo_box::State 等不可序列化类型。
/// 用独立的 Persisted 结构体隔离"运行时状态"与"可持久化数据"，两者各自清晰。
#[derive(Serialize, Deserialize)]
struct Persisted {
    log_dir: String,
    #[serde(default)]
    log_dir_custom: bool,
    #[serde(default = "default_font_size")]
    font_size: f32,
    #[serde(default = "default_left_width")]
    left_width: f32,
    #[serde(default = "default_right_width")]
    right_width: f32,
    #[serde(default)]
    last_port: Option<String>,
    #[serde(default = "default_baud")]
    last_baud: String,
    #[serde(default)]
    send_mode: SendMode,
    #[serde(default)]
    timestamp_format: TimestampFormat,
}

// serde 的 default 属性要求具名函数，不能直接用字面量
fn default_font_size()   -> f32    { 13.0 }    // 13px：在 1080p 屏幕上清晰且不占用过多空间
fn default_left_width()  -> f32    { 220.0 }   // 220px：足以显示端口配置所有标签和控件
fn default_right_width() -> f32    { 150.0 }   // 150px：快捷命令按钮的最小舒适宽度
fn default_baud()        -> String { "115200".to_string() } // 最常见的现代串口波特率

impl AppSettings {
    pub fn new() -> Self {
        let log_dir = dirs::document_dir()
            .or_else(|| dirs::home_dir())
            .unwrap_or_else(|| PathBuf::from("."));
        let mut s = Self {
            log_dir,
            log_dir_custom: false,
            font_size: default_font_size(),
            left_width: default_left_width(),
            right_width: default_right_width(),
            last_port: None,
            last_baud: default_baud(),
            send_mode: SendMode::default(),
            timestamp_format: TimestampFormat::default(),
        };
        s.load();
        s
    }

    /// 将当前设置序列化为 JSON 并写入配置文件；写入失败时静默忽略（不影响正常使用）
    pub fn save(&self) {
        let path = Self::config_path();
        let p = Persisted {
            log_dir:        self.log_dir.to_string_lossy().into_owned(),
            log_dir_custom: self.log_dir_custom,
            font_size:      self.font_size,
            left_width:     self.left_width,
            right_width:    self.right_width,
            last_port:      self.last_port.clone(),
            last_baud:        self.last_baud.clone(),
            send_mode:        self.send_mode,
            timestamp_format: self.timestamp_format,
        };
        if let Ok(json) = serde_json::to_string_pretty(&p) {
            fs::write(path, json).ok();
        }
    }

    /// 检查自定义日志目录是否存在（用于在设置面板显示目录不存在警告）
    pub fn log_dir_valid(&self) -> bool {
        self.log_dir.is_dir()
    }

    /// 实际使用的日志保存目录：
    /// - log_dir_custom == false：exe 所在目录（安装版即安装目录），开发时为当前目录
    /// - log_dir_custom == true：用户自定义的 log_dir
    pub fn effective_log_dir(&self) -> PathBuf {
        if self.log_dir_custom {
            self.log_dir.clone()
        } else {
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                .unwrap_or_else(|| PathBuf::from("."))
        }
    }

    /// 从配置文件读取并覆盖当前设置；文件不存在或解析失败时保留默认值（静默忽略）
    fn load(&mut self) {
        if let Ok(data) = fs::read_to_string(Self::config_path()) {
            if let Ok(p) = serde_json::from_str::<Persisted>(&data) {
                self.log_dir        = PathBuf::from(p.log_dir);
                self.log_dir_custom = p.log_dir_custom;
                self.font_size  = p.font_size;
                self.left_width  = p.left_width;
                self.right_width = p.right_width;
                self.last_port  = p.last_port;
                self.last_baud        = p.last_baud;
                self.send_mode        = p.send_mode;
                self.timestamp_format = p.timestamp_format;
            }
        }
    }

    pub fn config_path() -> PathBuf {
        let mut p = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        p.push("iced_serialtool");
        fs::create_dir_all(&p).ok();
        p.push("settings.json");
        p
    }

    pub fn reset_to_defaults(&mut self) {
        self.font_size      = default_font_size();
        self.log_dir_custom = false;
        self.log_dir        = dirs::document_dir()
            .or_else(|| dirs::home_dir())
            .unwrap_or_else(|| PathBuf::from("."));
        self.send_mode        = crate::types::SendMode::default();
        self.timestamp_format = crate::types::TimestampFormat::default();
        self.left_width  = default_left_width();
        self.right_width = default_right_width();
        // last_port / last_baud 不重置：复位设置通常是调整 UI 偏好，不应让用户重新选设备
        self.save();
    }
}
