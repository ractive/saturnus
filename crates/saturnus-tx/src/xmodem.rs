//! The fast transfer of the 48GX and 49G: one variable put or got by
//! XModem instead of Kermit ([`Request::XmodemPut`], [`Request::XmodemGet`]).
//!
//! `XRECV` and `XSEND` cannot run inside the Kermit server (wiki:
//! protocols/xmodem-hp, "Port Not Available"), so a fast transfer is:
//!
//! 1. Checks while the server still answers: the model has XModem (48G/GX,
//!    49G), the name, the directory's listing (a put's name must be free:
//!    the 48GX refuses a name in use with "Name Conflict" and the 49G
//!    stores the object as `NAME.1` instead; a get's variable must be
//!    there), and a put's file is a binary transfer file of the model's
//!    family (XModem on the calculators is binary only).
//! 2. A 49G in algebraic mode is switched to RPN (flag -95, one host
//!    command that also says whether it was), then the server ends
//!    (FINISH), and the [`XmodemPlan`] says what to type: `'NAME' XRECV`
//!    or `'NAME' XSEND`. The steps of `satx-core`'s `PrepareXmodem`, done
//!    here so that the listing of step 1 is not read again and the RPN
//!    switch is known before FINISH. From FINISH on, sent whether or not
//!    it was answered, the device has left server mode
//!    ([`Device::left_server`]): every request but [`Request::Connect`] is
//!    [`TxError::LeftServer`].
//! 3. [`Event::Xmodem`] with the plan, for the host to show.
//! 4. The transfer ([`Op::XmodemSend`], [`Op::XmodemReceive`] with
//!    [`XmodemOptions::for_model`]: 1k blocks and HP's CRC on the 49G,
//!    128-byte blocks and checksum on the 48GX), waiting up to the
//!    request's `wait` for the user to start the command.
//! 5. The user starts `SERVER` again and the host connects: the transfer
//!    mode the user had before the fast transfer is kept for the
//!    disconnect, and a 49G switched to RPN goes back to algebraic mode
//!    (`-95 SF`, as the satx tool does; wiki: protocols/xmodem-hp).
//!
//! [`transfer`] is the part after the server has ended, for any XModem
//! peer the host drives through a [`Machine`](satx_core::Machine): the
//! same session code serves another calculator's XModem (an aplet sent to
//! a 39G or 40G).
//!
//! [`Device::left_server`]: crate::Device::left_server

use satx_core::calc::validate_name;
use satx_core::machine::{Op, Reply};
use satx_core::time::Duration;
use satx_core::xmodem::{XmodemDirection, XmodemOptions, XmodemPlan};
use satx_core::{Model, TransferMode};

use crate::copy;
use satx_core::reply::parse_real;

use crate::device::{
    self, Ctx, Event, File, Format, LeftServer, Outcome, Result, TxError, list_dir,
};
use crate::names::{self, FileKind};

/// How long a fast transfer waits for the user to start `XRECV` or
/// `XSEND` by default: typing the name and the command takes a while on
/// a calculator's keyboard.
pub const DEFAULT_WAIT: Duration = Duration::from_secs(180);

/// Whether `model` has the fast transfer.
pub fn has_xmodem(model: Model) -> bool {
    matches!(model, Model::Hp48Gx | Model::Hp49G)
}

/// The XModem options for `model`, waiting `wait` for the start.
pub fn options(model: Model, wait: Duration) -> Result<XmodemOptions> {
    if !has_xmodem(model) {
        return Err(TxError::NotForXmodem(format!(
            "the {} has no fast transfer; the normal copy works",
            model.name()
        )));
    }
    let mut options = XmodemOptions::for_model(model)?;
    options.start_timeout = wait;
    Ok(options)
}

/// Why `data` cannot go to a `model` by XModem, or `None`.
pub fn put_problem(data: &[u8], model: Model) -> Option<String> {
    if FileKind::of(data) != FileKind::Binary {
        return Some(
            "only a binary calculator file (.hp) goes by fast transfer; the normal copy takes \
             text and other files"
                .into(),
        );
    }
    copy::header_check(data, model).map(|m| {
        format!(
            "{} The normal copy sends it as text instead.",
            m.file_message()
        )
    })
}

/// The transfer once the server has ended: send `data` (a put) or receive
/// a file (a get) with `options`.
pub(crate) async fn transfer(
    ctx: &Ctx,
    data: Option<Vec<u8>>,
    options: XmodemOptions,
) -> Result<Reply> {
    let op = match data {
        Some(data) => Op::XmodemSend { data, options },
        None => Op::XmodemReceive { options },
    };
    ctx.op(op).await
}

/// The host command that switches a 49G to RPN and leaves 1 on the stack
/// when it was in algebraic mode, 0 otherwise.
const TO_RPN: &str = "IF -95. FS? THEN -95. CF 1. ELSE 0. END";

/// A 49G in algebraic mode switched to RPN: whether it was.
async fn to_rpn(ctx: &Ctx) -> Result<bool> {
    let levels = match ctx.op(Op::Run(TO_RPN.to_string())).await? {
        Reply::Stack(reply) => match reply.error {
            None => reply.levels,
            Some(message) => {
                return Err(TxError::Calc(satx_core::Error::Calculator {
                    message,
                    stack: reply.levels,
                }));
            }
        },
        other => return Err(device::unexpected("-95. FS?", &other)),
    };
    let was = levels.first().and_then(|v| parse_real(v)) == Some(1.0);
    // The answer off the user's stack again.
    device::expect_stack(ctx.op(Op::Run("DROP".to_string())).await?)?;
    Ok(was)
}

/// End the server for `direction` and `name` in `dir` (checked already,
/// the current directory): the plan, told to the host as
/// [`Event::Xmodem`].
async fn prepare(
    ctx: &Ctx,
    direction: XmodemDirection,
    name: &str,
    exists: bool,
) -> Result<XmodemPlan> {
    let connection = ctx.session(|s| s.connection.clone())?;
    let model = connection.model;
    let algebraic = model == Model::Hp49G && to_rpn(ctx).await?;
    // Recorded before FINISH: a FINISH whose answer is lost may still have
    // ended the server, and the reconnect must put algebraic mode back.
    ctx.left_server(LeftServer {
        user_mode: connection.user_mode,
        algebraic,
    });
    device::expect_done(ctx.op(Op::Finish).await?)?;
    let mut notes = Vec::new();
    if direction == XmodemDirection::ToCalculator && model == Model::Hp48Gx {
        notes.push(format!(
            "If the transfer fails, {name} may be left holding an empty string."
        ));
    }
    if algebraic {
        notes.push(
            "The calculator was in algebraic mode and is now in RPN mode; the reconnect \
             switches it back."
                .to_string(),
        );
    }
    notes.push(
        "The calculator has left server mode; start SERVER on it after the transfer.".to_string(),
    );
    let plan = XmodemPlan {
        model,
        direction,
        name: name.to_string(),
        keys: format!("'{name}' {}", direction.calculator_command()),
        switched_to_rpn: algebraic,
        exists,
        notes,
    };
    ctx.event(Event::Xmodem(plan.clone()));
    Ok(plan)
}

/// [`Request::XmodemPut`](crate::Request::XmodemPut).
pub(crate) async fn put(
    ctx: &Ctx,
    dir: &[String],
    name: String,
    data: Vec<u8>,
    wait: Duration,
) -> Result<Outcome> {
    let model = ctx.model()?;
    let options = options(model, wait)?;
    validate_name(&name).map_err(|_| TxError::BadName(name.clone()))?;
    if let Some(why) = put_problem(&data, model) {
        return Err(TxError::NotForXmodem(why));
    }
    let listing = list_dir(ctx, dir).await?;
    if let Some(entry) = listing.entries.iter().find(|e| e.name == name) {
        return Err(if entry.is_directory() {
            TxError::IsDirectory(name)
        } else {
            TxError::NameClash(name)
        });
    }
    prepare(ctx, XmodemDirection::ToCalculator, &name, false).await?;
    match transfer(ctx, Some(data), options).await? {
        Reply::XmodemSent(_) => Ok(Outcome::Stored(name)),
        other => Err(device::unexpected("XRECV", &other)),
    }
}

/// [`Request::XmodemGet`](crate::Request::XmodemGet).
pub(crate) async fn get(
    ctx: &Ctx,
    dir: &[String],
    name: String,
    wait: Duration,
) -> Result<Outcome> {
    let model = ctx.model()?;
    let options = options(model, wait)?;
    validate_name(&name).map_err(|_| TxError::BadName(name.clone()))?;
    let listing = list_dir(ctx, dir).await?;
    match listing.entries.iter().find(|e| e.name == name) {
        None => return Err(TxError::NotFound(name)),
        // A directory goes as a folder of files, by the normal copy.
        Some(entry) if entry.is_directory() => return Err(TxError::IsDirectory(name)),
        Some(_) => {}
    }
    prepare(ctx, XmodemDirection::FromCalculator, &name, true).await?;
    match transfer(ctx, None, options).await? {
        Reply::XmodemReceived(got) => Ok(Outcome::File(File {
            file_name: names::file_name(&name, Format::Binary),
            name,
            format: Format::Binary,
            data: got.data,
        })),
        other => Err(device::unexpected("XSEND", &other)),
    }
}

/// After the reconnect that follows a fast transfer: a 49G that was
/// switched to RPN for it goes back to algebraic mode. Best effort: the
/// connection is good either way.
pub(crate) async fn after_reconnect(ctx: &Ctx, left: &LeftServer) {
    if left.algebraic {
        let _ = ctx.op(Op::Run("-95. SF".to_string())).await;
    }
}

/// The transfer mode a connect keeps for the disconnect: the user's from
/// before the fast transfer, not the binary mode the page had set.
pub(crate) fn user_mode(read: TransferMode, left: Option<&LeftServer>) -> TransferMode {
    left.map_or(read, |l| l.user_mode)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn only_the_48gx_and_49g() {
        assert!(has_xmodem(Model::Hp48Gx) && has_xmodem(Model::Hp49G));
        assert!(!has_xmodem(Model::Hp48Sx) && !has_xmodem(Model::Unknown));
        assert!(matches!(
            options(Model::Hp48Sx, DEFAULT_WAIT),
            Err(TxError::NotForXmodem(_))
        ));
        let gx = options(Model::Hp48Gx, Duration::from_secs(30)).unwrap();
        assert_eq!(gx.start_timeout, Duration::from_secs(30));
        assert_eq!(
            gx.xmodem.block_size,
            satx_core::xmodem_proto::BlockSize::B128
        );
        let g49 = options(Model::Hp49G, DEFAULT_WAIT).unwrap();
        assert_eq!(
            g49.xmodem.block_size,
            satx_core::xmodem_proto::BlockSize::B1k
        );
    }

    #[test]
    fn a_put_takes_binary_files_of_the_family() {
        let file48 = b"HPHP48-R\x2c\x2a\x20\x00\x00".to_vec();
        let file49 = b"HPHP49-C\x2c\x2a\x20\x00\x00".to_vec();
        assert_eq!(put_problem(&file48, Model::Hp48Gx), None);
        assert_eq!(put_problem(&file49, Model::Hp49G), None);
        assert!(
            put_problem(&file48, Model::Hp49G)
                .unwrap()
                .contains("HP 48 file")
        );
        assert!(
            put_problem(&file49, Model::Hp48Gx)
                .unwrap()
                .contains("HP 49G file")
        );
        let text = b"%%HP: T(3)A(R)F(.);\r\n1\r\n".to_vec();
        assert!(
            put_problem(&text, Model::Hp48Gx)
                .unwrap()
                .contains("binary")
        );
        assert!(put_problem(b"hello", Model::Hp49G).is_some());
    }

    #[test]
    fn the_user_mode_from_before_wins() {
        let left = LeftServer {
            user_mode: TransferMode::Ascii,
            algebraic: false,
        };
        assert_eq!(
            user_mode(TransferMode::Binary, Some(&left)),
            TransferMode::Ascii
        );
        assert_eq!(user_mode(TransferMode::Binary, None), TransferMode::Binary);
    }
}
