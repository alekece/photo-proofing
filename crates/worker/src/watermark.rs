//! Draws a studio [`Watermark`] over a proof.
//!
//! Like the rest of rendering it is blocking and needs libvips with pango for text. The mark is
//! white with a soft dark shadow, so it reads on light and dark photos alike.

use std::fmt::Debug;

use libvips::{
    VipsImage, VipsInterpolate,
    ops::{
        self, Align, ArrayjoinOptions, BandFormat, BlendMode, Composite2Options, CopyOptions,
        ExtractBandOptions, Interpretation, RotateOptions,
    },
};
use pp_database::settings::{GridCell, Watermark, WatermarkPlacement};

use crate::render::RenderError;

/// Width of the mark as a share of the photo's width, per size step, in percent.
const WIDTH_SHARES: [i32; 10] = [10, 13, 16, 20, 24, 29, 34, 41, 48, 58];
/// The smallest mark width, so a tiny thumbnail still shows something.
const MIN_MARK_WIDTH: i32 = 24;
/// Rendering size of the studio name, in 1/1024 points, before it is scaled to the mark width.
const TEXT_SIZE: u32 = 64 * 1024;
/// Tallest a logo may be, relative to the mark width.
const LOGO_MAX_HEIGHT: f64 = 0.5;
/// Tallest the studio name may be, relative to the mark width, so short names stay modest.
const TEXT_MAX_HEIGHT: f64 = 0.2;
/// Opacity of the mark itself, as a fraction.
const MARK_OPACITY: f64 = 0.6;
/// Opacity of the shadow behind it, as a fraction.
const SHADOW_OPACITY: f64 = 0.55;
/// Distance of a single mark from the photo's edge, relative to the shorter side.
const EDGE_MARGIN: f64 = 0.03;
/// Space between repeated marks, relative to the mark's size.
const REPEAT_GAP: (f64, f64) = (0.6, 1.2);
/// How far repeated marks are turned, in degrees clockwise: the same slant as the Settings
/// preview.
const REPEAT_ANGLE: f64 = -24.0;

/// Paints one watermark onto photos.
pub struct WatermarkPainter<'a> {
    watermark: &'a Watermark,
}

impl<'a> WatermarkPainter<'a> {
    pub const fn new(watermark: &'a Watermark) -> Self {
        Self { watermark }
    }

    /// Returns `image`, an sRGB photo without alpha, with the watermark drawn on it.
    pub fn paint(&self, image: &VipsImage) -> Result<VipsImage, RenderError> {
        let (width, height) = (image.get_width(), image.get_height());
        let mark = self.mark(width)?;
        let image =
            &Self::as_srgb(&ops::bandjoin_const(image, &mut [255.0]).map_err(Self::fault)?)?;
        let painted = match self.watermark.placement {
            WatermarkPlacement::Once(cell) => {
                let (x, y) = Self::cell_origin(cell, (width, height), &mark);

                ops::composite2_with_opts(
                    image,
                    &mark,
                    BlendMode::Over,
                    &Composite2Options {
                        x,
                        y,
                        ..Composite2Options::default()
                    },
                )
            }
            WatermarkPlacement::Repeat => Self::paint_repeated(image, &mark),
        }
        .map_err(Self::fault)?;

        ops::extract_band_with_opts(&painted, 0, &ExtractBandOptions { n: 3 }).map_err(Self::fault)
    }

    fn fault(error: impl Debug) -> RenderError {
        RenderError::Library(format!("watermark: {error:?}"))
    }

    /// The mark, shadow included, sized for a photo `width` pixels wide.
    fn mark(&self, width: i32) -> Result<VipsImage, RenderError> {
        let share = WIDTH_SHARES[usize::from(self.watermark.size.get())];
        let target = (width * share / 100).max(MIN_MARK_WIDTH);
        let padding = (target / 25).max(2);
        let mut parts = Vec::new();

        if let Some(png) = &self.watermark.logo {
            parts.push(Self::logo(png, target)?);
        }

        if let Some(text) = &self.watermark.text {
            parts.push(Self::text(text, target)?);
        }

        let mark = if parts.len() == 1 {
            parts.remove(0)
        } else {
            // Spacing is the size of each grid cell, which libvips would otherwise take as 1.
            let cell = |size: fn(&VipsImage) -> i32| parts.iter().map(size).max().unwrap_or(1);
            let (cell_width, cell_height) =
                (cell(VipsImage::get_width), cell(VipsImage::get_height));

            ops::arrayjoin_with_opts(
                &mut parts,
                &ArrayjoinOptions {
                    across: 1,
                    shim: padding,
                    halign: Align::Centre,
                    valign: Align::Centre,
                    hspacing: cell_width,
                    vspacing: cell_height,
                    background: vec![0.0, 0.0, 0.0, 0.0],
                },
            )
            .map_err(Self::fault)?
        };
        let mark = ops::embed(
            &Self::as_srgb(&mark)?,
            padding,
            padding,
            mark.get_width() + 2 * padding,
            mark.get_height() + 2 * padding,
        )
        .map_err(Self::fault)?;

        Self::with_shadow(&mark, f64::from((target / 80).max(1)))
    }

    /// The logo as a white-faded RGBA image, no wider than `target`.
    fn logo(png: &[u8], target: i32) -> Result<VipsImage, RenderError> {
        let logo = VipsImage::new_from_buffer(png, "").map_err(Self::fault)?;
        let logo = if matches!(logo.get_interpretation(), Ok(Interpretation::Srgb)) {
            logo
        } else {
            ops::colourspace(&logo, Interpretation::Srgb).map_err(Self::fault)?
        };
        let logo = if logo.image_hasalpha() {
            logo
        } else {
            ops::bandjoin_const(&logo, &mut [255.0]).map_err(Self::fault)?
        };
        let (width, height) = (f64::from(logo.get_width()), f64::from(logo.get_height()));
        let scale = (f64::from(target) / width).min(f64::from(target) * LOGO_MAX_HEIGHT / height);
        let logo = ops::resize(&logo, scale).map_err(Self::fault)?;
        let colour = ops::extract_band_with_opts(&logo, 0, &ExtractBandOptions { n: 3 })
            .map_err(Self::fault)?;
        let alpha = ops::extract_band(&logo, 3).map_err(Self::fault)?;

        Self::join(&colour, &Self::faded(&alpha, MARK_OPACITY)?)
    }

    /// The studio name as white RGBA text, no wider than `target`.
    fn text(text: &str, target: i32) -> Result<VipsImage, RenderError> {
        // The typed `text_with_opts` passes the output-only `autofit-dpi` as an input, which
        // crashes libvips, so styling goes through pango markup in the plain call.
        let escaped = text
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        let mask = ops::text(&format!(
            "<b><span size=\"{TEXT_SIZE}\">{escaped}</span></b>"
        ))
        .map_err(Self::fault)?;
        let (width, height) = (f64::from(mask.get_width()), f64::from(mask.get_height()));
        let scale = (f64::from(target) / width).min(f64::from(target) * TEXT_MAX_HEIGHT / height);
        let mask = ops::resize(&mask, scale).map_err(Self::fault)?;
        let colour = Self::flat_colour(&mask, 255.0)?;

        Self::join(&colour, &Self::faded(&mask, MARK_OPACITY)?)
    }

    /// Three bands of `value`, as large as `like`.
    fn flat_colour(like: &VipsImage, value: f64) -> Result<VipsImage, RenderError> {
        let mut bands = Vec::new();

        for _ in 0..3 {
            bands.push(VipsImage::new_from_image1(like, value).map_err(Self::fault)?);
        }

        ops::bandjoin(&mut bands).map_err(Self::fault)
    }

    /// `alpha` scaled by `opacity`, as bytes.
    fn faded(alpha: &VipsImage, opacity: f64) -> Result<VipsImage, RenderError> {
        let alpha = ops::cast(alpha, BandFormat::Float).map_err(Self::fault)?;
        let factor = VipsImage::new_from_image1(&alpha, opacity).map_err(Self::fault)?;
        let faded = ops::multiply(&alpha, &factor).map_err(Self::fault)?;

        ops::cast(&faded, BandFormat::Uchar).map_err(Self::fault)
    }

    /// An RGBA image from three colour bands and an alpha band.
    fn join(colour: &VipsImage, alpha: &VipsImage) -> Result<VipsImage, RenderError> {
        let rgba = ops::bandjoin(&mut [
            ops::extract_band(colour, 0).map_err(Self::fault)?,
            ops::extract_band(colour, 1).map_err(Self::fault)?,
            ops::extract_band(colour, 2).map_err(Self::fault)?,
            ops::copy(alpha).map_err(Self::fault)?,
        ])
        .map_err(Self::fault)?;

        Self::as_srgb(&rgba)
    }

    /// `image`, a byte image, labelled sRGB: bands joined by hand are only `multiband`, which
    /// compositing cannot convert.
    fn as_srgb(image: &VipsImage) -> Result<VipsImage, RenderError> {
        ops::copy_with_opts(
            image,
            &CopyOptions {
                width: image.get_width(),
                height: image.get_height(),
                bands: image.get_bands(),
                format: BandFormat::Uchar,
                interpretation: Interpretation::Srgb,
                xres: 1.0,
                yres: 1.0,
                ..CopyOptions::default()
            },
        )
        .map_err(Self::fault)
    }

    /// `mark` over a blurred dark copy of itself.
    fn with_shadow(mark: &VipsImage, sigma: f64) -> Result<VipsImage, RenderError> {
        let alpha = ops::extract_band(mark, 3).map_err(Self::fault)?;
        let blurred = ops::gaussblur(&alpha, sigma).map_err(Self::fault)?;
        let shadow = Self::join(
            &Self::flat_colour(&blurred, 0.0)?,
            &Self::faded(&blurred, SHADOW_OPACITY)?,
        )?;

        ops::composite2(&shadow, mark, BlendMode::Over).map_err(Self::fault)
    }

    /// The top-left corner that puts `mark` in `cell` of the 3 × 3 grid over the photo.
    fn cell_origin(cell: GridCell, (width, height): (i32, i32), mark: &VipsImage) -> (i32, i32) {
        let margin = (f64::from(width.min(height)) * EDGE_MARGIN).round() as i32;
        let spot = |cell_index: u8, photo: i32, mark: i32| {
            let room = (photo - 2 * margin - mark).max(0);

            margin + room * i32::from(cell_index) / 2
        };

        (
            spot(cell.get() % 3, width, mark.get_width()),
            spot(cell.get() / 3, height, mark.get_height()),
        )
    }

    /// `image` with `mark` copied over it on a staggered lattice turned by [`REPEAT_ANGLE`] about
    /// the photo's centre. The mark is turned once, so its edges are resampled a single time and
    /// stay sharp, instead of resampling a photo-sized overlay.
    fn paint_repeated(image: &VipsImage, mark: &VipsImage) -> libvips::Result<VipsImage> {
        let (width, height) = (image.get_width(), image.get_height());
        let turned = Self::turned(mark)?;
        let (turned_width, turned_height) = (turned.get_width(), turned.get_height());
        let step_x = f64::from(mark.get_width()) * (1.0 + REPEAT_GAP.0);
        let step_y = f64::from(mark.get_height()) * (1.0 + REPEAT_GAP.1);
        let (centre_x, centre_y) = (f64::from(width) / 2.0, f64::from(height) / 2.0);
        let (sin, cos) = REPEAT_ANGLE.to_radians().sin_cos();
        // Half the diagonal reaches every corner from the centre, whatever the angle.
        let reach = centre_x.hypot(centre_y);
        let columns = (reach / step_x).ceil() as i32 + 1;
        let rows = (reach / step_y).ceil() as i32 + 1;
        // `VipsImage::clone` copies the handle without taking a reference, so take a real one.
        let mut painted = ops::copy(image)?;

        for row in -rows..=rows {
            // Every other row is shifted by half a step.
            let shift = if row.rem_euclid(2) == 1 { 0.5 } else { 0.0 };

            for column in -columns..=columns {
                let x = (f64::from(column) + shift) * step_x;
                let y = f64::from(row) * step_y;
                let left = (centre_x + x * cos - y * sin).round() as i32 - turned_width / 2;
                let top = (centre_y + x * sin + y * cos).round() as i32 - turned_height / 2;
                let visible = left < width
                    && top < height
                    && left + turned_width > 0
                    && top + turned_height > 0;

                if visible {
                    painted = ops::composite2_with_opts(
                        &painted,
                        &turned,
                        BlendMode::Over,
                        &Composite2Options {
                            x: left,
                            y: top,
                            ..Composite2Options::default()
                        },
                    )?;
                }
            }
        }

        Ok(painted)
    }

    /// `mark` turned by [`REPEAT_ANGLE`] on a transparent background, with a bicubic filter and
    /// premultiplied alpha so the edges get no dark fringe. Every option is spelled out because
    /// dropping `RotateOptions::default()` releases libvips' shared nearest-neighbour filter.
    fn turned(mark: &VipsImage) -> libvips::Result<VipsImage> {
        let premultiplied = ops::premultiply(mark)?;
        let turned = ops::rotate_with_opts(
            &premultiplied,
            REPEAT_ANGLE,
            &RotateOptions {
                interpolate: VipsInterpolate::new_from_name("bicubic")?,
                background: vec![0.0, 0.0, 0.0, 0.0],
                odx: 0.0,
                ody: 0.0,
                idx: 0.0,
                idy: 0.0,
            },
        )?;
        let straight = ops::cast(&ops::unpremultiply(&turned)?, BandFormat::Uchar)?;

        ops::copy_with_opts(
            &straight,
            &CopyOptions {
                width: straight.get_width(),
                height: straight.get_height(),
                bands: straight.get_bands(),
                format: BandFormat::Uchar,
                interpretation: Interpretation::Srgb,
                xres: 1.0,
                yres: 1.0,
                ..CopyOptions::default()
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use libvips::ops::{avg, black, jpegsave_buffer, pngsave_buffer};
    use pp_database::settings::SizeStep;

    use super::*;
    use crate::render::tests::init_libvips;

    const WIDTH: i32 = 900;
    const HEIGHT: i32 = 600;

    fn watermark(text: Option<&str>, logo: bool, placement: WatermarkPlacement) -> Watermark {
        Watermark {
            text: text.map(str::to_owned),
            logo: logo.then(white_logo),
            placement,
            size: SizeStep::try_from(2).unwrap(),
        }
    }

    fn white_logo() -> Vec<u8> {
        init_libvips();

        let solid = black(80, 40).unwrap();
        let white = VipsImage::new_from_image1(&solid, 255.0).unwrap();
        let rgba = ops::bandjoin(&mut [
            VipsImage::new_from_image1(&solid, 255.0).unwrap(),
            VipsImage::new_from_image1(&solid, 255.0).unwrap(),
            white,
            VipsImage::new_from_image1(&solid, 255.0).unwrap(),
        ])
        .unwrap();
        let rgba = WatermarkPainter::as_srgb(&rgba).unwrap();

        pngsave_buffer(&rgba).unwrap()
    }

    fn painted(watermark: &Watermark) -> VipsImage {
        init_libvips();

        let photo = black(WIDTH, HEIGHT).unwrap();
        let photo = ops::colourspace(&photo, Interpretation::Srgb).unwrap();
        let photo = ops::bandjoin(&mut [
            ops::extract_band(&photo, 0).unwrap(),
            ops::extract_band(&photo, 0).unwrap(),
            ops::extract_band(&photo, 0).unwrap(),
        ])
        .unwrap();
        let photo = WatermarkPainter::as_srgb(&photo).unwrap();

        WatermarkPainter::new(watermark).paint(&photo).unwrap()
    }

    /// Mean brightness of the `third`-th column and row of a 3 × 3 split of `image`.
    fn brightness(image: &VipsImage, cell: u8) -> f64 {
        let (w, h) = (image.get_width() / 3, image.get_height() / 3);
        let area = ops::extract_area(
            image,
            w * i32::from(cell % 3),
            h * i32::from(cell / 3),
            w,
            h,
        )
        .unwrap();

        avg(&area).unwrap()
    }

    fn cells(image: &VipsImage) -> Vec<bool> {
        (0..9).map(|cell| brightness(image, cell) > 0.5).collect()
    }

    #[test]
    fn a_single_name_lands_in_its_grid_cell_only() {
        for cell in [0, 4, 8] {
            let mark = watermark(
                Some("Studio"),
                false,
                WatermarkPlacement::Once(GridCell::try_from(cell).unwrap()),
            );
            let lit = cells(&painted(&mark));

            for (index, lit) in lit.into_iter().enumerate() {
                assert_eq!(
                    lit,
                    index == usize::from(cell),
                    "cell {cell}, index {index}"
                );
            }
        }
    }

    #[test]
    fn a_repeated_name_covers_the_whole_photo() {
        let image = painted(&watermark(
            Some("Studio"),
            false,
            WatermarkPlacement::Repeat,
        ));

        assert_eq!(image.get_width(), WIDTH);
        assert_eq!(image.get_height(), HEIGHT);
        assert_eq!(image.get_bands(), 3);
        assert!(
            cells(&image).into_iter().all(|lit| lit),
            "{:?}",
            cells(&image)
        );
    }

    /// Which rows of the `width`-pixel-wide strip of `image` starting at column `left` hold a lit
    /// pixel.
    fn lit_rows(image: &VipsImage, left: i32, width: i32) -> Vec<bool> {
        let strip = ops::extract_area(image, left, 0, width, image.get_height()).unwrap();
        let pixels = strip.image_write_to_memory();

        pixels
            .chunks(width as usize * 3)
            .map(|row| row.iter().any(|&value| value > 128))
            .collect()
    }

    #[test]
    fn a_repeated_name_runs_along_the_diagonal_of_the_preview() {
        let image = painted(&watermark(
            Some("Studio"),
            false,
            WatermarkPlacement::Repeat,
        ));
        let left = lit_rows(&image, 0, 120);
        let right = lit_rows(&image, WIDTH - 120, 120);
        let both = left.iter().zip(&right).filter(|(l, r)| **l && **r).count();
        let either = left.iter().zip(&right).filter(|(l, r)| **l || **r).count();
        let overlap = both as f64 / either as f64;

        // Axis-aligned rows would light the very same pixel rows on both sides.
        assert!(left.iter().filter(|lit| **lit).count() > 40);
        assert!(overlap < 0.7, "marks share {overlap} of their rows");
    }

    #[test]
    fn a_logo_is_drawn_alone_or_above_the_name() {
        let once = WatermarkPlacement::Once(GridCell::try_from(4).unwrap());
        // libvips reads the logo buffer lazily, so the watermarks must outlive the images.
        let (logo_mark, both_mark, name_mark) = (
            watermark(None, true, once),
            watermark(Some("Studio"), true, once),
            watermark(Some("Studio"), false, once),
        );
        let (logo, both, name) = (
            painted(&logo_mark),
            painted(&both_mark),
            painted(&name_mark),
        );

        assert_eq!(cells(&logo).iter().filter(|lit| **lit).count(), 1);
        assert!(cells(&logo)[4]);
        assert!(brightness(&both, 4) > brightness(&name, 4));
    }

    #[test]
    fn a_larger_size_step_draws_more() {
        let once = WatermarkPlacement::Once(GridCell::try_from(4).unwrap());
        let mut small = watermark(Some("Studio"), false, once);
        let mut large = small.clone();

        small.size = SizeStep::try_from(0).unwrap();
        large.size = SizeStep::try_from(9).unwrap();

        assert!(brightness(&painted(&large), 4) > brightness(&painted(&small), 4));
    }

    #[test]
    fn a_mark_survives_jpeg_encoding_of_a_small_thumbnail() {
        init_libvips();

        let photo = ops::colourspace(&black(120, 80).unwrap(), Interpretation::Srgb).unwrap();
        let photo = ops::bandjoin(&mut [
            ops::extract_band(&photo, 0).unwrap(),
            ops::extract_band(&photo, 0).unwrap(),
            ops::extract_band(&photo, 0).unwrap(),
        ])
        .unwrap();
        let mark = watermark(
            Some("A very long studio name indeed"),
            false,
            WatermarkPlacement::Repeat,
        );
        let image = WatermarkPainter::new(&mark).paint(&photo).unwrap();

        assert!(avg(&image).unwrap() > 0.5);
        assert!(!jpegsave_buffer(&image).unwrap().is_empty());
    }
}
