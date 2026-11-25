fn main() {
    // Declare the app's own IPC commands so the ACL generates their
    // permissions (`allow-provider-msg`), which the capsule capability
    // references (`capabilities/capsule-provider.json`).
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&["provider_msg"])),
    )
    .expect("tauri build");
}
