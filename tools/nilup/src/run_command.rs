use eyre::eyre;
use std::ffi::OsStr;

/// run a command with the given arguments
pub async fn run_command<I, S, C>(command: C, args: I) -> eyre::Result<()>
where
    C: AsRef<OsStr>,
    I: IntoIterator<Item = S> + Clone,
    S: AsRef<OsStr>,
{
    let command = command.as_ref();
    let args_str =
        args.clone().into_iter().map(|s| s.as_ref().to_string_lossy().to_string()).collect::<Vec<String>>().join(" ");

    let full_command_str = format!("{} {args_str}", command.to_string_lossy());

    let command_out = tokio::process::Command::new(command).args(args).output().await?;
    if !command_out.status.success() {
        return Err(eyre!(
            "Failed to run command {full_command_str}: \nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&command_out.stdout),
            String::from_utf8_lossy(&command_out.stderr)
        ));
    }
    Ok(())
}
