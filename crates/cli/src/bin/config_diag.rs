//! 配置写盘验证：把设置写进**真实默认位置**并读回。
//!
//! 这是给"设置能不能存住"这个问题做一次端到端确认：
//! 之前因为各种受限环境（沙箱、受控文件夹访问）一直写不进去，
//! 而界面上只表现为"设置重开就没了"。
//!
//! 运行前请先关掉正在运行的应用（避免两边同时写）。
//! 输出里会打印**实际写入的路径**——这才是唯一可信的答案。

fn main() {
    println!("=== 配置写盘验证 ===");
    let store = d2r_app::ConfigStore::new();
    println!("首选路径: {}", store.path().display());
    println!("候选顺序:");
    for p in store.candidates() {
        println!("    {}", p.display());
    }

    let Some(writable) = store.writable_path() else {
        println!();
        println!("结论: 所有候选目录都不可写，设置无法保存。");
        println!("      这是环境问题（权限 / 安全软件），需要按上面的候选顺序排查。");
        return;
    };
    println!();
    println!("探测到可写路径: {}", writable.display());

    let mut settings = store.load().unwrap_or_default();
    println!(
        "读到已有设置: inventory={}x{}, books_single_cell={}",
        settings.layout.inventory_cols,
        settings.layout.inventory_rows,
        settings.layout.books_single_cell
    );

    // 用一组好认的值写回去，再读一次确认落盘
    settings.layout.inventory_cols = 17;
    settings.layout.inventory_rows = 9;
    settings.layout.books_single_cell = true;
    settings.layout.confirmed_by_user = true;

    match store.save(&settings) {
        Ok(path) => {
            println!("写入成功: {}", path.display());
            let back = store.load();
            match back {
                Some(s) => {
                    println!(
                        "读回验证: inventory={}x{}, books_single_cell={}, confirmed={}",
                        s.layout.inventory_cols,
                        s.layout.inventory_rows,
                        s.layout.books_single_cell,
                        s.layout.confirmed_by_user
                    );
                    if s.layout.inventory_cols == 17 && s.layout.books_single_cell {
                        println!();
                        println!("结论: 设置能正常保存与读回，重开应用会带上这组值。");
                    } else {
                        println!();
                        println!("结论: 写进去了但读回的值不对，需要进一步排查。");
                    }
                }
                None => println!("结论: 写入自称成功，但读不回来。"),
            }
        }
        Err(e) => {
            println!("写入失败: {e}");
            println!();
            println!("结论: 该路径不可写。请检查安全软件的「受控文件夹访问」是否拦截了本程序。");
        }
    }
}
