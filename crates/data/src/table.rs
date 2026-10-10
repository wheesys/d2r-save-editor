//! 一张数据表（`global/excel/*.txt`）：列名 + 行。
//!
//! 游戏表是制表符分隔的文本：第一行列名，其余行数据；字段可能为空（表示 0 或默认）。
//! 本类型刻意保持"无模式"——具体含义（哪一列是 invwidth、哪一列是 Save Bits）
//! 由上层按需读取，避免把游戏常量固化进代码。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// 一张已解析的表。
#[derive(Debug, Clone)]
pub struct Table {
    /// 表名（通常等于文件名去掉扩展名）。
    pub name: String,
    /// 来源文件。
    pub source: PathBuf,
    /// 列名（保持文件顺序）。
    pub columns: Vec<String>,
    /// 数据行（每行长度与列数一致，缺失补空串）。
    pub rows: Vec<Vec<String>>,
    /// 列名 → 下标，便于按名取列。
    index: HashMap<String, usize>,
}

impl Table {
    /// 从制表符分隔的文本解析。
    pub fn parse_tsv(name: impl Into<String>, source: impl Into<PathBuf>, text: &str) -> Self {
        let mut lines = text.lines().filter(|l| !l.trim().is_empty());
        let columns: Vec<String> = lines
            .next()
            .map(|h| h.split('\t').map(|c| c.trim().to_string()).collect())
            .unwrap_or_default();
        let mut rows = Vec::new();
        for line in lines {
            let mut cells: Vec<String> = line.split('\t').map(|c| c.trim().to_string()).collect();
            cells.resize(columns.len(), String::new());
            rows.push(cells);
        }
        let index = columns
            .iter()
            .enumerate()
            .map(|(i, c)| (c.to_ascii_lowercase(), i))
            .collect();
        Self {
            name: name.into(),
            source: source.into(),
            columns,
            rows,
            index,
        }
    }

    /// 从文件读取（按 UTF-8 解析，失败回退 GBK 常见兼容无法处理时给原始错误）。
    pub fn from_file(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path)?;
        let text = decode_text(&bytes);
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        Ok(Self::parse_tsv(name, path, &text))
    }

    /// 行数。
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// 是否为空表。
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// 列下标（大小写不敏感）。
    pub fn column_index(&self, column: &str) -> Option<usize> {
        self.index.get(&column.to_ascii_lowercase()).copied()
    }

    /// 取某行某列。
    pub fn cell(&self, row: usize, column: &str) -> Option<&str> {
        let c = self.column_index(column)?;
        self.rows.get(row)?.get(c).map(|s| s.as_str())
    }

    /// 取某行某列并解析为整数（空串按 0，游戏表惯例）。
    pub fn int(&self, row: usize, column: &str) -> Option<i64> {
        let raw = self.cell(row, column)?.trim();
        if raw.is_empty() {
            return Some(0);
        }
        raw.parse::<i64>()
            .ok()
            .or_else(|| raw.parse::<f64>().ok().map(|f| f as i64))
    }

    /// 某列的全部取值（保持行序）。
    pub fn column_values(&self, column: &str) -> Vec<&str> {
        let Some(c) = self.column_index(column) else {
            return Vec::new();
        };
        self.rows
            .iter()
            .filter_map(|r| r.get(c).map(|s| s.as_str()))
            .collect()
    }

    /// 按行号取某列（空串返回 None，便于"取有效值"）。
    pub fn cell_nonempty(&self, row: usize, column: &str) -> Option<&str> {
        self.cell(row, column)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
    }

    /// 取某列的整数（空串按 0）。
    pub fn int_at(&self, row: usize, column: &str) -> Option<i64> {
        self.int(row, column)
    }

    /// 在整列里查找首个等于 `value` 的行号（大小写不敏感，两边都 trim）。
    pub fn find_row_by(&self, column: &str, value: &str) -> Option<usize> {
        let c = self.column_index(column)?;
        let needle = value.trim().to_ascii_lowercase();
        self.rows.iter().position(|r| {
            r.get(c)
                .map(|v| v.trim().to_ascii_lowercase() == needle)
                .unwrap_or(false)
        })
    }
}

/// 文本解码：优先 UTF-8，带 BOM 时剥掉；失败则按 GBK 近似（仅用于容错）。
fn decode_text(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|b| *b as char).collect(),
    }
}

/// 数据集错误。
#[derive(Debug)]
pub enum DataError {
    /// 文件读取失败。
    Io(PathBuf, std::io::Error),
    /// JSON 解析失败。
    Json(PathBuf, serde_json::Error),
    /// 期望的目录不存在。
    Missing(PathBuf),
}

impl std::fmt::Display for DataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(p, e) => write!(f, "读取失败 {}：{e}", p.display()),
            Self::Json(p, e) => write!(f, "JSON 解析失败 {}：{e}", p.display()),
            Self::Missing(p) => write!(f, "目录不存在：{}", p.display()),
        }
    }
}

impl std::error::Error for DataError {}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "index\tinvwidth\tinvheight\tName\n0\t1\t1\tbox\n1\t2\t3\t\n";

    #[test]
    fn parses_tsv_with_empty_cells() {
        let t = Table::parse_tsv("inventory", "inventory.txt", SAMPLE);
        assert_eq!(t.columns, vec!["index", "invwidth", "invheight", "Name"]);
        assert_eq!(t.len(), 2);
        assert_eq!(t.cell(0, "invwidth"), Some("1"));
        assert_eq!(t.cell(1, "invheight"), Some("3"));
        // 空串按游戏表惯例当 0
        assert_eq!(t.int(1, "Name"), Some(0));
        assert_eq!(t.int(1, "invwidth"), Some(2));
        assert_eq!(t.column_values("Name"), vec!["box", ""]);
    }

    #[test]
    fn column_lookup_is_case_insensitive() {
        let t = Table::parse_tsv("x", "x.txt", "Save Bits\tSave Add\n9\t0\n");
        assert_eq!(t.int(0, "save bits"), Some(9));
        assert_eq!(t.int(0, "SAVE ADD"), Some(0));
        assert_eq!(t.column_index("nope"), None);
    }

    #[test]
    fn ragged_rows_are_padded() {
        let t = Table::parse_tsv("x", "x.txt", "a\tb\tc\n1\t2\n");
        assert_eq!(t.rows[0].len(), 3);
        assert_eq!(t.cell(0, "c"), Some(""));
    }
}
