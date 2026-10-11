/**
 * 后台渲染校验：不开窗口，直接跑界面里那套网格渲染函数。
 *
 * 为什么需要它：网格渲染最容易出的错（占格算错、物品被判越界、
 * 该显示名字却显示代码）在截图里看不出来——差一格、少一行都很难肉眼发现。
 * 这个脚本用**固定样本**喂给 `renderInventory()`，把结论打成可核对的文本，
 * 因此在全屏玩游戏时也能验证界面逻辑，不需要开着应用窗口。
 *
 * 运行（Node ≥ 22，原生支持 .ts）：
 *     node tools/render-check.mjs
 * 退出码非 0 表示有断言失败。
 */

import { buildGrids, renderInventory, setItemIcons } from "../web/src/inventory.ts";
import { STAT_TABLE_HEAD, renderStatRow } from "../web/src/statrow.ts";

/** 构造一件物品（字段与后端 ItemDto 对齐） */
function item(overrides) {
  return {
    index: 0,
    code: "hp1",
    name: "治疗药水",
    nameResolved: true,
    quality: "Normal",
    level: 1,
    mode: "Stored",
    bodyLocation: "None",
    storePage: "Inventory",
    invX: 0,
    invY: 0,
    statsCount: 0,
    defense: null,
    durability: null,
    maxDurability: 0,
    quantity: null,
    socketCount: 0,
    socketedCount: 0,
    socketedFlag: false,
    width: 1,
    height: 1,
    ...overrides,
  };
}

const layout = {
  inventoryCols: 10,
  inventoryRows: 4,
  cubeCols: 3,
  cubeRows: 4,
  stashCols: 10,
  stashRows: 10,
  mercFullEquipment: false,
  confirmedByUser: true,
};

let failures = 0;
function check(label, condition, detail = "") {
  if (condition) {
    console.log(`  ✓ ${label}`);
  } else {
    failures += 1;
    console.log(`  ✗ ${label}${detail ? ` — ${detail}` : ""}`);
  }
}

console.log("=== 1. 网格划分（物品该落在哪个容器）===");
{
  // 腰带：storePage 是 Null、靠 mode=InBelt 判定（真实存档就是这样）
  const items = [
    item({ index: 0, code: "rvs", mode: "InBelt", storePage: "Null", invX: 0, invY: 0 }),
    item({ index: 1, code: "rvs", mode: "InBelt", storePage: "Null", invX: 15, invY: 0 }),
    item({ index: 2, code: "cm1", mode: "Stored", storePage: "Stash", invX: 3, invY: 4 }),
    item({ index: 3, code: "amu", mode: "Equipped", storePage: "Null", bodyLocation: "Neck" }),
    item({ index: 4, code: "mpi", mode: "Stored", storePage: "Cube", invX: 1, invY: 1 }),
  ];
  const grids = buildGrids(layout, items);
  const byKey = Object.fromEntries(grids.map((g) => [g.key, g]));

  check("5 个容器", grids.length === 5, `实际 ${grids.length}`);
  // 装备位不再用行列网格，而是按游戏装备页的真实布局摆放（值见 EQUIP_SLOT_LAYOUT）
  check(
    "装备位用装备页布局（约 645×435，放大 3 倍）",
    byKey.Equip.doll !== undefined &&
      byKey.Equip.doll.width > 600 &&
      byKey.Equip.doll.width < 700 &&
      byKey.Equip.doll.height > 400 &&
      byKey.Equip.doll.height < 480,
    JSON.stringify(byKey.Equip.doll),
  );
  check("装备位没有可拖拽（槽位是固定的）", byKey.Equip.draggable === false);
  check("背包取布局设置 10×4", byKey.Inventory.cols === 10 && byKey.Inventory.rows === 4);
  check("方块取布局设置 3×4", byKey.Cube.cols === 3 && byKey.Cube.rows === 4);
  // 腰带按实际坐标自适应：最大 x=15 的一格药水 → 至少 16 列
  check("腰带按实际坐标自适应到 ≥16 列", byKey.Belt.cols >= 16, `实际 ${byKey.Belt.cols}`);

  const beltItems = items.filter((i) => byKey.Belt.match(i));
  check("两瓶腰带药水都归腰带", beltItems.length === 2, `实际 ${beltItems.length}`);
  check("装备位只收 Equipped", items.filter((i) => byKey.Equip.match(i)).length === 1);
  check("背包不收 Stash 物品", items.filter((i) => byKey.Inventory.match(i)).length === 0);
  check("箱子收 Stash 物品", items.filter((i) => byKey.Stash.match(i)).length === 1);
  check("方块收 Cube 物品", items.filter((i) => byKey.Cube.match(i)).length === 1);
}

console.log("=== 2. 物品块的位置与占格（像素级）===");
{
  const CELL = 34;
  // 2×3 的物品放在箱子 (3,4)：left=3*34=102, top=4*34=136, 宽=2*34-2=66, 高=3*34-2=100
  const items = [item({ index: 7, code: "crs", mode: "Stored", storePage: "Stash", invX: 3, invY: 4, width: 2, height: 3 })];
  const html = renderInventory(layout, items, null);
  check("left/top 按格子换算", html.includes(`left:${3 * CELL}px;top:${4 * CELL}px`), html.match(/style="[^"]*"/)?.[0] ?? "");
  check("宽高按占格换算", html.includes(`width:${2 * CELL - 2}px;height:${3 * CELL - 2}px`));
  check("品质样式挂在块上", html.includes("q-superior") === false && html.includes("q-normal"));
  check("块上带物品序号", html.includes('data-item="7"'));

  // **绝不能给物品块加 `draggable`**：WebView2 里设了它的元素会把 click 吞掉，
  // 表现就是"背包里的物品点不开属性弹窗"（用户实际遇到过）。
  // 拖拽已改用指针事件实现，不需要 HTML5 拖放。
  check(
    "物品块没有 draggable 属性（否则点不开弹窗）",
    !html.includes("draggable"),
    html.match(/draggable[^ >]*/)?.[0] ?? "",
  );
}

console.log("=== 3. 越界物品要单独列出而不是消失 ===");
{
  // 背包 10×4，物品放在 (9,3) 且 2×2 → 越界
  const items = [item({ index: 9, code: "gth", storePage: "Inventory", invX: 9, invY: 3, width: 2, height: 2 })];
  const html = renderInventory(layout, items, null);
  check("出现越界提示", html.includes("越界"), "没有越界区块");
  check("越界物品仍带序号（可点开）", html.includes('data-item="9"'));
  check("没有把它画进背包网格", !html.includes(`left:${9 * 34}px;top:${3 * 34}px`));
}

console.log("=== 4. 图标与名字 ===");
{
  setItemIcons([["hp1", "data:image/png;base64,AAAA"]]);
  const withIcon = renderInventory(layout, [item({ index: 1, code: "hp1", storePage: "Inventory" })], null);
  check("有条目的物品显示图标", withIcon.includes('class="blk-icon"') && withIcon.includes("data:image/png;base64,AAAA"));

  const withoutIcon = renderInventory(layout, [item({ index: 2, code: "zzz", storePage: "Inventory" })], null);
  check("没图标的物品退化成代码", withoutIcon.includes('class="blk-code"') && withoutIcon.includes("zzz"));

  check("悬浮提示含序号与品质", (() => {
    const html = renderInventory(layout, [item({ index: 3, storePage: "Inventory" })], null);
    return html.includes("#3") && html.includes("品质");
  })());
}

console.log("=== 5. 装备位按游戏角色装备页布局 ===");
{
  const items = [
    item({ index: 4, code: "xsk", mode: "Equipped", storePage: "Null", bodyLocation: "Head", quality: "Unique" }),
    item({ index: 5, code: "rin", mode: "Equipped", storePage: "Null", bodyLocation: "RightRing", quality: "Rare" }),
  ];
  const html = renderInventory(layout, items, null);

  // 按容器分段统计块数。
  // 注意：要用 `data-grid="`（带引号）定位，否则会被同一标签里的 `data-cols="…"` 干扰。
  const countIn = (key) => {
    const start = html.indexOf(`data-grid="${key}"`);
    if (start < 0) return -1;
    const rest = html.slice(start);
    const next = rest.indexOf('data-grid="', 1);
    const seg = next < 0 ? rest : rest.slice(0, next);
    // 块有两种类名：`class="blk ..."` 与空槽的 `class="blk empty"`
    return (seg.match(/class="blk[ "]/g) ?? []).length;
  };

  // 装备位是槽位型：12 个固定槽框，空槽也画
  check("装备位画出 12 个槽（含空槽）", countIn("Equip") === 12, `实际 ${countIn("Equip")}`);
  check("装备物品只画一次（不重复）", (html.match(/data-item="4"/g) ?? []).length === 1);
  // 其它容器只画"有物品"的块（空格子由 CSS 网格背景表现），所以这里没有物品就是 0
  check(
    "只有装备时其它容器没有块",
    ["Inventory", "Cube", "Stash", "Belt"].every((k) => countIn(k) === 0),
    ["Inventory", "Cube", "Stash", "Belt"].map((k) => `${k}=${countIn(k)}`).join(" "),
  );

  // 槽位是**按真实布局摆放**的，不再是等间距网格：抽出来逐项验证几何关系
  const equipSeg = html.slice(html.indexOf('data-grid="Equip"'), html.indexOf('data-grid="Inventory"'));
  const positions = [
    ...equipSeg.matchAll(/left:(-?\d+)px;top:(-?\d+)px;width:(\d+)px;height:(\d+)px/g),
  ].map((m) => ({ left: +m[1], top: +m[2], w: +m[3], h: +m[4] }));

  check("装备页有 12 个定位块", positions.length === 12, `实际 ${positions.length}`);
  check(
    "不是等间距网格（不同的 x 至少 5 种）",
    new Set(positions.map((p) => p.left)).size >= 5,
    `实际 ${new Set(positions.map((p) => p.left)).size} 种`,
  );

  const [head, , torso, rArm, lArm] = positions;
  check("头在躯干上方", head.top < torso.top, `头 top=${head.top}，躯干 top=${torso.top}`);
  check(
    "左右臂在躯干两侧且同一高度",
    rArm.top === lArm.top && rArm.left < torso.left && lArm.left > torso.left,
    `rArm=${rArm.left} 躯干=${torso.left} lArm=${lArm.left}`,
  );
  check("躯干比头大", torso.h > head.h, `躯干 h=${torso.h}，头 h=${head.h}`);
  check("戒指槽标为估算（虚线）", html.includes("estimated"));
  // 装备页放大约 3 倍（原 215×145 → 约 645×435），所以装备位独占一行、背包在下面
  check(
    "装备页放大约 3 倍（约 645×435）",
    /width:(6[0-9][0-9])px;height:(4[0-9][0-9])px/.test(equipSeg),
    equipSeg.match(/style="width:\d+px;height:\d+px"/)?.[0] ?? "（没找到画布尺寸）",
  );
}

console.log("=== 6. 页面结构（吸顶靠结构保证，不能靠眼睛看） ===");
{
  // 页签条必须**嵌在 header 里**：header 是 sticky，页签作为它的子元素才会
  // 跟着一起停住。两个同级 sticky 各自贴 top 会互相顶掉（表现为页签消失）。
  // 这条只能查 HTML，所以放在这里而不是 JS 渲染断言里。
  const { readFileSync } = await import("node:fs");
  const html = readFileSync(new URL("../web/index.html", import.meta.url), "utf8");

  const headerStart = html.indexOf("<header");
  const headerEnd = html.indexOf("</header>");
  const tabsAt = html.indexOf('id="tabs"');
  check("页签条在 header 内部（吸顶靠结构）", headerStart >= 0 && tabsAt > headerStart && tabsAt < headerEnd,
    `header=${headerStart}..${headerEnd}，tabs=${tabsAt}`);
  // 设置是全局的，不能混在"跟存档有关"的页签里，所以只该有 5 个页签
  check("页签只有 5 个（设置已移出页签）", (html.match(/data-tab="/g) ?? []).length === 5,
    `实际 ${(html.match(/data-tab="/g) ?? []).length} 个`);
  check(
    "tab 面板与页签一一对应",
    ["char", "skills", "quests", "items", "merc"].every((k) => html.includes(`id="tab-${k}"`)),
    "有页签找不到对应面板",
  );
  check("设置改成了工具栏按钮", html.includes('id="btn-settings"'), "缺少设置按钮");
  check("设置弹窗存在（不再占页签）", html.includes('id="settings-modal"') && !html.includes('id="tab-settings"'));
  check(
    "设置弹窗里保留了布局输入项",
    ["l-inv-cols", "l-inv-rows", "l-cube-cols", "l-cube-rows", "l-stash-cols", "l-stash-rows", "l-merc"].every(
      (id) => html.includes(`id="${id}"`),
    ),
    "有输入项丢失",
  );
  // 角色区块里必须有表单子元素：曾经用 innerHTML 覆盖整块，把这些元素删掉，
  // 渲染时报"缺少元素 #f-level"
  check("角色区块保留了表单子元素", html.includes('id="f-level"') && html.includes('id="f-strength"'));
  check("角色区块有容器存在", html.includes('id="skills"') && html.includes('id="inventory"'));

  // 技能页要按游戏的三系技能树渲染（容器得在位，否则分组画不出来）
  check(
    "技能页有技能树容器",
    html.includes('id="skills-head"') && html.includes('id="skills-unclassified"'),
    "缺少 skills-head 或 skills-unclassified",
  );
  check(
    "技能页不再用旧的「显示全部」开关",
    !html.includes('id="show-all-skills"'),
    "旧开关还在，但代码里已移除监听",
  );

  // main.ts 里 `$("#xxx")` 引用的元素必须在 index.html 里存在。
  // 移动/重排区块时最容易漏掉某个 id，而这类问题只在运行时点开对应界面才暴露
  // （把设置从页签搬到弹窗时就差点漏掉）。
  const js = readFileSync(new URL("../web/src/main.ts", import.meta.url), "utf8");
  const referenced = new Set([...js.matchAll(/\$\("#([A-Za-z0-9-]+)"\)/g)].map((m) => m[1]));
  const absent = [...referenced].filter((id) => !html.includes(`id="${id}"`));
  check(
    `main.ts 引用的 ${referenced.size} 个元素都存在`,
    absent.length === 0,
    `缺失: ${absent.join(", ")}`,
  );
}

console.log("=== 6. 属性行：值1 必须真的渲染出来 ===");
{
  // 这段路径出过很难查的 bug：界面上"值"整列空白，而模板产出、单测、
  // 真实 DOM 复现全对。所以这里直接断言**渲染结果里有值**，
  // 而不是只看代码写没写。
  const plain = {
    id: 27,
    layer: 0,
    value: 255,
    name: "法力回复",
    userVisible: true,
    rangeText: "0–255",
    minValue: 0,
    maxValue: 255,
    displayStep: 1,
    hasLayer: false,
    paramOptions: [],
  };
  const row = renderStatRow(plain);
  check("值1 带上了 value 属性", row.includes('value="255"'), row);
  check("值1 是文本输入框（避开 number 的空白渲染）", row.includes('type="text"'));
  check("值1 带上属性 ID 与参数", row.includes('data-stat-id="27"') && row.includes('data-stat-layer="0"'));
  check("普通属性没有值2", !row.includes("data-stat-layer-sel"));
  check("值都是数字，没有 undefined/NaN", !/undefined|NaN/.test(row), row);

  // 带参数位：值2 应给出可选值（用户直接选"法师"）
  const layered = {
    ...plain,
    id: 83,
    layer: 1,
    value: 7,
    name: "法师技能等级",
    hasLayer: true,
    paramLabel: "法师 技能等级 +7",
    paramOptions: [
      { layer: 0, label: "亚马逊" },
      { layer: 1, label: "法师" },
    ],
  };
  const lrow = renderStatRow(layered);
  check("带参数属性有值2 下拉框", lrow.includes("data-stat-layer-sel=\"83\""));
  check("值2 选中当前项", lrow.includes('value="1" selected'), lrow);
  check("值2 列出可选值", lrow.includes("法师") && lrow.includes("亚马逊"));
  check("值1 仍然渲染", lrow.includes('value="7"'));

  // 值为 0 也必须显示成 "0"（不能被当成空）
  const zero = renderStatRow({ ...plain, id: 7, value: 0, name: "生命" });
  check("值为 0 时显示 0 而不是空白", zero.includes('value="0"'), zero);

  // 表头列名：内部名列已去掉，两个值列叫 值1 / 值2
  check("表头是 值1 / 值2", STAT_TABLE_HEAD.includes("<th>值1</th>") && STAT_TABLE_HEAD.includes("<th>值2</th>"));
  check("表头不再有「内部名」列", !STAT_TABLE_HEAD.includes("内部名"));
}

console.log("");
if (failures === 0) {
  console.log("全部通过：容器划分、像素位置、越界处理、图标回退、装备槽都符合预期。");
} else {
  console.log(`${failures} 项失败`);
}
process.exit(failures === 0 ? 0 : 1);
