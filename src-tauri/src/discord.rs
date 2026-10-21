use std::fs::OpenOptions;
use std::io::Write;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use discord_rich_presence::{DiscordIpc, DiscordIpcClient};

use crate::java::pid_is_running;

const BRAND: &str = "WinModing";
/// Official Minecraft Discord application. Voice overlay uses this app's icon
/// (the pickaxe), and the profile shows "Playing Minecraft" — not a launcher.
const MINECRAFT_DISCORD_APP: &str = "356304566206464000";
/// Theseus / Modrinth desktop apps own the Modrinth voice-overlay icon.
const BLOCKED_CLIENTS: &[&str] = &["1123683254248148992", "1190718475832918136"];

#[derive(Clone)]
enum Cmd {
    Playing { instance: String, pid: u32 },
    Clear,
}

static TX: OnceLock<Sender<Cmd>> = OnceLock::new();
static LAST_GAME: Mutex<Option<(u32, String)>> = Mutex::new(None);

fn log_line(message: &str) {
    let Some(dir) = dirs::data_local_dir() else {
        return;
    };
    let path = dir.join(crate::paths::APP_FOLDER).join("discord-rpc.log");
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{message}");
    }
}

fn remember_game(pid: Option<u32>, instance: Option<&str>) {
    if let Ok(mut slot) = LAST_GAME.lock() {
        *slot = match (pid, instance) {
            (Some(pid), Some(name)) if pid != 0 => Some((pid, name.to_string())),
            _ => None,
        };
    }
}

fn started_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or(0)
}

fn secret_text(file_name: &str, env: &str) -> String {
    if let Some(value) = crate::protocol::env_nonempty(env) {
        return value;
    }
    dirs::data_local_dir()
        .and_then(|dir| {
            std::fs::read_to_string(
                dir.join(crate::paths::APP_FOLDER)
                    .join("meta")
                    .join(file_name),
            )
            .ok()
        })
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_default()
}

fn is_blocked_client(id: &str) -> bool {
    let id = id.trim();
    BLOCKED_CLIENTS.contains(&id)
}

fn client_ids() -> Vec<String> {
    let mut ids = Vec::new();
    let custom = secret_text("discord.client_id", "WINMODING_DISCORD_CLIENT_ID");
    if !custom.is_empty() && !is_blocked_client(&custom) {
        ids.push(custom);
    }
    ids.push(MINECRAFT_DISCORD_APP.to_string());
    ids
}

fn clear_blocked_clients() {
    for id in BLOCKED_CLIENTS {
        if let Some(mut client) = connect_one((*id).to_string()) {
            let _ = client.clear_activity();
            log_line(&format!("cleared leftover client_id={id}"));
        }
    }
}

fn shortcut_image() -> Option<String> {
    let configured = secret_text("discord.image", "WINMODING_DISCORD_IMAGE");
    if configured.starts_with("https://") && !configured.to_ascii_lowercase().contains("modrinth") {
        return Some(configured);
    }
    let asset = secret_text("discord.asset", "WINMODING_DISCORD_ASSET");
    if !asset.is_empty() && asset != "modrinth_simple" {
        return Some(asset);
    }
    None
}

fn ipc_available() -> bool {
    for i in 0..10u8 {
        for prefix in [r"\\.\pipe\discord-ipc-", r"\\?\pipe\discord-ipc-"] {
            let path = format!("{prefix}{i}");
            if std::fs::metadata(&path).is_ok() {
                return true;
            }
        }
    }
    false
}

fn connect_one(client_id: String) -> Option<DiscordIpcClient> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut client = match DiscordIpcClient::new(&client_id) {
            Ok(client) => client,
            Err(_) => return,
        };
        if client.connect().is_ok() {
            let _ = tx.send(client);
        }
    });
    rx.recv_timeout(Duration::from_secs(4)).ok()
}

fn send_activity(client: &mut DiscordIpcClient, pid: u32, activity: serde_json::Value) -> bool {
    let payload = serde_json::json!({
        "cmd": "SET_ACTIVITY",
        "args": { "pid": pid, "activity": activity },
        "nonce": uuid::Uuid::new_v4().to_string()
    });
    match client.send(payload, 1) {
        Ok(()) => true,
        Err(err) => {
            log_line(&format!("set_activity failed: {err}"));
            false
        }
    }
}

fn apply_playing(client: &mut DiscordIpcClient, instance: &str, pid: u32, started: i64) -> bool {
    let game_pid = if pid == 0 { std::process::id() } else { pid };
    let mut activity = serde_json::json!({
        "name": "Minecraft",
        "details": instance,
        "state": BRAND,
        "type": 0,
        "timestamps": { "start": started }
    });
    if let Some(image) = shortcut_image() {
        activity["assets"] = serde_json::json!({
            "large_image": image,
            "large_text": BRAND
        });
    }
    if send_activity(client, game_pid, activity.clone()) {
        return true;
    }
    if let Some(map) = activity.as_object_mut() {
        map.remove("assets");
    }
    send_activity(client, game_pid, activity)
}

struct Rpc {
    client: Option<DiscordIpcClient>,
    started: i64,
}

impl Rpc {
    fn try_clients(started: i64) -> Self {
        for client_id in client_ids() {
            match connect_one(client_id.clone()) {
                Some(client) => {
                    log_line(&format!("connected client_id={client_id}"));
                    return Self {
                        client: Some(client),
                        started,
                    };
                }
                None => log_line(&format!("connect failed client_id={client_id}")),
            }
        }
        Self {
            client: None,
            started,
        }
    }

    fn connect() -> Self {
        let started = started_unix();
        if !ipc_available() {
            log_line("discord is not running");
            return Self {
                client: None,
                started,
            };
        }
        Self::try_clients(started)
    }

    fn apply_playing(&mut self, instance: &str, pid: u32) {
        if self.client.is_none() {
            *self = Self::connect();
        }
        let started = self.started;
        let ok = self
            .client
            .as_mut()
            .map(|client| apply_playing(client, instance, pid, started))
            .unwrap_or(false);
        if !ok {
            self.client = None;
        }
    }

    fn disconnect(&mut self) {
        if let Some(mut client) = self.client.take() {
            let _ = client.clear_activity();
        }
    }
}

fn apply_cmd(rpc: &mut Rpc, cmd: &Cmd) {
    match cmd {
        Cmd::Clear => rpc.disconnect(),
        Cmd::Playing { instance, pid } => rpc.apply_playing(instance, *pid),
    }
}

pub fn start() {
    if TX.get().is_some() {
        return;
    }
    let (tx, rx) = mpsc::channel::<Cmd>();
    let _ = TX.set(tx);
    thread::spawn(move || {
        log_line("rpc thread start");
        clear_blocked_clients();
        let mut rpc = Rpc::connect();
        let mut mode = Cmd::Clear;
        loop {
            match rx.recv_timeout(Duration::from_secs(30)) {
                Ok(Cmd::Clear) => {
                    apply_cmd(&mut rpc, &Cmd::Clear);
                    clear_blocked_clients();
                    mode = Cmd::Clear;
                }
                Ok(cmd) => {
                    mode = cmd;
                    apply_cmd(&mut rpc, &mode);
                }
                Err(RecvTimeoutError::Timeout) => {
                    if !matches!(mode, Cmd::Clear) {
                        apply_cmd(&mut rpc, &mode);
                    }
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    });
}

fn send_cmd(cmd: Cmd) {
    start();
    if let Some(tx) = TX.get() {
        let _ = tx.send(cmd);
    }
}

pub fn set_idle() {
    remember_game(None, None);
    send_cmd(Cmd::Clear);
}

pub fn set_playing(instance: &str, pid: u32) {
    remember_game(Some(pid), Some(instance));
    send_cmd(Cmd::Playing {
        instance: instance.to_string(),
        pid,
    });
}

pub fn spawn_watcher_if_playing() {
    let playing = LAST_GAME.lock().ok().and_then(|mut slot| slot.take());
    if let Some((pid, name)) = playing {
        if pid != 0 && pid_is_running(pid) {
            spawn_watcher(pid, &name);
        }
    }
}

pub fn disconnect() {
    send_cmd(Cmd::Clear);
}

pub fn spawn_watcher(pid: u32, instance: &str) {
    disconnect();
    let exe = match std::env::current_exe() {
        Ok(path) => path,
        Err(_) => return,
    };
    let mut command = std::process::Command::new(exe);
    command.args(["--discord-rpc", &pid.to_string(), instance]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000 | 0x0000_0008 | 0x0000_0200);
        command.stdin(std::process::Stdio::null());
        command.stdout(std::process::Stdio::null());
        command.stderr(std::process::Stdio::null());
    }
    let _ = command.spawn();
}

pub fn watch_until_exit(pid: u32, instance: &str) {
    thread::sleep(Duration::from_millis(400));
    let mut rpc = Rpc::connect();
    let playing = Cmd::Playing {
        instance: instance.to_string(),
        pid,
    };
    apply_cmd(&mut rpc, &playing);
    loop {
        thread::sleep(Duration::from_secs(5));
        if pid == 0 || !pid_is_running(pid) {
            rpc.disconnect();
            break;
        }
        apply_cmd(&mut rpc, &playing);
    }
}
