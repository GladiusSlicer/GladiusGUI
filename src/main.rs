#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
mod object;


use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::process::{Command, Stdio};
use egui::{Align, Direction, FontDefinitions, FontFamily, Layout, Pos2, Style, TextStyle, Vec2};
use egui::CursorIcon::Text;
use egui::epaint::text::layout;
use glam::Vec3;
use glium::{glutin, implement_vertex, Surface, uniform};
use native_dialog::FileDialog;
use winit::event::{DeviceEvent, ElementState, MouseScrollDelta,WindowEvent};
use crate::object::{load, DisplayVertex};

use gladius_shared::messages::Message;

fn vertex(pos: [f32; 3]) -> DisplayVertex {
    DisplayVertex{position: (pos[0],pos[1],pos[2])}
}

fn create_mesh(display: &glium::Display,) -> (glium::VertexBuffer<DisplayVertex>, glium::IndexBuffer<u32> ) {
    create_cube_mesh(display, (-5.0,5.0),(-5.0,5.0), (0.0,10.0))
}

fn create_bp_mesh(display: &glium::Display,) -> (glium::VertexBuffer<DisplayVertex>, glium::IndexBuffer<u32> ) {
    create_cube_mesh(display, (0.0,20.0),(0.0,20.0), (-0.1,-0.00))
}


fn create_cube_mesh(display: &glium::Display,x: (f32,f32),y: (f32,f32),z: (f32,f32),) -> (glium::VertexBuffer<DisplayVertex>, glium::IndexBuffer<u32> ){
    let vertex_positions = [
        // far side (0.0, 0.0, 1.0)
        vertex([x.0, y.0, z.1]),
        vertex([x.1, y.0, z.1]),
        vertex([x.1, y.1, z.1]),
        vertex([x.0, y.1, z.1]),
        // near side (0.0, 0.0, -1.0)
        vertex([x.0, y.1, z.0]),
        vertex([x.1, y.1, z.0]),
        vertex([x.1, y.0, z.0]),
        vertex([x.0, y.0, z.0]),
        // right side (1.0, 0.0, 0.0)
        vertex([x.1, y.0, z.0]),
        vertex([x.1, y.1, z.0]),
        vertex([x.1, y.1, z.1]),
        vertex([x.1, y.0, z.1]),
        // left side (-1.0, 0.0, 0.0)
        vertex([x.0, y.0, z.1]),
        vertex([x.0, y.1, z.1]),
        vertex([x.0, y.1, z.0]),
        vertex([x.0, y.0, z.0]),
        // top (0.0, 1.0, 0.0)
        vertex([x.1, y.1, z.0]),
        vertex([x.0, y.1, z.0]),
        vertex([x.0, y.1, z.1]),
        vertex([x.1, y.1, z.1]),
        // bottom (0.0, -1.0, 0.0)
        vertex([x.1, y.0, z.1]),
        vertex([x.0, y.0, z.1]),
        vertex([x.0, y.0, z.0]),
        vertex([x.1, y.0, z.0]),
    ];

    let index_data: &[u32] = &[
        0, 1, 2, 2, 3, 0, // far
        4, 5, 6, 6, 7, 4, // near
        8, 9, 10, 10, 11, 8, // right
        12, 13, 14, 14, 15, 12, // left
        16, 17, 18, 18, 19, 16, // top
        20, 21, 22, 22, 23, 20, // bottom
    ];

    let positions = glium::VertexBuffer::new(display, &vertex_positions).unwrap();
    let indices = glium::IndexBuffer::new(display, glium::index::PrimitiveType::TrianglesList, index_data).unwrap();
    (positions,indices)

}

fn create_build_area(display: &glium::Display, build_x:f32, build_y : f32, build_z : f32) -> (glium::VertexBuffer<DisplayVertex>, glium::IndexBuffer<u32> ){
    let mut vertex_positions : Vec<DisplayVertex> = vec![
        // far side (0.0, 0.0, 1.0)
        vertex([0.0, 0.0, 0.0]),
        vertex([0.0, build_y, 0.0]),
        vertex([build_x, 0.0, 0.0]),
        vertex([build_x, build_y, 0.0]),
        vertex([0.0, 0.0, build_z]),
        vertex([0.0, build_y, build_z]),
        vertex([build_x, 0.0, build_z]),
        vertex([build_x, build_y, build_z]),
    ];

    let mut index_data: Vec<u32> = vec![
        0, 1,
        0, 2,
        1, 3,
        2, 3,
        4, 5,
        4, 6,
        5, 7,
        6, 7,
        0, 4,
        1, 5,
        2, 6,
        3, 7,
    ];

    let step_size = 10.0;
    (0..(build_x / step_size)as u32)
        .into_iter()
        .map(|index| index as f32 * step_size)
        .for_each(|x|{
            let index_pos = vertex_positions.len() as u32;
            vertex_positions.push(vertex([x, 0.0, 0.0]));
            vertex_positions.push(vertex([x, build_y, 0.0]));
            index_data.push(index_pos);
            index_data.push(index_pos+1);
        });

    (0..(build_y / step_size)as u32)
        .into_iter()
        .map(|index| index as f32 * step_size)
        .for_each(|y|{
            let index_pos = vertex_positions.len() as u32;
            vertex_positions.push(vertex([0.0, y, 0.0]));
            vertex_positions.push(vertex([build_x, y, 0.0]));
            index_data.push(index_pos);
            index_data.push(index_pos+1);
        });

    let positions = glium::VertexBuffer::new(display, &vertex_positions).unwrap();
    let indices = glium::IndexBuffer::new(display, glium::index::PrimitiveType::LinesList, &index_data).unwrap();
    (positions,indices)

}



fn create_display(event_loop: &glutin::event_loop::EventLoop<()>) -> glium::Display {
    let window_builder = glutin::window::WindowBuilder::new()
        .with_resizable(true)
        .with_inner_size(glutin::dpi::LogicalSize {
            width: 800.0,
            height: 600.0,
        })
        .with_title("Gladius");

    let context_builder = glutin::ContextBuilder::new()
        .with_depth_buffer(24)
        .with_multisampling(16)
        .with_srgb(true)
        .with_vsync(true);

    glium::Display::new(window_builder, context_builder, event_loop).unwrap()
}

fn main() {
    let event_loop = glutin::event_loop::EventLoop::with_user_event();
    let display = create_display(&event_loop);

    let mut egui_glium = egui_glium::EguiGlium::new(&display);


    let vertex_shader_src = r#"
        #version 150
        in vec3 position;
        out vec3 viewPosition;
        out vec3 v_position;
        uniform mat4 perspective;
        uniform mat4 view;
        uniform mat4 model;
        void main() {
            mat4 modelview = view * model;
            viewPosition = (view * vec4(position, 1.0)).xyz;
            gl_Position = perspective * modelview * vec4(position, 1.0);

            v_position = gl_Position.xyz / gl_Position.w;
        }
    "#;
    let fragment_shader_src = r#"
        #version 140
        in vec3 viewPosition;
        in vec3 v_position;
        out vec4 color;
        void main() {
            vec3 xTangent = dFdx( viewPosition );
            vec3 yTangent = dFdy( viewPosition );
            vec3 faceNormal = normalize( cross( xTangent, yTangent ));

            const vec3 ambient_color = vec3(0.0, 0.0, 0.0);
            const vec3 diffuse_color = vec3(1.0, 1.0, 0.0);
            const vec3 specular_color = vec3(1.0, 1.0, 1.0);

            float diffuse = max(dot(normalize(faceNormal), normalize(vec3(0., 0.0, 0.1))), 0.0);
            vec3 camera_dir = normalize(-v_position);
            vec3 half_direction = normalize(camera_dir);
            float specular = pow(max(dot(half_direction, -normalize(faceNormal)), 0.0), 128.0);
            color = vec4(ambient_color + diffuse * diffuse_color + specular * specular_color, 1.0);


        }
    "#;
    let line_vertex_shader_src = r#"
        #version 150
        in vec3 position;
        uniform mat4 perspective;
        uniform mat4 view;
        uniform mat4 model;
        void main() {
            mat4 modelview = view * model;
            gl_Position = perspective * modelview * vec4(position, 1.0);
        }
    "#;
    let line_fragment_shader_src = r#"
        #version 140
        out vec4 color;
        void main() {

            color = vec4( vec3(0.0, 0.0, 1.0), 1.0);

        }
    "#;
    let mut camera_pitch = std::f32::consts::FRAC_PI_4;
    let mut camera_yaw= -std::f32::consts::FRAC_PI_4 + 0.12;
    let mut zoom= 50.0;
    let mut center_pos= (125.0, 105.0);
    let mut left_mouse_button_state = ElementState::Released;
    let mut on_render_screen = false;
    let mut in_window = false;
    let mut calc_vals = None;
    let mut gcode = None;
    let mut error = None;

     let mut model_path="".to_string();
     let mut settings_path= "".to_string();
     let mut output_path= "".to_string();

    let model_program = glium::Program::from_source(&display, vertex_shader_src, fragment_shader_src, None).unwrap();
    let line_program = glium::Program::from_source(&display, line_vertex_shader_src, line_fragment_shader_src, None).unwrap();

    let mut objects = vec![];

    let mut rect = None;

    let (line_positions, line_indices) = create_build_area(&display,250.0 , 210.0,210.0);
    event_loop.run(move |event, _, control_flow| {
        let mut redraw = || {
            let mut quit = false;


            let (needs_repaint, shapes) = egui_glium.run(&display, |egui_ctx| {
               rect = Some(egui::SidePanel::left("my_side_panel").show(egui_ctx, |ui| {
                   ui.heading("Print Setup");
                   ui.horizontal(|ui| {
                       ui.label("Model path: ");
                       if ui.button("Choose Model").clicked() {
                           let path = FileDialog::new()
                               .add_filter("Supported Model Types", &["stl", "3mf"])
                               .show_open_single_file()
                               .unwrap();

                           let path = match path {
                               Some(path) => path,
                               None => return,
                           };

                           model_path = path.into_os_string().into_string().unwrap();

                           objects.extend(load(&model_path, &display).into_iter());
                       }
                   });
                   ui.horizontal(|ui| {
                       ui.label("Settings path: ");
                       ui.text_edit_singleline(&mut settings_path);
                       if ui.button("Choose settings").clicked() {
                           let path = FileDialog::new()
                               .add_filter("Supported settings Types", &["json"])
                               .show_open_single_file()
                               .unwrap();

                           let path = match path {
                               Some(path) => path,
                               None => return,
                           };

                           settings_path = path.into_os_string().into_string().unwrap();
                       }
                   });
                   ui.group(|ui| {
                       objects = objects.drain(..).filter_map(|mut obj| {
                           let mut remove = false;
                           let mut duplicate = false;
                           ui.horizontal(|ui| {
                               ui.label(obj.name.to_string());
                               ui.add(egui::DragValue::new(&mut obj.location.x)
                                   .speed(1.0)
                                   .clamp_range(f64::NEG_INFINITY..=f64::INFINITY)
                                   .prefix("x: "));
                               ui.add(egui::DragValue::new(&mut obj.location.y)
                                   .speed(1.0)
                                   .clamp_range(f64::NEG_INFINITY..=f64::INFINITY)
                                   .prefix("y: "));
                               remove = ui.button("Remove").clicked();
                               duplicate = ui.button("Duplicate").clicked();
                           });

                           if !remove {
                               if duplicate {
                                   Some(vec![obj.make_copy(&display), obj].into_iter())
                               } else {
                                   Some(vec![obj].into_iter())
                               }
                           } else {
                               None
                           }
                       }).flatten().collect();
                   });

                   ui.horizontal(|ui| {


                       ui.style_mut().spacing.button_padding = Vec2::new(50., 20.);
                       ui.style_mut().body_text_style = TextStyle::Heading;
                       ui.style_mut().override_text_style = Some(TextStyle::Heading);


                       ui.centered_and_justified(|ui| {
                           let mut fonts = FontDefinitions::default();

                           // Large button text:
                           fonts.family_and_size.insert(
                               TextStyle::Button,
                               (FontFamily::Proportional, 32.0)
                           );

                           //ui.ctx().set_fonts(fonts);


                           if ui.button("Slice").clicked() {
                               let mut command = if cfg!(target_os = "linux") {
                                   Command::new("./slicer/gladius_slicer")
                               } else if cfg!(target_os = "windows") {
                                   Command::new("slicer\\gladius_slicer.exe")
                               } else {
                                   unimplemented!()
                               };

                               for obj in &objects {
                                   //"{\"Raw\":[\"test_3D_models\\3DBenchy.stl\",[[1.0,0.0,0.0,124.0],[0.0,1.0,0.0,105.0],[0.0,0.0,1.0,0.0],[0.0,0.0,0.0,1.0]] }"
                                   command.arg(format!("{{\"Raw\":[\"{}\",{:?}]}} ", obj.file_path.replace('\\', "\\\\"), glam::Mat4::from_translation(obj.location).transpose().to_cols_array_2d()));
                               }
                               let mut child = command
                                   .arg("-m")
                                   .arg("-s")
                                   .arg(format!("{}", settings_path.replace('\\', "\\\\")))
                                   .arg("-j")
                                   .arg(format!("{}", (num_cpus::get()).max(1)))
                                   .stdout(Stdio::piped())
                                   .spawn()
                                   .expect("failed to execute child");


                               // Loop over the output from the first process
                               if let Some(ref mut stdout) = child.stdout {
                                   for msg in serde_json::Deserializer::from_reader(stdout).into_iter::<Message>() {
                                       match msg.unwrap() {
                                           Message::CalculatedValues(cv) => {
                                               calc_vals = Some(cv);
                                           }
                                           Message::Commands(_) => {}
                                           Message::GCode(str) => {
                                               gcode = Some(str);
                                           }
                                           Message::Error(err) => {
                                               error = Some(err);
                                           }
                                       }
                                   }
                               }
                           }
                       });
                   });

                   if let Some(cv) = calc_vals.as_ref() {
                       ui.horizontal(|ui| {
                           ui.label(format!("This print will use {:.0} cm^3 of plastic",cv.plastic_volume));
                       });
                       ui.horizontal(|ui| {
                           ui.label(format!("This print will use {:.0} grams of plastic",cv.plastic_weight));
                       });
                       ui.horizontal(|ui| {
                           let (hour,min,_,_) = cv.get_hours_minutes_seconds_fract_time();
                           ui.label(format!("This print will take {} hours and {} minutes",hour,min));
                       });
                   };

                   if let Some(str) = gcode.as_ref() {
                       ui.horizontal(|ui| {
                           ui.style_mut().spacing.button_padding = Vec2::new(50., 20.);
                           ui.style_mut().body_text_style = TextStyle::Heading;
                           ui.style_mut().override_text_style = Some(TextStyle::Heading);


                           ui.centered_and_justified(|ui| {
                               if ui.button("Save").clicked() {
                                   let path = FileDialog::new()
                                       .add_filter("gcode", &["gcode"])
                                       .show_save_single_file()
                                       .unwrap();

                                   let path = match path {
                                       Some(path) => path,
                                       None => return,
                                   };

                                   let mut file = File::create(path).unwrap();
                                   file.write_all(str.as_bytes());
                               }
                           });
                       });
                   }



                }).response.rect);

            });

            *control_flow = if quit {
                glutin::event_loop::ControlFlow::Exit
            } else if needs_repaint {
                display.gl_window().window().request_redraw();
                glutin::event_loop::ControlFlow::Poll
            } else {
                glutin::event_loop::ControlFlow::Wait
            };

            {
                use glium::Surface as _;
                let mut target = display.draw();

                let color = egui::Rgba::from_rgb(0.0, 0.0, 0.0);
                target.clear_color_and_depth((color[0], color[1], color[2], color[3]),1.0);

                // draw things behind egui here

                let camera_vec = glam::Vec3::new(zoom * camera_yaw.cos() * camera_pitch.cos() ,zoom * camera_yaw.sin() * camera_pitch.cos() , zoom * camera_pitch.sin());

                let camera_location = camera_vec + glam::Vec3::new(center_pos.0 ,center_pos.1 , 0.0);
                //let view = glam::Mat4::from_euler(glam::EulerRot::XYZ, -data.camera_pitch, -data.camera_yaw, 0.0);
                let view = glam::Mat4::look_at_rh(camera_location,glam::Vec3::new( center_pos.0 ,center_pos.1,0.0),glam::Vec3::new(0.0,0.0,1.0));

                let (width, height) = target.get_dimensions();
                let aspect_ratio = width as f32 / height as f32;

                let perspective = glam::Mat4::perspective_infinite_rh(60.0_f32.to_radians(),aspect_ratio,0.1).to_cols_array_2d();

                let view :[[f32;4];4] = view.to_cols_array_2d();


                let line_model = glam::Mat4::from_translation(Vec3::new(0.0,0.0,0.0)).to_cols_array_2d();

                let params = glium::DrawParameters {
                    depth: glium::Depth {
                        test: glium::draw_parameters::DepthTest::IfLess,
                        write: true,
                        .. Default::default()
                    },
                    //backface_culling: glium::draw_parameters::BackfaceCullingMode::CullClockwise,
                    .. Default::default()
                };

                for obj in &objects{
                    let model = glam::Mat4::from_translation(obj.location).to_cols_array_2d();
                    let (positions, indices) = (&obj.vert_buff,&obj.index_buff);//create_mesh(&display);
                    target.draw(positions, indices, &model_program, &uniform! { model: model, view: view, perspective: perspective }, &params).unwrap();
                }


                target.draw(&line_positions, &line_indices, &line_program, &uniform! { model: line_model, view: view, perspective: perspective }, &params).unwrap();

                egui_glium.paint(&display, &mut target, shapes);

                // draw things on top of egui here

                target.finish().unwrap();
            }
        };

        match event {
            // Platform-dependent event handlers to workaround a winit bug
            // See: https://github.com/rust-windowing/winit/issues/987
            // See: https://github.com/rust-windowing/winit/issues/1619
            glutin::event::Event::RedrawEventsCleared if cfg!(windows) => redraw(),
            glutin::event::Event::RedrawRequested(_) if !cfg!(windows) => redraw(),

            glutin::event::Event::WindowEvent { event, .. } => {


                match event{
                    WindowEvent::CloseRequested | WindowEvent::Destroyed => {
                        *control_flow = glutin::event_loop::ControlFlow::Exit;
                    }
                    WindowEvent::CursorMoved {position, ..} => {
                        on_render_screen = if let Some(rect ) = rect {
                            !rect.contains(Pos2{x: position.x as f32,y: position.y as f32 })
                        }
                        else{
                            false
                        };
                        in_window = true;
                    }
                    WindowEvent::CursorLeft {..} =>{
                        in_window = false;
                    }
                    _ => {}
                }

                egui_glium.on_event(&event);

                display.gl_window().window().request_redraw(); // TODO: ask egui if the events warrants a repaint instead
            }
            glutin::event::Event::DeviceEvent {event, ..} =>
            {
                match event {
                    DeviceEvent::Button {button, state} => {
                        if button == 1{
                            left_mouse_button_state = state;
                        }

                    }

                    DeviceEvent::MouseMotion {delta: (dx,dy)} =>{
                        if let Some(rect ) = rect {
                            if left_mouse_button_state == ElementState::Pressed && on_render_screen && in_window{
                                camera_yaw += dx as f32 * 0.01;
                                camera_pitch = (camera_pitch + dy as f32 * 0.01).min(std::f32::consts::FRAC_PI_2 - 0.001).max(-std::f32::consts::FRAC_PI_2 + 0.001);
                            }
                        }
                    }
                    DeviceEvent::MouseWheel {delta} =>{
                        if  on_render_screen && in_window {
                            if let MouseScrollDelta::LineDelta(x, y) = delta {
                                zoom = (zoom * (1.0 - (0.1 * y.signum()))).min(1000.0).max(5.0);
                            }
                        }
                    }

                    _ => {
                        //println!("{:?}",event);
                    }
                }
            }

            _ => {
                //println!("{:?}",event)
            },
        }
    });
}


fn view_matrix(position: &[f32; 3], direction: &[f32; 3], up: &[f32; 3]) -> [[f32; 4]; 4] {
    let f = {
        let f = direction;
        let len = f[0] * f[0] + f[1] * f[1] + f[2] * f[2];
        let len = len.sqrt();
        [f[0] / len, f[1] / len, f[2] / len]
    };

    let s = [up[1] * f[2] - up[2] * f[1],
             up[2] * f[0] - up[0] * f[2],
             up[0] * f[1] - up[1] * f[0]];

    let s_norm = {
        let len = s[0] * s[0] + s[1] * s[1] + s[2] * s[2];
        let len = len.sqrt();
        [s[0] / len, s[1] / len, s[2] / len]
    };

    let u = [f[1] * s_norm[2] - f[2] * s_norm[1],
             f[2] * s_norm[0] - f[0] * s_norm[2],
             f[0] * s_norm[1] - f[1] * s_norm[0]];

    let p = [-position[0] * s_norm[0] - position[1] * s_norm[1] - position[2] * s_norm[2],
             -position[0] * u[0] - position[1] * u[1] - position[2] * u[2],
             -position[0] * f[0] - position[1] * f[1] - position[2] * f[2]];

    [
        [s_norm[0], u[0], f[0], 0.0],
        [s_norm[1], u[1], f[1], 0.0],
        [s_norm[2], u[2], f[2], 0.0],
        [p[0], p[1], p[2], 1.0],
    ]
}