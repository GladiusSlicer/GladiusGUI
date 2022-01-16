use std::cmp::min;
use std::ffi::OsStr;
use std::io::BufReader;
use std::path::Path;
use gladius_shared::loader::*;
use glam::Vec3;
use glium::implement_vertex;
use crate::vertex;

#[derive(Copy, Clone)]
pub struct DisplayVertex {
    pub position: (f32, f32, f32)
}

implement_vertex!(DisplayVertex, position);


pub struct Object{
    pub name : String,
    pub file_path : String,
    pub location : Vec3,
    pub color    : Vec3,
    pub vert_buff : glium::VertexBuffer<DisplayVertex>,
    pub index_buff: glium::IndexBuffer<u32>

}



pub fn load(
    filepath: &str,
    display: &glium::Display,
) -> Vec<Object> {
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


    match loader.load(model_path.to_str().unwrap()) {
        Ok(v) => v,
        Err(err) => {
            err.show_error_message();
            std::process::exit(-1);
        }
    }
        .into_iter()
        .map(|(vertices, triangles)|
            {
                let display_vertices: Vec<DisplayVertex> = vertices
                    .into_iter()
                    .map(|v| vertex( [v.x as f32,v.y as f32,v.z as f32]))
                    .collect();

                let indices: Vec<u32> = triangles
                    .into_iter()
                    .flat_map(|tri| {
                        tri.verts.into_iter()
                    })
                    .map(|u| u as u32)
                    .collect();

                let positions = glium::VertexBuffer::new(display, &display_vertices).unwrap();
                let indices = glium::IndexBuffer::new(display, glium::index::PrimitiveType::TrianglesList, &indices).unwrap();

                let min_z = display_vertices.iter().map(|v| v.position.2).min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap();

                let model_path = Path::new(filepath).file_name().unwrap();
                Object { name: model_path.to_string_lossy().to_string(), file_path: filepath.to_string(), location: Vec3::new(0.0, 0.0, -min_z), color: Vec3::new(1.0, 0.0, 0.0), index_buff: indices, vert_buff: positions }
            }
        )
        .collect()



}

