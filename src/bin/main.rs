
//! Yoosee TUI — Main entry point with Ratatui TUI.

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};
use std::io;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;
use yoosee_tui::{Camera, CameraStatus, Config};

/// Application state
struct App {
    cameras: Vec<Camera>,
    selected: usize,
    layout: LayoutMode,
    running: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum LayoutMode {
    Single,
    Dual,
    Grid,
}

impl App {
    fn new(cameras: Vec<Camera>) -> Self {
        Self {
            cameras,
            selected: 0,
            layout: LayoutMode::Single,
            running: true,
        }
    }

    fn next_camera(&mut self) {
        if !self.cameras.is_empty() {
            self.selected = (self.selected + 1) % self.cameras.len();
        }
    }

    fn prev_camera(&mut self) {
        if !self.cameras.is_empty() {
            self.selected = (self.selected + self.cameras.len() - 1) % self.cameras.len();
        }
    }

    fn select_camera(&mut self, idx: usize) {
        if idx < self.cameras.len() {
            self.selected = idx;
        }
    }

    fn toggle_layout(&mut self) {
        self.layout = match self.layout {
            LayoutMode::Single => LayoutMode::Dual,
            LayoutMode::Dual => LayoutMode::Grid,
            LayoutMode::Grid => LayoutMode::Single,
        };
    }
}

fn render_camera_view(f: &mut Frame, area: Rect, camera: &Camera) {
    let status_color = match camera.status {
        CameraStatus::Connected => Color::Green,
        CameraStatus::Connecting => Color::Yellow,
        CameraStatus::Disconnected => Color::DarkGray,
        CameraStatus::Error | CameraStatus::AuthFailed => Color::Red,
    };

    let block = Block::default()
        .title(format!(" {} ", camera.name))
        .title_style(Style::default().fg(Color::White))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(status_color));

    let placeholder = Paragraph::new("Video stream here...")
        .block(block)
        .style(Style::default().bg(Color::Black));

    f.render_widget(placeholder, area);
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(f.area());

    // Header
    let header_block = Block::default()
        .title(" Header ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let header_text = Paragraph::new(vec![
        Line::from(vec![
            ratatui::text::Span::raw("Yoosee TUI"),
            ratatui::text::Span::raw(" | "),
            ratatui::text::Span::raw(format!("{} cameras", app.cameras.len())),
            ratatui::text::Span::raw(" | Layout: "),
            ratatui::text::Span::raw(format!("{:?}", app.layout)),
        ]),
    ])
    .style(Style::default().fg(Color::Cyan))
    .block(header_block);

    f.render_widget(header_text, chunks[0]);

    // Main content
    match app.layout {
        LayoutMode::Single => {
            if let Some(cam) = app.cameras.get(app.selected) {
                render_camera_view(f, chunks[1], cam);
            } else {
                let no_cam = Paragraph::new("No cameras configured")
                    .block(Block::default().borders(Borders::ALL));
                f.render_widget(no_cam, chunks[1]);
            }
        }
        LayoutMode::Dual => {
            if app.cameras.len() >= 2 {
                let halves = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(chunks[1]);

                if let Some(cam0) = app.cameras.get(0) {
                    render_camera_view(f, halves[0], cam0);
                }
                if let Some(cam1) = app.cameras.get(1) {
                    render_camera_view(f, halves[1], cam1);
                }
            } else if let Some(cam) = app.cameras.first() {
                render_camera_view(f, chunks[1], cam);
            }
        }
        LayoutMode::Grid => {
            let cols = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(chunks[1]);

            let row0 = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(cols[0]);

            let row1 = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(cols[1]);

            for (i, area) in [row0[0], row0[1], row1[0], row1[1]].iter().enumerate() {
                if let Some(cam) = app.cameras.get(i) {
                    render_camera_view(f, *area, cam);
                }
            }
        }
    }

    // Footer
    let footer_block = Block::default()
        .title(" Controls ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let footer_text = Paragraph::new(vec![
        Line::from(vec![
            ratatui::text::Span::raw("[1-4] Select  "),
            ratatui::text::Span::raw("[G] Layout  "),
            ratatui::text::Span::raw("[F] Fullscreen  "),
            ratatui::text::Span::raw("[S] Snapshot  "),
            ratatui::text::Span::raw("[Q] Quit"),
        ]),
    ])
    .style(Style::default().fg(Color::DarkGray))
    .block(footer_block);

    f.render_widget(footer_text, chunks[2]);
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("yoosee_tui=info".parse()?))
        .init();

    info!("Starting Yoosee TUI v{}", env!("CARGO_PKG_VERSION"));

    // Try to load config
    let config_path = Config::default_path();
    let cameras = if config_path.exists() {
        match Config::load(&config_path) {
            Ok(cfg) => {
                info!("Loaded {} cameras from config", cfg.cameras.len());
                cfg.cameras.into_iter().enumerate().map(|(i, c)| {
                    Camera {
                        uid: format!("cam-{}", i),
                        name: c.name,
                        host: c.host.parse().unwrap_or_else(|_| "0.0.0.0".parse().unwrap()),
                        rtsp_port: c.rtsp_port,
                        onvif_port: c.onvif_port,
                        username: c.username,
                        password: c.password.unwrap_or_default(),
                        status: CameraStatus::Disconnected,
                    }
                }).collect()
            }
            Err(e) => {
                eprintln!("Failed to load config: {}. Using empty camera list.", e);
                vec![]
            }
        }
    } else {
        eprintln!("No config found at {}. Run with --discover to find cameras.", config_path.display());
        vec![]
    };

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(cameras);
    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;

    if let Err(e) = res {
        error!("Application error: {}", e);
        eprintln!("Error: {}", e);
    }

    Ok(())
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') | KeyCode::Char('Q') => {
                    app.running = false;
                    break;
                }
                KeyCode::Char('g') | KeyCode::Char('G') => {
                    app.toggle_layout();
                }
                KeyCode::Char('1') => app.select_camera(0),
                KeyCode::Char('2') => app.select_camera(1),
                KeyCode::Char('3') => app.select_camera(2),
                KeyCode::Char('4') => app.select_camera(3),
                KeyCode::Tab => app.next_camera(),
                KeyCode::BackTab => app.prev_camera(),
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    if let Some(cam) = app.cameras.get(app.selected) {
                        info!("Snapshot requested for camera: {}", cam.name);
                    }
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    if let Some(cam) = app.cameras.get(app.selected) {
                        info!("Record toggle requested for camera: {}", cam.name);
                    }
                }
                KeyCode::Char('f') | KeyCode::Char('F') => {
                    info!("Fullscreen toggle");
                }
                KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down => {
                    if let Some(cam) = app.cameras.get(app.selected) {
                        info!("PTZ direction {:?} for camera: {}", key.code, cam.name);
                    }
                }
                KeyCode::Char('+') | KeyCode::Char('=') => {
                    if let Some(cam) = app.cameras.get(app.selected) {
                        info!("PTZ zoom in for camera: {}", cam.name);
                    }
                }
                KeyCode::Char('-') => {
                    if let Some(cam) = app.cameras.get(app.selected) {
                        info!("PTZ zoom out for camera: {}", cam.name);
                    }
                }
                _ => {}
            }
        }

        if !app.running {
            break;
        }
    }

    Ok(())
}
