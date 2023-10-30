use std::{ops::Range, rc::Rc};

use wgpu::{BufferAddress, DynamicOffset, IndexFormat};

use crate::gfx::{draw::DrawCtx, wgpu_util::texture::Texture, window::DeviceSurface};

#[derive(Debug, Clone)]
pub enum GpuCommand {
    SetPipeline(Rc<wgpu::RenderPipeline>),
    ///
    /// pub fn set_bind_group(
    ///     _,
    ///    index: u32,
    ///    bind_group: &'a BindGroup,
    ///    offsets: &[DynamicOffset],
    /// );
    SetBindGroup(u32, Rc<wgpu::BindGroup>, Option<Vec<DynamicOffset>>),
    // SetBindGroup_ {
    // index: u32,
    // bind_group: Rc<wgpu::BindGroup>,
    // offsets: Option<Vec<DynamicOffset>>,
    // },
    /// pub fn set_blend_constant(_, color: Color)
    SetBlendConstant(wgpu::Color),
    /// pub fn set_index_buffer(&mut self, buffer_slice: BufferSlice<'a>, index_format: IndexFormat)
    SetIndexBuffer(Rc<wgpu::Buffer>, IndexFormat),
    /// pub fn set_vertex_buffer(&mut self, slot: u32, buffer_slice: BufferSlice<'a>)
    SetVertexBuffer(u32, Rc<wgpu::Buffer>),
    /// pub fn set_scissor_rect(&mut self, x: u32, y: u32, width: u32, height: u32)
    SetScissorRect(u32, u32, u32, u32),
    /// pub fn set_viewport(&mut self, x: f32, y: f32, w: f32, h: f32, min_depth: f32, max_depth: f32)
    SetViewPort(f32, f32, f32, f32, f32, f32),
    /// pub fn set_stencil_reference(&mut self, reference: u32)
    SetStencilReference(u32),
    /// pub fn draw(&mut self, vertices: Range<u32>, instances: Range<u32>)
    Draw(Range<u32>, Range<u32>),
    /// pub fn insert_debug_marker(&mut self, label: &str)
    InsertDebugMarker(String),
    /// pub fn push_debug_group(&mut self, label: &str)
    PushDebugGroup(String),
    /// pub fn pop_debug_group(&mut self)
    PopDebugGroup,
    /// pub fn draw_indexed(&mut self, indices: Range<u32>, base_vertex: i32, instances: Range<u32>)
    DrawIndexed(Range<u32>, i32, Range<u32>),
    /// pub fn draw_indirect(&mut self, indirect_buffer: &'a Buffer, indirect_offset: BufferAddress)
    DrawIndirect(Rc<wgpu::Buffer>, BufferAddress),
    ///
    /// pub fn draw_indexed_indirect(
    ///    &mut self,
    ///    indirect_buffer: &'a Buffer,
    ///    indirect_offset: BufferAddress,
    /// )
    DrawIndexedIndirect(Rc<wgpu::Buffer>, BufferAddress),

    /// TODO ::
    /// pub fn execute_bundles<I: IntoIterator<Item = &'a RenderBundle> + 'a>(
    ///    &mut self,
    ///    render_bundles: I,
    /// )
    ExecuteBundles(),
}

#[derive(Debug, Clone)]
pub enum EncoderCommand {
    CopyBufferToBuffer {
        src: Rc<wgpu::Buffer>,
        src_offset: wgpu::BufferAddress,
        dst: Rc<wgpu::Buffer>,
        dst_offset: wgpu::BufferAddress,
        copy_size: wgpu::BufferAddress,
    },
    // TODO :: CopyBufferToTexture would require lifetime annotation...
    // im gunna skip the rest of them since im probably never going to use them...
}

#[derive(Debug, Clone)]
pub struct CommandQueue {
    pub draw_commands: Vec<GpuCommand>,
    pub encoder_commands: Vec<EncoderCommand>,
}

impl CommandQueue {
    pub fn new() -> Self {
        Self {
            draw_commands: Vec::new(),
            encoder_commands: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.draw_commands.clear();
        self.encoder_commands.clear();
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RenderPassOp {
    Clear(wgpu::Color),
    LoadFromMemory,
}

impl RenderPassOp {
    pub const CLEAR_BLACK: RenderPassOp = RenderPassOp::Clear(wgpu::Color::BLACK);
    pub const CLEAR_WHITE: RenderPassOp = RenderPassOp::Clear(wgpu::Color::WHITE);
}

#[derive(Clone, Debug)]
pub struct RenderPass {
    pub command_queue: CommandQueue,
    pub surface: Rc<DeviceSurface>,

    pub depth_texture: Rc<Texture>,
    pub op: RenderPassOp,
}

impl RenderPass {
    const DEFAULT_CLEAR_COLOR: wgpu::Color = wgpu::Color::BLACK;
    pub fn new(surface: &Rc<DeviceSurface>, depth_texture: &Rc<Texture>, op: RenderPassOp) -> Self {
        Self {
            command_queue: CommandQueue::new(),
            surface: surface.clone(),
            op,
            depth_texture: depth_texture.clone(),
        }
    }

    pub fn from_draw_ctx(ctx: &DrawCtx, op: RenderPassOp) -> Self {
        Self::new(&ctx.device, &ctx.depth_texture, op)
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.surface.create_command_encoder();

        {
            for cmd in self.command_queue.encoder_commands.iter() {
                match cmd {
                    EncoderCommand::CopyBufferToBuffer {
                        src,
                        src_offset,
                        dst,
                        dst_offset,
                        copy_size,
                    } => {
                        encoder.copy_buffer_to_buffer(
                            src,
                            *src_offset,
                            dst,
                            *dst_offset,
                            *copy_size,
                        );
                    }
                }
            }
        }

        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: match self.op {
                            RenderPassOp::Clear(color) => wgpu::LoadOp::Clear(color),
                            RenderPassOp::LoadFromMemory => wgpu::LoadOp::Load,
                        },
                        store: true,
                    },
                })],
                depth_stencil_attachment: {
                    Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.depth_texture.view,
                        depth_ops: Some(wgpu::Operations {
                            load: match self.op {
                                RenderPassOp::Clear(_) => wgpu::LoadOp::Clear(1.0),
                                RenderPassOp::LoadFromMemory => wgpu::LoadOp::Load,
                            },
                            store: true,
                        }),
                        stencil_ops: None,
                    })
                },
            });

            for cmd in self.command_queue.draw_commands.iter() {
                match cmd {
                    GpuCommand::SetPipeline(pipeline) => rp.set_pipeline(&pipeline),
                    GpuCommand::SetBindGroup(slot, bind_group, offsets) => {
                        let offsets = match offsets {
                            Some(os) => os.as_slice(),
                            None => &[],
                        };
                        rp.set_bind_group(*slot, bind_group.as_ref(), offsets);
                    }
                    GpuCommand::SetBlendConstant(color) => rp.set_blend_constant(*color),
                    GpuCommand::SetIndexBuffer(buffer, index_format) => {
                        rp.set_index_buffer(buffer.slice(..), *index_format)
                    }
                    GpuCommand::SetVertexBuffer(slot, buffer) => {
                        rp.set_vertex_buffer(*slot, buffer.slice(..))
                    }
                    GpuCommand::SetScissorRect(x, y, width, height) => {
                        rp.set_scissor_rect(*x, *y, *width, *height)
                    }
                    GpuCommand::SetViewPort(x, y, w, h, min_depth, max_depth) => {
                        rp.set_viewport(*x, *y, *w, *h, *min_depth, *max_depth)
                    }
                    GpuCommand::SetStencilReference(reference) => {
                        rp.set_stencil_reference(*reference)
                    }
                    GpuCommand::Draw(vertices, instances) => {
                        rp.draw(vertices.clone(), instances.clone())
                    }
                    GpuCommand::InsertDebugMarker(label) => rp.insert_debug_marker(label),
                    GpuCommand::PushDebugGroup(label) => rp.push_debug_group(label),
                    GpuCommand::PopDebugGroup => rp.pop_debug_group(),
                    GpuCommand::DrawIndexed(indices, base_vertex, instances) => {
                        rp.draw_indexed(indices.clone(), *base_vertex, instances.clone())
                    }
                    GpuCommand::DrawIndirect(indirect_buffer, indirect_offset) => {
                        rp.draw_indirect(indirect_buffer, *indirect_offset)
                    }
                    GpuCommand::DrawIndexedIndirect(indirect_buffer, indirect_offset) => {
                        rp.draw_indexed_indirect(indirect_buffer, *indirect_offset)
                    }
                    GpuCommand::ExecuteBundles() => todo!(),
                }
            }
        }
        self.command_queue.clear();

        self.surface.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(())
    }
}
