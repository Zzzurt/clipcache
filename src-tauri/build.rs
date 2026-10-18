fn main() {
    // 图标换了必须让 Windows 资源重编一遍：cargo 默认不会因为 icons/*.png 变化而
    // 重跑 build script，于是 exe 里嵌着的仍是旧 ico（改了图标、构建也绿，但
    // 任务栏/资源管理器还是老图标）。这里把图标目录纳入重跑条件。
    println!("cargo:rerun-if-changed=icons");
    for entry in std::fs::read_dir("icons").into_iter().flatten().flatten() {
        println!("cargo:rerun-if-changed={}", entry.path().display());
    }
    tauri_build::build()
}
