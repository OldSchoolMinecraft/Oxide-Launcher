//! Tile the original classic launcher dirt pixels without an image decoder.
const DIRT: &[u8; 16 * 16 * 3] = include_bytes!("../assets/dirt.rgb");
use iced::{
    Color, Point, Rectangle, Renderer, Theme, mouse,
    widget::canvas::{self, Frame, Geometry},
};
pub struct Dirt;
impl<Message> canvas::Program<Message> for Dirt {
    type State = ();
    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), color(0x25201b));
        // Nearest-neighbor 4x scaling keeps the original 16x16 tile crisp.
        for y in 0..(bounds.height / 4.).ceil() as u32 {
            for x in 0..(bounds.width / 4.).ceil() as u32 {
                let offset = (((y % 16) * 16 + x % 16) * 3) as usize;
                frame.fill_rectangle(
                    Point::new(x as f32 * 4., y as f32 * 4.),
                    iced::Size::new(4., 4.),
                    Color::from_rgb8(DIRT[offset], DIRT[offset + 1], DIRT[offset + 2]),
                );
            }
        }
        vec![frame.into_geometry()]
    }
}
pub fn color(hex: u32) -> Color {
    Color::from_rgb8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}
