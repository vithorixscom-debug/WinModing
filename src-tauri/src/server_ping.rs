//! Native Minecraft Java Server List Ping (SLP). Works in RF without mcsrvstat.

use std::net::{Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::error::LauncherError;

const CONNECT_MS: u64 = 2200;
const IO_MS: u64 = 2800;
const DEFAULT_PORT: u16 = 25565;

fn write_varint(buf: &mut Vec<u8>, value: i32) {
    // Minecraft VarInt uses unsigned logical shifts (Java `>>>`), not arithmetic.
    let mut value = value as u32;
    loop {
        let mut temp = value & 0x7F;
        value >>= 7;
        if value != 0 {
            temp |= 0x80;
        }
        buf.push(temp as u8);
        if value == 0 {
            break;
        }
    }
}

fn write_string(buf: &mut Vec<u8>, value: &str) {
    let bytes = value.as_bytes();
    write_varint(buf, bytes.len() as i32);
    buf.extend_from_slice(bytes);
}

fn write_packet(out: &mut Vec<u8>, packet_id: i32, payload: &[u8]) {
    let mut inner = Vec::with_capacity(payload.len() + 5);
    write_varint(&mut inner, packet_id);
    inner.extend_from_slice(payload);
    write_varint(out, inner.len() as i32);
    out.extend_from_slice(&inner);
}

async fn read_varint(stream: &mut TcpStream) -> Result<i32, LauncherError> {
    let mut num_read = 0u32;
    let mut result = 0i32;
    loop {
        let mut byte = [0u8; 1];
        stream
            .read_exact(&mut byte)
            .await
            .map_err(|err| LauncherError::Message(err.to_string()))?;
        let value = byte[0] as i32;
        result |= (value & 0x7F) << (7 * num_read);
        num_read += 1;
        if num_read > 5 {
            return Err(LauncherError::Message("bad varint".into()));
        }
        if (value & 0x80) == 0 {
            break;
        }
    }
    Ok(result)
}

fn split_host_port(address: &str) -> (String, u16) {
    let trimmed = address.trim();
    if let Some((host, port)) = trimmed.rsplit_once(':') {
        if !host.is_empty()
            && !host.contains(']')
            && port.chars().all(|c| c.is_ascii_digit())
        {
            if let Ok(p) = port.parse::<u16>() {
                return (host.to_string(), p);
            }
        }
    }
    if let Some(rest) = trimmed.strip_prefix('[') {
        if let Some((host, port_part)) = rest.split_once("]:") {
            if let Ok(p) = port_part.parse::<u16>() {
                return (host.to_string(), p);
            }
        }
    }
    (trimmed.to_string(), DEFAULT_PORT)
}

/// Prefer IPv4 — many RF ISPs break Minecraft over AAAA.
async fn resolve_targets(host: &str, port: u16) -> Result<Vec<SocketAddr>, LauncherError> {
    let keyed = format!("{host}:{port}");
    let addrs: Vec<SocketAddr> = tokio::net::lookup_host(&keyed)
        .await
        .map_err(|err| LauncherError::Message(err.to_string()))?
        .collect();
    if addrs.is_empty() {
        return Err(LauncherError::Message(format!("cannot resolve {keyed}")));
    }
    let mut v4: Vec<SocketAddr> = addrs.iter().copied().filter(|a| a.is_ipv4()).collect();
    let v6: Vec<SocketAddr> = addrs.iter().copied().filter(|a| a.is_ipv6()).collect();
    if v4.is_empty() {
        v4 = v6;
    }
    // Cap attempts so one dead host doesn't burn the whole UI.
    v4.truncate(3);
    Ok(v4)
}

/// Minimal DNS SRV lookup for `_minecraft._tcp.<host>`.
async fn lookup_minecraft_srv(host: &str) -> Option<(String, u16)> {
    let name = format!("_minecraft._tcp.{host}");
    let query = build_srv_query(&name)?;
    let sock = tokio::net::UdpSocket::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)))
        .await
        .ok()?;
    // System / common public resolvers — first success wins.
    let resolvers = [
        "172.19.0.2:53",
        "1.1.1.1:53",
        "8.8.8.8:53",
        "77.88.8.8:53",
    ];
    for resolver in resolvers {
        let _ = sock.connect(resolver).await.ok();
        if sock.send(&query).await.is_err() {
            continue;
        }
        let mut buf = [0u8; 512];
        let Ok(Ok(n)) = timeout(Duration::from_millis(900), sock.recv(&mut buf)).await else {
            continue;
        };
        if let Some(target) = parse_srv_answer(&buf[..n]) {
            return Some(target);
        }
    }
    None
}

fn build_srv_query(name: &str) -> Option<Vec<u8>> {
    let mut packet = Vec::with_capacity(64);
    packet.extend_from_slice(&[0x12, 0x34]); // id
    packet.extend_from_slice(&[0x01, 0x00]); // recursion desired
    packet.extend_from_slice(&[0x00, 0x01]); // qdcount
    packet.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
    for label in name.trim_end_matches('.').split('.') {
        let bytes = label.as_bytes();
        if bytes.is_empty() || bytes.len() > 63 {
            return None;
        }
        packet.push(bytes.len() as u8);
        packet.extend_from_slice(bytes);
    }
    packet.push(0);
    packet.extend_from_slice(&[0x00, 0x21]); // SRV
    packet.extend_from_slice(&[0x00, 0x01]); // IN
    Some(packet)
}

fn parse_dns_name(data: &[u8], mut offset: usize) -> Option<(String, usize)> {
    let mut labels = Vec::new();
    let mut jumped = false;
    let mut return_at = offset;
    let mut guard = 0;
    loop {
        if offset >= data.len() || guard > 20 {
            return None;
        }
        guard += 1;
        let len = data[offset];
        if len == 0 {
            offset += 1;
            if !jumped {
                return_at = offset;
            }
            break;
        }
        if len & 0xC0 == 0xC0 {
            if offset + 1 >= data.len() {
                return None;
            }
            let ptr = (((len as usize) & 0x3F) << 8) | data[offset + 1] as usize;
            if !jumped {
                return_at = offset + 2;
            }
            offset = ptr;
            jumped = true;
            continue;
        }
        offset += 1;
        let end = offset + len as usize;
        if end > data.len() {
            return None;
        }
        labels.push(String::from_utf8_lossy(&data[offset..end]).into_owned());
        offset = end;
        if !jumped {
            return_at = offset;
        }
    }
    Some((labels.join("."), return_at))
}

fn parse_srv_answer(data: &[u8]) -> Option<(String, u16)> {
    if data.len() < 12 {
        return None;
    }
    let qd = u16::from_be_bytes([data[4], data[5]]) as usize;
    let an = u16::from_be_bytes([data[6], data[7]]) as usize;
    let mut offset = 12usize;
    for _ in 0..qd {
        let (_, next) = parse_dns_name(data, offset)?;
        offset = next + 4; // type + class
    }
    for _ in 0..an {
        let (_, next) = parse_dns_name(data, offset)?;
        offset = next;
        if offset + 10 > data.len() {
            return None;
        }
        let rtype = u16::from_be_bytes([data[offset], data[offset + 1]]);
        let rdlen = u16::from_be_bytes([data[offset + 8], data[offset + 9]]) as usize;
        offset += 10;
        if offset + rdlen > data.len() {
            return None;
        }
        if rtype == 33 && rdlen >= 6 {
            let port = u16::from_be_bytes([data[offset + 4], data[offset + 5]]);
            let (target, _) = parse_dns_name(data, offset + 6)?;
            let target = target.trim_end_matches('.').to_string();
            if !target.is_empty() && port != 0 {
                return Some((target, port));
            }
        }
        offset += rdlen;
    }
    None
}

fn normalize_favicon(raw: Option<&str>) -> Option<String> {
    let value = raw?.trim();
    if value.is_empty() {
        return None;
    }
    if value.starts_with("data:image") {
        return Some(value.to_string());
    }
    // Some servers send bare base64.
    if value.len() > 64 && !value.contains("://") {
        return Some(format!("data:image/png;base64,{value}"));
    }
    None
}

fn parse_status_json(raw: &str) -> Value {
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        return json!({ "online": true, "players": { "online": null, "max": null } });
    };
    let online_players = value
        .pointer("/players/online")
        .and_then(|v| v.as_i64())
        .or_else(|| value.get("onlinePlayers").and_then(|v| v.as_i64()));
    let max_players = value
        .pointer("/players/max")
        .and_then(|v| v.as_i64())
        .or_else(|| value.get("maxPlayers").and_then(|v| v.as_i64()));
    let version = value
        .pointer("/version/name")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let favicon = normalize_favicon(value.get("favicon").and_then(|v| v.as_str()));
    let description = value.get("description").cloned();
    json!({
        "online": true,
        "players": {
            "online": online_players,
            "max": max_players,
        },
        "version": version,
        "motd": description,
        "favicon": favicon,
    })
}

async fn ping_addr(
    host_header: &str,
    port: u16,
    addr: SocketAddr,
) -> Result<(Value, u64), LauncherError> {
    let started = Instant::now();
    let mut stream = timeout(Duration::from_millis(CONNECT_MS), TcpStream::connect(addr))
        .await
        .map_err(|_| LauncherError::Message("connect timeout".into()))?
        .map_err(|err| LauncherError::Message(err.to_string()))?;

    let mut handshake_payload = Vec::new();
    // Modern protocol id; status responses ignore the exact version on most hosts.
    write_varint(&mut handshake_payload, 767);
    write_string(&mut handshake_payload, host_header);
    handshake_payload.extend_from_slice(&port.to_be_bytes());
    write_varint(&mut handshake_payload, 1); // status

    let mut request = Vec::new();
    write_packet(&mut request, 0x00, &handshake_payload);
    write_packet(&mut request, 0x00, &[]);

    timeout(Duration::from_millis(IO_MS), stream.write_all(&request))
        .await
        .map_err(|_| LauncherError::Message("write timeout".into()))?
        .map_err(|err| LauncherError::Message(err.to_string()))?;

    let packet_len = timeout(Duration::from_millis(IO_MS), read_varint(&mut stream))
        .await
        .map_err(|_| LauncherError::Message("read timeout".into()))??;
    if !(1..=1_048_576).contains(&packet_len) {
        return Err(LauncherError::Message("bad packet length".into()));
    }
    let mut packet = vec![0u8; packet_len as usize];
    timeout(Duration::from_millis(IO_MS), stream.read_exact(&mut packet))
        .await
        .map_err(|_| LauncherError::Message("read timeout".into()))?
        .map_err(|err| LauncherError::Message(err.to_string()))?;

    let mut idx = 0usize;
    let (_packet_id, id_len) = read_varint_slice(&packet[idx..])?;
    idx += id_len;
    let (json_len, len_len) = read_varint_slice(&packet[idx..])?;
    idx += len_len;
    if json_len < 0 || idx + json_len as usize > packet.len() {
        return Err(LauncherError::Message("bad status json".into()));
    }
    let raw = String::from_utf8_lossy(&packet[idx..idx + json_len as usize]).into_owned();
    let latency = started.elapsed().as_millis() as u64;
    Ok((parse_status_json(&raw), latency))
}

async fn ping_host_port(host: &str, port: u16) -> Result<(Value, u64), LauncherError> {
    let targets = resolve_targets(host, port).await?;
    let mut last = LauncherError::Message("no addresses".into());
    for addr in targets {
        match ping_addr(host, port, addr).await {
            Ok(ok) => return Ok(ok),
            Err(err) => last = err,
        }
    }
    Err(last)
}

fn read_varint_slice(data: &[u8]) -> Result<(i32, usize), LauncherError> {
    let mut num_read = 0usize;
    let mut result = 0i32;
    for byte in data.iter().copied() {
        let value = byte as i32;
        result |= (value & 0x7F) << (7 * num_read);
        num_read += 1;
        if num_read > 5 {
            return Err(LauncherError::Message("bad varint".into()));
        }
        if (value & 0x80) == 0 {
            return Ok((result, num_read));
        }
    }
    Err(LauncherError::Message("truncated varint".into()))
}

/// Ping a Minecraft Java server. Returns mcsrvstat-compatible JSON (+ optional favicon).
pub async fn ping_java(address: &str) -> Value {
    let (host, port) = split_host_port(address);
    if host.is_empty() || host.len() > 128 {
        return json!({ "online": false });
    }

    // Direct host:port first (fast path for most RF servers).
    let direct = ping_host_port(&host, port).await;
    let result = match direct {
        Ok(ok) => Ok(ok),
        Err(direct_err) => {
            // Fall back to SRV when default port is dead (e.g. some hubs).
            if let Some((srv_host, srv_port)) = lookup_minecraft_srv(&host).await {
                match ping_host_port(&srv_host, srv_port).await {
                    Ok(ok) => Ok(ok),
                    Err(_) => Err(direct_err),
                }
            } else {
                Err(direct_err)
            }
        }
    };

    match result {
        Ok((mut value, latency)) => {
            if let Some(obj) = value.as_object_mut() {
                obj.insert("online".into(), json!(true));
                obj.insert("latency".into(), json!(latency));
                obj.insert("ip".into(), json!(host));
                obj.insert("port".into(), json!(port));
            }
            value
        }
        Err(_) => json!({
            "online": false,
            "ip": host,
            "port": port,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_ports() {
        assert_eq!(
            split_host_port("play.example.com:25566"),
            ("play.example.com".into(), 25566)
        );
        assert_eq!(split_host_port("hypixel.net"), ("hypixel.net".into(), 25565));
    }

    #[test]
    fn favicon_normalizes() {
        assert!(normalize_favicon(Some("data:image/png;base64,aaa"))
            .unwrap()
            .starts_with("data:image"));
        assert!(normalize_favicon(Some(&"a".repeat(80)))
            .unwrap()
            .starts_with("data:image/png;base64,"));
    }

    #[test]
    fn varint_encodes_negative_protocol() {
        let mut buf = Vec::new();
        write_varint(&mut buf, -1);
        assert_eq!(buf, vec![0xFF, 0xFF, 0xFF, 0xFF, 0x0F]);
        buf.clear();
        write_varint(&mut buf, 767);
        assert_eq!(buf, vec![0xFF, 0x05]);
    }
}
