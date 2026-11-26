//! Native-looking classic launcher chrome, independent of news/editor colors.
use iced::{
    Background, Border, Color, Font, Radians, Theme, gradient,
    widget::{button, container, pick_list, text, text_input},
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
            background: color(if dark { 0x2b2b2b } else { 0xf0f0f0 }),
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
pub fn danger(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(theme.palette().danger),
    }
}
pub fn muted_text(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(muted(theme)),
    }
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
/// The launcher's own surface: tab strip, launch bar, status bar and editors.
pub fn shell(theme: &Theme) -> container::Style {
    container::Style::default()
        .background(theme.palette().background)
        .color(theme.palette().text)
}
pub fn desktop(theme: &Theme) -> container::Style {
    container::Style::default().background(theme.palette().background)
}
pub fn frame(theme: &Theme) -> container::Style {
    container::Style::default().border(Border {
        radius: 0.into(),
        ..border(if dark(theme) { 0x565656 } else { 0xaaaaaa })
    })
}
/// The log terminal, which can be dark on an otherwise light launcher.
pub fn inset(dark: bool) -> container::Style {
    container::Style::default()
        .background(color(if dark { 0x202020 } else { 0xffffff }))
        .color(color(if dark { 0xe6e6e6 } else { 0x000000 }))
}
pub fn tip(theme: &Theme) -> container::Style {
    shell(theme).border(border(0x767676))
}
pub fn control(theme: &Theme, status: button::Status) -> button::Style {
    let (top, bottom, edge, foreground) = match (dark(theme), status) {
        (false, button::Status::Hovered) => (0xecf4fc, 0xdcecfc, 0x0078d7, 0x000000),
        (false, button::Status::Pressed) => (0xc4dff6, 0xe0effb, 0x005499, 0x000000),
        (false, button::Status::Disabled) => (0xf2f2f2, 0xe8e8e8, 0xbfbfbf, 0x838383),
        (false, button::Status::Active) => (0xfafafa, 0xe5e5e5, 0x8f8f8f, 0x000000),
        (true, button::Status::Hovered) => (0x55616e, 0x46515d, 0x4a9eea, 0xffffff),
        (true, button::Status::Pressed) => (0x2c4763, 0x385876, 0x4a9eea, 0xffffff),
        (true, button::Status::Disabled) => (0x363636, 0x323232, 0x484848, 0x7c7c7c),
        (true, button::Status::Active) => (0x4c4c4c, 0x3d3d3d, 0x6a6a6a, 0xe6e6e6),
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
        style.background = Some(color(if dark(theme) { 0x1f1f1f } else { 0xffffff }).into());
    }
    style
}
pub fn select(theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    let look = control(
        theme,
        match status {
            pick_list::Status::Active => button::Status::Active,
            pick_list::Status::Hovered | pick_list::Status::Opened { .. } => {
                button::Status::Hovered
            }
        },
    );
    pick_list::Style {
        background: look.background.unwrap_or(theme.palette().background.into()),
        border: look.border,
        text_color: look.text_color,
        handle_color: look.text_color,
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
pub fn player_input(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let mut style = input(theme, status);
    style.border.radius = 0.into();
    style
}
