use crate::{
    app::{App, Message},
    chrome::color,
    releases::Note,
};
use iced::{
    Border, Color, Element, Fill, Font, Pixels, Rectangle, Renderer, Theme, mouse, padding,
    widget::{
        Column, Space, button, canvas, column, container, image, markdown, rich_text, row, rule,
        scrollable, span, stack, text,
    },
};
use std::{
    collections::HashSet,
    io,
    process::Command,
    sync::{Arc, LazyLock},
};

const FONT: Font = Font::with_name("Arial");
const BOLD: Font = Font {
    weight: iced::font::Weight::Bold,
    ..FONT
};
const LINK: Color = color(0xaaaaff);
const TEXT: Color = color(0xe0d0d0);
const MUTED: Color = color(0xbdb4b4);
pub(crate) const TILE_SIZE: u32 = 48;
pub(crate) const TILE: &[u8; (TILE_SIZE * TILE_SIZE * 4) as usize] =
    include_bytes!("../assets/stone.rgba");

pub(crate) struct Entry {
    tag: String,
    date: Option<String>,
    notes: Vec<markdown::Item>,
}

#[derive(PartialEq, Eq)]
pub(crate) enum HistoryStatus {
    Idle,
    Loading,
    Unavailable,
}

pub(crate) fn entries(mut notes: Vec<Note>) -> Vec<Entry> {
    notes.sort_by(|a, b| b.published_at.cmp(&a.published_at));
    let mut seen = HashSet::new();
    notes
        .into_iter()
        .filter(|note| seen.insert(note.tag_name.clone()))
        .map(|note| Entry {
            tag: note.tag_name,
            date: note.published_at,
            notes: parse(note.body.as_deref()),
        })
        .collect()
}

pub(crate) fn parse(body: Option<&str>) -> Vec<markdown::Item> {
    markdown::parse(
        body.filter(|body| !body.trim().is_empty())
            .unwrap_or("No release notes available."),
    )
    .collect()
}

pub(crate) fn view(app: &App) -> Element<'_, Message> {
    let latest = app.release.as_ref();
    let latest_tag = latest.map(|release| release.tag_name.as_str());
    let mut release = release_header(
        latest_tag,
        latest.and_then(|release| release.published_at.as_deref()),
    );
    if let Some(error) = &app.error {
        release = release.push(text(error).font(FONT).size(14).color(color(0xffb8a6)));
    }
    if app.has_game_update() {
        release = release.push(
            button(text("Install latest release").font(FONT).size(14))
                .padding([5, 8])
                .style(update_button)
                .on_press_maybe((!app.busy() && app.game.is_none()).then_some(Message::Install)),
        );
    }
    let mut notes = column![text("Oxide News").size(27).font(BOLD), release]
        .spacing(14)
        .width(Fill);
    if latest.is_some() || app.release_history.is_empty() {
        notes = notes.push(release_body(&app.release_notes));
    }
    for entry in &app.release_history {
        if Some(entry.tag.as_str()) == latest_tag {
            continue;
        }
        notes = notes
            .push(rule::horizontal(1).style(|_| rule::Style {
                color: color(0x505050),
                radius: 0.into(),
                fill_mode: rule::FillMode::Full,
                snap: true,
            }))
            .push(release_header(Some(&entry.tag), entry.date.as_deref()))
            .push(release_body(&entry.notes));
    }
    if let Some(status) = match app.history_status {
        HistoryStatus::Loading if latest.is_some() && app.release_history.is_empty() => {
            Some("Loading older releases…")
        }
        HistoryStatus::Unavailable if latest.is_some() || !app.release_history.is_empty() => {
            Some("Could not refresh older release notes.")
        }
        _ => None,
    } {
        notes = notes.push(text(status).font(FONT).size(13).color(MUTED));
    }

    let mut links = column![text("Official links:").size(22).font(BOLD)]
        .spacing(7)
        .width(156);
    for (label, url) in [
        ("GitHub", "https://github.com/OldSchoolMinecraft"),
        ("OSM Website", "https://os-mc.net"),
        ("Discord", "https://discord.gg/J3vKjD3"),
        ("Wiki", "https://wiki.os-mc.net/"),
    ] {
        links = links.push(
            button(
                rich_text![span(label).underline(true)]
                    .on_link_click(iced::never)
                    .font(FONT)
                    .size(15),
            )
            .padding(0)
            .style(|_, status| button::Style {
                text_color: match status {
                    button::Status::Hovered | button::Status::Pressed => color(0xd5d5ff),
                    _ => LINK,
                },
                ..Default::default()
            })
            .on_press(Message::OpenLink(url.into())),
        );
    }

    let page = container(row![notes, links].spacing(32))
        .width(Fill)
        .max_width(1120)
        .style(|_| container::Style::default().color(TEXT));
    stack![
        container(Space::new().width(Fill).height(Fill))
            .style(|_| container::Style::default().background(color(0x202020))),
        canvas::Canvas::new(Stone).width(Fill).height(Fill),
        scrollable(container(page).center_x(Fill).padding([22, 28]))
            .width(Fill)
            .height(Fill)
            .spacing(2)
            .style(|_, status| scrollable::default(&Theme::Dark, status)),
    ]
    .clip(true)
    .height(Fill)
    .into()
}

fn release_header<'a>(tag: Option<&str>, date: Option<&'a str>) -> Column<'a, Message> {
    let title = tag.map_or_else(
        || "Welcome to Oxide!".to_owned(),
        |tag| format!("Oxide {tag}"),
    );
    let mut header = column![
        rich_text![span(title).underline(tag.is_some())]
            .on_link_click(iced::never)
            .font(FONT)
            .size(18)
            .color(LINK),
    ]
    .spacing(6);
    if let Some(date) = date {
        header = header.push(
            text(date.split_once('T').map_or(date, |(date, _)| date))
                .font(FONT)
                .size(13)
                .color(MUTED),
        );
    }
    header
}

fn release_body(items: &[markdown::Item]) -> Element<'_, Message> {
    markdown::view_with(
        items,
        markdown::Settings {
            h1_size: 20.into(),
            h2_size: 18.into(),
            h3_size: 16.into(),
            h4_size: 15.into(),
            spacing: 10.into(),
            ..markdown::Settings::with_text_size(
                15,
                markdown::Style {
                    font: FONT,
                    link_color: LINK,
                    ..markdown::Style::from(Theme::Dark)
                },
            )
        },
        &Notes,
    )
}

struct Notes;

impl<'a> markdown::Viewer<'a, Message> for Notes {
    fn on_link_click(url: markdown::Uri) -> Message {
        Message::OpenLink(url)
    }

    fn heading(
        &self,
        settings: markdown::Settings,
        level: &'a markdown::HeadingLevel,
        text: &'a markdown::Text,
        index: usize,
    ) -> Element<'a, Message> {
        use markdown::HeadingLevel::*;

        let size = match level {
            H1 => settings.h1_size,
            H2 => settings.h2_size,
            H3 => settings.h3_size,
            H4 => settings.h4_size,
            H5 => settings.h5_size,
            H6 => settings.h6_size,
        };
        container(
            rich_text(note_spans(text, settings.style))
                .size(size)
                .on_link_click(Self::on_link_click),
        )
        .padding(padding::top(if index > 0 {
            settings.text_size / 2.
        } else {
            Pixels::ZERO
        }))
        .into()
    }

    fn paragraph(
        &self,
        settings: markdown::Settings,
        text: &markdown::Text,
    ) -> Element<'a, Message> {
        rich_text(note_spans(text, settings.style))
            .size(settings.text_size)
            .on_link_click(Self::on_link_click)
            .into()
    }
}

fn note_spans(
    text: &markdown::Text,
    style: markdown::Style,
) -> Arc<[text::Span<'static, markdown::Uri>]> {
    let mut spans = text.spans(style);
    if spans.iter().any(|span| span.link.is_some()) {
        for span in Arc::make_mut(&mut spans) {
            span.underline |= span.link.is_some();
        }
    }
    spans
}

fn update_button(_: &Theme, status: button::Status) -> button::Style {
    let (background, edge, text_color) = match status {
        button::Status::Active => (0x303030, 0x727272, LINK),
        button::Status::Hovered => (0x414141, 0x979797, color(0xd5d5ff)),
        button::Status::Pressed => (0x222222, 0x727272, LINK),
        button::Status::Disabled => (0x292929, 0x555555, MUTED),
    };
    button::Style {
        background: Some(color(background).into()),
        text_color,
        border: Border {
            color: color(edge),
            width: 1.,
            ..Border::default()
        },
        ..Default::default()
    }
}

struct Stone;

impl<Message> canvas::Program<Message> for Stone {
    type State = ();

    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        static STONE: LazyLock<image::Handle> =
            LazyLock::new(|| image::Handle::from_rgba(TILE_SIZE, TILE_SIZE, TILE.as_slice()));
        let size = TILE_SIZE as f32;
        let tile = canvas::Image::new(STONE.clone())
            .filter_method(image::FilterMethod::Nearest)
            .snap(true);
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        for y in 0..(bounds.height / size).ceil() as u32 {
            for x in 0..(bounds.width / size).ceil() as u32 {
                frame.draw_image(
                    Rectangle {
                        x: x as f32 * size,
                        y: y as f32 * size,
                        width: size,
                        height: size,
                    },
                    tile.clone(),
                );
            }
        }
        vec![frame.into_geometry()]
    }
}

fn web_url(link: &str) -> Option<reqwest::Url> {
    reqwest::Url::parse(link)
        .ok()
        .filter(|url| matches!(url.scheme(), "https" | "http"))
}

pub(crate) fn open_link(link: &str) -> io::Result<()> {
    let Some(url) = web_url(link) else {
        return Ok(());
    };
    #[cfg(target_os = "windows")]
    let mut command = Command::new("rundll32");
    #[cfg(target_os = "windows")]
    command.arg("url.dll,FileProtocolHandler");
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let mut command = Command::new("xdg-open");
    command.arg(url.as_str()).spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_keeps_each_release_once_with_its_own_notes() {
        let notes = entries(vec![
            Note {
                tag_name: "v1.0.0".into(),
                published_at: Some("2026-11-22T16:12:20Z".into()),
                body: Some("## First release".into()),
            },
            Note {
                tag_name: "v1.0.1".into(),
                published_at: Some("2026-11-22T18:12:25Z".into()),
                body: Some("- Fix".into()),
            },
            Note {
                tag_name: "v0.9.0".into(),
                published_at: None,
                body: None,
            },
            Note {
                tag_name: "v1.0.0".into(),
                published_at: Some("2026-11-22T16:12:20Z".into()),
                body: None,
            },
        ]);
        let tags: Vec<_> = notes.iter().map(|entry| entry.tag.as_str()).collect();
        assert_eq!(tags, ["v1.0.1", "v1.0.0", "v0.9.0"]);
        assert!(matches!(
            notes[0].notes.as_slice(),
            [markdown::Item::List { .. }]
        ));
        assert!(matches!(
            notes[1].notes.as_slice(),
            [markdown::Item::Heading(..)]
        ));
        assert!(matches!(
            notes[2].notes.as_slice(),
            [markdown::Item::Paragraph(_)]
        ));
    }

    #[test]
    fn links_only_open_web_urls() {
        assert!(web_url("https://github.com/LeowFlow/Oxide").is_some());
        assert!(web_url("http://example.org/notes").is_some());
        for link in [
            "javascript:alert(1)",
            "file:///C:/test.exe",
            "mailto:test@example.org",
            "relative/path",
            "",
        ] {
            assert!(web_url(link).is_none());
        }
    }

    #[test]
    fn news_markdown_renders_structure_and_inline_styles() {
        let items = parse(Some(
            "# Release\n\n**Bold** and *italic* with `code` and [GitHub](https://github.com/LeowFlow/Oxide).\n\n- Sounds\n- Textures\n\n```text\nhydra-client --username Steve\n```\n\n> Note",
        ));
        assert!(matches!(items[0], markdown::Item::Heading(..)));
        let markdown::Item::Paragraph(ref text) = items[1] else {
            panic!("paragraph")
        };
        let spans = note_spans(text, markdown::Style::from(Theme::Dark));
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
        assert!(spans.iter().any(|s| s.link.as_deref()
            == Some("https://github.com/LeowFlow/Oxide")
            && s.underline));
        assert!(
            items
                .iter()
                .any(|i| matches!(i, markdown::Item::List { bullets, .. } if bullets.len()==2))
        );
        assert!(items.iter().any(
            |i| matches!(i, markdown::Item::CodeBlock { code, .. } if code.contains("--username"))
        ));
        assert!(items.iter().any(|i| matches!(i, markdown::Item::Quote(_))));
        assert_eq!(parse(None).len(), 1);
    }
}
