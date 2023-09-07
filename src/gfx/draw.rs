use std::{ops::Range, rc::Rc, sync::Arc};

use wgpu::{BufferAddress, DynamicOffset, IndexFormat};

use crate::eng::{
    command::RenderCommand,
    render::{
        light::{draw_light_mesh_instanced, draw_light_model_instanced},
        mesh::{draw_mesh_instanced, draw_model_instanced},
        RenderCamera, RenderWindow,
    },
};

use super::model::{Material, Mesh, Model};

pub struct DrawCtx {
    command_queue: Vec<RenderCommand>,
    camera_bind_group: Arc<wgpu::BindGroup>,
    light_bind_group: Arc<wgpu::BindGroup>,
    light_render_pipeline: Arc<wgpu::RenderPipeline>,
    render_pipeline: Arc<wgpu::RenderPipeline>,
}

/*
*
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
*
* */

impl DrawCtx {
    pub const fn command_queue(&self) -> &Vec<RenderCommand> {
        &self.command_queue
    }

    pub fn from_window(window: &RenderWindow) -> Self {
        Self {
            command_queue: Vec::new(),
            camera_bind_group: window.camera_bind_group(),
            light_bind_group: window.light_bind_group(),
            light_render_pipeline: window.light_render_pipeline(),
            render_pipeline: window.pipeline(),
        }
    }

    pub fn set_pipeline(&mut self, pipeline: Arc<wgpu::RenderPipeline>) {
        self.command_queue
            .push(RenderCommand::SetPipeline(pipeline.clone()));
    }

    pub fn set_bind_group(
        &mut self,
        index: u32,
        bind_group: Arc<wgpu::BindGroup>,
        offsets: Option<Vec<DynamicOffset>>,
    ) {
        self.command_queue.push(RenderCommand::SetBindGroup(
            index,
            bind_group.clone(),
            offsets,
        ));
    }

    pub fn set_blend_constant(&mut self, color: wgpu::Color) {
        self.command_queue
            .push(RenderCommand::SetBlendConstant(color));
    }

    pub fn set_index_buffer(&mut self, buffer: Arc<wgpu::Buffer>, format: IndexFormat) {
        self.command_queue
            .push(RenderCommand::SetIndexBuffer(buffer.clone(), format));
    }

    pub fn set_vertex_buffer(&mut self, slot: u32, buffer: Arc<wgpu::Buffer>) {
        self.command_queue
            .push(RenderCommand::SetVertexBuffer(slot, buffer.clone()));
    }

    pub fn set_scissor_rect(&mut self, x: u32, y: u32, width: u32, height: u32) {
        self.command_queue
            .push(RenderCommand::SetScissorRect(x, y, width, height));
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
        self.command_queue.push(RenderCommand::SetViewPort(
            x, y, width, height, min_depth, max_depth,
        ));
    }

    pub fn set_stencil_reference(&mut self, reference: u32) {
        self.command_queue
            .push(RenderCommand::SetStencilReference(reference));
    }

    pub fn draw(&mut self, verticies: Range<u32>, instances: Range<u32>) {
        self.command_queue
            .push(RenderCommand::Draw(verticies, instances));
    }

    pub fn insert_debug_marker(&mut self, label: &str) {
        self.command_queue
            .push(RenderCommand::InsertDebugMarker(String::from(label)))
    }

    pub fn push_debug_group(&mut self, label: &str) {
        self.command_queue
            .push(RenderCommand::PushDebugGroup(String::from(label)));
    }

    pub fn pop_debug_group(&mut self) {
        self.command_queue.push(RenderCommand::PopDebugGroup);
    }

    pub fn draw_indexed(&mut self, indices: Range<u32>, base_vertex: i32, instances: Range<u32>) {
        self.command_queue
            .push(RenderCommand::DrawIndexed(indices, base_vertex, instances));
    }

    pub fn draw_indirect(
        &mut self,
        indirect_buffer: Arc<wgpu::Buffer>,
        indirect_offset: BufferAddress,
    ) {
        self.command_queue.push(RenderCommand::DrawIndirect(
            indirect_buffer.clone(),
            indirect_offset,
        ));
    }

    pub fn draw_indexed_indirect(
        &mut self,
        indirect_buffer: Arc<wgpu::Buffer>,
        indirect_offset: BufferAddress,
    ) {
        self.command_queue.push(RenderCommand::DrawIndexedIndirect(
            indirect_buffer.clone(),
            indirect_offset,
        ));
    }

    pub fn draw_light_model(&mut self, model: &Model) {
        self.draw_light_model_instanced(model, 0..1);
    }
    pub fn draw_light_model_instanced(&mut self, model: &Model, instances: Range<u32>) {
        self.command_queue.push(RenderCommand::SetPipeline(
            self.light_render_pipeline.clone(),
        ));

        let cmds = draw_light_model_instanced(
            model,
            instances,
            self.camera_bind_group.clone(),
            self.light_bind_group.clone(),
        );
        self.command_queue.extend(cmds);
    }
    pub fn draw_light_mesh(&mut self, mesh: &Mesh) {
        self.draw_light_mesh_instanced(mesh, 0..1);
    }
    pub fn draw_light_mesh_instanced(&mut self, mesh: &Mesh, instances: Range<u32>) {
        self.command_queue.push(RenderCommand::SetPipeline(
            self.light_render_pipeline.clone(),
        ));
        let cmds = draw_light_mesh_instanced(
            mesh,
            instances,
            self.camera_bind_group.clone(),
            self.light_bind_group.clone(),
        );
        self.command_queue.extend(cmds);
    }
    pub fn draw_mesh(&mut self, mesh: &Mesh, mat: &Material) {
        self.draw_mesh_instanced(mesh, mat, 0..1);
    }
    pub fn draw_mesh_instanced(&mut self, mesh: &Mesh, mat: &Material, instances: Range<u32>) {
        self.command_queue
            .push(RenderCommand::SetPipeline(self.render_pipeline.clone()));

        let cmds = draw_mesh_instanced(
            mesh,
            mat,
            instances,
            self.camera_bind_group.clone(),
            self.light_bind_group.clone(),
        );
        self.command_queue.extend(cmds);
    }

    pub fn draw_model(&mut self, model: &Model) {
        self.draw_model_instanced(model, 0..1);
    }
    pub fn draw_model_instanced(&mut self, model: &Model, instances: Range<u32>) {
        self.command_queue
            .push(RenderCommand::SetPipeline(self.render_pipeline.clone()));

        let cmds = draw_model_instanced(
            model,
            instances,
            self.camera_bind_group.clone(),
            self.light_bind_group.clone(),
        );
        self.command_queue.extend(cmds);
    }
}
