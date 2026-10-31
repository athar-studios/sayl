fn main() {
    tauri_build::build();
    // The desktop test harness imports comctl32 v6 entry points
    // (TaskDialogIndirect, SetWindowSubclass). Unlike the Tauri app binary,
    // that harness ships no manifest, so Windows binds comctl32 v5 and the
    // process exits 0xc0000139 before any test runs. lib unit tests are not
    // a [[test]] target, so rustc-link-arg-tests is rejected.
    if std::env::var("CARGO_CFG_TARGET_ENV").ok().as_deref() == Some("msvc") {
        println!(
            "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='Win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
    }
}
