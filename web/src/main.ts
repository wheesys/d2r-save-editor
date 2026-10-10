import { invoke } from "@tauri-apps/api/core";

import { bindGridDrag, renderInventory } from "./inventory";
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

async function refreshDataset() {
  const info = await call<DatasetInfo>("dataset_info");
  $("#dataset").textContent =
    `数据集：${info.tables} 张表｜字符串表 ${info.strings} 条（${info.languages.length} 种语言）｜${info.root}`;
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

function renderWaypoints(list: WaypointsDto[]) {
  $("#waypoints").innerHTML = list
    .map(
      (w, d) => `
      <div>
        <div class="muted">${w.difficulty}（已开通 ${w.active.filter(Boolean).length} / ${w.active.length}）</div>
        <div class="diff">
          ${w.active
            .map(
              (on, i) =>
                `<label class="wp ${on ? "on" : ""}"><input type="checkbox" data-wp-diff="${d}" data-wp-index="${i}" ${on ? "checked" : ""}/>${i}</label>`,
            )
            .join("")}
        </div>
      </div>`,
    )
    .join("");
  $("#waypoints-section").classList.remove("hidden");
}

/** 当前选中的物品序号（用于明细面板） */
let selectedItem: number | null = null;
/** 属性目录缓存 */
let statCatalog: StatCatalogEntry[] = [];
/** 当前布局设置（容器网格尺寸要用） */
let currentLayout: LayoutSettings | null = null;

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

/** 选中一件物品，打开明细面板 */
async function selectItem(index: number) {
  selectedItem = index;
  renderItems(items);
  try {
    const detail = await call<ItemDetailDto>("item_detail", { index });
    renderItemDetail(detail);
  } catch (e) {
    showError(e, "读取物品明细失败");
  }
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
      toast("已记录删除，点「应用该物品的修改」生效");
    });
  });

  $("#item-detail-section").classList.remove("hidden");
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
    await loadAll();
    if (selectedItem !== null) await selectItem(selectedItem);
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
  try {
    const info = await call<BundleInfo>("bundle_info");
    if (info.available) {
      $("#dataset").textContent += `｜内置数据包 ${(info.totalBytes / 1048576).toFixed(1)} MB`;
    }
  } catch {
    /* 老版本后端没有这个命令，忽略 */
  }
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
  renderWaypoints(await call<WaypointsDto[]>("waypoints"));
  items = await call<ItemDto[]>("item_details");
  renderItems(items);
  await loadStatCatalog();
  await refreshLayout();
  setDirty(await call<boolean>("is_dirty"));
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

  void bootIfAlreadyOpen();

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
