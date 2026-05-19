use std::process::ExitCode;

fn main() -> ExitCode {
    reset_sigpipe();
    match sqlt::cli::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(exit_code_for(&e))
        }
    }
}

#[cfg(unix)]
fn reset_sigpipe() {
    // Restore default SIGPIPE behaviour so piping into a pager (`sqlt man |
    // less`) that exits early ends the process quietly instead of panicking
    // on the next `println!`. Safe: we're installing a default signal
    // disposition before any threads exist.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

#[cfg(not(unix))]
fn reset_sigpipe() {}

fn exit_code_for(err: &sqlt::error::Error) -> u8 {
    use sqlt::error::Error;
    match err {
        Error::Parse(_) | Error::Encoding(_) | Error::LintFindings => 1,
        Error::UnknownDialect(_) | Error::UnknownEncoding(_) | Error::UnknownRule(_) => 2,
        Error::StrictWarnings => 3,
        _ => 1,
    }
}
