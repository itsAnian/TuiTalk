use crate::command;
use crate::ui;
use color_eyre::Result;
use futures_channel::mpsc::UnboundedSender;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use shared::*;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

const FAST_SCROLL: usize = 10;
const DEFAULT_SCROLL: usize = 1;

pub struct App {
    pub input: String,
    pub character_index: usize,
    pub input_mode: InputMode,
    pub scroll: usize,
    pub max_scroll: usize,
    pub auto_scroll: bool,
    pub communication: Arc<Mutex<Vec<TalkProtocol>>>,
    pub tx: UnboundedSender<TalkProtocol>,
    pub username: String,
    pub room: i32,
    pub uuid: Uuid,
}

pub enum InputMode {
    Normal,
    Editing,
}

impl App {
    pub fn new(
        transmit: UnboundedSender<TalkProtocol>,
        com: Arc<Mutex<Vec<TalkProtocol>>>,
    ) -> Self {
        let uuid = Self::load_or_create_uuid().unwrap_or_else(|_| Uuid::new_v4());
        Self {
            input: String::new(),
            input_mode: InputMode::Normal,
            communication: com,
            scroll: 0,
            max_scroll: 0,
            auto_scroll: true,
            character_index: 0,
            tx: transmit,
            username: "Client".to_string(),
            room: 0,
            uuid,
        }
    }
    fn load_or_create_uuid() -> Result<Uuid, Box<dyn std::error::Error>> {
        let config_dir = dirs::config_dir()
            .ok_or("Cannot find config directory")?
            .join("TuiTalk");

        fs::create_dir_all(&config_dir)?;

        let config_file = config_dir.join("uuid.cfg");

        if config_file.exists() {
            let content = fs::read_to_string(&config_file)?;
            if let Ok(uuid) = Uuid::parse_str(&content.trim()) {
                return Ok(uuid);
            }
        }

        let new_uuid = Uuid::new_v4();
        fs::write(&config_file, new_uuid.to_string())?;

        Ok(new_uuid)
    }

    fn move_cursor_left(&mut self) {
        let cursor_moved_left = self.character_index.saturating_sub(1);
        self.character_index = self.clamp_cursor(cursor_moved_left);
    }

    fn move_cursor_right(&mut self) {
        let cursor_moved_right = self.character_index.saturating_add(1);
        self.character_index = self.clamp_cursor(cursor_moved_right);
    }

    fn enter_char(&mut self, new_char: char) {
        let index = self.byte_index();
        self.input.insert(index, new_char);
        self.move_cursor_right();
    }

    fn byte_index(&self) -> usize {
        self.input
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.character_index)
            .unwrap_or(self.input.len())
    }

    fn delete_char(&mut self) {
        let is_not_cursor_leftmost = self.character_index != 0;
        if is_not_cursor_leftmost {
            let current_index = self.character_index;
            let from_left_to_current_index = current_index - 1;
            let before_char_to_delete = self.input.chars().take(from_left_to_current_index);
            let after_char_to_delete = self.input.chars().skip(current_index);

            self.input = before_char_to_delete.chain(after_char_to_delete).collect();
            self.move_cursor_left();
        }
    }

    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(0, self.input.chars().count())
    }

    fn reset_cursor(&mut self) {
        self.character_index = 0;
    }

    fn submit_message(&mut self) {
        let _ = command::parse(self);
        self.input.clear();
        self.reset_cursor();
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        let tick_rate = Duration::from_millis(100);
        let _ = command::join_initial_room(&mut self);
        loop {
            terminal.draw(|frame| self.draw(frame))?;

            let last_tick = Instant::now();
            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or(Duration::from_secs(0));

            if event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    match self.input_mode {
                        InputMode::Normal => match key.code {
                            KeyCode::Char('i') => {
                                self.input_mode = InputMode::Editing;
                            }
                            KeyCode::Char('q') => {
                                let _ = command::quit_app(&mut self);
                                return Ok(());
                            }
                            KeyCode::Char('g') => {
                                self.scroll = self.max_scroll;
                                self.auto_scroll = true;
                            }
                            KeyCode::Char('G') => {
                                self.auto_scroll = false;
                                self.scroll = 0;
                            }
                            KeyCode::Char('k') => {
                                if self.scroll < self.max_scroll {
                                    self.scroll += DEFAULT_SCROLL;
                                }
                                if self.scroll >= self.max_scroll {
                                    self.auto_scroll = true;
                                }
                            }
                            KeyCode::Char('K') => {
                                if self.max_scroll >= FAST_SCROLL
                                    && self.scroll < self.max_scroll - FAST_SCROLL
                                {
                                    self.scroll += FAST_SCROLL;
                                } else {
                                    self.scroll = self.max_scroll;
                                }
                                if self.scroll >= self.max_scroll {
                                    self.auto_scroll = true;
                                }
                            }
                            KeyCode::Char('j') => {
                                self.auto_scroll = false;
                                if self.scroll > 0 {
                                    self.scroll -= DEFAULT_SCROLL;
                                }
                            }
                            KeyCode::Char('J') => {
                                self.auto_scroll = false;
                                if self.scroll > FAST_SCROLL {
                                    self.scroll -= FAST_SCROLL;
                                } else {
                                    self.scroll = 0;
                                }
                            }
                            _ => {}
                        },
                        InputMode::Editing if key.kind == KeyEventKind::Press => match key.code {
                            KeyCode::Enter => self.submit_message(),
                            KeyCode::Char(to_insert) => self.enter_char(to_insert),
                            KeyCode::Backspace => self.delete_char(),
                            KeyCode::Left => self.move_cursor_left(),
                            KeyCode::Right => self.move_cursor_right(),
                            KeyCode::Esc => self.input_mode = InputMode::Normal,
                            _ => {}
                        },
                        InputMode::Editing => {}
                    }
                }
            }
        }
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        ui::draw(self, frame);
    }
}
