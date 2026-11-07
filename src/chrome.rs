//! Shared colors and beveled, glossy surfaces for both appearance modes.
use crate::landscape::color;
use iced::{
    Background, Border, Color, Radians, Shadow, Theme, gradient,
    widget::{button, container, text_input},
};

pub fn theme(dark: bool) -> Theme {
    Theme::custom(
        if dark {
            "Midnight glass"
        } else {
            "Silver glass"
        },
        iced::theme::Palette {
            background: color(if dark { 0x101923 } else { 0xd4dfe8 }),
            text: color(if dark { 0xe0e9f0 } else { 0x203346 }),
            primary: color(if dark { 0x78bce3 } else { 0x23648b }),
            success: color(0x8dbb68),
            warning: color(0xd2ab60),
            danger: color(if dark { 0xf3a59a } else { 0x9e352e }),
        },
    )
}
pub fn dark(theme: &Theme) -> bool {
    theme.extended_palette().is_dark
}
pub fn muted(theme: &Theme) -> Color {
    color(if dark(theme) { 0x9caebd } else { 0x52687a })
}
pub fn line(theme: &Theme) -> Color {
    color(if dark(theme) { 0x43596b } else { 0x8da5b7 })
}
fn border(theme: &Theme, radius: f32) -> Border {
    Border {
        color: line(theme),
        width: 1.,
        radius: radius.into(),
    }
}
fn gradient(top: u32, bottom: u32) -> Background {
    gradient::Linear::new(Radians(std::f32::consts::PI))
        .add_stop(0., color(top))
        .add_stop(1., color(bottom))
        .into()
}
fn gloss(top: u32, middle: u32, bottom: u32) -> Background {
    gradient::Linear::new(Radians(std::f32::consts::PI))
        .add_stop(0., color(top))
        .add_stop(0.49, color(middle))
        .add_stop(0.50, color(bottom))
        .add_stop(1., color(middle))
        .into()
}
pub fn desktop(theme: &Theme) -> container::Style {
    container::Style::default().background(if dark(theme) {
        gradient(0x263f56, 0x090f17)
    } else {
        gradient(0xb4d1e5, 0xe1e8ee)
    })
}
pub fn frame(theme: &Theme) -> container::Style {
    container::Style::default()
        .background(if dark(theme) {
            gradient(0x31485b, 0x152331)
        } else {
            gradient(0xe4eff7, 0xaec3d4)
        })
        .border(border(theme, 7.))
}
pub fn panel(theme: &Theme) -> container::Style {
    container::Style::default()
        .background(color(if dark(theme) { 0x17232e } else { 0xf2f5f8 }))
        .border(border(theme, 3.))
}
pub fn heading(theme: &Theme) -> container::Style {
    container::Style::default()
        .background(if dark(theme) {
            gloss(0x40596e, 0x2e4355, 0x233647)
        } else {
            gloss(0xf7fbff, 0xdce7ef, 0xc8d8e4)
        })
        .border(border(theme, 2.))
}
pub fn inset(theme: &Theme) -> container::Style {
    container::Style::default()
        .background(color(if dark(theme) { 0x101b25 } else { 0xe3ebf1 }))
        .border(border(theme, 2.))
}
pub fn control(theme: &Theme, status: button::Status) -> button::Style {
    let is_dark = dark(theme);
    let mut s = button::Style {
        background: Some(if is_dark {
            gloss(0x50677b, 0x344b60, 0x23384a)
        } else {
            gloss(0xffffff, 0xe3ebf1, 0xc6d6e3)
        }),
        text_color: theme.palette().text,
        border: border(theme, 3.),
        ..Default::default()
    };
    match status {
        button::Status::Hovered => {
            s.border.color = color(0x90ccef);
            s.background = Some(if is_dark {
                gloss(0x587e9a, 0x355d7b, 0x24465e)
            } else {
                gloss(0xffffff, 0xd4edff, 0xb9dcf4)
            });
        }
        button::Status::Pressed => {
            s.background = Some(gradient(
                if is_dark { 0x192b3a } else { 0xafcadd },
                if is_dark { 0x35556e } else { 0xd4e6f3 },
            ));
            s.shadow = Shadow::default();
        }
        button::Status::Disabled => {
            s.text_color = muted(theme);
            s.background = s.background.map(|b| b.scale_alpha(0.45));
            s.shadow = Shadow::default();
        }
        button::Status::Active => {}
    }
    s
}
pub fn launch(theme: &Theme, status: button::Status) -> button::Style {
    let mut s = control(theme, status);
    if status != button::Status::Disabled {
        s.background = Some(match status {
            button::Status::Hovered => gloss(0xabc77a, 0x759a48, 0x4b7328),
            button::Status::Pressed => gradient(0x38581f, 0x6e9148),
            _ => gloss(0x8fac68, 0x5e823c, 0x385e25),
        });
        s.text_color = Color::WHITE;
        s.border.color = color(0xb7cf91);
    }
    s
}
pub fn tab(theme: &Theme, status: button::Status, active: bool) -> button::Style {
    let mut s = control(theme, status);
    if active {
        s.background = Some(if dark(theme) {
            gloss(0x557b98, 0x355970, 0x234457)
        } else {
            gloss(0xffffff, 0xd0e7f7, 0xb6d6ec)
        });
        s.border.color = color(0x86badd);
    }
    s
}
pub fn input(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let mut s = text_input::default(theme, status);
    s.background = color(if dark(theme) { 0x0c151e } else { 0xffffff }).into();
    s.border = border(theme, 2.);
    if matches!(status, text_input::Status::Focused { .. }) {
        s.border.color = color(0x8bc5ec);
    }
    s.value = theme.palette().text;
    s.placeholder = muted(theme);
    s
}
