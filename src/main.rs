use clap::Parser;
fn main() -> std::process::ExitCode {
    match yaktool::cli::run(yaktool::cli::Cli::parse()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            std::process::ExitCode::FAILURE
        }
    }
}
