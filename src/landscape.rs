//! An original, resolution-independent block landscape. No game assets required.
use iced::{
    Color, Point, Rectangle, Renderer, Theme, mouse,
    widget::canvas::{self, Frame, Geometry, Path},
};
pub struct Landscape;
impl<Message> canvas::Program<Message> for Landscape {
    type State = ();
    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut f = Frame::new(renderer, bounds.size());
        let sx = bounds.width / 1000.;
        let sy = bounds.height / 300.;
        let mut rect = |x: f32, y: f32, w: f32, h: f32, c: u32| {
            f.fill_rectangle(
                Point::new(x * sx, y * sy),
                iced::Size::new(w * sx, h * sy),
                color(c),
            );
        };
        let dark = crate::chrome::dark(theme);
        rect(0., 0., 1000., 300., if dark { 0x1d344d } else { 0x86adc8 });
        if dark {
            for (x, y) in [
                (460., 24.),
                (531., 65.),
                (615., 30.),
                (822., 22.),
                (932., 90.),
                (679., 83.),
            ] {
                rect(x, y, 2., 3., 0xb6ccdd);
            }
        }
        rect(710., 38., 46., 46., if dark { 0xd1e2e8 } else { 0xf5e3ae });
        for (x, y, w) in [(60., 46., 95.), (280., 80., 70.), (810., 66., 120.)] {
            rect(x, y, w, 12., 0xdbe4ce);
            rect(x + 18., y - 10., w * 0.55, 10., 0xdbe4ce);
        }
        for i in 0..25 {
            let x = i as f32 * 42.;
            let y = 138. - ((i * 7 % 9) as f32) * 6.;
            rect(x, y, 43., 300. - y, 0x819e8c);
        }
        for i in 0..34 {
            let x = i as f32 * 31.;
            let y = 182. - ((i * 3 % 11) as f32) * 5.;
            rect(x, y, 32., 300. - y, 0x597d69);
        }
        rect(0., 250., 1000., 50., 0x557e80);
        rect(380., 259., 260., 3., 0x7faaa0);
        rect(530., 278., 300., 3., 0x729b95);
        for (x, y, w, h) in [
            (0., 203., 205., 97.),
            (0., 187., 114., 113.),
            (92., 220., 176., 80.),
            (830., 210., 170., 90.),
            (910., 186., 90., 114.),
            (790., 245., 65., 55.),
        ] {
            rect(x, y, w, h, 0x655c44);
            rect(x, y, w, 10., 0x89a25d);
            rect(x, y + 10., w, 5., 0x4f7144);
            for i in 0..(w as i32 / 17) {
                if i % 3 == 0 {
                    rect(
                        x + i as f32 * 17.,
                        y + 28. + (i % 2) as f32 * 18.,
                        12.,
                        8.,
                        0x75684b,
                    );
                }
            }
        }
        for (x, y) in [(60., 115.), (170., 153.), (884., 126.), (957., 102.)] {
            rect(x + 18., y + 30., 13., 65., 0x514d39);
            rect(x - 9., y + 12., 68., 35., 0x345f48);
            rect(x, y - 5., 50., 42., 0x3d6b4c);
            rect(x + 9., y - 18., 32., 24., 0x4e7a50);
        }
        // A few reeds at the water's edge.
        for x in [277., 285., 763., 774.] {
            rect(x, 247., 3., 22., 0x9eae6e);
        }
        // Subtle bottom shade anchors the illustration to the panel below.
        f.fill(
            &Path::rectangle(
                Point::new(0., bounds.height - 3.),
                iced::Size::new(bounds.width, 3.),
            ),
            color(0x304a3b),
        );
        if dark {
            f.fill_rectangle(
                Point::ORIGIN,
                bounds.size(),
                Color::from_rgba(0.02, 0.05, 0.10, 0.30),
            );
        }
        vec![f.into_geometry()]
    }
}
pub fn color(hex: u32) -> Color {
    Color::from_rgb8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}
