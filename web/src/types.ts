// 与 Rust 侧 dto.rs 对应的类型（字段名已按 camelCase 序列化）
export interface DatasetInfo {
  root: string;
  tables: number;
  strings: number;
  languages: string[];
}

export interface SaveInfo {
  path: string;
  kind: string;
  version: number;
  size: number;
  checksum: number | null;
  roundtripIdentical: boolean;
  /** 存档里的角色名（.d2i 为空），保存前用来确认要覆盖谁 */
  characterName: string;
}

export interface CharacterDto {
  name: string;
  class: string;
  level: number;
  strength: number;
  dexterity: number;
  vitality: number;
  energy: number;
  statPoints: number;
  skillPoints: number;
  life: number;
  maxLife: number;
  mana: number;
  maxMana: number;
  stamina: number;
  experience: number;
  gold: number;
  stashGold: number;
}

export interface SkillDto {
  index: number;
  skillId: number | null;
  name: string | null;
  level: number;
}

export interface WaypointsDto {
  difficulty: string;
  active: boolean[];
}

export interface ItemDto {
  index: number;
  code: string;
  name: string;
  // 后端这些字段是 camelCase 序列化的（DTO 上是 snake_case），
  // 早期这里跟着 Rust 字段名写成了 snake_case，导致界面上取到 undefined。
  nameResolved: boolean;
  quality: string;
  level: number;
  mode: string;
  bodyLocation: string;
  storePage: string;
  invX: number;
  invY: number;
  statsCount: number;
  defense: number | null;
  durability: number | null;
  maxDurability: number | null;
  quantity: number | null;
  socketCount: number;
  socketedCount: number;
  socketedFlag: boolean;
  width: number;
  height: number;
}

export interface ItemStats {
  total: number;
  byPage: [string, number][];
  byQuality: [string, number][];
  equipped: number;
  withSockets: number;
}

export interface BundleInfo {
  available: boolean;
  excelTables: number;
  stringFiles: number;
  icons: number;
  totalBytes: number;
}

export interface VerifyResult {
  ok: boolean;
  bytes: number;
  message: string;
}

export interface SaveResult {
  path: string;
  bytes: number;
  backed_up: boolean;
  edits: number;
}

export interface AppError {
  kind: string;
  message: string;
}

// 修改指令（与 d2r_core::Edit 的 JSON 形态一致：tag = "kind"）
export type Edit =
  | { kind: "setLevel"; level: number }
  | { kind: "setStat"; stat: number; value: number }
  | { kind: "setSkill"; index: number; level: number }
  | { kind: "unlockWaypoints"; difficulty: number }
  | { kind: "setWaypoint"; difficulty: number; index: number; active: boolean };

// StatId 常量（与 d2s::StatId 一致）
export type ItemEdit =
  | { kind: "setQuantity"; index: number; value: number }
  | { kind: "setDurability"; index: number; value: number }
  | { kind: "setDefense"; index: number; value: number }
  | { kind: "setSockets"; index: number; count: number }
  | { kind: "setStat"; index: number; stat: number; value: number }
  | { kind: "removeStat"; index: number; stat: number }
  | { kind: "moveTo"; index: number; col: number; row: number }
  | { kind: "remove"; index: number };

export const STAT = {
  STRENGTH: 0,
  ENERGY: 1,
  DEXTERITY: 2,
  VITALITY: 3,
  STAT_POINTS: 4,
  SKILL_POINTS: 5,
  GOLD: 14,
  STASH_GOLD: 15,
} as const;

export interface LayoutSettings {
  inventoryCols: number;
  inventoryRows: number;
  cubeCols: number;
  cubeRows: number;
  stashCols: number;
  stashRows: number;
  mercFullEquipment: boolean;
  confirmedByUser: boolean;
}

export interface LayoutHints {
  inventoryExtent: [number, number] | null;
  inventoryMax: [number, number] | null;
  cubeMax: [number, number] | null;
  stashExtent: [number, number] | null;
  stashMax: [number, number] | null;
  mercSection: boolean;
  mercItems: number;
  unknowns: string[];
}

export interface BoundsIssue {
  index: number;
  code: string;
  page: string;
  at: [number, number];
  size: [number, number];
  container: [number, number];
}
export interface ItemStatDto {
  id: number;
  layer: number;
  value: number;
  name: string;
  internal: string;
  step: number;
  hasParamBits: boolean;
  userVisible: boolean;
}

export interface ItemDetailDto {
  item: ItemDto;
  stats: ItemStatDto[];
  runewordId: number | null;
}

export interface StatCatalogEntry {
  id: number;
  name: string;
  internal: string;
  step: number;
  hasParamBits: boolean;
  userVisible: boolean;
  descTemplate: string | null;
}
export interface MercenaryItem {
  index: number;
  code: string;
  quality: string;
  level: number;
  bodyLocation: string;
  statsCount: number;
  socketCount: number;
  socketedCount: number;
  stats: [number, number, number][];
}
/** 任务状态（与后端 QuestState 对应）。 */
export type QuestState = "notStarted" | "inProgress" | "completed";

export interface QuestDto {
  act: number;
  slot: number;
  name: string;
  /** 原始位（显示用，便于核对；判定不用它） */
  flags: number;
  state: QuestState;
  /** 是否是可编辑的真任务槽（序章/收尾/NPC 对话槽为 false） */
  editable: boolean;
}

export interface QuestDifficultyDto {
  difficulty: string;
  difficultyIndex: number;
  /** [幕][槽] */
  acts: QuestDto[][];
}