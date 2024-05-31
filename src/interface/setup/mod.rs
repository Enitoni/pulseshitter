use crossterm::event::{Event, KeyCode};
use form::{Form, SelectedField};
use tui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use crate::{
    app::{AppAction, AppContext},
    dickcord::State as DiscordState,
    state::Config,
};

use super::View;
mod form;

pub struct Setup {
    context: AppContext,
    form: Form,
}

impl Setup {
    pub fn new(context: AppContext) -> Self {
        Self {
            context,
            form: Form::new(),
        }
    }

    fn is_valid(&self) -> bool {
        let (bot_token, user_id) = self.form.current_values();
        !(bot_token.is_empty()) && !(user_id.is_empty())
    }
}

impl View for Setup {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let outer_margin = Layout::default()
            .constraints([Constraint::Percentage(100)])
            .margin(2)
            .vertical_margin(1)
            .split(area);

        let block = Block::default()
            .title("─ Setup ")
            .borders(Borders::all())
            .border_style(Style::default().fg(Color::DarkGray));

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(7),
                Constraint::Length(3),
                Constraint::Percentage(100),
            ])
            .margin(1)
            .horizontal_margin(2)
            .split(block.inner(outer_margin[0]));

        let discord_state = self.context.discord_state();

        let status_text = match &discord_state {
            DiscordState::Idle => {
                "Press tab to switch between fields. Press enter to connect when you're done."
                    .to_string()
            }
            DiscordState::Connecting => "Connecting...".to_string(),
            DiscordState::Connected(_, _) => "Connected!".to_string(),
            DiscordState::Error(error) => error.to_string(),
        };

        let status_color = match &discord_state {
            DiscordState::Idle => Color::DarkGray,
            DiscordState::Connecting => Color::Yellow,
            DiscordState::Connected(_, _) => Color::Green,
            DiscordState::Error(_) => Color::Red,
        };

        let status_text = Paragraph::new(status_text).style(Style::default().fg(status_color));

        let help_text = match self.form.current_selection() {
            SelectedField::BotToken => get_bot_token_help(),
            SelectedField::UserId => get_user_id_help(),
        };

        self.form.render(chunks[0], buf);
        status_text.render(chunks[1], buf);
        help_text.render(chunks[2], buf);
        block.render(outer_margin[0], buf);
    }

    fn handle_event(&mut self, event: Event) {
        let discord_state = self.context.discord_state();

        // Don't allow the user to input while we're connecting
        if !matches!(discord_state, DiscordState::Idle | DiscordState::Error(_)) {
            return;
        }

        // Handle submission
        if let Event::Key(key) = event {
            if key.code == KeyCode::Enter && self.is_valid() {
                let (bot_token, user_id) = self.form.current_values();
                let safe_user_id = user_id.parse().unwrap_or_default();

                let new_config = Config::new(bot_token, safe_user_id);
                self.context
                    .dispatch_action(AppAction::SetConfig(new_config));

                return;
            }
        }

        // Otherwise, handle the form events
        self.form.handle_event(event);
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
