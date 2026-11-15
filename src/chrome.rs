//! Native-looking classic launcher chrome, independent of news/editor colors.
use crate::landscape::color;
use iced::{
    Background, Border, Color, Radians, Theme, gradient,
    widget::{button, container, text_input},
};
pub fn theme(dark: bool) -> Theme {
    Theme::custom(
        if dark {
            "Classic dark editor"
        } else {
            "Classic light editor"
        },
        iced::theme::Palette {
            background: color(if dark { 0x303030 } else { 0xf0f0f0 }),
            text: color(if dark { 0xe6e6e6 } else { 0x000000 }),
            primary: color(0x567da4),
            success: color(0x507b36),
            warning: color(0xc09a36),
            danger: color(if dark { 0xffb0a0 } else { 0x990000 }),
        },
    )
}
pub fn dark(theme: &Theme) -> bool {
    theme.extended_palette().is_dark
}
pub fn muted(theme: &Theme) -> Color {
    color(if dark(theme) { 0xbcbcbc } else { 0x555555 })
}
fn border(value: u32) -> Border {
    Border {
        color: color(value),
        width: 1.,
        radius: 2.into(),
    }
}
fn gradient(top: u32, bottom: u32) -> Background {
    gradient::Linear::new(Radians(std::f32::consts::PI))
        .add_stop(0., color(top))
        .add_stop(1., color(bottom))
        .into()
}
pub fn shell(_: &Theme) -> container::Style {
    container::Style::default()
        .background(color(0xf0f0f0))
        .color(Color::BLACK)
}
pub fn desktop(theme: &Theme) -> container::Style {
    container::Style::default().background(theme.palette().background)
}
pub fn frame(_: &Theme) -> container::Style {
    container::Style::default().border(Border {
        radius: 0.into(),
        ..border(0xaaaaaa)
    })
}
pub fn panel(theme: &Theme) -> container::Style {
    container::Style::default()
        .background(theme.palette().background)
        .border(border(if dark(theme) { 0x606060 } else { 0xb0b0b0 }))
}
pub fn heading(theme: &Theme) -> container::Style {
    container::Style::default().background(color(if dark(theme) { 0x383838 } else { 0xe8e8e8 }))
}
pub fn inset(theme: &Theme) -> container::Style {
    container::Style::default().background(color(if dark(theme) { 0x202020 } else { 0xffffff }))
}
pub fn news(_: &Theme) -> container::Style {
    container::Style::default().color(color(0xe0d0d0))
}
pub fn control(_: &Theme, status: button::Status) -> button::Style {
    let (top, bottom, edge, foreground) = match status {
        button::Status::Hovered => (0xecf4fc, 0xdcecfc, 0x0078d7, 0x000000),
        button::Status::Pressed => (0xc4dff6, 0xe0effb, 0x005499, 0x000000),
        button::Status::Disabled => (0xf2f2f2, 0xe8e8e8, 0xbfbfbf, 0x838383),
        button::Status::Active => (0xfafafa, 0xe5e5e5, 0x8f8f8f, 0x000000),
    };
    button::Style {
        background: Some(gradient(top, bottom)),
        text_color: color(foreground),
        border: border(edge),
        ..Default::default()
    }
}
pub fn launch(theme: &Theme, status: button::Status) -> button::Style {
    control(theme, status)
}
pub fn tab(theme: &Theme, status: button::Status, active: bool) -> button::Style {
    let mut style = control(theme, status);
    style.border.radius = iced::border::Radius {
        top_left: 3.,
        top_right: 3.,
        bottom_left: 0.,
        bottom_right: 0.,
    };
    if active {
        style.background = Some(color(0xffffff).into());
    }
    style
}
pub fn news_link(_: &Theme, status: button::Status) -> button::Style {
    button::Style {
        text_color: color(match status {
            button::Status::Hovered | button::Status::Pressed => 0xddddff,
            button::Status::Disabled => 0x888888,
            _ => 0xaaaaff,
        }),
        ..Default::default()
    }
}
pub fn input(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let mut style = text_input::default(theme, status);
    style.background = color(if dark(theme) { 0x202020 } else { 0xffffff }).into();
    style.border = border(if matches!(status, text_input::Status::Focused { .. }) {
        0x0078d7
    } else {
        0x8f8f8f
    });
    style.value = theme.palette().text;
    style.placeholder = muted(theme);
    style
}
pub fn player_input(_: &Theme, status: text_input::Status) -> text_input::Style {
    let mut style = input(&theme(false), status);
    style.border.radius = 0.into();
    style
}
