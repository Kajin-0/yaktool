use crate::{
    error::{Error, Result},
    execute,
    filesystem::Root,
    interpret::{Interpreter, RuleInterpreter},
    journal::Journal,
    plan::Plan,
    render, resolve,
};
use clap::{Parser, Subcommand};
use std::{
    io::{self, Write},
    path::{Path, PathBuf},
};

#[derive(Parser)]
#[command(
    version,
    about = "YakTool — Tell your computer what to do.",
    after_help = "Examples:\n  yaktool \"show Downloads\"\n  yaktool \"find PDFs modified this week in Documents\"\n  yaktool \"find files larger than 500 MB in Downloads\"\n  yaktool \"move PNG files older than 30 days from Downloads to Archive\"\n\nMoves require confirmation. Non-recursive. No shell, overwrite, delete, or AI model."
)]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    #[arg(value_name = "REQUEST")]
    request: Option<String>,
}
#[derive(Subcommand)]
enum Command {
    Undo,
    History,
    ShowPlan { id: String },
    Doctor,
}
fn journal_path(root: &Root) -> Result<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| root.path().join(".local/share"));
    root.relative(&base)?;
    Ok(base.join("yaktool/yaktool.db"))
}
fn initialize_data(root: &Root, path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("DATABASE_ERROR", "Invalid journal path"))?;
    let relative = root.relative(parent)?;
    let mut current = PathBuf::from(".");
    for component in relative.components() {
        if let std::path::Component::Normal(name) = component {
            let fd = root.directory(&current)?;
            match rustix::fs::mkdirat(&fd, name, rustix::fs::Mode::from_raw_mode(0o700)) {
                Ok(()) | Err(rustix::io::Errno::EXIST) => (),
                Err(e) => return Err(e.into()),
            }
            current.push(name);
            root.directory(&current)?;
        }
    }
    Ok(())
}
fn mutate(root: &Root, plan: Plan, journal: &mut Journal) -> Result<()> {
    journal.store(&plan)?;
    print!("{}", render::preview(&plan, false));
    if plan.data().operations.is_empty() {
        println!("No matching files. No mutation.");
        return Ok(());
    }
    print!("Proceed? [y/N] ");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    let Some(confirmed) = plan.confirm(&answer) else {
        println!("Cancelled. No files moved.");
        return Ok(());
    };
    let result = execute::execute(root, &confirmed, journal)?;
    print!(
        "{}",
        render::result(&result, &confirmed.plan().data().plan_id)
    );
    println!(
        "Undo available for verified forward moves: {}",
        if confirmed.plan().data().undo_of.is_none()
            && result
                .operations
                .iter()
                .any(|o| o.state == execute::OperationState::Verified)
        {
            "Yes"
        } else {
            "No"
        }
    );
    if result.status != "verified" {
        return Err(Error::new(
            "EXECUTION_FAILED",
            "Execution stopped; inspect transaction history",
        ));
    }
    Ok(())
}
pub fn run(cli: Cli) -> Result<()> {
    let root = Root::production()?;
    let path = journal_path(&root)?;
    match cli.command {
        Some(Command::Doctor) => {
            println!("YakTool doctor\nVersion: {}\nPlatform: Linux\nEffective UID: {}\nApproved root: {}\nData directory: {}\nJournal path: {}\nSQLite: {}\nopenat2 beneath/no-symlink/no-mount lookup: supported\nNo-replace rename: Linux primitive required; filesystem support checked on use\nStatus: read-only diagnostics complete", env!("CARGO_PKG_VERSION"), rustix::process::geteuid().as_raw(), render::escaped(root.path()), render::escaped(path.parent().ok_or_else(|| Error::new("DATABASE_ERROR", "Missing data directory"))?), render::escaped(&path), Journal::check_readonly(&path)?);
        }
        Some(command) => {
            let mut journal = if matches!(command, Command::Undo) {
                initialize_data(&root, &path)?;
                Journal::open(&path)?
            } else {
                if root.absent(&path)? {
                    println!("No journal history yet.");
                    return Ok(());
                }
                Journal::read_only(&path)?
            };
            match command {
                Command::History => {
                    for r in journal.recent()? {
                        println!(
                            "{}  {}  {}  {}  {}/{} verified",
                            r.plan.data().plan_id,
                            r.plan.data().created_at,
                            r.plan.data().action,
                            r.status,
                            r.result.as_ref().map_or(0, |v| v
                                .operations
                                .iter()
                                .filter(|o| o.state == execute::OperationState::Verified)
                                .count()),
                            r.plan.data().operations.len()
                        );
                    }
                }
                Command::ShowPlan { id } => {
                    let r = journal.get(&id)?;
                    print!("{}", render::preview(&r.plan, true));
                    println!(
                        "Stored status: {}\nCanonical JSON:\n{}",
                        r.status,
                        r.plan.json()?
                    );
                    if let Some(result) = r.result {
                        print!("{}", render::result(&result, &r.plan.data().plan_id));
                    }
                }
                Command::Undo => {
                    let plan = execute::prepare_undo(&root, &journal)?;
                    mutate(&root, plan, &mut journal)?;
                }
                Command::Doctor => (),
            }
        }
        None => {
            let input = cli.request.ok_or_else(|| {
                Error::new(
                    "UNSUPPORTED_REQUEST",
                    "Supply a quoted request or use --help",
                )
            })?;
            let intent = RuleInterpreter.interpret(&input)?;
            let resolved = resolve::resolve(&root, &intent, chrono::Local::now())?;
            if let Some(plan) = resolved.plan {
                initialize_data(&root, &path)?;
                let mut journal = Journal::open(&path)?;
                mutate(&root, plan, &mut journal)?;
            } else {
                for (p, s) in &resolved.entries {
                    println!("{}", render::entry(p, s));
                }
                println!("{} matching entries", resolved.entries.len());
            }
        }
    }
    Ok(())
}
