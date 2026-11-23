//! Classic launcher proportions with native Rust controls and offline access.
use crate::{
    app::{App, Field, Message, Page},
    chrome::{self, BOLD, color},
    news, profile,
};
use iced::{
    Element, Fill, Font,
    widget::{button, column, container, rich_text, row, scrollable, span, text, text_input},
};

impl App {
    pub(crate) fn view(&self) -> Element<'_, Message> {
        let mut tab_row = row![
            self.page_tab("Update Notes", Page::News),
            self.page_tab("Launcher Log", Page::Log),
        ];
        if self.game.is_some() {
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
            Page::News => news::view(self),
            Page::Log => self.log_view(),
            Page::GameLog => self.game_log_view(),
            Page::Settings => profile::view(self),
        };
        let content = container(content)
            .width(Fill)
            .height(Fill)
            .style(chrome::frame);
        let mut body = column![tabs, content, self.launch_bar()].spacing(0);
        if let Some(error) = &self.save_error {
            body = body.push(
                container(text(error).size(12).color(color(0xa00000)))
                    .padding([2, 6])
                    .width(Fill)
                    .style(chrome::shell),
            );
        }
        body = body.push(
            container(
                row![
                    column![
                        text(&self.status).size(11),
                        text(&self.launcher_update_status).size(11)
                    ]
                    .width(Fill),
                    text(if self.settings_dirty() {
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
        let name = self.settings.player_name();
        let title = if self.game.is_some() {
            "Game running"
        } else if self.busy() || self.launcher_updating() {
            "Please wait…"
        } else if self.settings.username.trim().is_empty() {
            "Enter username"
        } else if name.is_none() {
            "Invalid username"
        } else if self.installed.is_none() && self.settings.executable.trim().is_empty() {
            "Install & Play"
        } else {
            "Play"
        };
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
                    .on_press_maybe((!self.busy()).then_some(Message::Check)),
            ]
            .spacing(6)
        ]
        .spacing(6);
        let play = button(
            container(text(title).size(16).font(BOLD))
                .center_x(Fill)
                .center_y(Fill),
        )
        .width(Fill)
        .height(54)
        .padding(6)
        .style(chrome::control)
        .on_press_maybe(self.can_play().then_some(Message::Play));
        let welcome: Element<'_, Message> = if let Some(name) = name {
            rich_text![span("Welcome, "), span(name).font(BOLD)]
                .on_link_click(iced::never)
                .align_x(iced::Right)
                .size(12)
                .into()
        } else {
            text("Welcome").size(12).into()
        };
        let account = column![
            welcome,
            text(if self.installed.is_some() {
                "Ready to play Minecraft Beta 1.7.3"
            } else {
                "Ready to install Minecraft Beta 1.7.3"
            })
            .size(11),
        ]
        .spacing(4)
        .width(Fill)
        .align_x(iced::Right);
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

    fn log_view(&self) -> Element<'_, Message> {
        let lines = self
            .launcher_log
            .iter()
            .fold(String::new(), |mut lines, line| {
                lines.push_str(line);
                lines.push('\n');
                lines
            });
        log_view(lines, false)
    }

    fn game_log_view(&self) -> Element<'_, Message> {
        log_view(String::from_utf8_lossy(&self.game_log).into_owned(), true)
    }
}

fn log_view(lines: String, follow: bool) -> Element<'static, Message> {
    let log = scrollable(text(lines).size(12).font(Font::MONOSPACE)).height(Fill);
    container(if follow { log.anchor_bottom() } else { log })
        .padding(6)
        .width(Fill)
        .height(Fill)
        .style(chrome::inset)
        .into()
}
