//! Planning a write: the steps of each [`Op`].

use super::*;

impl Transfer {
    /// Check `op` against the calculator as it is and plan it; nothing
    /// runs yet, and an error leaves the machine untouched.
    pub fn new(m: &Machine, op: Op) -> crate::Result<Transfer> {
        if let Some(why) = no_server(m.model()) {
            return Err(why.into());
        }
        if let Op::Server { end } = op {
            let steps = if may_serve(m) {
                VecDeque::from([Step::Ask { end }])
            } else {
                VecDeque::new()
            };
            let result = TransferResult {
                server: Some(false),
                ..TransferResult::default()
            };
            return Ok(Transfer::planned(m, steps, result));
        }
        let u = UserMemory::of(m).map_err(|e| format!("{e:#}"))?;
        let flags = u.flags().map_err(|e| format!("{e:#}"))?;
        let tree = u.tree().map_err(|e| format!("{e:#}"))?;
        let path = u.current_path().map_err(|e| format!("{e:#}"))?;
        let here: Vec<String> = path.iter().skip(1).cloned().collect();
        let line = cmdline::command_line(m).map_err(|e| format!("{e:#}"))?;
        if line.active {
            return Err(
                "the command line is open: finish it (ENTER) or cancel it (ON) first"
                    .to_string()
                    .into(),
            );
        }
        let mut result = TransferResult::default();
        let mut steps = VecDeque::new();
        let alg = algebraic(m.model(), &flags);
        if let Op::SetClock { time, date } = &op {
            let depth = u.stack_addresses().map_err(|e| format!("{e:#}"))?.len();
            let day_first = flags.get(-42) == Some(true);
            let comma = flags.get(-51) == Some(true);
            for line in crate::clock::rpl_lines(time, date, day_first, comma, alg) {
                steps.push_back(Step::Type(line));
                // A line the calculator refused is cancelled; what an
                // error left on the stack, or algebraic mode's echo, is
                // dropped.
                steps.push_back(Step::CancelLine);
                steps.push_back(Step::DropTo(depth));
            }
            result.keys = true;
            return Ok(Transfer::planned(m, steps, result));
        }
        if let Op::SetFlag { flag, on } = op {
            let max = flag_range(m.model());
            if flag == 0 || flag.abs() > max {
                return Err(format!("flag {flag} is not between -{max} and {max} (not 0)").into());
            }
            let verb = if on { "SF" } else { "CF" };
            if alg {
                // A server entered in algebraic mode packs the stack in a
                // list: keys instead.
                let depth = u.stack_addresses().map_err(|e| format!("{e:#}"))?.len();
                steps.push_back(Step::Type(format!("{verb}({flag})")));
                steps.push_back(Step::DropTo(depth));
                result.keys = true;
                return Ok(Transfer::planned(m, steps, result));
            }
            steps.push_back(Step::Host {
                text: format!("{flag} {verb}"),
                cleanup: false,
            });
            return Transfer::serve(m, &flags, steps, result);
        }
        let mut back = None;
        let mut temp_variable = matches!(op, Op::StoreText { .. } | Op::StoreScreen { .. });
        let mut replaced = None;
        let mut backup_name = None;
        match op {
            Op::ChangeDir { dir } => {
                let dir = components(&dir)?;
                directory(&tree, dir)?;
                steps.push_back(Step::Host {
                    text: cd_command(dir),
                    cleanup: false,
                });
            }
            Op::Store {
                dir,
                name,
                data,
                replace,
            } => {
                check_name(&name)?;
                if data.len() > MAX_FILE_BYTES {
                    return Err(format!("the file is larger than {MAX_FILE_BYTES} bytes").into());
                }
                let dir = components(&dir)?;
                let vars = directory(&tree, dir)?;
                let taken = vars.iter().find(|v| v.name == name);
                let replacing = match (replace, taken) {
                    (None, _) | (Some(_), None) => false,
                    (Some(_), Some(v)) if v.variables.is_some() => {
                        return Err(format!(
                            "{name} in {{ {} }} is a directory, which a file does not replace: purge it first",
                            cd_command(dir)
                        )
                        .into());
                    }
                    (Some(false), Some(_)) => {
                        return Err(
                            format!("{name} already exists in {{ {} }}", cd_command(dir)).into(),
                        );
                    }
                    (Some(true), Some(_)) => true,
                };
                let directory_file = holds_directory(&data);
                let size = data.len();
                let (data, binary) = store_bytes(&data);
                back = Transfer::enter(&mut steps, dir, &here);
                if replacing {
                    // Under a free name first: a failed transfer leaves
                    // the old variable as it was.
                    let temp = free_name(vars, REPLACE_VARIABLE);
                    let (t, q) = (quoted(&temp), quoted(&name));
                    if directory_file {
                        // A directory goes over the old variable only once
                        // that is purged: checked first that the temporary
                        // copy and the recalled one fit, so nothing is
                        // purged that cannot be replaced.
                        steps.push_back(Step::Free {
                            needed: 2.0 * size as f64 + BACKUP_MARGIN,
                            refusal: format!("replacing {name} needs "),
                        });
                    }
                    Transfer::with_mode(
                        &mut steps,
                        &flags,
                        binary,
                        Step::Send {
                            name: temp.clone(),
                            data,
                        },
                    );
                    // STO overwrites the old variable (as `copy` does);
                    // the temporary one goes only once the name holds the
                    // new object, and an error names it while it is the
                    // new object's only copy.
                    let mut texts = Vec::new();
                    if directory_file {
                        texts.push(format!("{q} PURGE"));
                    }
                    texts.push(format!("{t} RCL {q} STO"));
                    texts.push(format!(
                        "{t} {}",
                        if directory_file { "PGDIR" } else { "PURGE" }
                    ));
                    for text in texts {
                        steps.push_back(Step::Host {
                            text,
                            cleanup: false,
                        });
                    }
                    temp_variable = true;
                    replaced = Some((temp, name.clone(), cd_command(dir)));
                    result.name = Some(name);
                } else {
                    Transfer::with_mode(&mut steps, &flags, binary, Step::Send { name, data });
                }
            }
            Op::Fetch { dir, name, text } => {
                check_name(&name)?;
                let dir = components(&dir)?;
                let vars = directory(&tree, dir)?;
                if !vars.iter().any(|v| v.name == name) {
                    return Err(format!("no variable {name} in {{ {} }}", cd_command(dir)).into());
                }
                back = Transfer::enter(&mut steps, dir, &here);
                Transfer::with_mode(
                    &mut steps,
                    &flags,
                    !text,
                    Step::Get {
                        name: name.clone(),
                        text,
                    },
                );
                result.name = Some(name);
            }
            Op::Backup => {
                let home: f64 = tree.iter().map(|v| v.size).sum();
                let needed = 2.0 * home + BACKUP_MARGIN;
                let n = free_name(&tree, BACKUP_VARIABLE);
                let (port, q) = (format!(":0:{n}"), quoted(&n));
                back = Transfer::enter(&mut steps, &[], &here);
                steps.push_back(Step::Free {
                    needed,
                    refusal: BACKUP_NO_ROOM.to_string(),
                });
                // STO before the port's copy is purged, else "Object In
                // Use". The purges also run after an error, the port's
                // only when this ARCHIVE made it (a name taken in port 0
                // makes ARCHIVE fail, and the port keeps it); `N` is free
                // in HOME.
                for text in [
                    format!("{port} ARCHIVE"),
                    format!("{port} RCL"),
                    format!("{q} STO"),
                ] {
                    steps.push_back(Step::Host {
                        text,
                        cleanup: false,
                    });
                }
                steps.push_back(Step::Unarchive);
                Transfer::with_mode(
                    &mut steps,
                    &flags,
                    true,
                    Step::Get {
                        name: n.clone(),
                        text: false,
                    },
                );
                steps.push_back(Step::Host {
                    text: Leftovers::unstore(&n),
                    cleanup: true,
                });
                result.needed = Some(needed);
                backup_name = Some(n);
            }
            Op::Purge { dir, name } => {
                check_name(&name)?;
                let dir = components(&dir)?;
                let vars = directory(&tree, dir)?;
                let var = vars
                    .iter()
                    .find(|v| v.name == name)
                    .ok_or_else(|| format!("no variable {name} in {{ {} }}", cd_command(dir)))?;
                if var.variables.is_some() && inside(&here, dir, &name) {
                    return Err(format!(
                        "{name} holds the current directory: change to another one first"
                    )
                    .into());
                }
                let verb = if var.variables.is_some() {
                    "PGDIR"
                } else {
                    "PURGE"
                };
                back = Transfer::enter(&mut steps, dir, &here);
                steps.push_back(Step::Host {
                    text: format!("{} {verb}", quoted(&name)),
                    cleanup: false,
                });
            }
            Op::Rename { dir, name, to } => {
                check_name(&name)?;
                check_name(&to)?;
                let dir = components(&dir)?;
                let vars = directory(&tree, dir)?;
                let var = vars
                    .iter()
                    .find(|v| v.name == name)
                    .ok_or_else(|| format!("no variable {name} in {{ {} }}", cd_command(dir)))?;
                if vars.iter().any(|v| v.name == to) {
                    return Err(format!("{to} already exists in {{ {} }}", cd_command(dir)).into());
                }
                if var.variables.is_some() && inside(&here, dir, &name) {
                    return Err(format!(
                        "{name} holds the current directory: change to another one first"
                    )
                    .into());
                }
                let verb = if var.variables.is_some() {
                    "PGDIR"
                } else {
                    "PURGE"
                };
                back = Transfer::enter(&mut steps, dir, &here);
                // Copy, then purge the old name: on a failure the old one
                // is still there.
                steps.push_back(Step::Host {
                    text: format!("{} RCL {} STO", quoted(&name), quoted(&to)),
                    cleanup: false,
                });
                steps.push_back(Step::Host {
                    text: format!("{} {verb}", quoted(&name)),
                    cleanup: false,
                });
            }
            Op::CreateDir { dir, name } => {
                check_name(&name)?;
                let dir = components(&dir)?;
                let vars = directory(&tree, dir)?;
                if vars.iter().any(|v| v.name == name) {
                    return Err(
                        format!("{name} already exists in {{ {} }}", cd_command(dir)).into(),
                    );
                }
                back = Transfer::enter(&mut steps, dir, &here);
                steps.push_back(Step::Host {
                    text: format!("{} CRDIR", quoted(&name)),
                    cleanup: false,
                });
            }
            Op::Copy {
                dir,
                name,
                to,
                replace,
                remove,
            } => {
                check_name(&name)?;
                let from = components(&dir)?;
                let into = components(&to)?;
                let vars = directory(&tree, from)?;
                let var = vars
                    .iter()
                    .find(|v| v.name == name)
                    .ok_or_else(|| format!("no variable {name} in {{ {} }}", cd_command(from)))?;
                let targets = directory(&tree, into)?;
                let is_dir = var.variables.is_some();
                for step in copy_plan(
                    from,
                    &name,
                    into,
                    &CopyCase {
                        is_dir,
                        taken: targets
                            .iter()
                            .find(|v| v.name == name)
                            .map(|v| v.variables.is_some()),
                        replace,
                        remove,
                        holds_here: is_dir && inside(&here, from, &name),
                    },
                )? {
                    steps.push_back(step);
                }
                // The commands change directory: back to the current one.
                if here.iter().all(|n| is_plain_name(n)) {
                    back = Some(cd_command(&here));
                }
            }
            Op::StoreText { dir, target, text } => {
                if let Some(why) = text_refusal(&text) {
                    return Err(why.into());
                }
                let family = tfile::Family::of(m.model());
                let string = saturnus_objects::Object::String {
                    value: wrapped(&text),
                };
                let file = tfile::encode_file(&string, family)
                    .map_err(|e| format!("{e:#}"))?
                    .ok_or("a string has a binary file")?;
                if file.len() > MAX_FILE_BYTES {
                    return Err(format!("the text is longer than {MAX_FILE_BYTES} bytes").into());
                }
                let place = match &target {
                    Target::Variable(name) => {
                        check_name(name)?;
                        let dir = components(&dir)?;
                        let vars = directory(&tree, dir)?;
                        if vars
                            .iter()
                            .any(|v| &v.name == name && v.variables.is_some())
                        {
                            return Err(format!("{name} is a directory, which has no text").into());
                        }
                        back = Transfer::enter(&mut steps, dir, &here);
                        format!("DROP 1 GET {} STO", quoted(name))
                    }
                    Target::Level(n) => {
                        let depth = u.stack_addresses().map_err(|e| format!("{e:#}"))?.len();
                        if *n == 0 || *n > depth {
                            return Err(
                                format!("there is no level {n} (the stack has {depth})").into()
                            );
                        }
                        // The new object goes where the old one was.
                        format!("DROP 1 GET {} ROLL DROP {n} ROLLD", n + 1)
                    }
                };
                let vars = match &target {
                    Target::Variable(_) => directory(&tree, components(&dir)?)?,
                    Target::Level(_) => directory(&tree, &here)?,
                };
                let temp = free_name(vars, TEXT_VARIABLE);
                Transfer::with_mode(
                    &mut steps,
                    &flags,
                    true,
                    Step::Send {
                        name: temp.clone(),
                        data: file,
                    },
                );
                // Recalled and purged before it is compiled, so a failed
                // compile leaves nothing behind.
                steps.push_back(Step::Compile {
                    text: format!("{t} RCL {t} PURGE STR→ DUP SIZE 2 MIN", t = quoted(&temp)),
                });
                steps.push_back(Step::Host {
                    text: place,
                    cleanup: false,
                });
            }
            Op::StoreScreen {
                dir,
                name,
                replace,
                screen,
            } => {
                let file = tfile::graphic_file(&screen, tfile::Family::of(m.model()))
                    .map_err(|e| format!("{e:#}"))?;
                let vars = match &name {
                    Some(name) => {
                        check_name(name)?;
                        let dir = components(&dir)?;
                        let vars = directory(&tree, dir)?;
                        match vars.iter().find(|v| &v.name == name) {
                            Some(v) if v.variables.is_some() => {
                                return Err(format!(
                                    "{name} in {{ {} }} is a directory: choose another name",
                                    cd_command(dir)
                                )
                                .into());
                            }
                            Some(_) if !replace => {
                                return Err(format!(
                                    "{name} already exists in {{ {} }}",
                                    cd_command(dir)
                                )
                                .into());
                            }
                            _ => {}
                        }
                        back = Transfer::enter(&mut steps, dir, &here);
                        vars
                    }
                    None => directory(&tree, &here)?,
                };
                let temp = free_name(vars, SCREEN_VARIABLE);
                Transfer::with_mode(
                    &mut steps,
                    &flags,
                    true,
                    Step::Send {
                        name: temp.clone(),
                        data: file,
                    },
                );
                // Recalled and purged first: whatever fails after, the
                // temporary name is gone.
                let t = quoted(&temp);
                steps.push_back(match &name {
                    Some(name) => Step::Host {
                        text: format!("{t} RCL {t} PURGE {} STO", quoted(name)),
                        cleanup: false,
                    },
                    None => Step::Push {
                        text: format!("{t} RCL {t} PURGE"),
                    },
                });
                result.name = name;
            }
            Op::SetFlag { .. } | Op::Server { .. } | Op::SetClock { .. } => {}
        }
        if let Some(text) = back {
            steps.push_back(Step::Host {
                text,
                cleanup: true,
            });
        }
        let mut t = Transfer::serve(m, &flags, steps, result)?;
        if alg {
            let depth = u.stack_addresses().map_err(|e| format!("{e:#}"))?.len();
            t.in_rpn(depth);
        }
        t.temp_variable = temp_variable;
        t.leftovers.replace = replaced;
        t.leftovers.backup = backup_name;
        Ok(t)
    }

    /// Change to `dir` unless it is current; the command that changes
    /// back, if one is needed and the current path can be typed.
    pub(super) fn enter(
        steps: &mut VecDeque<Step>,
        dir: &[String],
        here: &[String],
    ) -> Option<String> {
        if dir == here {
            return None;
        }
        steps.push_back(Step::Host {
            text: cd_command(dir),
            cleanup: false,
        });
        here.iter()
            .all(|n| is_plain_name(n))
            .then(|| cd_command(here))
    }

    /// `step` with flag -35 set to `binary`, and put back afterwards.
    pub(super) fn with_mode(steps: &mut VecDeque<Step>, flags: &Flags, binary: bool, step: Step) {
        let was = flags.get(-35) == Some(true);
        let set = |on: bool| if on { "-35 SF" } else { "-35 CF" };
        if was != binary {
            steps.push_back(Step::Host {
                text: set(binary).into(),
                cleanup: false,
            });
        }
        steps.push_back(step);
        if was != binary {
            steps.push_back(Step::Host {
                text: set(was).into(),
                cleanup: true,
            });
        }
    }

    /// `steps` inside a server session: `SERVER`, its first NAK, the
    /// baseline, the steps, `G F` and the wait for the stack.
    pub(super) fn serve(
        m: &Machine,
        flags: &Flags,
        steps: VecDeque<Step>,
        result: TransferResult,
    ) -> crate::Result<Transfer> {
        if m.model() != Model::Hp49g && flags.get(-33) == Some(true) {
            return Err(
                "flag -33 is set (I/O over infrared): clear it for transfers over the wire"
                    .to_string()
                    .into(),
            );
        }
        let mut all = VecDeque::from([
            Step::Type("SERVER".into()),
            Step::WaitNak,
            Step::Host {
                text: String::new(),
                cleanup: false,
            },
        ]);
        all.extend(steps);
        all.push_back(Step::Finish);
        all.push_back(Step::Settle);
        Ok(Transfer::planned(m, all, result))
    }

    /// The 49G in algebraic mode, with `depth` stack levels: RPN for the
    /// server (`CF(-95)` typed before it), algebraic again after it (`-95
    /// SF` typed once it has ended, also after an error or a dead server,
    /// and on a stop); the echo of each typed line dropped.
    pub(super) fn in_rpn(&mut self, depth: usize) {
        self.steps.push_front(Step::DropTo(depth));
        self.steps.push_front(Step::Type(TO_RPN.into()));
        self.steps.push_back(Step::Algebraic);
        self.steps.push_back(Step::DropTo(depth));
        self.algebraic = Some(depth);
    }

    /// A write with nothing planned, for the tests of what holds one.
    #[cfg(test)]
    pub(crate) fn empty(m: &Machine) -> Transfer {
        Self::planned(m, VecDeque::new(), TransferResult::default())
    }

    pub(super) fn planned(m: &Machine, steps: VecDeque<Step>, result: TransferResult) -> Transfer {
        Transfer {
            steps,
            running: None,
            wire: Wire::new(),
            server: false,
            entering: false,
            baseline: None,
            error: None,
            result,
            fetched: None,
            temp_variable: false,
            pushed: 0,
            started: m.cycles(),
            done: false,
            algebraic: None,
            restoring: false,
            restore_error: None,
            stopped: false,
            lines_run: 0,
            probe_finish: false,
            probes: 0,
            asking: None,
            leftovers: Leftovers::default(),
        }
    }
}
