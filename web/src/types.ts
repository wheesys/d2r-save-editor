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
  name_resolved: boolean;
  quality: string;
  level: number;
  mode: string;
  body_location: string;
  store_page: string;
  inv_x: number;
  inv_y: number;
  stats_count: number;
  defense: number | null;
  durability: number | null;
  maxDurability: number | null;
  quantity: number | null;
  socketCount: number;
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