//! 存档文件的读取 / 写回，以及"往返一致性"（round-trip）校验。
//!
//! 设计要点（对应调研文档 §4.5）：
//! 1. **往返一致性是写路径的门槛**：读入后原样写出必须与原文件逐字节一致；
//! 2. 解析器只做"字节 ↔ 结构"，任何游戏常量（位宽、物品宽高、容器尺寸）
//!    都由 `d2r-data` 提供，不在本 crate 里写死；
//! 3. 失败不静默：解析/写出错误带上文件偏移或上下文，便于定位存档格式差异。

use std::fmt;
use std::path::{Path, PathBuf};

use crate::header::{SaveHeader, SaveVersion};

/// 存档类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveKind {
    /// 角色存档 `.d2s`。
    Character,
    /// 共享储物箱 `.d2i`。
    Stash,
}

impl SaveKind {
    /// 按扩展名判断。
    pub fn from_path(path: &Path) -> Option<Self> {
        match path.extension().and_then(|e| e.to_str())?.to_ascii_lowercase().as_str() {
            "d2s" => Some(Self::Character),
            "d2i" => Some(Self::Stash),
            _ => None,
        }
    }
}

/// 一个已加载的存档：原始字节 + 已解析结构 + 头信息。
pub struct SaveFile {
    /// 来源路径（从内存加载时可为空）。
    pub path: Option<PathBuf>,
    /// 存档类型。
    pub kind: SaveKind,
    /// 公共头。
    pub header: SaveHeader,
    /// 原始字节（用于往返比对与"未知尾随数据"透传）。
    pub original: Vec<u8>,
    /// 解析后的结构。
    pub save: d2s::D2Save,
    /// `.d2i` 结构化视图（仅共享箱有）。
    ///
    /// 共享箱目前用"字节保真"策略：物品段位流布局尚未逐字段验证，
    /// 所以写回时用这里保存的原始字节，保证逐字节一致。
    pub stash: Option<crate::d2i::SharedStash>,
}

impl fmt::Debug for SaveFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SaveFile")
            .field("path", &self.path)
            .field("kind", &self.kind)
            .field("header", &self.header)
            .field("len", &self.original.len())
            .field("stash", &self.stash.as_ref().map(|s| s.item_count))
            .finish()
    }
}

impl SaveFile {
    /// 从磁盘读取。
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SaveError> {
        let path = path.as_ref();
        let kind = SaveKind::from_path(path).ok_or_else(|| SaveError::UnknownKind(path.to_path_buf()))?;
        let bytes = std::fs::read(path).map_err(|e| SaveError::Io(path.to_path_buf(), e.to_string()))?;
        let mut me = Self::from_bytes(kind, bytes).map_err(|e| e.with_path(path))?;
        me.path = Some(path.to_path_buf());
        Ok(me)
    }

    /// 从内存读取（用于测试与从旁车/网络来源加载）。
    pub fn from_bytes(kind: SaveKind, bytes: Vec<u8>) -> Result<Self, SaveError> {
        let header = SaveHeader::parse_checked(&bytes).map_err(SaveError::Header)?;
        // `.d2i` 的段结构与 `.d2s` 不同（没有任务/传送点/属性/技能段），
        // 必须走共享箱解析器；d2s 解析库对它会报 "invalid quest section magic"。
        if kind == SaveKind::Stash {
            let stash = crate::d2i::SharedStash::parse(&bytes).map_err(|e| SaveError::Parse(e.to_string()))?;
            // `save` 字段仅为兼容既有接口；共享箱的真实数据在 `stash`，写回也走 `stash`。
            let save = d2s::D2Save::from_bytes(&bytes).unwrap_or_default();
            return Ok(Self { path: None, kind, header, original: bytes, save, stash: Some(stash) });
        }
        let save = d2s::D2Save::from_bytes(&bytes).map_err(|e| SaveError::Parse(e.to_string()))?;
        Ok(Self { path: None, kind, header, original: bytes, save, stash: None })
    }

    /// 存档版本。
    ///
    /// `.d2i` 的头部第 2 个 DWORD 是**结构版本**（样本 = 2），真正的存档版本在偏移 8，
    /// 所以共享箱取 [`crate::d2i::SharedStash::save_version`]。
    pub fn version(&self) -> SaveVersion {
        match &self.stash {
            Some(s) => SaveVersion::from_number(s.save_version),
            None => SaveVersion::from_number(self.header.version),
        }
    }

    /// 序列化回字节。
    ///
    /// - `.d2i`：用字节保真的共享箱包装（见 [`crate::d2i::SharedStash`]）；
    /// - `.d2s`：交给解析库序列化。
    pub fn to_bytes(&self) -> Result<Vec<u8>, SaveError> {
        if let Some(stash) = &self.stash {
            return Ok(stash.to_bytes());
        }
        self.save.to_bytes().map_err(|e| SaveError::Write(e.to_string()))
    }

    /// 原样写回的往返校验。
    ///
    /// 这是写路径的第一道门槛：任何一个存档做不到，就不允许发布写功能。
    pub fn check_roundtrip(&self) -> RoundTrip {
        match self.to_bytes() {
            Ok(rewritten) => RoundTrip::compare(&self.original, &rewritten),
            Err(e) => RoundTrip { identical: false, original_len: self.original.len(), rewritten_len: 0, first_diff: None, error: Some(e.to_string()) },
        }
    }
}

/// 往返比对结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundTrip {
    /// 是否逐字节一致。
    pub identical: bool,
    /// 原文件长度。
    pub original_len: usize,
    /// 写回后长度。
    pub rewritten_len: usize,
    /// 首个差异偏移。
    pub first_diff: Option<usize>,
    /// 写出失败时的错误。
    pub error: Option<String>,
}

impl RoundTrip {
    /// 比对两份字节。
    pub fn compare(original: &[u8], rewritten: &[u8]) -> Self {
        let first_diff = original.iter().zip(rewritten.iter()).position(|(a, b)| a != b);
        let identical = original == rewritten;
        Self {
            identical,
            original_len: original.len(),
            rewritten_len: rewritten.len(),
            first_diff: if identical { None } else { first_diff.or(Some(original.len().min(rewritten.len()))) },
            error: None,
        }
    }
}

impl fmt::Display for RoundTrip {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(e) = &self.error {
            return write!(f, "写出失败：{e}");
        }
        if self.identical {
            write!(f, "逐字节一致（{} 字节）", self.original_len)
        } else {
            write!(
                f,
                "不一致：原 {} 字节 / 新 {} 字节，首个差异偏移 {:?}",
                self.original_len, self.rewritten_len, self.first_diff
            )
        }
    }
}

/// 存档操作错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveError {
    /// 不认识的扩展名。
    UnknownKind(PathBuf),
    /// 文件读写失败。
    Io(PathBuf, String),
    /// 头部解析失败。
    Header(crate::header::HeaderError),
    /// 结构解析失败。
    Parse(String),
    /// 写出失败。
    Write(String),
}

impl SaveError {
    fn with_path(self, path: &Path) -> Self {
        match self {
            Self::Io(_, msg) => Self::Io(path.to_path_buf(), msg),
            other => other,
        }
    }
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownKind(p) => write!(f, "不是 .d2s/.d2i 文件：{}", p.display()),
            Self::Io(p, m) => write!(f, "读写失败 {}：{m}", p.display()),
            Self::Header(e) => write!(f, "头部错误：{e}"),
            Self::Parse(e) => write!(f, "解析失败：{e}"),
            Self::Write(e) => write!(f, "写出失败：{e}"),
        }
    }
}

impl std::error::Error for SaveError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::MAGIC;

    /// 造一个"头正确、但主体不是合法存档"的字节串，用于验证错误路径。
    fn fake(version: u32, body: &[u8]) -> Vec<u8> {
        let total = 16 + body.len();
        let mut v = Vec::new();
        v.extend_from_slice(&MAGIC.to_le_bytes());
        v.extend_from_slice(&version.to_le_bytes());
        v.extend_from_slice(&(total as u32).to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(body);
        v
    }

    #[test]
    fn kind_by_extension() {
        assert_eq!(SaveKind::from_path(Path::new("a.d2s")), Some(SaveKind::Character));
        assert_eq!(SaveKind::from_path(Path::new("A.D2I")), Some(SaveKind::Stash));
        assert_eq!(SaveKind::from_path(Path::new("a.txt")), None);
    }

    #[test]
    fn roundtrip_compare_reports_first_diff() {
        let r = RoundTrip::compare(b"abc", b"abc");
        assert!(r.identical);
        assert_eq!(r.first_diff, None);

        let r = RoundTrip::compare(b"abc", b"abd");
        assert!(!r.identical);
        assert_eq!(r.first_diff, Some(2));

        let r = RoundTrip::compare(b"abc", b"abcd");
        assert!(!r.identical);
        assert_eq!(r.first_diff, Some(3));
    }

    #[test]
    fn parse_failure_is_not_silent() {
        let bytes = fake(105, &[0u8; 64]);
        let err = SaveFile::from_bytes(SaveKind::Character, bytes).unwrap_err();
        // 头是合法的，所以错误必须来自结构解析
        assert!(matches!(err, SaveError::Parse(_)), "unexpected: {err}");
    }

    #[test]
    fn rejects_bad_magic() {
        let mut bytes = fake(105, b"whatever");
        bytes[0] = 0x00;
        let err = SaveFile::from_bytes(SaveKind::Character, bytes).unwrap_err();
        assert!(matches!(err, SaveError::Header(_)), "unexpected: {err}");
    }
}
