use std::cmp::min;
use std::io::BufReader;
use std::path::Path;
use glam::Vec3;
use glium::implement_vertex;

#[derive(Copy, Clone)]
pub struct Vertex {
    pub position: (f32, f32, f32)
}

implement_vertex!(Vertex, position);


pub struct Object{
    pub name : String,
    pub file_path : String,
    pub location : Vec3,
    pub color    : Vec3,
    pub vert_buff : glium::VertexBuffer<Vertex>,
    pub index_buff: glium::IndexBuffer<u32>

}



pub fn load(
    filepath: &str,
    display: &glium::Display,
) -> Object {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .open(filepath)
        .unwrap();

    let mut root_vase = BufReader::new(&file);
    let mesh: nom_stl::IndexMesh = nom_stl::parse_stl(&mut root_vase)
        .unwrap()
        .into();

    let vertices = mesh
        .vertices()
        .iter()
        .map(|vert| Vertex {
            position: (vert[0],vert[1],vert[2]),
        })
        .collect::<Vec<Vertex>>();

    let indices : Vec<u32> =  mesh.triangles()
        .into_iter()
        .flat_map(|tri|{
            tri.vertices_indices().into_iter()
        })
        .map(|u| u as u32)
        .collect();

    let positions = glium::VertexBuffer::new(display, &vertices).unwrap();
    let indices = glium::IndexBuffer::new(display, glium::index::PrimitiveType::TrianglesList, &indices).unwrap();

    let min_z = vertices.iter().map(|v| v.position.2).min_by(|a,b| a.partial_cmp(b).unwrap()).unwrap();

    let model_path = Path::new(filepath).file_name().unwrap();
    Object{ name: model_path.to_string_lossy().to_string(), file_path: filepath.to_string(), location: Vec3::new(0.0, 0.0, -min_z), color:Vec3::new(1.0, 0.0, 0.0),index_buff: indices, vert_buff: positions }
}

