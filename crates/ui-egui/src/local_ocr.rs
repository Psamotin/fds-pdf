//! Desktop-only external OCR adapter. It never edits the source document or engine history.
//! All runtime paths are resolved from the running executable; no shell or global environment.
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub const PYTHON_VERSION: &str = "3.13.15";
pub const OCRMY_PDF_VERSION: &str = "17.4.0";
pub const TESSERACT_VERSION: &str = "5.5.3";
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureKind {
    Runtime,
    Launch,
    Recognition,
    Cancelled,
    OutputExists,
    Io,
    InvalidPdf,
}

#[derive(Clone, Debug)]
pub struct Failure {
    pub kind: FailureKind,
    pub details: String,
}
impl Failure {
    pub fn new(kind: FailureKind, details: impl ToString) -> Self {
        Self { kind, details: details.to_string() }
    }
    pub fn message(&self) -> &str {
        crate::i18n::text(match self.kind {
            FailureKind::Runtime => "ui.ocr_local_runtime_missing",
            FailureKind::Launch => "ui.ocr_local_launch_failed",
            FailureKind::Recognition => "ui.ocr_local_failed",
            FailureKind::Cancelled => "ui.ocr_local_cancelled",
            FailureKind::OutputExists => "ui.ocr_local_output_exists",
            FailureKind::Io => "ui.ocr_local_io_failed",
            FailureKind::InvalidPdf => "ui.ocr_local_invalid_pdf",
        })
    }
}
fn io_failure(error: impl ToString) -> Failure {
    Failure::new(FailureKind::Io, error)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Runtime {
    pub root: PathBuf,
    pub python: PathBuf,
    pub tesseract: PathBuf,
    pub tessdata: PathBuf,
}
impl Runtime {
    pub fn for_executable(executable: &Path) -> Result<Self, Failure> {
        if !executable.is_absolute() {
            return Err(Failure::new(FailureKind::Runtime, "Executable path must be absolute"));
        }
        let root = executable.parent().ok_or_else(|| Failure::new(FailureKind::Runtime, "Executable has no parent"))?.join("OCR");
        Ok(Self {
            python: root.join("Python/python.exe"),
            tesseract: root.join("Tesseract-OCR/tesseract.exe"),
            tessdata: root.join("Tesseract-OCR/tessdata"),
            root,
        })
    }
    pub fn check_files(&self) -> Result<(), Failure> {
        let mut required = vec![self.python.clone(), self.tesseract.clone(), self.root.join("Python/Lib/site-packages/sitecustomize.py")];
        required.extend(["rus", "eng", "osd"].map(|language| self.tessdata.join(format!("{language}.traineddata"))));
        for path in required {
            let metadata = std::fs::metadata(&path).map_err(|e| Failure::new(FailureKind::Runtime, format!("{}: {e}", path.display())))?;
            if !metadata.is_file() || metadata.len() == 0 {
                return Err(Failure::new(FailureKind::Runtime, format!("Missing/empty runtime file: {}", path.display())));
            }
        }
        Ok(())
    }
    pub fn environment(&self, inherited_path: Option<OsString>) -> Result<Vec<(OsString, OsString)>, Failure> {
        let mut paths = vec![self.root.join("Tesseract-OCR"), self.root.join("Python"), self.root.join("Python/Scripts")];
        if let Some(path) = inherited_path {
            paths.extend(std::env::split_paths(&path));
        }
        let path = std::env::join_paths(paths).map_err(io_failure)?;
        Ok(vec![
            ("PATH".into(), path),
            ("TESSDATA_PREFIX".into(), self.tessdata.as_os_str().to_owned()),
            ("PYTHONNOUSERSITE".into(), "1".into()),
            ("PYTHONUTF8".into(), "1".into()),
            ("FDS_OCR_HIDE_CHILDREN".into(), "1".into()),
        ])
    }
    fn spec(&self, program: PathBuf, arguments: Vec<OsString>) -> Result<CommandSpec, Failure> {
        Ok(CommandSpec {
            program,
            arguments,
            working_directory: Some(self.root.join("Tesseract-OCR")),
            environment: self.environment(std::env::var_os("PATH"))?,
            removed_environment: vec!["PYTHONPATH".into(), "PYTHONHOME".into()],
        })
    }
    pub fn command_spec(&self, input: &Path, output: &Path, options: Options) -> Result<CommandSpec, Failure> {
        if !input.is_absolute() || !output.is_absolute() || input == output {
            return Err(Failure::new(FailureKind::Io, "OCR requires distinct absolute input/output paths"));
        }
        let mut args: Vec<OsString> = ["-I", "-m", "ocrmypdf", "--language", "rus+eng", "--mode", "skip"].map(OsString::from).into();
        if options.rotate {
            args.push("--rotate-pages".into());
        }
        if options.deskew {
            args.push("--deskew".into());
        }
        args.extend(["--rasterizer", "pypdfium", "--output-type", "pdf", "--optimize", "0"].map(OsString::from));
        args.extend([input.as_os_str().to_owned(), output.as_os_str().to_owned()]);
        self.spec(self.python.clone(), args)
    }
    /// Run lightweight version/import/language probes in a worker, never on the UI thread.
    pub fn validate(&self) -> Result<(), Failure> {
        self.check_files()?;
        let cancelled = AtomicBool::new(false);
        let python = self.spec(
            self.python.clone(),
            vec![
                "-I".into(),
                "-c".into(),
                "import sys, ocrmypdf; assert sys.version_info[:3] == (3,13,15); assert ocrmypdf.__version__ == '17.4.0'; import _winapi; assert getattr(_winapi, '_fds_hidden_children', False)".into(),
            ],
        )?;
        execute(&python, &cancelled, Some(Duration::from_secs(30))).map_err(|e| Failure::new(FailureKind::Runtime, e.details))?;
        for argument in ["--version", "--list-langs"] {
            let tess = self.spec(self.tesseract.clone(), vec![argument.into()])?;
            let output = execute(&tess, &cancelled, Some(Duration::from_secs(30))).map_err(|e| Failure::new(FailureKind::Runtime, e.details))?;
            let valid = if argument == "--version" {
                output.contains(&format!("tesseract {TESSERACT_VERSION}"))
            } else {
                ["rus", "eng", "osd"].iter().all(|language| output.lines().any(|line| line.trim() == *language))
            };
            if !valid {
                return Err(Failure::new(FailureKind::Runtime, format!("Unexpected Tesseract runtime: {output}")));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub rotate: bool,
    pub deskew: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self { rotate: true, deskew: true }
    }
}

#[derive(Debug)]
pub struct CommandSpec {
    pub program: PathBuf,
    pub arguments: Vec<OsString>,
    pub working_directory: Option<PathBuf>,
    pub environment: Vec<(OsString, OsString)>,
    pub removed_environment: Vec<OsString>,
}
impl CommandSpec {
    pub fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        if let Some(directory) = &self.working_directory {
            command.current_dir(directory);
        }
        command.args(&self.arguments).envs(self.environment.iter().cloned());
        for key in &self.removed_environment {
            command.env_remove(key);
        }
        command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        command
    }
}

fn read_pipe<R: Read + Send + 'static>(mut pipe: R) -> Result<std::thread::JoinHandle<String>, Failure> {
    std::thread::Builder::new()
        .name("fds-ocr-diagnostics".into())
        .spawn(move || {
            let mut output = Vec::new();
            let mut buffer = [0; 4096];
            while let Ok(size) = pipe.read(&mut buffer) {
                if size == 0 {
                    break;
                }
                let keep = size.min((64 * 1024usize).saturating_sub(output.len()));
                output.extend_from_slice(&buffer[..keep]);
            }
            String::from_utf8_lossy(&output).into_owned()
        })
        .map_err(|error| Failure::new(FailureKind::Launch, error))
}

fn execute(spec: &CommandSpec, cancelled: &AtomicBool, timeout: Option<Duration>) -> Result<String, Failure> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(Failure::new(FailureKind::Cancelled, "Cancelled before launch"));
    }
    #[cfg(windows)]
    let tree = {
        let mut limits = win32job::ExtendedLimitInfo::new();
        limits.limit_kill_on_job_close();
        win32job::Job::create_with_limit_info(&limits).map_err(|e| Failure::new(FailureKind::Launch, e))?
    };
    let mut child = spec.command().spawn().map_err(|e| Failure::new(FailureKind::Launch, e))?;
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        if let Err(e) = tree.assign_process(child.as_raw_handle() as isize) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Failure::new(FailureKind::Launch, e));
        }
    }
    let readers = match (child.stdout.take(), child.stderr.take()) {
        (Some(stdout), Some(stderr)) => match (read_pipe(stdout), read_pipe(stderr)) {
            (Ok(stdout), Ok(stderr)) => Ok((stdout, stderr)),
            (Err(error), _) | (_, Err(error)) => Err(error),
        },
        _ => Err(Failure::new(FailureKind::Launch, "Missing diagnostic pipe")),
    };
    let (stdout, stderr) = match readers {
        Ok(readers) => readers,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    let started = Instant::now();
    let result = loop {
        if cancelled.load(Ordering::Relaxed) {
            break Err(Failure::new(FailureKind::Cancelled, "Cancelled"));
        }
        if timeout.is_some_and(|limit| started.elapsed() > limit) {
            break Err(Failure::new(FailureKind::Runtime, "Runtime probe timed out"));
        }
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break Ok(()),
            Ok(Some(status)) => break Err(Failure::new(FailureKind::Recognition, format!("OCR exit code: {:?}", status.code()))),
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(error) => break Err(Failure::new(FailureKind::Launch, error)),
        }
    };
    // Closing the job terminates the complete Windows process tree, including workers.
    #[cfg(windows)]
    drop(tree);
    if result.is_err() {
        let _ = child.kill();
    }
    let _ = child.wait();
    let output = stdout.join().unwrap_or_default();
    let details = stderr.join().unwrap_or_default();
    result.map(|()| format!("{output}\n{details}")).map_err(|mut error| {
        if !details.is_empty() {
            error.details.push('\n');
            error.details.push_str(&details);
        }
        error
    })
}

pub fn output_name(name: &str) -> String {
    let stem = Path::new(name).file_stem().unwrap_or_else(|| std::ffi::OsStr::new("document"));
    format!("{}_OCR.pdf", stem.to_string_lossy())
}
pub fn output_path(input: &Path) -> Result<PathBuf, Failure> {
    let absolute = if input.is_absolute() { input.to_path_buf() } else { std::env::current_dir().map_err(io_failure)?.join(input) };
    let parent = absolute.parent().ok_or_else(|| io_failure("Input has no folder"))?;
    let name = input.file_name().ok_or_else(|| io_failure("Input has no filename"))?.to_string_lossy();
    let base = output_name(&name);
    for number in 1..=10_000 {
        let name = if number == 1 { base.clone() } else { format!("{}_{}.pdf", base.trim_end_matches(".pdf"), number) };
        let path = parent.join(name);
        if !path.exists() {
            return Ok(path);
        }
    }
    Err(Failure::new(FailureKind::OutputExists, "No free OCR output filename"))
}

/// Publish a complete new file atomically. persist_noclobber handles races and never replaces a file.
fn publish(source: &Path, output: &Path) -> Result<(), Failure> {
    let parent = output.parent().ok_or_else(|| io_failure("Output has no folder"))?;
    let mut staged = tempfile::NamedTempFile::new_in(parent).map_err(io_failure)?;
    let mut input = std::fs::File::open(source).map_err(io_failure)?;
    std::io::copy(&mut input, staged.as_file_mut()).map_err(io_failure)?;
    staged.as_file_mut().flush().map_err(io_failure)?;
    staged.as_file().sync_all().map_err(io_failure)?;
    staged.persist_noclobber(output).map_err(|error| {
        Failure::new(
            if error.error.kind() == std::io::ErrorKind::AlreadyExists { FailureKind::OutputExists } else { FailureKind::Io },
            error.error.to_string(),
        )
    })?;
    Ok(())
}

pub fn recognize(runtime: &Runtime, bytes: &[u8], output: &Path, options: Options, cancelled: &AtomicBool) -> Result<PathBuf, Failure> {
    if output.exists() {
        return Err(Failure::new(FailureKind::OutputExists, output.display()));
    }
    let temporary = tempfile::Builder::new().prefix("FDS-PDF-OCR-").tempdir().map_err(io_failure)?;
    let input = temporary.path().join("input.pdf");
    let result = temporary.path().join("output.pdf");
    std::fs::write(&input, bytes).map_err(io_failure)?;
    execute(&runtime.command_spec(&input, &result, options)?, cancelled, None)?;
    if cancelled.load(Ordering::Relaxed) {
        return Err(Failure::new(FailureKind::Cancelled, "Cancelled before publishing"));
    }
    let mut header = [0; 5];
    std::fs::File::open(&result).and_then(|mut file| file.read_exact(&mut header)).map_err(io_failure)?;
    if &header != b"%PDF-" {
        return Err(Failure::new(FailureKind::InvalidPdf, "OCR output is not a PDF"));
    }
    publish(&result, output)?;
    Ok(output.to_path_buf())
}

pub struct Cancellation(pub Arc<AtomicBool>);
impl Default for Cancellation {
    fn default() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }
}
impl Drop for Cancellation {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn packaged_command_uses_relative_runtime_and_exact_profile_without_mutating_environment() {
        let folder = tempfile::tempdir().unwrap();
        let exe = folder.path().join("ФДС ПДФ/FDS-PDF.exe");
        let runtime = Runtime::for_executable(&exe).unwrap();
        assert_eq!(runtime.python, exe.parent().unwrap().join("OCR/Python/python.exe"));
        let before = std::env::var_os("PATH");
        let spec =
            runtime.command_spec(&folder.path().join("input scan.pdf"), &folder.path().join("input scan_OCR.pdf"), Options::default()).unwrap();
        assert_eq!(spec.working_directory.as_ref().unwrap(), &runtime.root.join("Tesseract-OCR"));
        let args: Vec<_> = spec.arguments.iter().map(|s| s.to_string_lossy().into_owned()).collect();
        assert_eq!(
            &args[..15],
            &[
                "-I",
                "-m",
                "ocrmypdf",
                "--language",
                "rus+eng",
                "--mode",
                "skip",
                "--rotate-pages",
                "--deskew",
                "--rasterizer",
                "pypdfium",
                "--output-type",
                "pdf",
                "--optimize",
                "0"
            ]
        );
        assert!(args[15].ends_with("input scan.pdf") && args[16].ends_with("input scan_OCR.pdf"));
        assert_eq!(std::env::var_os("PATH"), before);
        assert_eq!(spec.environment.iter().find(|(k, _)| k == "FDS_OCR_HIDE_CHILDREN").unwrap().1, "1");
        assert!(spec.removed_environment.contains(&OsString::from("PYTHONPATH")));
        let path = spec.environment.iter().find(|(k, _)| k == "PATH").unwrap().1.clone();
        let paths: Vec<_> = std::env::split_paths(&path).collect();
        assert_eq!(paths[0], runtime.root.join("Tesseract-OCR"));
        assert_eq!(paths[1], runtime.root.join("Python"));
        assert_eq!(paths[2], runtime.root.join("Python/Scripts"));
        assert_eq!(spec.environment.iter().find(|(k, _)| k == "TESSDATA_PREFIX").unwrap().1, runtime.tessdata.as_os_str());
        let spec =
            runtime.command_spec(&folder.path().join("in.pdf"), &folder.path().join("out.pdf"), Options { rotate: false, deskew: false }).unwrap();
        assert!(!spec.arguments.contains(&OsString::from("--rotate-pages")) && !spec.arguments.contains(&OsString::from("--deskew")));
    }
    #[test]
    fn output_and_publish_preserve_source_and_existing_results() {
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("договор.pdf");
        std::fs::write(&original, b"source").unwrap();
        assert_eq!(output_name("договор.pdf"), "договор_OCR.pdf");
        let out = output_path(&original).unwrap();
        std::fs::write(&out, b"existing").unwrap();
        assert!(output_path(&original).unwrap().ends_with("договор_OCR_2.pdf"));
        let candidate = dir.path().join("candidate.pdf");
        std::fs::write(&candidate, b"%PDF-new").unwrap();
        assert_eq!(publish(&candidate, &out).unwrap_err().kind, FailureKind::OutputExists);
        assert_eq!(std::fs::read(&out).unwrap(), b"existing");
        assert_eq!(std::fs::read(&original).unwrap(), b"source");
        let fresh = output_path(&original).unwrap();
        publish(&candidate, &fresh).unwrap();
        assert_eq!(std::fs::read(fresh).unwrap(), b"%PDF-new");
    }
    #[cfg(unix)]
    #[test]
    fn process_reports_failure_and_honours_cancellation_without_ocr() {
        let spec = |script: &str| CommandSpec {
            program: PathBuf::from("/bin/sh"),
            arguments: vec!["-c".into(), script.into()],
            working_directory: None,
            environment: vec![],
            removed_environment: vec![],
        };
        let cancelled = AtomicBool::new(false);
        assert!(execute(&spec("printf runtime-ok"), &cancelled, Some(Duration::from_secs(2))).unwrap().contains("runtime-ok"));
        let error = execute(&spec("printf diagnostic >&2; exit 7"), &cancelled, None).unwrap_err();
        assert_eq!(error.kind, FailureKind::Recognition);
        assert!(error.details.contains("7") && error.details.contains("diagnostic"));
        let cancelled = Arc::new(AtomicBool::new(false));
        let trigger = cancelled.clone();
        let thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            trigger.store(true, Ordering::Relaxed);
        });
        let started = Instant::now();
        assert_eq!(execute(&spec("exec sleep 5"), &cancelled, None).unwrap_err().kind, FailureKind::Cancelled);
        thread.join().unwrap();
        assert!(started.elapsed() < Duration::from_secs(2));
        assert_eq!(execute(&spec("exit 0"), &cancelled, None).unwrap_err().kind, FailureKind::Cancelled);
    }
    #[test]
    fn missing_runtime_and_unsafe_output_fail_before_launch() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Runtime::for_executable(Path::new("relative.exe")).is_err());
        let runtime = Runtime::for_executable(&dir.path().join("FDS-PDF.exe")).unwrap();
        assert_eq!(runtime.check_files().unwrap_err().kind, FailureKind::Runtime);
        let file = dir.path().join("source.pdf");
        assert!(runtime.command_spec(&file, &file, Options::default()).is_err());
    }
}
