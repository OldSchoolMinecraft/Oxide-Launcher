#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod app;
mod archive;
mod chrome;
mod game;
mod github;
mod news;
mod profile;
mod releases;
mod settings;
mod ui;
mod updater;

use app::App;
use iced::window;
use settings::{DEFAULT_WINDOW_SIZE, MIN_WINDOW_SIZE, Paths, Settings};

fn main() -> iced::Result {
    if matches!(std::env::args().nth(1).as_deref(), Some("--version" | "-V")) {
        println!("oxide-launcher {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let (width, height) = Paths::new()
        .ok()
        .and_then(|paths| Settings::load(&paths.config).ok())
        .and_then(|settings| settings.launcher_size())
        .unwrap_or(DEFAULT_WINDOW_SIZE);
    iced::application(App::new, App::update, App::view)
        .title("Oxide Launcher")
        .default_font(chrome::FONT)
        .theme(App::theme)
        .subscription(App::subscription)
        .window(window::Settings {
            icon: Some(
                window::icon::from_rgba(
                    include_bytes!("../assets/icons/minecraft.rgba").to_vec(),
                    32,
                    32,
                )
                .expect("embedded 32x32 Minecraft icon"),
            ),
            #[cfg(target_os = "linux")]
            platform_specific: window::settings::PlatformSpecific {
                application_id: "dev.oxide.launcher".into(),
                ..Default::default()
            },
            size: iced::Size::new(width as f32, height as f32),
            min_size: Some(iced::Size::new(
                MIN_WINDOW_SIZE.0 as f32,
                MIN_WINDOW_SIZE.1 as f32,
            )),
            ..Default::default()
        })
        .run()
}
