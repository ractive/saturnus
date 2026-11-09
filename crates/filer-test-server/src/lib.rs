//! The palmtop of Files' tests: `filer-proto`'s Filer server over its
//! in-memory files, as WebAssembly for the page's and Node's tests
//! (`web/test/tx-fake-palmtop.js`, `tx-palmtop.test.mjs`,
//! `tx-page.test.mjs`). A package of its own, built by
//! `web/test/build-filer.sh` (`just web`) into `web/test/pkg-filer/`, so
//! the app's wasm (`crates/saturnus-web`, what the site ships) never holds
//! a test server. Not published.

use filer_proto::{MemoryFs, Server, ServerConfig};
use js_sys::{Array, Reflect, Uint8Array};
use wasm_bindgen::prelude::*;

fn js_err(e: impl std::fmt::Display) -> JsValue {
    js_sys::Error::new(&e.to_string()).into()
}

/// `filer-proto`'s server over its in-memory files: a palmtop in Filer
/// server mode for the page's tests, on a link of their own
/// (`web/test/tx-palmtop.test.mjs`). Drives `A:`, `C:` and `D:`, D:
/// write-protected as the palmtop's ROM disk.
#[wasm_bindgen]
pub struct TxFilerServer {
    server: Server<MemoryFs>,
}

impl std::fmt::Debug for TxFilerServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TxFilerServer").finish_non_exhaustive()
    }
}

/// A DOS path of the server's files from the page's `C:\SUB\X.TXT`.
fn server_path(path: &str) -> Result<Vec<u8>, JsValue> {
    filer_proto::path::normalize(path, b'C').map_err(js_err)
}

#[wasm_bindgen]
impl TxFilerServer {
    /// A server holding `files` (`[{path: "C:\\SUB\\X.TXT", data}]`; no
    /// `data`: a directory).
    #[wasm_bindgen(constructor)]
    pub fn new(files: JsValue) -> Result<TxFilerServer, JsValue> {
        let mut fs = MemoryFs::new(b"ACD");
        fs.set_read_only(b'D');
        if Array::is_array(&files) {
            for item in Array::from(&files).iter() {
                let path = Reflect::get(&item, &"path".into())?
                    .as_string()
                    .ok_or_else(|| js_err("a file's path is a string"))?;
                let path = server_path(&path)?;
                let data = Reflect::get(&item, &"data".into())?;
                if data.is_instance_of::<Uint8Array>() {
                    fs.insert_file(&path, Uint8Array::new(&data).to_vec());
                } else {
                    fs.insert_dir(&path);
                }
            }
        }
        Ok(TxFilerServer {
            server: Server::new(fs, ServerConfig::default()),
        })
    }

    /// Bytes the page wrote.
    pub fn input(&mut self, bytes: &[u8]) {
        self.server.handle_input(bytes);
    }

    /// The replies to send back, each a `Uint8Array`.
    pub fn output(&mut self) -> Array {
        let out = Array::new();
        while let Some(frame) = self.server.poll_output() {
            out.push(&Uint8Array::from(frame.as_slice()));
        }
        out
    }

    /// Lose the next `n` replies (a lot: a palmtop gone silent; 0 answers
    /// again).
    #[wasm_bindgen(js_name = dropReplies)]
    pub fn drop_replies(&mut self, n: u32) {
        self.server.drop_replies(n);
    }

    /// A file's bytes, or `undefined`.
    pub fn file(&self, path: &str) -> Result<Option<Uint8Array>, JsValue> {
        let path = server_path(path)?;
        Ok(self.server.fs().file(&path).map(Uint8Array::from))
    }

    /// Every path the server holds, `C:\SUB\X.TXT`.
    pub fn paths(&self) -> Array {
        self.server
            .fs()
            .paths()
            .iter()
            .map(|p| JsValue::from(filer_proto::path::display(p)))
            .collect()
    }
}
