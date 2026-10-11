/**
 * 容器网格视图（背包 / 合成箱 / 箱子 / 腰带 / 装备位）。
 *
 * 参考 diablo_edit 的做法：容器是一个 `列×行` 的网格，
 * 每件物品按自己的 `占格宽高` 画在网格坐标上（`invX/invY` 是格子坐标）。
 *
 * 两点这里的特殊性（实测结论）：
 * 1. 容器尺寸**存档里读不出来**（mod 会改），所以尺寸来自 `layout`（用户确认或默认值）。
 * 2. 一旦有物品的坐标超出网格，不隐藏它——放到"越界"区显示，避免看起来像丢东西。
 */

import type { ItemDto, LayoutSettings } from "./types";

/** 一个容器网格的定义（对应参考实现里的 GridView） */
export interface GridSpec {
  key: string;
  title: string;
  cols: number;
  rows: number;
  /** 判定物品属于本网格 */
  match: (item: ItemDto) => boolean;
  /** 是否允许拖拽移动 */
  draggable: boolean;
  /**
   * 槽位型容器的槽位列表（`slots` 里的 key 与 `EQUIP_SLOT_LAYOUT` 对应）。
   * 这类容器按槽位画，不做格子越界判断。
   */
  slots?: string[];
  /** 槽位型容器的画布尺寸（装备页按真实布局摆放，所以不用行列）。 */
  doll?: { width: number; height: number };
}

/** 单格像素尺寸 */
const CELL = 34;

/** 物品代码 → 图标 data URL（启动时从后端取一次） */
let iconByCode: Record<string, string> = {};

/** 设置图标表（由 main.ts 启动时注入） */
export function setItemIcons(icons: [string, string][]) {
  iconByCode = Object.fromEntries(icons);
}

/** 取某物品代码的图标（没有则返回空字符串，界面会退化成只显示代码） */
function iconOf(code: string): string {
  return iconByCode[code] ?? "";
}

/**
 * 装备位：按**游戏里角色装备页**的真实布局摆放。
 *
 * 坐标来自数据层的布局表（`Inventory.txt`，这里用 Sorceress 行；
 * 实测各职业的槽位坐标一致）：把原来的 `invLeft/invTop` 当原点、
 * 各槽的 `xxxLeft/Top/Width/Height` 减掉原点，再乘一个统一缩放。
 *
 * 单位：布局表里 `gridBoxWidth = 29`，而背包一格的边长是 inventory 面板宽度
 * 除以列数（320/10 = 32），两者接近，所以这里按"玩家熟悉的格子大小"把
 * 装备页整体缩到约 0.62 倍（原版 346×234 → 约 215×145），
 * 这样 12 个槽加上背包能一屏放下。
 *
 * 例外：**戒指两个槽在布局表里没有坐标**（游戏把它们写在界面代码里），
 * 位置是按左右对称估的，标了 `estimated`。
 */
const EQUIP_SLOT_LAYOUT: {
  key: string;
  label: string;
  /** 相对装备页原点的像素（未缩放）。 */
  x: number;
  y: number;
  w: number;
  h: number;
  /** 位置是不是估的（布局表里没有）。 */
  estimated?: boolean;
}[] = [
  // 头 455,8 / 54×51
  { key: "Head", label: "头", x: 455 - 320, y: 8, w: 54, h: 51 },
  // 颈 529,35 / 23×24
  { key: "Neck", label: "颈", x: 529 - 320, y: 35, w: 23, h: 24 },
  // 身 453,77 / 56×82
  { key: "Torso", label: "身", x: 453 - 320, y: 77, w: 56, h: 82 },
  // 右臂 340,47 / 55×112
  { key: "RightArm", label: "右臂", x: 340 - 320, y: 47, w: 55, h: 112 },
  // 左臂 571,47 / 55×112
  { key: "LeftArm", label: "左臂", x: 571 - 320, y: 47, w: 55, h: 112 },
  // 手（手套）341,181 / 54×53
  { key: "Gloves", label: "手套", x: 341 - 320, y: 181, w: 54, h: 53 },
  // 脚 572,182 / 54×52
  { key: "Feet", label: "脚", x: 572 - 320, y: 182, w: 54, h: 52 },
  // 腰 456,179 / 52×25
  { key: "Belt", label: "腰", x: 456 - 320, y: 179, w: 52, h: 25 },
  // 右手（武器）415,180 / 23×24
  { key: "RightHand", label: "右手", x: 415 - 320, y: 180, w: 23, h: 24 },
  // 左手（副手）529,180 / 23×24
  { key: "LeftHand", label: "左手", x: 529 - 320, y: 180, w: 23, h: 24 },
  // 戒指：布局表里没有坐标，按与双手对称的位置估
  { key: "RightRing", label: "右戒", x: 390 - 320, y: 145, w: 23, h: 24, estimated: true },
  { key: "LeftRing", label: "左戒", x: 555 - 320, y: 145, w: 23, h: 24, estimated: true },
];

/**
 * 装备页整体缩放。
 *
 * 布局表里原版装备页是 346×234（相对坐标），这里按 0.62 缩到约 215×145：
 * 这样它和背包（默认 10 格 = 340 宽）在窗口里各自占一行的位置，
 * 背包自然落到下面一行，不再是"挤在同一排、后半截看不到"。
 */
const EQUIP_SCALE = 0.62;

/** 缩放后的装备页尺寸。 */
const EQUIP_DOLL_SIZE = {
  width: Math.round(346 * EQUIP_SCALE),
  height: Math.round(234 * EQUIP_SCALE),
};

/** 装备位（背包页之外的固定槽位） */
const EQUIP_SLOTS: { key: string; label: string }[] = EQUIP_SLOT_LAYOUT.map((s) => ({
  key: s.key,
  label: s.label,
}));

/** 按用户确认的布局生成各容器网格 */
export function buildGrids(layout: LayoutSettings, items: ItemDto[] = []): GridSpec[] {
  const isStored = (i: ItemDto) => i.mode === "Stored";
  const isEquipped = (i: ItemDto) => i.mode === "Equipped";
  const isInBelt = (i: ItemDto) => i.mode === "InBelt";

  // 腰带尺寸：存档里读不出来，而且 mod 会把它加宽（实测出现 x=15 的药水格）。
  // 所以按**实际用到的坐标**取跨度，至少 4×4（原版大小）。
  let beltCols = 4;
  let beltRows = 4;
  for (const i of items) {
    if (!isInBelt(i) && !isStored(i)) continue;
    if (!isInBelt(i) && i.storePage !== "Belt") continue;
    if (i.invX + i.width > beltCols) beltCols = i.invX + i.width;
    if (i.invY + i.height > beltRows) beltRows = i.invY + i.height;
  }

  return [
    {
      key: "Equip",
      title: "装备位",
      cols: 1,
      rows: 1,
      draggable: false,
      slots: EQUIP_SLOTS.map((s) => s.key),
      doll: EQUIP_DOLL_SIZE,
      match: isEquipped,
    },
    {
      key: "Inventory",
      title: "背包",
      cols: layout.inventoryCols,
      rows: layout.inventoryRows,
      draggable: true,
      match: (i) => isStored(i) && i.storePage === "Inventory",
    },
    {
      key: "Cube",
      title: "合成箱",
      cols: layout.cubeCols,
      rows: layout.cubeRows,
      draggable: true,
      match: (i) => isStored(i) && i.storePage === "Cube",
    },
    {
      key: "Stash",
      title: "箱子",
      cols: layout.stashCols,
      rows: layout.stashRows,
      draggable: true,
      match: (i) => isStored(i) && i.storePage === "Stash",
    },
    {
      key: "Belt",
      title: "腰带",
      cols: beltCols,
      rows: beltRows,
      draggable: true,
      match: (i) => isInBelt(i) || (isStored(i) && i.storePage === "Belt"),
    },
  ];
}

/** 装备位 → 槽位下标 */
function equipSlotIndex(item: ItemDto): number {
  if (item.bodyLocation === "None") return -1;
  // 双手武器会同时占 RightHand/LeftHand，这里按第一个匹配的槽位画
  return EQUIP_SLOTS.findIndex((s) => s.key === item.bodyLocation);
}

/** 渲染一个容器网格 */
function renderGrid(spec: GridSpec, items: ItemDto[], selected: number | null): string {
  const mine = items.filter(spec.match);
  const inBounds: ItemDto[] = [];
  const outOfBounds: ItemDto[] = [];

  // 槽位型容器（装备位）按槽位画，物品的 (invX,invY) 对它没有意义，
  // 所以**不做格子越界判断**——否则戴在身上的 2×2 物品会被误判成"越界"而不显示。
  if (spec.slots) {
    inBounds.push(...mine);
  } else {
    for (const it of mine) {
      const fits = it.invX + it.width <= spec.cols && it.invY + it.height <= spec.rows;
      (fits ? inBounds : outOfBounds).push(it);
    }
  }

  // 槽位型容器（装备位）只用下面的槽位框渲染，这里不要再画一遍，否则同一件装备会出现两次。
  const blocks = spec.slots
    ? ""
    : inBounds
        .map((it) => {
          const sel = selected === it.index ? " selected" : "";
          const style =
            `left:${it.invX * CELL}px;top:${it.invY * CELL}px;` +
            `width:${it.width * CELL - 2}px;height:${it.height * CELL - 2}px;`;
          const label = itemLabel(it);
          const socketNote =
            it.socketCount > 0 ? `<span class="blk-sock">${it.socketedCount}/${it.socketCount}</span>` : "";
          const icon = iconOf(it.code);
          const art = icon
            ? `<img class="blk-icon" src="${icon}" alt="${escapeAttr(it.code)}" draggable="false" />`
            : `<span class="blk-code">${escapeHtml(it.code)}</span>`;
          return `<div class="blk q-${it.quality.toLowerCase()}${sel}" style="${style}"
        data-item="${it.index}" title="${escapeAttr(tooltip(it))}">
        ${art}
        <span class="blk-name">${escapeHtml(label)}</span>
        ${socketNote}
      </div>`;
        })
        .join("");

  // 装备位：按真实布局摆放的槽位框（坐标来自 EQUIP_SLOT_LAYOUT）
  let slotBoxes = "";
  if (spec.slots) {
    const bySlot = new Map<number, ItemDto>();
    for (const it of mine) {
      const idx = equipSlotIndex(it);
      if (idx >= 0 && !bySlot.has(idx)) bySlot.set(idx, it);
    }
    slotBoxes = EQUIP_SLOT_LAYOUT.map((slot, i) => {
      const it = bySlot.get(i);
      const label = it ? itemLabel(it) : slot.label;
      const sel = it && selected === it.index ? " selected" : "";
      const cls = it ? `blk q-${it.quality.toLowerCase()}${sel}` : "blk empty";
      const data = it ? ` data-item="${it.index}"` : "";
      const note = slot.estimated ? "（位置为估算）" : "";
      const title = it
        ? escapeAttr(tooltip(it))
        : `${slot.label}（空）${note}`;
      const sock =
        it && it.socketCount > 0
          ? `<span class="blk-sock">${it.socketedCount}/${it.socketCount}</span>`
          : "";
      const icon = it ? iconOf(it.code) : "";
      const art = icon
        ? `<img class="blk-icon" src="${icon}" alt="${escapeAttr(it ? it.code : "")}" draggable="false" />`
        : `<span class="blk-code">${escapeHtml(it ? it.code : slot.label)}</span>`;
      // 槽位按布局表的比例缩放定位；小槽（戒指/双手）不放名称，避免文字溢出
      const w = Math.round(slot.w * EQUIP_SCALE);
      const h = Math.round(slot.h * EQUIP_SCALE);
      const showName = w >= 40 && !it;
      return `<div class="${cls}${slot.estimated ? " estimated" : ""}"
        style="left:${Math.round(slot.x * EQUIP_SCALE)}px;top:${Math.round(slot.y * EQUIP_SCALE)}px;width:${w}px;height:${h}px"
        ${data} title="${title}">
        ${art}
        ${showName ? `<span class="blk-name">${escapeHtml(label)}</span>` : ""}
        ${sock}
      </div>`;
    }).join("");
  }

  const oob = outOfBounds.length
    ? `<div class="oob">越界（坐标超出当前容器设置，可能是别的面板的物品）：` +
      outOfBounds
        .map(
          (it) =>
            `<span class="oob-item" data-item="${it.index}" title="${escapeAttr(tooltip(it))}">#${it.index} ${escapeHtml(it.code)} (${it.invX},${it.invY})</span>`,
        )
        .join("") +
      `</div>`
    : "";

  // 画布尺寸：槽位型容器用装备页的真实尺寸，网格型用 格子数×格宽
  const canvas = spec.doll
    ? { width: spec.doll.width, height: spec.doll.height }
    : { width: spec.cols * CELL, height: spec.rows * CELL };
  const sizeNote = spec.doll ? "游戏角色装备页布局" : `${spec.cols}×${spec.rows}`;

  return `<div class="grid-wrap" data-grid="${spec.key}">
    <div class="grid-head">${spec.title}
      <span class="muted">${mine.length} 件${spec.draggable ? "｜可拖拽移动" : ""}｜${sizeNote}</span>
    </div>
    <div class="grid${spec.doll ? " doll" : ""}" data-cols="${spec.cols}" data-rows="${spec.rows}"
      style="width:${canvas.width}px;height:${canvas.height}px">
      ${slotBoxes}
      ${blocks}
    </div>
    ${oob}
  </div>`;
}

/** 渲染全部容器 */
export function renderInventory(layout: LayoutSettings, items: ItemDto[], selected: number | null): string {
  const grids = buildGrids(layout, items);
  return grids.map((g) => renderGrid(g, items, selected)).join("");
}

/** 块上显示的名称（太长会溢出，截断一下） */
function itemLabel(item: ItemDto): string {
  const name = item.name || item.code;
  return name.length > 6 ? `${name.slice(0, 6)}…` : name;
}

/** 悬浮提示 */
function tooltip(item: ItemDto): string {
  const parts = [
    `#${item.index} ${item.name}（${item.code}）`,
    `品质 ${item.quality}｜等级 ${item.level}`,
    `位置 ${item.storePage}${item.bodyLocation !== "None" ? "·" + item.bodyLocation : ` (${item.invX},${item.invY})`}`,
    `占格 ${item.width}×${item.height}｜属性 ${item.statsCount} 条`,
  ];
  if (item.durability !== null && item.maxDurability) parts.push(`耐久 ${item.durability}/${item.maxDurability}`);
  if (item.quantity !== null) parts.push(`数量 ${item.quantity}`);
  if (item.socketCount > 0) parts.push(`孔 ${item.socketedCount}/${item.socketCount}`);
  return parts.join("\n");
}

export function escapeHtml(text: string): string {
  return text.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}

export function escapeAttr(text: string): string {
  return escapeHtml(text).replace(/\n/g, "&#10;");
}

/** 网格拖拽：把物品移动到指针所在的格子 */
export function bindGridDrag(
  root: HTMLElement,
  layout: LayoutSettings,
  items: ItemDto[],
  onSelect: (index: number) => void,
  onMove: (index: number, page: string, col: number, row: number) => void,
) {
  root.querySelectorAll<HTMLElement>(".blk[data-item]").forEach((el) => {
    el.addEventListener("click", () => onSelect(Number(el.dataset.item)));
    el.draggable = true;
    el.addEventListener("dragstart", (ev) => {
      (ev as DragEvent).dataTransfer?.setData("text/plain", el.dataset.item!);
    });
  });

  root.querySelectorAll<HTMLElement>(".oob-item").forEach((el) => {
    el.addEventListener("click", () => onSelect(Number(el.dataset.item)));
  });

  root.querySelectorAll<HTMLElement>(".grid").forEach((grid) => {
    const wrap = grid.closest(".grid-wrap") as HTMLElement;
    const gridKey = wrap?.dataset.grid ?? "";
    const cols = Number(grid.dataset.cols);
    const rows = Number(grid.dataset.rows);
    grid.addEventListener("dragover", (ev) => {
      ev.preventDefault();
      (ev as DragEvent).dataTransfer!.dropEffect = "move";
    });
    grid.addEventListener("drop", (ev) => {
      ev.preventDefault();
      const index = Number((ev as DragEvent).dataTransfer?.getData("text/plain"));
      if (!Number.isInteger(index)) return;
      const rect = grid.getBoundingClientRect();
      const scale = grid.clientWidth / (cols * CELL) || 1;
      const col = Math.max(0, Math.min(cols - 1, Math.floor((ev.clientX - rect.left) / (CELL * scale))));
      const row = Math.max(0, Math.min(rows - 1, Math.floor((ev.clientY - rect.top) / (CELL * scale))));
      const item = items.find((i) => i.index === index);
      if (!item) return;
      if (item.invX === col && item.invY === row && item.storePage === gridKey) return;
      onMove(index, gridKey, col, row);
    });
  });
  void layout;
}
