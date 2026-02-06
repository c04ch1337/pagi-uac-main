//! pagi-offsec-ui: TUI with Ratatui. Raw data streams, network logs, keyboard-driven.
//! AGI reasoning: pagi_core::Orchestrator::dispatch().

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use pagi_core::Goal;
use pagi_offsec_ui::{build_orchestrator, default_tenant};
use ratatui::{
    backend::CrosstermBackend,
    prelude::*,
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};
use std::io::stdout;
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let storage = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let storage = storage.join("data");
    let orchestrator = build_orchestrator(&storage)?;
    let ctx = default_tenant();

    let mut log_lines: Vec<String> = vec![
        "[pagi-offsec-ui] TUI started. Keys: R = dispatch QueryKnowledge, Q = quit.".to_string(),
        "[stream] Ready for raw data / network log view.".to_string(),
    ];
    let mut scroll = 0;
    let mut scroll_state = ScrollbarState::default();

    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    loop {
        scroll_state = scroll_state.content_length(log_lines.len()).position(scroll);
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(10),
                    Constraint::Length(1),
                ])
                .split(f.area());

            let header = Paragraph::new("PAGI OffSec UI — [R] Reason (dispatch)  [Q] Quit")
                .block(Block::default().borders(Borders::ALL).title(" Header "));
            f.render_widget(header, chunks[0]);

            let log_text = log_lines.join("\n");
            let para = Paragraph::new(log_text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Log / Data stream "),
                )
                .scroll((scroll as u16, 0));
            f.render_widget(para, chunks[1]);
            f.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .thumb_symbol("█")
                    .track_symbol(Some("│")),
                chunks[1],
                &mut scroll_state,
            );

            let help = Paragraph::new("keyboard-driven: R=dispatch, Q=quit");
            f.render_widget(help, chunks[2]);
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => break,
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        let orch = Arc::clone(&orchestrator);
                        let ctx = ctx.clone();
                        let goal = Goal::QueryKnowledge {
                            slot_id: 1,
                            query: "brand_voice".to_string(),
                        };
                        log_lines.push("[dispatch] Calling Orchestrator::dispatch(QueryKnowledge)...".to_string());
                        let result = tokio::runtime::Runtime::new()?.block_on(orch.dispatch(&ctx, goal));
                        let line = match &result {
                            Ok(v) => format!("[result] {}", serde_json::to_string(v).unwrap_or_else(|_| v.to_string())),
                            Err(e) => format!("[error] {}", e),
                        };
                        log_lines.push(line);
                        scroll = log_lines.len().saturating_sub(10).max(0);
                    }
                    KeyCode::Up => scroll = scroll.saturating_sub(1),
                    KeyCode::Down => scroll = (scroll + 1).min(log_lines.len().saturating_sub(1)),
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
