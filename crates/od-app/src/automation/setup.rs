//! Helping the user connect an assistant: where the `opendraft-mcp` program
//! is (it is shipped next to the app) and the configuration to give an MCP
//! host so it starts that program.

use std::path::{Path, PathBuf};

/// The sidecar's file name on this system.
fn sidecar_name() -> String {
    format!("opendraft-mcp{}", std::env::consts::EXE_SUFFIX)
}

/// The sidecar beside `app`, if it is there.
pub fn sidecar_beside(app: &Path) -> Option<PathBuf> {
    let candidate = app.parent()?.join(sidecar_name());
    candidate.is_file().then_some(candidate)
}

/// Names the sidecar when it is not beside the app (an unusual install).
const SIDECAR_ENV: &str = "OD_MCP";

/// The sidecar `OD_MCP` names if it is a file, else the one beside the
/// running app.
pub fn sidecar_path() -> Option<PathBuf> {
    let named = std::env::var_os(SIDECAR_ENV)
        .map(PathBuf::from)
        .filter(|p| p.is_file());
    named.or_else(|| sidecar_beside(&std::env::current_exe().ok()?))
}

/// The `mcpServers` entry most MCP hosts (Claude Desktop, Cursor and others)
/// read. The path is written with the escaping JSON needs.
pub fn config_snippet(program: &Path) -> String {
    let config = serde_json::json!({
        "mcpServers": {
            "opendraft": { "command": program.to_string_lossy() }
        }
    });
    serde_json::to_string_pretty(&config).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_snippet_is_json_that_names_the_program_exactly() {
        for path in [
            r"C:\Program Files\OpenDraft\opendraft-mcp.exe",
            "/opt/opendraft/opendraft-mcp",
            "C:\\Users\\jo \"quoted\"\\x\\opendraft-mcp.exe",
        ] {
            let snippet = config_snippet(Path::new(path));
            let parsed: serde_json::Value = serde_json::from_str(&snippet).unwrap();
            assert_eq!(
                parsed["mcpServers"]["opendraft"]["command"], path,
                "{snippet}"
            );
        }
    }

    #[test]
    fn the_sidecar_is_found_only_beside_the_app() {
        let dir = std::env::temp_dir().join(format!("od-setup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let app = dir.join(format!("opendraft{}", std::env::consts::EXE_SUFFIX));
        assert_eq!(sidecar_beside(&app), None);
        let sidecar = dir.join(sidecar_name());
        std::fs::write(&sidecar, b"x").unwrap();
        assert_eq!(sidecar_beside(&app), Some(sidecar));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
