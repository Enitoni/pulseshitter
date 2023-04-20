use std::sync::{Arc, Mutex};

use crossbeam::channel::Sender;
use crossterm::event::{Event, KeyCode};
use enum_iterator::{next_cycle, Sequence};
use tui::{
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use crate::{dickcord::DiscordStatus, state::Config, Action};

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
        let mut bot_token = Field::new("Bot token");
        bot_token.focus();

        let user_id = Field::new("User id");

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
        let block = Block::default().title("Setup").borders(Borders::all());
        let block_inner = block.inner(area);
        block.render(area, buf);

        let status_text = match &*(self.status.lock().unwrap()) {
            DiscordStatus::Idle => {
                "Press tab to switch between fields. Press enter to connect when you're done."
                    .to_string()
            }
            DiscordStatus::Connecting => "Connecting...".to_string(),
            DiscordStatus::Connected => "Connected!".to_string(),
            DiscordStatus::Failed(error) => {
                format!("Uh oh, something went wrong. {}", error)
            }
            _ => String::new(),
        };

        let status_text = Paragraph::new(status_text);

        let help_text = match self.selected_field {
            SelectedField::BotToken => BOT_TOKEN_HELP,
            SelectedField::UserId => USER_ID_HELP,
        };

        let help_text = Paragraph::new(help_text).wrap(Wrap { trim: false });

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(2),
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

const BOT_TOKEN_HELP: &str = "The bot token is the password of your bot. This can be found in https://discord.com/developers under \"Applications\" and \"Bot\", in which you can generate your token there.";
const USER_ID_HELP: &str = "The user that the bot should follow, which in most cases is yourself. The bot will join the same voice call that the user is in. Right click on a user and press \"Copy User ID\" listed at the bottom. If no such button exists, enable developer mode by going in Settings > Appearance > Developer Mode (found at the bottom).";
