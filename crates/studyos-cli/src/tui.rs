use std::{io, time::Duration};

use anyhow::Result;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    prelude::*,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
use studyos_core::{ContentBlock, PanelTab, ResponseWidget, SessionPlanSummary};

use crate::app::{App, FocusRegion, widget_validation_warning};

pub fn run(mut app: App) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|frame| render(frame, app))?;

        if app.should_quit {
            break;
        }

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                app.handle_key(key);
                app.persist_resume_state()?;
            }
        }
    }

    Ok(())
}

fn render(frame: &mut Frame<'_>, app: &App) {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(12),
            Constraint::Length(12),
            Constraint::Length(2),
        ])
        .split(frame.area());

    render_header(frame, app, vertical[0]);
    render_body(frame, app, vertical[1]);
    render_answer_area(frame, app, vertical[2]);
    render_footer(frame, app, vertical[3]);

    if app.show_help {
        render_help_overlay(frame, app);
    }
}

fn render_header(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let title = Line::from(vec![
        Span::styled(
            "StudyOS",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            app.snapshot.mode.label(),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw("  "),
        Span::raw(&app.snapshot.course),
    ]);

    let meta = Line::from(vec![
        Span::raw(format!("Time {}m", app.snapshot.time_remaining_minutes)),
        Span::raw("  "),
        Span::raw(format!("Due {}", app.snapshot.metrics.due_reviews)),
        Span::raw("  "),
        Span::raw(format!(
            "Deadlines {}",
            app.snapshot.metrics.upcoming_deadlines
        )),
        Span::raw("  "),
        Span::styled(
            format!("Urgency {}", app.snapshot.deadline_urgency.label()),
            Style::default().fg(Color::Magenta),
        ),
    ]);

    let paragraph = Paragraph::new(vec![title, meta])
        .block(Block::default().borders(Borders::ALL).title("Session"));

    frame.render_widget(paragraph, area);
}

fn render_body(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(50), Constraint::Length(38)])
        .split(area);

    render_transcript(frame, app, horizontal[0]);
    render_panel(frame, app, horizontal[1]);
}

fn render_transcript(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let lines = transcript_lines(app);
    let title = if app.focus == FocusRegion::Transcript {
        "Transcript [focus]"
    } else {
        "Transcript"
    };

    let widget = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false })
        .scroll((app.transcript_scroll, 0));

    frame.render_widget(widget, area);
}

fn transcript_lines(app: &App) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    for (index, block) in app.snapshot.transcript.iter().enumerate() {
        match block {
            ContentBlock::Heading(heading) => {
                lines.push(Line::from(Span::styled(
                    heading.text.clone(),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )));
            }
            ContentBlock::Paragraph(paragraph) => {
                lines.push(Line::from(paragraph.text.clone()));
            }
            ContentBlock::BulletList(items) => {
                for item in items {
                    lines.push(Line::from(format!("• {}", item)));
                }
            }
            ContentBlock::MathBlock(math) => {
                lines.push(Line::from(Span::styled(
                    format!("math: {}", math.fallback_text),
                    Style::default().fg(Color::Green),
                )));
            }
            ContentBlock::MatrixBlock(matrix) => {
                lines.push(Line::from(Span::styled(
                    matrix.title.clone(),
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                )));
                for row in &matrix.rows {
                    lines.push(Line::from(format!("| {} |", row.join("  "))));
                }
            }
            ContentBlock::QuestionCard(card) => {
                let prefix = if index == app.active_question_index {
                    ">"
                } else {
                    " "
                };
                lines.push(Line::from(Span::styled(
                    format!("{prefix} question: {}", card.title),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )));
                lines.push(Line::from(format!("  {}", card.prompt)));
            }
            ContentBlock::HintCard(card) => {
                lines.push(Line::from(Span::styled(
                    format!("hint: {}", card.title),
                    Style::default().fg(Color::Magenta),
                )));
                lines.push(Line::from(format!("  {}", card.body)));
            }
            ContentBlock::WarningBox(boxed) => {
                lines.push(Line::from(Span::styled(
                    format!("warning: {}", boxed.title),
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )));
                lines.push(Line::from(format!("  {}", boxed.body)));
            }
            ContentBlock::RecapBox(boxed) => {
                lines.push(Line::from(Span::styled(
                    format!("recap: {}", boxed.title),
                    Style::default().fg(Color::LightBlue),
                )));
                for item in &boxed.highlights {
                    lines.push(Line::from(format!("  - {}", item)));
                }
            }
            ContentBlock::Divider => lines.push(Line::from("")),
        }
        lines.push(Line::from(""));
    }

    lines
}

fn render_panel(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let title = if app.focus == FocusRegion::Panel {
        format!("{} [focus]", app.snapshot.panel_tab.label())
    } else {
        app.snapshot.panel_tab.label().to_string()
    };

    let lines = match app.snapshot.panel_tab {
        PanelTab::SessionPlan => session_plan_lines(&app.snapshot.plan),
        PanelTab::DueReviews => simple_lines(app.review_summary()),
        PanelTab::Deadlines => simple_lines(app.deadline_summary()),
        PanelTab::Misconceptions => simple_lines(app.misconceptions_summary()),
        PanelTab::Scratchpad => simple_lines(
            app.snapshot
                .scratchpad
                .lines()
                .map(ToOwned::to_owned)
                .collect(),
        ),
        PanelTab::Activity => activity_lines(app),
    };

    let panel = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false });

    frame.render_widget(panel, area);
}

fn session_plan_lines(plan: &SessionPlanSummary) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(Span::styled(
            format!("Why now: {}", plan.why_now),
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("Duration: {} minutes", plan.recommended_duration_minutes),
            Style::default().fg(Color::Green),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Warm-up",
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ];

    for question in &plan.warm_up_questions {
        lines.push(Line::from(format!("• {}", question)));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Core targets",
        Style::default().add_modifier(Modifier::BOLD),
    )));

    for target in &plan.core_targets {
        lines.push(Line::from(format!("• {}", target)));
    }

    if let Some(stretch) = &plan.stretch_target {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Stretch",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(format!("• {}", stretch)));
    }

    lines
}

fn simple_lines(items: Vec<String>) -> Vec<Line<'static>> {
    items.into_iter().map(Line::from).collect()
}

fn activity_lines(app: &App) -> Vec<Line<'static>> {
    app.snapshot
        .activity
        .iter()
        .map(|item| {
            let color = match item.status {
                studyos_core::ActivityStatus::Idle => Color::DarkGray,
                studyos_core::ActivityStatus::Running => Color::Yellow,
                studyos_core::ActivityStatus::Healthy => Color::Green,
            };
            Line::from(vec![
                Span::styled(format!("{}: ", item.name), Style::default().fg(color)),
                Span::raw(item.detail.clone()),
            ])
        })
        .collect()
}

fn render_answer_area(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let title = if app.focus == FocusRegion::Widget {
        format!("{} [focus]", app.active_question_title())
    } else {
        app.active_question_title()
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(3)])
        .split(area);

    let widget_lines = match app.active_widget() {
        Some(widget) => widget_lines(widget),
        None => vec![Line::from("No active structured question.")],
    };

    let widget = Paragraph::new(widget_lines)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false });

    frame.render_widget(widget, chunks[0]);

    let warning_lines = app
        .active_widget()
        .and_then(widget_validation_warning)
        .map(|warning| {
            vec![
                Line::from(Span::styled(
                    warning.title,
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )),
                Line::from(warning.body),
            ]
        })
        .unwrap_or_else(|| {
            vec![
                Line::from(Span::styled(
                    "Validation",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from("Structured attempt recorded locally in widget state."),
            ]
        });

    let warnings = Paragraph::new(warning_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Answer checks"),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(warnings, chunks[1]);
}

fn widget_lines(widget: &ResponseWidget) -> Vec<Line<'static>> {
    match widget {
        ResponseWidget::MatrixGrid(state) => {
            let mut lines = vec![
                Line::from("Matrix grid"),
                Line::from("Arrow keys move, type to fill cells, backspace to edit."),
                Line::from(""),
            ];

            for (row_index, row) in state.cells.iter().enumerate() {
                let mut spans = vec![Span::raw("| ")];
                for (col_index, cell) in row.iter().enumerate() {
                    let is_selected =
                        row_index == state.selected_row && col_index == state.selected_col;
                    let display = if cell.is_empty() { "·" } else { cell.as_str() };
                    let style = if is_selected {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    spans.push(Span::styled(format!("{display:^7}"), style));
                }
                spans.push(Span::raw(" |"));
                lines.push(Line::from(spans));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(
                "Press ] or [ to switch active question cards in the transcript.",
            ));
            lines
        }
        ResponseWidget::WorkingAnswer(state) => vec![
            Line::from("Working + final answer"),
            Line::from("Type normally to build working. Shift+character appends to final answer."),
            Line::from(""),
            Line::from(Span::styled(
                "Working",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(state.working.clone()),
            Line::from(""),
            Line::from(Span::styled(
                "Final answer",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(if state.final_answer.is_empty() {
                "·".to_string()
            } else {
                state.final_answer.clone()
            }),
        ],
        ResponseWidget::StepList(state) => {
            let mut lines = vec![
                Line::from("Step list"),
                Line::from("Enter adds a new step. Type to edit the selected step."),
                Line::from(""),
            ];

            for (index, step) in state.steps.iter().enumerate() {
                let prefix = if index == state.selected_step {
                    ">"
                } else {
                    " "
                };
                let display = if step.is_empty() { "·" } else { step.as_str() };
                lines.push(Line::from(format!("{prefix} {}. {}", index + 1, display)));
            }

            lines
        }
        ResponseWidget::RetrievalResponse(state) => vec![
            Line::from("Short retrieval response"),
            Line::from("Type a compact answer. This widget is meant for quick recall prompts."),
            Line::from(""),
            Line::from(if state.response.is_empty() {
                "·".to_string()
            } else {
                state.response.clone()
            }),
        ],
    }
}

fn render_footer(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let footer = Paragraph::new(app.status_line())
        .block(Block::default().borders(Borders::ALL).title("Status"));
    frame.render_widget(footer, area);
}

fn render_help_overlay(frame: &mut Frame<'_>, app: &App) {
    let area = centered_rect(70, 70, frame.area());
    frame.render_widget(Clear, area);

    let mut lines = vec![
        Line::from(Span::styled(
            "StudyOS shell keybindings",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("q        safe quit"),
        Line::from("tab      cycle focus"),
        Line::from("?        toggle this help"),
        Line::from("1..6     switch side panel tabs"),
        Line::from("[ / ]    move between question cards"),
        Line::from(""),
        Line::from(format!("Current focus: {}", app.focus.label())),
        Line::from("Transcript focus: arrows scroll"),
        Line::from("Panel focus: arrows rotate panel tabs"),
        Line::from("Widget focus: edit the active structured response"),
        Line::from("Scratchpad focus: plain text notes, autosave later"),
    ];

    for hint in &app.snapshot.keybindings {
        lines.push(Line::from(format!("{:<8} {}", hint.key, hint.description)));
    }

    let overlay = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Help"))
        .wrap(Wrap { trim: false });

    frame.render_widget(overlay, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
