//! The real `opendraft-mcp` binary, started and spoken to as an MCP client
//! would over stdio.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

pub fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("od-mcp-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

pub struct Sidecar {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Sidecar {
    pub fn start(dir: &PathBuf, extra: &[&str]) -> Sidecar {
        let mut child = Command::new(env!("CARGO_BIN_EXE_opendraft-mcp"))
            .args(extra)
            .env("OD_AUTOMATION_DIR", dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Sidecar {
            child,
            stdin,
            stdout,
            next_id: 1,
        }
    }

    pub fn send(&mut self, message: &Value) {
        writeln!(self.stdin, "{message}").unwrap();
        self.stdin.flush().unwrap();
    }

    /// Every line the server writes to stdout must be a JSON-RPC message.
    pub fn receive(&mut self) -> Value {
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        let message: Value = serde_json::from_str(line.trim()).unwrap_or_else(|e| {
            panic!("stdout carried something that is not JSON ({e}): {line:?}")
        });
        assert_eq!(message["jsonrpc"], "2.0", "{message}");
        message
    }

    pub fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let reply = self.receive();
        assert_eq!(reply["id"], id, "{reply}");
        reply
    }

    pub fn initialise(&mut self) -> Value {
        let reply = self.request(
            "initialize",
            json!({"protocolVersion": "2025-11-25", "capabilities": {},
                   "clientInfo": {"name": "contract-test", "version": "0"}}),
        );
        self.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        reply
    }

    pub fn call(&mut self, tool: &str, arguments: Value) -> Value {
        let reply = self.request("tools/call", json!({"name": tool, "arguments": arguments}));
        reply["result"].clone()
    }
}

impl Drop for Sidecar {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
