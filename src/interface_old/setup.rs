use std::{
    sync::{Arc, Mutex},
    vec,
};

use crossbeam::channel::Sender;
use crossterm::event::{Event, KeyCode};
use enum_iterator::{next_cycle, Sequence};
use tui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use crate::{dickcord_old::DiscordStatus, state::Config, Action};

use super::{field::Field, ViewController};

#[derive(Default, PartialEq, Sequence)]
enum SelectedField {
    #[default]
    BotToken,
    UserId,
}

pub struct SetupView {
    selected_field: SelectedField,
    actions: Sender<Action>,
    status: Arc<Mutex<DiscordStatus>>,

    bot_token: Field,
    user_id: Field,
}

impl SetupView {
    pub fn new(actions: Sender<Action>, status: Arc<Mutex<DiscordStatus>>) -> Self {
        let mut bot_token = Field::new("Bot Token");
        bot_token.focus();

        let user_id = Field::new("User ID");

        Self {
            selected_field: Default::default(),
            actions,
            status,

            bot_token,
            user_id,
        }
    }

    fn cycle_selection(&mut self) {
        self.selected_field = next_cycle(&self.selected_field).expect("Implements sequence");

        let (focus, blur) = match self.selected_field {
            SelectedField::BotToken => (&mut self.bot_token, &mut self.user_id),
            SelectedField::UserId => (&mut self.user_id, &mut self.bot_token),
        };

        focus.focus();
        blur.blur();
    }

    fn is_valid(&self) -> bool {
        !(self.bot_token.value().is_empty()) && !(self.user_id.value().is_empty())
    }
}

impl ViewController for SetupView {
    fn handle_event(&mut self, event: Event) {
        // Don't allow any inputs while it's connecting
        {
            let status = self.status.lock().unwrap();
            if !matches!(*status, DiscordStatus::Idle | DiscordStatus::Failed(_)) {
                return;
            }
        }

        if let Event::Key(key) = event {
            if key.code == KeyCode::Enter && self.is_valid() {
                let new_config = Config::new(
                    self.bot_token.value(),
                    self.user_id.value().parse().unwrap_or_default(),
                );

                self.actions.send(Action::SetConfig(new_config)).unwrap();

                return;
            }

            if key.code == KeyCode::Tab || key.code == KeyCode::Enter {
                self.cycle_selection();
                return;
            }

            match self.selected_field {
                SelectedField::BotToken => self.bot_token.handle_event(event),
                SelectedField::UserId => self.user_id.handle_event(event),
            };
        }
    }
}

impl Widget for &SetupView {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let block = Block::default()
            .title("─ Setup ")
            .borders(Borders::all())
            .border_style(Style::default().fg(Color::DarkGray));

        let block_inner = block.inner(area);
        block.render(area, buf);

        let status = self.status.lock().unwrap().clone();

        let status_text = match &status {
            DiscordStatus::Idle => {
                "Press tab to switch between fields. Press enter to connect when you're done."
                    .to_string()
            }
            DiscordStatus::Connecting => "Connecting...".to_string(),
            DiscordStatus::Connected => "Connected!".to_string(),
            DiscordStatus::Failed(error) => {
                format!("{}", error)
            }
            _ => String::new(),
        };

        let status_color = match &status {
            DiscordStatus::Idle => Color::DarkGray,
            DiscordStatus::Connecting => Color::Yellow,
            DiscordStatus::Connected => Color::Green,
            DiscordStatus::Failed(_) => Color::Red,
            _ => Color::Reset,
        };

        let status_text = Paragraph::new(status_text).style(Style::default().fg(status_color));

        let help_text = match self.selected_field {
            SelectedField::BotToken => get_bot_token_help(),
            SelectedField::UserId => get_user_id_help(),
        };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(4),
                Constraint::Length(3),
                Constraint::Percentage(100),
            ])
            .margin(1)
            .horizontal_margin(2)
            .split(block_inner);

        self.bot_token.render(chunks[0], buf);
        self.user_id.render(chunks[1], buf);
        status_text.render(chunks[2], buf);
        help_text.render(chunks[3], buf);
    }
}

fn get_bot_token_help() -> Paragraph<'static> {
    let bold = Style::default().add_modifier(Modifier::BOLD);

    let spans = vec![
        Spans::from(vec![
            Span::raw("Required to connect to Discord with a bot. "),
            Span::styled(
                "Make sure \"Server Members Intent\" is enabled in the Bot settings.",
                bold,
            ),
        ]),
        Spans::default(),
        Spans::from(vec![
            Span::raw("If you don't have a bot token at hand, visit https://discord.com/developers/applications/ , Create a new bot by pressing \"New Application\" on the top right of the page, on the application settings page, on the left sidebar go to \"Bot\". Click \"Reset Token\", Discord will generate a bot token pulseshitter can use; copy it, keep it secured, don't lose it. Scroll down, "),
            Span::styled("enable \"Server Members Intent\"", bold),
            Span::raw(", then Save Changes.")
        ]),
        Spans::default(),
        Spans::from(Span::raw("You will need a link to invite your bot into the servers you need to use. On the sidebar, head over to \"OAuth2 > URL Generator.\" Under \"Scopes\" select \"bot\", under \"Bot Permissions\" select \"Connect\" and \"Speak.\" At the bottom, copy the Generated URL and visit it in your browser to invite your bot to your server"))
    ];

    Paragraph::new(spans).wrap(Wrap { trim: false })
}

fn get_user_id_help() -> Paragraph<'static> {
    let spans = vec![
        Spans::from(Span::raw("pulseshitter will automatically join all voice channels this user joins. This should be your own user ID. ")),
        Spans::default(),
        Spans::from(Span::raw("To get your user ID, right click yourself in any server's user list or in a message, then click \"Copy ID.\"")),
        Spans::default(),
        Spans::from(Span::raw("If there's no \"Copy ID\" option, go to Settings > Advanced and enable \"Developer Mode\" and try again."))
    ];

    Paragraph::new(spans).wrap(Wrap { trim: false })
}
