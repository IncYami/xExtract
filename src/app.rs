use crate::{
    config_loader::LoadedConfig,
    domain::{ExtractJob, MkvInfo},
    mkv_tool::{MkvTool, OutputStream},
    ui,
    worker::{self, WorkerEvent},
};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, Sender},
    },
    time::Duration,
};

#[derive(Debug, Clone)]
pub enum FileStatus {
    Pending,
    Analyzing,
    Ready,
    Extracting,
    Completed,
    Stopped,
    Error,
}

#[derive(Debug)]
pub struct FileItem {
    pub path: PathBuf,
    pub size: u64,
    pub status: FileStatus,
    pub progress: u8,
    pub info: Option<MkvInfo>,
    pub job: Option<ExtractJob>,
}

#[derive(Debug)]
pub(crate) struct LogEntry {
    pub(crate) file_name: String,
    pub(crate) stream: OutputStream,
    pub(crate) line: String,
}

impl FileItem {
    fn from_path(path: PathBuf) -> Self {
        let size = std::fs::metadata(&path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        Self {
            path,
            size,
            status: FileStatus::Pending,
            progress: 0,
            info: None,
            job: None,
        }
    }
}

pub struct XExtractApp {
    pub(crate) config: crate::domain::AppConfig,
    pub(crate) config_path: Option<PathBuf>,
    pub(crate) mkv_version: String,
    pub(crate) files: Vec<FileItem>,
    pub(crate) logs: Vec<LogEntry>,
    pub(crate) selected_file: Option<usize>,
    pub(crate) selected_profile: usize,
    pub(crate) running: bool,
    pub(crate) cancel_requested: bool,
    worker_sender: Sender<WorkerEvent>,
    worker_receiver: Receiver<WorkerEvent>,
    cancel_flag: Option<Arc<AtomicBool>>,
}

impl XExtractApp {
    pub fn new(
        loaded: LoadedConfig,
        startup_files: Vec<PathBuf>,
        profile_name: Option<String>,
    ) -> Self {
        let tool = MkvTool::new(&loaded.config.mkv_tool_nix_dir);
        let selected_profile = 0;
        let (worker_sender, worker_receiver) = worker::channel();
        let mut app = Self {
            config: loaded.config,
            config_path: loaded.path,
            mkv_version: tool.version(),
            files: Vec::new(),
            logs: Vec::new(),
            selected_file: None,
            selected_profile,
            running: false,
            cancel_requested: false,
            worker_sender,
            worker_receiver,
            cancel_flag: None,
        };
        app.add_files(startup_files);
        if let Some(name) = profile_name
            && let Some(index) = app
                .config
                .profiles
                .iter()
                .position(|profile| profile.name.eq_ignore_ascii_case(&name))
        {
            app.selected_profile = index;
        }
        app
    }

    pub(crate) fn add_files(&mut self, paths: impl IntoIterator<Item = PathBuf>) {
        for path in paths {
            if !is_mkv(&path) || self.files.iter().any(|item| item.path == path) {
                continue;
            }
            self.files.push(FileItem::from_path(path));
        }
        if self.selected_file.is_none() && !self.files.is_empty() {
            self.selected_file = Some(0);
        }
    }

    pub(crate) fn add_folder(&mut self, folder: &Path) {
        let Ok(entries) = std::fs::read_dir(folder) else {
            return;
        };
        self.add_files(entries.filter_map(Result::ok).map(|entry| entry.path()));
    }

    pub(crate) fn remove_file(&mut self, index: usize) {
        if self.running || index >= self.files.len() {
            return;
        }
        self.files.remove(index);
        self.selected_file = if self.files.is_empty() {
            None
        } else {
            Some(index.min(self.files.len() - 1))
        };
    }

    pub(crate) fn clear_files(&mut self) {
        if !self.running {
            self.files.clear();
            self.selected_file = None;
        }
    }

    pub(crate) fn start_batch(&mut self) {
        if self.running || self.files.is_empty() || self.config.profiles.is_empty() {
            return;
        }

        let profile = self.config.profiles[self.selected_profile].clone();
        let files = self.files.iter().map(|item| item.path.clone()).collect();
        for item in &mut self.files {
            item.status = FileStatus::Pending;
            item.progress = 0;
            item.info = None;
            item.job = None;
        }
        self.running = true;
        self.cancel_requested = false;
        self.cancel_flag = Some(worker::start_batch(
            files,
            self.config.clone(),
            profile,
            self.worker_sender.clone(),
        ));
    }

    pub(crate) fn stop_batch(&mut self) {
        if !self.running || self.cancel_requested {
            return;
        }
        if let Some(cancel) = &self.cancel_flag {
            cancel.store(true, Ordering::Relaxed);
        }
        self.cancel_requested = true;
    }

    fn poll_worker(&mut self, context: &egui::Context) {
        while let Ok(event) = self.worker_receiver.try_recv() {
            match event {
                WorkerEvent::Analyzing { index } => {
                    if let Some(item) = self.files.get_mut(index) {
                        item.status = FileStatus::Analyzing;
                        self.selected_file = Some(index);
                    }
                }
                WorkerEvent::Ready { index, info, job } => {
                    if let Some(item) = self.files.get_mut(index) {
                        item.info = Some(info);
                        item.job = Some(*job);
                        item.status = FileStatus::Ready;
                    }
                }
                WorkerEvent::Extracting { index } => {
                    if let Some(item) = self.files.get_mut(index) {
                        item.status = FileStatus::Extracting;
                        item.progress = 0;
                    }
                }
                WorkerEvent::Progress { index, value } => {
                    if let Some(item) = self.files.get_mut(index) {
                        item.progress = value;
                    }
                }
                WorkerEvent::Log {
                    index,
                    stream,
                    line,
                } => {
                    let file_name = self
                        .files
                        .get(index)
                        .and_then(|item| item.path.file_name())
                        .and_then(|name| name.to_str())
                        .unwrap_or("Unknown file")
                        .to_owned();
                    self.logs.push(LogEntry {
                        file_name,
                        stream,
                        line,
                    });
                }
                WorkerEvent::Completed { index } => {
                    if let Some(item) = self.files.get_mut(index) {
                        item.status = FileStatus::Completed;
                        item.progress = 100;
                    }
                }
                WorkerEvent::Stopped { index } => {
                    if let Some(item) = self.files.get_mut(index) {
                        item.status = FileStatus::Stopped;
                    }
                }
                WorkerEvent::Error { index, message } => {
                    if let Some(item) = self.files.get_mut(index) {
                        item.status = FileStatus::Error;
                    }
                    let file_name = self
                        .files
                        .get(index)
                        .and_then(|item| item.path.file_name())
                        .and_then(|name| name.to_str())
                        .unwrap_or("Unknown file")
                        .to_owned();
                    self.logs.push(LogEntry {
                        file_name,
                        stream: OutputStream::App,
                        line: message,
                    });
                }
                WorkerEvent::Finished => {
                    self.running = false;
                    self.cancel_requested = false;
                    self.cancel_flag = None;
                }
            }
        }

        if self.running {
            context.request_repaint_after(Duration::from_millis(100));
        }
    }
}

impl XExtractApp {
    pub fn update(&mut self, ui: &mut egui::Ui) {
        self.poll_worker(ui.ctx());
        ui::draw(self, ui);
    }
}

fn is_mkv(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("mkv"))
}
