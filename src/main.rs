#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
mod gui_app;
mod shaders;
mod object;
mod model;

use crate::shaders::*;


use glium::backend::glutin::SimpleWindowBuilder;
use glium::glutin::config::ConfigTemplateBuilder;
use glium::glutin::surface::WindowSurface;
use gui_app::GuiApp;
use model::Camera;
use native_dialog::FileDialog;


use egui::{
    Color32, FontDefinitions, FontFamily, InnerResponse, Pos2, Sense, Stroke, TextStyle, ViewportId,
};
use gladius_shared::prelude::*;
use glam::{Vec2, Vec3};
use glium::{glutin, Surface, uniform};
use itertools::Itertools;
use object::DisplayVertex;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::Window;
use std::fs::File;
use json_gettext::JSONGetText;
use winit::event::{DeviceEvent, ElementState, MouseScrollDelta, WindowEvent};
#[macro_use] extern crate json_gettext;

fn vertex(pos: [f32; 3]) -> DisplayVertex {
    DisplayVertex {
        position: (pos[0], pos[1], pos[2]),
    }
}

#[derive( Debug)]
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



fn create_display(event_loop: &EventLoop<()>) -> (Window,glium::Display<WindowSurface>) {
    SimpleWindowBuilder::new()
        .set_window_builder(
            Window::default_attributes()
            .with_resizable(true)
            .with_maximized(true)
        )
        .with_inner_size(800, 600)
        .with_title("Gladius")
        .with_config_template_builder(
            ConfigTemplateBuilder::default()
                .with_depth_size(24)
                .with_multisampling(16)
        )
        .build(event_loop)
}

fn main() {

    let ctx = static_json_gettext_build!(
        "en_US";
        "en_US" => "langs/en_US.json",
    ).unwrap();

    let lang = "en_US";

    let event_loop = EventLoop::new().unwrap();
    let (window,display) = create_display(&event_loop);

    let egui_glium = egui_glium::EguiGlium::new(ViewportId::ROOT, &display, &window, &event_loop);


    let mut left_mouse_button_state = ElementState::Released;
    let mut on_render_screen = true;
    let mut in_window = false;
    let mut index = 0;
    let mut layers : usize = 0;


    let mut viewer_open = false;

    let mut build_x = 220.0;
    let mut build_y = 220.0;
    let mut build_z = 220.0;



    //let mut gui_data = GUIData::new(Vec2::new(400.0, 400.0),Vec3::new(build_x,build_y,build_z));



    //let mut plot_window_resp = None;
    //let mut window_rec = None;
    let mut window_clicked = false;

    //let r = gui_data.check_refresh_and_clear();

    //let (mut line_positions, mut line_indices) = create_build_area(&display, build_x, build_y, build_z);

    let egui_glium = egui_glium::EguiGlium::new(ViewportId::ROOT, &display, &window, &event_loop);

    let mut center_pos = (220.0/2.0,220.0/2.0);

    let mut gui_app= GuiApp {
        objects: vec![],
        egui_glium,
        window,
        display,
        camera: Camera::new(Vec2::new(400.0, 400.0), Vec3::new(center_pos.0 ,center_pos.1 , 0.0)),
        printer: gui_app::PrinterInfo{build_x:220., build_y:220., build_z: 220.},
        settings_panel: gui_app::SettingsPanel::default(),
        left_mouse_button_state,
        closest_object_point: None,
        selected_object: None,
        dragging: false,
        on_render_screen,
    };

    event_loop.run_app(&mut gui_app).unwrap();

    
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