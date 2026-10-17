import { invoke } from "@tauri-apps/api/core";

import {
  bindGridDrag,
  escapeAttr,
  escapeHtml,
  renderInventory,
  renderMercDoll,
  setItemIcons,
} from "./inventory";
import { STAT_TABLE_HEAD, renderLayerControl, renderStatRow, statValueText } from "./statrow";
import type { LayerOption } from "./statrow";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";

import {
  type AppError,
  type CharacterDto,
  type DatasetInfo,
  type Edit,
  type ItemDto,
  type ItemEdit,
  type ItemStats,
  type ItemStatDto,
  type BundleInfo,
  type ItemDetailDto,
  type StatCatalogEntry,
  type LibraryItemDto,
  type QuestDifficultyDto,
  type QuestState,
  type LayoutSettings,
  type LayoutHints,
  type BoundsIssue,
  type SaveInfo,
  type SaveResult,
  type SkillDto,
  type SkillsViewDto,
  STAT,
  type VerifyResult,
  type WaypointsDto,
} from "./types";

const $ = <T extends HTMLElement>(sel: string): T => {
  const el = document.querySelector<T>(sel);
  if (!el) throw new Error(`缺少元素 ${sel}`);
  return el;
};

let skills: SkillDto[] = [];
let items: ItemDto[] = [];
/** 技能页视图（按游戏三系技能树分组）。后端不支持时为 null，界面退回平铺。 */
let skillsView: SkillsViewDto | null = null;
/** 技能图标缓存（按英文技能名取，与物品图标分开）。 */
let skillIcons: Record<string, string> = {};

function toast(message: string, kind: "ok" | "error" = "ok", ms = 4000) {
  const el = $("#toast");
  el.textContent = message;
  el.className = `toast ${kind === "error" ? "error" : ""}`;
  window.setTimeout(() => el.classList.add("hidden"), ms);
}

function showError(e: unknown, prefix = "操作失败") {
  const err = e as AppError | string;
  const msg = typeof err === "string" ? err : `${err.kind ?? "error"}: ${err.message ?? ""}`;
  toast(`${prefix}\n${msg}`, "error", 7000);
}

/** 调用后端命令，失败时抛出可读错误 */
async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(cmd, args);
}

/** 数据包信息缓存（数据集那行要显示它的体积） */
let bundleInfo: BundleInfo | null = null;

/** 取数据包信息（后端缺失该命令时返回 null） */
async function loadBundleInfo(): Promise<BundleInfo | null> {
  try {
    return await call<BundleInfo>("bundle_info");
  } catch {
    return null;
  }
}

/**
 * 整行重建数据集那行。
 *
 * 不要改成 `+=` 追加：`loadAll()` 会在刷新/保存后被反复调用，
 * 追加会让这一行越贴越长。
 */
async function refreshDataset() {
  // 用户要求首页**不展示**数据来源那行（表数/字符串数/图标数/数据包路径）。
  // 仍保留 `dataset_info` 调用：它顺带完成一次数据可用性探测，
  // 也能让后端在缺数据时给出提示；只是不再把细节渲染到界面。
  const info = await call<DatasetInfo>("dataset_info");
  const el = $("#dataset");
  el.textContent = "";
  el.classList.add("hidden");
  void info;
  void bundleInfo;
}

function setDirty(dirty: boolean) {
  $("#dirty").classList.toggle("hidden", !dirty);
  $("#btn-save").toggleAttribute("disabled", !dirty);
  // 「放弃修改」也要跟着启用/禁用：以前它只在 HTML 里写死 `disabled`，
  // 而这里只切换了「保存」，于是**永远点不了**（用户反馈过）。
  // 没有改动时它可以点但没意义，所以跟「保存」用同一条件。
  $("#btn-reload").toggleAttribute("disabled", !dirty);
}

function renderSaveInfo(info: SaveInfo) {
  const rows: [string, string][] = [
    ["路径", info.path],
    ["类型", info.kind === "stash" ? "共享储物箱（.d2i）" : "角色存档（.d2s）"],
  ];
  // 角色名：同目录下多个存档很容易改错文件，显式显示一下
  if (info.characterName) rows.push(["角色", info.characterName]);
  rows.push(
    ["存档版本", String(info.version)],
    ["文件大小", `${info.size} 字节`],
    ["校验和", info.checksum === null ? "无（.d2i 不存）" : `0x${(info.checksum >>> 0).toString(16).toUpperCase()}`],
    ["往返一致", info.roundtripIdentical ? "是" : "否（结构有差异，谨慎保存）"],
  );
  $("#save-info").innerHTML = rows.map(([k, v]) => `<tr><th>${k}</th><td>${v}</td></tr>`).join("");
  $("#overview").classList.remove("hidden");
}

function fillCharacter(c: CharacterDto) {
  ($("#f-level") as HTMLInputElement).value = String(c.level);
  ($("#f-strength") as HTMLInputElement).value = String(c.strength);
  ($("#f-dexterity") as HTMLInputElement).value = String(c.dexterity);
  ($("#f-vitality") as HTMLInputElement).value = String(c.vitality);
  ($("#f-energy") as HTMLInputElement).value = String(c.energy);
  ($("#f-stat-points") as HTMLInputElement).value = String(c.statPoints);
  ($("#f-skill-points") as HTMLInputElement).value = String(c.skillPoints);
  ($("#f-gold") as HTMLInputElement).value = String(c.gold);
  ($("#f-stash-gold") as HTMLInputElement).value = String(c.stashGold);
  $("#char-readonly").textContent =
    `${c.name}｜${c.class}｜生命 ${c.life}/${c.maxLife}｜法力 ${c.mana}/${c.maxMana}｜体力 ${c.stamina}｜经验 ${c.experience}`;
  $("#char-section").classList.remove("hidden");
}

/**
 * 技能页：按游戏的**三系技能树**渲染。
 *
 * 每棵树是一个 2×5 的方块网格（技能图标 + 等级输入），与游戏里的技能树形态一致。
 * 分组由后端 `skills_view` 给出——技能树的归属不在数据文件里，靠内置名单，
 * 所以这里**只负责画**，不做归属判断。
 *
 * 职业未知（例如某个 mod 的自创职业没有名单）时后端会给空 `trees`，
 * 那就退回平铺列表，并把未归类的技能照常列出来，免得有技能完全无法编辑。
 */
function renderSkills() {
  // 顶部：职业 + 剩余技能点（游戏里也是这个位置）
  const head = $("#skills-head");
  if (skillsView && skillsView.classCode) {
    head.innerHTML =
      `<span class="badge">职业 ${escapeHtml(skillsView.classCode)}</span>` +
      `<span class="muted">剩余技能点 <b>${skillsView.skillPoints}</b></span>`;
  } else {
    head.innerHTML = "";
  }

  const trees = skillsView?.trees ?? [];
  const host = $("#skills");
  if (trees.length > 0) {
    host.innerHTML = trees
      .map(
        (tree) => `
      <div class="skill-tree">
        <div class="skill-tree-head">${escapeHtml(tree.name)}</div>
        <div class="skill-grid">
          ${tree.skills.map((s) => skillCell(s)).join("")}
        </div>
      </div>`,
      )
      .join("");
  } else {
    // 没有技能树：平铺已加点的技能（未知职业时的退路）
    host.innerHTML = "";
  }

  // 未归类的技能：单独一块，保证照样能改
  const rest = skillsView?.unclassified ?? [];
  const unclassifiedHost = $("#skills-unclassified");
  if (rest.length > 0) {
    unclassifiedHost.innerHTML =
      `<div class="skill-tree"><div class="skill-tree-head">未归类技能（${rest.length}）</div>` +
      `<div class="skill-list">${rest.map((s) => skillRow(s)).join("")}</div></div>`;
  } else if (trees.length === 0) {
    // 完全没有分组信息时，把所有技能平铺出来
    unclassifiedHost.innerHTML =
      `<div class="skill-tree"><div class="skill-tree-head">技能（${skills.length}）</div>` +
      `<div class="skill-list">${skills.map((s) => skillRow(s)).join("")}</div></div>`;
  } else {
    unclassifiedHost.innerHTML = "";
  }
}

/** 技能树里的一个方块：图标 + 等级输入 + 名字。 */
function skillCell(s: SkillDto): string {
  // 占位项（技能树里的空格）：渲染成不可见的空格子，保住 6×3 的网格形状
  if (s.skillId === null && s.name === null) {
    return `<div class="skill-empty"></div>`;
  }
  const icon = skillIcons[iconCodeOf(s)] ?? "";
  const art = icon
    ? `<img src="${icon}" alt="" />`
    // 没有技能图标数据源时，用技能名前两字占位（空白格子看不出是什么）
    : `<span class="skill-fallback">${escapeHtml((s.name ?? "?").slice(0, 2))}</span>`;
  const zeroCls = s.level > 0 ? "" : " zero";
  const title = `槽位 ${s.index}｜技能ID ${s.skillId ?? "?"}｜${s.name ?? "（未解析）"}`;
  return `<div class="skill-cell${zeroCls}" title="${escapeAttr(title)}">
    <div class="skill-icon">${art}</div>
    <div class="skill-name">${escapeHtml(s.name ?? `技能 ${s.skillId ?? s.index}`)}</div>
    <input type="number" min="0" max="99" data-skill-index="${s.index}" value="${s.level}" />
  </div>`;
}

/** 平铺列表里的一行（未归类技能用）。 */
function skillRow(s: SkillDto): string {
  return `<div class="row${s.level === 0 ? " zero" : ""}">
    <span class="name" title="槽位 ${s.index}｜技能ID ${s.skillId ?? "?"}">${escapeHtml(
      s.name ?? `（未命名技能 ${s.skillId ?? s.index}）`,
    )}</span>
    <input type="number" min="0" max="99" data-skill-index="${s.index}" value="${s.level}" />
  </div>`;
}

/**
 * 技能图标按**技能代码名**查（图标文件名就是 skills.txt 的 skill 列改成小写下划线）。
 *
 * 名字是本地化的，所以先用技能树名单里的英文名反查；拿不到就退化成文字。
 */
function iconCodeOf(s: SkillDto): string {
  const english = skillsView?.englishNames?.[String(s.skillId ?? "")];
  return english ? english.toLowerCase().replace(/[^a-z0-9]+/g, "") : "";
}

/**
 * 传送点：显示**小站名字**而不是编号。
 *
 * 名字来自 `Levels.txt` 的 `Waypoint` 列（0..=38，就是存档里的位下标）
 * 对应的关卡名，再由文本表翻成中文；`waypointNames[i]` 即第 i 位。
 */
function renderWaypoints(list: WaypointsDto[]) {
  // 没有名字的位（存档第 39 位是填充位）直接跳过，不要显示成"小站#N"
  const nameOf = (i: number) => waypointNames[i] ?? "";
  $("#waypoints").innerHTML = list
    .map(
      (w, d) => `
      <div class="wp-group">
        <div class="muted">${w.difficulty}（已开通 ${w.active.filter(Boolean).length} / ${waypointNames.filter((n) => n).length}）</div>
        <div class="wp-row">
          ${w.active
            .map((on, i) => {
              const name = nameOf(i);
              if (!name) return "";
              return `<label class="wp ${on ? "on" : ""}" title="${escapeAttr(name)}">
                   <input type="checkbox" data-wp-diff="${d}" data-wp-index="${i}" ${on ? "checked" : ""}/>
                   ${escapeHtml(name)}
                 </label>`;
            })
            .join("")}
        </div>
      </div>`,
    )
    .join("");
  $("#waypoints-section").classList.remove("hidden");
}

/** 当前选中的物品序号（用于属性弹窗） */
let selectedItem: number | null = null;

/**
 * 弹窗当前操作的是哪个物品列表。
 *
 * 佣兵段与主物品列表**序号体系不同**，所以这个目标必须跟着弹窗走，
 * 改值/删属性/应用修改都要带上它。
 */
type ItemTarget = "inventory" | "mercenary";
let selectedTarget: ItemTarget = "inventory";
/** 属性目录缓存 */
let statCatalog: StatCatalogEntry[] = [];
/** 当前布局设置（容器网格尺寸要用） */
let currentLayout: LayoutSettings | null = null;
/** 小站名表（下标 = 存档里的位下标） */
let waypointNames: string[] = [];

/** 物品列表行：点一行进入明细 */
function renderItems(list: ItemDto[]) {
  const byPage = new Map<string, number>();
  for (const it of list) byPage.set(it.storePage, (byPage.get(it.storePage) ?? 0) + 1);
  $("#items-summary").textContent =
    `共 ${list.length} 件｜` +
    [...byPage.entries()].map(([k, v]) => `${k}: ${v}`).join("｜");

  const rows = list
    .map((it) => {
      const durCell =
        it.maxDurability && it.maxDurability > 0
          ? `<input type="number" class="cell-input" min="0" max="${it.maxDurability}" value="${it.durability ?? 0}" data-dur-index="${it.index}" /> / ${it.maxDurability}`
          : `<span class="muted">—</span>`;
      const qtyCell =
        it.quantity !== null
          ? `<input type="number" class="cell-input" min="1" max="511" value="${it.quantity}" data-qty-index="${it.index}" />`
          : `<span class="muted">—</span>`;
      // 孔数：0 孔也给个输入框，方便加孔
      const sockCell =
        `<input type="number" class="cell-input" min="0" max="7" value="${it.socketCount}" data-sock-index="${it.index}" />` +
        (it.socketedCount > 0 ? `<span class="muted"> 镶${it.socketedCount}</span>` : "");
      const sel = selectedItem === it.index ? " class=\"selected\"" : "";
      return `<tr data-item-index="${it.index}"${sel}>
        <td>${it.index}</td>
        <td>${it.code}</td>
        <td>${it.name}${it.nameResolved ? "" : " <span class='muted'>(未解析)</span>"}</td>
        <td>${it.quality}</td>
        <td>${it.level}</td>
        <td>${shortPage(it.storePage)}${it.bodyLocation !== "None" ? ` · ${shortSlot(it.bodyLocation)}` : ` (${it.invX},${it.invY})`}</td>
        <td>${it.width}×${it.height}</td>
        <td>${durCell}</td>
        <td>${qtyCell}</td>
        <td>${sockCell}</td>
        <td>${it.statsCount}</td>
      </tr>`;
    })
    .join("");

  $("#items").innerHTML =
    `<thead><tr>
      <th>#</th><th>代码</th><th>名称</th><th>品质</th><th>等级</th>
      <th>位置</th><th>占格</th><th>耐久</th><th>数量</th><th>孔</th><th>属性</th>
     </tr></thead><tbody>${rows}</tbody>`;

  // 点行选中（点输入框不算）
  document.querySelectorAll<HTMLTableRowElement>("#items tbody tr").forEach((tr) => {
    tr.addEventListener("click", (ev) => {
      const target = ev.target as HTMLElement;
      if (target.tagName === "INPUT") return;
      const idx = Number(tr.dataset.itemIndex);
      void selectItem(idx);
    });
  });
  $("#items-section").classList.remove("hidden");
  renderContainerView();
}

/** 渲染容器网格视图（背包/方块/箱子/腰带/装备位） */
function renderContainerView() {
  if (!currentLayout) return;
  const host = $("#inventory");
  host.innerHTML = renderInventory(currentLayout, items, selectedItem);
  bindGridDrag(
    host,
    currentLayout,
    items,
    // 左键只做选中（属性窗由右键菜单里的"编辑"打开）
    (index) => void selectItem(index),
    async (index, page, col, row) => {
      try {
        const n = await call<number>("apply_item_edits", {
          edits: [{ kind: "relocate", index, page, col, row }],
        });
        toast(`已移动（${n} 条修改）。点「保存」写入文件。`);
        await loadAll();
        // 只刷新列表，不打开属性窗（拖完就弹窗很烦）
      } catch (e) {
        showError(e, "移动被拒绝");
      }
    },
    (target, x, y) => showItemMenu(target, x, y),
  );
  $("#inventory-section").classList.remove("hidden");
  bindItemMenu();
}

/** 存储页中文缩写 */
function shortPage(page: string): string {
  const map: Record<string, string> = {
    Inventory: "背包",
    Stash: "箱子",
    Cube: "方块",
    Belt: "腰带",
    Equip: "装备",
    Null: "—",
    Trade: "交易",
  };
  return map[page] ?? page;
}

/** 装备位中文缩写 */
function shortSlot(slot: string): string {
  const map: Record<string, string> = {
    Head: "头",
    Neck: "颈",
    Torso: "身",
    RightArm: "右臂",
    LeftArm: "左臂",
    RightRing: "右戒",
    LeftRing: "左戒",
    Belt: "腰",
    Feet: "脚",
    Gloves: "手",
    RightHand: "右手",
    LeftHand: "左手",
  };
  return map[slot] ?? slot;
}

/** 收集物品表里的改动（耐久/数量/孔数） */
function collectItemEdits(): ItemEdit[] {
  const edits: ItemEdit[] = [];
  document.querySelectorAll<HTMLInputElement>("input[data-dur-index]").forEach((el) => {
    const index = Number(el.dataset.durIndex);
    const value = Number(el.value);
    const old = items.find((i) => i.index === index)?.durability ?? 0;
    if (value !== old) edits.push({ kind: "setDurability", index, value });
  });
  document.querySelectorAll<HTMLInputElement>("input[data-qty-index]").forEach((el) => {
    const index = Number(el.dataset.qtyIndex);
    const value = Number(el.value);
    const old = items.find((i) => i.index === index)?.quantity ?? 0;
    if (value !== old) edits.push({ kind: "setQuantity", index, value });
  });
  document.querySelectorAll<HTMLInputElement>("input[data-sock-index]").forEach((el) => {
    const index = Number(el.dataset.sockIndex);
    const count = Number(el.value);
    const old = items.find((i) => i.index === index)?.socketCount ?? 0;
    if (count !== old) edits.push({ kind: "setSockets", index, count });
  });
  return edits;
}

/**
 * 选中一件物品：打开**属性弹窗**。
 *
 * `target` 指明改哪个列表：佣兵段与主物品列表的**序号体系不同**，
 * 不传目标就会按同一个序号去改另一个列表（改错物品）。
 */
async function selectItem(index: number, target: ItemTarget = "inventory") {
  selectedItem = index;
  selectedTarget = target;
  renderItems(items);
  try {
    const detail = await call<ItemDetailDto>("item_detail", { index, target });
    renderItemDetail(detail);
    openItemModal();
  } catch (e) {
    showError(e, "读取物品明细失败");
  }
}

/** 打开物品属性弹窗 */
function openItemModal() {
  $("#item-modal").classList.remove("hidden");
  $("#detail-result").textContent = "";
}

/** 关闭物品属性弹窗 */
function closeItemModal() {
  $("#item-modal").classList.add("hidden");
}

/** 当前弹窗里展示的属性（收集改动时用来判断"有没有真的变"）。 */
let detailStats: ItemStatDto[] = [];

/** 渲染明细面板 */
function renderItemDetail(detail: ItemDetailDto) {
  const it = detail.item;
  detailStats = detail.stats;
  $("#detail-title").textContent = `${it.name}（${it.code} #${it.index}）`;
  $("#detail-summary").textContent =
    `品质 ${it.quality}｜等级 ${it.level}｜位置 ${shortPage(it.storePage)}` +
    (it.bodyLocation !== "None" ? `·${shortSlot(it.bodyLocation)}` : ` (${it.invX},${it.invY})`) +
    `｜占格 ${it.width}×${it.height}｜已镶嵌 ${it.socketedCount}` +
    (detail.runewordId !== null ? `｜符文之语 ${detail.runewordId}` : "");

  // 基础字段：没有该概念的禁用掉
  const setNum = (sel: string, value: number | null, enabled: boolean, max?: number) => {
    const el = $(sel) as HTMLInputElement;
    el.value = value === null ? "" : String(value);
    el.disabled = !enabled;
    if (max !== undefined) el.max = String(max);
  };
  setNum("#d-sockets", it.socketCount, true, 7);
  setNum("#d-durability", it.durability, it.maxDurability !== null && it.maxDurability > 0, it.maxDurability ?? undefined);
  setNum("#d-quantity", it.quantity, it.quantity !== null, 511);
  setNum("#d-defense", it.defense, it.defense !== null, 65535);

  $("#stat-count").textContent = String(detail.stats.length);
  // 行的渲染在 `statrow.ts` 里（纯函数、无 DOM 依赖）。
  // 抽出去是为了让 `tools/render-check.mjs` 能在**不开窗口**时断言
  // "每条属性都带上了值"，这条路径出过很难查的"整列空白"。
  const rows = detail.stats.map((s) => renderStatRow(s)).join("");
  // 内部名列已去掉：内部名是排查用的，对用户没意义，占地方还挤窄了别的列。
  // 两个值列统一叫 **值1 / 值2**（跟游戏里一样，用户只看数值，
  // 含义由"说明"列讲清楚），不出现"层"这种内部术语。
  $("#item-stats").innerHTML = STAT_TABLE_HEAD + `<tbody>${rows}</tbody>`;

  // 值再用显式赋值兜一道。
  //
  // `renderStatRow` 已经把值写进 `value` 属性（浏览器解析 HTML 时同步建立，
  // 不依赖脚本执行）；这里再赋一次，兜住属性解析异常的情况。
  // 两条路径任一条可用，值就不会空白。
  //
  // 查询**限定在 `#item-stats` 内**：全文档查询会把别处同名的元素也捞进来
  // （属性行的收集曾经因此把"新增属性"那一行的值2 当成已有属性提交）。
  $("#item-stats")
    .querySelectorAll<HTMLInputElement>("input[data-stat-id]")
    .forEach((el) => {
      const stat = detail.stats.find(
        (s) => s.id === Number(el.dataset.statId) && s.layer === Number(el.dataset.statLayer),
      );
      if (stat) el.value = statValueText(stat);
    });

  // 改了就生效：值1 失焦 / 值2 变更时只提交这一行
  bindStatRowAutoApply();
  // 基础字段（孔数/耐久/数量/防御）同样失焦即生效（不再有「应用修改」按钮）
  bindDetailFieldAutoApply();

  // 删除属性：**立即生效**（和值、值2 一致，不再需要另点「应用修改」）
  document.querySelectorAll<HTMLButtonElement>("button[data-del-stat]").forEach((btn) => {
    btn.addEventListener("click", () => {
      void removeStatNow(Number(btn.dataset.delStat), Number(btn.dataset.delLayer));
    });
  });
}

/** 立即删除一条属性（按 `stat + layer` 精确定位）。 */
async function removeStatNow(stat: number, layer: number) {
  if (selectedItem === null) return;
  try {
    await call<number>("apply_item_edits", {
      // **总是带上层**：后端对"不带层"的语义是"删掉该 ID 的全部记录"，
      // 于是删一条 `item_addclassskills`（层 0）会把层 1 那条也一起删掉
      // （用户实际遇到过）。
      edits: [{ kind: "removeStat", index: selectedItem, stat, layer }],
      target: selectedTarget,
    });
    toast(
      `已删除：属性 ${stat}${layer === 0 ? "" : `（值2 = ${layer}）`}。点「保存」才写入文件。`,
    );
    $("#detail-result").textContent = "";
    await reloadItemDetail();
  } catch (e) {
    showError(e, "删除属性被拒绝");
  }
}

/**
 * 「添加」属性：**立即生效**，不再需要另点「应用修改」。
 *
 * 这里曾经是把 `setStat` 挂到 `pendingDetailEdits`、等用户再点「应用修改」。
 * 结果是**同一个属性被提交两次**（挂起的那条 + 界面上输入框的那条），
 * 后端按顺序先处理挂起的旧值 → 报"超出范围" → 整批回滚，
 * 于是用户看到"我明明改成 107 了还说我超范围"，而且什么都没生效。
 * 现在直接把当前界面上的全部改动一起提交，列表立刻刷新，所见即所得。
 */
async function addStatNow() {
  if (selectedItem === null) {
    toast("先在物品表里点一件物品", "error");
    return;
  }
  const id = Number(($("#add-stat-select") as HTMLSelectElement).value);
  const rawValue = ($("#add-stat-value") as HTMLInputElement).value;
  if (rawValue === "") {
    toast("请填写属性值", "error");
    return;
  }
  const entry = statCatalog.find((s) => s.id === id);
  const name = entry?.name ?? `属性#${id}`;
  const value = Number(rawValue);
  // 值2（参数）：带参数位的属性才有。含义见 renderAddStatValue2。
  const layer = readAddStatValue2(id);

  // 先按该属性的可表示范围自查，给出**具体到这条属性**的提示
  const problem = statValueProblem(id, value, $("#add-stat-value") as HTMLInputElement);
  if (problem) {
    $("#detail-result").innerHTML = `<div class="problem">${escapeHtml(problem)}</div>`;
    return;
  }

  try {
    // 直接把这条属性应用到物品上（后端会校验并读回比对）
    await call<number>("apply_item_edits", {
      edits: [{ kind: "addStat", index: selectedItem, stat: id, value, layer }],
      target: selectedTarget,
    });
    toast(`已添加：${name} = ${value}。点「保存」才写入文件。`);
    // 重新读明细 → 新属性立刻出现在上面的列表里
    await reloadItemDetail();
  } catch (e) {
    showError(e, `添加「${name}」被拒绝`);
    $("#detail-result").textContent = String((e as { message?: string })?.message ?? e);
  }
}

/**
 * 选中的属性要在他**填值之前**就显示最大/最小值，并把输入框限制住。
 *
 * 之前只在提交后才报"超出范围"，用户等于盲填。
 */
function showAddStatHint() {
  const sel = $("#add-stat-select") as HTMLSelectElement;
  const input = $("#add-stat-value") as HTMLInputElement;
  const hint = $("#add-stat-hint");
  const id = Number(sel.value);
  const entry = statCatalog.find((s) => s.id === id);
  if (!entry) {
    hint.textContent = "";
    $("#add-stat-value2-slot").innerHTML = "";
    return;
  }
  // 不同属性的"值2"形态不同（下拉框或数字框），选中就重建
  renderAddStatValue2(id);

  const parts: string[] = [];
  if (entry.minValue !== null && entry.maxValue !== null) {
    parts.push(`有效范围 ${entry.minValue}–${entry.maxValue}`);
  }
  if (entry.displayStep > 1) parts.push(`取值须是 ${entry.displayStep} 的倍数`);
  if (entry.hasParamBits) parts.push("有值2（第二个可改的数）");
  if (!entry.userVisible) parts.push("内部字段（改了可能不生效）");
  hint.textContent = parts.join("｜");

  // 范围与步长写到 data-* 上，由 statValueProblem() 统一校验
  // （输入框是 type=text，没有原生 min/max/step）
  input.dataset.min = entry.minValue === null ? "" : String(entry.minValue);
  input.dataset.max = entry.maxValue === null ? "" : String(entry.maxValue);
  input.dataset.step = String(entry.displayStep);
  // 默认值取范围内的值，免得一打开就是非法的
  if (entry.minValue !== null && Number(input.value) < entry.minValue) {
    input.value = String(entry.minValue);
  }
}

/**
 * 值2（参数）的可选值；空表示该属性没有值2。
 *
 * `paramOptions` 由后端给出（如 `item_addclassskills` 列出各职业）。
 */
function paramOptionsOf(id: number): LayerOption[] {
  return statCatalog.find((s) => s.id === id)?.paramOptions ?? [];
}

/**
 * 根据选中的属性重建"值2"控件。
 *
 * - 属性没有值2 → 不显示控件（普通属性只有一个数）；
 * - 有已知可选值 → 下拉框（直接选"法师"）；
 * - 没有已知可选值 → 数字输入框（自己填参数值）。
 */
function renderAddStatValue2(id: number) {
  const slot = $("#add-stat-value2-slot");
  const entry = statCatalog.find((s) => s.id === id);
  if (!entry?.hasParamBits) {
    slot.innerHTML = "";
    return;
  }
  // 与属性表里的"值2"用同一个控件：技能类属性会是**两级选单**（职业 → 技能）
  slot.innerHTML = renderLayerControl(
    id,
    0,
    paramOptionsOf(id),
    `id="add-stat-value2-sel" class="cell-select" title="值2（参数）"`,
  );
  // 注意：新增属性的值2 **不用** `data-stat-layer-sel`，
  // 那个属性名属于属性表里的行；共用会被 collectDetailEdits 误当成行改动。
}

/** 读"值2"控件当前的值；没有控件时返回 `undefined`（表示不带参数）。 */
function readAddStatValue2(id: number) {
  const entry = statCatalog.find((s) => s.id === id);
  if (!entry?.hasParamBits) return undefined;
  const el = document.querySelector<HTMLInputElement | HTMLSelectElement>(
    "#add-stat-value2-sel, #add-stat-value2",
  );
  if (!el) return undefined;
  const n = Number(el.value);
  return Number.isNaN(n) ? undefined : n;
}

/**
 * 按搜索词重新填满"新增属性"的下拉框。
 *
 * 属性有 300 多条，滚动找太费劲。搜索同时匹配**中文名与内部名**
 * （习惯英文的人也能按 `item_aura` 找）。
 */
function fillAddStatOptions() {
  const sel = $("#add-stat-select") as HTMLSelectElement;
  const keyword = ($("#add-stat-search") as HTMLInputElement).value.trim().toLowerCase();
  const matched = statCatalog.filter(
    (s) =>
      keyword === "" ||
      s.name.toLowerCase().includes(keyword) ||
      s.internal.toLowerCase().includes(keyword),
  );
  // 尽量保留当前选择；它被筛掉时退回第一项
  const previous = sel.value;
  sel.innerHTML = matched
    .map(
      (s) =>
        `<option value="${s.id}">${escapeHtml(s.name)}（${escapeHtml(s.internal)}）</option>`,
    )
    .join("");
  if (matched.some((s) => String(s.id) === previous)) sel.value = previous;
  showAddStatHint();
}

/** 重新读当前物品的明细并刷新弹窗（添加属性后立刻反映到列表里）。 */
async function reloadItemDetail() {
  const keep = selectedItem;
  if (keep === null) return;
  await loadAll();
  const detail = await call<ItemDetailDto>("item_detail", {
    index: keep,
    target: selectedTarget,
  });
  renderItemDetail(detail);
  openItemModal();
}


/**
 * 改了就生效：只把**这一行**的改动提交到后端，成功后再读回刷新。
 *
 * 为什么只提交一行而不是整表：整表提交会把别的输入框里"看起来变了的"
 * 值一起写进去，用户只想改一个数却连带改了别的（之前踩过）。
 */
async function applyRowNow(stat: number, rowLayer: number) {
  if (selectedItem === null) return;
  const el = $(`#item-stats input[data-stat-id="${stat}"][data-stat-layer="${rowLayer}"]`) as
    | HTMLInputElement
    | null;
  if (!el) return;
  const original = detailStats.find((s) => s.id === stat && s.layer === rowLayer);
  if (!original) return;

  const value = Number(el.value);
  const problem = statValueProblem(stat, value, el);
  if (problem) {
    $("#detail-result").innerHTML = `<div class="problem">${escapeHtml(problem)}</div>`;
    return;
  }

  const sel = $(`#item-stats [data-stat-layer-sel="${stat}"][data-stat-layer-cur="${rowLayer}"]`) as
    | HTMLInputElement
    | HTMLSelectElement
    | null;
  const newLayer = sel === null ? undefined : Number(sel.value);
  const layerChanged = newLayer !== undefined && !Number.isNaN(newLayer) && newLayer !== rowLayer;
  const valueChanged = value !== original.value;
  if (!layerChanged && !valueChanged) return;

  const edit: ItemEdit = layerChanged
    ? { kind: "setStat", index: selectedItem, stat, value, layer: newLayer }
    : { kind: "setStat", index: selectedItem, stat, value, layer: rowLayer };

  const shown = rowLayer === 0 ? `属性 ${stat}` : `属性 ${stat}（值2 = ${rowLayer}）`;
  try {
    await call<number>("apply_item_edits", {
      edits: [edit],
      target: selectedTarget,
    });
    toast(`已改：${shown}。点「保存」才写入文件。`);
    $("#detail-result").textContent = "";
    await reloadItemDetail();
  } catch (e) {
    showError(e, `修改「${shown}」被拒绝`);
  }
}

/**
 * 把属性表里每一行的输入框与选择框接上"改了就生效"。
 *
 * 触发时机用 `blur` 而不是 `input`：用户打字过程中每敲一个字符就提交
 * 会写出一串中间值（例如把 7 改成 31 会先写 3），既慢又容易写坏存档。
 */
function bindStatRowAutoApply() {
  $("#item-stats")
    .querySelectorAll<HTMLInputElement>("input[data-stat-id]")
    .forEach((el) => {
      const stat = Number(el.dataset.statId);
      const layer = Number(el.dataset.statLayer);
      // 只绑 `blur`：**不注册任何键盘快捷键**（用户要求）。
      // 失焦即提交，点别处 / 按 Tab 走开都会生效。
      el.addEventListener("blur", () => void applyRowNow(stat, layer));
    });
  $("#item-stats")
    .querySelectorAll<HTMLElement>("[data-stat-layer-sel]")
    .forEach((el) => {
      const stat = Number(el.dataset.statLayerSel);
      const layer = Number(el.dataset.statLayerCur);
      // 值2 换了选项 = 换了一条属性（层是属性语义的一部分）。
      // 这里必须发"换层"，不能发"改值"：改值找不到该层会直接被拒
      // （用户实测："该物品没有属性 ID 97（层 58）"）。
      el.addEventListener("change", () => {
        const to = Number((el as HTMLSelectElement).value);
        if (!Number.isFinite(to) || to === layer) return;
        void relayerStatNow(stat, layer, to);
      });
    });
  // 值2 直接输入的场合（没有可选列表）：失焦时同样按整行提交
  $("#item-stats")
    .querySelectorAll<HTMLInputElement>("input[data-stat-layer-sel]")
    .forEach((el) => {
      const stat = Number(el.dataset.statLayerSel);
      const layer = Number(el.dataset.statLayerCur);
      el.addEventListener("blur", () => void applyRowNow(stat, layer));
    });
}

/**
 * 物品基础字段（孔数/耐久/数量/防御）也做成**失焦即生效**。
 *
 * 「应用修改」按钮已去掉，所以这些字段必须自己提交，否则改了不会保存。
 * 触发用 `blur`（不是 `input`）：打字过程中每敲一下都提交会写出一串中间值。
 */
function bindDetailFieldAutoApply() {
  const fields: {
    sel: string;
    build: (index: number, value: number) => ItemEdit;
    /** 取当前物品上的原值，用来判断有没有真的改 */
    original: (item: ItemDto) => number | null;
  }[] = [
    {
      sel: "#d-sockets",
      build: (index, count) => ({ kind: "setSockets", index, count }),
      original: (item) => item.socketCount,
    },
    {
      sel: "#d-durability",
      build: (index, value) => ({ kind: "setDurability", index, value }),
      original: (item) => item.durability,
    },
    {
      sel: "#d-quantity",
      build: (index, value) => ({ kind: "setQuantity", index, value }),
      original: (item) => item.quantity,
    },
    {
      sel: "#d-defense",
      build: (index, value) => ({ kind: "setDefense", index, value }),
      original: (item) => item.defense,
    },
  ];
  for (const field of fields) {
    const el = $(field.sel) as HTMLInputElement;
    el.addEventListener("blur", () => {
      const index = selectedItem;
      if (index === null || el.disabled || el.value === "") return;
      const item = items.find((i) => i.index === index);
      if (!item) return;
      const value = Number(el.value);
      if (Number.isNaN(value) || value === field.original(item)) return;
      void applyBaseFieldNow(field.build(index, value));
    });
  }
}

/** 提交一个基础字段的改动，成功后重新读明细刷新界面。 */
async function applyBaseFieldNow(edit: ItemEdit) {
  try {
    await call<number>("apply_item_edits", { edits: [edit], target: selectedTarget });
    toast("已改。点「保存」才写入文件。");
    $("#detail-result").textContent = "";
  } catch (e) {
    showError(e, "修改被拒绝");
  }
  // 成功与失败都重新读一遍：成功是让用户看到规范化后的结果，
  // 失败是把界面上的值退回存档里的值（避免显示与存档不一致）。
  await reloadItemDetail();
}

/**
 * 检查一个属性值是否可表示；不可表示时返回可读原因。
 *
 * 规则与后端一致（存档写入时会右移 `ValShift` 位、值右移后为 0 会被整个跳过），
 * 这里只是提前告诉用户，避免提交后才被拒。
 */
function statValueProblem(id: number, value: number, el: HTMLInputElement): string | null {
  if (Number.isNaN(value)) return `属性 ID ${id} 的值不是数字`;
  // 范围与步长写在 data-* 上（输入框是 type=text，没有原生 min/max/step）
  const min = el.dataset.min === "" || el.dataset.min === undefined ? null : Number(el.dataset.min);
  const max = el.dataset.max === "" || el.dataset.max === undefined ? null : Number(el.dataset.max);
  const step = el.dataset.step === undefined || el.dataset.step === "" ? 1 : Number(el.dataset.step);
  if (step > 1 && value % step !== 0) {
    return `属性 ID ${id} 的值 ${value} 存不进去：必须是 ${step} 的倍数（否则会被截断）`;
  }
  if (step > 1 && value <= 0) {
    return `属性 ID ${id} 的值 ${value} 会让这条属性在写盘后消失（该属性按 ${step} 的精度存储）`;
  }
  if (min !== null && value < min) {
    return `属性 ID ${id} 的值 ${value} 小于可表示的最小值 ${min}`;
  }
  if (max !== null && value > max) {
    return `属性 ID ${id} 的值 ${value} 超过可表示的最大值 ${max}`;
  }
  return null;
}

/** 载入属性目录并填充下拉 */
async function loadStatCatalog() {
  try {
    statCatalog = await call<StatCatalogEntry[]>("stat_catalog", { all: false });
    // 下拉框的内容由 fillAddStatOptions() 统一填（它负责搜索过滤），
    // 这里不要各写一份，否则搜索一改就漏。
    fillAddStatOptions();
  } catch {
    /* 老后端没有这个命令 */
  }
}

async function loadAll() {
  // 真实内容开始渲染，先去掉"未打开存档"的提示
  clearEmptyState();
  // 数据集那行由 refreshDataset() 整行重建：不能在这里用 `+=` 追加，
  // 否则每调用一次 loadAll() 就多贴一段（刷新/保存后都会调用）。
  bundleInfo = await loadBundleInfo();
  await refreshDataset();
  try {
    const st = await call<ItemStats>("item_stats");
    $("#items-stats").textContent =
      `共 ${st.total} 件｜已装备 ${st.equipped}｜含镶嵌物 ${st.withSockets}｜` +
      st.byQuality.map(([k, v]) => `${k} ${v}`).join("｜");
  } catch {
    /* 忽略 */
  }
  const char = await call<CharacterDto>("character");
  fillCharacter(char);
  skills = await call<SkillDto[]>("skills");
  // 技能树分组：后端给不出（老后端 / 未知职业）时留 null，界面退回平铺列表
  try {
    skillsView = await call<SkillsViewDto>("skills_view");
  } catch {
    skillsView = null;
  }
  renderSkills();
  // 小站名要在渲染传送点之前拿到，否则只能显示编号
  if (waypointNames.length === 0) {
    try {
      waypointNames = await call<string[]>("waypoint_names");
    } catch {
      /* 老后端没有这个命令 */
    }
  }
  renderWaypoints(await call<WaypointsDto[]>("waypoints"));
  await loadQuests();
  try {
    mercItems = await call<ItemDetailDto[]>("mercenary_items");
  } catch {
    mercItems = [];
  }
  items = await call<ItemDto[]>("item_details");
  renderItems(items);
  await loadStatCatalog();
  // 先拿布局（容器网格尺寸来源，它会顺带渲染一次容器视图），再载图标并重渲染；
  // 反过来会让第一次渲染缺图标。
  await refreshLayout();
  await loadIcons();
  renderMerc();
  $("#tabs").classList.remove("hidden");
  setDirty(await call<boolean>("is_dirty"));
}

/** 任务视图缓存 */
let quests: QuestDifficultyDto[] = [];
/** 当前显示的任务难度下标 */
let questDifficulty = 0;

/** 按难度渲染任务列表（用真实的位判定状态） */
function renderQuests() {
  const data = quests[questDifficulty];
  if (!data) {
    $("#quests").innerHTML = `<div class="muted">没有任务数据。</div>`;
    return;
  }
  const stateLabel: Record<QuestState, string> = {
    notStarted: "未开始",
    inProgress: "进行中",
    completed: "已完成",
  };
  $("#quests").innerHTML =
    `<div class="quest-diff">` +
    quests
      .map(
        (q, i) =>
          `<button class="mini ${i === questDifficulty ? "active" : ""}" data-quest-diff="${i}">${escapeHtml(q.difficulty)}</button>`,
      )
      .join("") +
    `</div>` +
    data.acts
      .map((slots, act) => {
        // 只显示真任务槽，非任务槽（序章/收尾/对话）不列出来，避免误导
        const real = slots.filter((s) => s.editable);
        if (real.length === 0) return "";
        return `<div class="quest-act">
          <div class="quest-act-head">第 ${act + 1} 幕</div>
          <div class="quest-list">
            ${real
              .map(
                (q) => `<label class="quest ${q.state}">
                  <input type="checkbox" data-quest-act="${q.act}" data-quest-slot="${q.slot}"
                    ${q.state === "completed" ? "checked" : ""} />
                  <span class="quest-name">${escapeHtml(q.name)}</span>
                  <span class="quest-state">${stateLabel[q.state]}</span>
                </label>`,
              )
              .join("")}
          </div>
        </div>`;
      })
      .join("");

  // 难度切换
  document.querySelectorAll<HTMLButtonElement>("[data-quest-diff]").forEach((btn) => {
    btn.addEventListener("click", () => {
      questDifficulty = Number(btn.dataset.questDiff);
      renderQuests();
    });
  });
  // 勾选：立即应用（不攒到最后，任务之间没有依赖）
  document.querySelectorAll<HTMLInputElement>("input[data-quest-act]").forEach((el) => {
    el.addEventListener("change", async () => {
      const act = Number(el.dataset.questAct);
      const slot = Number(el.dataset.questSlot);
      const edit = el.checked
        ? { kind: "completeQuest", difficulty: questDifficulty, act, slot }
        : { kind: "clearQuest", difficulty: questDifficulty, act, slot };
      try {
        await call<number>("apply_quest_edits", { edits: [edit] });
        toast(`已更新「${quests[questDifficulty].acts[act][slot].name}」。点「保存」才会写入文件。`);
        await loadQuests();
      } catch (e) {
        showError(e, "任务修改被拒绝");
        await loadQuests();
      }
    });
  });
  $("#quests-section").classList.remove("hidden");
}

/** 重新读取任务视图 */
async function loadQuests() {
  quests = await call<QuestDifficultyDto[]>("quests");
  renderQuests();
}

/** 把当前难度的全部任务标记完成 */
async function completeAllQuests() {
  try {
    const n = await call<number>("apply_quest_edits", {
      edits: [{ kind: "completeAllQuests", difficulty: questDifficulty }],
    });
    toast(`已把「${quests[questDifficulty]?.difficulty ?? ""}」全部任务标记完成（${n} 条）。点「保存」才会写入文件。`);
    await loadQuests();
  } catch (e) {
    showError(e, "任务修改被拒绝");
  }
}

/**
 * 佣兵：用**与角色装备位完全相同**的布局渲染。
 *
 * 佣兵段与主物品列表分开存储，序号互不相干，所以点击时必须带 `mercenary` 目标，
 * 否则会按同一个序号改到主列表的物品。
 */
let mercItems: ItemDetailDto[] = [];
function renderMerc() {
  const host = $("#merc");
  if (mercItems.length === 0) {
    host.innerHTML = `<div class="muted">这件存档的佣兵段里没有物品。</div>`;
    $("#merc-section").classList.remove("hidden");
    return;
  }
  // 佣兵物品带 bodyLocation，直接复用装备页布局（尺寸与角色装备位一致）
  host.innerHTML = renderMercDoll(
    mercItems.map((d) => d.item),
    selectedTarget === "mercenary" ? selectedItem : null,
  );
  host.querySelectorAll<HTMLElement>(".blk[data-item]").forEach((el) => {
    el.addEventListener("click", () => void selectItem(Number(el.dataset.item), "mercenary"));
  });
  $("#merc-section").classList.remove("hidden");
}



async function applyEdits(edits: Edit[], label: string) {
  if (edits.length === 0) {
    toast("没有需要应用的修改", "error");
    return;
  }
  try {
    const n = await call<number>("apply_edits", { edits });
    toast(`已应用 ${n} 条修改（${label}）。点「保存」才会写入文件。`);
    await loadAll();
  } catch (e) {
    showError(e, "修改被拒绝");
  }
}


/**
 * 读取并展示布局设置（在设置弹窗里）。
 *
 * 设置是全局的，所以**没打开存档时也要能用**：那种情况下拿不到"从存档推断"
 * 的提示（后端需要会话），只显示当前设置值与说明。
 */
async function refreshLayout() {
  try {
    const settings = await call<LayoutSettings>("layout");
    ($("#l-inv-cols") as HTMLInputElement).value = String(settings.inventoryCols);
    ($("#l-inv-rows") as HTMLInputElement).value = String(settings.inventoryRows);
    ($("#l-cube-cols") as HTMLInputElement).value = String(settings.cubeCols);
    ($("#l-cube-rows") as HTMLInputElement).value = String(settings.cubeRows);
    ($("#l-stash-cols") as HTMLInputElement).value = String(settings.stashCols);
    ($("#l-stash-rows") as HTMLInputElement).value = String(settings.stashRows);
    ($("#l-merc") as HTMLInputElement).checked = settings.mercFullEquipment;
    ($("#l-books") as HTMLInputElement).checked = settings.booksSingleCell;
    ($("#l-cube-one") as HTMLInputElement).checked = settings.cubeSingleCell;
    // 记住布局并刷新容器网格：网格尺寸（背包/方块/箱子行列数）全部来自它。
    // 漏了赋值会让 renderContainerView 恒早退，容器视图根本不渲染。
    currentLayout = settings;
    renderContainerView();

    const confirmNote = settings.confirmedByUser
      ? "（这些设置已保存，下次启动自动带上）"
      : "（当前是默认值，确认后会记住）";
    // 存档推断只在有会话时可用
    let hint = "";
    try {
      const [hints, questions] = await Promise.all([
        call<LayoutHints>("layout_hints"),
        call<string[]>("layout_questions"),
      ]);
      hint =
        `推断：背包最大 (${hints.inventoryMax?.[0] ?? "-"},${hints.inventoryMax?.[1] ?? "-"})｜` +
        `合成箱最大 (${hints.cubeMax?.[0] ?? "-"},${hints.cubeMax?.[1] ?? "-"})｜` +
        `箱子坐标最大 (${hints.stashMax?.[0] ?? "-"},${hints.stashMax?.[1] ?? "-"})｜` +
        `佣兵段 ${hints.mercSection ? `${hints.mercItems} 件` : "无"}`;
      const bullets = questions
        .map((q) => `<div class="question">需要确认：${q}</div>`)
        .join("");
      $("#layout-questions").innerHTML =
        `<div class="muted">${hint}${confirmNote}</div>${bullets}`;
    } catch {
      // 没打开存档：只给说明，不给"从存档推断"
      $("#layout-questions").innerHTML =
        `<div class="muted">当前没有打开存档，所以没有"从坐标推断"的提示。` +
        `这些设置是全局的，改完保存即生效。${confirmNote}</div>`;
    }
  } catch (e) {
    showError(e, "读取布局设置失败");
  }
}

/** 打开设置弹窗（设置与存档无关，所以随时可开）。 */
async function openSettingsModal() {
  $("#settings-modal").classList.remove("hidden");
  try {
    const [path, note] = await call<[string, string | null]>("settings_status");
    $("#settings-path").textContent = note ?? `设置文件：${path}`;
  } catch {
    $("#settings-path").textContent = "";
  }
  $("#bounds-result").textContent = "";
  await refreshLayout();
}

/** 关闭设置弹窗。 */
function closeSettingsModal() {
  $("#settings-modal").classList.add("hidden");
}

async function applyLayout() {
  const layout: LayoutSettings = {
    inventoryCols: Number(($("#l-inv-cols") as HTMLInputElement).value),
    inventoryRows: Number(($("#l-inv-rows") as HTMLInputElement).value),
    cubeCols: Number(($("#l-cube-cols") as HTMLInputElement).value),
    cubeRows: Number(($("#l-cube-rows") as HTMLInputElement).value),
    stashCols: Number(($("#l-stash-cols") as HTMLInputElement).value),
    stashRows: Number(($("#l-stash-rows") as HTMLInputElement).value),
    mercFullEquipment: ($("#l-merc") as HTMLInputElement).checked,
    booksSingleCell: ($("#l-books") as HTMLInputElement).checked,
    cubeSingleCell: ($("#l-cube-one") as HTMLInputElement).checked,
    confirmedByUser: true,
  };
  try {
    await call<void>("set_layout", { layout });
    await refreshLayout();
    // 如实报告存到哪了：写盘失败时 set_layout 会抛错，走下面的分支
    const [path, note] = await call<[string, string | null]>("settings_status");
    $("#bounds-result").innerHTML =
      `<span class="ok">布局已应用</span>｜${escapeHtml(note ?? `设置文件：${path}`)}`;
    toast("布局已确认。之后保存时按这套尺寸做越界校验。");
  } catch (e) {
    // 写盘失败：设置这次仍然生效，但下次启动会丢，必须说清
    showError(e, "布局已应用，但**没能保存到磁盘**（下次启动会恢复默认）");
    await refreshLayout();
  }
}

async function checkBounds() {
  try {
    const issues = await call<BoundsIssue[]>("validate_bounds");
    $("#bounds-result").innerHTML =
      issues.length === 0
        ? `<span class="ok">越界校验通过（按当前布局设置）</span>`
        : issues
            .map(
              (i) =>
                `<div class="question">[${i.index}] ${i.code} 在${i.page}的 (${i.at[0]},${i.at[1]}) 占 ${i.size[0]}×${i.size[1]}，超出容器 ${i.container[0]}×${i.container[1]}</div>`,
            )
            .join("");
    toast(issues.length === 0 ? "越界校验通过" : `发现 ${issues.length} 件超出容器`, issues.length === 0 ? "ok" : "error");
  } catch (e) {
    showError(e, "越界校验失败");
  }
}

/**
 * 切换页签。
 *
 * 面板 id 约定为 `tab-<key>`；物品栏/佣兵等面板的内容在 loadAll() 时就已填好，
 * 这里只切换可见性，不做重新加载（避免切页签反复请求后端）。
 */
function activateTab(key: string) {
  document.querySelectorAll<HTMLButtonElement>("#tabs .tab").forEach((btn) => {
    btn.classList.toggle("active", btn.dataset.tab === key);
  });
  document.querySelectorAll<HTMLElement>(".tab-panel").forEach((panel) => {
    panel.classList.toggle("hidden", panel.id !== `tab-${key}`);
  });
  // 切到物品栏时容器网格可能因为之前尺寸未知而没渲染，这里补一次
  if (key === "items") renderContainerView();
}
/**
 * 启动时**不**自动打开任何存档。
 *
 * 这是刻意的安全设计：自动带出一个可编辑的存档，万一用户没注意就改了值再保存，
 * 会直接写坏真实存档。启动后停在空状态，必须由用户显式选择文件。
 *
 * 这里只保留一次"后端是否已经有打开的存档"的检查：正常启动永远是 `None`，
 * 但保留它可以让将来加"命令行传入存档"这类功能时不用再改前端。
 */
/**
 * 启动自检：后端若**已经**打开过存档就直接进入编辑界面。
 *
 * 正常启动永远是"没有"——应用刻意不自动打开存档（自动带出可编辑存档有改错的风险）。
 * 保留这个检查是为了将来加"命令行传入存档"之类的入口时不用再改前端。
 */
async function checkAlreadyOpen() {
  try {
    const info = await call<SaveInfo | null>("current_save");
    if (!info) return;
    renderSaveInfo(info);
    if (info.kind === "stash") {
      // 共享箱没有角色/技能/任务/佣兵，这几块藏掉
      for (const sel of ["#char-section", "#skills-section", "#quests-section", "#merc-section"]) {
        $(sel).classList.add("hidden");
      }
    }
    await loadAll();
  } catch {
    /* 后端没有这个命令时忽略 */
  }
}

/**
 * 载入物品图标（游戏原图，随程序内嵌）。
 *
 * 一次全量取回并缓存在前端：图标很小（约 300 张），逐个请求反而更麻烦；
 * 后端不可用时静默跳过，网格会退化成只显示物品代码。
 */
let iconsLoaded = false;
async function loadIcons() {
  if (iconsLoaded) return;
  try {
    const icons = await call<[string, string][]>("item_icons");
    if (icons.length === 0) return;
    setItemIcons(icons);
    iconsLoaded = true;
    renderContainerView();
    renderMerc();
  } catch {
    /* 老后端没有这个命令 */
  }
}
/**
 * 还没打开存档时，给每个页签放一句提示。
 *
 * 为什么需要它：各区块默认是隐藏的（`hidden`），没打开存档时点页签就是一片空白，
 * 看起来像"坏了"。这里把"该做什么"直接写在页签里，不用猜。
 *
 * **只能"插"不能"覆盖"**：这些区块里本来就有子元素（角色表单的 `#f-level` 等），
 * 直接覆盖 `innerHTML` 会把它们删掉，之后 `loadAll()` 渲染时就报"缺少元素"。
 * 所以提示是额外插入一个 `[data-empty-hint]` 节点，渲染真实内容前再删掉它。
 */
function renderEmptyState() {
  const placeholders: [string, string][] = [
    // 角色页（首页）不再显示"这里显示角色属性…"这类占位文案（用户要求）
    ["#skills-section", "这里显示该角色的技能加点。"],
    ["#waypoints-section", "这里显示三个难度的小站开通情况。"],
    ["#quests-section", "这里显示各幕任务进度（未开始 / 进行中 / 已完成）。"],
    ["#inventory-section", "这里按容器网格显示物品栏（装备位 / 背包 / 方块 / 箱子 / 腰带）。"],
    ["#items-section", "这里显示物品清单。"],
    ["#merc-section", "这里显示佣兵装备。"],
  ];
  for (const [sel, text] of placeholders) {
    const el = $(sel);
    if (el.querySelector("[data-empty-hint]")) continue;
    const hint = document.createElement("p");
    hint.className = "empty-hint";
    hint.dataset.emptyHint = "1";
    hint.innerHTML =
      `${text}<br /><br />点左上角 <b>「打开存档…」</b> ` +
      `选择角色存档（<code>.d2s</code>）或共享储物箱（<code>.d2i</code>）。`;
    el.prepend(hint);
    el.classList.remove("hidden");
  }
}

/** 去掉"未打开存档"的提示（真实内容开始渲染时调用）。 */
function clearEmptyState() {
  document.querySelectorAll("[data-empty-hint]").forEach((el) => el.remove());
}


function bind() {
  // 明细面板：应用该物品的修改 / 添加属性
  $("#btn-add-stat").addEventListener("click", addStatNow);

  // 选中的属性要在**添加之前**就告诉他最大/最小值（否则填了才知道超范围）
  $("#add-stat-select").addEventListener("change", showAddStatHint);
  // 属性有 300 多条：搜索框实时筛选下拉列表（匹配中文名与内部名）
  $("#add-stat-search").addEventListener("input", fillAddStatOptions);
  $("#add-stat-value").addEventListener("input", showAddStatHint);
  // 属性目录是异步载入的，拿到后补一次提示
  window.setTimeout(() => {
    if (statCatalog.length > 0) showAddStatHint();
  }, 800);

  // 导入/导出物品
  $("#btn-export-item").addEventListener("click", async () => {
    if (selectedItem === null) {
      toast("先在物品表里点一件物品", "error");
      return;
    }
    try {
      const bp = await call<unknown>("export_item", { index: selectedItem });
      const dest = await saveDialog({
        defaultPath: `item-${selectedItem}.json`,
        filters: [{ name: "物品蓝图", extensions: ["json"] }],
      });
      if (!dest || typeof dest !== "string") return;
      await call<string>("write_blueprint", { path: dest, json: JSON.stringify(bp, null, 2) });
      toast(`已导出到 ${dest}`);
    } catch (e) {
      showError(e, "导出物品失败");
    }
  });

  $("#btn-import-item").addEventListener("click", async () => {
    try {
      const picked = await openDialog({
        multiple: false,
        filters: [{ name: "物品蓝图", extensions: ["json"] }],
      });
      if (!picked || typeof picked !== "string") return;
      const json = await call<string>("read_blueprint", { path: picked });
      const templateRaw = window.prompt(
        `蓝图：${picked}\n\n用存档里的哪件物品当模板？（填序号，回车确认）`,
        String(selectedItem ?? 0),
      );
      if (templateRaw === null) return;
      const template = Number(templateRaw);
      if (!Number.isInteger(template) || template < 0) {
        toast("模板序号必须是整数", "error");
        return;
      }
      const outcome = await call<{ index: number; statsReplaced: number }>("import_item", {
        blueprintJson: json,
        template,
      });
      toast(`已导入为 #${outcome.index}（覆盖 ${outcome.statsReplaced} 条属性）。点「保存」写入文件。`);
      await loadAll();
      await selectItem(outcome.index);
    } catch (e) {
      showError(e, "导入物品失败");
    }
  });

  // 设置弹窗：工具栏按钮打开，关闭按钮 / 遮罩 / Esc 关闭
  $("#btn-settings").addEventListener("click", () => void openSettingsModal());
  document.querySelectorAll("[data-close-settings]").forEach((el) => {
    el.addEventListener("click", closeSettingsModal);
  });
  // 物品属性弹窗：关闭按钮 / 遮罩点击 / Esc
  document.querySelectorAll("[data-close-modal]").forEach((el) => {
    el.addEventListener("click", closeItemModal);
  });

  $("#btn-quests-all").addEventListener("click", () => void completeAllQuests());

  // 值2 是两级选单时：切换第一级（分组/职业）就重建第二级的选项。
  // 只重建 DOM，不派发 change，所以不会误触发"应用修改"。
  document.addEventListener("change", (ev) => {
    const el = ev.target as HTMLElement | null;
    if (!el?.classList.contains("layer-group-sel")) return;
    const id = Number(el.dataset.layerFor ?? "0");
    const group = (el as HTMLSelectElement).value;
    const options = (id > 0 ? paramOptionsOf(id) : []).filter(
      (o) => (o.group ?? "") === group,
    );
    const pair = el.closest(".layer-pair");
    const skillSel = pair?.querySelector<HTMLSelectElement>(
      "select[data-stat-layer-sel], #add-stat-value2-sel",
    );
    if (!skillSel) return;
    skillSel.innerHTML = options
      .map((o) => `<option value="${o.layer}">${escapeHtml(o.label)}</option>`)
      .join("");
  });

  // 版本号 + 检查更新
  void setupVersionAndUpdate();

  // 弹窗打开时锁住背景滚动
  watchModalScrollLock();

  // 重命名弹窗
  $("#btn-rename-ok").addEventListener("click", () => void applyRename());
  $("#rename-input").addEventListener("input", updateRenamePreview);
  document.querySelectorAll("[data-close-rename]").forEach((el) => {
    el.addEventListener("click", () => $("#rename-modal").classList.add("hidden"));
  });

  // 导出整栏物品
  $("#btn-export-all-items").addEventListener("click", () => void exportAllItems());

  // 装备库弹窗（搜索 / 类型筛选 / 关闭）——点条目即导入，无需确认按钮
  $("#lib-search").addEventListener("input", renderLibraryItems);
  $("#lib-group").addEventListener("change", renderLibraryItems);
  document.querySelectorAll("[data-close-lib]").forEach((el) => {
    el.addEventListener("click", () => $("#lib-modal").classList.add("hidden"));
  });

  // 页签切换
  document.querySelectorAll<HTMLButtonElement>("#tabs .tab").forEach((btn) => {
    btn.addEventListener("click", () => activateTab(btn.dataset.tab ?? "char"));
  });

  // **不注册任何键盘快捷键**（用户要求）：
  // 数字键切页签、Esc 关弹窗都去掉了，避免和别的程序抢键
  // （之前那个"取出 key 转数字"的写法还会让 F1 之类的功能键也走一遍处理）。

  // 还没打开存档前，每个页签先显示"该做什么"，避免点进去看到一片空白
  renderEmptyState();


  void checkAlreadyOpen();

  // 开发/自测用：`VITE_INITIAL_TAB=quests` 时启动就切到该页签，方便截图核对。
  // 正常构建里该值是 undefined，等价于不生效。
  const initialTab = (import.meta as { env?: Record<string, string> }).env?.VITE_INITIAL_TAB;
  if (initialTab) activateTab(initialTab);

  $("#btn-open").addEventListener("click", async () => {
    try {
      // 从上次打开的目录开始，免得每次都要重新翻到存档目录。
      // 用目录而不是文件名：对话框插件对各平台的 defaultPath 处理不一致，
      // 给目录最稳（后端也保证只在目录还存在时才给）。
      const startDir = await call<string | null>("default_open_dir").catch(() => null);
      const picked = await openDialog({
        multiple: false,
        defaultPath: startDir ?? undefined,
        filters: [{ name: "D2R 存档", extensions: ["d2s", "d2i"] }],
      });
      if (!picked || typeof picked !== "string") return;
      const info = await call<SaveInfo>("open_save", { path: picked });
      // 记下路径（跨重启保留），下次对话框从这里开始
      await call<void>("remember_open_path", { path: picked }).catch(() => undefined);
      renderSaveInfo(info);
      if (info.kind === "stash") {
        toast("共享箱（.d2i）暂只支持查看与保真保存，编辑功能待后续版本。", "error", 6000);
        // 共享箱没有角色/技能/任务/佣兵，这几块藏掉；但**不能直接 return**，
        // 否则物品页签也不会渲染（曾经就是这个 bug：箱子打开了却一片空白）
        for (const sel of ["#char-section", "#skills-section", "#quests-section", "#merc-section"]) {
          $(sel).classList.add("hidden");
        }
      }
      await loadAll();
    } catch (e) {
      showError(e, "打开存档失败");
    }
  });

  $("#btn-save").addEventListener("click", async () => {
    try {
      const v = await call<VerifyResult>("verify");
      if (!v.ok) {
        showError(v.message, "保存前校验未通过，已放弃写入");
        return;
      }
      const r = await call<SaveResult>("save", { path: null });
      toast(`已保存：${r.path}\n${r.bytes} 字节，${r.edits} 条修改${r.backed_up ? "，原文件已备份为 .bak" : ""}`);
      setDirty(false);
    } catch (e) {
      showError(e, "保存失败");
    }
  });

  $("#btn-reload").addEventListener("click", async () => {
    try {
      await call<SaveInfo>("reload");
      await loadAll();
      toast("已丢弃未保存修改并重新加载");
    } catch (e) {
      showError(e, "重新加载失败");
    }
  });


  $("#btn-apply-items").addEventListener("click", async () => {
    const edits = collectItemEdits();
    if (edits.length === 0) {
      toast("物品表里没有改动（改完耐久/数量再点这里）", "error");
      return;
    }
    try {
      const n = await call<number>("apply_item_edits", { edits, target: selectedTarget });
      toast(`已应用 ${n} 条物品修改。点「保存」才会写入文件。`);
      await loadAll();
    } catch (e) {
      showError(e, "物品修改被拒绝");
    }
  });

  $("#btn-apply-layout").addEventListener("click", applyLayout);
  $("#btn-check-bounds").addEventListener("click", checkBounds);

  $("#btn-apply-char").addEventListener("click", async () => {
    const num = (sel: string) => Number(($(sel) as HTMLInputElement).value);
    const char = await call<CharacterDto>("character");
    const edits: Edit[] = [];
    const want = {
      level: num("#f-level"),
      strength: num("#f-strength"),
      dexterity: num("#f-dexterity"),
      vitality: num("#f-vitality"),
      energy: num("#f-energy"),
      statPoints: num("#f-stat-points"),
      skillPoints: num("#f-skill-points"),
      gold: num("#f-gold"),
      stashGold: num("#f-stash-gold"),
    };
    if (want.level !== char.level) edits.push({ kind: "setLevel", level: want.level });
    const statPairs: [number, number, number][] = [
      [STAT.STRENGTH, want.strength, char.strength],
      [STAT.DEXTERITY, want.dexterity, char.dexterity],
      [STAT.VITALITY, want.vitality, char.vitality],
      [STAT.ENERGY, want.energy, char.energy],
      [STAT.STAT_POINTS, want.statPoints, char.statPoints],
      [STAT.SKILL_POINTS, want.skillPoints, char.skillPoints],
      [STAT.GOLD, want.gold, char.gold],
      [STAT.STASH_GOLD, want.stashGold, char.stashGold],
    ];
    for (const [stat, value, old] of statPairs) {
      if (value !== old) edits.push({ kind: "setStat", stat, value });
    }
    await applyEdits(edits, "角色");
  });

  $("#btn-waypoints-all").addEventListener("click", async () => {
    await applyEdits(
      [0, 1, 2].map((difficulty) => ({ kind: "unlockWaypoints", difficulty }) as Edit),
      "传送点",
    );
  });

  $("#btn-apply-skills").addEventListener("click", async () => {
    const inputs = Array.from(document.querySelectorAll<HTMLInputElement>("input[data-skill-index]"));
    const edits: Edit[] = [];
    for (const input of inputs) {
      const index = Number(input.dataset.skillIndex);
      const level = Number(input.value);
      const old = skills.find((s) => s.index === index)?.level ?? 0;
      if (level !== old) edits.push({ kind: "setSkill", index, level });
    }
    await applyEdits(edits, "技能");
  });

  document.addEventListener("change", async (ev) => {
    const el = ev.target as HTMLInputElement;
    if (!el.dataset.wpIndex) return;
    await applyEdits(
      [
        {
          kind: "setWaypoint",
          difficulty: Number(el.dataset.wpDiff),
          index: Number(el.dataset.wpIndex),
          active: el.checked,
        },
      ],
      "传送点",
    );
  });
}

window.addEventListener("DOMContentLoaded", async () => {
  bind();
  try {
    await refreshDataset();
  } catch (e) {
    showError(e, "读取数据集失败（保存仍可用，界面名字会退化为代码）");
  }
});

/** 右键菜单的目标与当前选中的装备库条目。 */
type ItemMenuTarget =
  | { kind: "item"; index: number }
  | { kind: "empty"; page: string };

let menuTarget: ItemMenuTarget | null = null;
let libraryItems: LibraryItemDto[] = [];
let librarySelected: string | null = null;
/**
 * 已展开的分组键集合（任意层级）。
 *
 * 装备库是三层树（绿装/暗金 → 套装名/类型 → 物品），
 * 默认**全部收起**；点组头展开，再点收起。
 */
const expandedGroups = new Set<string>();

/** 关掉右键菜单（点别处、按 Esc 之外的任何操作都会关）。 */
function hideItemMenu() {
  $("#item-menu").classList.add("hidden");
}

/**
 * 弹出右键菜单。
 *
 * 物品上：编辑 / 复制 / 导入。
 * 空白处：只有导入（装备库 / 本地文件）——用户明确要求"空白地方右键"能导入。
 */
function showItemMenu(target: ItemMenuTarget, x: number, y: number) {
  menuTarget = target;
  const menu = $("#item-menu");
  const entries =
    target.kind === "item"
      ? [
          { label: "编辑属性", action: () => void selectItem(target.index) },
          { label: "复制这件", action: () => void duplicateItemNow(target.index) },
          { label: "导出这件…", action: () => void exportItemNow(target.index) },
          { label: "重命名…", action: () => openRename(target.index) },
          { label: "删除这件", action: () => void deleteItemNow(target.index) },
          { label: "从装备库导入…", action: () => void openLibrary() },
          { label: "从文件导入…", action: () => void importFromFile() },
        ]
      : [
          { label: "从装备库导入…", action: () => void openLibrary() },
          { label: "从文件导入…", action: () => void importFromFile() },
        ];
  menu.innerHTML = entries
    .map(
      (e, i) =>
        `<button class="ctx-item" data-ctx="${i}">${escapeHtml(e.label)}</button>`,
    )
    .join("");
  menu.style.left = `${x}px`;
  menu.style.top = `${y}px`;
  menu.classList.remove("hidden");
  menu.querySelectorAll<HTMLButtonElement>("button[data-ctx]").forEach((btn) => {
    btn.addEventListener("click", () => {
      hideItemMenu();
      entries[Number(btn.dataset.ctx)].action();
    });
  });
}

/** 全局关闭菜单：点别处 / 滚轮 / 窗口变化。 */
function bindItemMenu() {
  document.addEventListener("click", (ev) => {
    if (!(ev.target as HTMLElement | null)?.closest("#item-menu")) hideItemMenu();
  });
  window.addEventListener("blur", hideItemMenu);
}

/** 复制一件物品到空位。 */
async function duplicateItemNow(index: number) {
  try {
    const r = await call<{ index: number; col: number; row: number }>("duplicate_item", {
      index,
    });
    toast(`已复制到 (${r.col}, ${r.row})。点「保存」才写入文件。`);
    await loadAll();
  } catch (e) {
    showError(e, "复制被拒绝");
  }
}

/** 打开装备库（第一次会从后端取，之后用缓存数组）。 */
async function openLibrary() {
  $("#lib-modal").classList.remove("hidden");
  $("#lib-detail").textContent = "";
  if (libraryItems.length === 0) {
    $("#lib-items").innerHTML = `<div class="muted">正在读取装备库…</div>`;
    try {
      libraryItems = await call<LibraryItemDto[]>("item_library");
    } catch (e) {
      showError(e, "读取装备库失败");
      return;
    }
  }
  fillLibraryGroups();
  renderLibraryItems();
}

/** 装备库条目的显示品质：套装在界面上叫"绿装"。 */
function qualityLabelOf(it: LibraryItemDto): string {
  return it.quality === "套装" ? "绿装" : it.quality;
}

/**
 * 装备库的层级：**绿装/暗金 → （套装名 / 物品类型）→ 物品**。
 *
 * 每一层都可折叠，默认全部收起——一打开就把几百件铺开会没有重点。
 * 返回从根到该条目路径上每一层的键，供展开状态按路径记录。
 */
function libraryPathOf(it: LibraryItemDto): { level1: string; level2: string } {
  const level1 = qualityLabelOf(it);
  const level2 =
    it.quality === "套装" && it.setName !== "" ? it.setName : it.group;
  return { level1, level2 };
}

/** 展开状态的键：用 `层级|名字` 拼，避免不同层同名互相影响。 */
function groupKey(level1: string, level2?: string): string {
  return level2 === undefined ? `L1|${level1}` : `L2|${level1}|${level2}`;
}

/** 填"按类型筛选"下拉：按第一层（绿装/暗金）与第二层组合。 */
function fillLibraryGroups() {
  const groups = new Set<string>();
  for (const it of libraryItems) {
    const { level1, level2 } = libraryPathOf(it);
    groups.add(`${level1}·${level2}`);
  }
  const sel = $("#lib-group") as HTMLSelectElement;
  const previous = sel.value;
  sel.innerHTML =
    `<option value="">全部</option>` +
    [...groups]
      .sort()
      .map((g) => `<option value="${escapeAttr(g)}">${escapeHtml(g)}</option>`)
      .join("");
  if ([...groups].includes(previous)) sel.value = previous;
}

/** 渲染装备库：三层树（绿装/暗金 → 套装名/类型 → 物品）。 */
function renderLibraryItems() {
  const keyword = ($("#lib-search") as HTMLInputElement).value.trim().toLowerCase();
  const filter = ($("#lib-group") as HTMLSelectElement).value;
  const matched = libraryItems.filter((it) => {
    const { level1, level2 } = libraryPathOf(it);
    if (filter && `${level1}·${level2}` !== filter) return false;
    if (keyword === "") return true;
    return it.name.toLowerCase().includes(keyword) || it.code.includes(keyword);
  });
  $("#lib-count").textContent = `${matched.length} / ${libraryItems.length} 件`;

  // 搜索时忽略折叠状态：否则打完关键字只看到一串收起的组头。
  const searching = keyword !== "" || filter !== "";
  const isOpen = (key: string) => searching || expandedGroups.has(key);

  // 按 第一层 → 第二层 建树，保持数据本身的顺序
  const tree = new Map<string, Map<string, LibraryItemDto[]>>();
  for (const it of matched) {
    const { level1, level2 } = libraryPathOf(it);
    if (!tree.has(level1)) tree.set(level1, new Map());
    const inner = tree.get(level1)!;
    if (!inner.has(level2)) inner.set(level2, []);
    inner.get(level2)!.push(it);
  }

  const blocks: string[] = [];
  for (const [level1, level2Map] of tree) {
    const key1 = groupKey(level1);
    const open1 = isOpen(key1);
    const total1 = [...level2Map.values()].reduce((n, l) => n + l.length, 0);
    blocks.push(
      `<button class="lib-group-head lv1${open1 ? " open" : ""}" data-key="${escapeAttr(key1)}">` +
        `<span class="caret">${open1 ? "▾" : "▸"}</span>${escapeHtml(level1)}` +
        `<span class="lib-group-count">${level2Map.size} 类 / ${total1} 件</span></button>`,
    );
    if (!open1) continue;

    for (const [level2, list] of level2Map) {
      const key2 = groupKey(level1, level2);
      const open2 = isOpen(key2);
      blocks.push(
        `<button class="lib-group-head lv2${open2 ? " open" : ""}" data-key="${escapeAttr(key2)}">` +
          `<span class="caret">${open2 ? "▾" : "▸"}</span>${escapeHtml(level2)}` +
          `<span class="lib-group-count">${list.length} 件</span></button>`,
      );
      if (!open2) continue;
      for (const it of list) {
        blocks.push(
          `<button class="lib-item lv3" data-lib="${escapeAttr(it.id)}">` +
            `<span class="lib-name">${escapeHtml(it.name)}</span>` +
            `<span class="lib-meta">${it.width}×${it.height}｜需${it.requiredLevel}级｜${it.stats.length} 条属性</span>` +
            `</button>`,
        );
      }
    }
  }
  $("#lib-items").innerHTML =
    blocks.length > 0 ? blocks.join("") : `<div class="muted">没有匹配的装备</div>`;

  // 每层组头都可折叠
  $("#lib-items")
    .querySelectorAll<HTMLButtonElement>("button[data-key]")
    .forEach((btn) => {
      btn.addEventListener("click", () => {
        const key = btn.dataset.key ?? "";
        if (expandedGroups.has(key)) expandedGroups.delete(key);
        else expandedGroups.add(key);
        renderLibraryItems();
      });
    });

  // **点一下就直接放进物品栏**：不再"先选中、再点导入"两步确认。
  // 导入成功后关掉弹窗，让用户直接看到物品落到了哪一格。
  $("#lib-items")
    .querySelectorAll<HTMLButtonElement>("button[data-lib]")
    .forEach((btn) => {
      btn.addEventListener("click", () => {
        librarySelected = btn.dataset.lib ?? null;
        void importLibraryNow();
      });
    });
}

/** 把装备库选中项导入到当前容器（物品上右键时用它的容器，空白处用所在容器）。 */
async function importLibraryNow() {
  if (!librarySelected) return;
  const page = menuTarget?.kind === "empty" ? menuTarget.page : "Inventory";
  try {
    const r = await call<{ index: number; name: string; stats: number; col: number; row: number }>(
      "import_library_item",
      { id: librarySelected, storePage: page },
    );
    toast(`已放入「${r.name}」（${r.stats} 条属性）到 (${r.col}, ${r.row})。点「保存」才写入文件。`);
    librarySelected = null;
    $("#lib-modal").classList.add("hidden");
    await loadAll();
  } catch (e) {
    showError(e, "导入被拒绝");
  }
}

/** 从本地文件导入（沿用已有的蓝图导入：选文件 + 指定模板）。 */
async function importFromFile() {
  const path = await openDialog({
    multiple: false,
    filters: [{ name: "物品蓝图", extensions: ["json"] }],
  });
  if (typeof path !== "string") return;
  // 模板：用当前存档里第一件物品当形状参考（蓝图会整体覆盖属性）
  const template = items.length > 0 ? items[0].index : null;
  if (template === null) {
    toast("存档里至少要有一件物品当模板", "error");
    return;
  }
  try {
    const json = await call<string>("read_blueprint", { path });
    const out = await call<{ index: number }>("import_item", {
      blueprintJson: json,
      template,
    });
    toast(`已从文件导入（序号 ${out.index}）。点「保存」才写入文件。`);
    await loadAll();
  } catch (e) {
    showError(e, "从文件导入失败");
  }
}

/**
 * 导出单件物品为 JSON 蓝图。
 *
 * 蓝图里既有塑形信息（代码/品质）也有全部属性，配合「从文件导入」可以
 * 在存档之间搬运物品；右键菜单里直接可用，不必先打开属性窗。
 */
async function exportItemNow(index: number) {
  try {
    const bp = await call<unknown>("export_item", { index });
    const dest = await saveDialog({
      defaultPath: `item-${index}.json`,
      filters: [{ name: "物品蓝图", extensions: ["json"] }],
    });
    if (!dest || typeof dest !== "string") return;
    await call<string>("write_blueprint", {
      path: dest,
      json: JSON.stringify(bp, null, 2),
    });
    toast(`已导出到 ${dest}`);
  } catch (e) {
    showError(e, "导出物品失败");
  }
}

/**
 * 导出**当前存档的全部物品**为一份 JSON（数组，每项一个蓝图）。
 *
 * 与单件导出的区别：这里导的是整栏，便于备份/迁移整套装备。
 * 文件里同时写入导出时间与物品数量，方便日后分辨。
 */
async function exportAllItems() {
  try {
    const list = await call<unknown[]>("export_items");
    if (!Array.isArray(list) || list.length === 0) {
      toast("这件存档里没有可导出的物品", "error");
      return;
    }
    const dest = await saveDialog({
      defaultPath: "items.json",
      filters: [{ name: "物品蓝图", extensions: ["json"] }],
    });
    if (!dest || typeof dest !== "string") return;
    const payload = {
      // 只写"导了多少件、什么时候导的"，不写存档路径等本地信息
      exportedAt: new Date().toISOString(),
      count: list.length,
      items: list,
    };
    await call<string>("write_blueprint", {
      path: dest,
      json: JSON.stringify(payload, null, 2),
    });
    toast(`已导出 ${list.length} 件物品到 ${dest}`);
  } catch (e) {
    showError(e, "导出物品栏失败");
  }
}

/** 删除一件物品（含其中镶嵌的宝石/符文）。 */
async function deleteItemNow(index: number) {
  try {
    const left = await call<number>("delete_item", { index });
    toast(`已删除，物品栏剩 ${left} 件。点「保存」才写入文件。`);
    await loadAll();
  } catch (e) {
    showError(e, "删除物品被拒绝");
  }
}

/** 正在重命名的物品序号。 */
let renameTarget: number | null = null;

/** 打开重命名弹窗，预填当前的自定义名字。 */
function openRename(index: number) {
  renameTarget = index;
  const current = items.find((i) => i.index === index)?.personalizedName ?? "";
  const input = $("#rename-input") as HTMLInputElement;
  input.value = current;
  updateRenamePreview();
  $("#rename-modal").classList.remove("hidden");
  input.focus();
}

/** 更新预览：游戏会显示成「名字's 底材名」。 */
function updateRenamePreview() {
  const value = ($("#rename-input") as HTMLInputElement).value.trim();
  const base =
    (items.find((i) => i.index === renameTarget)?.name ?? "底材名").split("'s ").pop() ??
    "底材名";
  $("#rename-preview").textContent = value === "" ? "（未命名）" : value;
  void base;
}

/** 提交重命名。 */
async function applyRename() {
  if (renameTarget === null) return;
  const name = ($("#rename-input") as HTMLInputElement).value.trim();
  try {
    await call<null>("rename_item", { index: renameTarget, name });
    $("#rename-modal").classList.add("hidden");
    toast(name === "" ? "已清除命名。" : `已命名为「${name}」。点「保存」才写入文件。`);
    await loadAll();
  } catch (e) {
    showError(e, "重命名被拒绝");
  }
}

/**
 * 有弹窗打开时锁住页面滚动。
 *
 * 用 MutationObserver 统一处理，而不是在每个 open/close 里各写一遍：
 * 弹窗有好几个（属性、设置、装备库、重命名），漏掉一处就会"弹窗开着还能滚背景"。
 */
function watchModalScrollLock() {
  const sync = () => {
    const anyOpen = document.querySelector(".modal:not(.hidden)") !== null;
    document.body.classList.toggle("modal-open", anyOpen);
  };
  const observer = new MutationObserver(sync);
  document.querySelectorAll(".modal").forEach((el) => {
    observer.observe(el, { attributes: true, attributeFilter: ["class"] });
  });
  sync();
}

/** 把一条属性的层从 `from` 换成 `to`（值保持不变）。 */
async function relayerStatNow(stat: number, from: number, to: number) {
  if (selectedItem === null) return;
  try {
    await call<number>("apply_item_edits", {
      edits: [{ kind: "relayerStat", index: selectedItem, stat, from, to }],
    });
    toast("值2 已更改。点「保存」才写入文件。");
    await loadAll();
  } catch (e) {
    showError(e, "修改值2 被拒绝");
  }
}

/**
 * 版本号与"检查更新"。
 *
 * 版本号从后端拿（`Cargo.toml` 是唯一来源，不在前端写死，免得两边不同步）。
 * "检查更新"不在程序里联网，直接打开项目的 Releases 页面。
 */
async function setupVersionAndUpdate() {
  try {
    const version = await call<string>("app_version");
    $("#app-version").textContent = `v${version}`;
    $("#settings-version").textContent = `当前版本 v${version}`;
  } catch {
    // 拿不到版本不影响使用，静默跳过
  }
  $("#btn-check-update").addEventListener("click", async () => {
    try {
      await call<string>("open_releases_page");
      toast("已在浏览器打开 Releases 页面");
    } catch (e) {
      showError(e, "打开 Releases 页面失败");
    }
  });
}
