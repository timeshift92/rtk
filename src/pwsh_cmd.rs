use crate::tracking;
use crate::utils::resolved_command;
use anyhow::{Context, Result};

pub fn run(args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let mut pwsh_args = if args.is_empty() {
        vec!["-NoLogo".to_string(), "-NoProfile".to_string()]
    } else {
        args.to_vec()
    };

    if !pwsh_args
        .iter()
        .any(|arg| arg.eq_ignore_ascii_case("-NoLogo"))
    {
        pwsh_args.insert(0, "-NoLogo".to_string());
    }
    if !pwsh_args
        .iter()
        .any(|arg| arg.eq_ignore_ascii_case("-NoProfile"))
    {
        pwsh_args.insert(1.min(pwsh_args.len()), "-NoProfile".to_string());
    }

    if verbose > 0 {
        eprintln!("Running: pwsh {}", pwsh_args.join(" "));
    }

    let mut cmd = resolved_command("pwsh");
    cmd.args(&pwsh_args);

    let output = cmd.output().context("Failed to run PowerShell (pwsh)")?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let exit_code = output.status.code().unwrap_or(1);

    if !stderr.trim().is_empty() {
        eprint!("{}", stderr);
    }

    if !stdout.is_empty() {
        print!("{}", stdout);
    }

    if exit_code != 0 {
        std::process::exit(exit_code);
    }

    timer.track(
        &format!("pwsh {}", pwsh_args.join(" ")),
        &format!("rtk pwsh {}", pwsh_args.join(" ")),
        &stdout,
        &stdout,
    );

    Ok(())
}
