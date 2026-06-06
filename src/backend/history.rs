use crate::types::HistoryItem;
use chrono::Local;
use std::{fs, path::PathBuf};

/// 最多保留 50 条历史：够覆盖一次调试会话的常用命令，同时控制 JSON 文件大小
const MAX_ITEMS: usize = 50;

/// 发送历史记录，持久化到 `~/.config/iced_serialtool/history.json`。
/// 始终按时间倒序存储（index 0 = 最新），方便键盘导航（Up 键直接取 [0]）。
pub struct History {
    items: Vec<HistoryItem>,
}

impl History {
    pub fn new() -> Self {
        let mut h = Self { items: Vec::new() };
        h.load();
        h
    }

    /// 添加一条记录到列表头部，超出上限时丢弃最旧的记录，并立即持久化
    pub fn add(&mut self, text: String, is_hex: bool) {
        if text.trim().is_empty() {
            return;
        }
        let item = HistoryItem {
            text,
            is_hex,
            time: Local::now().format("%H:%M:%S").to_string(),
        };
        self.items.insert(0, item);
        if self.items.len() > MAX_ITEMS {
            self.items.pop();
        }
        self.save();
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.save();
    }

    pub fn items(&self) -> &[HistoryItem] {
        &self.items
    }

    fn config_path() -> PathBuf {
        let mut p = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        p.push("iced_serialtool");
        fs::create_dir_all(&p).ok();
        p.push("history.json");
        p
    }

    fn load(&mut self) {
        let data = fs::read_to_string(Self::config_path()).unwrap_or_default();
        // 解析失败（文件损坏、格式不兼容）时回退为空列表，不崩溃
        self.items = serde_json::from_str(&data).unwrap_or_default();
        // 兼容旧版本可能写入超过 MAX_ITEMS 条记录的文件
        self.items.truncate(MAX_ITEMS);
    }

    fn save(&self) {
        let json = serde_json::to_string_pretty(&self.items).unwrap_or_default();
        fs::write(Self::config_path(), json).ok();
    }
}
