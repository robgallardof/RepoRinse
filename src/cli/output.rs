//! Terminal output honouring `--json`, `--quiet`, colors and verbosity.

use std::io::Write;

use serde::Serialize;

pub struct Output<'a> {
    out: &'a mut dyn Write,
    err: &'a mut dyn Write,
    pub json: bool,
    pub quiet: bool,
    pub color: bool,
    pub verbose: bool,
}

impl<'a> Output<'a> {
    pub fn new(
        out: &'a mut dyn Write,
        err: &'a mut dyn Write,
        json: bool,
        quiet: bool,
        color: bool,
        verbose: bool,
    ) -> Output<'a> {
        Output {
            out,
            err,
            json,
            quiet,
            color: color && !json,
            verbose,
        }
    }

    /// Informational line on stdout; suppressed by `--quiet` and `--json`.
    pub fn line(&mut self, text: impl AsRef<str>) {
        if !self.quiet && !self.json {
            let _ = writeln!(self.out, "{}", text.as_ref());
        }
    }

    /// Only with `--verbose` (and not `--json`).
    pub fn detail(&mut self, text: impl AsRef<str>) {
        if self.verbose && !self.json {
            let _ = writeln!(self.out, "{}", text.as_ref());
        }
    }

    /// Line that must survive `--quiet` (blocking findings). Still suppressed by `--json`.
    pub fn important(&mut self, text: impl AsRef<str>) {
        if !self.json {
            let _ = writeln!(self.out, "{}", text.as_ref());
        }
    }

    /// Raw stdout, used for JSON documents and hook protocol payloads.
    pub fn raw(&mut self, text: impl AsRef<str>) {
        let _ = write!(self.out, "{}", text.as_ref());
    }

    pub fn print_json<T: Serialize + ?Sized>(&mut self, value: &T) {
        let text = serde_json::to_string_pretty(value).expect("serializable output");
        let _ = writeln!(self.out, "{text}");
    }

    /// Always printed to stderr.
    pub fn error(&mut self, text: impl AsRef<str>) {
        let prefix = self.paint("error:", "31");
        let _ = writeln!(self.err, "{prefix} {}", text.as_ref());
    }

    pub fn warn(&mut self, text: impl AsRef<str>) {
        if !self.quiet {
            let prefix = self.paint("warning:", "33");
            let _ = writeln!(self.err, "{prefix} {}", text.as_ref());
        }
    }

    /// Raw stderr, used by hook protocols.
    pub fn raw_err(&mut self, text: impl AsRef<str>) {
        let _ = write!(self.err, "{}", text.as_ref());
    }

    pub fn bold(&self, s: &str) -> String {
        self.paint(s, "1")
    }

    pub fn dim(&self, s: &str) -> String {
        self.paint(s, "2")
    }

    pub fn green(&self, s: &str) -> String {
        self.paint(s, "32")
    }

    pub fn yellow(&self, s: &str) -> String {
        self.paint(s, "33")
    }

    pub fn red(&self, s: &str) -> String {
        self.paint(s, "31")
    }

    fn paint(&self, s: &str, code: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }
}
