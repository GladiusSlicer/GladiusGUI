#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
mod object;
mod shaders;
mod model;

use crate::object::{load, DisplayVertex, Object};
use crate::shaders::*;
use crate::model::*;

use native_dialog::FileDialog;

use egui::plot::{Corner, Legend, Line, Plot, Value, Values};
use egui::{
    Color32, FontDefinitions, FontFamily, InnerResponse, Pos2, Sense, Stroke, TextStyle,
};
use gladius_shared::error::SlicerErrors;
use gladius_shared::messages::Message;
use glam::{Vec2, Vec3};
use glium::{glutin, Surface, uniform};
use itertools::Itertools;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, RwLock};
use json_gettext::JSONGetText;
use winit::event::{DeviceEvent, ElementState, MouseScrollDelta, WindowEvent};
#[macro_use] extern crate json_gettext;
fn vertex(pos: [f32; 3]) -> DisplayVertex {
    DisplayVertex {
        position: (pos[0], pos[1], pos[2]),
    }
}

#[derive(Clone, Debug)]
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

    let ctx = static_json_gettext_build!(
        "en_US";
        "en_US" => "langs/en_US.json",
    ).unwrap();

    let lang = "en_US";

    let event_loop = glutin::event_loop::EventLoop::with_user_event();
    let display = create_display(&event_loop);

    let mut egui_glium = egui_glium::EguiGlium::new(&display);


    let mut left_mouse_button_state = ElementState::Released;
    let mut on_render_screen = false;
    let mut in_window = false;
    let mut index = 0;
    let mut layers = 0;


    let mut viewer_open = false;

    let build_x = 250.0;
    let build_y = 210.0;
    let build_z = 210.0;

    let model_program =
        glium::Program::from_source(&display, VERTEX_SHADER_SRC, FRAGMENT_SHADER_SRC, None)
            .unwrap();
    let line_program = glium::Program::from_source(
        &display,
        LINE_VERTEX_SHADER_SRC,
        LINE_FRAGMENT_SHADER_SRC,
        None,
    )
    .unwrap();

    let mut gui_data = GUIData::new(Vec2::new(400.0, 400.0),Vec3::new(build_x,build_y,build_z));



    let mut plot_window_resp = None;
    let mut window_rec = None;
    let mut window_clicked = false;

    let r = gui_data.check_refresh_and_clear();

    let (line_positions, line_indices) = create_build_area(&display, build_x, build_y, build_z);
    event_loop.run(move |event, _, control_flow| {
        let mut redraw = || {
            let mut quit = false;


            let needs_repaint = egui_glium.run(&display, |egui_ctx| {

                gui_data.update_colors();

                plot_window_resp = None;
                window_clicked = false;
               let resp = egui::SidePanel::left("my_side_panel").show(&egui_ctx, |ui| {
                   ui.heading(&get_translated_string(&ctx, lang, "setup_bar_heading"));
                   ui.horizontal(|ui| {
                       ui.label(&get_translated_string(&ctx, lang, "model_path"));
                       if ui.button(&get_translated_string(&ctx, lang,  "choose_model_button")).clicked() {
                            gui_data.load_model(    &display);
                       }
                   });
                   ui.horizontal(|ui| {
                       ui.label(&get_translated_string(&ctx, lang, "settings_path"));
                       let mut short = gui_data.get_settings_path().clone();
                       if short.len() > 13{
                           short.truncate(10);
                           short += "...";
                       }
                       ui.label(short);
                   });
                   ui.horizontal(|ui| {
                       if ui.button("Choose settings").clicked() {
                            gui_data.load_settings_file();
                       }
                   });
                   ui.group(|ui| {

                       gui_data.get_objects().iter().enumerate()
                           .for_each(|(i,mut obj)| {
                           ui.horizontal(|ui| {
                               ui.label(obj.name.to_string());
                           });

                           let mut changed = false;

                           /*ui.horizontal(|ui| {
                               changed |= ui.add(egui::DragValue::new(&mut obj.get_mut_location().x)
                                   .speed(1.0)
                                   .clamp_range(f64::NEG_INFINITY..=f64::INFINITY)
                                   .prefix("x: "))
                                   .changed();
                               changed |= ui.add(egui::DragValue::new(&mut obj.get_mut_location().y)
                                   .speed(1.0)
                                   .clamp_range(f64::NEG_INFINITY..=f64::INFINITY)
                                   .prefix("y: "))
                                   .changed();
                               changed |= ui.add(egui::DragValue::new(&mut obj.get_mut_scale().x)
                                   .speed(0.01)
                                   .clamp_range(0.0..=f64::INFINITY)
                                   .prefix("scale: "))
                                   .changed();

                               obj.get_mut_scale().y = obj.get_scale().x;
                               obj.get_mut_scale().z = obj.get_scale().x;
                           });

                           ui.horizontal(|ui| {
                               if ui.button(&get_translated_string(&ctx, lang,"remove")).clicked() {
                                   remove = Some(i);
                               };
                               if ui.button(&get_translated_string(&ctx, lang, "copy")).clicked(){
                                   copy = Some(i)
                               }
                               if ui.button(&get_translated_string(&ctx, lang, "center")).clicked(){
                                   obj.get_mut_location().x = build_x /2.0;
                                   obj.get_mut_location().y = build_y /2.0;
                                   changed = true;
                               }

                           });*/

                           if changed{
                               //obj.revalidate_cache();
                               //*gcode.write().unwrap() = None;
                               //*calc_vals.write().unwrap() = None;
                           }
                       });
                   });

                   ui.horizontal(|ui| {


                       ui.style_mut().spacing.button_padding = egui::Vec2::new(50., 20.);
                       ui.style_mut().override_text_style = Some(TextStyle::Heading);


                       ui.centered_and_justified(|ui| {
                           let mut fonts = FontDefinitions::default();

                           // Large button text:

                           //ui.ctx().set_fonts(fonts);


                           if ui.add_enabled( gui_data.can_slice(), egui::Button::new(&get_translated_string(&ctx, lang, "slice"))).clicked() {
                               index = 0;
                                layers = 0;
                                viewer_open = true;

                               gui_data.start_slice();
                           }
                       });
                   });

                   if let Some(cv) = gui_data.get_calculated_values(){
                       ui.horizontal(|ui| {
                           ui.label(get_translated_string_argument(&ctx,lang,"plastic_volume_msg",format!("{:.0}",cv.plastic_volume)));
                       });
                       ui.horizontal(|ui| {
                           ui.label(get_translated_string_argument(&ctx,lang,"plastic_weight_msg",format!("{:.0}",cv.plastic_weight)));

                       });
                       ui.horizontal(|ui| {
                           let (hour,min,_,_) = cv.get_hours_minutes_seconds_fract_time();
                           ui.label(get_translated_string_arguments(&ctx,lang,"print_time_msg",&[hour.to_string(), min.to_string()]));
                       });
                   };

                   for err in gui_data.get_errors(){

                       let (code, message) = err.get_code_and_message();
                       ui.horizontal(|ui| {
                           ui.label(format!("Error {:#X}",code));
                       });
                       ui.horizontal(|ui| {
                           ui.label(message.to_string());
                       });
                   };

                   if gui_data.is_command_running(){
                       ui.horizontal(|ui| {
                           ui.heading("Running");
                       });
                       ui.horizontal(|ui| {
                           ui.label(format!("Status {}",gui_data.get_command_state()));
                       });
                   }

                   if let Some(str) = gui_data.get_gcode() {
                       ui.horizontal(|ui| {
                           ui.style_mut().spacing.button_padding = egui::Vec2::new(50., 20.);
                           //ui.style_mut().body_text_style = TextStyle::Heading;
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
                   if let Some(cmds) = gui_data.get_commands() {
                        plot_window_resp = egui::Window::new(&get_translated_string(&ctx, lang, "viewer"))
                            .open(&mut viewer_open)
                            .default_size(egui::Vec2::new(400.0, 400.0))
                            .show(&egui_ctx, |ui| {


                                let line = Line::new(Values::from_values(vec![Value{x:0.0,y: 0.0},Value{x:0.0,y: build_y as f64 },Value{x:build_x as f64 ,y: build_y as f64 },Value{x:build_x as f64,y: 0.0  },Value{x:0.0,y: 0.0},Value{x:0.0,y: build_y as f64 }])).width(5.0);


                                let mut layer_height = 0.0;

                                ui.style_mut().spacing.slider_width = ui.available_width() - 100.0;
                                let drag_resp = ui.add(egui::Slider::new(&mut index, 1..=layers-1)
                                    .prefix("x: "));


                                let plot = Plot::new("items_demo")
                                    .legend(Legend::default().position(Corner::RightBottom))
                                    .show_x(false)
                                    .show_y(false)
                                    .data_aspect(1.0);


                                let resp = plot.show(ui, |plot_ui| {
                                    plot_ui.line(line.name(&get_translated_string(&ctx, lang, "Border")));

                                    let p1 = plot_ui.screen_from_plot(Value{x:0.0,y:0.0});
                                    let p2 = plot_ui.screen_from_plot(Value{x:0.0,y:1.0});


                                    let pixels_per_plot_unit = (p2-p1).length();


                                    let mut moves : Vec<_>= (&cmds
                                        .iter()
                                        .group_by(|cmd|{
                                            if let gladius_shared::types::Command::LayerChange { z, index: _index } = cmd{
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
                                                                 .stroke(Stroke{width: *width as f32 * pixels_per_plot_unit* 0.95,color: Color32::BLUE}))

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

                window_rec = plot_window_resp.as_ref().map(|r| r.response.rect);

                let full_resp = match plot_window_resp.take(){
                    Some(mut window_resp) => {

                        resp.response.union(window_resp.response.interact(Sense::click_and_drag()).union(window_resp.inner.take().unwrap()))
                    },
                    None => {
                        resp.response
                    }
                };

               on_render_screen = !egui_ctx.wants_pointer_input();
            });

            *control_flow = if quit {
                glutin::event_loop::ControlFlow::Exit
            } else if needs_repaint {

                gui_data.check_refresh_and_clear();
                display.gl_window().window().request_redraw();

                glutin::event_loop::ControlFlow::Poll
            } else if gui_data.is_command_running(){
                //If command is running keep refreshing
                glutin::event_loop::ControlFlow::Poll
            }else{
                glutin::event_loop::ControlFlow::Wait
            };


            {
                use glium::Surface as _;
                let mut target = display.draw();

                let color = egui::Rgba::from_rgb(0.0, 0.0, 0.0);
                target.clear_color_and_depth((color[0], color[1], color[2], color[3]),1.0);

                // draw things behind egui here

                let (view ,perspective ) =  gui_data.get_camera_view_and_proj_matrix();

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

                gui_data.update_colors();
                gui_data.update_screen_dimensions(Vec2::new(target.get_dimensions().0 as f32,target.get_dimensions().1 as f32 ));

                for obj in gui_data.get_objects(){
                    let model = obj.get_model_matrix().to_cols_array_2d();
                    let color = obj.color.to_array();
                    let (positions, indices) = (&obj.vert_buff,&obj.index_buff);//create_mesh(&display);
                    target.draw(positions, indices, &model_program, &uniform! {color: color,  model: model, view: view, perspective: perspective }, &params).unwrap();
                }


                target.draw(&line_positions, &line_indices, &line_program, &uniform! { model: line_model, view: view, perspective: perspective }, &params).unwrap();

                egui_glium.paint(&display, &mut target);

                // draw things on top of egui here

                target.finish().unwrap();
            }
        };




        if r {
            redraw();
        }

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

                        if on_render_screen {
                            gui_data.mouse_move(Vec2::new(position.x as f32, position.y as f32));
                        }

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
                            if left_mouse_button_state == ElementState::Released && state == ElementState::Pressed {
                                gui_data.select_button_pressed()
                            }
                            if state == ElementState::Released {

                                gui_data.select_button_released()
                            }
                            left_mouse_button_state = state;
                        }
                    }

                    DeviceEvent::MouseMotion {delta: (dx,dy)} =>{
                        if left_mouse_button_state == ElementState::Pressed && on_render_screen && in_window{
                            gui_data.mouse_move_delta(Vec2::new(dx as f32,dy as f32))
                        }
                    }
                    DeviceEvent::MouseWheel {delta} =>{
                        if  on_render_screen && in_window {
                            if let MouseScrollDelta::LineDelta(_x, y) = delta {
                                gui_data.mouse_wheel(y)
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


fn get_translated_string(ctx:& JSONGetText, lang: &str,index_str: &str) -> String{
    get_text!(ctx,lang, index_str).map_or(String::from("Not Translated"),|s| s.to_string())
}
fn get_translated_string_argument(ctx:& JSONGetText, lang: &str,index_str: &str, argument: String) -> String{
    get_text!(ctx,lang, index_str).map_or(String::from("Not Translated {}"),|s| s.to_string()).replace("{}", &argument)
}

fn get_translated_string_arguments(ctx:& JSONGetText, lang: &str,index_str: &str, arguments: &[String]) -> String{
    let mut ret_string = get_text!(ctx,lang, index_str).map_or(String::from("Not Translated {}"),|s| s.to_string());
    for (count,arg) in arguments.iter().enumerate(){
        ret_string = ret_string.replace(&format!("{{{}}}",count), &arg);
    }

    ret_string
}