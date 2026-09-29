//! Renders the README screenshots from demo data.
//!
//! ```text
//! cargo run --release --example screenshots -- docs/screenshots
//! ```
//!
//! The real views and update logic are driven with an offline session, so no
//! server is needed and the output is reproducible. A window appears for a
//! few seconds while the scenes are captured; no input is simulated.

use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use chrono::{TimeZone, Utc};
use crossbeam_channel::{Receiver, unbounded};
use iced::widget::text_editor;
use iced::window::{self, Screenshot};
use iced::{Element, Size, Subscription, Task};

use rust_ssh_client::app::messages::Message;
use rust_ssh_client::app::state::{AppState, Modal, Route};
use rust_ssh_client::app::{update, view};
use rust_ssh_client::models::{
    AuthType, FileEntry, FileKind, HostRecord, LoginRequest, SaveLifetime, SshKeyRecord,
    TransferDirection, TransferProgress, TransferStatus, WorkspaceTab,
};
use rust_ssh_client::ssh::client::HostKeyInfo;
use rust_ssh_client::ssh::session::{SessionCommand, SessionEvent, SessionHandle};
use rust_ssh_client::storage::StorageFacade;
use rust_ssh_client::ui::theme;

const SCENES: [&str; 5] = ["login", "host-key", "terminal", "editor", "explorer-menu"];
const PROJECT: &str = "/home/deploy/orders-api";

fn main() -> iced::Result {
    let output = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("docs/screenshots"));
    std::fs::create_dir_all(&output).expect("create output directory");

    let mut application =
        iced::application(move || Tour::new(output.clone()), Tour::update, Tour::view)
            .subscription(Tour::subscription)
            .theme(|_: &Tour| theme::app_theme())
            .title(|_: &Tour| "RustSSH".to_string())
            .default_font(theme::UI_FONT)
            .window(window::Settings {
                size: Size::new(1280.0, 800.0),
                resizable: false,
                ..window::Settings::default()
            })
            .antialiasing(true);

    for font in theme::FONTS {
        application = application.font(font);
    }

    application.run()
}

#[derive(Debug, Clone)]
enum TourMessage {
    App(Message),
    Setup,
    Settled,
    Freeze,
    Capture,
    Captured(Screenshot),
}

struct Tour {
    app: AppState,
    scene: usize,
    output: PathBuf,
    // Keeps the offline session's command channel open.
    _commands: Receiver<SessionCommand>,
    _vault: tempfile::TempDir,
}

impl Tour {
    fn new(output: PathBuf) -> (Self, Task<TourMessage>) {
        let vault = tempfile::tempdir().expect("temporary vault");
        let (command_tx, command_rx) = unbounded();
        let (session, _events) = SessionHandle::from_channels(command_tx);

        let mut app = AppState::with_storage(StorageFacade::for_root(vault.path().to_path_buf()));
        app.hosts = demo_hosts();
        app.keys = vec![demo_key()];
        app.workspace.session = Some(session);

        let tour = Self {
            app,
            scene: 0,
            output,
            _commands: command_rx,
            _vault: vault,
        };
        (tour, delay(1200, TourMessage::Setup))
    }

    fn update(&mut self, message: TourMessage) -> Task<TourMessage> {
        match message {
            TourMessage::App(message) => {
                update::update(&mut self.app, message).map(TourMessage::App)
            }
            TourMessage::Setup => {
                let task = self.setup_scene();
                Task::batch([task, delay(700, TourMessage::Settled)])
            }
            TourMessage::Settled => {
                let task = self.settle_scene();
                Task::batch([task, delay(500, TourMessage::Freeze)])
            }
            TourMessage::Freeze => {
                // Pin the blinking cursor in its visible phase and drop stray
                // toasts. A frame must render before capturing, because the
                // screenshot replays the last rendered frame.
                self.app.workspace.terminal_cursor_visible = true;
                self.app.workspace.last_terminal_activity = Instant::now();
                let keep_toasts = SCENES[self.scene] == "editor";
                self.app.notifications.retain(|_| keep_toasts);
                delay(400, TourMessage::Capture)
            }
            TourMessage::Capture => window::latest()
                .and_then(window::screenshot)
                .map(TourMessage::Captured),
            TourMessage::Captured(screenshot) => {
                let path = self.output.join(format!("{}.png", SCENES[self.scene]));
                save_png(&screenshot, &path);
                println!("wrote {}", path.display());

                self.scene += 1;
                if self.scene == SCENES.len() {
                    iced::exit()
                } else {
                    Task::done(TourMessage::Setup)
                }
            }
        }
    }

    fn view(&self) -> Element<'_, TourMessage> {
        view::view(&self.app).map(TourMessage::App)
    }

    fn subscription(&self) -> Subscription<TourMessage> {
        update::subscription(&self.app).map(TourMessage::App)
    }

    fn send(&mut self, event: SessionEvent) -> Task<TourMessage> {
        let id = self.app.workspace.session.as_ref().expect("session").id();
        update::update(&mut self.app, Message::Session { id, event }).map(TourMessage::App)
    }

    fn setup_scene(&mut self) -> Task<TourMessage> {
        match SCENES[self.scene] {
            "login" => {
                let id = self.app.hosts[0].id;
                update::update(&mut self.app, Message::HostSelected(id)).map(TourMessage::App)
            }
            "host-key" => {
                let id = self.app.hosts[1].id;
                let task = update::update(&mut self.app, Message::HostSelected(id));
                self.app.login.connecting = true;
                self.app.modal = Some(Modal::HostKey(HostKeyInfo {
                    host: "staging.example.com".into(),
                    port: 22,
                    key_type: "ED25519".into(),
                    fingerprint: "SHA256:q3Vd8mTzR1kXo9pF2LwY7bNcHs4Ue6AjGiK0tQvZ5Ex".into(),
                }));
                task.map(TourMessage::App)
            }
            "terminal" => {
                self.app.modal = None;
                self.app.login.connecting = false;
                let id = self.app.hosts[0].id;
                let _ = update::update(&mut self.app, Message::HostSelected(id));
                self.app.route = Route::Workspace;

                let mut tasks = vec![
                    self.send(SessionEvent::Connected {
                        cwd: PROJECT.into(),
                        latency_ms: 38,
                        peer: "api.example.com".into(),
                    }),
                    self.send(SessionEvent::DirectoryLoaded {
                        request: PROJECT.into(),
                        cwd: PROJECT.into(),
                        entries: project_entries(),
                    }),
                ];
                tasks.push(
                    update::update(
                        &mut self.app,
                        Message::ExplorerEntryPressed(format!("{PROJECT}/src")),
                    )
                    .map(TourMessage::App),
                );
                tasks.push(self.send(SessionEvent::DirectoryChildrenLoaded {
                    directory: format!("{PROJECT}/src"),
                    entries: src_entries(),
                }));
                tasks.extend(
                    demo_transfers()
                        .into_iter()
                        .map(|t| self.send(SessionEvent::Transfer(t))),
                );
                self.app.workspace.selected_file = Some(format!("{PROJECT}/src/main.rs"));
                Task::batch(tasks)
            }
            "editor" => {
                let path = format!("{PROJECT}/src/main.rs");
                self.app
                    .workspace
                    .open_editor_tab(format!("{PROJECT}/Cargo.toml"));
                self.app
                    .workspace
                    .apply_editor_content(&format!("{PROJECT}/Cargo.toml"), CARGO_TOML.into());
                self.app.workspace.open_editor_tab(path.clone());
                self.app
                    .workspace
                    .apply_editor_content(&path, MAIN_RS.into());
                for action in [
                    text_editor::Action::Move(text_editor::Motion::Down),
                    text_editor::Action::Move(text_editor::Motion::Down),
                    text_editor::Action::Move(text_editor::Motion::End),
                    text_editor::Action::Edit(text_editor::Edit::Enter),
                    text_editor::Action::Edit(text_editor::Edit::Paste(
                        "use tower_http::trace::TraceLayer;".to_string().into(),
                    )),
                ] {
                    self.app.workspace.apply_editor_action(&path, action);
                }
                self.app.workspace.active_tab = WorkspaceTab::Editor(path);
                self.app.notifications.clear();
                self.app.notification(
                    rust_ssh_client::app::state::NotificationLevel::Success,
                    "Saved Cargo.toml",
                );
                Task::none()
            }
            "explorer-menu" => {
                self.app.workspace.active_tab = WorkspaceTab::Terminal;
                let path = format!("{PROJECT}/docker-compose.yml");
                update::update(&mut self.app, Message::ExplorerEntrySecondaryPressed(path))
                    .map(TourMessage::App)
            }
            _ => Task::none(),
        }
    }

    /// Runs after the first layout, once the terminal knows its real size.
    fn settle_scene(&mut self) -> Task<TourMessage> {
        if SCENES[self.scene] == "terminal" {
            self.send(SessionEvent::Output(terminal_session().into_bytes()))
        } else {
            Task::none()
        }
    }
}

fn delay(millis: u64, message: TourMessage) -> Task<TourMessage> {
    Task::perform(
        async move { std::thread::sleep(Duration::from_millis(millis)) },
        move |_| message.clone(),
    )
}

fn save_png(screenshot: &Screenshot, path: &PathBuf) {
    let file = BufWriter::new(File::create(path).expect("create screenshot"));
    let mut encoder = png::Encoder::new(file, screenshot.size.width, screenshot.size.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("png header");
    writer.write_image_data(&screenshot.rgba).expect("png data");
}

// ---------------------------------------------------------------------------
// Demo data
// ---------------------------------------------------------------------------

fn demo_key() -> SshKeyRecord {
    let mut key = SshKeyRecord::new(
        "deploy_ed25519",
        "-----BEGIN OPENSSH PRIVATE KEY-----\ndemo\n-----END OPENSSH PRIVATE KEY-----\n",
    );
    key.created_at = Utc.with_ymd_and_hms(2026, 3, 14, 9, 0, 0).unwrap();
    key
}

fn demo_hosts() -> Vec<HostRecord> {
    let key = demo_key();
    let hosts = [
        (
            "Production API",
            "api.example.com",
            22,
            "deploy",
            AuthType::Key,
        ),
        (
            "Staging web",
            "staging.example.com",
            22,
            "deploy",
            AuthType::Key,
        ),
        (
            "Analytics DB",
            "10.0.4.12",
            22,
            "postgres",
            AuthType::Password,
        ),
        (
            "Build runner",
            "ci-runner-02.example.com",
            2222,
            "ci",
            AuthType::Key,
        ),
        (
            "Bastion",
            "bastion.example.com",
            22,
            "ops",
            AuthType::Password,
        ),
    ];

    hosts
        .into_iter()
        .enumerate()
        .map(|(index, (label, host, port, username, auth_type))| {
            let request = LoginRequest {
                label: Some(label.into()),
                host: host.into(),
                port,
                username: username.into(),
                password: (auth_type == AuthType::Password).then(|| "demo-password".into()),
                auth_type,
                key_reference: (auth_type == AuthType::Key).then_some(key.id),
                save_host: true,
                save_lifetime: SaveLifetime::Forever,
            };
            let mut record = HostRecord::new(&request);
            record.updated_at = Utc::now() - chrono::Duration::hours(index as i64 * 7);
            record
        })
        .collect()
}

fn entry(name: &str, parent: &str, kind: FileKind, size: u64) -> FileEntry {
    FileEntry {
        name: name.into(),
        path: format!("{parent}/{name}"),
        permissions: if kind == FileKind::Directory {
            "drwxr-xr-x".into()
        } else {
            "-rw-r--r--".into()
        },
        kind,
        is_symlink: false,
        size,
        owner: Some("1001".into()),
        modified: Some(Utc.with_ymd_and_hms(2026, 9, 29, 9, 12, 0).unwrap()),
    }
}

fn project_entries() -> Vec<FileEntry> {
    use FileKind::{Directory, File};
    vec![
        entry(".github", PROJECT, Directory, 0),
        entry("config", PROJECT, Directory, 0),
        entry("migrations", PROJECT, Directory, 0),
        entry("src", PROJECT, Directory, 0),
        entry(".env.example", PROJECT, File, 412),
        entry("Cargo.lock", PROJECT, File, 81_233),
        entry("Cargo.toml", PROJECT, File, 1_104),
        entry("Dockerfile", PROJECT, File, 689),
        entry("README.md", PROJECT, File, 3_870),
        entry("docker-compose.yml", PROJECT, File, 1_212),
        entry("deploy.sh", PROJECT, File, 947),
    ]
}

fn src_entries() -> Vec<FileEntry> {
    let src = format!("{PROJECT}/src");
    vec![
        entry("handlers", &src, FileKind::Directory, 0),
        entry("config.rs", &src, FileKind::File, 2_310),
        entry("db.rs", &src, FileKind::File, 4_502),
        entry("main.rs", &src, FileKind::File, 1_781),
        entry("routes.rs", &src, FileKind::File, 3_095),
    ]
}

fn demo_transfers() -> Vec<TransferProgress> {
    let mut done = TransferProgress::queued(
        "release-2026.09.tar.gz",
        TransferDirection::Download,
        48_300_000,
    );
    done.transferred_bytes = done.total_bytes;
    done.status = TransferStatus::Completed;

    let mut running =
        TransferProgress::queued("orders-api (x86_64)", TransferDirection::Upload, 18_600_000);
    running.transferred_bytes = 11_900_000;
    running.status = TransferStatus::Running;

    vec![done, running]
}

fn terminal_session() -> String {
    let prompt = "\x1b[1;32mdeploy@api-01\x1b[0m:\x1b[1;34m~/orders-api\x1b[0m$ ";
    let lines = [
        format!("{prompt}git log --oneline -6"),
        "\x1b[33m9f3c2ab\x1b[0m (\x1b[1;36mHEAD -> \x1b[1;32mmain\x1b[0m, \x1b[1;31morigin/main\x1b[0m) Add request rate limiting".into(),
        "\x1b[33m41d7e0c\x1b[0m Cache session lookups in Redis".into(),
        "\x1b[33mc28a915\x1b[0m Upgrade to tokio 1.47".into(),
        "\x1b[33m7be4f10\x1b[0m Fix pagination off-by-one in /v1/orders".into(),
        "\x1b[33m1a0d3e2\x1b[0m Structured JSON logging".into(),
        "\x1b[33me6c91b4\x1b[0m (\x1b[1;33mtag: v1.0.0\x1b[0m) Initial release".into(),
        format!("{prompt}systemctl status orders-api --no-pager"),
        "\x1b[1;32m\u{25cf}\x1b[0m orders-api.service - Orders API".into(),
        "     Loaded: loaded (/etc/systemd/system/orders-api.service; \x1b[1;32menabled\x1b[0m; preset: \x1b[1;32menabled\x1b[0m)".into(),
        "     Active: \x1b[1;32mactive (running)\x1b[0m since Mon 2026-09-28 08:14:02 UTC; 1 day 6h ago".into(),
        "   Main PID: 48213 (orders-api)".into(),
        "      Tasks: 18 (limit: 9387)".into(),
        "     Memory: 42.6M (peak: 61.3M)".into(),
        "        CPU: 12min 4.218s".into(),
        format!("{prompt}ls -l"),
        "total 120".into(),
        "-rw-r--r-- 1 deploy deploy 81233 Sep 29 09:12 Cargo.lock".into(),
        "-rw-r--r-- 1 deploy deploy  1104 Sep 29 09:12 Cargo.toml".into(),
        "-rw-r--r-- 1 deploy deploy   689 Sep 29 09:12 Dockerfile".into(),
        "-rw-r--r-- 1 deploy deploy  3870 Sep 29 09:12 README.md".into(),
        "drwxr-xr-x 2 deploy deploy  4096 Sep 29 09:12 \x1b[1;34mconfig\x1b[0m".into(),
        "-rwxr-xr-x 1 deploy deploy   947 Sep 29 09:12 \x1b[1;32mdeploy.sh\x1b[0m".into(),
        "-rw-r--r-- 1 deploy deploy  1212 Sep 29 09:12 docker-compose.yml".into(),
        "drwxr-xr-x 2 deploy deploy  4096 Sep 29 09:12 \x1b[1;34mmigrations\x1b[0m".into(),
        "drwxr-xr-x 3 deploy deploy  4096 Sep 29 09:12 \x1b[1;34msrc\x1b[0m".into(),
        format!("{prompt}df -h /"),
        "Filesystem      Size  Used Avail Use% Mounted on".into(),
        "/dev/nvme0n1p1   80G   31G   49G  39% /".into(),
        prompt.to_string(),
    ];
    lines.join("\r\n")
}

const CARGO_TOML: &str = r#"[package]
name = "orders-api"
version = "1.4.2"
edition = "2024"
"#;

const MAIN_RS: &str = r#"use std::net::SocketAddr;

use axum::{Router, routing::get};
use tokio::net::TcpListener;
use tracing::info;

mod config;
mod db;
mod handlers;
mod routes;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().json().init();

    let config = config::Config::from_env()?;
    let pool = db::connect(&config.database_url).await?;

    let app = Router::new()
        .route("/health", get(handlers::health))
        .nest("/v1", routes::v1(pool.clone()))
        .with_state(pool);

    let address = SocketAddr::from(([0, 0, 0, 0], config.port));
    let listener = TcpListener::bind(address).await?;
    info!(%address, "orders-api listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("install Ctrl+C handler");
    info!("shutting down");
}
"#;
