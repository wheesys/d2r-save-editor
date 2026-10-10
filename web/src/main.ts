import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";

import {
  type AppError,
  type CharacterDto,
  type DatasetInfo,
  type Edit,
  type ItemDto,
  type ItemEdit,
  type ItemStats,
  type BundleInfo,
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

function renderItems(list: ItemDto[]) {
  const byPage = new Map<string, number>();
  for (const it of list) byPage.set(it.store_page, (byPage.get(it.store_page) ?? 0) + 1);
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
      return `<tr>
        <td>${it.index}</td>
        <td>${it.code}</td>
        <td>${it.name}${it.name_resolved ? "" : " <span class='muted'>(未解析)</span>"}</td>
        <td>${it.quality}</td>
        <td>${it.level}</td>
        <td>${it.store_page}${it.body_location !== "None" ? ` · ${it.body_location}` : ""}</td>
        <td>${it.width}×${it.height}</td>
        <td>${durCell}</td>
        <td>${qtyCell}</td>
        <td>${it.socketCount > 0 ? it.socketCount : "—"}</td>
        <td>${it.stats_count}</td>
      </tr>`;
    })
    .join("");

  $("#items").innerHTML =
    `<thead><tr>
      <th>#</th><th>代码</th><th>名称</th><th>品质</th><th>等级</th>
      <th>位置</th><th>占格</th><th>耐久</th><th>数量</th><th>孔</th><th>属性</th>
     </tr></thead><tbody>${rows}</tbody>`;
  $("#items-section").classList.remove("hidden");
}

/** 收集物品表里的改动（耐久/数量） */
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
  return edits;
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

function bind() {
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
