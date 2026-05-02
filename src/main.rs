use std::process::ExitCode;

fn main() -> ExitCode {
    match sqlt::cli::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(exit_code_for(&e))
        }
    }
}

fn exit_code_for(err: &sqlt::error::Error) -> u8 {
    use sqlt::error::Error;
    match err {
        Error::Parse(_) | Error::Encoding(_) | Error::LintFindings => 1,
        Error::UnknownDialect(_) | Error::UnknownEncoding(_) | Error::UnknownRule(_) => 2,
        Error::StrictWarnings => 3,
        _ => 1,
    }
}
