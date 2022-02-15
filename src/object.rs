use crate::vertex;
use gladius_shared::error::SlicerErrors;
use gladius_shared::loader::*;
use glam::{Mat4, Vec3, Vec4};
use glium::implement_vertex;
use std::ffi::OsStr;
use std::path::Path;
use itertools::*;

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

    pub fn get_model_matrix(&self) -> Mat4 {
        (glam::Mat4::from_translation(self.location) * glam::Mat4::from_scale(self.scale) *glam::Mat4::from_translation(self.default_offset))
    }

    pub fn intersect_with_ray(&self, ray_origin: Vec3, ray_dir: Vec3) -> Option<Vec3>{
        let vertices = self.vert_buff.read().unwrap();

        let mat = self.get_model_matrix();

        self.index_buff.read()
            .unwrap()
            .iter()
            .tuples::<(_,_,_)>()
            .map(|(v0,v1,v2)|{
                ///make points
                (vertices[*v0 as usize],vertices[*v1 as usize],vertices[*v2 as usize] )
            })
            .map(|(v0,v1,v2)|{
                (
                    mat.transform_point3(Vec3::new(v0.position.0,v0.position.1,v0.position.2)),
                    mat.transform_point3(Vec3::new(v1.position.0,v1.position.1,v1.position.2)),
                    mat.transform_point3(Vec3::new(v2.position.0,v2.position.1,v2.position.2)),
                    )
            })
            .filter_map(|(v0,v1,v2)|{
                let edge1 = v1 - v0;
                let edge2 = v2 - v0;

                let h = ray_dir.cross(edge2);
                let a = edge1.dot(h);

                if a > -0.0000001 && a < 0.0000001{
                    None
                }
                else{
                    let f = 1.0/a;
                    let s = ray_origin - v0;
                    let u = f*s.dot(h);
                    if u < 0.0 || u> 1.0{
                        None
                    }
                    else{
                        let q = s.cross(edge1);
                        let v = f* ray_dir.dot(q);
                        if v < 0.0 || u+v> 1.0{
                            None
                        }
                        else {
                            let t = f * edge2.dot(q);
                            if t > 0.0000001// ray intersection
                            {
                                Some(t)
                            }
                            else // This means that there is a line intersection but not a ray intersection.
                            {
                                None
                            }
                        }
                    }
                }
            })
            .min_by(|a,b| a.partial_cmp(b).unwrap())
            .map(|t| ray_origin + ray_dir*t)

    }

/*
    const float EPSILON = 0.0000001;
    Vector3D vertex0 = inTriangle->vertex0;
    Vector3D vertex1 = inTriangle->vertex1;
    Vector3D vertex2 = inTriangle->vertex2;
    Vector3D edge1, edge2, h, s, q;
    float a,f,u,v;
    edge1 = vertex1 - vertex0;
    edge2 = vertex2 - vertex0;
    h = rayVector.crossProduct(edge2);
    a = edge1.dotProduct(h);
    if (a > -EPSILON && a < EPSILON)
        return false;    // This ray is parallel to this triangle.
    f = 1.0/a;
    s = rayOrigin - vertex0;
    u = f * s.dotProduct(h);
    if (u < 0.0 || u > 1.0)
        return false;
    q = s.crossProduct(edge1);
    v = f * rayVector.dotProduct(q);
    if (v < 0.0 || u + v > 1.0)
        return false;
    // At this stage we can compute t to find out where the intersection point is on the line.
    float t = f * edge2.dotProduct(q);
    if (t > EPSILON) // ray intersection
    {
        outIntersectionPoint = rayOrigin + rayVector * t;
        return true;
    }
    else // This means that there is a line intersection but not a ray intersection.
        return false;
}*/
}

pub fn load(filepath: &str, display: &glium::Display) -> Result<Vec<Object>, SlicerErrors> {
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

    Ok(loader
        .load(model_path.to_str().unwrap())?
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

            let (min_x, max_x, min_y, max_y, min_z) = display_vertices.iter().fold(
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
                default_offset: Vec3::new(-(max_x + min_x) / 2.0, -(max_y + min_y) / 2.0, -min_z),
                color: Vec3::new(1.0, 1.0, 0.0),
                index_buff: indices,
                vert_buff: positions,
            }
        })
        .collect())
}

