use std::{ops::Range, rc::Rc};

use wgpu::{BufferAddress, DynamicOffset, IndexFormat};

use crate::eng::command::{EncoderCommand, GpuCommand, RenderPass, RenderPassOp};

use super::{
    geom::QuadBuffer,
    shader::Shader,
    wgpu_util::{
        buffer::{BufferType, StagingBuffer},
        texture::Texture,
    },
    window::DeviceSurface,
};

pub struct DrawCtx {
    passes: Vec<RenderPass>,
    pub device: Rc<DeviceSurface>,
    pub depth_texture: Rc<Texture>,
}

impl DrawCtx {
    pub fn submit(mut self) -> Result<(), wgpu::SurfaceError> {
        for pass in self.passes.iter_mut() {
            pass.render()?
        }
        Ok(())
    }

    pub fn copy_buffer(
        &mut self,
        src: &Rc<wgpu::Buffer>,
        src_offset: u64,
        dst: &Rc<wgpu::Buffer>,
        dst_offset: u64,
        copy_size: u64,
    ) {
        self.current_pass_mut().command_queue.encoder_commands.push(
            EncoderCommand::CopyBufferToBuffer {
                src: src.clone(),
                src_offset,
                dst: dst.clone(),
                dst_offset,
                copy_size,
            },
        );
    }

    pub fn write_buffer(&self, dst: &Rc<wgpu::Buffer>, offset: u64, data: &[u8]) {
        self.device.queue.write_buffer(dst.as_ref(), offset, data);
    }
    // pub fn command_queue(&self) -> &Vec<RenderCommand> {
    // &self.current_pass_mut().command_queue
    // }
    //

    // pub fn command_queue(&self) -> &Vec<RenderCommand> {
    // &self.current_pass_mut().command_queue
    // }

    pub fn begin_render_pass(&mut self, op: RenderPassOp) {
        self.passes.push(RenderPass::from_draw_ctx(self, op));
    }

    pub fn current_pass_mut(&mut self) -> &mut RenderPass {
        self.passes
            .last_mut()
            .expect("DrawCtx::current_pass => RenderPass queue on DrawCtx empty")
    }

    pub fn new(device: &Rc<DeviceSurface>, depth_texture: &Rc<Texture>) -> Self {
        Self {
            device: device.clone(),
            passes: Vec::new(),
            depth_texture: depth_texture.clone(),
        }
    }

    pub fn set_pipeline(&mut self, pipeline: &Rc<wgpu::RenderPipeline>) {
        self.current_pass_mut()
            .command_queue
            .draw_commands
            .push(GpuCommand::SetPipeline(pipeline.clone()));
    }

    pub fn set_bind_group(
        &mut self,
        index: u32,
        bind_group: &Rc<wgpu::BindGroup>,
        offsets: Option<Vec<DynamicOffset>>,
    ) {
        self.current_pass_mut()
            .command_queue
            .draw_commands
            .push(GpuCommand::SetBindGroup(index, bind_group.clone(), offsets));
    }

    pub fn set_blend_constant(&mut self, color: wgpu::Color) {
        self.current_pass_mut()
            .command_queue
            .draw_commands
            .push(GpuCommand::SetBlendConstant(color));
    }

    pub fn set_index_buffer(&mut self, buffer: &Rc<wgpu::Buffer>, format: IndexFormat) {
        self.current_pass_mut()
            .command_queue
            .draw_commands
            .push(GpuCommand::SetIndexBuffer(buffer.clone(), format));
    }

    pub fn set_vertex_buffer(&mut self, slot: u32, buffer: &Rc<wgpu::Buffer>) {
        self.current_pass_mut()
            .command_queue
            .draw_commands
            .push(GpuCommand::SetVertexBuffer(slot, buffer.clone()));
    }

    pub fn set_scissor_rect(&mut self, x: u32, y: u32, width: u32, height: u32) {
        self.current_pass_mut()
            .command_queue
            .draw_commands
            .push(GpuCommand::SetScissorRect(x, y, width, height));
    }

    pub fn set_viewport(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        min_depth: f32,
        max_depth: f32,
    ) {
        self.current_pass_mut()
            .command_queue
            .draw_commands
            .push(GpuCommand::SetViewPort(
                x, y, width, height, min_depth, max_depth,
            ));
    }

    pub fn set_stencil_reference(&mut self, reference: u32) {
        self.current_pass_mut()
            .command_queue
            .draw_commands
            .push(GpuCommand::SetStencilReference(reference));
    }

    pub fn draw(&mut self, verticies: Range<u32>, instances: Range<u32>) {
        self.current_pass_mut()
            .command_queue
            .draw_commands
            .push(GpuCommand::Draw(verticies, instances));
    }

    pub fn insert_debug_marker(&mut self, label: &str) {
        self.current_pass_mut()
            .command_queue
            .draw_commands
            .push(GpuCommand::InsertDebugMarker(String::from(label)))
    }

    pub fn push_debug_group(&mut self, label: &str) {
        self.current_pass_mut()
            .command_queue
            .draw_commands
            .push(GpuCommand::PushDebugGroup(String::from(label)));
    }

    pub fn pop_debug_group(&mut self) {
        self.current_pass_mut()
            .command_queue
            .draw_commands
            .push(GpuCommand::PopDebugGroup);
    }

    pub fn draw_indexed(&mut self, indices: Range<u32>, base_vertex: i32, instances: Range<u32>) {
        self.current_pass_mut()
            .command_queue
            .draw_commands
            .push(GpuCommand::DrawIndexed(indices, base_vertex, instances));
    }

    pub fn draw_indirect(
        &mut self,
        indirect_buffer: &Rc<wgpu::Buffer>,
        indirect_offset: BufferAddress,
    ) {
        self.current_pass_mut()
            .command_queue
            .draw_commands
            .push(GpuCommand::DrawIndirect(
                indirect_buffer.clone(),
                indirect_offset,
            ));
    }

    pub fn draw_indexed_indirect(
        &mut self,
        indirect_buffer: &Rc<wgpu::Buffer>,
        indirect_offset: BufferAddress,
    ) {
        self.current_pass_mut()
            .command_queue
            .draw_commands
            .push(GpuCommand::DrawIndexedIndirect(
                indirect_buffer.clone(),
                indirect_offset,
            ));
    }

    pub fn use_shader(&mut self, shader: &Shader) {
        for (i, u) in shader.uniforms.iter().enumerate() {
            self.set_bind_group(i as u32, &u.bgroup, None);
        }
        self.set_pipeline(&shader.pipeline);
    }

    /// Uses Vertex Buffer slot 0. Assumes we are not using instancing.
    /// also creates a Stagingbuffer so we can send CPU data in Quadbuffer to GPU.
    pub fn draw_quad_buffer(&mut self, qb: &QuadBuffer) {
        let device = self.device.as_ref();

        // TODO/WARN :: This might not work or at the very least is really slow... We are creating a new
        // buffer (potentiall every frame) and sending that newly created buffer to the GPU.
        // Ideally we already have a vert/index buffer on the renderer that we should be copying
        // to...
        let vert_sb = StagingBuffer::new(device, qb.vertex_buffer(), BufferType::Vertex);
        let index_sb = StagingBuffer::new(device, qb.index_buffer(), BufferType::Index);

        let indices = qb.index_buffer().len() as u32;

        self.set_vertex_buffer(0, &vert_sb.buf);
        self.set_index_buffer(&index_sb.buf, IndexFormat::Uint32);
        self.draw_indexed(0..indices, 0, 0..1);
    }
}
