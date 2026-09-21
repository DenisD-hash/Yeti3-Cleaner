//! About YETI³ Cleaner.

pub const PRODUCT: &str = "YETI³ Cleaner";
pub const SUBTITLE: &str = "Deep macOS Cleanup";
pub const MOTTO: &str = "CODE. CLEAN. OPTIMIZE. REPEAT.";

pub const STACK: &[&str] = &["Rust", "Golang", "Kubernetes", "Swift / ObjC", "JS"];

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
