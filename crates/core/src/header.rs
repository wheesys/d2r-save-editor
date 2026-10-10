//! 存档公共头解析：`.d2s` 与 `.d2i` 都以 `0xAA55AA55` 开头。
//!
//! 头 16 字节布局（两种文件相同，但 `.d2i` 的第 2 个 DWORD 不是"存档版本"）：
//!
//! | 偏移 | 长度 | 字段 |
//! |---|---|---|
//! | 0x00 | 4 | 魔数 `0xAA55AA55` |
//! | 0x04 | 4 | 版本号（`.d2s`：96..105；`.d2i`：结构版本，本机样本为 2） |
//! | 0x08 | 4 | 文件长度（写回时必须重算） |
//! | 0x0C | 4 | 校验和（写回时必须重算） |

use std::fmt;

/// 存档魔数。
pub const MAGIC: u32 = 0xAA55_AA55;

/// 公共头原始字段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SaveHeader {
    /// 魔数（应为 [`MAGIC`]）。
    pub magic: u32,
    /// 版本号 / `.d2i` 结构版本。
    pub version: u32,
    /// 头部声明的文件长度。
    pub file_size: u32,
    /// 头部声明的校验和。
    pub checksum: u32,
}

/// 头解析错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeaderError {
    /// 文件不足 16 字节。
    TooShort(usize),
    /// 魔数不匹配。
    BadMagic(u32),
}

impl fmt::Display for HeaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HeaderError::TooShort(n) => write!(f, "文件过短：{n} 字节，至少需要 16 字节"),
            HeaderError::BadMagic(m) => write!(f, "魔数不匹配：0x{m:08X}（期望 0x{MAGIC:08X}）"),
        }
    }
}

impl std::error::Error for HeaderError {}

impl SaveHeader {
    /// 从文件前 16 字节解析头（不校验魔数）。
    pub fn parse(bytes: &[u8]) -> Result<Self, HeaderError> {
        if bytes.len() < 16 {
            return Err(HeaderError::TooShort(bytes.len()));
        }
        Ok(Self {
            magic: u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            version: u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            file_size: u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
            checksum: u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]),
        })
    }

    /// 解析头并校验魔数。
    pub fn parse_checked(bytes: &[u8]) -> Result<Self, HeaderError> {
        let h = Self::parse(bytes)?;
        if h.magic != MAGIC {
            return Err(HeaderError::BadMagic(h.magic));
        }
        Ok(h)
    }

    /// 头部声明的文件长度是否等于实际长度。
    pub fn size_matches(&self, actual_len: usize) -> bool {
        self.file_size as usize == actual_len
    }
}

/// 已知存档版本（`.d2s`），来自社区逆向结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveVersion {
    /// 暗黑2 毁灭之王 1.10+（32 位物品代码）。
    Lod96,
    /// D2R 2.0（引入 Huffman 物品代码）。
    D2R97,
    /// D2R 2.4–2.8。
    D2R99,
    /// D2R 3.0（高级储物箱 / Chronicle / Demon 段 / 新头部布局 / quantity 位）。
    D2R100To105,
    /// 未知版本。
    Unknown(u32),
}

impl SaveVersion {
    /// 由版本号归类。
    pub fn from_number(v: u32) -> Self {
        match v {
            96 => Self::Lod96,
            97 => Self::D2R97,
            98 | 99 => Self::D2R99,
            100..=105 => Self::D2R100To105,
            other => Self::Unknown(other),
        }
    }

    /// 是否属于 D2R（>= 97）。
    pub fn is_d2r(&self) -> bool {
        !matches!(self, Self::Lod96)
    }
}

impl fmt::Display for SaveVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lod96 => write!(f, "v96 (LoD 1.10+)"),
            Self::D2R97 => write!(f, "v97 (D2R 2.0)"),
            Self::D2R99 => write!(f, "v99 (D2R 2.4-2.8)"),
            Self::D2R100To105 => write!(f, "v100-105 (D2R 3.x / RotW)"),
            Self::Unknown(v) => write!(f, "v{v} (未知)"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(version: u32, size: u32, checksum: u32) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&MAGIC.to_le_bytes());
        v.extend_from_slice(&version.to_le_bytes());
        v.extend_from_slice(&size.to_le_bytes());
        v.extend_from_slice(&checksum.to_le_bytes());
        v
    }

    #[test]
    fn parses_header() {
        let bytes = sample(105, 2425, 0xDEAD_BEEF);
        let h = SaveHeader::parse_checked(&bytes).unwrap();
        assert_eq!(h.version, 105);
        assert_eq!(h.file_size, 2425);
        assert_eq!(h.checksum, 0xDEAD_BEEF);
        assert!(h.size_matches(2425));
        assert!(!h.size_matches(2424));
    }

    #[test]
    fn rejects_short_and_bad_magic() {
        assert_eq!(SaveHeader::parse_checked(&[0u8; 8]), Err(HeaderError::TooShort(8)));
        let mut bytes = sample(105, 16, 0);
        bytes[0] = 0;
        assert!(matches!(SaveHeader::parse_checked(&bytes), Err(HeaderError::BadMagic(_))));
    }

    #[test]
    fn classifies_versions() {
        assert_eq!(SaveVersion::from_number(96), SaveVersion::Lod96);
        assert_eq!(SaveVersion::from_number(99), SaveVersion::D2R99);
        assert_eq!(SaveVersion::from_number(105), SaveVersion::D2R100To105);
        assert_eq!(SaveVersion::from_number(200), SaveVersion::Unknown(200));
        assert!(SaveVersion::from_number(105).is_d2r());
        assert!(!SaveVersion::from_number(96).is_d2r());
    }
}
