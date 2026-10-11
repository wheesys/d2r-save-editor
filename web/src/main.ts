import { invoke } from "@tauri-apps/api/core";

import {
  bindGridDrag,
  escapeAttr,
  escapeHtml,
  renderInventory,
  renderMercDoll,
  setItemIcons,
} from "./inventory";
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
  const icon = skillIcons[iconCodeOf(s)] ?? "";
  const art = icon
    ? `<img src="${icon}" alt="" />`
    : `<span class="skill-fallback">${escapeHtml((s.name ?? "?").slice(0, 3))}</span>`;
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
      // 逐条属性各有自己的有效范围（来自 ItemStatCost 的位宽/偏移/移位），
      // 直接显示出来并作为输入框的上下限，用户就不会填到会被截断的值。
      const parts: string[] = [];
      // 按等级加成：只显示"每级加多少"，不显示合计（用户明确要求）。
      // 数值部分是系数，`displayValue` 已经是"每级"的量（可能是小数，如 7.875）。
      if (s.perLevel) {
        parts.push(`每级 +${s.displayValue}`);
      }
      if (s.rangeText) parts.push(`有效范围 ${s.rangeText}`);
      // 带参数属性的参数含义（如"获得 5 级「Holy Shock」光环"）
      if (s.paramLabel) parts.push(s.paramLabel);
      if (!s.userVisible) parts.push("内部字段");
      const note = parts.length
        ? `<span class="muted">${escapeHtml(parts.join("｜"))}</span>`
        : "";
      const nameCls = s.userVisible ? "" : ' class="muted"';
      // 输入框里放的是**游戏显示值**，上下限也是显示值——
      // 存档里的原始数由后端换算，用户不需要知道。
      const bounds =
        s.minValue !== null && s.maxValue !== null
          ? ` min="${s.minValue}" max="${s.maxValue}"`
          : "";
      const stepAttr = s.displayStep > 1 ? ` step="${s.displayStep}"` : "";
      return `<tr>
        <td>${s.id}</td>
        <td${nameCls}>${s.name}${s.userVisible ? "" : " <span class='muted'>(内部字段)</span>"}</td>
        <td class="muted">${s.internal}</td>
        <td><input type="number" class="cell-input" value="${s.displayValue}"${bounds}${stepAttr} data-stat-id="${s.id}" data-stat-layer="${s.layer}" /></td>
        <td>${note}</td>
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

  // 先按该属性的可表示范围自查，给出**具体到这条属性**的提示
  const problem = statValueProblem(id, value, $("#add-stat-value") as HTMLInputElement);
  if (problem) {
    $("#detail-result").innerHTML = `<div class="problem">${escapeHtml(problem)}</div>`;
    return;
  }

  try {
    // 直接把这条属性应用到物品上（后端会校验并读回比对）
    await call<number>("apply_item_edits", {
      edits: [{ kind: "setStat", index: selectedItem, stat: id, value }],
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
    return;
  }
  const parts: string[] = [];
  if (entry.minValue !== null && entry.maxValue !== null) {
    parts.push(`有效范围 ${entry.minValue}–${entry.maxValue}`);
  }
  if (entry.displayStep > 1) parts.push(`按 1/${entry.displayStep} 精度存储`);
  if (entry.hasParamBits) parts.push("含参数位");
  if (!entry.userVisible) parts.push("内部字段（改了可能不生效）");
  hint.textContent = parts.join("｜");

  // 把范围写进输入框，浏览器会拦住越界与非法步长
  if (entry.minValue !== null) input.min = String(entry.minValue);
  else input.removeAttribute("min");
  if (entry.maxValue !== null) input.max = String(entry.maxValue);
  else input.removeAttribute("max");
  if (entry.displayStep > 1) input.step = String(entry.displayStep);
  else input.removeAttribute("step");
  // 默认值取范围内的值，免得一打开就是非法的
  if (entry.minValue !== null && Number(input.value) < entry.minValue) {
    input.value = String(entry.minValue);
  }
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
  collectProblems.length = 0;
  document.querySelectorAll<HTMLInputElement>("input[data-stat-id]").forEach((el) => {
    const stat = Number(el.dataset.statId);
    const value = Number(el.value);
    // 前端先按输入框上的 min/max/step 拦一道，给出**具体到哪条属性**的提示，
    // 而不是等后端拒绝后只看到一句"ID 7 的值 100 存不进去"。
    // 后端仍会再校验一次（前端只是让提示更好懂，不替代后端校验）。
    const problem = statValueProblem(stat, value, el);
    if (problem) collectProblems.push(problem);
    edits.push({ kind: "setStat", index, stat, value });
  });
  return edits;
}

/// 收集时发现的非法值（由调用方读取后提示；避免为它多加返回值）。
const collectProblems: string[] = [];

/**
 * 检查一个属性值是否可表示；不可表示时返回可读原因。
 *
 * 规则与后端一致（存档写入时会右移 `ValShift` 位、值右移后为 0 会被整个跳过），
 * 这里只是提前告诉用户，避免提交后才被拒。
 */
function statValueProblem(id: number, value: number, el: HTMLInputElement): string | null {
  if (Number.isNaN(value)) return `属性 ID ${id} 的值不是数字`;
  const min = el.min === "" ? null : Number(el.min);
  const max = el.max === "" ? null : Number(el.max);
  const step = el.step === "" ? 1 : Number(el.step);
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

/** 应用明细面板的改动 */
async function applyDetailEdits() {
  const edits = collectDetailEdits();
  // 有不可表示的值就**先不提交**，把每条原因列出来：
  // 这些值写进去会被静默截断，或者让整条属性消失，必须让用户先改对。
  if (collectProblems.length > 0) {
    $("#detail-result").innerHTML = collectProblems
      .map((p) => `<div class="problem">${escapeHtml(p)}</div>`)
      .join("");
    showError(new Error(collectProblems[0]), `${collectProblems.length} 个属性值不可表示`);
    return;
  }
  if (edits.length === 0) {
    toast("没有需要应用的修改", "error");
    return;
  }
  try {
    const n = await call<number>("apply_item_edits", { edits, target: selectedTarget });
    pendingDetailEdits.length = 0;
    $("#detail-result").textContent = "";
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
    ["#char-section", "这里显示角色属性、金币与经验。"],
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
  $("#btn-apply-detail").addEventListener("click", applyDetailEdits);
  $("#btn-add-stat").addEventListener("click", addStatNow);

  // 选中的属性要在**添加之前**就告诉他最大/最小值（否则填了才知道超范围）
  $("#add-stat-select").addEventListener("change", showAddStatHint);
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
  document.addEventListener("keydown", (ev) => {
    if (ev.key !== "Escape") return;
    if (!$("#item-modal").classList.contains("hidden")) closeItemModal();
    if (!$("#settings-modal").classList.contains("hidden")) closeSettingsModal();
  });

  $("#btn-quests-all").addEventListener("click", () => void completeAllQuests());

  // 页签切换
  document.querySelectorAll<HTMLButtonElement>("#tabs .tab").forEach((btn) => {
    btn.addEventListener("click", () => activateTab(btn.dataset.tab ?? "char"));
  });

  // 数字键 1..6 直接切页签（键盘操作比鼠标快，也方便截图核对）
  document.addEventListener("keydown", (ev) => {
    if (ev.target instanceof HTMLInputElement || ev.target instanceof HTMLSelectElement) return;
    const order = ["char", "skills", "quests", "items", "merc"];
    const n = Number(ev.key);
    if (Number.isInteger(n) && n >= 1 && n <= order.length) activateTab(order[n - 1]);
  });

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
