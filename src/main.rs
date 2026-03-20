#![forbid(unsafe_code)]

use plugin_coding_pack::CodingPackPlugin;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let plugin = CodingPackPlugin;
    pulse_plugin_sdk::dev_adapter::DevAdapterWithStepExecutor::new(plugin).serve_stdio();
}

#[cfg(target_arch = "wasm32")]
fn main() {}
