use std::{collections::VecDeque, ops::Range, sync::Arc};

use wgpu::{BufferAddress, DynamicOffset, IndexFormat, RenderPass};

#[derive(Debug, Clone)]
pub enum RenderCommand {
    SetPipeline(Arc<wgpu::RenderPipeline>),
    ///
    /// pub fn set_bind_group(
    ///     _,
    ///    index: u32,
    ///    bind_group: &'a BindGroup,
    ///    offsets: &[DynamicOffset],
    /// );
    SetBindGroup(u32, Arc<wgpu::BindGroup>, Option<Vec<DynamicOffset>>),
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

pub fn process_draw_queue<'a>(rp: &'a mut RenderPass<'a>, queue: &'a Vec<RenderCommand>) {
    for cmd in queue.iter() {
        match cmd {
            RenderCommand::SetPipeline(pipeline) => rp.set_pipeline(&pipeline),
            RenderCommand::SetBindGroup(slot, bind_group, offsets) => {
                let offsets = match offsets {
                    Some(os) => os.as_slice(),
                    None => &[],
                };
                rp.set_bind_group(*slot, bind_group.as_ref(), offsets);
            }
            RenderCommand::SetBlendConstant(color) => rp.set_blend_constant(*color),
            RenderCommand::SetIndexBuffer(buffer, index_format) => {
                rp.set_index_buffer(buffer.slice(..), *index_format)
            }
            RenderCommand::SetVertexBuffer(slot, buffer) => {
                rp.set_vertex_buffer(*slot, buffer.slice(..))
            }
            RenderCommand::SetScissorRect(x, y, width, height) => {
                rp.set_scissor_rect(*x, *y, *width, *height)
            }
            RenderCommand::SetViewPort(x, y, w, h, min_depth, max_depth) => {
                rp.set_viewport(*x, *y, *w, *h, *min_depth, *max_depth)
            }
            RenderCommand::SetStencilReference(reference) => rp.set_stencil_reference(*reference),
            RenderCommand::Draw(vertices, instances) => {
                rp.draw(vertices.clone(), instances.clone())
            }
            RenderCommand::InsertDebugMarker(label) => rp.insert_debug_marker(label),
            RenderCommand::PushDebugGroup(label) => rp.push_debug_group(label),
            RenderCommand::PopDebugGroup => rp.pop_debug_group(),
            RenderCommand::DrawIndexed(indices, base_vertex, instances) => {
                rp.draw_indexed(indices.clone(), *base_vertex, instances.clone())
            }
            RenderCommand::DrawIndirect(indirect_buffer, indirect_offset) => {
                rp.draw_indirect(indirect_buffer, *indirect_offset)
            }
            RenderCommand::DrawIndexedIndirect(indirect_buffer, indirect_offset) => {
                rp.draw_indexed_indirect(indirect_buffer, *indirect_offset)
            }
            RenderCommand::ExecuteBundles() => todo!(),
        }
    }
}
