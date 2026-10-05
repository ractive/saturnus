//! End-to-end tests of the MCP server against real ROMs, gated by
//! `SATURNUS_ROM_DIR` (see `kb/docs/test-policy.md`): an rmcp client talks
//! to the server over an in-process duplex pipe, boots the 48SX, computes
//! 6 x 7 on the keyboard, reads the stack over Kermit and fetches the
//! screen as a PNG; the same Kermit path on the 49G; and the speed
//! benchmark against real-hardware timings (build with `--features
//! profile` to also print the executed-instruction profile).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Instant;

use base64::Engine;
use rmcp::model::{CallToolRequestParams, CallToolResult, ContentBlock};
use rmcp::service::{RoleClient, RunningService};
use rmcp::{ServiceExt, serde_json};
use saturnus::Model;
use saturnus_drive::session::Limits;
use saturnus_mcp::emulator::Emulator;
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

#[tokio::test(flavor = "multi_thread")]
async fn hp49g_kermit_host_command_and_stack() {
    let Some(rom) = rom("rom.49g") else { return };
    let client = connect().await;
    let boot = ok(call(
        &client,
        "boot",
        serde_json::json!({"model": "49g", "rom_path": rom, "autostart": true}),
    )
    .await);
    assert!(text_of(&boot).contains("booted 49g"), "{}", text_of(&boot));
    let r = ok(call(
        &client,
        "run_command",
        serde_json::json!({"command": "6 7 *"}),
    )
    .await);
    assert!(text_of(&r).contains("1: 42"), "{}", text_of(&r));
    let stack = ok(call(&client, "read_stack", serde_json::json!({})).await);
    assert_eq!(text_of(&stack), "1: 42");
    client.cancel().await.unwrap();
}

/// The HP Museum summation benchmark (wiki:
/// sources/hpmuseum-summation-benchmark, thread
/// <https://www.hpmuseum.org/forum/thread-9750.html>), timed with TICKS
/// (1/8192 s) through the Kermit host command, unpaced. Real-hardware
/// times from the thread:
/// - 48SX, sum function, n = 1000: 95.5 s (Bob Prosperi, post 135).
/// - 48GX, sum function, n = 100: 5.9 s (first-post summary).
/// - 49G ROM 2.10, FOR/NEXT in radians, n = 100: 5.5 s (post 195).
///
/// The 48SX runs n = 1000 because that is the measured figure (n = 100
/// costs about 5% more than a tenth of it, start-up included). On the
/// 49G the loop uses real numbers: with exact integers the ROM computes
/// symbolically and never gets near 5.5 s. Each figure must be met within
/// 5%; the calibration is `Model::cycle_scale_permille`.
#[test]
fn summation_benchmark_matches_real_hardware() {
    let Some(dir) = std::env::var_os("SATURNUS_ROM_DIR") else {
        eprintln!("SATURNUS_ROM_DIR not set: skipping the speed benchmark");
        return;
    };
    let sigma = |n: u32| {
        format!("RAD TICKS '\\GS(X=1,{n},XROOT(3,EXP(SIN(ATAN(X)))))' EVAL SWAP TICKS SWAP -")
    };
    let for_next =
        |n: u32| format!("RAD TICKS 0. 1. {n}. FOR X X ATAN SIN EXP 3. INV ^ + NEXT TICKS ROT -");
    let cases = [
        (Model::Hp48sx, "sxrom-j", sigma(1000), "1395.3462877", 95.5),
        (Model::Hp48gx, "gxrom-r", sigma(100), "139.297187047", 5.9),
        (
            Model::Hp49g,
            "rom-2.10.49g",
            for_next(100),
            "139.297187047",
            5.5,
        ),
    ];
    let mut failures = Vec::new();
    for (model, rom, program, sum, real_s) in cases {
        let path = std::path::Path::new(&dir).join(rom);
        // The default 6 s Kermit timeout: the link does not count time the
        // calculator spends computing (n = 1000 takes 95 s).
        let (mut emu, _) = Emulator::boot(model, &path, true, Limits::default()).unwrap();
        #[cfg(feature = "profile")]
        emu.with_machine(|m| m.profile = Default::default())
            .unwrap();
        let reply = emu.run_command(&program).unwrap();
        #[cfg(feature = "profile")]
        eprintln!("{}", emu.with_machine(|m| m.profile.report()).unwrap());
        assert_eq!(reply.error, None, "{model:?}: {program}");
        assert_eq!(
            reply.levels.get(1).map(String::as_str),
            Some(sum),
            "{model:?}"
        );
        let ticks_text = reply.levels[0].trim_start_matches("# ");
        let ticks: f64 = match ticks_text.strip_suffix('d') {
            Some(dec) => dec.parse().unwrap(),
            None => {
                let hex = ticks_text.strip_suffix('h').unwrap();
                u64::from_str_radix(hex, 16).unwrap() as f64
            }
        };
        let seconds = ticks / 8192.0;
        let error = seconds / real_s - 1.0;
        eprintln!(
            "{model:?}: {ticks} ticks = {seconds:.2} s, real {real_s} s ({:+.1}%)",
            100.0 * error
        );
        if error.abs() > 0.05 {
            failures.push(format!("{model:?} {seconds:.2} s vs {real_s} s"));
        }
    }
    assert!(failures.is_empty(), "outside 5%: {failures:?}");
}

/// `type_text "HELLO WORLD"` on the 39G and 40G (same ROM image, 40G
/// hardware profile): the HOME edit line must show the text. Golden
/// screens `tests/golden/{39g,40g}-hello-world.txt` (the 40G's menu has
/// a CAS key); `SATURNUS_BLESS=1` rewrites them.
#[test]
fn aplet_models_type_hello_world() {
    let Some(rom) = rom("rom.39g") else { return };
    for model in [Model::Hp39g, Model::Hp40g] {
        let (mut emu, _) =
            Emulator::boot(model, std::path::Path::new(&rom), false, Limits::default()).unwrap();
        let report = emu.type_text("HELLO WORLD").unwrap();
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        let (screen, _) = emu.screen_text().unwrap();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("tests/golden/{}-hello-world.txt", model.name()));
        if std::env::var_os("SATURNUS_BLESS").is_some() {
            std::fs::write(&path, &screen).unwrap();
            continue;
        }
        let want = std::fs::read_to_string(&path).unwrap();
        assert!(screen == want, "{model:?} screen:\n{screen}");
    }
}
