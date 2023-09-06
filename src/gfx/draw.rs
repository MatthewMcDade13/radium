use std::{ops::Range, rc::Rc, sync::Arc};

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
