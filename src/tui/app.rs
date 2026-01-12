use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use parking_lot::RwLock;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::Paragraph;
use ratatui::Terminal;
use std::io::stdout;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

use crate::export::export_json_file;
use crate::state::Session;
use crate::tui::views::{HelpView, HopDetailView, MainView};

/// UI state
pub struct UiState {
    /// Currently selected hop index (0-indexed into displayed hops)
    pub selected: Option<usize>,
    /// Whether probing is paused
    pub paused: bool,
    /// Show help overlay
    pub show_help: bool,
    /// Show expanded hop view
    pub show_hop_detail: bool,
    /// Status message to display
    pub status_message: Option<(String, std::time::Instant)>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            selected: None,
            paused: false,
            show_help: false,
            show_hop_detail: false,
            status_message: None,
        }
    }
}

impl UiState {
    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some((msg.into(), std::time::Instant::now()));
    }

    pub fn clear_old_status(&mut self) {
        if let Some((_, time)) = &self.status_message {
            if time.elapsed() > Duration::from_secs(3) {
                self.status_message = None;
            }
        }
    }
}

/// Run the TUI application
pub async fn run_tui(
    state: Arc<RwLock<Session>>,
    cancel: CancellationToken,
) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut ui_state = UiState::default();
    let tick_rate = Duration::from_millis(100);

    let result = run_app(&mut terminal, state.clone(), &mut ui_state, cancel.clone(), tick_rate).await;

    // Restore terminal
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    result
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    state: Arc<RwLock<Session>>,
    ui_state: &mut UiState,
    cancel: CancellationToken,
    tick_rate: Duration,
) -> Result<()> {
    loop {
        // Check cancellation
        if cancel.is_cancelled() {
            break;
        }

        // Clear old status messages
        ui_state.clear_old_status();

        // Draw
        terminal.draw(|f| {
            let session = state.read();
            draw_ui(f, &session, ui_state);
        })?;

        // Handle input with timeout
        if event::poll(tick_rate)? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                // Handle overlays first
                if ui_state.show_help {
                    ui_state.show_help = false;
                    continue;
                }

                if ui_state.show_hop_detail {
                    if key.code == KeyCode::Esc {
                        ui_state.show_hop_detail = false;
                    }
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') => {
                        cancel.cancel();
                        break;
                    }
                    KeyCode::Char('?') | KeyCode::Char('h') => {
                        ui_state.show_help = true;
                    }
                    KeyCode::Char('p') => {
                        ui_state.paused = !ui_state.paused;
                        // TODO: Actually pause/resume probe engine
                        ui_state.set_status(if ui_state.paused {
                            "Paused"
                        } else {
                            "Resumed"
                        });
                    }
                    KeyCode::Char('r') => {
                        // Reset stats (would need mutable access)
                        ui_state.set_status("Stats reset not yet implemented");
                    }
                    KeyCode::Char('e') => {
                        let session = state.read();
                        match export_json_file(&session) {
                            Ok(filename) => {
                                ui_state.set_status(format!("Exported to {}", filename));
                            }
                            Err(e) => {
                                ui_state.set_status(format!("Export failed: {}", e));
                            }
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        let session = state.read();
                        let hop_count = session.hops.iter().filter(|h| h.sent > 0).count();
                        if hop_count > 0 {
                            ui_state.selected = Some(match ui_state.selected {
                                Some(i) if i > 0 => i - 1,
                                Some(_) => hop_count - 1,
                                None => hop_count - 1,
                            });
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        let session = state.read();
                        let hop_count = session.hops.iter().filter(|h| h.sent > 0).count();
                        if hop_count > 0 {
                            ui_state.selected = Some(match ui_state.selected {
                                Some(i) if i < hop_count - 1 => i + 1,
                                Some(_) => 0,
                                None => 0,
                            });
                        }
                    }
                    KeyCode::Enter => {
                        if ui_state.selected.is_some() {
                            ui_state.show_hop_detail = true;
                        }
                    }
                    KeyCode::Esc => {
                        ui_state.selected = None;
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

fn draw_ui(f: &mut ratatui::Frame, session: &Session, ui_state: &UiState) {
    let area = f.area();

    // Layout: main view + status bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    // Main view
    let main_view = MainView::new(session, ui_state.selected, ui_state.paused);
    f.render_widget(main_view, chunks[0]);

    // Status bar
    let status_text = if let Some((ref msg, _)) = ui_state.status_message {
        msg.clone()
    } else {
        "q quit | p pause | r reset | e export | ? help | \u{2191}\u{2193} select | \u{23ce} expand".to_string()
    };

    let status_bar = Paragraph::new(status_text)
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(status_bar, chunks[1]);

    // Overlays
    if ui_state.show_help {
        f.render_widget(HelpView, area);
    }

    if ui_state.show_hop_detail {
        if let Some(selected) = ui_state.selected {
            let hops: Vec<_> = session.hops.iter().filter(|h| h.sent > 0).collect();
            if let Some(hop) = hops.get(selected) {
                f.render_widget(HopDetailView::new(hop), area);
            }
        }
    }
}
