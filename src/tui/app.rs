use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use grammers_client::{
    session::{PackedChat, Session},
    types::{Chat, Downloadable, LoginToken, PasswordToken},
    Client, Config, SignInError,
};
use ratatui::widgets::ListState;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

use crate::{config::Config as AppConfig, media, telegram};

// ── State machine ─────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq, Debug)]
pub enum Screen {
    Connecting,
    LoginPhone,
    LoginCode,
    LoginPassword,
    Chats,
    DownloadConfig,
    Downloading,
}

// ── Per-domain data ────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ChatEntry {
    pub id: i64,
    pub name: String,
    pub kind: ChatKind,
    pub packed: PackedChat,
}

#[derive(Clone, Debug)]
pub enum ChatKind {
    Private,
    Group,
    Channel,
}

impl ChatKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Private => "Private",
            Self::Group => "Group",
            Self::Channel => "Channel",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum MediaFilter {
    All,
    Photo,
    Video,
    Document,
    Audio,
}

impl MediaFilter {
    pub fn cycle(&self) -> Self {
        match self {
            Self::All => Self::Photo,
            Self::Photo => Self::Video,
            Self::Video => Self::Document,
            Self::Document => Self::Audio,
            Self::Audio => Self::All,
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Photo => "Photo",
            Self::Video => "Video",
            Self::Document => "Document",
            Self::Audio => "Audio",
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Photo => "photo",
            Self::Video => "video",
            Self::Document => "document",
            Self::Audio => "audio",
        }
    }
}

#[derive(Clone, Debug)]
pub struct DownloadEntry {
    pub filename: String,
    pub size: u64,
    pub downloaded: u64,
    pub state: DlState,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DlState {
    Pending,
    Active,
    Done,
    Failed(String),
}

impl DownloadEntry {
    pub fn ratio(&self) -> f64 {
        if self.size == 0 {
            if self.state == DlState::Done { 1.0 } else { 0.0 }
        } else {
            (self.downloaded as f64 / self.size as f64).min(1.0)
        }
    }
}

// ── Background events ──────────────────────────────────────────────────────────

pub enum AppEvent {
    Connected(Client, bool),
    ChatsLoaded(Vec<ChatEntry>),
    DlStarted { filename: String, size: u64 },
    DlProgress { filename: String, delta: u64 },
    DlDone { filename: String },
    DlFailed { filename: String, error: String },
    DlFinished,
    Error(String),
}

// ── App ────────────────────────────────────────────────────────────────────────

pub struct App {
    pub screen: Screen,
    pub should_quit: bool,
    pub status: Option<(String, bool)>, // (message, is_error)

    config: AppConfig,
    client: Option<Client>,

    // text input (reused across login screens)
    pub input: String,
    pub input_masked: bool,
    login_token: Option<LoginToken>,
    password_token: Option<PasswordToken>,

    // chat list
    pub chats: Vec<ChatEntry>,
    pub chats_loading: bool,
    pub list_state: ListState,
    pub filter: String,
    pub filter_active: bool,

    // download
    pub selected_chat: Option<ChatEntry>,
    pub media_filter: MediaFilter,
    pub output_dir: Option<PathBuf>,
    pub downloads: Vec<DownloadEntry>,
    pub dl_scroll: usize,
    pub dl_finished: bool,

    tx: mpsc::UnboundedSender<AppEvent>,
    pub rx: mpsc::UnboundedReceiver<AppEvent>,
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        App {
            screen: Screen::Connecting,
            should_quit: false,
            status: None,
            config,
            client: None,
            input: String::new(),
            input_masked: false,
            login_token: None,
            password_token: None,
            chats: Vec::new(),
            chats_loading: false,
            list_state: ListState::default(),
            filter: String::new(),
            filter_active: false,
            selected_chat: None,
            media_filter: MediaFilter::All,
            output_dir: None,
            downloads: Vec::new(),
            dl_scroll: 0,
            dl_finished: false,
            tx,
            rx,
        }
    }

    // ── Background launchers ───────────────────────────────────────────────────

    pub fn start_connect(&self) {
        let tx = self.tx.clone();
        let api_id = self.config.api_id;
        let api_hash = self.config.api_hash.clone();
        let session_path = AppConfig::session_path().unwrap();

        tokio::spawn(async move {
            let session = match Session::load_file_or_create(&session_path) {
                Ok(s) => s,
                Err(e) => {
                    tx.send(AppEvent::Error(e.to_string())).ok();
                    return;
                }
            };
            match Client::connect(Config {
                session,
                api_id,
                api_hash,
                params: Default::default(),
            })
            .await
            {
                Ok(client) => {
                    let authed = client.is_authorized().await.unwrap_or(false);
                    tx.send(AppEvent::Connected(client, authed)).ok();
                }
                Err(e) => {
                    tx.send(AppEvent::Error(e.to_string())).ok();
                }
            }
        });
    }

    fn start_load_chats(&self) {
        let tx = self.tx.clone();
        let client = self.client.clone().unwrap();

        tokio::spawn(async move {
            let mut entries = Vec::new();
            let mut dialogs = client.iter_dialogs();

            while let Ok(Some(dialog)) = dialogs.next().await {
                let chat = dialog.chat();
                entries.push(ChatEntry {
                    id: chat.id(),
                    name: chat.name().to_string(),
                    kind: match chat {
                        Chat::User(_) => ChatKind::Private,
                        Chat::Group(_) => ChatKind::Group,
                        Chat::Channel(_) => ChatKind::Channel,
                    },
                    packed: chat.pack(),
                });
            }

            tx.send(AppEvent::ChatsLoaded(entries)).ok();
        });
    }

    fn start_download(&self, packed: PackedChat, chat_name: String) {
        let tx = self.tx.clone();
        let client = self.client.clone().unwrap();
        let filter_str = self.media_filter.as_str().to_string();
        let output_dir = self.output_dir.clone().unwrap_or_else(|| {
            dirs::download_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("teldrop")
                .join(media::sanitize_name(&chat_name))
        });

        tokio::spawn(async move {
            let _ = std::fs::create_dir_all(&output_dir);
            let mut iter = client.iter_messages(packed);

            while let Ok(Some(message)) = iter.next().await {
                let Some(m) = message.media() else { continue };
                if !media::is_downloadable(&m) { continue; }
                if !media::matches_filter(&m, &filter_str) { continue; }

                let filename = media::media_filename(&m, &message);
                let size = media::media_size(&m);
                let path = output_dir.join(&filename);

                tx.send(AppEvent::DlStarted { filename: filename.clone(), size }).ok();

                let downloadable = Downloadable::Media(m);
                match tokio::fs::File::create(&path).await {
                    Ok(mut file) => {
                        let mut dl = client.iter_download(&downloadable);
                        let mut failed = false;

                        while let Ok(Some(chunk)) = dl.next().await {
                            if file.write_all(&chunk).await.is_err() {
                                tx.send(AppEvent::DlFailed {
                                    filename: filename.clone(),
                                    error: "write error".into(),
                                })
                                .ok();
                                failed = true;
                                break;
                            }
                            tx.send(AppEvent::DlProgress {
                                filename: filename.clone(),
                                delta: chunk.len() as u64,
                            })
                            .ok();
                        }

                        if !failed {
                            let _ = file.flush().await;
                            tx.send(AppEvent::DlDone { filename }).ok();
                        }
                    }
                    Err(e) => {
                        tx.send(AppEvent::DlFailed {
                            filename,
                            error: e.to_string(),
                        })
                        .ok();
                    }
                }
            }

            tx.send(AppEvent::DlFinished).ok();
        });
    }

    // ── Event processing (called each frame) ───────────────────────────────────

    pub fn process_events(&mut self) {
        while let Ok(ev) = self.rx.try_recv() {
            match ev {
                AppEvent::Connected(client, authed) => {
                    self.client = Some(client);
                    if authed {
                        self.screen = Screen::Chats;
                        self.chats_loading = true;
                        self.start_load_chats();
                    } else {
                        self.screen = Screen::LoginPhone;
                    }
                }
                AppEvent::ChatsLoaded(chats) => {
                    self.chats = chats;
                    self.chats_loading = false;
                    if !self.chats.is_empty() {
                        self.list_state.select(Some(0));
                    }
                }
                AppEvent::DlStarted { filename, size } => {
                    self.downloads.push(DownloadEntry {
                        filename,
                        size,
                        downloaded: 0,
                        state: DlState::Active,
                    });
                }
                AppEvent::DlProgress { filename, delta } => {
                    if let Some(e) = self.downloads.iter_mut().find(|e| e.filename == filename) {
                        e.downloaded += delta;
                    }
                }
                AppEvent::DlDone { filename } => {
                    if let Some(e) = self.downloads.iter_mut().find(|e| e.filename == filename) {
                        e.state = DlState::Done;
                        e.downloaded = e.size.max(e.downloaded);
                    }
                }
                AppEvent::DlFailed { filename, error } => {
                    if let Some(e) = self.downloads.iter_mut().find(|e| e.filename == filename) {
                        e.state = DlState::Failed(error);
                    }
                }
                AppEvent::DlFinished => {
                    self.dl_finished = true;
                    self.status = Some(("All downloads finished!".into(), false));
                }
                AppEvent::Error(msg) => {
                    self.status = Some((msg, true));
                }
            }
        }
    }

    // ── Key handling ───────────────────────────────────────────────────────────

    pub async fn handle_key(&mut self, key: KeyEvent) {
        // Ctrl+C always quits
        if key.code == KeyCode::Char('c')
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            self.should_quit = true;
            return;
        }

        match self.screen.clone() {
            Screen::Connecting => {}
            Screen::LoginPhone => self.key_login_phone(key).await,
            Screen::LoginCode => self.key_login_code(key).await,
            Screen::LoginPassword => self.key_login_password(key).await,
            Screen::Chats => self.key_chats(key),
            Screen::DownloadConfig => self.key_download_config(key),
            Screen::Downloading => self.key_downloading(key),
        }
    }

    // ── Login screens ──────────────────────────────────────────────────────────

    async fn key_login_phone(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.should_quit = true,
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => self.input.push(c),
            KeyCode::Enter if !self.input.is_empty() => {
                let phone = self.input.trim().to_string();
                self.status = Some(("Sending code…".into(), false));
                let client = self.client.clone().unwrap();
                match client.request_login_code(&phone).await {
                    Ok(token) => {
                        self.login_token = Some(token);
                        self.input.clear();
                        self.screen = Screen::LoginCode;
                        self.status = None;
                    }
                    Err(e) => self.status = Some((format!("Error: {e}"), true)),
                }
            }
            _ => {}
        }
    }

    async fn key_login_code(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.screen = Screen::LoginPhone;
                self.input.clear();
                self.login_token = None;
                self.status = None;
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => self.input.push(c),
            KeyCode::Enter if !self.input.is_empty() => {
                let code = self.input.trim().to_string();
                self.status = Some(("Verifying…".into(), false));
                let client = self.client.clone().unwrap();
                let token = self.login_token.as_ref().unwrap();

                match client.sign_in(token, &code).await {
                    Ok(_) => {
                        telegram::save_session(&client).ok();
                        self.input.clear();
                        self.screen = Screen::Chats;
                        self.chats_loading = true;
                        self.status = None;
                        self.start_load_chats();
                    }
                    Err(SignInError::PasswordRequired(pt)) => {
                        self.password_token = Some(pt);
                        self.input.clear();
                        self.input_masked = true;
                        self.screen = Screen::LoginPassword;
                        self.status = None;
                    }
                    Err(e) => self.status = Some((format!("Error: {e}"), true)),
                }
            }
            _ => {}
        }
    }

    async fn key_login_password(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.screen = Screen::LoginPhone;
                self.input.clear();
                self.input_masked = false;
                self.login_token = None;
                self.password_token = None;
                self.status = None;
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => self.input.push(c),
            KeyCode::Enter if !self.input.is_empty() => {
                let password = self.input.trim().to_string();
                self.status = Some(("Checking password…".into(), false));
                let client = self.client.clone().unwrap();
                let pt = self.password_token.take().unwrap();

                match client.check_password(pt, &password).await {
                    Ok(_) => {
                        telegram::save_session(&client).ok();
                        self.input.clear();
                        self.input_masked = false;
                        self.screen = Screen::Chats;
                        self.chats_loading = true;
                        self.status = None;
                        self.start_load_chats();
                    }
                    Err(e) => {
                        self.status = Some((format!("Wrong password: {e}"), true));
                        self.input.clear();
                        // PasswordToken was consumed — send back to phone screen
                        self.screen = Screen::LoginPhone;
                        self.input_masked = false;
                        self.login_token = None;
                    }
                }
            }
            _ => {}
        }
    }

    // ── Chat list ──────────────────────────────────────────────────────────────

    fn key_chats(&mut self, key: KeyEvent) {
        if self.filter_active {
            match key.code {
                KeyCode::Esc => {
                    self.filter_active = false;
                    self.filter.clear();
                    self.reset_list_selection();
                }
                KeyCode::Backspace => {
                    self.filter.pop();
                    self.reset_list_selection();
                }
                KeyCode::Char(c) => {
                    self.filter.push(c);
                    self.reset_list_selection();
                }
                KeyCode::Enter => self.filter_active = false,
                KeyCode::Up => self.list_up(),
                KeyCode::Down => self.list_down(),
                _ => {}
            }
        } else {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
                KeyCode::Char('/') => self.filter_active = true,
                KeyCode::Up => self.list_up(),
                KeyCode::Down => self.list_down(),
                KeyCode::Enter => {
                    if let Some(chat) = self.get_selected_chat().cloned() {
                        self.selected_chat = Some(chat);
                        self.screen = Screen::DownloadConfig;
                        self.downloads.clear();
                        self.dl_finished = false;
                        self.status = None;
                    }
                }
                _ => {}
            }
        }
    }

    fn list_up(&mut self) {
        let n = self.filtered_chats_count();
        if n == 0 { return; }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some(if i == 0 { n - 1 } else { i - 1 }));
    }

    fn list_down(&mut self) {
        let n = self.filtered_chats_count();
        if n == 0 { return; }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some((i + 1) % n));
    }

    fn reset_list_selection(&mut self) {
        if self.filtered_chats_count() > 0 {
            self.list_state.select(Some(0));
        } else {
            self.list_state.select(None);
        }
    }

    pub fn filtered_chats_count(&self) -> usize {
        self.filtered_chats().count()
    }

    pub fn filtered_chats(&self) -> impl Iterator<Item = &ChatEntry> {
        let f = self.filter.to_lowercase();
        self.chats.iter().filter(move |c| {
            f.is_empty() || c.name.to_lowercase().contains(&f)
        })
    }

    pub fn get_selected_chat(&self) -> Option<&ChatEntry> {
        let idx = self.list_state.selected()?;
        self.filtered_chats().nth(idx)
    }

    // ── Download config ────────────────────────────────────────────────────────

    fn key_download_config(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.screen = Screen::Chats;
                self.selected_chat = None;
            }
            KeyCode::Char('t') => {
                self.media_filter = self.media_filter.cycle();
            }
            KeyCode::Enter | KeyCode::Char('d') => {
                if let Some(chat) = self.selected_chat.clone() {
                    self.screen = Screen::Downloading;
                    self.status = None;
                    self.start_download(chat.packed, chat.name.clone());
                }
            }
            _ => {}
        }
    }

    // ── Downloading ────────────────────────────────────────────────────────────

    fn key_downloading(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') if self.dl_finished => {
                self.screen = Screen::Chats;
            }
            KeyCode::Up => {
                self.dl_scroll = self.dl_scroll.saturating_sub(1);
            }
            KeyCode::Down => {
                let max = self.downloads.len().saturating_sub(1);
                self.dl_scroll = (self.dl_scroll + 1).min(max);
            }
            _ => {}
        }
    }

    // ── Stats helpers ──────────────────────────────────────────────────────────

    pub fn dl_stats(&self) -> (usize, usize, usize, u64, u64) {
        let done = self.downloads.iter().filter(|e| e.state == DlState::Done).count();
        let active = self.downloads.iter().filter(|e| e.state == DlState::Active).count();
        let failed = self.downloads.iter().filter(|e| matches!(e.state, DlState::Failed(_))).count();
        let bytes_done: u64 = self.downloads.iter().map(|e| e.downloaded).sum();
        let bytes_total: u64 = self.downloads.iter().map(|e| e.size).sum();
        (done, active, failed, bytes_done, bytes_total)
    }
}
