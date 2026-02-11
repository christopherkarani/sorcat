use std::process::ExitCode;

fn main() -> ExitCode {
    match sorcat_cli::run_from(std::env::args_os()) {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
