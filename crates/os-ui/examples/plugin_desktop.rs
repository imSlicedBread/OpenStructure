#[path = "support/plugin_desktop.rs"]
mod harness;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    harness::run(false)
}
