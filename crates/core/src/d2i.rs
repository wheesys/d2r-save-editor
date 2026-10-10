//! `.d2i` 共享储物箱的头部解析与"字节保真"包装（里程碑 3 的 .d2i 部分）。
//!
//! ## 已实测确认的结构（2026-10-10，样本 `ModernSharedStashSoftCoreV2.d2i` 6833 字节）
//!
//! ```text
//! 偏移  长度  内容
//! 0x00   4    魔数 0xAA55AA55
//! 0x04   4    结构版本 = 2（不是 .d2s 的存档版本！）
//! 0x08   4    0x00000069 = 105  ← 这才是该存档的 D2R 存档版本
//! 0x0C   4    文件长度 = 2500000（声明值，远大于实际 6833，暂不理解其含义）
//! 0x10  32    储物箱名（此样本全 0）
//! 0x30   4    金币
//! 0x34  12    未知/保留
//! 0x40   -    物品段：`JM`(0x4A4D) + 16 位物品数量 + 位紧凑物品流
//! ```
//!
//! ## 关键事实
//!
//! - **没有校验和字段**：试了 5 种起点（8/12/16/64/80 到文件末尾）都无法与偏移 8 的值对上，
//!   说明 .d2i 不存 CRC（与 .d2s 不同）。
//! - **没有 d2s 的那些段**：全文件搜不到 `Woof!`(任务)、`WS`(传送点)、`gf`(属性)、
//!   `if`(技能)、`jf`(佣兵)、`kf`(魔像)、`lf`(Demon) —— 共享箱只有"头部 + 物品段"。
//! - `JM` 出现在偏移 64，数量字段 = 41（该箱有 41 件物品）。
//!
//! ## 当前策略：**字节保真**
//!
//! 物品段的位流布局尚未逐字段验证，所以本模块**只解析出数量与位置**，
//! 物品段原样保留（`items_bytes`）；写回时头部与物品段都按原字节写，
//! 保证"读入→写出"逐字节一致。等物品段布局验证通过后再升级为结构化读写。

use crate::header::MAGIC;

/// `.d2i` 段标记：物品段 `JM`。
pub const ITEM_LIST_MARKER: [u8; 2] = [0x4A, 0x4D];

/// `.d2i` 解析/写出错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum D2iError {
    /// 文件太短。
    TooShort {
        /// 实际长度。
        len: usize,
        /// 需要的最小长度。
        need: usize,
    },
    /// 魔数不对。
    BadMagic(u32),
    /// 结构性错误（找不到物品段等）。
    Malformed(String),
}

impl std::fmt::Display for D2iError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort { len, need } => write!(f, ".d2i 过短：{len} 字节，至少需要 {need}"),
            Self::BadMagic(m) => write!(f, ".d2i 魔数不对：0x{m:08X}（期望 0x{MAGIC:08X}）"),
            Self::Malformed(s) => write!(f, ".d2i 结构错误：{s}"),
        }
    }
}

impl std::error::Error for D2iError {}

/// `.d2i` 共享储物箱。
#[derive(Debug, Clone)]
pub struct SharedStash {
    /// 结构版本（样本为 2）。
    pub structure_version: u32,
    /// D2R 存档版本（样本 105）。
    pub save_version: u32,
    /// 头部声明的长度字段。
    pub declared_size: u32,
    /// 储物箱名（头部 32 字节，去掉尾部 0）。
    pub name: String,
    /// 金币。
    pub gold: u32,
    /// 头部原始字节（0..物品段起点），写回时原样使用。
    pub header_bytes: Vec<u8>,
    /// 物品段的**原始字节**（含 `JM` 与数量），写回时原样使用。
    pub items_bytes: Vec<u8>,
    /// 物品数量（取自 `JM` 后的 16 位）。
    pub item_count: u16,
}

impl SharedStash {
    /// 物品段在文件中的偏移（= 头部长度）。
    pub fn items_offset(&self) -> usize {
        self.header_bytes.len()
    }

    /// 解析 `.d2i`。
    pub fn parse(bytes: &[u8]) -> Result<Self, D2iError> {
        if bytes.len() < 68 {
            return Err(D2iError::TooShort {
                len: bytes.len(),
                need: 68,
            });
        }
        let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if magic != MAGIC {
            return Err(D2iError::BadMagic(magic));
        }
        let structure_version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let save_version = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        let declared_size = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        let name_raw = &bytes[16..48];
        let name_end = name_raw
            .iter()
            .position(|b| *b == 0)
            .unwrap_or(name_raw.len());
        let name = String::from_utf8_lossy(&name_raw[..name_end]).to_string();
        let gold = u32::from_le_bytes([bytes[48], bytes[49], bytes[50], bytes[51]]);

        // 找物品段标记：头部之后第一个 `JM`
        let scan_from = 52;
        let marker_at = bytes[scan_from..]
            .windows(2)
            .position(|w| w == ITEM_LIST_MARKER)
            .map(|p| p + scan_from)
            .ok_or_else(|| D2iError::Malformed("找不到物品段标记 JM".into()))?;
        if marker_at + 4 > bytes.len() {
            return Err(D2iError::Malformed("物品段数量字段被截断".into()));
        }
        let item_count = u16::from_le_bytes([bytes[marker_at + 2], bytes[marker_at + 3]]);

        Ok(Self {
            structure_version,
            save_version,
            declared_size,
            name,
            gold,
            header_bytes: bytes[..marker_at].to_vec(),
            items_bytes: bytes[marker_at..].to_vec(),
            item_count,
        })
    }

    /// 按原始字节写回（逐字节保真）。
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.header_bytes.len() + self.items_bytes.len());
        out.extend_from_slice(&self.header_bytes);
        out.extend_from_slice(&self.items_bytes);
        out
    }

    /// 修改金币（会写回头部，因此仍是逐字节可逆的）。
    pub fn set_gold(&mut self, gold: u32) {
        self.gold = gold;
        let off = 48;
        if self.header_bytes.len() >= off + 4 {
            self.header_bytes[off..off + 4].copy_from_slice(&gold.to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 造一个最小合法 .d2i：头 64 字节 + JM + 数量 + 一点数据
    fn build(item_count: u16, payload: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&MAGIC.to_le_bytes()); // 0x00
        v.extend_from_slice(&2u32.to_le_bytes()); // 0x04 结构版本
        v.extend_from_slice(&105u32.to_le_bytes()); // 0x08 存档版本
        v.extend_from_slice(&6833u32.to_le_bytes()); // 0x0C 声明长度
        v.extend_from_slice(&[0u8; 32]); // 0x10 名称
        v.extend_from_slice(&1234u32.to_le_bytes()); // 0x30 金币
        v.extend_from_slice(&[0u8; 12]); // 0x34 保留
        debug_assert_eq!(v.len(), 64);
        v.extend_from_slice(&ITEM_LIST_MARKER);
        v.extend_from_slice(&item_count.to_le_bytes());
        v.extend_from_slice(payload);
        v
    }

    #[test]
    fn parses_header_and_item_section() {
        let bytes = build(41, &[0xAA, 0xBB, 0xCC]);
        let s = SharedStash::parse(&bytes).unwrap();
        assert_eq!(s.structure_version, 2);
        assert_eq!(s.save_version, 105);
        assert_eq!(s.declared_size, 6833);
        assert_eq!(s.gold, 1234);
        assert_eq!(s.item_count, 41);
        assert_eq!(s.items_offset(), 64);
        assert_eq!(s.items_bytes.len(), 2 + 2 + 3);
    }

    #[test]
    fn roundtrip_is_byte_identical() {
        let bytes = build(7, &[1, 2, 3, 4, 5]);
        let s = SharedStash::parse(&bytes).unwrap();
        assert_eq!(s.to_bytes(), bytes, "读入→写回必须逐字节一致");
    }

    #[test]
    fn gold_edit_is_byte_reversible() {
        let bytes = build(1, &[9]);
        let mut s = SharedStash::parse(&bytes).unwrap();
        s.set_gold(999);
        let out = s.to_bytes();
        assert_eq!(
            u32::from_le_bytes([out[48], out[49], out[50], out[51]]),
            999
        );
        // 除金币 4 字节外其余不变
        let mut expect = bytes.clone();
        expect[48..52].copy_from_slice(&999u32.to_le_bytes());
        assert_eq!(out, expect);
    }

    #[test]
    fn rejects_bad_magic_and_short_files() {
        assert!(matches!(
            SharedStash::parse(&[0u8; 10]),
            Err(D2iError::TooShort { .. })
        ));
        let mut bytes = build(0, &[]);
        bytes[0] = 0;
        assert!(matches!(
            SharedStash::parse(&bytes),
            Err(D2iError::BadMagic(_))
        ));
    }

    #[test]
    fn rejects_missing_item_marker() {
        let mut bytes = build(0, &[]);
        // 抹掉 JM
        bytes[64] = 0;
        bytes[65] = 0;
        assert!(matches!(
            SharedStash::parse(&bytes),
            Err(D2iError::Malformed(_))
        ));
    }
}
