//! `opendraft-mcp`: speaks MCP on stdin/stdout and talks to a running
//! OpenDraft over its local automation socket. stdout carries protocol
//! messages only; anything for a human goes to stderr (or, for `--check`,
//! which does not speak MCP, stdout).

mod client;
mod doctor;
mod server;
mod tools;

use std::io::Write;
use std::process::ExitCode;

use rmcp::transport::stdio;
use rmcp::ServiceExt;

const USAGE: &str = "opendraft-mcp [--instance <id>] [--check]
Speaks MCP on stdio. Turn the server on in OpenDraft (Settings > Automation) first.
  --instance <id>  use this OpenDraft when several are running
  --check          say whether a running OpenDraft can be reached and what it would
                   allow, then exit (0 if one can be used)
  --version        print the version";

fn say(message: &str) {
    let _ = writeln!(std::io::stderr(), "{message}");
}

/// "1 instance", "3 instances".
fn plural(n: usize) -> String {
    format!("{n} instance{}", if n == 1 { "" } else { "s" })
}

fn main() -> ExitCode {
    let mut wanted = None;
    let mut check = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--instance" => wanted = args.next(),
            "--check" => check = true,
            "--version" => {
                say(env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            "--help" | "-h" => {
                say(USAGE);
                return ExitCode::SUCCESS;
            }
            other => {
                say(&format!("Unknown argument {other:?}.\n{USAGE}"));
                return ExitCode::from(2);
            }
        }
    }
    if check {
        let dir = od_api::instance::directory();
        let (text, usable) = doctor::report(dir.as_deref(), wanted.as_deref());
        let _ = write!(std::io::stdout(), "{text}");
        return if usable {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        };
    }
    let Some(dir) = od_api::instance::directory() else {
        say("Cannot tell where OpenDraft keeps its instance files on this system.");
        return ExitCode::FAILURE;
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            say(&format!("Cannot start the runtime: {error}"));
            return ExitCode::FAILURE;
        }
    };
    let server = server::Server::new(client::Client::new(dir, wanted));
    let outcome = runtime.block_on(async {
        let running = server.serve(stdio()).await.map_err(|e| e.to_string())?;
        running.waiting().await.map_err(|e| e.to_string())?;
        Ok::<_, String>(())
    });
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            say(&format!("MCP server stopped: {error}"));
            ExitCode::FAILURE
        }
    }
}
