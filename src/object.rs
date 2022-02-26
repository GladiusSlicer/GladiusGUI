use crate::vertex;
use gladius_shared::error::SlicerErrors;
use gladius_shared::loader::*;
use glam::{Mat4, Vec3};
use glium::implement_vertex;
use itertools::*;
use std::ffi::OsStr;
use std::path::Path;

#[derive(Copy, Clone, Debug)]
pub struct DisplayVertex {
    pub position: (f32, f32, f32),
}

implement_vertex!(DisplayVertex, position);

#[derive(Debug)]
pub struct AABB {
    pub min_x: f32,
    pub min_y: f32,
    pub min_z: f32,
    pub max_x: f32,
    pub max_y: f32,
    pub max_z: f32,
}

impl AABB {
    pub fn intersect_with_ray(&self, ray_origin: Vec3, ray_dir: Vec3) -> bool {
        //https://gamedev.stackexchange.com/questions/18436/most-efficient-aabb-vs-ray-collision-algorithms
        // r.dir is unit direction vector of ray
        // lb is the corner of AABB with minimal coordinates - left bottom, rt is maximal corner
        // r.org is origin of ray
        let t1 = (self.min_x - ray_origin.x) / ray_dir.x;
        let t2 = (self.max_x - ray_origin.x) / ray_dir.x;
        let t3 = (self.min_y - ray_origin.y) / ray_dir.y;
        let t4 = (self.max_y - ray_origin.y) / ray_dir.y;
        let t5 = (self.min_z - ray_origin.z) / ray_dir.z;
        let t6 = (self.max_z - ray_origin.z) / ray_dir.z;

        let tmin = t1.min(t2).max(t3.min(t4)).max(t5.min(t6));
        let tmax = t1.max(t2).min(t3.max(t4)).min(t5.max(t6));

        // if tmax < 0, ray (line) is intersecting AABB, but the whole AABB is behind us
        if tmax < 0.0 {
            return false;
        }

        // if tmin > tmax, ray doesn't intersect AABB
        if tmin > tmax {
            return false;
        }
        true
    }
}
#[derive(Debug)]
pub struct Object {
    pub name: String,
    pub file_path: String,
    location: Vec3,
    default_offset: Vec3,
    scale: Vec3,
    pub color: Vec3,
    pub hovered: bool,
    pub vert_buff: glium::VertexBuffer<DisplayVertex>,
    pub transformed_verts: Option<Vec<Vec3>>,
    pub aabb: Option<AABB>,
    pub index_buff: glium::IndexBuffer<u32>,
}
impl Object {
    pub fn set_scale(&mut self, scale: Vec3) {
        self.scale = scale;
    }

    pub fn set_location(&mut self, location: Vec3) {
        self.location = location;
    }

    pub fn get_mut_location(&mut self) -> &mut Vec3 {
        &mut self.location
    }

    pub fn get_location(&self) -> &Vec3 {
        &self.location
    }

    pub fn get_mut_scale(&mut self) -> &mut Vec3 {
        &mut self.scale
    }
    pub fn get_scale(&self) -> &Vec3 {
        &self.scale
    }

    pub fn invalidate_cache(&mut self) {
        self.transformed_verts = None;
        self.aabb = None;
    }

    pub fn revalidate_cache(&mut self) {
        println!("revalidate");
        let vertices = {
            let mat = self.get_model_matrix();

            self.vert_buff
                .read()
                .unwrap()
                .iter()
                .map(|v| mat.transform_point3(Vec3::new(v.position.0, v.position.1, v.position.2)))
                .collect_vec()
        };

        let aabb = {
            let (min_x, max_x, min_y, max_y, min_z, max_z) = vertices.iter().fold(
                (
                    f32::INFINITY,
                    f32::NEG_INFINITY,
                    f32::INFINITY,
                    f32::NEG_INFINITY,
                    f32::INFINITY,
                    f32::NEG_INFINITY,
                ),
                |a, b| {
                    (
                        a.0.min(b.x),
                        a.1.max(b.x),
                        a.2.min(b.y),
                        a.3.max(b.y),
                        a.4.min(b.z),
                        a.5.max(b.z),
                    )
                },
            );

            AABB {
                min_x,
                max_x,
                min_y,
                max_y,
                min_z,
                max_z,
            }
        };

        self.transformed_verts = Some(vertices);
        self.aabb = Some(aabb);
    }

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
            hovered: false,
            vert_buff: positions,
            index_buff: indices,
            transformed_verts: None,
            aabb: None,
        }
    }

    pub fn get_model_matrix(&self) -> Mat4 {
        glam::Mat4::from_translation(self.location)
            * glam::Mat4::from_scale(self.scale)
            * glam::Mat4::from_translation(self.default_offset)
    }

    pub fn intersect_with_ray(&mut self, ray_origin: Vec3, ray_dir: Vec3) -> Option<(f32, Vec3)> {
        let vertices = self.transformed_verts.take().unwrap_or_else(|| {
            let mat = self.get_model_matrix();

            self.vert_buff
                .read()
                .unwrap()
                .iter()
                .map(|v| mat.transform_point3(Vec3::new(v.position.0, v.position.1, v.position.2)))
                .collect()
        });

        let aabb = self.aabb.take().unwrap_or_else(|| {
            let (min_x, max_x, min_y, max_y, min_z, max_z) = vertices.iter().fold(
                (
                    f32::INFINITY,
                    f32::NEG_INFINITY,
                    f32::INFINITY,
                    f32::NEG_INFINITY,
                    f32::INFINITY,
                    f32::NEG_INFINITY,
                ),
                |a, b| {
                    (
                        a.0.min(b.x),
                        a.1.max(b.x),
                        a.2.min(b.y),
                        a.3.max(b.y),
                        a.4.min(b.z),
                        a.5.max(b.z),
                    )
                },
            );

            AABB {
                min_x,
                max_x,
                min_y,
                max_y,
                min_z,
                max_z,
            }
        });

        if !aabb.intersect_with_ray(ray_origin, ray_dir) {
            self.transformed_verts = Some(vertices);
            self.aabb = Some(aabb);
            return None;
        }

        let ret = self
            .index_buff
            .read()
            .unwrap()
            .iter()
            .tuples::<(_, _, _)>()
            .map(|(v0, v1, v2)| {
                //make points
                (
                    vertices[*v0 as usize],
                    vertices[*v1 as usize],
                    vertices[*v2 as usize],
                )
            })
            .filter_map(|(v0, v1, v2)| {
                let edge1 = v1 - v0;
                let edge2 = v2 - v0;

                let h = ray_dir.cross(edge2);
                let a = edge1.dot(h);

                if a > -0.0001 && a < 0.0001 {
                    None
                } else {
                    let f = 1.0 / a;
                    let s = ray_origin - v0;
                    let u = f * s.dot(h);
                    if u < 0.0 || u > 1.0 {
                        None
                    } else {
                        let q = s.cross(edge1);
                        let v = f * ray_dir.dot(q);
                        if v < 0.0 || u + v > 1.0 {
                            None
                        } else {
                            let t = f * edge2.dot(q);
                            if t > f32::EPSILON
                            // ray intersection
                            {
                                Some(t)
                            } else // This means that there is a line intersection but not a ray intersection.
                            {
                                None
                            }
                        }
                    }
                }
            })
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .map(|t| (t, ray_origin + ray_dir * t));

        self.transformed_verts = Some(vertices);
        self.aabb = Some(aabb);

        ret
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
                transformed_verts: None,
                aabb: None,
                hovered: false,
            }
        })
        .collect())
}
