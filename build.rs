use std::env;
use std::path::PathBuf;

fn main() {
    // 设置 LVGL 配置文件路径
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let lv_config_path = PathBuf::from(&manifest_dir).join("lvgl-config");

    // 设置环境变量供 lvgl-sys 使用
    println!("cargo:rustc-env=DEP_LV_CONFIG_PATH={}", lv_config_path.display());

    // 重新运行条件
    println!("cargo:rerun-if-changed=lvgl-config/lv_conf.h");
    println!("cargo:rerun-if-changed=build.rs");
}
