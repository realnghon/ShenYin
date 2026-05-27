use foldbox::app::{self, Action, CliCommand};
use std::env;
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
        print!("{}", app::help_text("foldbox"));
        return Ok(());
    }

    let command = app::parse_cli(args).map_err(|error| error.to_string())?;
    execute(command).map(|output_path| {
        println!("created: {}", output_path.display());
    })
}

fn execute(command: CliCommand) -> Result<PathBuf, String> {
    match command.action {
        Action::Pack => {
            let passphrase =
                rpassword::prompt_password("Enter passphrase: ").map_err(|error| error.to_string())?;
            let confirm = rpassword::prompt_password("Confirm passphrase: ")
                .map_err(|error| error.to_string())?;
            app::pack_file(
                &command.input_path,
                command.output_path.as_deref(),
                &passphrase,
                &confirm,
            )
            .map_err(|error| error.to_string())
        }
        Action::Unpack => {
            let passphrase =
                rpassword::prompt_password("Enter passphrase: ").map_err(|error| error.to_string())?;
            app::unpack_file(
                &command.input_path,
                command.output_path.as_deref(),
                &passphrase,
            )
            .map_err(|error| error.to_string())
        }
    }
}
