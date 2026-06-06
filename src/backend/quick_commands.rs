use crate::types::QuickCommand;
use std::fs;
use std::path::PathBuf;

/// 固定 20 个槽位（比历史记录少，因为快捷命令是主动管理的，20 个已足够覆盖常用场景）
const MAX_ITEMS: usize = 20;

/// 快捷命令集合，持久化到 `~/.config/iced_serialtool/quick_commands.json`。
/// 始终维护 MAX_ITEMS 个槽位（固定长度），空槽 text 为空字符串。
/// 固定槽位设计使 UI 位置稳定，不因增删而跳动，用户可建立"位置 = 功能"的肌肉记忆。
pub struct QuickCommands {
    items: Vec<QuickCommand>,
}

impl QuickCommands {
    pub fn new() -> Self {
        let mut cmds = Self {
            items: Vec::with_capacity(MAX_ITEMS),
        };
        cmds.load();
        cmds
    }

    pub fn items(&self) -> &[QuickCommand] {
        &self.items
    }

    /// 返回指定槽位的命令引用；空槽（text 为空）返回 None，防止发送空内容
    pub fn send_at(&self, index: usize) -> Option<&QuickCommand> {
        self.items.get(index).filter(|c| !c.text.trim().is_empty())
    }

    pub fn edit(&mut self, index: usize, name: String, text: String, is_hex: bool) {
        if let Some(item) = self.items.get_mut(index) {
            item.name = name;
            item.text = text;
            item.is_hex = is_hex;
            self.save();
        }
    }

    pub fn add_to_first_empty(&mut self, text: &str, is_hex: bool) -> Option<usize> {
        for (i, item) in self.items.iter_mut().enumerate() {
            if item.text.trim().is_empty() {
                item.text = text.to_string();
                item.is_hex = is_hex;
                self.save();
                return Some(i);
            }
        }
        None
    }

    /// "删除"指定槽位：将其重置为默认空槽（逻辑删除，不缩减数组）。
    /// 不物理移除是为了保持固定槽位设计——其他命令的位置不因删除而改变。
    pub fn delete(&mut self, index: usize) {
        if let Some(item) = self.items.get_mut(index) {
            item.name = format!("命令 {}", index + 1);
            item.text.clear();
            item.is_hex = false;
            self.save();
        }
    }

    pub fn clear_all(&mut self) {
        for (i, item) in self.items.iter_mut().enumerate() {
            item.name = format!("命令 {}", i + 1);
            item.text.clear();
            item.is_hex = false;
        }
        self.save();
    }

    fn config_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("iced_serialtool");
        fs::create_dir_all(&path).ok();
        path.push("quick_commands.json");
        path
    }

    fn load(&mut self) {
        let path = Self::config_path();
        let data = fs::read_to_string(&path).unwrap_or_default();

        let loaded: Vec<QuickCommand> = serde_json::from_str(&data).unwrap_or_default();

        // 始终用 MAX_ITEMS 个槽位填充：文件中有的复用，不足的补充默认空槽。
        // 这样即使文件损坏或槽位数量变更，内存中始终有完整的 20 个槽位。
        self.items.clear();
        self.items.reserve(MAX_ITEMS);
        for i in 0..MAX_ITEMS {
            if let Some(item) = loaded.get(i) {
                self.items.push(item.clone());
            } else {
                self.items.push(QuickCommand {
                    name: format!("命令 {}", i + 1),
                    text: String::new(),
                    is_hex: false,
                });
            }
        }
    }

    fn save(&self) {
        let path = Self::config_path();
        let json = serde_json::to_string_pretty(&self.items).unwrap_or_default();
        fs::write(&path, json).ok();
    }
}
