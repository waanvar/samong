//! MCP server over stdio — plug banyan into Claude Code / Claude Desktop:
//! stdout carries only protocol messages; anything else goes to stderr.

use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = banyan::mcp::handle_line(&line) {
            if writeln!(stdout, "{response}")
                .and_then(|_| stdout.flush())
                .is_err()
            {
                break; // client closed the pipe
            }
        }
    }
}
