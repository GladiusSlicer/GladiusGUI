use std::{default, path::PathBuf};

use egui::{scroll_area::ScrollBarVisibility, Color32, Frame, Image, Layout, Sense, Ui, ViewportId};
use egui_extras::{Size, StripBuilder};
use gladius_shared::{error::SlicerErrors, prelude::*};
use glam::{ Vec2, Vec3};
use glium::{backend::glutin::SimpleWindowBuilder, glutin::surface::WindowSurface, uniform};
use native_dialog::FileDialog;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, MouseScrollDelta, StartCause, WindowEvent},
    event_loop::{self, ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

use crate::{model::Camera, object::{DisplayVertex, Object}, vertex, FRAGMENT_SHADER_SRC, LINE_FRAGMENT_SHADER_SRC, LINE_VERTEX_SHADER_SRC, VERTEX_SHADER_SRC};


pub struct GuiApp {
    pub objects: Vec<Object>,
    pub egui_glium: egui_glium::EguiGlium,
    pub window: winit::window::Window,
    pub display: glium::Display<WindowSurface>,
    pub camera: Camera,
    pub printer: PrinterInfo,
    pub settings_panel: SettingsPanel,
    pub left_mouse_button_state : ElementState,
    pub closest_object_point: Option<(usize,Vec3,Vec3 )>,
    pub selected_object: Option<usize >,
    pub dragging: bool,
    pub on_render_screen: bool,
}

impl ApplicationHandler for GuiApp {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let mut redraw = || {
            let mut quit = false;

            self.egui_glium.run(&self.window, |egui_ctx| {
                egui_extras::install_image_loaders(egui_ctx);

                egui::TopBottomPanel::top("menu_bar").show(egui_ctx, |ui| {
                    egui::menu::bar(ui, |ui| {
                        ui.menu_button("File", |ui| {
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                    
                            if ui
                                .add(
                                    egui::Button::new("Organize Windows"),
                                )
                                .clicked()
                            {
                                ui.ctx().memory_mut(|mem| mem.reset_areas());
                                ui.close_menu();
                            }
                    
                        });
                    });
                });

                egui::SidePanel::right("Settings_Menu").show(egui_ctx, |ui|{
                    self.settings_panel.build_settings_panel(ui);
                });




                egui::SidePanel::left("my_side_panel").show_separator_line(false).frame(Frame::default().fill(Color32::from_white_alpha(0))).show(egui_ctx, |ui| {
                    ui.vertical_centered( |ui| {
                        let spacing = ui.available_width()*3.0;
                        StripBuilder::new(ui)
                            .size(Size::remainder())
                            .size(Size::exact(spacing))
                            .size(Size::remainder())
                            .vertical(|mut strip|{


                                strip.cell(|ui|{});
                                strip.cell(|ui|{
                                    let mut frame = egui::Frame::default()
                                        .stroke((2.0,ui.visuals().window_stroke.color))
                                        .rounding(10.0)
                                        .inner_margin(10.0)
                                        .outer_margin(5.0)
                                        .fill(Color32::from_white_alpha(0))
                                        .show(ui, |ui|{

                                            Self::draw_square_button(
                                                ui, 
                                                egui::include_image!("..\\resources\\icons\\toolbar_open.svg"), 
                                                egui::include_image!("..\\resources\\icons\\toolbar_open_selected.svg"),
                                                ui.available_width(),
                                            ||{
                                                Self::load_model(&mut self.objects,&self.display);
                                            },
                                                "Open".to_string(),
                                            true);

                                            Self::draw_square_button(
                                                ui, 
                                                egui::include_image!("..\\resources\\icons\\toolbar_delete.svg"), 
                                                egui::include_image!("..\\resources\\icons\\toolbar_delete_selected.svg"),
                                                ui.available_width(),
                                            ||{},
                                                "Delete".to_string(),
                                            true);

                                            Self::draw_square_button(
                                                ui, 
                                                egui::include_image!("..\\resources\\icons\\toolbar_move.svg"), 
                                                egui::include_image!("..\\resources\\icons\\toolbar_move_selected.svg"),
                                                ui.available_width(),
                                            ||{},
                                                "Move".to_string(),
                                            false);
                                            
                                        });
            

                                    



                                });
                                strip.cell(|ui|{});
                            })


                    });
                });

            });

            if quit {
                event_loop.exit()
            }

            {
                use glium::Surface as _;
                let mut target = self.display.draw();

                let color = egui::Rgba::from_rgb(0.0, 0.0, 0.0);
                target.clear_color_and_depth((color[0], color[1], color[2], color[3]),1.0);

                self.update_screen_dimensions(Vec2::new(target.get_dimensions().0 as f32,target.get_dimensions().1 as f32 ));

                
                let (mut line_positions, mut line_indices) = create_build_area(&self.display, &self.printer);

                // draw things behind egui here

                let line_model = glam::Mat4::from_translation(Vec3::new(0.0,0.0,0.0)).to_cols_array_2d();
                
                let line_program = glium::Program::from_source(
                    &self.display,
                    LINE_VERTEX_SHADER_SRC,
                    LINE_FRAGMENT_SHADER_SRC,
                    None,
                )
                .unwrap();
                let (view ,perspective ) =  self.get_camera_view_and_proj_matrix();

                
                let params = glium::DrawParameters {
                    depth: glium::Depth {
                        test: glium::draw_parameters::DepthTest::IfLess,
                        write: true,
                        .. Default::default()
                    },
                    //backface_culling: glium::draw_parameters::BackfaceCullingMode::CullClockwise,
                    .. Default::default()
                };

                target.draw(&line_positions, &line_indices, &line_program, &uniform! { model: line_model, view: view, perspective: perspective }, &params).unwrap();



                let model_program = glium::Program::from_source(
                    &self.display,
                    VERTEX_SHADER_SRC,
                    FRAGMENT_SHADER_SRC,
                    None,
                )
                .unwrap();

            
                for obj in &self.objects{
                    let model = obj.get_model_matrix().to_cols_array_2d();
                    let color = obj.color.to_array();
                    let (positions, indices) = (&obj.vert_buff,&obj.index_buff);//create_mesh(&display);
                    target.draw(positions, indices, &model_program, &uniform! {color: color,  model: model, view: view, perspective: perspective }, &params).unwrap();
                }


                //draw egui here
                self.egui_glium.paint(&self.display, &mut target);

                // draw things on top of egui here

                target.finish().unwrap();
            }
        };

              
        use winit::event::WindowEvent;
        match &event {
            WindowEvent::CloseRequested | WindowEvent::Destroyed => event_loop.exit(),
            WindowEvent::Resized(new_size) => {
                self.display.resize((*new_size).into());
            }
            WindowEvent::RedrawRequested => redraw(),
            WindowEvent::MouseWheel { device_id, delta, phase } =>{
                if let MouseScrollDelta::LineDelta(_x, y) = delta {
                    self.mouse_wheel(*y)
                }
            }
            WindowEvent::MouseInput { button,state,.. } =>{
                if *button == MouseButton::Left{
                    if self.left_mouse_button_state == ElementState::Released && *state == ElementState::Pressed && self.on_render_screen {
                        self.select_button_pressed()
                    }
                    if *state == ElementState::Released {
                        self.select_button_released()
                    }
                    self.left_mouse_button_state = *state;
                }
            }
            WindowEvent::CursorMoved { device_id, position }  =>{
                if self.on_render_screen {
                    self.mouse_move(Vec2::new(position.x as f32, position.y as f32));
                }

            }
            _ => {}
        }

        let event_response = self.egui_glium.on_event(&self.window, &event);

        if event_response.repaint {
            self.window.request_redraw();
        }
    }


    fn device_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            device_id: winit::event::DeviceId,
            event: winit::event::DeviceEvent,
        ) {
        match event{
            winit::event::DeviceEvent::MouseMotion {delta: (dx,dy)} =>{
                if self.left_mouse_button_state == ElementState::Pressed && self.on_render_screen {
                    self.mouse_move_delta(Vec2::new(dx as f32,dy as f32))
                }
            }
            _ => {}
        }
    }
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if let StartCause::ResumeTimeReached { .. } = cause {
            self.window.request_redraw();
        }
    }
}


impl GuiApp{

    pub fn select_button_pressed(&mut self){

        if let Some((i,_,_)) = self.closest_object_point {
            if !self.dragging {
                self.dragging = true;
                self.selected_object = Some(i);
            }
        }
        else{
            self.selected_object = None;
        }
    }

    pub fn select_button_released(&mut self){

        self.dragging = false;

    }
    pub fn mouse_move(&mut self, new_position: Vec2) {

        let inv_vp = (self.camera.proj_mat * self.camera.view_mat ).inverse();

        let mouse_pos = Vec3::new((new_position.x / (self.camera.screen_dimensions.x as f32 * 0.5) - 1.0) as f32, -(new_position.y / (self.camera.screen_dimensions.y as f32 * 0.5) - 1.0) as f32, 1.0);

        let world_pos = inv_vp.transform_point3(mouse_pos);

        let cam_dir = world_pos.normalize();

        if !self.dragging {
            self.closest_object_point = self.objects
                .iter_mut()
                .enumerate()
                .filter_map(|(en, obj)| {
                    obj.hovered = false;
                    obj.intersect_with_ray(self.camera.location, cam_dir).map(|p| (en, obj, p))
                })
                .min_by(|(_, _, (ta, _)), (_, _, (tb, _))| ta.partial_cmp(tb).unwrap())
                .map(|(index, obj, point)| (index, point.1,*obj.get_location()));

            if let Some((index, _,_)) = self.closest_object_point {
                self.objects[index].hovered = true;
            }
        }else{
            let (index, intersect_point, translation) = self.closest_object_point.expect("If selected closest point must be set");
            let z_height = intersect_point.z;

            let (x_intercept, y_intercept) ={
                let z_diff = self.camera.location.z - z_height;
                let y_over_z_slope =  cam_dir.y / cam_dir.z;
                let x_over_z_slope =  cam_dir.x / cam_dir.z;

                let x_diff = x_over_z_slope *  z_diff;
                let y_diff = y_over_z_slope *  z_diff;

                (self.camera.location.x - x_diff, self.camera.location.y - y_diff)
            };

            let x_diff =  x_intercept - intersect_point.x;
            let y_diff =  y_intercept - intersect_point.y;

            self.objects[index].get_mut_location().x = translation.x + x_diff;
            self.objects[index].get_mut_location().y = translation.y + y_diff;

            self.objects[index].revalidate_cache();
            //*gcode.write().unwrap() = None;
            //*calc_vals.write().unwrap() = None;

        }


    }

    
    pub fn can_slice(&self) -> bool{
        false
// !self.objects.is_empty() && !self.settings_path.is_empty() && ! *self.command_running.read().unwrap()
    }

    pub fn start_slice(&mut self) {

    }


    pub fn mouse_move_delta(&mut self, delta: Vec2) {
        if !self.dragging {
            self.camera.yaw += delta.x as f32 * -0.01;
            self.camera.pitch = (self.camera.pitch + delta.y as f32 * 0.01).min(std::f32::consts::FRAC_PI_2 - 0.001).max(-std::f32::consts::FRAC_PI_2 + 0.001);
        }
    }

    pub fn load_model(objects: &mut Vec<Object>, display: &glium::Display<WindowSurface>){
        let paths = FileDialog::new()
           .add_filter("Supported Model Types", &["stl", "3mf"])
           .show_open_multiple_file()
           .unwrap();

        for path in paths {

           let model_path = path.into_os_string().into_string().unwrap();

           match crate::object::load(&model_path,display)
           {
               Ok(objs) => { objects.extend(objs); }
               Err(e) => { /**self.error.write().unwrap() = Some(Errors::SlicerError(e))*/ panic!("Failed to load") }
           }
        }
    }

    pub fn get_camera_view_and_proj_matrix(&mut self) -> ([[f32;4];4],[[f32;4];4]){
        self.camera.rebuild_matrices();
        (self.camera.get_camera_view_matrix(),self.camera.get_camera_proj_matrix())
    }

    pub fn update_screen_dimensions(&mut self,screen_dimensions: Vec2){
        self.camera.update_screen_dimensions(screen_dimensions)
    }


    pub fn mouse_wheel(&mut self, wheel_move: f32) {
        self.camera.zoom = (self.camera.zoom * (1.0 - (0.1 * wheel_move.signum()))).min(1000.0).max(5.0);
    }

    pub fn draw_square_button<'a>(ui: &mut Ui, image: egui::ImageSource<'a>,hover_image: egui::ImageSource<'a>, size: f32, pressed: impl FnOnce(), hover_text: String, enabled: bool) {

        let (rect,resp) = ui.allocate_exact_size(egui::Vec2::new(size,size ), Sense::click());

        let final_icon = egui::Image::from(if resp.hovered() && enabled { hover_image }else{ image }).tint(if enabled {Color32::from_white_alpha(255)} else {Color32::from_rgb(64,64,64)}); 

        if resp.clicked(){
            pressed()
        }

        resp.on_hover_text(hover_text);
        final_icon.paint_at(ui, rect);

    }





}

fn create_build_area(
    display: &glium::Display<WindowSurface>,
    printer: &PrinterInfo

) -> (glium::VertexBuffer<DisplayVertex>, glium::IndexBuffer<u32>) {
    let mut vertex_positions: Vec<DisplayVertex> = vec![
        // far side (0.0, 0.0, 1.0)
        vertex([0.0, 0.0, 0.0]),
        vertex([0.0, printer.build_y, 0.0]),
        vertex([printer.build_x, 0.0, 0.0]),
        vertex([printer.build_x, printer.build_y, 0.0]),
        vertex([0.0, 0.0, printer.build_z]),
        vertex([0.0, printer.build_y, printer.build_z]),
        vertex([printer.build_x, 0.0, printer.build_z]),
        vertex([printer.build_x, printer.build_y, printer.build_z]),
    ];

    let mut index_data: Vec<u32> = vec![
        0, 1, 0, 2, 1, 3, 2, 3, 4, 5, 4, 6, 5, 7, 6, 7, 0, 4, 1, 5, 2, 6, 3, 7,
    ];

    let step_size = 10.0;
    (0..(printer.build_x / step_size) as u32)
        .into_iter()
        .map(|index| index as f32 * step_size)
        .for_each(|x| {
            let index_pos = vertex_positions.len() as u32;
            vertex_positions.push(vertex([x, 0.0, 0.0]));
            vertex_positions.push(vertex([x, printer.build_y, 0.0]));
            index_data.push(index_pos);
            index_data.push(index_pos + 1);
        });

    (0..(printer.build_y / step_size) as u32)
        .into_iter()
        .map(|index| index as f32 * step_size)
        .for_each(|y| {
            let index_pos = vertex_positions.len() as u32;
            vertex_positions.push(vertex([0.0, y, 0.0]));
            vertex_positions.push(vertex([printer.build_x, y, 0.0]));
            index_data.push(index_pos);
            index_data.push(index_pos + 1);
        });

    let positions = glium::VertexBuffer::new(display, &vertex_positions).unwrap();
    let indices =
        glium::IndexBuffer::new(display, glium::index::PrimitiveType::LinesList, &index_data)
            .unwrap();
    (positions, indices)
}

pub struct PrinterInfo{
    pub build_x: f32,
    pub build_y: f32,
    pub build_z: f32,
}

pub struct SettingsPanel{
    setting_entities: Vec<Option<SettingsEntry>>
}

impl Default for SettingsPanel{
    fn default() -> Self {
        Self { setting_entities: vec![None,None,None] }
    }
}


impl SettingsPanel{

    pub fn build_settings_panel(&mut self,ui: &mut Ui) {
        ui.add_space(4.0);
        ui.vertical_centered(|ui| {
            ui.heading("Settings");
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .scroll_bar_visibility(ScrollBarVisibility::VisibleWhenNeeded)
            .show(ui, |ui| {


                ui.with_layout(
                    egui::Layout::top_down(egui::Align::LEFT).with_cross_justify(true),
                    |ui| {
                        for (index,ent) in self.setting_entities.iter_mut().enumerate(){
                            Self::build_settings_entry(  ent, ui,index);
                        }
                    
                    },
                );
            });
    }

    fn build_settings_entry(opt_entry: &mut Option<SettingsEntry>,ui: &mut Ui, index: usize){

        egui::Frame::default()
            .stroke((2.0,ui.visuals().window_stroke.color))
            .rounding(4.0)
            .inner_margin(10.0)
            .outer_margin(5.0)
            .show(ui, |ui| {

                ui.horizontal(|ui|{
                    ui.heading("⬍");
                    if let Some(entry) =  opt_entry{
                        match entry{
                            SettingsEntry::File { file_path, partial_file } => {
            
                                if let Some(path ) = file_path{
                                    match partial_file{
                                        Ok(_) => {
                                            ui.label(path.to_str().unwrap());
                                        },
                                        Err(error) => {
                                            ui.label(&*error);
                                        },
                                    }
                                }
                                else{
                                    if ui.button("Load File").clicked(){
                                        *file_path = native_dialog::FileDialog::new()
                                            .add_filter("Supported settings Types", &["json"])
                                            .show_open_single_file().ok().flatten();
            
                                        if let Some(new_path) = file_path{
                                            let result ={
                                                gladius_shared::input::load_settings_json(new_path.to_str().unwrap())
                                                .map_err(|err| "Error Loading file".to_string() )
                                                .and_then(|settings_data| deser_hjson::from_str::<PartialSettingsFile>(&settings_data).map_err(|err| err.to_string()).map(|r| Some(r)))
                                                
                                            };
            
                                            *partial_file = result;
                                            
                                        }
                                        
            
                                    }
                                }
            
                            },
                            SettingsEntry::SingleItem { name, setting_type, partial_file } => {
                                ui.label("Value");
                            },
                            SettingsEntry::RecursiveItem { name, setting_type, sub_item, partial_file } => todo!(),
                        }
                
                    }else{
                        egui::ComboBox::from_id_salt(index) 
                        .show_ui(ui, |ui|{
                            ui.selectable_value(opt_entry,  None, "");
                            ui.selectable_value(opt_entry, Some(SettingsEntry::File { file_path: None, partial_file: Ok(None) }), "File");
                            ui.selectable_value(opt_entry, Some(SettingsEntry::SingleItem { name: None, setting_type: None, partial_file: None }), "Value");
                            
                        });
                    }
                });

           
        });


        //ui.end_row();
    }

    pub fn get_settings(&self) -> Result<Settings,SlicerErrors>
    {
        let k = self.setting_entities
            .iter()
            .filter_map(|entity|{
                entity.as_ref().map(|e|{
                    match e{
                        SettingsEntry::File { file_path, partial_file } => {
                            partial_file.clone().ok()
                        },
                        SettingsEntry::SingleItem { name, setting_type, partial_file } => {
                            None
                        },
                        SettingsEntry::RecursiveItem { name, setting_type, sub_item, partial_file } => todo!(),
                    }
                }).flatten().flatten()
            })
            .fold(Ok(PartialSettings::default()), |partial,item|{
                let mut new_settings =item.get_partial_settings(std::env::current_dir().expect("Current Direct Exists"))?;
                new_settings.combine(partial?);
                Ok(new_settings)
            })?;

        Settings::try_from(k).map_err(|err| {
            SlicerErrors::SettingsFileMissingSettings {
                missing_setting: err.0,
            }
        })
    }
}


pub enum SettingsEntry{
    File{
        file_path: Option<PathBuf>,
        partial_file: Result<Option<PartialSettingsFile>, String>,
    },
    SingleItem{
        name: Option<String>,
        setting_type: Option<String>,
        partial_file: Option<PartialSettingsFile>,
    },
    RecursiveItem{
        name: String,
        setting_type: Option<String>,
        sub_item: Option<Box<SettingsEntry>>,
        partial_file: Option<PartialSettingsFile>,
    }
}

impl PartialEq for SettingsEntry{
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::File { .. }, Self::File { ..}) => true,
            (Self::SingleItem { .. }, Self::SingleItem { ..}) => true,
            (Self::RecursiveItem { .. }, Self::RecursiveItem { .. }) => true,
            _ => false,
        }
    }
}

