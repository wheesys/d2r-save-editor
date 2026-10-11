//! 打印 item_detail 的 JSON（排查前端拿到 undefined 的问题）。
use d2r_app::Workspace;
fn main() {
    let path = std::env::args().nth(1).expect("存档路径");
    let index: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let mut ws = Workspace::new(d2r_app::default_game_dir());
    ws.open(&path).expect("打开失败");
    let dto = ws.item_detail_dto(index).expect("取明细失败");
    println!("{}", serde_json::to_string(&dto).expect("序列化失败"));
}
