//! The MCP server over a real pipe: initialize, list tools, build a page, render it, save it.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

struct Mcp {
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    next_id: u64,
}

impl Mcp {
    /// Sends a request and returns its result; a `notifications/…` method is sent without an id and returns null.
    fn call(&mut self, method: &str, params: Value) -> Value {
        if method.starts_with("notifications/") {
            writeln!(self.stdin, "{}", json!({"jsonrpc": "2.0", "method": method})).unwrap();
            self.stdin.flush().unwrap();
            return Value::Null;
        }
        self.next_id += 1;
        let msg = json!({"jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params});
        writeln!(self.stdin, "{msg}").unwrap();
        self.stdin.flush().unwrap();
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        let v: Value = serde_json::from_str(&line).expect("a JSON-RPC reply");
        assert_eq!(v["id"], json!(self.next_id), "reply {line}");
        assert!(v.get("error").is_none(), "error reply: {line}");
        v["result"].clone()
    }
}

#[test]
fn mcp_over_stdio() {
    let dir = std::env::temp_dir().join(format!("newpub-agent-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_newpub-agent"))
        .arg("--bundled-fonts")
        .current_dir(&dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("start newpub-agent");
    let mut mcp =
        Mcp { stdin: child.stdin.take().unwrap(), stdout: BufReader::new(child.stdout.take().unwrap()), next_id: 0 };
    let mut call = |method: &str, params: Value| mcp.call(method, params);

    let init = call(
        "initialize",
        json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "0"}}),
    );
    assert_eq!(init["protocolVersion"], "2025-06-18");
    assert_eq!(init["serverInfo"]["name"], "newpub");
    // A notification gets no reply; the next request's reply must still line up.
    call("notifications/initialized", Value::Null);
    let tools = call("tools/list", json!({}));
    let names: Vec<&str> = tools["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"newpub_action") && names.contains(&"newpub_render_page"), "{names:?}");

    let r = call("tools/call", json!({"name": "newpub_new", "arguments": {"width": "8.5in", "height": "11in"}}));
    assert_eq!(r["isError"], false);
    let r = call(
        "tools/call",
        json!({"name": "newpub_action", "arguments": {"action": "add_text_frame",
        "args": {"page": 0, "rect": {"x": 72, "y": 72, "w": 300, "h": 200}}}}),
    );
    assert_eq!(r["isError"], false, "{r}");
    let frame = r["structuredContent"]["created"][0].as_u64().expect("created id");
    let r = call(
        "tools/call",
        json!({"name": "newpub_actions", "arguments": {"actions": [
            {"action": "insert_text", "args": {"target": frame, "text": "Hello from the agent"}},
            {"action": "format_chars", "args": {"target": frame, "start": 0, "end": 5, "attrs": {"bold": true}}}
        ]}}),
    );
    assert_eq!(r["isError"], false, "{r}");
    let r = call(
        "tools/call",
        json!({"name": "newpub_query", "arguments": {"query": "story_text", "args": {"target": frame}}}),
    );
    assert_eq!(r["structuredContent"], json!("Hello from the agent"), "{r}");
    // One undo step for the whole batch.
    call("tools/call", json!({"name": "newpub_action", "arguments": {"action": "undo"}}));
    let r = call(
        "tools/call",
        json!({"name": "newpub_query", "arguments": {"query": "story_text", "args": {"target": frame}}}),
    );
    assert_eq!(r["structuredContent"], json!(""), "{r}");
    call("tools/call", json!({"name": "newpub_action", "arguments": {"action": "redo"}}));

    let r = call("tools/call", json!({"name": "newpub_render_page", "arguments": {"page": 0, "dpi": 36}}));
    let img = &r["content"][1];
    assert_eq!(img["type"], "image");
    assert_eq!(img["mimeType"], "image/png");
    assert!(img["data"].as_str().unwrap().len() > 100);
    assert_eq!(r["structuredContent"]["width"], 306);

    let r = call("tools/call", json!({"name": "newpub_status", "arguments": {}}));
    assert_eq!(r["structuredContent"]["page_count"], 1);
    assert_eq!(r["structuredContent"]["pages"][0]["objects"][0]["text"], "Hello from the agent");

    let r = call("tools/call", json!({"name": "newpub_save", "arguments": {"path": "agent.newspub"}}));
    assert_eq!(r["isError"], false, "{r}");
    assert!(dir.join("agent.newspub").is_file());
    // Errors are tool results, not protocol errors.
    let r = call("tools/call", json!({"name": "newpub_action", "arguments": {"action": "no_such_action"}}));
    assert_eq!(r["isError"], true);
    assert!(r["content"][0]["text"].as_str().unwrap().contains("newpub_reference"));
    let r = call("tools/call", json!({"name": "newpub_reference", "arguments": {"search": "link_frames"}}));
    assert!(r["content"][0]["text"].as_str().unwrap().contains("### `link_frames`"));

    drop(mcp);
    let status = child.wait().unwrap();
    assert!(status.success());
    let _ = std::fs::remove_dir_all(&dir);
}
