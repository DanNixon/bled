//! Interactive line editing for the shell.
//!
//! Wraps [`rustyline`] so the shell gets history, line editing and tab
//! completion. Rustyline is blocking, so each read happens on a blocking task
//! and the editor is handed back and forth across the await point.

use crate::cli::{
    BUFFER_SUBCOMMAND_NAMES, COMMAND_NAMES, CONFIG_SUBCOMMAND_NAMES, DEVICE_SUBCOMMAND_NAMES,
    FILE_SUBCOMMAND_NAMES,
};
use anyhow::{Context as _, Result};
use rustyline::{
    Cmd, CompletionType, Config, Context, Editor, KeyEvent, Movement,
    completion::{Completer, Pair},
    error::ReadlineError,
    history::DefaultHistory,
};
use rustyline::{Helper, Highlighter, Hinter, Validator};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

const PROMPT: &str = "bled> ";

/// Maximum number of lines kept in the on-disk history file.
const HISTORY_LIMIT: usize = 1000;

/// Completes command names, and device names for the `connect` command.
#[derive(Helper, Highlighter, Hinter, Validator)]
pub struct BledHelper {
    /// Tokens offered when completing a `connect` argument.
    devices: Vec<String>,
}

impl Completer for BledHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let (start, word) = word_at(line, pos);
        let preceding = &line[..start];
        let mut words = preceding.split_whitespace();

        let first = words.next();
        let second = words.next();
        let third = words.next();

        let candidates: Vec<&str> = match (first, second, third) {
            // Completing the first word: offer every command.
            (None, _, _) => COMMAND_NAMES.to_vec(),
            // Completing the sole argument of `connect`: offer known devices.
            (Some("connect"), None, _) => self.devices.iter().map(String::as_str).collect(),
            // Completing the action argument of `device`: offer device subcommands.
            (Some("device"), None, _) => DEVICE_SUBCOMMAND_NAMES.to_vec(),
            // Completing the action argument of `device config`: offer config subcommands.
            (Some("device"), Some("config"), None) => CONFIG_SUBCOMMAND_NAMES.to_vec(),
            // Completing the action argument of `buffer`: offer buffer subcommands.
            (Some("buffer"), None, _) => BUFFER_SUBCOMMAND_NAMES.to_vec(),
            // Completing the action argument of `file`: offer file subcommands.
            (Some("file"), None, _) => FILE_SUBCOMMAND_NAMES.to_vec(),
            _ => Vec::new(),
        };

        let word = word.to_lowercase();
        let matches = candidates
            .into_iter()
            .filter(|candidate| candidate.to_lowercase().starts_with(&word))
            .map(|candidate| Pair {
                display: candidate.to_owned(),
                replacement: candidate.to_owned(),
            })
            .collect();

        Ok((start, matches))
    }
}

/// Returns the byte offset and text of the word the cursor sits in.
fn word_at(line: &str, pos: usize) -> (usize, &str) {
    let head = &line[..pos];
    let start = head
        .char_indices()
        .rev()
        .find(|(_, character)| character.is_whitespace())
        .map_or(0, |(index, character)| index + character.len_utf8());
    (start, &line[start..pos])
}

/// A line editor for the shell.
pub struct Prompt {
    /// Only `None` while a read is in flight on a blocking task.
    editor: Option<Editor<BledHelper, DefaultHistory>>,
    history_path: Option<PathBuf>,
}

impl Prompt {
    /// Builds an editor and loads any previously saved history.
    pub fn new() -> Result<Self> {
        let config = Config::builder()
            .auto_add_history(true)
            .history_ignore_dups(true)
            .context("configuring line editor")?
            .history_ignore_space(true)
            .max_history_size(HISTORY_LIMIT)
            .context("configuring line editor")?
            .completion_type(CompletionType::List)
            .build();

        let mut editor: Editor<BledHelper, DefaultHistory> =
            Editor::with_config(config).context("initialising line editor")?;
        editor.set_helper(Some(BledHelper {
            devices: Vec::new(),
        }));
        // Ctrl-W stops at word boundaries rather than whitespace, which suits
        // arguments like `aa:bb:cc:dd:ee:ff`.
        editor.bind_sequence(
            KeyEvent::ctrl('W'),
            Cmd::Kill(Movement::BackwardWord(1, rustyline::Word::Big)),
        );

        let history_path = history_path();
        if let Some(path) = &history_path
            && path.exists()
            && let Err(error) = editor.load_history(path)
        {
            log::debug!("could not load history from {}: {error}", path.display());
        }

        Ok(Self {
            editor: Some(editor),
            history_path,
        })
    }

    /// Replaces the set of devices offered when completing `connect`.
    pub fn set_devices(&mut self, devices: Vec<String>) {
        if let Some(helper) = self.editor.as_mut().and_then(Editor::helper_mut) {
            helper.devices = devices;
        }
    }

    /// Reads one line, returning `Ok(None)` at end of input.
    ///
    /// Ctrl-C abandons the current line and re-prompts; Ctrl-D on an empty line
    /// ends input.
    pub async fn read_line(&mut self) -> Result<Option<String>> {
        loop {
            let mut editor = self
                .editor
                .take()
                .expect("line editor is missing; a previous read must have failed");

            // rustyline blocks, so read on a blocking task and take the editor
            // back afterwards.
            let (result, editor) = tokio::task::spawn_blocking(move || {
                let result = editor.readline(PROMPT);
                (result, editor)
            })
            .await
            .context("line editor task panicked")?;
            self.editor = Some(editor);

            return match result {
                Ok(line) => Ok(Some(line)),
                Err(ReadlineError::Interrupted) => continue,
                Err(ReadlineError::Eof) => Ok(None),
                Err(error) => Err(error).context("reading shell input"),
            };
        }
    }

    /// Writes history to disk, logging rather than failing on error.
    pub fn save_history(&mut self) {
        let (Some(editor), Some(path)) = (self.editor.as_mut(), self.history_path.as_ref()) else {
            return;
        };
        if let Some(parent) = path.parent()
            && let Err(error) = fs::create_dir_all(parent)
        {
            log::debug!("could not create {}: {error}", parent.display());
            return;
        }
        if let Err(error) = editor.save_history(path) {
            log::debug!("could not save history to {}: {error}", path.display());
        }
    }
}

/// Location of the history file, following the XDG base directory spec.
fn history_path() -> Option<PathBuf> {
    let non_empty = |value: std::ffi::OsString| (!value.is_empty()).then_some(value);

    if let Some(state) = env::var_os("XDG_STATE_HOME").and_then(non_empty) {
        return Some(Path::new(&state).join("bled").join("history"));
    }
    let home = env::var_os("HOME").and_then(non_empty)?;
    Some(
        Path::new(&home)
            .join(".local")
            .join("state")
            .join("bled")
            .join("history"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustyline::history::DefaultHistory;

    fn complete(helper: &BledHelper, line: &str) -> (usize, Vec<String>) {
        let history = DefaultHistory::new();
        let context = Context::new(&history);
        let (start, pairs) = helper.complete(line, line.len(), &context).unwrap();
        (
            start,
            pairs.into_iter().map(|pair| pair.replacement).collect(),
        )
    }

    fn helper() -> BledHelper {
        BledHelper {
            devices: vec![
                "bled-kitchen".to_owned(),
                "bled-hall".to_owned(),
                "AA:BB:CC:DD:EE:FF".to_owned(),
            ],
        }
    }

    #[test]
    fn first_word_completes_commands() {
        let (start, matches) = complete(&helper(), "co");
        assert_eq!(start, 0);
        assert_eq!(matches, ["connect", "commit"]);
    }

    #[test]
    fn empty_line_offers_every_command() {
        let (start, matches) = complete(&helper(), "");
        assert_eq!(start, 0);
        assert_eq!(matches.len(), COMMAND_NAMES.len());
    }

    #[test]
    fn connect_argument_completes_devices() {
        let (start, matches) = complete(&helper(), "connect bled-");
        assert_eq!(start, "connect ".len());
        assert_eq!(matches, ["bled-kitchen", "bled-hall"]);
    }

    #[test]
    fn device_completion_is_case_insensitive() {
        let (_, matches) = complete(&helper(), "connect aa:bb");
        assert_eq!(matches, ["AA:BB:CC:DD:EE:FF"]);
    }

    #[test]
    fn buffer_argument_completes_subcommands() {
        let (start, matches) = complete(&helper(), "buffer ");
        assert_eq!(start, "buffer ".len());
        assert_eq!(matches, ["info", "read", "write"]);
    }

    #[test]
    fn device_argument_completes_subcommands() {
        let (start, matches) = complete(&helper(), "device ");
        assert_eq!(start, "device ".len());
        assert_eq!(matches, ["info", "reset", "config"]);
    }

    #[test]
    fn device_config_argument_completes_subcommands() {
        let (start, matches) = complete(&helper(), "device config ");
        assert_eq!(start, "device config ".len());
        assert_eq!(matches, ["stat", "read"]);
    }

    #[test]
    fn file_argument_completes_subcommands() {
        let (start, matches) = complete(&helper(), "file ");
        assert_eq!(start, "file ".len());
        assert_eq!(matches, ["read", "write", "delete", "stat"]);
    }

    #[test]
    fn file_subcommand_completes_prefix() {
        let (start, matches) = complete(&helper(), "file wr");
        assert_eq!(start, "file ".len());
        assert_eq!(matches, ["write"]);
    }

    #[test]
    fn other_arguments_have_no_completions() {
        assert!(complete(&helper(), "range 0 ").1.is_empty());
        assert!(complete(&helper(), "connect bled-hall ").1.is_empty());
        assert!(complete(&helper(), "scan --scan-").1.is_empty());
    }

    #[test]
    fn word_boundaries_handle_multibyte_whitespace() {
        // U+3000 IDEOGRAPHIC SPACE is three bytes; the word must start after it.
        let line = "connect\u{3000}bl";
        let (start, word) = word_at(line, line.len());
        assert_eq!(start, "connect\u{3000}".len());
        assert_eq!(word, "bl");
    }
}
