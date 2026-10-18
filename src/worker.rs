use crate::{
    domain::{AppConfig, ExtractJob, MkvInfo, Profile},
    error::AppError,
    mkv_tool::{ExtractionEvent, MkvTool, OutputStream},
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
};

#[derive(Debug)]
pub enum WorkerEvent {
    Analyzing {
        index: usize,
    },
    Ready {
        index: usize,
        info: MkvInfo,
        job: Box<ExtractJob>,
    },
    Extracting {
        index: usize,
    },
    Progress {
        index: usize,
        value: u8,
    },
    Log {
        index: usize,
        stream: OutputStream,
        line: String,
    },
    Completed {
        index: usize,
    },
    Stopped {
        index: usize,
    },
    Error {
        index: usize,
        message: String,
    },
    Finished,
}

pub fn channel() -> (Sender<WorkerEvent>, Receiver<WorkerEvent>) {
    mpsc::channel()
}

pub fn start_batch(
    files: Vec<PathBuf>,
    config: AppConfig,
    profile: Profile,
    sender: Sender<WorkerEvent>,
) -> Arc<AtomicBool> {
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = Arc::clone(&cancel);
    thread::spawn(move || run_batch(files, config, profile, sender, worker_cancel));
    cancel
}

fn run_batch(
    files: Vec<PathBuf>,
    config: AppConfig,
    profile: Profile,
    sender: Sender<WorkerEvent>,
    cancel: Arc<AtomicBool>,
) {
    let tool = MkvTool::new(&config.mkv_tool_nix_dir);

    for (index, file) in files.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let _ = sender.send(WorkerEvent::Analyzing { index });
        let (tool_events_sender, tool_events_receiver) = mpsc::channel();
        let result = analyze_file(&tool, file, &config, &profile, &tool_events_sender);
        drop(tool_events_sender);
        for event in tool_events_receiver {
            forward_tool_event(index, event, &sender);
        }

        let (info, job) = match result {
            Ok(value) => value,
            Err(error) => {
                let _ = sender.send(WorkerEvent::Error {
                    index,
                    message: error.to_string(),
                });
                continue;
            }
        };

        let _ = sender.send(WorkerEvent::Ready {
            index,
            info,
            job: Box::new(job.clone()),
        });

        if cancel.load(Ordering::Relaxed) {
            let _ = sender.send(WorkerEvent::Stopped { index });
            break;
        }

        if !job.has_work() {
            let _ = sender.send(WorkerEvent::Completed { index });
            continue;
        }

        let _ = sender.send(WorkerEvent::Extracting { index });
        let (event_sender, event_receiver) = mpsc::channel();
        let tool_for_job = tool.clone();
        let worker_job = job.clone();
        let cancel_for_job = Arc::clone(&cancel);
        let extraction = thread::spawn(move || {
            tool_for_job.extract(&worker_job, &event_sender, &cancel_for_job)
        });

        while let Ok(event) = event_receiver.recv() {
            forward_tool_event(index, event, &sender);
        }

        match extraction.join() {
            Ok(Ok(crate::mkv_tool::ExtractionOutcome::Completed)) => {
                let _ = sender.send(WorkerEvent::Completed { index });
            }
            Ok(Ok(crate::mkv_tool::ExtractionOutcome::Cancelled)) => {
                let _ = sender.send(WorkerEvent::Stopped { index });
                break;
            }
            Ok(Err(error)) => send_error(&sender, index, error),
            Err(_) => send_error(
                &sender,
                index,
                AppError::message("extraction worker panicked"),
            ),
        }
    }

    let _ = sender.send(WorkerEvent::Finished);
}

fn analyze_file(
    tool: &MkvTool,
    file: &std::path::Path,
    config: &AppConfig,
    profile: &Profile,
    events: &Sender<ExtractionEvent>,
) -> Result<(MkvInfo, ExtractJob), AppError> {
    let info = tool.parse_info(file, events)?;
    let job = crate::domain::job::create_job(file, &info, profile, config)?;
    Ok((info, job))
}

fn forward_tool_event(index: usize, event: ExtractionEvent, sender: &Sender<WorkerEvent>) {
    let event = match event {
        ExtractionEvent::Progress(value) => WorkerEvent::Progress { index, value },
        ExtractionEvent::Output { stream, line } => WorkerEvent::Log {
            index,
            stream,
            line,
        },
    };
    let _ = sender.send(event);
}

fn send_error(sender: &Sender<WorkerEvent>, index: usize, error: AppError) {
    let _ = sender.send(WorkerEvent::Error {
        index,
        message: error.to_string(),
    });
}
