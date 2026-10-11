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
    "装备位用装备页布局（约 215×145）",
    byKey.Equip.doll !== undefined &&
      byKey.Equip.doll.width > 200 &&
      byKey.Equip.doll.width < 230 &&
      byKey.Equip.doll.height > 135 &&
      byKey.Equip.doll.height < 155,
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
  check(
    "装备页画布约 215×145",
    /width:(21[0-9])px;height:(14[0-9])px/.test(equipSeg),
    equipSeg.match(/style="width:\d+px;height:\d+px"/)?.[0] ?? "（没找到画布尺寸）",
  );
}

console.log("");
if (failures === 0) {
  console.log("全部通过：容器划分、像素位置、越界处理、图标回退、装备槽都符合预期。");
} else {
  console.log(`${failures} 项失败`);
}
process.exit(failures === 0 ? 0 : 1);
