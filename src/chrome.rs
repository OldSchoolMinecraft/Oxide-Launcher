//! Native-looking classic launcher chrome, independent of news/editor colors.
use iced::{
    Background, Border, Color, Font, Radians, Theme, gradient,
    widget::{button, container, pick_list, text_input},
};
pub(crate) const FONT: Font = if cfg!(windows) {
    Font::with_name("Tahoma")
} else {
    Font::DEFAULT
};
pub(crate) const BOLD: Font = Font {
    weight: iced::font::Weight::Bold,
    ..FONT
};

pub(crate) const fn color(hex: u32) -> Color {
    Color::from_rgb8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

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
pub fn inset(theme: &Theme) -> container::Style {
    container::Style::default().background(color(if dark(theme) { 0x202020 } else { 0xffffff }))
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
pub fn select(theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    let (top, bottom, edge) = match status {
        pick_list::Status::Active => (0xfafafa, 0xe5e5e5, 0x8f8f8f),
        pick_list::Status::Hovered | pick_list::Status::Opened { .. } => {
            (0xecf4fc, 0xdcecfc, 0x0078d7)
        }
    };
    pick_list::Style {
        background: gradient(top, bottom),
        border: border(edge),
        ..pick_list::default(theme, status)
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
