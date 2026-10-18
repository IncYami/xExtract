use crate::{
    domain::{ExtractJob, MkvInfo},
    error::AppError,
};
use std::{
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{ChildStderr, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
    },
    thread,
    time::Duration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractionOutcome {
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputStream {
    Stdout,
    Stderr,
    App,
}

#[derive(Debug)]
pub enum ExtractionEvent {
    Progress(u8),
    Output { stream: OutputStream, line: String },
}

#[derive(Debug, Clone)]
pub struct MkvTool {
    tool_dir: Option<PathBuf>,
}

impl MkvTool {
    pub fn new(tool_dir: &str) -> Self {
        Self {
            tool_dir: (!tool_dir.trim().is_empty()).then(|| PathBuf::from(tool_dir)),
        }
    }

    pub fn version(&self) -> String {
        match self.command("mkvmerge").arg("--version").output() {
            Ok(output) if output.status.success() => {
                let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
                text.strip_prefix("mkvmerge ").unwrap_or(&text).to_owned()
            }
            _ => "Unknown".to_owned(),
        }
    }

    pub fn parse_info(
        &self,
        file: &Path,
        events: &Sender<ExtractionEvent>,
    ) -> Result<MkvInfo, AppError> {
        let output = self.command("mkvmerge").arg("-J").arg(file).output()?;
        send_output_lines(events, OutputStream::Stderr, &output.stderr);

        if !output.status.success() {
            send_output_lines(events, OutputStream::Stdout, &output.stdout);
            return Err(AppError::message(format!(
                "mkvmerge exited with code {}",
                output.status.code().unwrap_or(-1)
            )));
        }

        let raw: serde_json::Value = match serde_json::from_slice(&output.stdout) {
            Ok(value) => value,
            Err(error) => {
                send_output_lines(events, OutputStream::Stdout, &output.stdout);
                let output_text = String::from_utf8_lossy(&output.stdout);
                let trailing_output = output_text
                    .chars()
                    .rev()
                    .take(160)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect::<String>();
                return Err(AppError::message(format!(
                    "Failed to parse mkvmerge JSON output: {error} ({} bytes captured; trailing output: {trailing_output:?})",
                    output.stdout.len()
                )));
            }
        };
        Ok(MkvInfo::from_raw(raw)?)
    }

    pub fn extract(
        &self,
        job: &ExtractJob,
        events: &Sender<ExtractionEvent>,
        cancel: &Arc<AtomicBool>,
    ) -> Result<ExtractionOutcome, AppError> {
        let mut command = if job.is_archive_mode {
            self.archive_command(job)?
        } else {
            self.extract_command(job)?
        };

        run_with_progress(&mut command, events, cancel)
    }

    fn extract_command(&self, job: &ExtractJob) -> Result<Command, AppError> {
        let mut command = self.command("mkvextract");
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        command.arg("--gui-mode");
        command.arg(&job.input_file);

        let mut target_dirs = Vec::new();
        if !job.track_ids.is_empty() {
            command.arg("tracks");
            for (id, file_name) in job.track_ids.iter().zip(&job.output_files) {
                let output = job.output_dir.join(file_name);
                if let Some(parent) = output.parent() {
                    target_dirs.push(parent.to_owned());
                }
                command.arg(format!("{id}:{}", output.display()));
            }
        }

        if !job.attachment_ids.is_empty() {
            command.arg("attachments");
            for (id, file_name) in job.attachment_ids.iter().zip(&job.attachment_files) {
                let output = job.output_dir.join(file_name);
                if let Some(parent) = output.parent() {
                    target_dirs.push(parent.to_owned());
                }
                command.arg(format!("{id}:{}", output.display()));
            }
        }

        if let Some(chapters) = &job.extract_chapters {
            let output = job.output_dir.join(chapters);
            if let Some(parent) = output.parent() {
                target_dirs.push(parent.to_owned());
            }
            command.arg("chapters").arg(output);
        }

        create_directories(target_dirs)?;
        Ok(command)
    }

    fn archive_command(&self, job: &ExtractJob) -> Result<Command, AppError> {
        let output_name = job
            .archive_output_file
            .as_ref()
            .ok_or_else(|| AppError::message("archive job has no output file"))?;
        let output = job.output_dir.join(output_name);
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut command = self.command("mkvmerge");
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        command.arg("--gui-mode").arg("-o").arg(output);

        add_track_selection(
            &mut command,
            "--video-tracks",
            &job.video_track_ids,
            "--no-video",
        );
        add_track_selection(
            &mut command,
            "--audio-tracks",
            &job.audio_track_ids,
            "--no-audio",
        );
        add_track_selection(
            &mut command,
            "--subtitle-tracks",
            &job.subtitle_track_ids,
            "--no-subtitles",
        );
        if job.extract_chapters.is_none() {
            command.arg("--no-chapters");
        }
        if job.attachment_ids.is_empty() {
            command.arg("--no-attachments");
        }
        command.arg(&job.input_file);
        Ok(command)
    }

    fn binary(&self, name: &str) -> PathBuf {
        self.tool_dir
            .as_ref()
            .map_or_else(|| PathBuf::from(name), |directory| directory.join(name))
    }

    fn command(&self, name: &str) -> Command {
        let mut command = Command::new(self.binary(name));
        // Keep tool output predictable without breaking UTF-8 metadata in JSON.
        command.env("LC_ALL", "C.UTF-8");
        command
    }
}

fn add_track_selection(command: &mut Command, option: &str, ids: &[u32], empty_option: &str) {
    if ids.is_empty() {
        command.arg(empty_option);
    } else {
        command
            .arg(option)
            .arg(ids.iter().map(u32::to_string).collect::<Vec<_>>().join(","));
    }
}

fn create_directories(directories: Vec<PathBuf>) -> Result<(), AppError> {
    for directory in directories {
        std::fs::create_dir_all(directory)?;
    }
    Ok(())
}

fn run_with_progress(
    command: &mut Command,
    events: &Sender<ExtractionEvent>,
    cancel: &Arc<AtomicBool>,
) -> Result<ExtractionOutcome, AppError> {
    let mut child = command.spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::message("failed to capture MKVToolNix stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppError::message("failed to capture MKVToolNix stderr"))?;

    let (event_sender, event_receiver) = std::sync::mpsc::channel();
    let stdout_events = event_sender.clone();
    let stdout_thread = thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(value) = parse_progress(&line) {
                let _ = stdout_events.send(ExtractionEvent::Progress(value));
            } else if !line.trim().is_empty() {
                let _ = stdout_events.send(ExtractionEvent::Output {
                    stream: OutputStream::Stdout,
                    line,
                });
            }
        }
    });
    let stderr_events = event_sender.clone();
    let stderr_thread = thread::spawn(move || read_stderr(stderr, stderr_events));
    drop(event_sender);

    let cancelled = loop {
        while let Ok(event) = event_receiver.try_recv() {
            let _ = events.send(event);
        }

        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            break true;
        }

        if child.try_wait()?.is_some() {
            break false;
        }
        thread::sleep(Duration::from_millis(40));
    };

    let _ = stdout_thread.join();
    let _ = stderr_thread.join();
    while let Ok(event) = event_receiver.try_recv() {
        let _ = events.send(event);
    }

    if cancelled {
        return Ok(ExtractionOutcome::Cancelled);
    }

    let status = child
        .try_wait()?
        .ok_or_else(|| AppError::message("MKVToolNix process ended without an exit status"))?;

    if !status.success() {
        return Err(AppError::message(format!(
            "MKVToolNix exited with code {}",
            status.code().unwrap_or(-1)
        )));
    }

    let _ = events.send(ExtractionEvent::Progress(100));
    Ok(ExtractionOutcome::Completed)
}

fn read_stderr(stderr: ChildStderr, events: Sender<ExtractionEvent>) {
    for line in BufReader::new(stderr).lines().map_while(Result::ok) {
        if !line.trim().is_empty() {
            let _ = events.send(ExtractionEvent::Output {
                stream: OutputStream::Stderr,
                line: line.clone(),
            });
        }
    }
}

fn send_output_lines(events: &Sender<ExtractionEvent>, stream: OutputStream, output: &[u8]) {
    for line in String::from_utf8_lossy(output).lines() {
        if !line.trim().is_empty() {
            let _ = events.send(ExtractionEvent::Output {
                stream,
                line: line.to_owned(),
            });
        }
    }
}

fn parse_progress(line: &str) -> Option<u8> {
    let line = line.trim();
    let progress = line
        .strip_prefix("#GUI#progress ")
        .or_else(|| line.strip_prefix("Progress:"))?;
    let digits = progress.trim_start().split('%').next()?.trim();
    let value = digits.parse::<u8>().ok()?;
    (value <= 100).then_some(value)
}
