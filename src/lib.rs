pub mod app;
pub mod error;
pub mod models;
pub mod sftp;
pub mod ssh;
pub mod storage;
pub mod ui;

pub fn run() -> iced::Result {
    app::run()
}
