//! Shared offscreen rendering and buffer blitting helpers.

use render::area::{Area, Size};
use render::buffer::{Buffer, Cell};
use render::chunk::Chunk;
use render::style::Stylized;

/// Options controlling how cells are copied from an offscreen buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct BlitOptions {
    skip_blank: bool,
}

impl BlitOptions {
    /// Skip cells that do not carry visible content or styling.
    pub(crate) fn skip_blank(mut self) -> Self {
        self.skip_blank = true;
        self
    }
}

/// Source and destination rectangle for a buffer copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BlitRegion {
    pub(crate) source_x: usize,
    pub(crate) source_y: usize,
    pub(crate) dest_x: i32,
    pub(crate) dest_y: i32,
    pub(crate) width: usize,
    pub(crate) height: usize,
}

impl BlitRegion {
    pub(crate) fn new(
        source_x: usize,
        source_y: usize,
        dest_x: i32,
        dest_y: i32,
        width: usize,
        height: usize,
    ) -> Self {
        Self {
            source_x,
            source_y,
            dest_x,
            dest_y,
            width,
            height,
        }
    }
}

/// A cell selected for blitting after source/destination clipping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BlitCell {
    pub(crate) source_x: u16,
    pub(crate) source_y: u16,
    pub(crate) dest_x: u16,
    pub(crate) dest_y: u16,
    pub(crate) content: Stylized,
}

/// Render arbitrary content into a fresh offscreen buffer.
pub(crate) fn render_to_offscreen(
    area: Area,
    render: impl FnOnce(&mut Chunk<'_>),
) -> Option<Buffer> {
    let mut buffer = Buffer::new(area.real_size());
    {
        let mut chunk = Chunk::new(&mut buffer, area).ok()?;
        render(&mut chunk);
    }
    Some(buffer)
}

/// Copy a clipped region from an offscreen buffer into a target chunk.
pub(crate) fn blit_region(
    target: &mut Chunk<'_>,
    source: &Buffer,
    region: BlitRegion,
    options: BlitOptions,
) {
    let target_size = target.area().size();
    for_each_blit_cell(source, region, Some(target_size), options, |cell| {
        let _ = target.set_forced(cell.dest_x, cell.dest_y, cell.content);
    });
}

/// Visit all cells in a blit region after source/destination clipping.
pub(crate) fn for_each_blit_cell(
    source: &Buffer,
    region: BlitRegion,
    dest_size: Option<Size>,
    options: BlitOptions,
    mut visit: impl FnMut(BlitCell),
) {
    let source_size = source.size();
    let source_width = source_size.width as usize;
    let source_height = source_size.height as usize;

    if source_width == 0 || source_height == 0 || region.width == 0 || region.height == 0 {
        return;
    }

    for row in 0..region.height {
        let source_y = region.source_y + row;
        if source_y >= source_height {
            break;
        }

        let dest_y = region.dest_y + row as i32;
        if !is_inside_axis(dest_y, dest_size.map(|size| size.height)) {
            continue;
        }

        for col in 0..region.width {
            let source_x = region.source_x + col;
            if source_x >= source_width {
                break;
            }

            let dest_x = region.dest_x + col as i32;
            if !is_inside_axis(dest_x, dest_size.map(|size| size.width)) {
                continue;
            }

            let index = source_y * source_width + source_x;
            let Some(cell) = source.content().get(index) else {
                continue;
            };
            if should_skip_cell(cell, options) {
                continue;
            }

            visit(BlitCell {
                source_x: source_x as u16,
                source_y: source_y as u16,
                dest_x: dest_x as u16,
                dest_y: dest_y as u16,
                content: cell.content,
            });
        }
    }
}

fn should_skip_cell(cell: &Cell, options: BlitOptions) -> bool {
    cell.is_occupied || (options.skip_blank && is_blank_cell(cell))
}

fn is_blank_cell(cell: &Cell) -> bool {
    match cell.content.c {
        None => true,
        Some(' ') => !cell.content.has_color() && !cell.content.has_attr(),
        Some(_) => false,
    }
}

fn is_inside_axis(value: i32, max: Option<u16>) -> bool {
    if value < 0 || value > u16::MAX as i32 {
        return false;
    }

    max.map(|max| value < max as i32).unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_to_offscreen_preserves_area_coordinates() {
        let area = Area::new((2, 1).into(), (3, 2).into());
        let buffer = render_to_offscreen(area, |chunk| {
            let _ = chunk.set_forced(0, 0, Stylized::plain('x'));
        })
        .unwrap();

        assert_eq!(buffer.size(), (5, 3).into());
        assert_eq!(cell_char(&buffer, 2, 1), Some('x'));
    }

    #[test]
    fn blit_region_clips_source_and_destination_bounds() {
        let mut source = Buffer::new((4, 1).into());
        let _ = source.overwrite((1, 0).into(), Stylized::plain('b'));
        let _ = source.overwrite((2, 0).into(), Stylized::plain('c'));
        let _ = source.overwrite((3, 0).into(), Stylized::plain('d'));

        let mut target = Buffer::new((3, 1).into());
        {
            let mut chunk =
                Chunk::new(&mut target, Area::new((0, 0).into(), (3, 1).into())).unwrap();
            blit_region(
                &mut chunk,
                &source,
                BlitRegion::new(1, 0, 1, 0, 4, 1),
                BlitOptions::default(),
            );
        }

        assert_eq!(cell_char(&target, 0, 0), Some(' '));
        assert_eq!(cell_char(&target, 1, 0), Some('b'));
        assert_eq!(cell_char(&target, 2, 0), Some('c'));
    }

    #[test]
    fn blit_region_can_skip_blank_cells() {
        let source = Buffer::new((1, 1).into());
        let mut target = Buffer::new((1, 1).into());
        let _ = target.overwrite((0, 0).into(), Stylized::plain('z'));

        {
            let mut chunk =
                Chunk::new(&mut target, Area::new((0, 0).into(), (1, 1).into())).unwrap();
            blit_region(
                &mut chunk,
                &source,
                BlitRegion::new(0, 0, 0, 0, 1, 1),
                BlitOptions::default().skip_blank(),
            );
        }

        assert_eq!(cell_char(&target, 0, 0), Some('z'));
    }

    fn cell_char(buffer: &Buffer, x: u16, y: u16) -> Option<char> {
        let index = (y * buffer.size().width + x) as usize;
        buffer.content().get(index)?.content.c
    }
}
