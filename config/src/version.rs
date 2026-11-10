use std::sync::OnceLock;

static VERSION: OnceLock<&'static str> = OnceLock::new();
static TRIPLE: OnceLock<&'static str> = OnceLock::new();

pub fn assign_version_info(version: &'static str, triple: &'static str) {
    VERSION.set(version).unwrap();
    TRIPLE.set(triple).unwrap();
}

pub fn wezterm_version() -> &'static str {
    VERSION
        .get()
        .unwrap_or(&"someone forgot to call assign_version_info")
}

pub fn wezterm_target_triple() -> &'static str {
    TRIPLE
        .get()
        .unwrap_or(&"someone forgot to call assign_version_info")
}
