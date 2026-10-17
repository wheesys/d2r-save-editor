/**
 * 物品属性行的渲染片段（**无 DOM 依赖**，便于后台校验）。
 *
 * 为什么单独抽一个文件：这条路径出过一次很难查的 bug——界面上"值"整列空白。
 * 模板产出、单测、真实 DOM 复现全都是对的，问题只在实际运行环境里出现，
 * 靠肉眼看截图来发现太慢。把渲染片段抽成纯函数后，
 * `tools/render-check.mjs` 就能在**不开窗口**的情况下断言
 * "每条属性都带上了 value 属性且数值正确"，下次再出立刻能定位。
 */

/** 属性值在输入框里显示成什么文本。 */
export interface StatLike {
  id: number;
  layer: number;
  /** 存档里的值（界面上就是这个数）。 */
  value?: number;
  /** 早期版本字段，兜底用。 */
  displayValue?: number;
  name: string;
  internal?: string;
  userVisible?: boolean;
  perLevel?: boolean;
  rangeText?: string | null;
  paramLabel?: string | null;
  hasLayer?: boolean;
  minValue?: number | null;
  maxValue?: number | null;
  displayStep?: number;
  paramOptions?: { layer: number; label: string }[];
}


/** 参数选项（值2 的可选值），可能带分组（如技能所属职业）。 */
export interface LayerOption {
  layer: number;
  label: string;
  group?: string | null;
}

/**
 * 生成"值2"控件。
 *
 * 三种形态：
 * 1. 没有可选值 → 数字输入框（自己填层号）；
 * 2. 有可选值、不分级 → 单个下拉（如职业、光环）；
 * 3. 有可选值、带分组 → **两级选单**：左边选分组（职业），右边选具体项（技能）。
 *
 * 为什么用两个下拉而不是 `<optgroup>`：原生 `optgroup` 只能做视觉分组，
 * 不能收起，也不能筛选；两个下拉里左边那个天然就是"收起/切换"的入口，
 * 200 多个技能也不会挤成一条长列表。
 *
 * `attrs` 是给**真正承载层号**的那个控件加的属性（`data-stat-layer-sel`
 * 等），上级的分组下拉不加，免得被当成值改动触发保存。
 */
export function renderLayerControl(
  id: number,
  current: number,
  options: LayerOption[],
  attrs: string,
): string {
  if (options.length === 0) {
    return `<input type="text" inputmode="numeric" value="${current}" ${attrs} />`;
  }
  const groups = [...new Set(options.map((o) => o.group ?? ""))];
  const grouped = groups.length > 1 || (groups.length === 1 && groups[0] !== "");
  if (!grouped) {
    return `<select ${attrs}>${options
      .map(
        (o) =>
          `<option value="${o.layer}"${o.layer === current ? " selected" : ""}>${escapeHtml(o.label)}</option>`,
      )
      .join("")}</select>`;
  }

  // 当前层所属分组；找不到就用第一个分组
  const currentGroup =
    options.find((o) => o.layer === current)?.group ??
    options.find((o) => (o.group ?? "") !== "")?.group ??
    groups[0];
  const inGroup = options.filter((o) => (o.group ?? "") === currentGroup);
  // 当前层不在任何分组里（未知值）时补一条，避免下拉"假装"选中了第一项
  const missing = !inGroup.some((o) => o.layer === current);
  const skills = missing
    ? [{ layer: current, label: `层 ${current}（未知）` }, ...inGroup]
    : inGroup;

  return (
    `<span class="layer-pair" data-layer-pair="${id}">` +
    `<select class="layer-group-sel" data-layer-for="${id}" title="分组">` +
    groups
      .map(
        (g) =>
          `<option value="${escapeAttr(g)}"${g === currentGroup ? " selected" : ""}>${escapeHtml(g || "其它")}</option>`,
      )
      .join("") +
    `</select>` +
    `<select class="cell-select" ${attrs}>` +
    skills
      .map(
        (o) =>
          `<option value="${o.layer}"${o.layer === current ? " selected" : ""}>${escapeHtml(o.label)}</option>`,
      )
      .join("") +
    `</select></span>`
  );
}

/** 最小 HTML 转义（属性值用）。 */
export function escapeAttr(text: unknown): string {
  return String(text)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

/** 最小 HTML 转义（文本节点用）。 */
export function escapeHtml(text: unknown): string {
  return escapeAttr(text);
}

/**
 * 属性值在输入框里显示成什么文本。
 *
 * 多路兜底：正常用 `value`（存档里的整数）；万一字段缺失，退到
 * `displayValue`，再不行给空串。
 */
export function statValueText(stat: StatLike): string {
  // **优先用 `displayValue`**：带 `ValShift` 的属性（生命/法力按等级）在存档里
  // 按 1/256 精度存，原始 `value` 是 16128，而游戏里是 63。后端已经算好
  // `displayValue`，这里再优先取 `value` 就会把 16128 显示出来（踩过）。
  if (
    typeof stat.displayValue === "number" &&
    Number.isFinite(stat.displayValue)
  ) {
    return String(stat.displayValue);
  }
  if (typeof stat.value === "number" && Number.isFinite(stat.value)) {
    return String(stat.value);
  }
  return "";
}

/**
 * 渲染一条属性行。
 *
 * 两个值列统一叫**值1 / 值2**（跟游戏里一样，用户只看数值，
 * 含义由"说明"列讲清楚），不再出现"层"这种内部术语。
 *
 * 值1 同时写 `value` 属性**和**由调用方显式赋值：属性路径不依赖脚本执行，
 * 显式赋值兜住属性解析异常，两条路径任一条可用就不会空白。
 */
export function renderStatRow(stat: StatLike): string {
  const parts: string[] = [];
  // 按等级加成要标出来（这类属性的值在游戏里随等级生效）
  if (stat.perLevel) {
    parts.push("按等级加成（游戏内按 值 × 等级 / 8 生效）");
  }
  if (stat.rangeText) parts.push(`有效范围 ${stat.rangeText}`);
  // 参数含义（如"获得 31 级「狂热」光环"、"法师 技能等级 +7"）
  // 说明里的参数名优先取**下拉选项里的标签**：后端那份 `paramLabel`
  // 可能和选项表对不上（出现过"下拉是能量盾、说明写攻击"）。
  const optionLabel = (stat.paramOptions ?? []).find(
    (o) => o.layer === stat.layer,
  )?.label;
  if (optionLabel) parts.push(optionLabel);
  else if (stat.paramLabel) parts.push(stat.paramLabel);
  if (stat.userVisible === false) parts.push("内部字段");
  const note = parts.length
    ? `<span class="muted">${escapeHtml(parts.join("｜"))}</span>`
    : "";

  // 值2：带参数位的属性才有。有已知可选值时是下拉框（用户直接选"法师"），
  // 否则是可编辑的数字输入框。
  let secondCell = "";
  if (stat.hasLayer) {
    secondCell = renderLayerControl(
      stat.id,
      stat.layer,
      stat.paramOptions ?? [],
      `class="cell-input" data-stat-layer-sel="${stat.id}" data-stat-layer-cur="${stat.layer}"`,
    );
  }

  const hint =
    stat.minValue !== null &&
    stat.minValue !== undefined &&
    stat.maxValue !== null &&
    stat.maxValue !== undefined
      ? `范围 ${stat.minValue} ~ ${stat.maxValue}`
      : "";

  return `<tr>
    <td>${stat.id}</td>
    <td>${escapeHtml(stat.name)}</td>
    <td><input type="text" inputmode="numeric" class="cell-input" value="${escapeAttr(statValueText(stat))}"
      data-stat-id="${stat.id}" data-stat-layer="${stat.layer}"
      data-min="${stat.minValue ?? ""}" data-max="${stat.maxValue ?? ""}"
      data-step="${stat.displayStep ?? 1}" title="${escapeAttr(hint)}" /></td>
    <td>${secondCell}</td>
    <td>${note}</td>
    <td><button class="mini danger" data-del-stat="${stat.id}" data-del-layer="${stat.layer}">删除</button></td>
  </tr>`;
}

/** 属性表的完整表头（列名与 `renderStatRow` 一一对应）。 */
export const STAT_TABLE_HEAD =
  "<thead><tr><th>ID</th><th>属性</th><th>值1</th><th>值2</th><th>说明</th><th></th></tr></thead>";
