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
  /** 装备位网格：每个格子只有一个槽位，画成固定槽 */
  slots?: string[];
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

/** 装备位（背包页之外的固定槽位） */
const EQUIP_SLOTS: { key: string; label: string }[] = [
  { key: "Head", label: "头" },
  { key: "Neck", label: "颈" },
  { key: "Torso", label: "身" },
  { key: "RightArm", label: "右臂" },
  { key: "LeftArm", label: "左臂" },
  { key: "RightRing", label: "右戒" },
  { key: "LeftRing", label: "左戒" },
  { key: "Gloves", label: "手" },
  { key: "Belt", label: "腰" },
  { key: "Feet", label: "脚" },
  { key: "RightHand", label: "右手" },
  { key: "LeftHand", label: "左手" },
];

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
      cols: 4,
      rows: 3,
      draggable: false,
      slots: EQUIP_SLOTS.map((s) => s.key),
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

  // 装备位：固定槽位框
  let slotBoxes = "";
  if (spec.slots) {
    const bySlot = new Map<number, ItemDto>();
    for (const it of mine) {
      const idx = equipSlotIndex(it);
      if (idx >= 0 && !bySlot.has(idx)) bySlot.set(idx, it);
    }
    slotBoxes = spec.slots
      .map((slotKey, i) => {
        const it = bySlot.get(i);
        const label = it ? itemLabel(it) : (EQUIP_SLOTS[i]?.label ?? slotKey);
        const sel = it && selected === it.index ? " selected" : "";
        const cls = it ? `blk q-${it.quality.toLowerCase()}${sel}` : "blk empty";
        const data = it ? ` data-item="${it.index}"` : "";
        const title = it ? escapeAttr(tooltip(it)) : `${EQUIP_SLOTS[i]?.label ?? slotKey}（空）`;
        const sock = it && it.socketCount > 0 ? `<span class="blk-sock">${it.socketedCount}/${it.socketCount}</span>` : "";
        const icon = it ? iconOf(it.code) : "";
        const art = icon
          ? `<img class="blk-icon" src="${icon}" alt="${escapeAttr(it ? it.code : "")}" draggable="false" />`
          : `<span class="blk-code">${escapeHtml(it ? it.code : (EQUIP_SLOTS[i]?.label ?? ""))}</span>`;
        return `<div class="${cls}" style="left:${(i % spec.cols) * CELL}px;top:${Math.floor(i / spec.cols) * CELL}px;width:${CELL - 2}px;height:${CELL - 2}px"
          ${data} title="${title}">
          ${art}
          <span class="blk-name">${escapeHtml(it ? label : "空")}</span>
          ${sock}
        </div>`;
      })
      .join("");
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

  return `<div class="grid-wrap" data-grid="${spec.key}">
    <div class="grid-head">${spec.title}
      <span class="muted">${mine.length} 件${spec.draggable ? "｜可拖拽移动" : ""}｜${spec.cols}×${spec.rows}</span>
    </div>
    <div class="grid" data-cols="${spec.cols}" data-rows="${spec.rows}"
      style="width:${spec.cols * CELL}px;height:${spec.rows * CELL}px">
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
