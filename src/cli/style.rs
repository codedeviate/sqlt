//! Shared ANSI colorization for `--examples` and the `man` subcommand.
//!
//! The text constants in `examples.rs` and `man.rs` are plain text — this
//! module post-processes them line-by-line into a recon-style colored
//! output when stdout is a TTY. The rules are deliberately heuristic:
//!
//!   * The first non-blank line of the text is the **title** (bold).
//!   * Lines made entirely of `─` are dividers (dimmed).
//!   * The non-blank line directly above or below a divider is a
//!     **section header** (yellow + bold).
//!   * Lines whose trimmed content starts with `# ` and that are indented
//!     by at least two spaces are shell comments embedded in an example
//!     block (green).
//!   * Lines whose trimmed content starts with `note:` are notes
//!     (bright-black / dimmed).
//!   * Lines starting (after at least two leading spaces) with a known
//!     command keyword — `sqlt`, `echo`, `printf`, `brew`, `cargo`,
//!     `jq`, `cp`, `mv`, `git` — are commands (cyan).
//!   * After a command line, subsequent deeply-indented (≥ 4 spaces) lines
//!     are treated as backslash-continuations and stay cyan, until we hit
//!     a blank line.
//!   * Column-0 short lines ending with `:` (`Examples:`, `Exit codes:`,
//!     `Dialect aliases:`, …) are sub-headings (bold).
//!
//! Anything else is rendered uncoloured.

use colored::Colorize;

pub fn print_colored(text: &str) {
    let lines: Vec<&str> = text.lines().collect();
    let mut in_command_block = false;
    let mut printed_title = false;

    for (i, line) in lines.iter().enumerate() {
        if line.trim().is_empty() {
            in_command_block = false;
            println!();
            continue;
        }

        let prev_is_divider = i > 0 && is_divider(lines[i - 1]);
        let next_is_divider = i + 1 < lines.len() && is_divider(lines[i + 1]);

        let (rendered, sets_command) = classify(
            line,
            prev_is_divider,
            next_is_divider,
            !printed_title,
            in_command_block,
        );
        printed_title = true;
        in_command_block = sets_command;

        println!("{rendered}");
    }
}

fn is_divider(line: &str) -> bool {
    let t = line.trim();
    !t.is_empty() && t.chars().all(|c| c == '─')
}

fn leading_spaces(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ').count()
}

fn starts_with_command_keyword(trimmed: &str) -> bool {
    const KEYWORDS: &[&str] = &[
        "sqlt ", "sqlt\n", "echo ", "printf ", "brew ", "cargo ", "jq ", "cp ", "mv ", "git ",
    ];
    if trimmed == "sqlt" {
        return true;
    }
    KEYWORDS.iter().any(|kw| trimmed.starts_with(kw))
}

/// Returns `(rendered_line, sets_in_command_block)`.
fn classify(
    line: &str,
    prev_is_divider: bool,
    next_is_divider: bool,
    is_first_visible_line: bool,
    in_command_block: bool,
) -> (String, bool) {
    let trimmed = line.trim_start();
    let leading = leading_spaces(line);

    if is_first_visible_line {
        return (line.bold().to_string(), false);
    }

    if is_divider(line) {
        return (line.bright_black().to_string(), false);
    }

    if (prev_is_divider || next_is_divider) && !trimmed.is_empty() {
        return (line.yellow().bold().to_string(), false);
    }

    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("note:") {
        return (line.bright_black().to_string(), false);
    }

    if leading >= 2 && trimmed.starts_with('#') {
        return (line.green().to_string(), false);
    }

    if leading >= 2 && starts_with_command_keyword(trimmed) {
        return (line.cyan().to_string(), true);
    }

    // Continuation lines inside a command block: any non-empty,
    // ≥4-space-indented line that comes right after a command (or another
    // continuation) is part of the same shell invocation. The block resets
    // on a blank line, so prose paragraphs that happen to be indented are
    // unaffected.
    if in_command_block && leading >= 4 {
        return (line.cyan().to_string(), true);
    }

    if leading == 0
        && trimmed.ends_with(':')
        && trimmed.split_whitespace().count() <= 5
        && trimmed
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic())
    {
        return (line.bold().to_string(), false);
    }

    (line.to_string(), false)
}
