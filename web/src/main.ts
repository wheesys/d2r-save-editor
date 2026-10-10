import { invoke } from "@tauri-apps/api/core";

import { bindGridDrag, escapeAttr, escapeHtml, renderInventory, setItemIcons } from "./inventory";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";

import {
  type AppError,
  type CharacterDto,
  type DatasetInfo,
  type Edit,
  type ItemDto,
  type ItemEdit,
  type ItemStats,
  type BundleInfo,
  type ItemDetailDto,
  type StatCatalogEntry,
  type MercenaryItem,
  type LayoutSettings,
  type LayoutHints,
  type BoundsIssue,
  type SaveInfo,
  type SaveResult,
  type SkillDto,
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
let showAllSkills = false;

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
  const info = await call<DatasetInfo>("dataset_info");
  // 数据来源那段由后端的 data_source 提供（含内置数据包的张数/字符串数/图标数/体积）。
  // 前端不要再拼一遍"内置数据包 xx MB"——那样这行会出现两遍同名信息。
  const extra = bundleInfo?.available ? `｜图标 ${bundleInfo.icons} 张` : "";
  $("#dataset").textContent =
    `数据集：${info.tables} 张表｜字符串表 ${info.strings} 条（${info.languages.length} 种语言）${extra}｜${info.root}`;
}

function setDirty(dirty: boolean) {
  $("#dirty").classList.toggle("hidden", !dirty);
  $("#btn-save").toggleAttribute("disabled", !dirty);
}

function renderSaveInfo(info: SaveInfo) {
  const rows: [string, string][] = [
    ["路径", info.path],
    ["类型", info.kind === "stash" ? "共享储物箱（.d2i）" : "角色存档（.d2s）"],
    ["存档版本", String(info.version)],
    ["文件大小", `${info.size} 字节`],
    ["校验和", info.checksum === null ? "无（.d2i 不存）" : `0x${(info.checksum >>> 0).toString(16).toUpperCase()}`],
    ["往返一致", info.roundtripIdentical ? "是" : "否（结构有差异，谨慎保存）"],
  ];
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

function renderSkills() {
  const visible = showAllSkills ? skills : skills.filter((s) => s.level > 0);
  $("#skills").innerHTML = visible
    .map(
      (s) => `
      <div class="row ${s.level === 0 ? "zero" : ""}">
        <span class="name" title="槽位 ${s.index}｜技能ID ${s.skillId ?? "?"}">${s.name ?? `（未命名技能 ${s.skillId ?? s.index}）`}</span>
        <input type="number" min="0" max="99" data-skill-index="${s.index}" value="${s.level}" />
      </div>`,
    )
    .join("");
  const zeroCount = skills.filter((s) => s.level === 0).length;
  if (!showAllSkills && zeroCount > 0) {
    $("#skills").insertAdjacentHTML(
      "beforeend",
      `<div class="row zero"><span class="name muted">…还有 ${zeroCount} 个未加点槽位（勾选「显示全部」查看）</span></div>`,
    );
  }
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
    (index) => void selectItem(index),
    async (index, page, col, row) => {
      try {
        const n = await call<number>("apply_item_edits", {
          edits: [{ kind: "relocate", index, page, col, row }],
        });
        toast(`已移动（${n} 条修改）。点「保存」写入文件。`);
        await loadAll();
        await selectItem(index);
      } catch (e) {
        showError(e, "移动被拒绝");
      }
    },
  );
  $("#inventory-section").classList.remove("hidden");
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

/** 选中一件物品：打开**属性弹窗**（不再往页面下面堆内容） */
async function selectItem(index: number) {
  selectedItem = index;
  renderItems(items);
  try {
    const detail = await call<ItemDetailDto>("item_detail", { index });
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
  pendingDetailEdits.length = 0;
}

/** 渲染明细面板 */
function renderItemDetail(detail: ItemDetailDto) {
  const it = detail.item;
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
  const rows = detail.stats
    .map((s) => {
      const stepNote =
        s.step > 1
          ? `<span class="muted">步长 ${s.step}</span>`
          : s.hasParamBits
            ? `<span class="muted">含参数</span>`
            : "";
      const nameCls = s.userVisible ? "" : " class=\"muted\"";
      return `<tr>
        <td>${s.id}</td>
        <td${nameCls}>${s.name}${s.userVisible ? "" : " <span class='muted'>(内部字段)</span>"}</td>
        <td class="muted">${s.internal}</td>
        <td><input type="number" class="cell-input" value="${s.value}" data-stat-id="${s.id}" data-stat-layer="${s.layer}" /></td>
        <td>${stepNote}</td>
        <td><button class="mini danger" data-del-stat="${s.id}" data-del-layer="${s.layer}">删除</button></td>
      </tr>`;
    })
    .join("");
  $("#item-stats").innerHTML =
    `<thead><tr><th>ID</th><th>属性</th><th>内部名</th><th>值</th><th>说明</th><th></th></tr></thead>` +
    `<tbody>${rows}</tbody>`;

  // 删除属性按钮
  document.querySelectorAll<HTMLButtonElement>("button[data-del-stat]").forEach((btn) => {
    btn.addEventListener("click", () => {
      pendingDetailEdits.push({
        kind: "removeStat",
        index: it.index,
        stat: Number(btn.dataset.delStat),
      });
      toast("已记录删除，点弹窗里的「应用修改」生效");
    });
  });
}

/** 明细面板里挂起的改动（删除属性等） */
const pendingDetailEdits: ItemEdit[] = [];

/** 收集明细面板的改动 */
function collectDetailEdits(): ItemEdit[] {
  const edits: ItemEdit[] = [...pendingDetailEdits];
  const index = selectedItem;
  if (index === null) return edits;
  const it = items.find((i) => i.index === index);
  if (!it) return edits;

  const num = (sel: string): number | null => {
    const el = $(sel) as HTMLInputElement;
    if (el.disabled || el.value === "") return null;
    return Number(el.value);
  };
  const sockets = num("#d-sockets");
  if (sockets !== null && sockets !== it.socketCount) {
    edits.push({ kind: "setSockets", index, count: sockets });
  }
  const dur = num("#d-durability");
  if (dur !== null && dur !== it.durability) edits.push({ kind: "setDurability", index, value: dur });
  const qty = num("#d-quantity");
  if (qty !== null && qty !== it.quantity) edits.push({ kind: "setQuantity", index, value: qty });
  const def = num("#d-defense");
  if (def !== null && def !== it.defense) edits.push({ kind: "setDefense", index, value: def });

  // 属性值
  document.querySelectorAll<HTMLInputElement>("input[data-stat-id]").forEach((el) => {
    const stat = Number(el.dataset.statId);
    const value = Number(el.value);
    edits.push({ kind: "setStat", index, stat, value });
  });
  return edits;
}

/** 应用明细面板的改动 */
async function applyDetailEdits() {
  const edits = collectDetailEdits().filter(
    (e) => e.kind !== "setStat" || true, // 属性值一律提交（后端幂等）
  );
  if (edits.length === 0) {
    toast("没有需要应用的修改", "error");
    return;
  }
  try {
    const n = await call<number>("apply_item_edits", { edits });
    pendingDetailEdits.length = 0;
    toast(`已应用 ${n} 条修改。点「保存」才会写入文件。`);
    const keep = selectedItem;
    await loadAll();
    // 重新读一遍这件物品并保持弹窗打开（值可能被规范化，要让用户看到结果）
    if (keep !== null) {
      const detail = await call<ItemDetailDto>("item_detail", { index: keep });
      renderItemDetail(detail);
      openItemModal();
    }
  } catch (e) {
    showError(e, "修改被拒绝");
    $("#detail-result").textContent = String((e as { message?: string })?.message ?? e);
  }
}

/** 载入属性目录并填充下拉 */
async function loadStatCatalog() {
  try {
    statCatalog = await call<StatCatalogEntry[]>("stat_catalog", { all: false });
    const sel = $("#add-stat-select") as HTMLSelectElement;
    sel.innerHTML = statCatalog
      .map((s) => `<option value="${s.id}">${s.name}（${s.internal}${s.step > 1 ? `，步长${s.step}` : ""}）</option>`)
      .join("");
  } catch {
    /* 老后端没有这个命令 */
  }
}

async function loadAll() {
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
  renderQuests();
  try {
    mercItems = await call<MercenaryItem[]>("mercenary_items");
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

/** 任务进度：后端还没暴露任务编辑，先把状态说清楚，不假装能改 */
function renderQuests() {
  $("#quests").innerHTML =
    `<div class="muted">任务进度的读写尚未接入（存档的任务段已能解析，编辑接口还没做）。</div>`;
  $("#quests-section").classList.remove("hidden");
}

/**
 * 佣兵：把存档**佣兵段**里的物品按身体槽位画出来。
 *
 * 佣兵段与主物品列表分开存储，所以这里用的是佣兵段自己的序号；
 * 图标/名字按物品代码解析（与物品栏一致）。
 */
let mercItems: MercenaryItem[] = [];
function renderMerc() {
  const host = $("#merc");
  if (mercItems.length === 0) {
    host.innerHTML = `<div class="muted">这件存档的佣兵段里没有物品。</div>`;
    $("#merc-section").classList.remove("hidden");
    return;
  }
  const slotLabel: Record<string, string> = {
    Head: "头", Neck: "颈", Torso: "身", RightArm: "右臂", LeftArm: "左臂",
    RightRing: "右戒", LeftRing: "左戒", Gloves: "手", Belt: "腰", Feet: "脚",
  };
  host.innerHTML = mercItems
    .map(
      (it) =>
        `<div class="merc-card">
           <span class="merc-slot">${escapeHtml(slotLabel[it.bodyLocation] ?? it.bodyLocation)}</span>
           <span class="merc-code">${escapeHtml(it.code)}</span>
           <span class="merc-body">
             <b>${escapeHtml(itemNameOf(it.code))}</b>
             <span class="muted">品质 ${escapeHtml(it.quality)}｜等级 ${it.level}｜属性 ${it.statsCount} 条${
               it.socketCount > 0 ? `｜孔 ${it.socketedCount}/${it.socketCount}` : ""
             }</span>
           </span>
         </div>`,
    )
    .join("");
  $("#merc-section").classList.remove("hidden");
}

/** 用物品代码在已加载的物品里找名字（佣兵物品不在主列表里，找不到就显示代码） */
function itemNameOf(code: string): string {
  const hit = items.find((i) => i.code === code);
  if (hit) return hit.name;
  return code;
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


/** 读取并展示布局设置与"需要确认的项" */
async function refreshLayout() {
  try {
    const [settings, hints, questions] = await Promise.all([
      call<LayoutSettings>("layout"),
      call<LayoutHints>("layout_hints"),
      call<string[]>("layout_questions"),
    ]);
    ($("#l-inv-cols") as HTMLInputElement).value = String(settings.inventoryCols);
    ($("#l-inv-rows") as HTMLInputElement).value = String(settings.inventoryRows);
    ($("#l-cube-cols") as HTMLInputElement).value = String(settings.cubeCols);
    ($("#l-cube-rows") as HTMLInputElement).value = String(settings.cubeRows);
    ($("#l-stash-cols") as HTMLInputElement).value = String(settings.stashCols);
    ($("#l-stash-rows") as HTMLInputElement).value = String(settings.stashRows);
    ($("#l-merc") as HTMLInputElement).checked = settings.mercFullEquipment;
    // 记住布局并刷新容器网格：网格尺寸（背包/方块/箱子行列数）全部来自它。
    // 漏了这三行会让 renderContainerView 恒早退，容器视图根本不渲染。
    currentLayout = settings;
    renderContainerView();

    const hint =
      `推断：背包最大 (${hints.inventoryMax?.[0] ?? "-"},${hints.inventoryMax?.[1] ?? "-"})｜` +
      `合成箱最大 (${hints.cubeMax?.[0] ?? "-"},${hints.cubeMax?.[1] ?? "-"})｜` +
      `箱子坐标最大 (${hints.stashMax?.[0] ?? "-"},${hints.stashMax?.[1] ?? "-"})｜` +
      `佣兵段 ${hints.mercSection ? `${hints.mercItems} 件` : "无"}`;
    const confirmNote = settings.confirmedByUser ? "（已由你确认）" : "（当前是默认值）";
    $("#layout-questions").innerHTML =
      `<div class="muted">${hint}${confirmNote}</div>` +
      (questions.length === 0
        ? ""
        : questions.map((q) => `<div class="question">需要确认：${q}</div>`).join(""));
    $("#layout-section").classList.remove("hidden");
  } catch (e) {
    showError(e, "读取布局设置失败");
  }
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
    confirmedByUser: true,
  };
  try {
    await call<void>("set_layout", { layout });
    toast("布局已确认。之后保存时按这套尺寸做越界校验。");
    await refreshLayout();
  } catch (e) {
    showError(e, "应用布局失败");
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

/** 启动自检：后端若已打开存档（例如开发时用 D2R_OPEN_SAVE 自动打开），直接进入编辑界面 */
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
async function bootIfAlreadyOpen() {
  try {
    const info = await call<SaveInfo | null>("current_save");
    if (!info) return;
    renderSaveInfo(info);
    if (info.kind === "stash") {
      $("#items-section").classList.add("hidden");
      return;
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
  } catch {
    /* 老后端没有这个命令 */
  }
}
function bind() {
  // 明细面板：应用该物品的修改 / 添加属性
  $("#btn-apply-detail").addEventListener("click", applyDetailEdits);
  $("#btn-add-stat").addEventListener("click", () => {
    const id = Number(($("#add-stat-select") as HTMLSelectElement).value);
    const value = Number(($("#add-stat-value") as HTMLInputElement).value);
    if (selectedItem === null) {
      toast("先在物品表里点一件物品", "error");
      return;
    }
    pendingDetailEdits.push({ kind: "setStat", index: selectedItem, stat: id, value });
    const entry = statCatalog.find((s) => s.id === id);
    toast(`已记录：${entry?.name ?? `属性#${id}`} = ${value}。点「应用该物品的修改」生效。`);
  });

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

  // 物品属性弹窗：关闭按钮 / 遮罩点击 / Esc
  document.querySelectorAll("[data-close-modal]").forEach((el) => {
    el.addEventListener("click", closeItemModal);
  });
  document.addEventListener("keydown", (ev) => {
    if (ev.key === "Escape" && !$("#item-modal").classList.contains("hidden")) closeItemModal();
  });

  // 页签切换
  document.querySelectorAll<HTMLButtonElement>("#tabs .tab").forEach((btn) => {
    btn.addEventListener("click", () => activateTab(btn.dataset.tab ?? "char"));
  });

  // 数字键 1..6 直接切页签（键盘操作比鼠标快，也方便截图核对）
  document.addEventListener("keydown", (ev) => {
    if (ev.target instanceof HTMLInputElement || ev.target instanceof HTMLSelectElement) return;
    const order = ["char", "skills", "quests", "items", "merc", "settings"];
    const n = Number(ev.key);
    if (Number.isInteger(n) && n >= 1 && n <= order.length) activateTab(order[n - 1]);
  });

  void bootIfAlreadyOpen();

  // 开发/自测用：`VITE_INITIAL_TAB=quests` 时启动就切到该页签，方便截图核对。
  // 正常构建里该值是 undefined，等价于不生效。
  const initialTab = (import.meta as { env?: Record<string, string> }).env?.VITE_INITIAL_TAB;
  if (initialTab) activateTab(initialTab);

  $("#btn-open").addEventListener("click", async () => {
    try {
      const picked = await openDialog({
        multiple: false,
        filters: [{ name: "D2R 存档", extensions: ["d2s", "d2i"] }],
      });
      if (!picked || typeof picked !== "string") return;
      const info = await call<SaveInfo>("open_save", { path: picked });
      renderSaveInfo(info);
      if (info.kind === "stash") {
        toast("共享箱（.d2i）暂只支持查看与保真保存，编辑功能待后续版本。", "error", 6000);
        $("#char-section").classList.add("hidden");
        $("#skills-section").classList.add("hidden");
        $("#waypoints-section").classList.add("hidden");
        $("#items-section").classList.add("hidden");
        return;
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

  $("#show-all-skills").addEventListener("change", (ev) => {
    showAllSkills = (ev.target as HTMLInputElement).checked;
    renderSkills();
  });

  $("#btn-apply-items").addEventListener("click", async () => {
    const edits = collectItemEdits();
    if (edits.length === 0) {
      toast("物品表里没有改动（改完耐久/数量再点这里）", "error");
      return;
    }
    try {
      const n = await call<number>("apply_item_edits", { edits });
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
