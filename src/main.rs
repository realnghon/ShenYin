use foldbox::app::{self, Action, CliCommand};
use std::env;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    if args.is_empty()
        || args
            .iter()
            .any(|argument| matches!(argument.as_str(), "--help" | "-h"))
    {
        write_stdout(&app::help_text("foldbox"))?;
        return Ok(());
    }

    let command = app::parse_cli(args).map_err(|error| error.to_string())?;
    execute(command).map(|output_path| {
        let _ = write_stdout_line(&format!("created: {}", output_path.display()));
    })
}

fn execute(command: CliCommand) -> Result<PathBuf, String> {
    match command.action {
        Action::Pack => {
            let passphrase = prompt_hidden("Enter passphrase: ")?;
            let confirm = prompt_hidden("Confirm passphrase: ")?;
            app::pack_file(
                &command.input_path,
                command.output_path.as_deref(),
                &passphrase,
                &confirm,
            )
            .map_err(|error| error.to_string())
        }
        Action::Unpack => {
            let passphrase = prompt_hidden("Enter passphrase: ")?;
            app::unpack_file(
                &command.input_path,
                command.output_path.as_deref(),
                &passphrase,
            )
            .map_err(|error| error.to_string())
        }
    }
}

fn prompt_hidden(prompt: &str) -> Result<String, String> {
    let mut stderr = io::stderr().lock();
    write!(stderr, "{prompt}").map_err(|error| error.to_string())?;
    stderr.flush().map_err(|error| error.to_string())?;
    rpassword::read_password().map_err(|error| error.to_string())
}

fn write_stdout(text: &str) -> Result<(), String> {
    let mut stdout = io::stdout().lock();
    write!(stdout, "{text}").map_err(|error| error.to_string())?;
    stdout.flush().map_err(|error| error.to_string())
}

fn write_stdout_line(text: &str) -> Result<(), String> {
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "{text}").map_err(|error| error.to_string())?;
    stdout.flush().map_err(|error| error.to_string())
}
