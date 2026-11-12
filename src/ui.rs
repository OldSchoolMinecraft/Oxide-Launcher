//! Launcher views. Shared chrome lives in `chrome`; branding is a replaceable SVG.
use crate::{
    App, Field, Message, Page, chrome, landscape,
    settings::{Animation, Lighting, Visibility},
};
use iced::{
    Element, Fill, Font,
    widget::{
        Space, button, canvas, checkbox, column, container, pick_list, row, scrollable, stack, svg,
        text, text_input,
    },
};

const LOGO: &[u8] = include_bytes!("../assets/logo-placeholder.svg");

impl App {
    pub(super) fn view(&self) -> Element<'_, Message> {
        let theme = self.theme();
        let muted = chrome::muted(&theme);
        let logo = row![
            svg(svg::Handle::from_memory(LOGO)).width(78).height(78),
            column![
                text("RUST173").size(38).font(Font {
                    weight: iced::font::Weight::Bold,
                    ..Font::MONOSPACE
                }),
                text("Minecraft Beta 1.7.3  /  Rust client")
                    .size(12)
                    .color(muted),
            ]
            .spacing(4)
        ]
        .spacing(16)
        .align_y(iced::Center);
        let masthead = stack![
            canvas::Canvas::new(landscape::Landscape)
                .width(Fill)
                .height(148),
            container(logo)
                .width(425)
                .height(148)
                .padding([28, 26])
                .style(chrome::heading),
        ];
        let toolbar = container(
            row![
                button("  Play  ")
                    .padding([7, 22])
                    .style(move |t, s| chrome::tab(t, s, self.page == Page::Play))
                    .on_press(Message::Page(Page::Play)),
                button("Settings")
                    .padding([7, 22])
                    .style(move |t, s| chrome::tab(t, s, self.page == Page::Settings))
                    .on_press(Message::Page(Page::Settings)),
                Space::new().width(Fill),
                button("Project website ↗")
                    .padding([7, 14])
                    .style(chrome::control)
                    .on_press(Message::OpenProject),
            ]
            .spacing(6)
            .align_y(iced::Center),
        )
        .padding(7)
        .style(chrome::heading);
        let content = match self.page {
            Page::Play => self.play_view(),
            Page::Settings => self.settings_view(),
        };
        let mut body = column![
            masthead,
            toolbar,
            container(content).padding(12).height(Fill)
        ]
        .spacing(1);
        if let Some(error) = &self.save_error {
            body = body.push(
                container(text(error).size(12).color(theme.palette().danger)).padding([6, 12]),
            );
        }
        let footer = container(
            row![
                text("Oxide Launcher  0.1.0").size(11),
                Space::new().width(Fill),
                text(if self.dirty.is_some() {
                    "Saving settings…"
                } else if self.save_error.is_some() {
                    "Settings not saved"
                } else {
                    "Settings saved"
                })
                .size(11),
            ]
            .spacing(12),
        )
        .padding([7, 12])
        .style(chrome::heading);
        body = body.push(footer);
        container(
            container(body)
                .padding(5)
                .width(Fill)
                .height(Fill)
                .style(chrome::frame),
        )
        .padding(20)
        .width(Fill)
        .height(Fill)
        .style(chrome::desktop)
        .into()
    }

    fn play_view(&self) -> Element<'_, Message> {
        let theme = self.theme();
        let muted = chrome::muted(&theme);
        let version = if !self.settings.executable.trim().is_empty() {
            "Local executable".into()
        } else {
            self.installed
                .as_ref()
                .map(|i| i.tag.clone())
                .unwrap_or("Not installed".into())
        };
        let title = if self.child.is_some() {
            "Game running"
        } else if self.busy {
            "Please wait…"
        } else if self.installed.is_none() && self.settings.executable.trim().is_empty() {
            "Install and play"
        } else {
            "Launch game"
        };
        let enabled = !self.busy
            && self.child.is_none()
            && (self.installed.is_some()
                || !self.settings.executable.trim().is_empty()
                || self.release.is_some());
        let launch = button(text(title).size(19).font(Font {
            weight: iced::font::Weight::Bold,
            ..Font::DEFAULT
        }))
        .padding([13, 18])
        .width(Fill)
        .style(chrome::launch)
        .on_press_maybe(enabled.then_some(Message::Play));
        let player = column![
            text("Username").size(13),
            text_input("Player name", &self.settings.username)
                .on_input(|v| Message::Edit(Field::Username, v))
                .padding(10)
                .size(17)
                .style(chrome::input),
            text("Offline player name; no account required.")
                .size(11)
                .color(muted),
            Space::new().height(3),
            row![
                text("Version").size(12).color(muted).width(Fill),
                text(version).size(12)
            ]
            .spacing(8),
            row![
                text("Window").size(12).color(muted).width(Fill),
                text(format!(
                    "{} × {}",
                    self.settings.width, self.settings.height
                ))
                .size(12)
            ],
            Space::new().height(5),
            launch,
        ]
        .spacing(9);
        let connection = column![
            row![
                text("Server").size(12).color(muted).width(Fill),
                text(if self.settings.server.trim().is_empty() {
                    "Not set"
                } else {
                    self.settings.server.trim()
                })
                .size(12),
            ]
            .spacing(8),
            button(text("Edit launch options…").size(13))
                .padding([7, 10])
                .width(Fill)
                .style(chrome::control)
                .on_press(Message::Page(Page::Settings)),
        ]
        .spacing(12);
        let sidebar = column![
            panel("Player", player.into()),
            panel("Connection", connection.into())
        ]
        .spacing(12)
        .width(iced::Length::FillPortion(4));
        let mut actions = row![
            button("Check for updates")
                .padding([7, 12])
                .style(chrome::control)
                .on_press_maybe((!self.busy).then_some(Message::Check))
        ]
        .spacing(8);
        if self
            .release
            .as_ref()
            .is_some_and(|r| self.installed.as_ref().is_none_or(|i| i.tag != r.tag_name))
        {
            actions = actions.push(
                button("Install latest")
                    .padding([7, 12])
                    .style(chrome::control)
                    .on_press_maybe(
                        (!self.busy && self.child.is_none()).then_some(Message::Install(false)),
                    ),
            );
        }
        let release_title = self
            .release
            .as_ref()
            .map(|r| format!("Oxide {}", r.tag_name))
            .unwrap_or("GitHub releases".into());
        let mut release = column![
            text(release_title).size(22),
            text(&self.status).size(13).color(muted),
            actions
        ]
        .spacing(14);
        if let Some(error) = &self.error {
            release = release.push(
                container(text(error).size(13).color(theme.palette().danger))
                    .padding(12)
                    .width(Fill)
                    .style(chrome::inset),
            );
        }
        release = release.push(text("Release notes").size(13)).push(
            container(
                scrollable(
                    text(
                        self.release
                            .as_ref()
                            .and_then(|r| r.body.as_deref())
                            .filter(|b| !b.trim().is_empty())
                            .unwrap_or("No release notes available."),
                    )
                    .size(13),
                )
                .spacing(8)
                .height(150),
            )
            .padding(12)
            .width(Fill)
            .style(chrome::inset),
        );
        let updates = panel(
            "Release information",
            scrollable(release).spacing(10).height(Fill).into(),
        )
        .width(iced::Length::FillPortion(6))
        .height(Fill);
        scrollable(
            column![
                row![updates, sidebar].spacing(12).height(430),
                container(
                    row![
                        text(if self.child.is_some() {
                            "■  Running"
                        } else if self.busy {
                            "■  Working"
                        } else {
                            "■  Idle"
                        })
                        .size(11),
                        Space::new().width(Fill),
                        text("Minecraft Beta 1.7.3").size(11)
                    ]
                    .spacing(8)
                )
                .padding([8, 10])
                .style(chrome::inset)
            ]
            .spacing(12),
        )
        .spacing(10)
        .height(Fill)
        .into()
    }

    fn settings_view(&self) -> Element<'_, Message> {
        let muted = chrome::muted(&self.theme());
        let appearance = column![
            checkbox(self.settings.dark_mode)
                .label("Dark mode")
                .on_toggle(Message::DarkMode),
            text("Use the dark appearance for the launcher.")
                .size(12)
                .color(muted),
        ]
        .spacing(8);
        let launcher = column![
            row![
                text("When the game starts").size(13).width(Fill),
                pick_list(
                    Visibility::ALL,
                    Some(self.settings.visibility),
                    Message::Visibility
                )
            ]
            .align_y(iced::Center)
            .spacing(12),
            checkbox(self.settings.auto_update)
                .label("Automatically install new GitHub releases")
                .on_toggle(Message::AutoUpdate),
            text("Checks on startup. Installs updates while the game is closed.")
                .size(12)
                .color(muted),
        ]
        .spacing(12);
        let resources = column![
            input(
                "Server address",
                "Optional · 127.0.0.1:25565",
                &self.settings.server,
                Field::Server
            ),
            input(
                "Singleplayer server JAR",
                "Optional · absolute path to a Beta 1.7.3 server JAR",
                &self.settings.singleplayer_server,
                Field::Singleplayer
            ),
            input(
                "Texture assets",
                "Optional · directory, JAR, ZIP, or terrain.png",
                &self.settings.assets,
                Field::Assets
            ),
            input(
                "Sounds & music",
                "Optional · absolute path to legacy resources",
                &self.settings.sounds,
                Field::Sounds
            ),
        ]
        .spacing(12);
        let game = column![
            row![
                input("Window width", "1280", &self.settings.width, Field::Width),
                input("Window height", "800", &self.settings.height, Field::Height)
            ]
            .spacing(16),
            row![
                column![
                    text("Smooth lighting").size(12),
                    pick_list(
                        Lighting::ALL,
                        Some(self.settings.lighting),
                        Message::Lighting
                    )
                ]
                .spacing(6)
                .width(Fill),
                column![
                    text("First-person animations").size(12),
                    pick_list(
                        Animation::ALL,
                        Some(self.settings.animation),
                        Message::Animation
                    )
                ]
                .spacing(6)
                .width(Fill)
            ]
            .spacing(16),
            checkbox(self.settings.mute)
                .label("Mute game audio")
                .on_toggle(Message::Mute),
        ]
        .spacing(14);
        let local = column![
            input(
                "Game executable override",
                "Optional · absolute path to rust173 or hydra-client",
                &self.settings.executable,
                Field::Executable
            ),
            text("Uses a local build. Resource paths should be absolute.")
                .size(12)
                .color(muted),
            text(
                self.paths
                    .as_ref()
                    .map(|p| format!("Game data & logs: {}", p.data.display()))
                    .unwrap_or_else(|e| e.clone())
            )
            .size(11)
            .color(muted),
        ]
        .spacing(10);
        scrollable(
            column![
                panel("Appearance", appearance.into()),
                panel("Launcher", launcher.into()),
                panel("Connection & resources", resources.into()),
                panel("Game window & audio", game.into()),
                panel("Local installation", local.into())
            ]
            .spacing(12),
        )
        .spacing(12)
        .height(Fill)
        .into()
    }
}

fn panel<'a>(
    title: &'a str,
    content: Element<'a, Message>,
) -> iced::widget::Container<'a, Message> {
    container(column![
        container(text(title).size(13).font(Font {
            weight: iced::font::Weight::Bold,
            ..Font::DEFAULT
        }))
        .padding([8, 12])
        .width(Fill)
        .style(chrome::heading),
        container(content).padding(14).width(Fill),
    ])
    .width(Fill)
    .height(iced::Length::Shrink)
    .style(chrome::panel)
}
fn input<'a>(
    label: &'a str,
    placeholder: &'a str,
    value: &'a str,
    field: Field,
) -> Element<'a, Message> {
    column![
        text(label).size(12),
        text_input(placeholder, value)
            .on_input(move |v| Message::Edit(field, v))
            .padding(9)
            .size(14)
            .style(chrome::input)
    ]
    .spacing(6)
    .width(Fill)
    .into()
}
