use crate::platform::{HEIGHT, WIDTH};
use anyhow::{Context, Result, bail, ensure};
use drm::{
    Device as _,
    buffer::{Buffer, DrmFourcc},
    control::{self, Device as _, connector, crtc, dumbbuffer::DumbBuffer, framebuffer},
};
use slint::platform::software_renderer::{MinimalSoftwareWindow, RenderingRotation, Rgb565Pixel};
use std::{
    fs::{File, OpenOptions},
    os::fd::{AsFd, BorrowedFd},
    path::Path,
};

pub struct Card(File);
impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}
impl drm::Device for Card {}
impl control::Device for Card {}

pub struct Display {
    card: Card,
    buffer: Option<DumbBuffer>,
    framebuffer: Option<framebuffer::Handle>,
    saved: crtc::Info,
    saved_connectors: Vec<connector::Handle>,
    connector: connector::Handle,
    mode: control::Mode,
    active: bool,
}

impl Display {
    pub fn open(path: &Path) -> Result<Self> {
        let card = Card(
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(path)
                .with_context(|| format!("open {}", path.display()))?,
        );
        let resources = card.resource_handles().context("DRM resources")?;
        let mut selected = None;
        for &handle in resources.connectors() {
            let info = card.get_connector(handle, true)?;
            if info.state() != connector::State::Connected {
                continue;
            }
            // This binary intentionally targets the U60 panel, not the host monitor.
            let Some(mode) = info
                .modes()
                .iter()
                .find(|mode| mode.size() == (WIDTH as u16, HEIGHT as u16))
                .copied()
            else {
                continue;
            };
            let mut crtc = info
                .current_encoder()
                .and_then(|e| card.get_encoder(e).ok())
                .and_then(|e| e.crtc());
            if crtc.is_none() {
                for &encoder in info.encoders() {
                    let enc = card.get_encoder(encoder)?;
                    crtc = resources
                        .filter_crtcs(enc.possible_crtcs())
                        .first()
                        .copied();
                    if crtc.is_some() {
                        break;
                    }
                }
            }
            if let Some(crtc) = crtc {
                selected = Some((handle, mode, crtc));
                break;
            }
        }
        let Some((connector, mode, crtc)) = selected else {
            bail!("no connected 320x480 DRM panel with a compatible CRTC");
        };
        let saved = card.get_crtc(crtc)?;
        let mut saved_connectors = Vec::new();
        for &handle in resources.connectors() {
            let conn = card.get_connector(handle, false)?;
            if conn
                .current_encoder()
                .and_then(|e| card.get_encoder(e).ok())
                .and_then(|e| e.crtc())
                == Some(crtc)
            {
                saved_connectors.push(handle);
            }
        }
        card.acquire_master_lock()
            .context("DRM master is occupied; use the guarded device trial")?;
        let mut display = Self {
            card,
            buffer: None,
            framebuffer: None,
            saved,
            saved_connectors,
            connector,
            mode,
            active: false,
        };
        let buffer = display
            .card
            .create_dumb_buffer((WIDTH as u32, HEIGHT as u32), DrmFourcc::Rgb565, 16)
            .context("RGB565 dumb buffer")?;
        display.buffer = Some(buffer);
        ensure!(
            buffer.pitch() as usize >= WIDTH * 2 && buffer.pitch().is_multiple_of(2),
            "invalid RGB565 pitch"
        );
        display.framebuffer = Some(
            display
                .card
                .add_framebuffer(&buffer, 16, 16)
                .context("RGB565 framebuffer")?,
        );
        Ok(display)
    }

    pub fn run(
        &mut self,
        window: &MinimalSoftwareWindow,
        rotate: bool,
        mut tick: impl FnMut() -> Result<bool>,
    ) -> Result<()> {
        // Map once; keep the mapping borrowed from the buffer until the loop exits.
        let buffer = self.buffer.as_mut().expect("buffer created");
        let pitch = buffer.pitch() as usize;
        let mut mapping = self.card.map_dumb_buffer(buffer)?;
        ensure!(mapping.len() >= pitch * HEIGHT, "short DRM mapping");
        let fb = self.framebuffer.expect("framebuffer created");
        self.card
            .set_crtc(
                self.saved.handle(),
                Some(fb),
                (0, 0),
                &[self.connector],
                Some(self.mode),
            )
            .context("set panel CRTC")?;
        self.active = true;
        eprintln!(
            "display: 320x480 RGB565, pitch={pitch}, rotation={}",
            if rotate { 180 } else { 0 }
        );
        // Render into an owned buffer, then copy only damaged pixels to the mmap.
        // No alignment casts or unsafe aliasing of device memory are needed.
        let mut pixels = vec![Rgb565Pixel(0); WIDTH * HEIGHT];
        let mut presented = false;
        while tick()? {
            let mut result = Ok(());
            window.draw_if_needed(|renderer| {
                renderer.set_rendering_rotation(if rotate {
                    RenderingRotation::Rotate180
                } else {
                    RenderingRotation::NoRotation
                });
                let region = renderer.render(&mut pixels, WIDTH);
                let mut clips = Vec::new();
                for (origin, size) in region.iter() {
                    let x1 = origin.x.max(0) as usize;
                    let y1 = origin.y.max(0) as usize;
                    let x2 = (x1 + size.width as usize).min(WIDTH);
                    let y2 = (y1 + size.height as usize).min(HEIGHT);
                    if x2 <= x1 || y2 <= y1 {
                        continue;
                    }
                    copy_rect(&pixels, &mut mapping, pitch, (x1, y1, x2, y2));
                    // Slint damage is already in physical, rotated coordinates. x2/y2 are exclusive.
                    clips.push(control::ClipRect::new(
                        x1 as u16, y1 as u16, x2 as u16, y2 as u16,
                    ));
                }
                if !clips.is_empty() {
                    result = self
                        .card
                        .dirty_framebuffer(fb, &clips)
                        .context("DRM DIRTYFB refresh");
                    if result.is_ok() && !presented {
                        eprintln!("display: first frame submitted");
                        presented = true;
                    }
                }
            });
            result?;
        }
        Ok(())
    }
}

fn copy_rect(
    pixels: &[Rgb565Pixel],
    map: &mut [u8],
    pitch: usize,
    (x1, y1, x2, y2): (usize, usize, usize, usize),
) {
    for y in y1..y2 {
        for x in x1..x2 {
            let offset = y * pitch + x * 2;
            map[offset..offset + 2].copy_from_slice(&pixels[y * WIDTH + x].0.to_le_bytes());
        }
    }
}

impl Drop for Display {
    fn drop(&mut self) {
        if self.active
            && let Err(error) = self.card.set_crtc(
                self.saved.handle(),
                self.saved.framebuffer(),
                self.saved.position(),
                &self.saved_connectors,
                self.saved.mode(),
            )
        {
            eprintln!(
                "display: saved CRTC restore failed ({error}); trial guard will restart vendor UI"
            );
        }
        if let Some(fb) = self.framebuffer.take() {
            let _ = self.card.destroy_framebuffer(fb);
        }
        if let Some(buffer) = self.buffer.take() {
            let _ = self.card.destroy_dumb_buffer(buffer);
        }
        let _ = self.card.release_master_lock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn damaged_pixels_respect_pitch_and_exclusive_edges() {
        let pixels = vec![Rgb565Pixel(0x1234); WIDTH * HEIGHT];
        let pitch = WIDTH * 2 + 32;
        let mut mapping = vec![0xaa; pitch * HEIGHT];
        copy_rect(&pixels, &mut mapping, pitch, (319, 479, 320, 480));
        let offset = 479 * pitch + 319 * 2;
        assert_eq!(&mapping[offset..offset + 2], &[0x34, 0x12]);
        assert!(mapping[..offset].iter().all(|b| *b == 0xaa));
        assert!(mapping[offset + 2..].iter().all(|b| *b == 0xaa));
    }
}
