use std::{collections::VecDeque, ops::Range, sync::Arc};

use wgpu::{BufferAddress, DynamicOffset, IndexFormat};

pub enum RenderCommand {
    SetPipeline(Arc<wgpu::RenderPipeline>),
    ///
    /// pub fn set_bind_group(
    ///     _,
    ///    index: u32,
    ///    bind_group: &'a BindGroup,
    ///    offsets: &[DynamicOffset],
    /// );
    SetBindGroup(u32, wgpu::BindGroup, Vec<DynamicOffset>),
    /// pub fn set_blend_constant(_, color: Color)
    SetBlendConstant(wgpu::Color),
    /// pub fn set_index_buffer(&mut self, buffer_slice: BufferSlice<'a>, index_format: IndexFormat)
    SetIndexBuffer(Arc<wgpu::Buffer>, IndexFormat),
    /// pub fn set_vertex_buffer(&mut self, slot: u32, buffer_slice: BufferSlice<'a>)
    SetVertexBuffer(u32, Arc<wgpu::Buffer>),
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
    DrawIndirect(Arc<wgpu::Buffer>, BufferAddress),
    ///
    /// pub fn draw_indexed_indirect(
    ///    &mut self,
    ///    indirect_buffer: &'a Buffer,
    ///    indirect_offset: BufferAddress,
    /// )
    DrawIndexedIndirect(Arc<wgpu::Buffer>, BufferAddress),

    /// TODO ::
    /// pub fn execute_bundles<I: IntoIterator<Item = &'a RenderBundle> + 'a>(
    ///    &mut self,
    ///    render_bundles: I,
    /// )
    ExecuteBundles(),
}

pub struct CommandQueue {
    render: VecDeque<RenderCommand>,
}

impl CommandQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_cmd(&mut self, cmd: RenderCommand) {
        self.render.push_back(cmd);
    }
}
impl Default for CommandQueue {
    fn default() -> Self {
        Self {
            render: VecDeque::new(),
        }
    }
}
