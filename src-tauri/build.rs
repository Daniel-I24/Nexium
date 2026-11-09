fn main() {
    println!("cargo:rerun-if-env-changed=NEXIUM_AI_SERVER_URL");
    tauri_build::build()
}
