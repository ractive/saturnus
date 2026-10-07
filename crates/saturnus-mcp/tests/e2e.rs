//! End-to-end tests of the MCP server against real ROMs, gated by
//! `SATURNUS_ROM_DIR` (see `kb/docs/test-policy.md`): an rmcp client talks
//! to the server over an in-process duplex pipe, boots the 48SX, computes
//! 6 x 7 on the keyboard, reads the stack over Kermit and fetches the
//! screen as a PNG; the same Kermit path on the 49G; the semantic tools
//! (eval, the typed stack, push/pop, variables) on the 48SX, 48GX and 49G
//! and their refusal on the 39G; and the speed benchmark against
//! real-hardware timings (build with `--features profile` to also print
//! the executed-instruction profile).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Instant;

use base64::Engine;
use rmcp::model::{CallToolRequestParams, CallToolResult, ContentBlock};
use rmcp::service::{RoleClient, RunningService};
use rmcp::{ServiceExt, serde_json};
use saturnus::Model;
use saturnus_drive::session::Limits;
use saturnus_mcp::emulator::Emulator;
use saturnus_mcp::object::Object;
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

    // Keys leave server mode by themselves.
    let keys = ok(call(
        &client,
        "press_keys",
        serde_json::json!({"script": "backspace"}),
    )
    .await);
    assert!(
        text_of(&keys).contains("left Kermit server mode first"),
        "{}",
        text_of(&keys)
    );

    client.cancel().await.unwrap();
}

/// A semantic tool's JSON result.
fn json_of(r: &CallToolResult) -> serde_json::Value {
    serde_json::from_str(&text_of(r)).unwrap_or_else(|e| panic!("{e}: {}", text_of(r)))
}

/// `eval` and level 1 of its typed result.
async fn eval1(client: &RunningService<RoleClient, ()>, source: &str) -> serde_json::Value {
    let r = ok(call(
        client,
        "eval",
        serde_json::json!({"source": source, "keep_server": true}),
    )
    .await);
    let v = json_of(&r);
    assert_eq!(v["server"], "running", "{v}");
    v["levels"][0].clone()
}

fn real(v: &serde_json::Value) -> f64 {
    assert_eq!(v["type"], "real", "{v}");
    v["value"].as_f64().unwrap_or_else(|| panic!("{v}"))
}

/// eval on the 48SX: the plan's cases (typed results, the acceptance
/// SIN(0.5) in RAD, errors), the typed stack after a few evals, and keys
/// right after eval without stop_server.
#[tokio::test(flavor = "multi_thread")]
async fn hp48sx_eval_and_typed_stack() {
    let Some(rom) = rom("sxrom-j") else { return };
    let client = connect().await;
    ok(call(
        &client,
        "boot",
        serde_json::json!({"model": "48sx", "rom_path": rom}),
    )
    .await);

    // No knowledge of server mode needed: eval enters and leaves it.
    let r = ok(call(&client, "eval", serde_json::json!({"source": "2 3 +"})).await);
    let v = json_of(&r);
    assert_eq!(real(&v["levels"][0]), 5.0);
    assert_eq!(v["display"], serde_json::json!(["5"]));
    assert_eq!(v["server"], "stopped");
    let status = json_of(&ok(call(&client, "status", serde_json::json!({})).await));
    assert_eq!(status["mode"], "keyboard");

    // Keys work right after eval, without stop_server.
    ok(call(
        &client,
        "press_keys",
        serde_json::json!({"script": "backspace"}),
    )
    .await);

    // The acceptance case: a fresh 48SX is in degrees.
    ok(call(
        &client,
        "eval",
        serde_json::json!({"source": "RAD", "keep_server": true}),
    )
    .await);
    let r = ok(call(
        &client,
        "eval",
        serde_json::json!({"source": "SIN(0.5)", "keep_server": true}),
    )
    .await);
    assert_eq!(
        json_of(&r)["levels"][0],
        serde_json::json!({"type": "real", "value": 0.479425538604})
    );
    let status = json_of(&ok(call(&client, "status", serde_json::json!({})).await));
    assert_eq!(status["mode"], "server");

    // The limit counts from the calculator's receipt of the command: the
    // smallest one, 1 s, fits a trivial command (the link's turnaround and
    // line time do not count).
    let r = ok(call(
        &client,
        "eval",
        serde_json::json!({"source": "1 2 + DROP", "timeout_ms": 1000, "keep_server": true}),
    )
    .await);
    assert_eq!(json_of(&r)["server"], "running");
    assert_eq!(real(&eval1(&client, "'X^2' 3 'X' STO EVAL").await), 9.0);
    assert_eq!(
        eval1(&client, "\"Hello\"").await,
        serde_json::json!({"type": "string", "value": "Hello"})
    );
    assert_eq!(
        eval1(&client, "{ 1 2.5 \"s\" X }").await,
        serde_json::json!({"type": "list", "items": [
            {"type": "real", "value": 1.0}, {"type": "real", "value": 2.5},
            {"type": "string", "value": "s"}, {"type": "name", "value": "X"}]})
    );
    assert_eq!(
        eval1(&client, "(1,-2) 2 *").await,
        serde_json::json!({"type": "complex", "re": 2.0, "im": -4.0})
    );
    assert_eq!(
        eval1(&client, "« 1 2 + »").await,
        serde_json::json!({"type": "program", "source": "« 1 2 +\n»"})
    );
    assert_eq!(
        eval1(&client, "#FFh").await,
        serde_json::json!({"type": "binary", "value": 255, "base": "dec", "text": "# 255d"})
    );

    // An error is a tool error with the calculator's message; the
    // arguments stay on the stack.
    // (The plan expected "Infinite Result" from 0 0 /; the ROM says
    // "Undefined Result" for 0/0 and "Infinite Result" for 1/0.)
    let r = call(
        &client,
        "eval",
        serde_json::json!({"source": "0 0 /", "keep_server": true}),
    )
    .await;
    assert_eq!(r.is_error, Some(true));
    assert_eq!(json_of(&r)["error"], "Undefined Result");
    let r = call(
        &client,
        "eval",
        serde_json::json!({"source": "1 0 /", "keep_server": true}),
    )
    .await;
    assert_eq!(r.is_error, Some(true));
    let e = json_of(&r);
    assert_eq!(e["error"], "Infinite Result", "{e}");
    assert_eq!(
        e["display"],
        serde_json::json!([
            "0",
            "1",
            "0",
            "0",
            "# 255d",
            "« 1 2 + »",
            "(2,-4)",
            "{ 1 2.5 \"s\" X }",
            "\"Hello\"",
            "9",
            ".479425538604"
        ])
    );
    // A syntax error leaves the stack as it was.
    let r = call(
        &client,
        "eval",
        serde_json::json!({"source": "1 2 )(", "keep_server": true}),
    )
    .await;
    assert_eq!(json_of(&r)["error"], "Invalid Syntax");

    // The typed stack after a few evals, highest first in display order.
    let st = json_of(&ok(call(
        &client,
        "stack",
        serde_json::json!({"levels": 5, "keep_server": true}),
    )
    .await));
    assert_eq!(st["depth"], 11, "{st}");
    assert_eq!(
        st["levels"][0],
        serde_json::json!({"type": "real", "value": 0.0})
    );
    assert_eq!(
        st["levels"][1],
        serde_json::json!({"type": "real", "value": 1.0})
    );
    assert_eq!(st["levels"][4]["text"], "# 255d");
    assert_eq!(st["display"].as_array().unwrap().len(), 5);
    let all = json_of(&ok(call(
        &client,
        "stack",
        serde_json::json!({"keep_server": true}),
    )
    .await));
    assert_eq!(all["levels"].as_array().unwrap().len(), 11);
    assert_eq!(
        all["levels"][10],
        serde_json::json!({"type": "real", "value": 0.479425538604})
    );

    // Keys after a kept server leave it.
    let keys = ok(call(
        &client,
        "press_keys",
        serde_json::json!({"script": "1 enter"}),
    )
    .await);
    assert!(
        text_of(&keys).contains("left Kermit server mode first"),
        "{}",
        text_of(&keys)
    );
    // 95 s of real-number work on a real 48SX. Stopped after 1 s, the ROM
    // is still compiling and puts the text back as a string (with Σ for
    // the trigraph); the tool drops it.
    let e = timeout_interrupts(
        &client,
        "'\\GS(X=1,1000,XROOT(3,EXP(SIN(ATAN(X)))))' EVAL",
        1000,
    )
    .await;
    assert!(e.contains("was dropped"), "{e}");
    client.cancel().await.unwrap();
}

/// push/pop/drop, variables and directories on the 48SX: objects round
/// trip exactly through binary and text transfers, no temporary variable
/// is left behind, long source travels as a string.
#[tokio::test(flavor = "multi_thread")]
async fn hp48sx_push_and_variables() {
    let Some(rom) = rom("sxrom-j") else { return };
    let client = connect().await;
    ok(call(
        &client,
        "boot",
        serde_json::json!({"model": "48sx", "rom_path": rom}),
    )
    .await);

    // push / pop / drop / clear_stack, exact through binary transfers: a
    // string with a quote and a tagged object only travel in binary.
    ok(call(
        &client,
        "clear_stack",
        serde_json::json!({"keep_server": true}),
    )
    .await);
    let objects = [
        serde_json::json!({"type": "real", "value": "-1.23456789012E-450"}),
        serde_json::json!({"type": "string", "value": "say \"hi\" «x»"}),
        serde_json::json!({"type": "tagged", "tag": "T", "object": {"type": "real", "value": 5.0}}),
        serde_json::json!({"type": "list", "items": [{"type": "name", "value": "A"}, {"type": "list", "items": []}]}),
        serde_json::json!({"type": "array", "dims": [2, 2], "items": [
            [{"type": "real", "value": 1.0}, {"type": "real", "value": 2.0}],
            [{"type": "real", "value": 3.0}, {"type": "real", "value": 4.5}]]}),
        serde_json::json!({"type": "unit", "value": 9.81, "unit": "m/s^2"}),
        serde_json::json!({"type": "algebraic", "source": "'X^2+1'"}),
    ];
    for o in &objects {
        ok(call(
            &client,
            "push",
            serde_json::json!({"object": o, "keep_server": true}),
        )
        .await);
        let st = json_of(&ok(call(
            &client,
            "stack",
            serde_json::json!({"levels": 1, "keep_server": true}),
        )
        .await));
        assert_eq!(&st["levels"][0], o, "{st}");
    }
    // Text that would break out of its quotes is refused, or sent in
    // binary: nothing runs, the depth grows by one, nothing is stored.
    let odd = serde_json::json!({"type": "name", "value": "X' 11. 'QA' STO 'Y"});
    let pushed = json_of(&ok(call(
        &client,
        "push",
        serde_json::json!({"object": odd, "keep_server": true}),
    )
    .await));
    assert_eq!(pushed["depth"], objects.len() + 1);
    let st = json_of(&ok(call(
        &client,
        "stack",
        serde_json::json!({"levels": 1, "keep_server": true}),
    )
    .await));
    assert_eq!(st["levels"][0], odd);
    ok(call(&client, "drop", serde_json::json!({"keep_server": true})).await);
    let bad = serde_json::json!({"type": "program", "source": "« 1 » 22. 'QB' STO « 2 »"});
    let r = call(
        &client,
        "set_var",
        serde_json::json!({"name": "P", "object": bad, "keep_server": true}),
    )
    .await;
    assert_eq!(r.is_error, Some(true));
    assert!(text_of(&r).contains("one « ... » group"), "{}", text_of(&r));
    let vars = json_of(&ok(call(
        &client,
        "list_vars",
        serde_json::json!({"keep_server": true}),
    )
    .await));
    assert!(
        !vars.to_string().contains("QA") && !vars.to_string().contains("QB"),
        "{vars}"
    );
    let popped = json_of(&ok(call(
        &client,
        "pop",
        serde_json::json!({"keep_server": true}),
    )
    .await));
    assert_eq!(&popped["levels"][0], &objects[6]);
    assert_eq!(popped["depth"], objects.len() - 1);
    let d = json_of(&ok(call(
        &client,
        "drop",
        serde_json::json!({"count": 2, "keep_server": true}),
    )
    .await));
    assert_eq!(d["depth"], objects.len() - 3);
    let r = call(
        &client,
        "drop",
        serde_json::json!({"count": 50, "keep_server": true}),
    )
    .await;
    assert_eq!(json_of(&r)["error"], "Too Few Arguments");

    // Variables and directories; no temporary variable is left behind.
    ok(call(
        &client,
        "eval",
        serde_json::json!({"source": "CLEAR 'D1' CRDIR", "keep_server": true}),
    )
    .await);
    let p = json_of(&ok(call(
        &client,
        "cd",
        serde_json::json!({"path": "D1", "keep_server": true}),
    )
    .await));
    assert_eq!(p["path"], serde_json::json!(["HOME", "D1"]));
    let prog = serde_json::json!({"type": "program", "source": "« 1 2 + »"});
    ok(call(
        &client,
        "set_var",
        serde_json::json!({"name": "P", "object": prog, "keep_server": true}),
    )
    .await);
    ok(call(
        &client,
        "set_var",
        serde_json::json!({"name": "R", "object": objects[0], "keep_server": true}),
    )
    .await);
    ok(call(
        &client,
        "set_var",
        serde_json::json!({"name": "R", "object": objects[1], "keep_server": true}),
    )
    .await);
    let g = json_of(&ok(call(
        &client,
        "get_var",
        serde_json::json!({"name": "R", "keep_server": true}),
    )
    .await));
    assert_eq!(g["object"], objects[1]);
    let g = json_of(&ok(call(
        &client,
        "get_var",
        serde_json::json!({"name": "P", "keep_server": true}),
    )
    .await));
    assert_eq!(g["object"]["type"], "program");
    assert_eq!(real(&eval1(&client, "P").await), 3.0);
    let vars = json_of(&ok(call(
        &client,
        "list_vars",
        serde_json::json!({"keep_server": true}),
    )
    .await));
    let names: Vec<&str> = vars["variables"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["R", "P"], "{vars}");
    let r = call(
        &client,
        "get_var",
        serde_json::json!({"name": "NOPE", "keep_server": true}),
    )
    .await;
    assert_eq!(r.is_error, Some(true));
    let p = json_of(&ok(
        call(&client, "cd", serde_json::json!({"path": ".."})).await
    ));
    assert_eq!(p["path"], serde_json::json!(["HOME"]));
    assert_eq!(p["server"], "stopped");
    let vars = json_of(&ok(call(&client, "list_vars", serde_json::json!({})).await));
    assert!(!vars.to_string().contains("SATRN"), "{vars}");
    // The same from RAM, with the server stopped and left stopped.
    let tree = json_of(&ok(
        call(&client, "memory_tree", serde_json::json!({})).await
    ));
    assert_eq!(tree["path"], serde_json::json!(["HOME"]));
    let d1 = &tree["variables"][0];
    assert_eq!(
        (&d1["name"], &d1["type"]),
        (&"D1".into(), &"Directory".into()),
        "{tree}"
    );
    assert_eq!(d1["size"], vars["variables"][0]["size"], "{tree}");
    let inner: Vec<&str> = d1["variables"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["name"].as_str().unwrap())
        .collect();
    assert_eq!(inner, ["R", "P"]);
    let flags = json_of(&ok(call(&client, "flags", serde_json::json!({})).await));
    assert_eq!(flags["system"].as_array().unwrap().len(), 1, "{flags}");
    let status = json_of(&ok(call(&client, "status", serde_json::json!({})).await));
    assert_eq!(status["mode"], "keyboard");

    // A long source travels as a string.
    let long = format!("0 {}", "1 + ".repeat(40));
    assert_eq!(real(&eval1(&client, &long).await), 40.0);
    client.cancel().await.unwrap();
}

/// A slow `eval` stopped by `timeout_ms`: the error says so, the
/// server is left, the evaluated text is not left on the stack as a
/// string (the 48SX ROM puts it back), and the next eval works.
async fn timeout_interrupts(
    client: &RunningService<RoleClient, ()>,
    slow: &str,
    timeout_ms: u64,
) -> String {
    let r = call(
        client,
        "eval",
        serde_json::json!({"source": slow, "timeout_ms": timeout_ms}),
    )
    .await;
    assert_eq!(r.is_error, Some(true));
    assert!(
        text_of(&r).contains("interrupted with ON"),
        "{}",
        text_of(&r)
    );
    let status = json_of(&ok(call(client, "status", serde_json::json!({})).await));
    assert_eq!(status["mode"], "keyboard");
    let st = json_of(&ok(call(
        client,
        "stack",
        serde_json::json!({"keep_server": true}),
    )
    .await));
    let quoted = format!("\"{slow}\"");
    assert!(
        st["display"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d.as_str() != Some(quoted.as_str())),
        "{st}"
    );
    let error = text_of(&r);
    let r = ok(call(
        client,
        "eval",
        serde_json::json!({"source": "CLEAR 6. 7. *"}),
    )
    .await);
    assert_eq!(real(&json_of(&r)["levels"][0]), 42.0);
    error
}

/// eval on the 48GX and the 49G: reals (the acceptance case) and a list;
/// the 49G's exact integers; a 49G symbolic computation stopped by
/// timeout_ms, after which eval works again.
#[tokio::test(flavor = "multi_thread")]
async fn hp48gx_and_49g_eval() {
    for (model, file) in [("48gx", "gxrom-r"), ("49g", "rom.49g")] {
        let Some(rom) = rom(file) else { return };
        let client = connect().await;
        ok(call(
            &client,
            "boot",
            serde_json::json!({"model": model, "rom_path": rom}),
        )
        .await);
        ok(call(&client, "eval", serde_json::json!({"source": "RAD"})).await);
        let r = ok(call(&client, "eval", serde_json::json!({"source": "SIN(0.5)"})).await);
        assert_eq!(
            json_of(&r)["levels"][0],
            serde_json::json!({"type": "real", "value": 0.479425538604}),
            "{model}"
        );
        assert_eq!(real(&eval1(&client, "2. 3. +").await), 5.0, "{model}");
        let list = eval1(&client, "{ 1. \"a\" { B } }").await;
        assert_eq!(
            list,
            serde_json::json!({"type": "list", "items": [
                {"type": "real", "value": 1.0}, {"type": "string", "value": "a"},
                {"type": "list", "items": [{"type": "name", "value": "B"}]}]}),
            "{model}"
        );
        if model == "48gx" {
            // 55 s of real-number work on a real 48GX.
            let _ = timeout_interrupts(
                &client,
                "'\\GS(X=1,1000,XROOT(3,EXP(SIN(ATAN(X)))))' EVAL",
                5000,
            )
            .await;
        }
        if model == "49g" {
            assert_eq!(
                eval1(&client, "2 3 +").await,
                serde_json::json!({"type": "integer", "value": 5})
            );
            let r = ok(call(
                &client,
                "eval",
                serde_json::json!({"source": "2 100 ^", "keep_server": true}),
            )
            .await);
            assert_eq!(
                json_of(&r)["levels"][0]["value"],
                "1267650600228229401496703205376"
            );
            // Exact integers make this sum symbolic: minutes of work.
            let _ = timeout_interrupts(
                &client,
                "'\\GS(X=1,100,XROOT(3,EXP(SIN(ATAN(X)))))' EVAL",
                5000,
            )
            .await;
        }
        client.cancel().await.unwrap();
    }
}

/// The semantic tools refuse the models without a Kermit server.
#[tokio::test(flavor = "multi_thread")]
async fn aplet_models_have_no_semantic_tools() {
    for (model, file) in [
        ("38g", "38G_A167.ROM"),
        ("39g", "rom.39g"),
        ("40g", "rom.39g"),
    ] {
        let Some(rom) = rom(file) else { return };
        let client = connect().await;
        ok(call(
            &client,
            "boot",
            serde_json::json!({"model": model, "rom_path": rom}),
        )
        .await);
        for tool in ["eval", "stack", "list_vars"] {
            let r = call(&client, tool, serde_json::json!({"source": "1"})).await;
            assert_eq!(r.is_error, Some(true));
            assert!(
                text_of(&r).contains("no Kermit server on this model"),
                "{model} {tool}: {}",
                text_of(&r)
            );
        }
        client.cancel().await.unwrap();
    }
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

/// The tree from `path` down as Kermit lists it: `cd` into every
/// directory, `G D` each (name, type, size, checksum), newest first.
fn kermit_tree(emu: &mut Emulator, path: &mut Vec<String>) -> Vec<serde_json::Value> {
    emu.cd(&path.join("/")).unwrap();
    let vars = emu.list_vars().unwrap();
    assert_eq!(vars.path, *path);
    let mut out = Vec::new();
    for v in vars.variables {
        let mut j = serde_json::json!({"name": v.name, "type": v.kind, "size": v.size, "checksum": v.checksum});
        if v.kind == "Directory" {
            path.push(v.name.clone());
            j["variables"] = serde_json::Value::Array(kermit_tree(emu, path));
            path.pop();
        }
        out.push(j);
    }
    out
}

/// The RAM-read tree without addresses, for comparing with Kermit's.
fn ram_tree_json(vars: &[saturnus_objects::Variable]) -> Vec<serde_json::Value> {
    vars.iter()
        .map(|v| {
            let mut j = serde_json::json!({"name": v.name, "type": v.kind, "size": v.size, "checksum": v.checksum});
            if let Some(sub) = &v.variables {
                j["variables"] = serde_json::Value::Array(ram_tree_json(sub));
            }
            j
        })
        .collect()
}

/// The memory read API (iteration 12a) against the Kermit server on the
/// 48SX, 48GX and 49G: a known tree, current directory, stack and flags
/// built over Kermit; with the server stopped the RAM reads must equal
/// what `G D` in every directory, `PATH`, the typed stack and `RCLF`
/// report; storing a variable moves the change counter, idling does not.
#[test]
fn ram_reads_match_kermit() {
    let Some(dir) = std::env::var_os("SATURNUS_ROM_DIR") else {
        eprintln!("SATURNUS_ROM_DIR not set: skipping the RAM read test");
        return;
    };
    let limit = std::time::Duration::from_secs(30);
    for (model, file) in [
        (Model::Hp48sx, "sxrom-j"),
        (Model::Hp48gx, "gxrom-r"),
        (Model::Hp49g, "rom.49g"),
    ] {
        let path = std::path::Path::new(&dir).join(file);
        let (mut emu, _) = Emulator::boot(model, &path, true, Limits::default()).unwrap();
        if model == Model::Hp49g {
            // RPN, and a server entered from RPN: entered from algebraic
            // mode (the 49G's default) FINISH leaves the server's stack
            // packed in a list.
            emu.run_command("-95 CF").unwrap();
            emu.stop_server().unwrap();
            emu.start_server().unwrap();
        }
        for cmd in [
            "CLEAR HOME 42.5 'X' STO \"HI\" 'S' STO { 1. \"A\" # 2Ah } 'L' STO",
            "'DA' CRDIR DA 5. 'Z' STO 'DB' CRDIR DB (1.,2.) 'C' STO",
            "HOME 'E' CRDIR DA HEX -2 SF 7 SF 21 SF",
            "1.5 \"two\" # 3h :T:4. { 5. 6. } [ 1. 2. ] 'N'",
        ] {
            let r = emu.run_command(cmd).unwrap();
            assert_eq!(r.error, None, "{model:?}: {cmd}");
        }
        // The oracle: what the calculator reports over Kermit.
        let stack = emu.typed_stack(None).unwrap().levels;
        let rclf = emu.eval("RCLF", 1, limit).unwrap().unwrap().levels;
        emu.run_command("DROP").unwrap();
        let Object::List { items } = &rclf[0] else {
            panic!("RCLF: {rclf:?}")
        };
        let words: Vec<u64> = items
            .iter()
            .map(|o| match o {
                Object::Binary { value, .. } => *value,
                o => panic!("RCLF item {o:?}"),
            })
            .collect();
        let cwd = emu.list_vars().unwrap().path;
        assert_eq!(cwd, ["HOME", "DA"]);
        let kermit = kermit_tree(&mut emu, &mut vec!["HOME".into()]);
        emu.cd("HOME/DA").unwrap();
        emu.stop_server().unwrap();

        // The same from RAM, without the server.
        let tree = emu.memory_tree().unwrap();
        assert_eq!(tree.path, cwd, "{model:?}");
        assert_eq!(ram_tree_json(&tree.variables), kermit, "{model:?}");
        let names: Vec<&str> = tree.variables.iter().map(|v| v.name.as_str()).collect();
        assert!(
            names.starts_with(&["E", "DA", "L", "S", "X"]),
            "{model:?}: {names:?}"
        );
        assert_eq!(emu.ram_stack().unwrap(), stack, "{model:?}");
        assert_eq!(stack.len(), 7, "{model:?}");
        let flags = emu.ram_flags().unwrap();
        let mut ram_words = Vec::new();
        for (s, u) in flags.system.iter().zip(&flags.user) {
            ram_words.extend([*s, *u]);
        }
        assert_eq!(ram_words, words, "{model:?}: RCLF");
        assert_eq!(flags.get(-2), Some(true), "{model:?}");
        assert_eq!(flags.get(7), Some(true), "{model:?}");
        assert_eq!(flags.base(), saturnus_objects::Base::Hex, "{model:?}");

        // The change counter: still while idle, moves with a STO.
        let c0 = emu.ram_changes().unwrap();
        emu.press_keys("wait 500").unwrap();
        assert_eq!(emu.ram_changes().unwrap(), c0, "{model:?}");
        emu.semantic(false, |e| e.run_command("7. 'W' STO"))
            .unwrap();
        assert!(!emu.server_running());
        let c1 = emu.ram_changes().unwrap();
        assert_ne!(c1, c0, "{model:?}");
        let tree = emu.memory_tree().unwrap();
        assert_eq!(
            tree.variables[1].variables.as_ref().unwrap()[0].name,
            "W",
            "{model:?}"
        );
    }
}

/// The status line's rows (above the separator), where the clock shows.
fn status_rows(emu: &Emulator) -> String {
    let (screen, _) = emu.screen_text().unwrap();
    screen
        .lines()
        .take_while(|l| !l.bytes().all(|c| c == b'#'))
        .collect::<Vec<_>>()
        .join("\n")
}

/// With the clock shown (flag -40) every key arrives and the clock keeps
/// ticking. A key interrupt in service when TIMER2 expired used to freeze
/// the 48SX: TIMER2 read as #FFFFFFFF until the next vectoring while the
/// ROM's handler waited for it to change (decision log, fix/48sx-clock-hang).
#[test]
fn clock_display_keeps_keys_and_time() {
    let Some(dir) = std::env::var_os("SATURNUS_ROM_DIR") else {
        eprintln!("SATURNUS_ROM_DIR not set: skipping the clock display test");
        return;
    };
    for (model, file) in [
        (Model::Hp48sx, "sxrom-j"),
        (Model::Hp48gx, "gxrom-r"),
        (Model::Hp49g, "rom.49g"),
    ] {
        let path = std::path::Path::new(&dir).join(file);
        let (mut emu, _) = Emulator::boot(model, &path, true, Limits::default()).unwrap();
        // RPN on the 49G, so level 1 is the number typed.
        let flags = if model == Model::Hp49g {
            "-40 SF -95 CF"
        } else {
            "-40 SF"
        };
        let r = emu.run_command(flags).unwrap();
        assert_eq!(r.error, None, "{model:?}");
        emu.stop_server().unwrap();
        let r = emu.press_keys("1 2 3 4 5 6 7 8 9 0").unwrap();
        assert!(r.warnings.is_empty(), "{model:?}: {:?}", r.warnings);
        let r = emu.press_keys("enter").unwrap();
        assert!(r.warnings.is_empty(), "{model:?}: {:?}", r.warnings);
        // The clock must keep moving, not just change once: 16 samples
        // 250 ms of emulated time apart (4 s). The 48s show seconds (a
        // change a second); the 49G shows HH:MM with a colon that blinks.
        let mut last = status_rows(&emu);
        let mut changes = 0;
        for _ in 0..16 {
            emu.press_keys("wait 250").unwrap();
            let now = status_rows(&emu);
            changes += usize::from(now != last);
            last = now;
        }
        eprintln!("{model:?}: {changes} clock changes in 4 s");
        assert!(
            changes >= 3,
            "{model:?}: the clock stopped ({changes} changes in 4 s)"
        );
        emu.start_server().unwrap();
        let levels = emu.read_stack(None).unwrap();
        assert_eq!(
            levels.first().map(String::as_str),
            Some("1234567890"),
            "{model:?}: {levels:?}"
        );
    }
}

/// Structure cases for the decompiler oracle, written by hand: every
/// control structure, locals, quoted names, each precedence level and the
/// special algebraic forms, units, tagged objects, nesting, strings.
const DECOMPILER_CORPUS: &[&str] = &[
    "\u{ab} 1 2 + \u{bb}",
    "'A+1'",
    "{ 1 SIN }",
    "\u{ab} IF A THEN 1 ELSE 2 END \u{bb}",
    "\u{ab} IF 'A>B' THEN 1 END \u{bb}",
    "\u{ab} \u{2192} a b \u{ab} a b + \u{bb} \u{bb}",
    "\u{ab} \u{2192} X 'X^2' \u{bb}",
    "\u{ab} 'X' STO \u{bb}",
    "\u{ab} 1 5 FOR i i NEXT \u{bb}",
    "\u{ab} 1 2 START 3 STEP 1 2 START 3 NEXT \u{bb}",
    "\u{ab} DO 1 UNTIL 1 END WHILE 1 REPEAT 2 END \u{bb}",
    "\u{ab} CASE A THEN 1 END B THEN 2 END 3 END \u{bb}",
    "\u{ab} IFERR 1 THEN 2 ELSE 3 END IFERR 1 THEN 2 END \u{bb}",
    "\u{ab} \u{ab} 1 \u{bb} EVAL { \u{ab} 2 \u{bb} } HALT \u{bb}",
    "\u{ab} \"a b\" { X Y } [ 1 2 ] (1,2) # 2Ah 'A+1' \u{bb}",
    "\u{ab} [[ 1 2 ] [ 3 4 ]] \u{bb}",
    "{ 1.5 -2 1E-5 .001 123456789012 1.5E300 }",
    "{ :T:5 :a b:\"x\" }",
    "{ \"q\" X 'Y' { } { { 1 } } }",
    "[ 1 2 3 ]",
    "[[ 1 2 ] [ 3 4 ]]",
    "[ (1,2) (3,4) ]",
    "[ 'X' 'Y+1' ]",
    "1_m/s^2",
    "1_m*s",
    "1_km",
    "1_(m/s)",
    "1_1/s",
    "2.5_kg*m^2/s^2",
    "1_\u{b0}C",
    "1_cm/(s*K)",
    "\u{ab} 1_m \u{bb}",
    "'2_m+3_m'",
    "'F(A,B)'",
    "'F(X)=X^2+1'",
    "'-A'",
    "'SIN(X)^2'",
    "'(A+B)*C'",
    "'A+B*C'",
    "'A-(B-C)'",
    "'A-B-C'",
    "'A/(B*C)'",
    "'A/B/C'",
    "'A^B^C'",
    "'A^(B^C)'",
    "'(A^B)^C'",
    "'-A^2'",
    "'(-A)^2'",
    "'A*-B'",
    "'A+-B'",
    "'A^-B'",
    "'A^-2'",
    "'-(A+B)'",
    "'-(A*B)'",
    "'-A*B'",
    "'-(-A)'",
    "'X!'",
    "'(X+1)!'",
    "'\u{221a}X'",
    "'\u{221a}(X+1)'",
    "'\u{221a}X^2'",
    "'\u{221a}-X'",
    "'MAX(A,B)'",
    "'IFTE(X>0,X,-X)'",
    "'A==B'",
    "'A<B'",
    "'A\u{2264}B'",
    "'A AND B'",
    "'A OR B AND C'",
    "'A XOR B'",
    "'NOT A'",
    "'NOT (A AND B)'",
    "'NOT A==B'",
    "'X<Y AND Y<Z'",
    "'A=B+1'",
    "'\u{2202}X(SIN(X))'",
    "'\u{222b}(0,1,X,X)'",
    "'\u{3a3}(K=0,M,K)'",
    "'%(A,B)'",
    "'%CH(A,B)'",
    "'X|(X=2)'",
    "'X|(X=2,Y=3)'",
    "'(X|(X=2))+1'",
    "'-(X|(X=2))'",
    "'1+(X|(X=2))'",
    "'(X|(X=2))^2'",
    "'SIN(X|(X=2))'",
    "'XROOT(3,8)'",
    "'\u{3c0}'",
    "'e+i'",
    "'(1,2)*X'",
    "'1E12*X'",
    "'INV(X)+SQ(X+1)+ABS(X)'",
    "'MAXR'",
];

/// Display-mode cases: the transfer always uses the standard format, so
/// these compare with the stack display (`(setup, object)`).
const DECOMPILER_MODES: &[(&str, &str)] = &[
    ("3 FIX", "{ .5 -.5 .0004 .0005 999.9996 }"),
    ("3 FIX", "{ 1E11 1E12 -1E-12 0 -1 }"),
    ("0 FIX", "{ .5 2.5 -.4 12 }"),
    ("11 FIX", "{ .1 1E-11 }"),
    ("2 SCI", "{ 9.996 1E-300 -1 0 }"),
    ("3 ENG", "{ 1 1000 .01 999.95 }"),
    ("0 ENG", "{ 15 155 }"),
    ("STD -51 SF", "{ 1,5 (1,5;2) 'A+1,5' }"),
    ("-51 CF HEX 16 STWS", "{ #FFFFFh #12h }"),
    ("OCT", "{ #8d }"),
    ("BIN 64 STWS", "{ #5d }"),
    ("DEC -53 SF", "'A+B*C'"),
    ("-53 CF 3 FIX", "1234.5"),
    ("", "{ 12345678. }"),
    ("", "\u{ab} 1234.5 \u{bb}"),
    ("", "(1234.5,2)"),
    ("", "'A+1234.5'"),
    ("3 FIX", "1.5_m^2"),
    ("", "{ 2_m^2 }"),
    ("2 SCI", "1.5_m^2.5"),
    ("2 ENG", "1_cm^3"),
    ("-51 SF", "-1234567,5"),
    ("", "1,5_m^2,5"),
    ("", "{ 1234,5 }"),
    ("-51 CF STD", "{ :T:5 }"),
];

/// A deterministic source of generated corpus entries (xorshift).
struct Gen(u64);

impl Gen {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[(self.next() % items.len() as u64) as usize]
    }

    /// A fully parenthesised expression: the calculator parses it and
    /// keeps only the parentheses it needs.
    fn expr(&mut self, depth: u32) -> String {
        if depth == 0 || self.next().is_multiple_of(4) {
            return self
                .pick(&["A", "B", "X", "Y", "2", "3.5", "10", "\u{3c0}"])
                .to_string();
        }
        let a = self.expr(depth - 1);
        match self.next() % 6 {
            0..=2 => {
                let b = self.expr(depth - 1);
                let op = self.pick(&["+", "-", "*", "/", "^", "+", "*"]);
                format!("({a}{op}{b})")
            }
            3 => {
                let f = self.pick(&["SIN", "LN", "ABS", "EXP", "\u{221a}", "INV"]);
                format!("{f}({a})")
            }
            4 => format!("-({a})"),
            _ => {
                let b = self.expr(depth - 1);
                let f = self.pick(&["MAX", "MIN", "%", "XROOT"]);
                format!("{f}({a},{b})")
            }
        }
    }

    /// A short program of commands, data and structures.
    fn program(&mut self) -> String {
        let mut parts = vec!["\u{ab}".to_string()];
        for _ in 0..(2 + self.next() % 4) {
            let p = match self.next() % 8 {
                0 => self
                    .pick(&["DUP", "SWAP", "+", "*", "DROP", "SIN", "\u{2192}LIST"])
                    .into(),
                1 => self
                    .pick(&["1", "-2.5", "1E-3", "\"s t\"", "X", "# 10h"])
                    .into(),
                2 => format!("'{}'", self.expr(2)),
                3 => "IF X THEN 1 ELSE 2 END".into(),
                4 => "1 3 FOR j j NEXT".into(),
                5 => "\u{2192} a \u{ab} a \u{bb}".into(),
                6 => "{ A 1 }".into(),
                _ => "WHILE X REPEAT Y END".into(),
            };
            parts.push(p);
        }
        parts.push("\u{bb}".into());
        parts.join(" ")
    }

    /// A list of reals of mixed magnitudes.
    fn numbers(&mut self) -> String {
        let mut items = Vec::new();
        for _ in 0..4 {
            let mantissa = self.next() % 1_000_000;
            let exp = (self.next() % 40) as i64 - 20;
            let sign = if self.next().is_multiple_of(3) {
                "-"
            } else {
                ""
            };
            items.push(format!("{sign}{mantissa}E{exp}"));
        }
        format!("{{ {} }}", items.join(" "))
    }
}

/// The ASCII transfer of an object in the one-line form the stack shows,
/// for the decompiler oracle. Documented differences only (wiki:
/// protocols/hp-object-format "ASCII transfer and stack display";
/// observed in the three ROMs): the `%%HP:` header line; line breaks,
/// which the transfer inserts to keep lines short, become one space
/// between tokens and vanish inside an algebraic (where it breaks
/// anywhere); the indentation after them; the quotes around a unit object
/// (`'1_m'`), which the transfer adds so it reads back and the stack does
/// not show; and on the 49G a tag's leading colon (`:T:5` is shown
/// `T: 5`). The character translation is undone by
/// `hptx_core::charset::decode` before this.
fn transfer_to_display(ascii: &str, model: Model) -> String {
    let body = match ascii.find('\n') {
        Some(i) if ascii.starts_with("%%HP") => &ascii[i + 1..],
        _ => ascii,
    };
    let mut out = String::new();
    let (mut in_string, mut in_alg) = (false, false);
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if !in_alg => in_string = !in_string,
            '\'' if !in_string => in_alg = !in_alg,
            '\r' | '\n' if !in_string => {
                while matches!(chars.peek(), Some('\r' | '\n' | ' ')) {
                    chars.next();
                }
                if !in_alg && !out.ends_with(' ') && chars.peek().is_some() {
                    out.push(' ');
                }
                continue;
            }
            // A token starting with a colon is a tag.
            ':' if !in_string
                && !in_alg
                && model == Model::Hp49g
                && (out.is_empty() || out.ends_with([' ', '{'])) =>
            {
                continue;
            }
            _ => {}
        }
        out.push(c);
    }
    // Unquote unit objects: a quoted number_unit with one underscore.
    let mut result = String::new();
    let mut rest = out.trim_end();
    while let Some(i) = rest.find('\'') {
        result.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let Some(j) = after.find('\'') else {
            result.push_str(&rest[i..]);
            return result;
        };
        let inner = &after[..j];
        let number = inner.split('_').next().unwrap_or("");
        let unit = inner.matches('_').count() == 1
            && !number.is_empty()
            && number
                .chars()
                .all(|c| c.is_ascii_digit() || ".E-".contains(c));
        if unit {
            result.push_str(inner);
        } else {
            result.push('\'');
            result.push_str(inner);
            result.push('\'');
        }
        rest = &after[j + 1..];
    }
    result.push_str(rest);
    result
}

/// What one oracle case gave.
enum Case {
    /// Our text equals the ROM's.
    Match,
    /// They differ (the message says how).
    Mismatch(String),
    /// The calculator refused the source (its error message).
    Rejected(String),
}

/// One oracle case: compile `source` on the calculator, store it, read it
/// back through RAM with our decompiler and through the ROM (its ASCII
/// transfer, or with `display` its stack display).
fn decompiler_case(
    emu: &mut Emulator,
    names: &saturnus_objects::NameTable,
    source: &str,
    display: bool,
) -> Case {
    let model = emu.model();
    let mut shown = None;
    let stored = if display {
        let pushed = emu.run_command(source).unwrap();
        shown = pushed.levels.first().cloned();
        if pushed.error.is_some() || shown.is_none() {
            emu.run_command("CLEAR").unwrap();
            return Case::Rejected(pushed.error.unwrap_or_else(|| "nothing pushed".into()));
        }
        emu.run_command("'ORACLE' STO").unwrap()
    } else {
        emu.run_command(&format!("{source} 'ORACLE' STO")).unwrap()
    };
    if let Some(e) = stored.error {
        emu.run_command("CLEAR").unwrap();
        return Case::Rejected(e);
    }
    let want = match shown {
        Some(s) => s,
        None => {
            let ascii = emu
                .receive_object("ORACLE", hptx_core::TransferMode::Ascii)
                .unwrap();
            transfer_to_display(&hptx_core::charset::decode(&ascii), model)
        }
    };
    let tree = emu.memory_tree().unwrap();
    let var = tree.variables.iter().find(|v| v.name == "ORACLE").unwrap();
    let mut ours = emu
        .with_machine(|m| {
            let u = saturnus_objects::UserMemory::of(m)
                .unwrap()
                .with_names(names);
            let settings = saturnus_objects::Settings::from_flags(&u.flags().unwrap(), model);
            saturnus_objects::display(&u.object_at(var.address).unwrap(), &settings)
        })
        .unwrap();
    // The 49G's server shows only the first 20 characters of a level.
    if display && model == Model::Hp49g && want.chars().count() == 20 {
        ours = ours.chars().take(20).collect();
    }
    if ours == want {
        Case::Match
    } else {
        Case::Mismatch(format!("{model:?} {source:?}: ROM {want:?}, ours {ours:?}"))
    }
}

/// Sources a model's calculator refuses on purpose: the 48s have no
/// arrays of names (symbolic arrays are the 49G's).
fn expected_rejection(model: Model, source: &str) -> bool {
    model != Model::Hp49g && source == "[ 'X' 'Y+1' ]"
}

/// The decompiler (iteration 12c) against the ROM's own on the 48SX, 48GX
/// and 49G: the hand-written structure cases, generated algebraics,
/// programs and numbers, and the display modes; every case read from RAM
/// with our name table must give the text the ROM gives. Also reports the
/// name table's size and build time, and (when the 13a command catalog is
/// in `crates/saturnus-cli/data/commands`) that every catalog name resolves.
#[test]
fn decompiler_matches_the_rom() {
    let Some(dir) = std::env::var_os("SATURNUS_ROM_DIR") else {
        eprintln!("SATURNUS_ROM_DIR not set: skipping the decompiler oracle");
        return;
    };
    let mut mismatches = Vec::new();
    for (model, file, seed) in [
        (Model::Hp48sx, "sxrom-j", 0x1234_5678_9ABC_DEF1),
        (Model::Hp48gx, "gxrom-r", 0x0FED_CBA9_8765_4321),
        (Model::Hp49g, "rom.49g", 0x5555_AAAA_3333_CCCC),
    ] {
        let path = std::path::Path::new(&dir).join(file);
        let (mut emu, _) = Emulator::boot(model, &path, true, Limits::default()).unwrap();
        if model == Model::Hp49g {
            // RPN (see ram_reads_match_kermit).
            emu.run_command("-95 CF").unwrap();
            emu.stop_server().unwrap();
            emu.start_server().unwrap();
        }
        let start = Instant::now();
        let names = emu
            .with_machine(|m| saturnus_objects::NameTable::of(m))
            .unwrap();
        let stats = names.stats();
        eprintln!(
            "{model:?}: name table in {:.1} ms, {} KiB: {stats:?}",
            start.elapsed().as_secs_f64() * 1000.0,
            names.heap_bytes() / 1024
        );
        assert!(stats.names >= 400, "{model:?}: {stats:?}");
        for name in ["SIN", "DUP", "+", "IF", "\u{ab}", "\u{bb}", "STO"] {
            assert!(
                names.names().any(|(_, _, n)| n == name),
                "{model:?}: {name} missing"
            );
        }
        catalog_resolves(model, &names);

        // Generated cases: a few by default (the suite runs in a debug
        // build); SATURNUS_ORACLE_SCALE=N runs N times as many.
        let scale: usize = std::env::var("SATURNUS_ORACLE_SCALE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);
        let mut generated = Gen(seed);
        let mut corpus: Vec<String> = DECOMPILER_CORPUS.iter().map(|s| s.to_string()).collect();
        for _ in 0..8 * scale {
            corpus.push(format!("'{}'", generated.expr(3)));
        }
        for _ in 0..5 * scale {
            corpus.push(generated.program());
        }
        for _ in 0..2 * scale {
            corpus.push(generated.numbers());
        }
        // One packet carries at most 77 bytes of command, `'ORACLE' STO`
        // included.
        corpus.retain(|s| {
            hptx_core::charset::encode_command(&format!("{s} 'ORACLE' STO"))
                .is_ok_and(|b| b.len() <= 74)
        });
        let mut compared = 0;
        let mut rejected = 0;
        let cases = corpus
            .iter()
            .map(|s| ("", s.as_str(), false))
            .chain(DECOMPILER_MODES.iter().map(|(setup, s)| (*setup, *s, true)));
        for (setup, source, display) in cases {
            if !setup.is_empty() {
                let r = emu.run_command(setup).unwrap();
                assert_eq!(r.error, None, "{model:?}: {setup}");
            }
            match decompiler_case(&mut emu, &names, source, display) {
                Case::Match => compared += 1,
                Case::Mismatch(m) => {
                    compared += 1;
                    mismatches.push(m);
                }
                Case::Rejected(_) if expected_rejection(model, source) => rejected += 1,
                Case::Rejected(e) => {
                    mismatches.push(format!("{model:?} {source:?}: rejected unexpectedly: {e}"));
                }
            }
        }
        // The texts a front end shows and copies (`described`, the `text`
        // of each level) against the server's display of the same stack:
        // a small real with twelve digits, a unit with a whole number, a
        // name, a list holding a name and a tagged object and, on the 49G,
        // an array holding a name.
        emu.run_command("CLEAR STD").unwrap();
        let mut levels = vec!["1.23456789012E-5", "2_m", "'QQ'", "{ 1 QQ :T:2 }"];
        if model == Model::Hp49g {
            levels.push("[ 'QQ' ]");
        }
        for level in &levels {
            let r = emu.run_command(level).unwrap();
            assert_eq!(r.error, None, "{model:?}: {level}");
        }
        let shown = emu.run_command("").unwrap().levels;
        emu.stop_server().unwrap();
        let ours = emu
            .with_machine(|m| {
                saturnus_objects::UserMemory::of(m)
                    .unwrap()
                    .with_names(&names)
                    .stack_described()
                    .unwrap()
                    .iter()
                    .map(|o| o["text"].as_str().unwrap_or("(no text)").to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap();
        emu.start_server().unwrap();
        compared += ours.len();
        if ours != shown {
            mismatches.push(format!("{model:?} texts: ROM {shown:?}, ours {ours:?}"));
        }
        // Whole stack levels, which STO would untag: the stack read from
        // RAM against the server's display of it.
        emu.run_command("CLEAR 3 FIX").unwrap();
        for level in [":T:1234.5", "1234.5", ":U:{ 1 }"] {
            emu.run_command(level).unwrap();
        }
        let shown = emu.run_command("").unwrap().levels;
        emu.stop_server().unwrap();
        let ours = emu
            .with_machine(|m| {
                let u = saturnus_objects::UserMemory::of(m)
                    .unwrap()
                    .with_names(&names);
                let settings = saturnus_objects::Settings::from_flags(&u.flags().unwrap(), model);
                u.stack()
                    .unwrap()
                    .iter()
                    .map(|o| saturnus_objects::display(o, &settings))
                    .collect::<Vec<_>>()
            })
            .unwrap();
        compared += ours.len();
        if ours != shown {
            mismatches.push(format!("{model:?} stack: ROM {shown:?}, ours {ours:?}"));
        }
        eprintln!("{model:?}: {compared} cases compared, {rejected} rejected as expected");
    }
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

/// Every name of the command catalog (`crates/saturnus-cli/data/commands/<model>.json`, from
/// iteration 13a, when present) resolves to the same name by its XLIB
/// numbers.
fn catalog_resolves(model: Model, names: &saturnus_objects::NameTable) {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../saturnus-cli/data/commands");
    let dir = std::env::var_os("SATURNUS_COMMANDS_DIR").map_or(root, Into::into);
    let file = dir.join(format!("{}.json", model.name()));
    let Ok(text) = std::fs::read_to_string(&file) else {
        eprintln!("{model:?}: no {}: catalog check skipped", file.display());
        return;
    };
    let catalog: serde_json::Value = serde_json::from_str(&text).unwrap();
    let mut checked = 0;
    for c in catalog["commands"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        for x in c["xlib"].as_array().unwrap() {
            let lib = x[0].as_u64().unwrap() as u16;
            let cmd = x[1].as_u64().unwrap() as u16;
            let got = names.xlib(lib, cmd).and_then(|i| i.name);
            assert_eq!(got, Some(name), "{model:?}: XLIB {lib} {cmd}");
            checked += 1;
        }
    }
    eprintln!("{model:?}: all {checked} catalog names resolve");
}
