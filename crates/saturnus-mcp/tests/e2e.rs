//! End-to-end test of the MCP server against a real ROM, gated by
//! `SATURNUS_ROM_DIR` (see `kb/docs/test-policy.md`): an rmcp client talks
//! to the server over an in-process duplex pipe, boots the 48SX, computes
//! 6 x 7 on the keyboard, reads the stack over Kermit and fetches the
//! screen as a PNG.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Instant;

use base64::Engine;
use rmcp::model::{CallToolRequestParams, CallToolResult, ContentBlock};
use rmcp::service::{RoleClient, RunningService};
use rmcp::{ServiceExt, serde_json};
use saturnus_mcp::server::SaturnusMcp;

/// The ROM `name` in `$SATURNUS_ROM_DIR`, or `None` (test skipped).
fn rom(name: &str) -> Option<String> {
    let Some(dir) = std::env::var_os("SATURNUS_ROM_DIR") else {
        eprintln!("SATURNUS_ROM_DIR not set: skipping MCP ROM test ({name})");
        return None;
    };
    let path = std::path::Path::new(&dir).join(name);
    assert!(path.exists(), "{} missing", path.display());
    Some(path.to_string_lossy().into_owned())
}

async fn connect() -> RunningService<RoleClient, ()> {
    let (server_io, client_io) = tokio::io::duplex(1 << 16);
    tokio::spawn(async move {
        let server = SaturnusMcp::new().serve(server_io).await.unwrap();
        server.waiting().await.unwrap();
    });
    ().serve(client_io).await.unwrap()
}

async fn call(
    client: &RunningService<RoleClient, ()>,
    tool: &'static str,
    args: serde_json::Value,
) -> CallToolResult {
    let start = Instant::now();
    let mut params = CallToolRequestParams::new(tool);
    if let serde_json::Value::Object(map) = args {
        params = params.with_arguments(map);
    }
    let result = client.call_tool(params).await.unwrap();
    eprintln!("{tool}: {:.2} s", start.elapsed().as_secs_f64());
    result
}

fn text_of(r: &CallToolResult) -> String {
    r.content
        .iter()
        .filter_map(|c| c.as_text().map(|t| t.text.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn ok(r: CallToolResult) -> CallToolResult {
    assert_ne!(r.is_error, Some(true), "tool error: {}", text_of(&r));
    r
}

#[tokio::test(flavor = "multi_thread")]
async fn hp48sx_keys_stack_and_screen() {
    let Some(rom) = rom("sxrom-j") else { return };
    let client = connect().await;

    let tools = client.list_all_tools().await.unwrap();
    assert!(tools.iter().any(|t| t.name == "read_stack"));

    let boot = ok(call(
        &client,
        "boot",
        serde_json::json!({"model": "48sx", "rom_path": rom}),
    )
    .await);
    assert!(text_of(&boot).contains("booted 48sx"), "{}", text_of(&boot));

    // Kermit tools need the server; keys are refused while it runs.
    let r = call(&client, "read_stack", serde_json::json!({})).await;
    assert_eq!(r.is_error, Some(true));
    assert!(text_of(&r).contains("start_server"), "{}", text_of(&r));

    let keys = ok(call(
        &client,
        "press_keys",
        serde_json::json!({"script": "6 ENTER 7 * ENTER"}),
    )
    .await);
    assert!(
        text_of(&keys).contains("5 key presses"),
        "{}",
        text_of(&keys)
    );

    ok(call(&client, "start_server", serde_json::json!({})).await);
    let r = call(&client, "press_keys", serde_json::json!({"script": "1"})).await;
    assert_eq!(r.is_error, Some(true));

    // ENTER on an empty command line duplicates level 1.
    let stack = ok(call(&client, "read_stack", serde_json::json!({})).await);
    assert_eq!(text_of(&stack), "2: 42\n1: 42");
    let level1 = ok(call(&client, "read_stack", serde_json::json!({"levels": 1})).await);
    assert_eq!(text_of(&level1), "1: 42");

    let shot = ok(call(&client, "screen", serde_json::json!({"format": "png"})).await);
    let png = shot
        .content
        .iter()
        .find_map(|c| match c {
            ContentBlock::Image(img) => Some(img.clone()),
            _ => None,
        })
        .expect("an image block");
    assert_eq!(png.mime_type, "image/png");
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&png.data)
        .unwrap();
    let reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .unwrap();
    assert_eq!((reader.info().width, reader.info().height), (131, 64));

    client.cancel().await.unwrap();
}
