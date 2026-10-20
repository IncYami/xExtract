mod app;
mod config_loader;
mod domain;
mod error;
mod icons;
mod mkv_tool;
mod ui;
mod worker;

use app::XExtractApp;
use config_loader::{ConfigLoader, LoadedConfig};
use egui::ViewportId;
use egui_wgpu::{RendererOptions, WgpuConfiguration};
use egui_winit::State;
use std::{
    num::NonZeroU32,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowAttributes},
};

const DEFAULT_WINDOW_WIDTH: f64 = 960.0;
const DEFAULT_WINDOW_HEIGHT: f64 = 840.0;
const MIN_WINDOW_WIDTH: f64 = 900.0;
const MIN_WINDOW_HEIGHT: f64 = 760.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let loaded = match ConfigLoader::load() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("Could not load configuration: {error}");
            ConfigLoader::fallback()
        }
    };

    let startup = StartupOptions::from_args();
    let mut event_loop_builder = EventLoop::builder();
    #[cfg(target_os = "linux")]
    {
        use winit::platform::wayland::EventLoopBuilderExtWayland;
        use winit::platform::x11::EventLoopBuilderExtX11;

        let backend_override = std::env::var("XEXTRACT_BACKEND").ok();
        let has_wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
        let has_x11 = std::env::var_os("DISPLAY").is_some();
        match backend_override.as_deref() {
            Some("x11") => {
                event_loop_builder.with_x11();
            }
            Some("wayland") => {
                event_loop_builder.with_wayland();
            }
            _ if has_wayland && has_x11 => {
                event_loop_builder.with_x11();
            }
            _ => {}
        }
    }
    let event_loop = event_loop_builder.build()?;
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut native = NativeApp::new(loaded, startup);
    event_loop.run_app(&mut native)?;
    Ok(())
}

struct NativeApp {
    loaded: Option<LoadedConfig>,
    startup: Option<StartupOptions>,
    app: Option<XExtractApp>,
    context: Option<egui::Context>,
    egui_state: Option<State>,
    painter: Option<egui_wgpu::winit::Painter>,
    window: Option<Arc<Window>>,
}

impl NativeApp {
    fn new(loaded: LoadedConfig, startup: StartupOptions) -> Self {
        Self {
            loaded: Some(loaded),
            startup: Some(startup),
            app: None,
            context: None,
            egui_state: None,
            painter: None,
            window: None,
        }
    }

    fn redraw(&mut self) {
        let (Some(window), Some(context), Some(egui_state), Some(painter), Some(app)) = (
            self.window.clone(),
            self.context.clone(),
            self.egui_state.as_mut(),
            self.painter.as_mut(),
            self.app.as_mut(),
        ) else {
            return;
        };

        let raw_input = egui_state.take_egui_input(&window);
        let full_output = context.run_ui(raw_input, |ui| app.update(ui));
        let egui::FullOutput {
            platform_output,
            mut textures_delta,
            shapes,
            pixels_per_point,
            ..
        } = full_output;

        egui_state.handle_platform_output(&window, platform_output);
        let clipped_primitives = context.tessellate(shapes, pixels_per_point);
        painter.paint_and_update_textures(
            ViewportId::ROOT,
            pixels_per_point,
            [0.075, 0.075, 0.09, 1.0],
            &clipped_primitives,
            &mut textures_delta,
            Vec::new(),
            &window,
        );
    }
}

impl ApplicationHandler for NativeApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attributes = WindowAttributes::default()
            .with_title("xExtract - MKV Track Extractor")
            .with_inner_size(winit::dpi::LogicalSize::new(
                DEFAULT_WINDOW_WIDTH,
                DEFAULT_WINDOW_HEIGHT,
            ))
            .with_min_inner_size(winit::dpi::LogicalSize::new(
                MIN_WINDOW_WIDTH,
                MIN_WINDOW_HEIGHT,
            ));

        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                eprintln!("Could not create the window: {error}");
                event_loop.exit();
                return;
            }
        };

        let context = egui::Context::default();
        icons::install_fonts(&context);

        let mut painter = pollster::block_on(egui_wgpu::winit::Painter::new(
            context.clone(),
            WgpuConfiguration::default(),
            false,
            RendererOptions::default(),
        ));

        if let Err(error) =
            pollster::block_on(painter.set_window(ViewportId::ROOT, Some(window.clone())))
        {
            eprintln!("Could not initialize the wgpu surface: {error}");
            event_loop.exit();
            return;
        }

        let egui_state = State::new(
            context.clone(),
            ViewportId::ROOT,
            window.as_ref(),
            Some(window.scale_factor() as f32),
            window.theme(),
            painter.max_texture_side(),
        );

        let loaded = self.loaded.take().expect("loaded configuration missing");
        let startup = self.startup.take().expect("startup options missing");
        self.app = Some(XExtractApp::new(loaded, startup.files, startup.profile));
        self.context = Some(context);
        self.egui_state = Some(egui_state);
        self.painter = Some(painter);
        self.window = Some(window.clone());
        window.request_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self.window.clone() else {
            return;
        };
        if window.id() != window_id {
            return;
        }

        if matches!(event, WindowEvent::CloseRequested) {
            event_loop.exit();
            return;
        }

        if let (Some(egui_state), Some(context)) = (self.egui_state.as_mut(), self.context.as_ref())
        {
            let response = egui_state.on_window_event(&window, &event);
            if response.repaint {
                window.request_redraw();
            }
            context.request_repaint();
        }

        match event {
            WindowEvent::DroppedFile(path) => {
                if let Some(app) = self.app.as_mut() {
                    app.add_files([path]);
                }
                window.request_redraw();
            }
            WindowEvent::Resized(size) => {
                if let (Some(width), Some(height)) =
                    (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
                    && let Some(painter) = self.painter.as_mut()
                {
                    painter.on_window_resized(ViewportId::ROOT, width, height);
                }
                window.request_redraw();
            }
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.app.as_ref().is_some_and(|app| app.running) {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(100),
            ));
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}

struct StartupOptions {
    profile: Option<String>,
    files: Vec<PathBuf>,
}

impl StartupOptions {
    fn from_args() -> Self {
        let mut profile = None;
        let mut files = Vec::new();
        let mut args = std::env::args_os().skip(1);

        while let Some(argument) = args.next() {
            if argument == "--profile" {
                profile = args.next().and_then(|value| value.into_string().ok());
            } else {
                let path = PathBuf::from(argument);
                if path.is_file() {
                    files.push(path);
                }
            }
        }

        if files.is_empty()
            && let Ok(current_dir) = std::env::current_dir()
        {
            files = std::fs::read_dir(current_dir)
                .into_iter()
                .flat_map(|entries| entries.filter_map(Result::ok))
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension()
                        .and_then(|extension| extension.to_str())
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("mkv"))
                })
                .collect();
        }

        Self { profile, files }
    }
}
