//! Which local ports are listening, read from the operating system's socket
//! table. Ledgeling never connects to a port to find out: coding mode opens no
//! sockets at all.
use serde::Serialize;
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    process::Command,
};

#[derive(Clone, Serialize, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub struct Listener {
    pub port: u16,
    pub pid: u32,
    pub process: String,
    /// Loopback only, or reachable from the network (`*`, `0.0.0.0`).
    pub exposed: bool,
}

fn port_of(address: &str) -> Option<(u16, bool)> {
    let (host, port) = address.rsplit_once(':')?;
    let exposed = matches!(host, "*" | "0.0.0.0" | "[::]" | "::");
    Some((port.parse().ok()?, exposed))
}

/// `lsof -nP -iTCP -sTCP:LISTEN -F pcn`, one field per line.
pub fn parse_lsof(text: &str) -> Vec<Listener> {
    let mut out = BTreeSet::new();
    let (mut pid, mut process) = (0u32, String::new());
    for line in text.lines() {
        let (tag, value) = line.split_at(line.len().min(1));
        match tag {
            "p" => pid = value.parse().unwrap_or(0),
            "c" => process = value.to_string(),
            "n" => {
                if let Some((port, exposed)) = port_of(value) {
                    out.insert(Listener {
                        port,
                        pid,
                        process: process.clone(),
                        exposed,
                    });
                }
            }
            _ => {}
        }
    }
    merge(out)
}

/// `netstat -ano -p tcp` on Windows; process names come from `tasklist`.
pub fn parse_netstat(text: &str, names: &HashMap<u32, String>) -> Vec<Listener> {
    let mut out = BTreeSet::new();
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() == 5 && fields[0].starts_with("TCP") && fields[3] == "LISTENING" {
            let (Some((port, exposed)), Ok(pid)) = (port_of(fields[1]), fields[4].parse()) else {
                continue;
            };
            out.insert(Listener {
                port,
                pid,
                process: names.get(&pid).cloned().unwrap_or_default(),
                exposed,
            });
        }
    }
    merge(out)
}

/// IPv4 and IPv6 sockets for one port and process are one listener.
fn merge(listeners: BTreeSet<Listener>) -> Vec<Listener> {
    let mut by_key: HashMap<(u16, u32), Listener> = HashMap::new();
    for listener in listeners {
        by_key
            .entry((listener.port, listener.pid))
            .and_modify(|l| l.exposed |= listener.exposed)
            .or_insert(listener);
    }
    let mut out: Vec<_> = by_key.into_values().collect();
    out.sort();
    out
}

pub fn parse_tasklist(text: &str) -> HashMap<u32, String> {
    text.lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split("\",\"").collect();
            let name = fields.first()?.trim_matches('"').to_string();
            let pid = fields.get(1)?.trim_matches('"').parse().ok()?;
            Some((pid, name))
        })
        .collect()
}

#[cfg_attr(not(windows), allow(unused_mut))]
fn quiet(program: &str) -> Command {
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    command
}

fn output(command: &mut Command) -> Option<String> {
    let output = command.output().ok()?;
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn listeners() -> Result<Vec<Listener>, String> {
    if cfg!(windows) {
        let names = output(quiet("tasklist").args(["/FO", "CSV", "/NH"]))
            .map(|t| parse_tasklist(&t))
            .unwrap_or_default();
        let text = output(quiet("netstat").args(["-ano", "-p", "tcp"]))
            .ok_or("Could not read the list of open ports.")?;
        let mut all = parse_netstat(&text, &names);
        if let Some(v6) = output(quiet("netstat").args(["-ano", "-p", "tcpv6"])) {
            all.extend(parse_netstat(&v6, &names));
        }
        Ok(merge(all.into_iter().collect()))
    } else {
        let lsof = ["/usr/sbin/lsof", "/usr/bin/lsof", "lsof"]
            .into_iter()
            .find(|p| !p.starts_with('/') || std::path::Path::new(p).exists())
            .unwrap_or("lsof");
        let text = output(quiet(lsof).args(["-nP", "-iTCP", "-sTCP:LISTEN", "-F", "pcn"]))
            .ok_or("Could not read the list of open ports (lsof is missing).")?;
        Ok(parse_lsof(&text))
    }
}

/// Every process descended from `roots`, including the roots, so a port held
/// by `node` under `npm run dev` is credited to the Run tab that started it.
pub fn descendants(roots: &[u32]) -> HashSet<u32> {
    let mut out: HashSet<u32> = roots.iter().copied().collect();
    let pairs = parent_pairs();
    loop {
        let before = out.len();
        for (pid, parent) in &pairs {
            if out.contains(parent) {
                out.insert(*pid);
            }
        }
        if out.len() == before {
            break;
        }
    }
    out
}

fn parent_pairs() -> Vec<(u32, u32)> {
    let text = if cfg!(windows) {
        output(quiet("powershell").args([
            "-NoProfile",
            "-Command",
            "Get-CimInstance Win32_Process | ForEach-Object { \"$($_.ProcessId) $($_.ParentProcessId)\" }",
        ]))
    } else {
        output(quiet("ps").args(["-A", "-o", "pid=,ppid="]))
    };
    parse_pairs(&text.unwrap_or_default())
}

pub fn parse_pairs(text: &str) -> Vec<(u32, u32)> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsof_output_is_parsed_and_merged() {
        let text =
            "p501\ncnode\nf23\nn127.0.0.1:5173\nf24\nn[::1]:5173\np88\ncpython3\nf5\nn*:8000\n";
        let found = parse_lsof(text);
        assert_eq!(
            found,
            vec![
                Listener {
                    port: 5173,
                    pid: 501,
                    process: "node".into(),
                    exposed: false
                },
                Listener {
                    port: 8000,
                    pid: 88,
                    process: "python3".into(),
                    exposed: true
                },
            ]
        );
    }

    #[test]
    fn netstat_output_is_parsed() {
        let names = parse_tasklist("\"node.exe\",\"4242\",\"Console\",\"1\",\"50,000 K\"\n");
        let text = "\n  Proto  Local Address  Foreign Address  State  PID\n  TCP    0.0.0.0:3000   0.0.0.0:0   LISTENING   4242\n  TCP    127.0.0.1:50000  127.0.0.1:3000  ESTABLISHED  7\n";
        let found = parse_netstat(text, &names);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].port, 3000);
        assert_eq!(found[0].process, "node.exe");
        assert!(found[0].exposed);
    }

    #[test]
    fn process_pairs_are_parsed() {
        assert_eq!(parse_pairs("  1  0\n 42 1\nbad\n"), vec![(1, 0), (42, 1)]);
    }
}
