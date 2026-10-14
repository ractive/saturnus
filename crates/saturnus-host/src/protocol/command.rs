//! The protocol's commands (`web/protocol.md`) and what each one is: one
//! [`CommandKind`] per command name, and its [`Policy`] from one
//! exhaustive match, so a new command does not compile until its policy
//! is written. The engine's refusals ([`super::Engine::refusal`]), its
//! dispatch, the native runner's own commands and the HTTP control API's
//! endpoints all read it.

use std::str::FromStr;

/// The HTTP control API's endpoint (`saturnus run`, `web/protocol.md`,
/// "HTTP") whose JSON `POST` carries a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpEndpoint {
    /// `POST /v1/keys`.
    Keys,
    /// `POST /v1/type`.
    Type,
    /// `POST /v1/mem`.
    Mem,
    /// `POST /v1/memory`: the writes and `editText`.
    Memory,
}

/// What a command is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Policy {
    /// Refused while a send or a write runs: it presses keys, swaps or
    /// resets the machine, or reads the user memory the send changes.
    pub refused_while_typing: bool,
    /// A write to the user memory: a hidden Kermit transaction over the
    /// serial port ([`crate::transfer`]), refused while the page holds
    /// the port.
    pub write: bool,
    /// A question to the Kermit server over the serial port
    /// (`probeServer`, `endServer`): refused while the page holds it.
    pub server: bool,
    /// Uses the serial port a write needs: refused while a write runs.
    pub serial: bool,
    /// Served by the native hosts around the engine (`saturnus-drive`'s
    /// runner), not by the engine.
    pub native: bool,
    /// The HTTP endpoint whose `POST` takes it, if any.
    pub http: Option<HttpEndpoint>,
}

impl Policy {
    /// Nothing special: served at any time, not over HTTP's JSON posts.
    const PLAIN: Policy = Policy {
        refused_while_typing: false,
        write: false,
        server: false,
        serial: false,
        native: false,
        http: None,
    };
    /// Refused while a send types.
    const BUSY: Policy = Policy {
        refused_while_typing: true,
        ..Policy::PLAIN
    };
    /// A write to the user memory (`POST /v1/memory`).
    const WRITE: Policy = Policy {
        refused_while_typing: true,
        write: true,
        http: Some(HttpEndpoint::Memory),
        ..Policy::PLAIN
    };
}

macro_rules! commands {
    ($($kind:ident = $name:literal,)*) => {
        /// A command of the protocol, by its `cmd` name.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum CommandKind {
            $(
                #[doc = concat!("`", $name, "`.")]
                $kind,
            )*
        }

        impl CommandKind {
            /// Every command.
            pub const ALL: &'static [CommandKind] = &[$(CommandKind::$kind,)*];

            /// Its `cmd` name.
            pub const fn name(self) -> &'static str {
                match self {
                    $(CommandKind::$kind => $name,)*
                }
            }
        }

        impl FromStr for CommandKind {
            type Err = String;

            fn from_str(s: &str) -> Result<Self, String> {
                match s {
                    $($name => Ok(CommandKind::$kind),)*
                    other => Err(format!("unknown command {other:?}")),
                }
            }
        }
    };
}

commands! {
    Hello = "hello",
    Skin = "skin",
    Layout = "layout",
    Boot = "boot",
    KeyDown = "keyDown",
    KeyUp = "keyUp",
    KeyUpAll = "keyUpAll",
    TypeLetter = "typeLetter",
    TypeKeys = "typeKeys",
    ReleaseAll = "releaseAll",
    SetSpeed = "setSpeed",
    Pause = "pause",
    Unload = "unload",
    Reset = "reset",
    SetClock = "setClock",
    SaveState = "saveState",
    LoadState = "loadState",
    StateSlot = "stateSlot",
    OpenState = "openState",
    SaveFile = "saveFile",
    Visibility = "visibility",
    Stats = "stats",
    RomSlots = "romSlots",
    BootModel = "bootModel",
    ChooseRom = "chooseRom",
    DownloadRom = "downloadRom",
    ForgetRom = "forgetRom",
    RomSettings = "romSettings",
    WatchMemory = "watchMemory",
    MemoryTree = "memoryTree",
    Stack = "stack",
    StackTop = "stackTop",
    Flags = "flags",
    ObjectAt = "objectAt",
    CommandLine = "commandLine",
    StoreFile = "storeFile",
    FetchFile = "fetchFile",
    Purge = "purge",
    Rename = "rename",
    CreateDir = "createDir",
    ChangeDir = "changeDir",
    SetFlag = "setFlag",
    Copy = "copy",
    Move = "move",
    StoreScreen = "storeScreen",
    Backup = "backup",
    EditText = "editText",
    StoreText = "storeText",
    Insert = "insert",
    TypeText = "typeText",
    Run = "run",
    Replace = "replace",
    ProbeServer = "probeServer",
    EndServer = "endServer",
    SerialOpen = "serialOpen",
    SerialWrite = "serialWrite",
    SerialClose = "serialClose",
    Join = "join",
    Ping = "ping",
    Attach = "attach",
    Leave = "leave",
    Screen = "screen",
    Info = "info",
    Model = "model",
    Peek = "peek",
    Poke = "poke",
    KeyScript = "keyScript",
}

impl CommandKind {
    /// What the command is: one match, no default, so a new command
    /// states its policy.
    pub const fn policy(self) -> Policy {
        use CommandKind as C;
        let native = Policy {
            native: true,
            ..Policy::PLAIN
        };
        match self {
            C::Hello
            | C::Skin
            | C::Layout
            | C::SetSpeed
            | C::Pause
            | C::StateSlot
            | C::OpenState
            | C::SaveFile
            | C::Visibility
            | C::Stats
            | C::RomSlots
            | C::DownloadRom
            | C::ForgetRom
            | C::RomSettings
            | C::WatchMemory
            | C::CommandLine
            | C::Join
            | C::Ping
            | C::Attach
            | C::Leave => Policy::PLAIN,
            C::Boot
            | C::Unload
            | C::Reset
            | C::SetClock
            | C::SaveState
            | C::LoadState
            | C::BootModel
            | C::ChooseRom
            | C::MemoryTree
            | C::Stack
            | C::StackTop
            | C::Flags
            | C::ObjectAt
            | C::TypeLetter => Policy::BUSY,
            C::KeyDown | C::KeyUp | C::TypeKeys => Policy {
                http: Some(HttpEndpoint::Keys),
                ..Policy::BUSY
            },
            C::ReleaseAll | C::KeyUpAll => Policy {
                http: Some(HttpEndpoint::Keys),
                ..Policy::PLAIN
            },
            C::Insert | C::TypeText | C::Run | C::Replace => Policy {
                http: Some(HttpEndpoint::Type),
                ..Policy::BUSY
            },
            C::StoreFile
            | C::FetchFile
            | C::Purge
            | C::Rename
            | C::CreateDir
            | C::ChangeDir
            | C::SetFlag
            | C::Copy
            | C::Move
            | C::StoreScreen
            | C::StoreText
            | C::Backup => Policy::WRITE,
            // The read that goes with `storeText`.
            C::EditText => Policy {
                http: Some(HttpEndpoint::Memory),
                ..Policy::BUSY
            },
            C::ProbeServer | C::EndServer => Policy {
                server: true,
                ..Policy::BUSY
            },
            C::SerialOpen | C::SerialWrite => Policy {
                serial: true,
                ..Policy::PLAIN
            },
            C::SerialClose => Policy::PLAIN,
            C::Screen | C::Info | C::Model | C::Peek => native,
            C::Poke => Policy {
                refused_while_typing: true,
                native: true,
                http: Some(HttpEndpoint::Mem),
                ..Policy::PLAIN
            },
            C::KeyScript => Policy {
                refused_while_typing: true,
                native: true,
                http: Some(HttpEndpoint::Keys),
                ..Policy::PLAIN
            },
        }
    }

    /// The commands whose policy says `f`.
    pub fn with(f: impl Fn(Policy) -> bool) -> impl Iterator<Item = CommandKind> {
        CommandKind::ALL
            .iter()
            .copied()
            .filter(move |k| f(k.policy()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_go_both_ways() {
        for &k in CommandKind::ALL {
            assert_eq!(k.name().parse::<CommandKind>(), Ok(k));
        }
        assert_eq!(
            "nope".parse::<CommandKind>(),
            Err("unknown command \"nope\"".to_string())
        );
    }

    /// The tables the policy replaced, as they were (plus `backup`).
    #[test]
    fn the_policy_keeps_the_old_tables() {
        let names = |f: fn(Policy) -> bool| {
            let mut v: Vec<&str> = CommandKind::with(f).map(CommandKind::name).collect();
            v.sort_unstable();
            v
        };
        let sorted = |mut v: Vec<&'static str>| {
            v.sort_unstable();
            v
        };
        assert_eq!(
            names(|p| p.write),
            sorted(vec![
                "storeFile",
                "fetchFile",
                "purge",
                "rename",
                "createDir",
                "changeDir",
                "setFlag",
                "storeText",
                "copy",
                "move",
                "storeScreen",
                "backup",
            ])
        );
        assert_eq!(names(|p| p.server), ["endServer", "probeServer"]);
        assert_eq!(names(|p| p.serial), ["serialOpen", "serialWrite"]);
        assert_eq!(
            names(|p| p.native),
            ["info", "keyScript", "model", "peek", "poke", "screen"]
        );
        assert_eq!(names(|p| p.refused_while_typing).len(), 38);
        assert_eq!(
            names(|p| p.http == Some(HttpEndpoint::Keys)),
            [
                "keyDown",
                "keyScript",
                "keyUp",
                "keyUpAll",
                "releaseAll",
                "typeKeys"
            ]
        );
        assert_eq!(names(|p| p.http == Some(HttpEndpoint::Mem)), ["poke"]);
        assert_eq!(
            names(|p| p.http == Some(HttpEndpoint::Type)),
            ["insert", "replace", "run", "typeText"]
        );
    }
}
