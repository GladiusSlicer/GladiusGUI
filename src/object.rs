use crate::vertex;
use gladius_shared::loader::*;
use glam::Vec3;
use glium::implement_vertex;
use std::ffi::OsStr;
use std::path::Path;
use gladius_shared::error::SlicerErrors;

#[derive(Copy, Clone, Debug)]
pub struct DisplayVertex {
    pub position: (f32, f32, f32),
}

implement_vertex!(DisplayVertex, position);

#[derive(Debug)]
pub struct Object {
    pub name: String,
    pub file_path: String,
    pub location: Vec3,
    pub default_offset: Vec3,
    pub scale: Vec3,
    pub color: Vec3,
    pub vert_buff: glium::VertexBuffer<DisplayVertex>,
    pub index_buff: glium::IndexBuffer<u32>,
}
impl Object {
    pub fn make_copy(&self, display: &glium::Display) -> Self {
        let positions = glium::VertexBuffer::new(display, &self.vert_buff.read().unwrap()).unwrap();
        let indices = glium::IndexBuffer::new(
            display,
            glium::index::PrimitiveType::TrianglesList,
            &self.index_buff.read().unwrap(),
        )
        .unwrap();

        Object {
            name: self.name.clone(),
            file_path: self.file_path.clone(),
            location: self.location,
            default_offset: self.default_offset,
            scale: self.scale,
            color: self.color,
            vert_buff: positions,
            index_buff: indices,
        }
    }
}

pub fn load(filepath: &str, display: &glium::Display) -> Result<Vec<Object>,SlicerErrors> {
    let model_path = Path::new(filepath);
    let extension = model_path
        .extension()
        .and_then(OsStr::to_str)
        .expect("File Parse Issue");

    let loader: &dyn Loader = match extension.to_lowercase().as_str() {
        "stl" => &STLLoader {},
        "3mf" => &ThreeMFLoader {},
        _ => panic!("File Format {} not supported", extension),
    };


    Ok(loader.load(model_path.to_str().unwrap())?
    .into_iter()
    .map(|(vertices, triangles)| {
        let display_vertices: Vec<DisplayVertex> = vertices
            .into_iter()
            .map(|v| vertex([v.x as f32, v.y as f32, v.z as f32]))
            .collect();

        let indices: Vec<u32> = triangles
            .into_iter()
            .flat_map(|tri| tri.verts.into_iter())
            .map(|u| u as u32)
            .collect();

        let positions = glium::VertexBuffer::new(display, &display_vertices).unwrap();
        let indices = glium::IndexBuffer::new(
            display,
            glium::index::PrimitiveType::TrianglesList,
            &indices,
        )
        .unwrap();


        let (min_x, max_x, min_y, max_y, min_z) =
            display_vertices.iter().fold(
                (
                    f32::INFINITY,
                    f32::NEG_INFINITY,
                    f32::INFINITY,
                    f32::NEG_INFINITY,
                    f32::INFINITY,
                ),
                |a, b| {
                    (
                        a.0.min(b.position.0),
                        a.1.max(b.position.0),
                        a.2.min(b.position.1),
                        a.3.max(b.position.1),
                        a.4.min(b.position.2),
                    )
                },
            );


        let model_path = Path::new(filepath).file_name().unwrap();
        Object {
            name: model_path.to_string_lossy().to_string(),
            file_path: filepath.to_string(),
            location: Vec3::new(0.0, 0.0, 0.0),
            scale: Vec3::new(1.0, 1.0, 1.0),
            default_offset: Vec3::new((max_x + min_x)/2.0, (max_y + min_y)/2.0, -min_z),
            color: Vec3::new(1.0, 0.0, 0.0),
            index_buff: indices,
            vert_buff: positions,
        }
    })
    .collect())
}
