#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--discord-rpc") {
        let pid = args
            .get(2)
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        let name = args.get(3).cloned().unwrap_or_else(|| "Minecraft".into());
        winmoding_lib::run_discord_watch(pid, &name);
        return;
    }
    winmoding_lib::run();
}
