#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
mod object;
mod shaders;

use crate::object::{load, DisplayVertex};
use crate::shaders::*;
use egui::plot::{Corner, Legend, Line, Plot, Value, Values};
use egui::{
    vec2, Color32, FontDefinitions, FontFamily, InnerResponse, Pos2, Sense, Stroke, TextStyle, Vec2,
};
use env_logger::Target::Stderr;
use gladius_shared::error::SlicerErrors;
use gladius_shared::messages::Message;
use glam::Vec3;
use glium::{glutin, uniform};
use itertools::Itertools;
use native_dialog::FileDialog;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, RwLock};
use winit::event::{DeviceEvent, ElementState, MouseScrollDelta, WindowEvent};
use winit::window::Fullscreen;

fn vertex(pos: [f32; 3]) -> DisplayVertex {
    DisplayVertex {
        position: (pos[0], pos[1], pos[2]),
    }
}

enum Errors {
    SlicerCommunicationIssue,
    SlicerApplicationIssue,
    SlicerError(SlicerErrors),
}

impl Errors {
    ///Return the error code and pretty error message
    pub fn get_code_and_message(&self) -> (u32, String) {
        match self {
            Errors::SlicerApplicationIssue => {
                (0x8000, format!("Slicing Application could not be found."))
            }
            Errors::SlicerCommunicationIssue => (
                0x8001,
                format!("Error found in communication between GUI and slicer application."),
            ),
            Errors::SlicerError(e) => e.get_code_and_message(),
        }
    }
}
/*
fn create_mesh(
    display: &glium::Display,
) -> (glium::VertexBuffer<DisplayVertex>, glium::IndexBuffer<u32>) {
    create_cube_mesh(display, (-5.0, 5.0), (-5.0, 5.0), (0.0, 10.0))
}

fn create_bp_mesh(
    display: &glium::Display,
) -> (glium::VertexBuffer<DisplayVertex>, glium::IndexBuffer<u32>) {
    create_cube_mesh(display, (0.0, 20.0), (0.0, 20.0), (-0.1, -0.00))
}

fn create_cube_mesh(
    display: &glium::Display,
    x: (f32, f32),
    y: (f32, f32),
    z: (f32, f32),
) -> (glium::VertexBuffer<DisplayVertex>, glium::IndexBuffer<u32>) {
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
    let indices = glium::IndexBuffer::new(
        display,
        glium::index::PrimitiveType::TrianglesList,
        index_data,
    )
    .unwrap();
    (positions, indices)
}*/

fn create_build_area(
    display: &glium::Display,
    build_x: f32,
    build_y: f32,
    build_z: f32,
) -> (glium::VertexBuffer<DisplayVertex>, glium::IndexBuffer<u32>) {
    let mut vertex_positions: Vec<DisplayVertex> = vec![
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
        0, 1, 0, 2, 1, 3, 2, 3, 4, 5, 4, 6, 5, 7, 6, 7, 0, 4, 1, 5, 2, 6, 3, 7,
    ];

    let step_size = 10.0;
    (0..(build_x / step_size) as u32)
        .into_iter()
        .map(|index| index as f32 * step_size)
        .for_each(|x| {
            let index_pos = vertex_positions.len() as u32;
            vertex_positions.push(vertex([x, 0.0, 0.0]));
            vertex_positions.push(vertex([x, build_y, 0.0]));
            index_data.push(index_pos);
            index_data.push(index_pos + 1);
        });

    (0..(build_y / step_size) as u32)
        .into_iter()
        .map(|index| index as f32 * step_size)
        .for_each(|y| {
            let index_pos = vertex_positions.len() as u32;
            vertex_positions.push(vertex([0.0, y, 0.0]));
            vertex_positions.push(vertex([build_x, y, 0.0]));
            index_data.push(index_pos);
            index_data.push(index_pos + 1);
        });

    let positions = glium::VertexBuffer::new(display, &vertex_positions).unwrap();
    let indices =
        glium::IndexBuffer::new(display, glium::index::PrimitiveType::LinesList, &index_data)
            .unwrap();
    (positions, indices)
}

fn create_display(event_loop: &glutin::event_loop::EventLoop<()>) -> glium::Display {
    let window_builder = glutin::window::WindowBuilder::new()
        .with_resizable(true)
        .with_maximized(true)
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

    let mut camera_pitch = std::f32::consts::FRAC_PI_4;
    let mut camera_yaw = -std::f32::consts::FRAC_PI_4 + 0.12;
    let mut zoom = 400.0;
    let mut center_pos = (125.0, 105.0);
    let mut left_mouse_button_state = ElementState::Released;
    let mut on_render_screen = false;
    let mut in_window = false;
    let mut calc_vals = Arc::new(RwLock::new(None));
    let mut gcode = Arc::new(RwLock::new(None));
    let mut commands = Arc::new(RwLock::new(None));
    let mut error = Arc::new(RwLock::new(None));
    let mut command_running = Arc::new(RwLock::new(false));
    let mut command_state = Arc::new(RwLock::new(String::new()));
    let mut index = 0;
    let mut layers = 0;

    let mut viewer_open = false;

    let build_x = 250.0;
    let build_y = 210.0;
    let build_z = 210.0;

    let mut model_path = "".to_string();
    let mut settings_path = "".to_string();

    let model_program =
        glium::Program::from_source(&display, vertex_shader_src, fragment_shader_src, None)
            .unwrap();
    let line_program = glium::Program::from_source(
        &display,
        line_vertex_shader_src,
        line_fragment_shader_src,
        None,
    )
    .unwrap();

    let mut objects = vec![];

    let mut plot_window_resp = None;
    let mut window_rec = None;
    let mut window_clicked = false;

    let (line_positions, line_indices) = create_build_area(&display, build_x, build_y, build_z);
    event_loop.run(move |event, _, control_flow| {
        let mut redraw = || {
            let mut quit = false;


            let (needs_repaint, shapes) = egui_glium.run(&display, |egui_ctx| {

                plot_window_resp = None;
                window_clicked = false;
               let resp = egui::SidePanel::left("my_side_panel").show(egui_ctx, |ui| {
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

                           match load(&model_path, &display)
                           {
                               Ok(objs) => {objects.extend(objs.into_iter());}
                               Err(e) => {*error.write().unwrap() = Some(Errors::SlicerError(e))}
                           }
                       }
                   });
                   ui.horizontal(|ui| {
                       ui.label("Settings path: ");
                       let mut short = settings_path.clone();
                       if short.len() > 13{
                           short.truncate(10);
                           short += "...";
                       }
                       ui.label(short);
                   });
                   ui.horizontal(|ui| {
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
                           });
                           ui.horizontal(|ui| {
                               ui.add(egui::DragValue::new(&mut obj.location.x)
                                   .speed(1.0)
                                   .clamp_range(f64::NEG_INFINITY..=f64::INFINITY)
                                   .prefix("x: "));
                               ui.add(egui::DragValue::new(&mut obj.location.y)
                                   .speed(1.0)
                                   .clamp_range(f64::NEG_INFINITY..=f64::INFINITY)
                                   .prefix("y: "));
                               ui.add(egui::DragValue::new(&mut obj.scale.x)
                                   .speed(0.01)
                                   .clamp_range(0.0..=f64::INFINITY)
                                   .prefix("scale: "));

                               obj.scale.y = obj.scale.x;
                               obj.scale.z = obj.scale.x;
                           });

                           ui.horizontal(|ui| {
                               remove = ui.button("Remove").clicked();
                               duplicate = ui.button("Copy").clicked();
                               if ui.button("Center").clicked(){
                                   obj.location.x = build_x /2.0;
                                   obj.location.y = build_y /2.0;
                               }
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


                           if ui.add_enabled(!*command_running.read().unwrap() && !settings_path.is_empty() && !objects.is_empty(), egui::Button::new("Slice")).clicked() {
                               *calc_vals.write().unwrap() = None;
                               *gcode.write().unwrap() = None;
                               *error.write().unwrap() = None;
                               *commands.write().unwrap() = None;
                               *command_running.write().unwrap() = true;

                               index = 0;
                               layers = 0;

                               viewer_open = true;
                               let args :Vec<_>= objects.iter()
                                   .map(|obj|{
                                       format!("{{\"Raw\":[\"{}\",{:?}]}} ", obj.file_path.replace('\\', "\\\\"), (glam::Mat4::from_translation(obj.location) * glam::Mat4::from_scale(obj.scale) *glam::Mat4::from_translation(obj.default_offset)).transpose().to_cols_array_2d())
                                   })
                                   .collect();

                               let calc_vals_clone = calc_vals.clone();
                               let commands_clone = commands.clone();
                               let gcode_clone = gcode.clone();
                               let error_clone = error.clone();
                               let command_running_clone = command_running.clone();
                               let command_state_clone = command_state.clone();
                               let settings_path_clone = settings_path.clone();


                               std::thread::spawn(move ||{

                                   let mut command = if cfg!(target_os = "linux") {
                                       Command::new("./slicer/gladius_slicer")
                                   } else if cfg!(target_os = "windows") {
                                       Command::new("slicer\\gladius_slicer.exe")
                                   } else {
                                       unimplemented!()
                                   };

                                   for arg in &args {
                                       //"{\"Raw\":[\"test_3D_models\\3DBenchy.stl\",[[1.0,0.0,0.0,124.0],[0.0,1.0,0.0,105.0],[0.0,0.0,1.0,0.0],[0.0,0.0,0.0,1.0]] }"
                                       command.arg(arg);
                                   }

                                   let cpus = format!("{}", (num_cpus::get()).max(1));

                                   println!("{}",cpus);

                                   if let Ok(mut child) = command
                                       .arg("-m")
                                       .arg("-s")
                                       .arg(format!("{}", settings_path_clone.replace('\\', "\\\\")))
                                       .arg("-j")
                                       .arg(cpus)
                                       .stdout(Stdio::piped())
                                       .stderr(Stdio::piped())
                                       .spawn()
                                   {


                                       // Loop over the output from the first process
                                       if let Some(ref mut stdout) = child.stdout {
                                           while let Ok::<Message,_>( msg) = bincode::deserialize_from(&mut *stdout) {
                                               match msg {
                                                   Message::CalculatedValues(cv) => {
                                                       *calc_vals_clone.write().unwrap() = Some(cv);
                                                   }
                                                   Message::Commands(cmds) => {

                                                       *commands_clone.write().unwrap() = Some(cmds);
                                                   }
                                                   Message::GCode(str) => {
                                                       *gcode_clone.write().unwrap() = Some(str);
                                                   }
                                                   Message::Error(err) => {
                                                       *error_clone.write().unwrap() = Some(Errors::SlicerError(err));
                                                   }
                                                   Message::StateUpdate(msg) =>{
                                                       *command_state_clone.write().unwrap() = msg;
                                                   }
                                               }
                                           }
                                       }

                                       if let Some(ref mut stderr) = child.stderr {
                                           let mut buff = BufReader::new(stderr);
                                           if buff.lines().next().is_some() {
                                               *error_clone.write().unwrap() = Some(Errors::SlicerCommunicationIssue);
                                           }
                                       }
                                   }
                                   else{
                                       *error_clone.write().unwrap() = Some(Errors::SlicerApplicationIssue);

                                   }

                                   *command_running_clone.write().unwrap() = false;
                               });
                           }
                       });
                   });

                   if let Some(cv) = calc_vals.clone().read().unwrap().as_ref() {
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

                   if let Some(err) = error.read().unwrap().as_ref() {

                       let (code, message) = err.get_code_and_message();
                       ui.horizontal(|ui| {
                           ui.label(format!("Error {:#X}",code));
                       });
                       ui.horizontal(|ui| {
                           ui.label(format!("{}",message));
                       });
                   };

                   if *command_running.read().unwrap() {
                       ui.horizontal(|ui| {
                           ui.heading("Running");
                       });
                       ui.horizontal(|ui| {
                           ui.label(format!("Status {}",*command_state.read().unwrap()));
                       });
                   }

                   if let Some(str) = gcode.read().unwrap().as_ref() {
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
                                   file.write_all(str.as_bytes()).unwrap();
                               }
                           });
                       });
                   }
                   if let Some(cmds) = commands.read().unwrap().as_ref() {
                        plot_window_resp = egui::Window::new("2DViewer")
                            .open(&mut viewer_open)
                            .default_size(vec2(400.0, 400.0))
                            .show(egui_ctx, |ui| {


                                let mut line = Line::new(Values::from_values(vec![Value{x:0.0,y: 0.0},Value{x:0.0,y: build_y as f64 },Value{x:build_x as f64 ,y: build_y as f64 },Value{x:build_x as f64,y: 0.0  },Value{x:0.0,y: 0.0}])).width(20.0);


                                let mut layer_height = 0.0;


                                let drag_resp = ui.add(egui::DragValue::new(&mut index)
                                       .speed(1)
                                       .clamp_range(0..=layers-1)
                                       .prefix("x: "));


                                let plot = Plot::new("items_demo")
                                    .legend(Legend::default().position(Corner::RightBottom))
                                    .show_x(false)
                                    .show_y(false)
                                    .data_aspect(1.0);


                                let resp = plot.show(ui, |plot_ui| {
                                    plot_ui.line(line.name("Border"));

                                    let p1 = plot_ui.screen_from_plot(Value{x:0.0,y:0.0});
                                    let p2 = plot_ui.screen_from_plot(Value{x:0.0,y:1.0});


                                    let pixels_per_plot_unit = (p2-p1).length();


                                    let mut moves : Vec<_>= (&cmds
                                        .iter()
                                        .group_by(|cmd|{
                                            if let gladius_shared::types::Command::LayerChange { z } = cmd{
                                                let r = *z == layer_height;
                                                layer_height = *z;
                                                r
                                            }else{
                                                true
                                            }
                                        }))
                                        .into_iter()
                                        .filter_map(|(change,layer)|{
                                            if !change{
                                                None
                                            }else{
                                                let lines :Vec<_> = layer.into_iter().filter_map(|cmd|{

                                                    if let gladius_shared::types::Command::MoveAndExtrude{start,end,width,..} = cmd{
                                                        Some(Line::new(Values::from_values(vec![Value{x:start.x,y:start.y},Value{x:end.x,y: end.y }]))
                                                                 .stroke(Stroke{width: *width as f32 * pixels_per_plot_unit,color: Color32::BLUE}))

                                                    }else{
                                                        None
                                                    }
                                                }).collect();

                                                Some(lines)
                                            }
                                        })
                                        .collect();

                                    layers = moves.len();

                                    for move_line in moves.remove(index){
                                        plot_ui.line(move_line.name("Move"));
                                    }
                                });

                                drag_resp.union(resp.response)
                            });
                   }



                });

                window_rec = plot_window_resp.as_ref().map(|r| r.response.rect.clone());

                let full_resp = match plot_window_resp.take(){
                    Some(mut window_resp) => {

                        resp.response.union(window_resp.response.interact(Sense::click_and_drag()).union(window_resp.inner.take().unwrap()))
                    },
                    None => {
                        resp.response
                    }
                };
                 //println!("here {} {} {}",full_resp.hovered(),full_resp.dragged(),full_resp.is_pointer_button_down_on());

                on_render_screen = !full_resp.hovered() && !full_resp.dragged() && !full_resp.is_pointer_button_down_on();
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
                    let model = (glam::Mat4::from_translation(obj.location) * glam::Mat4::from_scale(obj.scale) *glam::Mat4::from_translation(obj.default_offset)).to_cols_array_2d();
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
                        on_render_screen = on_render_screen &&  if let Some(rect ) = window_rec {
                            //println!("{:?} {:?}",rect,position);
                            !rect.contains(Pos2{x: position.x as f32,y: position.y as f32 })
                        }
                        else{
                            true
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
                        if left_mouse_button_state == ElementState::Pressed && on_render_screen && in_window{
                            camera_yaw += dx as f32 * -0.01;
                            camera_pitch = (camera_pitch + dy as f32 * 0.01).min(std::f32::consts::FRAC_PI_2 - 0.001).max(-std::f32::consts::FRAC_PI_2 + 0.001);
                        }

                    }
                    DeviceEvent::MouseWheel {delta} =>{
                        if  on_render_screen && in_window {
                            if let MouseScrollDelta::LineDelta(_x, y) = delta {
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
