#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UiVertex {
    /// Screen-space pixel coordinates, origin top-left (matches
    /// window/mouse coordinates) — converted to NDC in the vertex
    /// shader via a screen-size uniform.
    pub position: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

impl UiVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 3] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4];

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<UiVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Turns a `ui::DrawList` (screen-space quads) into a flat vertex/index
/// buffer the GPU pipeline in `gpu.rs` can draw directly.
pub fn build_ui_mesh(draw_list: &ui::DrawList) -> (Vec<UiVertex>, Vec<u32>) {
    let mut vertices = Vec::with_capacity(draw_list.quads.len() * 4);
    let mut indices = Vec::with_capacity(draw_list.quads.len() * 6);

    for quad in &draw_list.quads {
        let base_index = vertices.len() as u32;
        let color = [quad.color.r, quad.color.g, quad.color.b, quad.color.a];
        let (u0, v0, u1, v1) = quad.uv;
        let r = quad.rect;

        vertices.push(UiVertex {
            position: [r.x, r.y],
            uv: [u0, v0],
            color,
        });
        vertices.push(UiVertex {
            position: [r.x + r.w, r.y],
            uv: [u1, v0],
            color,
        });
        vertices.push(UiVertex {
            position: [r.x + r.w, r.y + r.h],
            uv: [u1, v1],
            color,
        });
        vertices.push(UiVertex {
            position: [r.x, r.y + r.h],
            uv: [u0, v1],
            color,
        });

        indices.extend_from_slice(&[
            base_index,
            base_index + 1,
            base_index + 2,
            base_index,
            base_index + 2,
            base_index + 3,
        ]);
    }

    (vertices, indices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ui::geometry::{Color, Rect};
    use ui::{DrawList, Quad};

    #[test]
    fn one_quad_becomes_4_vertices_and_6_indices() {
        let mut list = DrawList::new();
        list.push(Quad {
            rect: Rect::new(10.0, 20.0, 30.0, 40.0),
            uv: (0.0, 0.0, 1.0, 1.0),
            color: Color::WHITE,
        });
        let (vertices, indices) = build_ui_mesh(&list);
        assert_eq!(vertices.len(), 4);
        assert_eq!(indices.len(), 6);
        // Top-left vertex should be exactly the rect's origin.
        assert_eq!(vertices[0].position, [10.0, 20.0]);
        assert_eq!(vertices[2].position, [40.0, 60.0]);
    }
}
