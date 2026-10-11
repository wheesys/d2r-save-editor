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
  if (typeof stat.value === "number" && Number.isFinite(stat.value)) {
    return String(stat.value);
  }
  if (
    typeof stat.displayValue === "number" &&
    Number.isFinite(stat.displayValue)
  ) {
    return String(stat.displayValue);
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
  if (stat.paramLabel) parts.push(stat.paramLabel);
  if (stat.userVisible === false) parts.push("内部字段");
  const note = parts.length
    ? `<span class="muted">${escapeHtml(parts.join("｜"))}</span>`
    : "";

  // 值2：带参数位的属性才有。有已知可选值时是下拉框（用户直接选"法师"），
  // 否则是可编辑的数字输入框。
  let secondCell = "";
  if (stat.hasLayer) {
    const options = stat.paramOptions ?? [];
    secondCell =
      options.length > 0
        ? `<select class="cell-select" data-stat-layer-sel="${stat.id}"
             data-stat-layer-cur="${stat.layer}">${options
             .map(
               (option) =>
                 `<option value="${option.layer}"${option.layer === stat.layer ? " selected" : ""}>${escapeHtml(option.label)}</option>`,
             )
             .join("")}</select>`
        : `<input type="text" inputmode="numeric" class="cell-input" value="${stat.layer}"
             data-stat-layer-sel="${stat.id}" data-stat-layer-cur="${stat.layer}"
             data-layer-plain="1" />`;
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
