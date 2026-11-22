//! Classic launcher proportions with native Rust controls and offline access.
use crate::{
    App, Field, Message, Page, chrome, landscape,
    settings::{Lighting, Visibility},
};
use iced::{
    Element, Fill, Font,
    widget::{
        button, canvas, checkbox, column, container, markdown, pick_list, row, scrollable, stack,
        text, text_input,
    },
};

pub(super) fn parse_release_notes(body: Option<&str>) -> Vec<markdown::Item> {
    markdown::parse(
        body.filter(|body| !body.trim().is_empty())
            .unwrap_or("No release notes available."),
    )
    .collect()
}

impl App {
    pub(super) fn view(&self) -> Element<'_, Message> {
        // LauncherTabPanel: compact system tabs above the content, not a toolbar.
        let mut tab_row = row![
            self.page_tab("Update Notes", Page::Play),
            self.page_tab("Launcher Log", Page::Log),
        ];
        if self.child.is_some() {
            tab_row = tab_row.push(self.page_tab("Game Log", Page::GameLog));
        }
        tab_row = tab_row.push(self.page_tab("Profile Editor", Page::Settings));
        let tabs = container(tab_row.spacing(2).align_y(iced::Bottom))
            .padding(iced::Padding {
                top: 2.,
                right: 4.,
                bottom: 0.,
                left: 4.,
            })
            .width(Fill)
            .style(chrome::shell);
        let content = match self.page {
            Page::Play => self.play_view(),
            Page::Log => self.log_view(),
            Page::GameLog => self.game_log_view(),
            Page::Settings => container(self.settings_view())
                .padding(10)
                .height(Fill)
                .into(),
        };
        let content = container(content)
            .width(Fill)
            .height(Fill)
            .style(chrome::frame);
        let mut body = column![tabs, content, self.launch_bar()].spacing(0);
        if let Some(error) = &self.save_error {
            body = body.push(
                container(text(error).size(12).color(landscape::color(0xa00000)))
                    .padding([2, 6])
                    .width(Fill)
                    .style(chrome::shell),
            );
        }
        // LauncherPanel: the thin status/progress area lives below the bottom bar.
        body = body.push(
            container(
                row![
                    column![
                        text(&self.status).size(11),
                        text(&self.launcher_update_status).size(11)
                    ]
                    .width(Fill),
                    text(if self.dirty.is_some() {
                        "Saving settings…"
                    } else if self.save_error.is_some() {
                        "Settings not saved"
                    } else {
                        "Settings saved"
                    })
                    .size(11),
                ]
                .spacing(8),
            )
            .padding([2, 6])
            .width(Fill)
            .style(chrome::shell),
        );
        container(body)
            .width(Fill)
            .height(Fill)
            .style(chrome::desktop)
            .into()
    }

    fn page_tab<'a>(&self, label: &'a str, page: Page) -> Element<'a, Message> {
        let active = self.page == page;
        button(text(label).size(12))
            .padding([4, 9])
            .style(move |t, s| chrome::tab(t, s, active))
            .on_press(Message::Page(page))
            .into()
    }

    fn launch_bar(&self) -> Element<'_, Message> {
        let version = if !self.settings.executable.trim().is_empty() {
            "local build"
        } else {
            self.installed
                .as_ref()
                .map(|i| i.tag.as_str())
                .unwrap_or("not installed")
        };
        let title = if self.child.is_some() {
            "Game running"
        } else if self.busy || self.launcher_updating {
            "Please wait…"
        } else if self.installed.is_none() && self.settings.executable.trim().is_empty() {
            "Install & Play"
        } else {
            "Play"
        };
        let enabled = !self.launcher_updating
            && !self.busy
            && self.child.is_none()
            && (self.installed.is_some()
                || !self.settings.executable.trim().is_empty()
                || self.release.is_some());
        let player = column![
            row![
                text("Username:").size(12),
                text_input("Player name", &self.settings.username)
                    .on_input(|v| Message::Edit(Field::Username, v))
                    .padding([5, 6])
                    .size(12)
                    .style(chrome::player_input)
            ]
            .spacing(6)
            .align_y(iced::Center),
            row![
                button(container(text("Edit Profile").size(12)).center_x(Fill))
                    .width(Fill)
                    .padding([5, 6])
                    .style(chrome::control)
                    .on_press(Message::Page(Page::Settings)),
                button(container(text("Check for updates").size(12)).center_x(Fill))
                    .width(Fill)
                    .padding([5, 6])
                    .style(chrome::control)
                    .on_press_maybe((!self.busy).then_some(Message::Check)),
            ]
            .spacing(6)
        ]
        .spacing(6);
        let play = button(
            container(text(title).size(16).font(Font {
                weight: iced::font::Weight::Bold,
                ..Font::DEFAULT
            }))
            .center_x(Fill)
            .center_y(Fill),
        )
        .width(Fill)
        .height(54)
        .padding(6)
        .style(chrome::launch)
        .on_press_maybe(enabled.then_some(Message::Play));
        let account = column![
            text(format!("Welcome, {}", self.settings.username)).size(12),
            text("Minecraft Beta 1.7.3").size(11),
            text(format!("Client version: {}", version)).size(11),
        ]
        .spacing(4)
        .width(Fill)
        .align_x(iced::Right);
        // Three balanced columns with shared vertical alignment and outer gutters.
        container(
            row![
                container(player).width(Fill).center_y(Fill),
                container(play).width(Fill).center_y(Fill),
                container(account).width(Fill).center_y(Fill),
            ]
            .spacing(24),
        )
        .padding([12, 14])
        .height(88)
        .width(Fill)
        .style(chrome::shell)
        .into()
    }

    fn play_view(&self) -> Element<'_, Message> {
        let title = self
            .release
            .as_ref()
            .map(|r| format!("Oxide {}", r.tag_name))
            .unwrap_or_else(|| "Welcome to Oxide!".into());
        let mut news = column![
            text("Oxide News").size(32).font(Font {
                weight: iced::font::Weight::Bold,
                ..Font::DEFAULT
            }),
            text(title)
                .size(16)
                .color(landscape::color(0xaaaaff))
                .font(Font {
                    weight: iced::font::Weight::Bold,
                    ..Font::DEFAULT
                }),
        ]
        .spacing(16);
        if let Some(error) = &self.error {
            news = news.push(text(error).size(13).color(landscape::color(0xffb0a0)));
        }
        if self
            .release
            .as_ref()
            .is_some_and(|r| self.installed.as_ref().is_none_or(|i| i.tag != r.tag_name))
        {
            news = news.push(
                button(text("Install latest release").size(14))
                    .style(chrome::news_link)
                    .padding(0)
                    .on_press_maybe(
                        (!self.busy && self.child.is_none()).then_some(Message::Install(false)),
                    ),
            );
        }
        news = news
            .push(text("Release notes").size(16).font(Font {
                weight: iced::font::Weight::Bold,
                ..Font::DEFAULT
            }))
            .push(
                markdown::view(
                    &self.release_notes,
                    markdown::Settings::with_text_size(
                        14,
                        markdown::Style {
                            link_color: landscape::color(0xaaaaff),
                            ..markdown::Style::from(chrome::theme(true))
                        },
                    ),
                )
                .map(Message::OpenNewsLink),
            );
        let sidebar = column![
            text("Project links:").size(20).font(Font {
                weight: iced::font::Weight::Bold,
                ..Font::DEFAULT
            }),
            button(text("GitHub Repo").size(14))
                .style(chrome::news_link)
                .padding(0)
                .on_press(Message::OpenProject),
            button(text("OS-MC Website").size(14))
                .style(chrome::news_link)
                .padding(0)
                .on_press(Message::OpenOldSchoolMinecraft),
            button(text("OS-MC Discord").size(14))
                .style(chrome::news_link)
                .padding(0)
                .on_press(Message::OpenDiscord),
        ]
        .spacing(6)
        .width(180);
        let page = container(row![container(news).width(Fill), sidebar].spacing(26))
            .padding(iced::Padding {
                top: 20.,
                right: 18.,
                bottom: 24.,
                left: 32.,
            })
            .width(Fill)
            .style(chrome::news);
        stack![
            canvas::Canvas::new(landscape::Dirt)
                .width(Fill)
                .height(Fill),
            scrollable(page).spacing(2).height(Fill),
        ]
        .height(Fill)
        .into()
    }

    fn log_view(&self) -> Element<'_, Message> {
        let lines = self
            .launcher_log
            .iter()
            .fold(String::new(), |mut lines, line| {
                lines.push_str(line);
                lines.push('\n');
                lines
            });
        container(scrollable(text(lines).size(12).font(Font::MONOSPACE)).height(Fill))
            .padding(6)
            .width(Fill)
            .height(Fill)
            .style(chrome::inset)
            .into()
    }

    fn game_log_view(&self) -> Element<'_, Message> {
        container(
            scrollable(
                text(String::from_utf8_lossy(&self.game_log).into_owned())
                    .size(12)
                    .font(Font::MONOSPACE),
            )
            .anchor_bottom()
            .height(Fill),
        )
        .padding(6)
        .width(Fill)
        .height(Fill)
        .style(chrome::inset)
        .into()
    }

    fn settings_view(&self) -> Element<'_, Message> {
        let muted = chrome::muted(&self.theme());
        let appearance = column![
            checkbox(self.settings.dark_mode)
                .label("Dark launcher log")
                .on_toggle(Message::DarkMode),
            text("Use dark colors in Launcher Log. Profile Editor uses the classic gray controls.")
                .size(12)
                .color(muted),
        ]
        .spacing(8);
        let launcher = column![
            row![
                input(
                    "Launcher startup width",
                    "900",
                    &self.settings.launcher_width,
                    Field::LauncherWidth
                ),
                input(
                    "Launcher startup height",
                    "550",
                    &self.settings.launcher_height,
                    Field::LauncherHeight
                ),
            ]
            .spacing(16),
            text(if self.settings.launcher_size().is_some() {
                "Applies when you reopen the launcher. Game size is configured separately below."
            } else {
                "Use width 780–4096 and height 480–4096. Invalid sizes open at 900 × 550."
            })
            .size(12)
            .color(muted),
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
                .label("Automatically install game client releases")
                .on_toggle(Message::AutoUpdate),
            checkbox(self.settings.launcher_auto_update)
                .label("Automatically update the launcher")
                .on_toggle(Message::LauncherAutoUpdate),
            text(format!("Launcher {} · {}", env!("CARGO_PKG_VERSION"), self.launcher_update_status))
                .size(12).color(muted),
            button(text("Install launcher update").size(12))
                .style(chrome::control)
                .on_press_maybe((self.launcher_update.is_some() && !self.launcher_updating && self.child.is_none())
                    .then_some(Message::UpdateLauncher)),
            text("Checks on startup. Updates install while the game is closed. Launcher updates take effect after restart.")
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
                input(
                    "Game window width",
                    "854",
                    &self.settings.width,
                    Field::Width
                ),
                input(
                    "Game window height",
                    "480",
                    &self.settings.height,
                    Field::Height
                )
            ]
            .spacing(16),
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
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn news_markdown_renders_structure_and_inline_styles() {
        let items = parse_release_notes(Some(
            "# Release\n\n**Bold** and *italic* with `code` and [GitHub](https://github.com/LeowFlow/Oxide).\n\n- Sounds\n- Textures\n\n```text\nhydra-client --username Steve\n```\n\n> Note",
        ));
        assert!(matches!(items[0], markdown::Item::Heading(..)));
        let markdown::Item::Paragraph(ref text) = items[1] else {
            panic!("paragraph")
        };
        let spans = text.spans(markdown::Style::from(chrome::theme(true)));
        assert!(
            spans
                .iter()
                .any(|s| s.font.is_some_and(|f| f.weight == iced::font::Weight::Bold))
        );
        assert!(
            spans
                .iter()
                .any(|s| s.font.is_some_and(|f| f.style == iced::font::Style::Italic))
        );
        assert!(spans.iter().any(|s| s.font == Some(Font::MONOSPACE)));
        assert!(
            spans
                .iter()
                .any(|s| s.link.as_deref() == Some("https://github.com/LeowFlow/Oxide"))
        );
        assert!(
            items
                .iter()
                .any(|i| matches!(i, markdown::Item::List { bullets, .. } if bullets.len()==2))
        );
        assert!(items.iter().any(
            |i| matches!(i, markdown::Item::CodeBlock { code, .. } if code.contains("--username"))
        ));
        assert!(items.iter().any(|i| matches!(i, markdown::Item::Quote(_))));
        assert_eq!(parse_release_notes(None).len(), 1);
    }
}
