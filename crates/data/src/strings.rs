//! 字符串表（`data/local/lng/strings/*.json`）。
//!
//! D2R 的字符串表是 JSON 数组，每个条目形如：
//!
//! ```json
//! [
//!   { "id": 0, "Key": "BnetConnect", "enUS": "...", "zhCN": "...", "zhTW": "..." }
//! ]
//! ```
//!
//! 键名大小写在不同版本/工具里可能变化（`Key`/`key`/`szKey`），这里全部容忍。

use std::collections::HashMap;
use std::path::Path;

/// 多语言字符串表：`键 → { 语言 → 文本 }`。
///
/// 键同时登记 `Key` 与 `id` 两种形式，方便按名字或按数字 ID 查。
#[derive(Debug, Default, Clone)]
pub struct Strings {
    /// 键（小写） → 语言（小写） → 文本。
    entries: HashMap<String, HashMap<String, String>>,
    /// 参与合并的语言集合（小写）。
    languages: Vec<String>,
}

/// 首选显示语言（本地 `bnet_emu.ini` 为 `Locale=zhCN`），找不到时按 [`FALLBACK_ORDER`] 回退。
pub const PREFERRED: &[&str] = &["zhcn", "zhtw", "enus"];

impl Strings {
    /// 条目数。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 已收录的语言（原始大小写按首次出现记录，小写）。
    pub fn languages(&self) -> &[String] {
        &self.languages
    }

    /// 合并另一份字符串表（同键同语言以后者覆盖）。
    pub fn merge(&mut self, other: Strings) {
        for lang in other.languages {
            if !self.languages.contains(&lang) {
                self.languages.push(lang);
            }
        }
        for (key, langs) in other.entries {
            self.entries.entry(key).or_default().extend(langs);
        }
    }

    /// 取指定语言的文本。
    pub fn get(&self, key: &str, lang: &str) -> Option<&str> {
        self.entries
            .get(&key.to_ascii_lowercase())?
            .get(&lang.to_ascii_lowercase())
            .map(|s| s.as_str())
            .filter(|s| !s.is_empty())
    }

    /// 按首选语言顺序取文本（zhCN → zhTW → enUS → 任意非空）。
    pub fn text(&self, key: &str) -> Option<&str> {
        for lang in PREFERRED {
            if let Some(v) = self.get(key, lang) {
                return Some(v);
            }
        }
        self.entries
            .get(&key.to_ascii_lowercase())?
            .values()
            .find(|s| !s.is_empty())
            .map(|s| s.as_str())
    }

    /// 从目录读取全部 `*.json`。
    pub fn from_dir(dir: impl AsRef<Path>) -> std::io::Result<Self> {
        let dir = dir.as_ref();
        let mut out = Strings::default();
        let mut files: Vec<_> = std::fs::read_dir(dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map(|e| e.eq_ignore_ascii_case("json")).unwrap_or(false))
            .collect();
        files.sort();
        for f in files {
            let bytes = std::fs::read(&f)?;
            out.merge(Self::from_json_bytes(&bytes));
        }
        Ok(out)
    }

    /// 解析一份字符串表 JSON。
    ///
    /// 容错点（实测导出的文本表文件都带 UTF-8 BOM）：
    /// - 剥掉 UTF-8 BOM；
    /// - 顶层是数组，或 `{ "...": [...] }` 之类的包装对象；
    /// - 无法识别时返回空表（缺字符串表不应阻断整个装载流程，但调用方可用
    ///   [`Strings::is_empty`] 判断，CLI/界面必须把"空表"当作需要提示的状态）。
    pub fn from_json_bytes(bytes: &[u8]) -> Self {
        let mut me = Strings::default();
        let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) else { return me };
        let array = match &value {
            serde_json::Value::Array(a) => a.clone(),
            // 也容忍 { "strings": [...] } 之类的包装
            serde_json::Value::Object(o) => match o.values().find(|v| v.is_array()) {
                Some(serde_json::Value::Array(a)) => a.clone(),
                _ => return me,
            },
            _ => return me,
        };
        for item in array {
            let serde_json::Value::Object(obj) = item else { continue };
            let mut key: Option<String> = None;
            let mut id: Option<String> = None;
            let mut langs: HashMap<String, String> = HashMap::new();
            for (k, v) in obj {
                let lower = k.to_ascii_lowercase();
                let Some(text) = v.as_str() else {
                    if lower == "id" {
                        if let Some(n) = v.as_i64() {
                            id = Some(n.to_string());
                        }
                    }
                    continue;
                };
                match lower.as_str() {
                    "key" | "szkey" | "name" => key = Some(text.to_string()),
                    "id" => id = Some(text.to_string()),
                    _ => {
                        let lang = lower.clone();
                        if !me.languages.contains(&lang) {
                            me.languages.push(lang.clone());
                        }
                        langs.insert(lang, text.to_string());
                    }
                }
            }
            if let Some(k) = key {
                me.entries.entry(k.to_ascii_lowercase()).or_default().extend(langs.clone());
            }
            if let Some(i) = id {
                me.entries.entry(i.to_ascii_lowercase()).or_default().extend(langs);
            }
        }
        me
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON: &[u8] = br#"[
      { "id": 2825, "Key": "ItemStats1", "enUS": "Required Strength", "zhCN": "\u9700\u6c42\u529b\u91cf", "zhTW": "\u9700\u6c42\u529b\u91cf" },
      { "id": 2826, "Key": "ItemStats2", "enUS": "", "zhCN": "" }
    ]"#;

    #[test]
    fn parses_by_key_and_id_with_language_fallback() {
        let s = Strings::from_json_bytes(JSON);
        assert_eq!(s.len(), 4, "两个条目各登记 Key 与 id");
        assert_eq!(s.get("ItemStats1", "zhCN"), Some("需求力量"));
        assert_eq!(s.get("itemstats1", "enUS"), Some("Required Strength"));
        // 按数字 id 也能查
        assert_eq!(s.get("2825", "zhCN"), Some("需求力量"));
        // 空串视为缺失 → 回退
        assert_eq!(s.text("ItemStats2"), None);
        assert_eq!(s.text("ItemStats1"), Some("需求力量"));
        assert_eq!(s.get("不存在", "zhCN"), None);
    }

    #[test]
    fn merge_prefers_later_values() {
        let mut a = Strings::from_json_bytes(JSON);
        let b = Strings::from_json_bytes(br#"[{ "Key": "ItemStats1", "zhCN": "\u65b0\u503c" }]"#);
        a.merge(b);
        assert_eq!(a.get("ItemStats1", "zhCN"), Some("新值"));
        // 未覆盖的语言保留
        assert_eq!(a.get("ItemStats1", "enUS"), Some("Required Strength"));
    }

    #[test]
    fn tolerates_garbage_and_wrapped_arrays() {
        assert!(Strings::from_json_bytes(b"not json").is_empty());
        let wrapped = Strings::from_json_bytes(br#"{ "strings": [ { "Key": "A", "enUS": "B" } ] }"#);
        assert_eq!(wrapped.get("A", "enUS"), Some("B"));
    }

    #[test]
    fn strips_utf8_bom() {
        // 导出的 JSON 都带 UTF-8 BOM，必须能解析
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(JSON);
        let s = Strings::from_json_bytes(&bytes);
        assert_eq!(s.get("ItemStats1", "zhCN"), Some("需求力量"));
    }
}
