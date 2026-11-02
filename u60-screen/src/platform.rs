use anyhow::Result;
use slint::{
    PhysicalSize,
    platform::{
        Platform, WindowAdapter,
        software_renderer::{
            MinimalSoftwareWindow, RenderingRotation, RepaintBufferType, Rgb565Pixel,
        },
    },
};
use std::{
    path::Path,
    rc::Rc,
    time::{Duration, Instant},
};

pub const WIDTH: usize = 320;
pub const HEIGHT: usize = 480;

struct SoftwarePlatform {
    window: Rc<MinimalSoftwareWindow>,
    start: Instant,
}
impl Platform for SoftwarePlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.window.clone())
    }
    fn duration_since_start(&self) -> Duration {
        self.start.elapsed()
    }
}

pub fn install() -> Result<Rc<MinimalSoftwareWindow>> {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
    slint::platform::set_platform(Box::new(SoftwarePlatform {
        window: window.clone(),
        start: Instant::now(),
    }))?;
    window.set_size(PhysicalSize::new(WIDTH as u32, HEIGHT as u32));
    Ok(window)
}

pub fn capture(window: &MinimalSoftwareWindow, rotation: bool) -> Vec<Rgb565Pixel> {
    let mut pixels = vec![Rgb565Pixel(0); WIDTH * HEIGHT];
    window.request_redraw();
    window.draw_if_needed(|renderer| {
        // A capture uses a fresh buffer, including regions the UI hasn't changed.
        renderer.set_repaint_buffer_type(RepaintBufferType::NewBuffer);
        renderer.set_rendering_rotation(if rotation {
            RenderingRotation::Rotate180
        } else {
            RenderingRotation::NoRotation
        });
        renderer.render(&mut pixels, WIDTH);
        renderer.set_repaint_buffer_type(RepaintBufferType::ReusedBuffer);
    });
    pixels
}

pub fn save_png(path: &Path, pixels: &[Rgb565Pixel]) -> Result<()> {
    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(std::fs::File::create(path)?),
        WIDTH as u32,
        HEIGHT as u32,
    );
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    let mut rgb = Vec::with_capacity(WIDTH * HEIGHT * 3);
    for &Rgb565Pixel(v) in pixels {
        let r = ((v >> 11) & 31) as u8;
        let g = ((v >> 5) & 63) as u8;
        let b = (v & 31) as u8;
        rgb.extend_from_slice(&[
            (r << 3) | (r >> 2),
            (g << 2) | (g >> 4),
            (b << 3) | (b >> 2),
        ]);
    }
    writer.write_image_data(&rgb)?;
    Ok(())
}
