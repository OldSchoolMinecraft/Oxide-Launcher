use crate::{
    app::{App, Field, Message},
    chrome::{self, BOLD},
    settings::{Lighting, Visibility},
};
use iced::{
    Element, Fill, padding,
    widget::{
        Checkbox, Row, TextInput, button, checkbox, column, container, pick_list, row, scrollable,
        stack, text, text_input,
    },
};

const LABEL_WIDTH: u32 = 148;
const FIELD_GAP: u32 = 8;

pub(crate) fn view(app: &App) -> Element<'_, Message> {
    let settings = &app.settings;
    let muted = chrome::muted(&app.theme());

    let profile = column![
        field(
            "Game resolution",
            row![
                edit("854", &settings.width, Field::Width).width(72),
                text("×").size(12),
                edit("480", &settings.height, Field::Height).width(72),
            ]
            .spacing(6)
            .align_y(iced::Center),
        ),
        field(
            "Launcher visibility",
            pick_list(
                Visibility::ALL,
                Some(settings.visibility),
                Message::Visibility
            )
            .text_size(12)
            .padding([4, 6])
            .width(210)
            .style(chrome::select),
        ),
        field(
            "Smooth lighting",
            pick_list(Lighting::ALL, Some(settings.lighting), Message::Lighting)
                .text_size(12)
                .padding([4, 6])
                .width(158)
                .style(chrome::select),
        ),
        toggle("Mute game audio", settings.mute, Message::Mute),
    ]
    .spacing(6);

    let resources = column![
        field(
            "Server address",
            edit(
                "Optional · 127.0.0.1:25565",
                &settings.server,
                Field::Server
            ),
        ),
        field(
            "Singleplayer server JAR",
            edit(
                "Optional · absolute path to a Beta 1.7.3 server JAR",
                &settings.singleplayer_server,
                Field::Singleplayer,
            ),
        ),
        field(
            "Texture assets",
            edit(
                "Optional · absolute path to a directory, JAR, ZIP, or terrain.png",
                &settings.assets,
                Field::Assets
            ),
        ),
        field(
            "Sounds & music",
            edit(
                "Optional · absolute path to legacy resources",
                &settings.sounds,
                Field::Sounds
            ),
        ),
    ]
    .spacing(6);

    let data_path = app
        .paths
        .as_ref()
        .map(|paths| paths.data.display().to_string())
        .unwrap_or_else(Clone::clone);
    let local = column![
        field(
            "Executable override",
            edit(
                "Optional · absolute path to rust173 or hydra-client",
                &settings.executable,
                Field::Executable,
            ),
        ),
        field("Game data & logs", text(data_path).size(11).color(muted)),
    ]
    .spacing(6);

    let launcher = column![
        field(
            "Launcher startup size",
            row![
                edit("900", &settings.launcher_width, Field::LauncherWidth).width(72),
                text("×").size(12),
                edit("550", &settings.launcher_height, Field::LauncherHeight).width(72),
            ]
            .spacing(6)
            .align_y(iced::Center),
        ),
        container(
            text(if settings.launcher_size().is_some() {
                "Applies when you reopen the launcher."
            } else {
                "Use width 780–4096 and height 480–4096. Invalid sizes open at 900 × 550."
            })
            .size(11)
            .color(muted),
        )
        .padding(padding::left(LABEL_WIDTH + FIELD_GAP)),
        toggle("Dark mode", settings.dark_theme, Message::DarkTheme),
        // The log is always dark in dark mode.
        toggle(
            "Dark launcher log",
            settings.dark_mode || settings.dark_theme,
            Message::DarkMode
        )
        .on_toggle_maybe(
            (!settings.dark_theme).then_some(Message::DarkMode as fn(bool) -> Message)
        ),
        toggle(
            "Automatically install game client releases",
            settings.auto_update,
            Message::AutoUpdate,
        ),
        toggle(
            "Automatically update the launcher",
            settings.launcher_auto_update,
            Message::LauncherAutoUpdate,
        ),
        field(
            "Launcher update",
            row![
                text(format!(
                    "Launcher {} · {}",
                    env!("CARGO_PKG_VERSION"),
                    app.launcher_update_status
                ))
                .size(11)
                .color(muted)
                .width(Fill),
                button(text("Install launcher update").size(12))
                    .padding([4, 8])
                    .style(chrome::control)
                    .on_press_maybe(app.can_update_launcher().then_some(Message::UpdateLauncher)),
            ]
            .align_y(iced::Center)
            .spacing(FIELD_GAP),
        ),
        text("Launcher updates install while the game is closed and take effect after restart.")
            .size(11)
            .color(muted),
    ]
    .spacing(6);

    let form = column![
        group("Profile Info", profile.into()),
        group("Connection & Resources", resources.into()),
        group("Local Installation", local.into()),
        group("Launcher Settings", launcher.into()),
    ]
    .spacing(9)
    .width(Fill)
    .max_width(720);
    container(
        scrollable(container(form).center_x(Fill).padding([10, 12]))
            .height(Fill)
            .spacing(3),
    )
    .width(Fill)
    .height(Fill)
    .style(chrome::shell)
    .into()
}

pub(crate) fn group<'a, Message: 'a>(
    title: &'a str,
    contents: Element<'a, Message>,
) -> Element<'a, Message> {
    stack![
        container(
            container(contents)
                .padding(padding::top(13).right(10).bottom(9).left(10))
                .width(Fill)
                .style(chrome::frame),
        )
        .padding(padding::top(7)),
        container(
            container(text(title).size(12).font(BOLD))
                .padding([0, 4])
                .style(chrome::shell),
        )
        .padding(padding::left(8)),
    ]
    .width(Fill)
    .into()
}

fn field<'a>(label: &'a str, control: impl Into<Element<'a, Message>>) -> Row<'a, Message> {
    row![text(label).size(12).width(LABEL_WIDTH), control.into()]
        .align_y(iced::Center)
        .spacing(FIELD_GAP)
        .width(Fill)
}

fn edit<'a>(placeholder: &str, value: &str, field: Field) -> TextInput<'a, Message> {
    text_input(placeholder, value)
        .on_input(move |value| Message::Edit(field, value))
        .padding([4, 6])
        .size(12)
        .style(chrome::input)
}

fn toggle(label: &str, value: bool, message: fn(bool) -> Message) -> Checkbox<'_, Message> {
    checkbox(value)
        .label(label)
        .on_toggle(message)
        .size(14)
        .spacing(7)
        .text_size(12)
}
