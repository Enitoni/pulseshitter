use crossterm::event::{Event, KeyCode};
use tui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::{
    app::{AppAction, AppContext},
    interface::View,
};

struct Option {
    context: AppContext,
    action: AppAction,
    paragraph: String,
    kind: OptionKind,
    focused: bool,
}

enum OptionKind {
    Switch(bool),
    Button,
}

pub struct SettingsModule {
    options: Vec<Option>,
    selected_option: usize,
    focused: bool,
}

impl SettingsModule {
    pub fn new(context: AppContext) -> Self {
        Self {
            options: vec![
                Option::new(
                    context.clone(),
                    "Show Meter".to_owned(),
                    OptionKind::Switch(false),
                    AppAction::Exit,
                ),
                Option::new(
                    context.clone(),
                    "Screenshare Only".to_owned(),
                    OptionKind::Switch(false),
                    AppAction::Exit,
                ),
                Option::new(
                    context.clone(),
                    "Redo Setup".to_owned(),
                    OptionKind::Button,
                    AppAction::Exit,
                ),
            ],
            focused: false,
            selected_option: 0,
        }
    }

    fn navigate(&mut self, amount: isize) {
        let new_index =
            ((self.selected_option) as isize + amount).rem_euclid(self.options.len() as isize);

        self.selected_option = new_index as usize;
        self.update_focus_states()
    }

    fn update_focus_states(&mut self) {
        for option in self.options.iter_mut() {
            option.blur();
        }

        if self.focused {
            self.options[self.selected_option].focus();
        }
    }

    pub fn focus(&mut self) {
        self.focused = true;
        self.update_focus_states();
    }

    pub fn blur(&mut self) {
        self.focused = false;
        self.update_focus_states();
    }
}

impl View for SettingsModule {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .border_style(Style::default().fg(Color::DarkGray))
            .title("─ Settings ")
            .borders(Borders::all());

        let block_inner = {
            let area = block.inner(area);
            Rect::new(
                area.left() + 2,
                area.top() + 1,
                area.width - 3,
                area.height - 1,
            )
        };

        let calculated_constraints: Vec<_> =
            self.options.iter().map(|_| Constraint::Length(1)).collect();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(calculated_constraints)
            .split(block_inner);

        block.render(area, buf);

        for (index, option) in self.options.iter().enumerate() {
            option.render(chunks[index], buf);
        }
    }

    fn handle_event(&mut self, event: Event) {
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Up => self.navigate(-1),
                KeyCode::Down => self.navigate(1),
                _ => {}
            };
        }

        self.options[self.selected_option].handle_event(event);
    }
}

impl Option {
    fn new(context: AppContext, name: String, kind: OptionKind, action: AppAction) -> Self {
        Self {
            context,
            paragraph: name,
            focused: false,
            action,
            kind,
        }
    }

    fn toggle_if_switch(&mut self) {
        if let OptionKind::Switch(is_selected) = &self.kind {
            self.kind = OptionKind::Switch(!is_selected);
        }
    }

    fn focus(&mut self) {
        self.focused = true;
    }

    fn blur(&mut self) {
        self.focused = false;
    }
}

impl View for Option {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(area.width - 3), Constraint::Length(3)])
            .split(area);

        let focus_symbol = if self.focused {
            FOCUS_SYMBOL
        } else {
            IDLE_SYMBOL
        };

        let paragraph = Paragraph::new(format!("{} {}", focus_symbol, self.paragraph));
        paragraph.render(chunks[0], buf);

        if let OptionKind::Switch(is_selected) = &self.kind {
            let symbol = if *is_selected { "ON" } else { "OFF" };

            let paragraph = Paragraph::new(symbol).style(if *is_selected {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::DarkGray)
            });

            paragraph.render(chunks[1], buf);
        }
    }

    fn handle_event(&mut self, event: Event) {
        if let Event::Key(key) = event {
            if key.code == KeyCode::Enter {
                self.toggle_if_switch();
                self.context.dispatch_action(self.action.clone());
            }
        }
    }
}

const IDLE_SYMBOL: &str = "○";
const FOCUS_SYMBOL: &str = "●";
