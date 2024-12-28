use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::{Arc, RwLock};
use gladius_shared::messages::Message;
use gladius_shared::types::{CalculatedValues};
use glam::{Mat4, Vec2, Vec3};
use glium::glutin::surface::WindowSurface;
use itertools::Itertools;
use crate::Errors;
use crate::object::{load, DisplayVertex, Object};

use native_dialog::FileDialog;

pub struct GUIData{
    objects: Vec<Object>,
    print_area_size: Vec3,
    closest_object_point: Option<(usize,Vec3,Vec3 )>,
    selected_object: Option<usize >,
    dragging: bool,
    camera: Camera,
    settings_path: String,
    calc_vals: Arc<RwLock<Option<CalculatedValues>>>,
    gcode: Arc<RwLock<Option<String>>>,
    commands: Arc<RwLock<Option<Vec<gladius_shared::types::Command>>>>,
    error: Arc<RwLock<Option<Errors>>>,
    command_running: Arc<RwLock<bool>>,
    command_state: Arc<RwLock<String>>,
    refresh: Arc<RwLock<bool>>

}

impl GUIData{
    pub fn new(screen_dimensions: Vec2,print_area_size: Vec3) -> Self{
        let mut center_pos = (print_area_size.x/2.0,print_area_size.y/2.0);



        GUIData{
            objects: vec![],
            print_area_size: print_area_size,
            closest_object_point: None,
            selected_object: None,
            dragging: false,
            camera: Camera::new(screen_dimensions, Vec3::new(center_pos.0 ,center_pos.1 , 0.0)),
            settings_path: String::new(),
            calc_vals: Arc::new(RwLock::new(None)),
            gcode:Arc::new(RwLock::new(None)),
            commands: Arc::new(RwLock::new(None)),
            error: Arc::new(RwLock::new(None)),
            command_running: Arc::new(RwLock::new(false)),
            command_state: Arc::new(RwLock::new(String::new())),
            refresh: Arc::new(RwLock::new(false)),
        }
    }

    pub fn extend_objects<I>(&mut self, objs: I )
        where I : IntoIterator<Item = Object>
    {
        self.objects.extend(objs.into_iter())
    }

    pub fn get_objects(&self) -> &Vec<Object>{
        &self.objects
    }

    pub fn get_command_line_args(&self) -> Vec<String> {
        self.objects.iter()
            .map(|obj|{
               format!("{{\"Raw\":[\"{}\",{:?}]}} ", obj.file_path.replace('\\', "\\\\"),obj.get_model_matrix().transpose().to_cols_array_2d())
            })
            .collect()
    }

    pub fn update_colors(&mut self){
        self.objects
            .iter_mut()
            .enumerate()
            .for_each(|(index,obj)| {
                let in_build_area = obj.aabb.as_ref().map(|aabb| {
                    !(aabb.min_x < 0.0 || aabb.min_y < 0.0 || aabb.min_z < 0.0 || aabb.max_x >  self.print_area_size.x || aabb.max_y >  self.print_area_size.y || aabb.max_z > self.print_area_size.z)
                }).unwrap_or(false);

                let this_selected = self.selected_object.map(|i| i== index).unwrap_or(false);

                 obj.color = match (obj.hovered,in_build_area,this_selected){
                     (false,false,false) => Vec3::new(1.0, 0.0, 0.0),
                     (false,true,false) => Vec3::new(1.0, 1.0, 0.0),
                     (true,true,false) => Vec3::new(0.0, 0.0, 1.0),
                     (true,false,false) => Vec3::new(1.0, 0.0, 0.0),
                     (_,false,true) => Vec3::new(1.0, 0.0, 1.0),
                     (_,true,true) => Vec3::new(0.0, 1.0, 1.0),
                 };


            });
    }

    pub fn set_settings_path(&mut self, path: String){
        self.settings_path = path;
    }

    pub fn get_settings_path(&mut self ) -> &String{
        &self.settings_path
    }


    pub fn can_slice(&self) -> bool{
        !self.objects.is_empty() && !self.settings_path.is_empty() && ! *self.command_running.read().unwrap()
    }

    pub fn start_slice(&mut self) {
        *self.calc_vals.write().unwrap() = None;
        *self.gcode.write().unwrap() = None;
        *self.error.write().unwrap() = None;
        *self.commands.write().unwrap() = None;
        *self.command_running.write().unwrap() = true;


        let args = self.get_command_line_args() ;

        let calc_vals_clone = self.calc_vals.clone();
        let commands_clone = self.commands.clone();
        let gcode_clone = self.gcode.clone();
        let error_clone = self.error.clone();
        let command_running_clone = self.command_running.clone();
        let command_state_clone = self.command_state.clone();
        let settings_path_clone = self.settings_path.clone();
        let refresh_clone = self.refresh.clone();


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
               print!("{}",arg.replace('\\', "\\\\").replace('\"', "\\\""));
           }

           println!();

           let cpus = format!("{}", (num_cpus::get()).max(1));

           println!("{}",cpus);

           if let Ok(mut child) = command
               .arg("-m")
               .arg("-s")
               .arg( settings_path_clone.replace('\\', "\\\\"))
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
                           Message::Warning(_warn) =>{
                           }
                       }

                       *refresh_clone.write().unwrap() = true;
                   }
               }

               if let Some(ref mut stderr) = child.stderr {
                   let buff = BufReader::new(stderr);
                   if let Some(line ) = buff.lines().next() {
                        println!("{:?}",line);
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

    pub fn mouse_move_delta(&mut self, delta: Vec2) {
        if !self.dragging {
            self.camera.yaw += delta.x as f32 * -0.01;
            self.camera.pitch = (self.camera.pitch + delta.y as f32 * 0.01).min(std::f32::consts::FRAC_PI_2 - 0.001).max(-std::f32::consts::FRAC_PI_2 + 0.001);
        }
    }

    pub fn mouse_wheel(&mut self, wheel_move: f32) {
        self.camera.zoom = (self.camera.zoom * (1.0 - (0.1 * wheel_move.signum()))).min(1000.0).max(5.0);
    }

    pub fn get_camera_view_and_proj_matrix(&mut self) -> ([[f32;4];4],[[f32;4];4]){
        self.camera.rebuild_matrices();
        (self.camera.get_camera_view_matrix(),self.camera.get_camera_proj_matrix())
    }

    pub fn select_object(&mut self, index: usize){

        if self.selected_object == Some(index){
            self.selected_object = None
        }
        else{
            self.selected_object = Some(index);
        }
    }


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

    pub fn update_screen_dimensions(&mut self,screen_dimensions: Vec2){
        self.camera.update_screen_dimensions(screen_dimensions)
    }

    pub fn is_command_running(&self) -> bool {
        *self.command_running.read().unwrap()
    }

    pub fn check_refresh_and_clear(&self) -> bool{
        let old = *self.refresh.write().unwrap();
        *self.refresh.write().unwrap() = false;
        old
    }

    pub fn load_settings_file(&mut self){
        let path = FileDialog::new()
            .add_filter("Supported settings Types", &["json"])
            .show_open_single_file()
            .unwrap();

        let path = match path {
            Some(path) => path,
            None => return,
        };

        self.set_settings_path(path.into_os_string().into_string().unwrap());


    }
    pub fn load_model(&mut self, display: &glium::Display<WindowSurface>){
        let paths = FileDialog::new()
           .add_filter("Supported Model Types", &["stl", "3mf"])
           .show_open_multiple_file()
           .unwrap();

        for path in paths {

           let model_path = path.into_os_string().into_string().unwrap();

           match load(&model_path,display)
           {
               Ok(objs) => { self.extend_objects(objs) }
               Err(e) => { *self.error.write().unwrap() = Some(Errors::SlicerError(e)) }
           }
        }
    }
/* 
    pub fn get_calculated_values(&self) -> Option<CalculatedValues>{
        self.calc_vals.read().unwrap().clone()
    }

    pub fn get_command_state(&self) -> String{
        self.command_state.read().unwrap().clone()
    }

    pub fn get_errors(&self) -> Vec<Errors>{
        self.error.read().unwrap().clone().into_iter().collect_vec()
    }

    pub fn get_gcode(&self) -> Option<String>{
        self.gcode.read().unwrap().clone()
    }*/

    pub fn get_commands(&self) ->  Option<Vec<gladius_shared::types::Command>>{
        self.commands.read().unwrap().clone()
    }

    pub fn copy_selected(&mut self,display: &glium::Display<WindowSurface>) {
        if let Some(i) = self.selected_object{
            let clone = self.objects[i].make_copy(display);

            self.objects.insert(i,clone);

        }
    }
    pub fn center_selected(&mut self) {
        if let Some(i) = self.selected_object{
            self.objects[i].get_mut_location().x = self.print_area_size.x/2.0;
            self.objects[i].get_mut_location().y = self.print_area_size.y/2.0;

            self.objects[i].revalidate_cache();
        }
    }
    pub fn delete_selected(&mut self) {
        if let Some(i) = self.selected_object{
            self.objects.remove(i);
            self.selected_object = None;
        }
    }

    pub fn object_selected(&mut self) -> bool{
        self.selected_object.is_some()
    }

    pub fn selected_object_z_rotation(&mut self) -> &mut f32 {
        if let Some(i) = self.selected_object{
            self.objects[i].get_mut_z_rotation()
        }
        else{
            panic!("Don't call without selected object");
        }
    }

    pub fn get_print_area(&self) -> (f32,f32,f32){
        (self.print_area_size.x,self.print_area_size.y,self.print_area_size.z)
    }

}

struct GcodeViewerStates{
    layer_count: usize
}

pub struct Camera{
    pub proj_mat: Mat4,
    pub view_mat: Mat4,
    pub location:  Vec3,
    pub center_loc:  Vec3,
    pub screen_dimensions: Vec2,
    pub zoom: f32,
    pub pitch: f32,
    pub yaw: f32

}

impl  Camera{
    pub fn new(screen_dimensions: Vec2, center_loc: Vec3) -> Self {
        let zoom = 400.0;
        let pitch = std::f32::consts::FRAC_PI_4;
        let yaw =  -std::f32::consts::FRAC_PI_4 + 0.12;

        let aspect_ratio = screen_dimensions.x as f32 / screen_dimensions.y as f32;

        let camera_vec = glam::Vec3::new(zoom *yaw.cos() * pitch.cos() ,zoom * yaw.sin() * pitch.cos() , zoom * pitch.sin());

        let proj_mat = glam::Mat4::perspective_infinite_rh(60.0_f32.to_radians(),aspect_ratio,0.1);
        let location = camera_vec +center_loc;
        let view_mat =glam::Mat4::look_at_rh(location,center_loc,glam::Vec3::new(0.0,0.0,1.0));


        Camera{
            zoom,
            pitch,
            yaw,
            center_loc,
            location,
            view_mat,
            proj_mat,
            screen_dimensions
        }
    }

    pub fn rebuild_matrices(&mut self) {

        let aspect_ratio = self.screen_dimensions.x as f32 / self.screen_dimensions.y as f32;

        let camera_vec = glam::Vec3::new(self.zoom *self.yaw.cos() * self.pitch.cos() ,self.zoom * self.yaw.sin() * self.pitch.cos() , self.zoom * self.pitch.sin());

        self.proj_mat = glam::Mat4::perspective_infinite_rh(60.0_f32.to_radians(),aspect_ratio,0.1);
        self.location = camera_vec +self.center_loc;
        self.view_mat =glam::Mat4::look_at_rh(self.location,self.center_loc,glam::Vec3::new(0.0,0.0,1.0));

    }

    pub fn update_screen_dimensions(&mut self,screen_dimensions: Vec2){
        self.screen_dimensions = screen_dimensions;
    }

    pub fn get_camera_view_matrix(&self)  -> [[f32;4];4] {
        self.view_mat.to_cols_array_2d()
    }

    pub fn get_camera_proj_matrix(&self) -> [[f32;4];4]{
        self.proj_mat.to_cols_array_2d()
    }

}

    // /* event_loop.run(move |event, _, control_flow| {
    //     let mut redraw = || {
    //         let mut quit = false;


    //         let needs_repaint = egui_glium.run(&window, |egui_ctx| {

    //             gui_data.update_colors();

    //             plot_window_resp = None;
    //             window_clicked = false;
    //            let resp = egui::SidePanel::left("my_side_panel").show(&egui_ctx, |ui| {
    //                ui.heading(&get_translated_string(&ctx, lang, "setup_bar_heading"));
    //                ui.horizontal(|ui| {
    //                    ui.label(&get_translated_string(&ctx, lang, "model_path"));
    //                    if ui.button(&get_translated_string(&ctx, lang,  "choose_model_button")).clicked() {
    //                         gui_data.load_model(    &display);
    //                    }
    //                });
    //                ui.horizontal(|ui| {
    //                    ui.label(&get_translated_string(&ctx, lang, "settings_path"));
    //                    let mut short = gui_data.get_settings_path().clone();
    //                    if short.len() > 13{
    //                        short.truncate(10);
    //                        short += "...";
    //                    }
    //                    ui.label(short);
    //                });
    //                ui.horizontal(|ui| {
    //                    if ui.button("Choose settings").clicked() {
    //                         gui_data.load_settings_file();

    //                        (build_x, build_y, build_z) = gui_data.get_print_area();

    //                        (line_positions, line_indices) = create_build_area(&display, build_x, build_y, build_z);
    //                    }
    //                });
    //                ui.group(|ui| {

    //                     let mut selected = None;

    //                    gui_data.get_objects().iter().enumerate()
    //                        .for_each(|(i,mut obj)| {
    //                        ui.horizontal(|ui| {
    //                             if ui.label(obj.name.to_string()).clicked(){
    //                                 selected = Some(i);
    //                                 println!("here")
    //                             }
    //                        });

    //                        let mut changed = false;

    //                        /*
    //                        ui.horizontal(|ui| {
    //                            changed |= ui.add(egui::DragValue::new(&mut obj.get_mut_location().x)
    //                                .speed(1.0)
    //                                .clamp_range(f64::NEG_INFINITY..=f64::INFINITY)
    //                                .prefix("x: "))
    //                                .changed();
    //                            changed |= ui.add(egui::DragValue::new(&mut obj.get_mut_location().y)
    //                                .speed(1.0)
    //                                .clamp_range(f64::NEG_INFINITY..=f64::INFINITY)
    //                                .prefix("y: "))
    //                                .changed();
    //                            changed |= ui.add(egui::DragValue::new(&mut obj.get_mut_scale().x)
    //                                .speed(0.01)
    //                                .clamp_range(0.0..=f64::INFINITY)
    //                                .prefix("scale: "))
    //                                .changed();

    //                            obj.get_mut_scale().y = obj.get_scale().x;
    //                            obj.get_mut_scale().z = obj.get_scale().x;
    //                        });

    //                        ui.horizontal(|ui| {
    //                            if ui.button(&get_translated_string(&ctx, lang,"remove")).clicked() {
    //                                remove = Some(i);
    //                            };
    //                            if ui.button(&get_translated_string(&ctx, lang, "copy")).clicked(){
    //                                copy = Some(i)
    //                            }
    //                            if ui.button(&get_translated_string(&ctx, lang, "center")).clicked(){
    //                                obj.get_mut_location().x = build_x /2.0;
    //                                obj.get_mut_location().y = build_y /2.0;
    //                                changed = true;
    //                            }

    //                        });

    //                        if changed{
    //                            //obj.revalidate_cache();
    //                            //*gcode.write().unwrap() = None;
    //                            //*calc_vals.write().unwrap() = None;
    //                        }
    //                    });

    //                    if let Some(i) = selected{
                        
    //                        gui_data.select_object(i);
                               
    //                    }
    //                });

    //                ui.horizontal(|ui| {


    //                    ui.style_mut().spacing.button_padding = egui::Vec2::new(50., 20.);
    //                    ui.style_mut().override_text_style = Some(TextStyle::Heading);


    //                    ui.centered_and_justified(|ui| {
    //                        let mut fonts = FontDefinitions::default();

    //                        // Large button text:

    //                        //ui.ctx().set_fonts(fonts);


    //                        if ui.add_enabled( gui_data.can_slice(), egui::Button::new(&get_translated_string(&ctx, lang, "slice"))).clicked() {
    //                            index = 0;
    //                             layers = 0;
    //                             viewer_open = true;

    //                            gui_data.start_slice();
    //                        }
    //                    });
    //                });

    //                if let Some(cv) = gui_data.get_calculated_values(){
    //                    ui.horizontal(|ui| {
    //                        ui.label(get_translated_string_argument(&ctx,lang,"plastic_volume_msg",format!("{:.0}",cv.plastic_volume)));
    //                    });
    //                    ui.horizontal(|ui| {
    //                        ui.label(get_translated_string_argument(&ctx,lang,"plastic_weight_msg",format!("{:.0}",cv.plastic_weight)));

    //                    });
    //                    ui.horizontal(|ui| {
    //                        let (hour,min,_,_) = cv.get_hours_minutes_seconds_fract_time();
    //                        ui.label(get_translated_string_arguments(&ctx,lang,"print_time_msg",&[hour.to_string(), min.to_string()]));
    //                    });
    //                };

    //                for err in gui_data.get_errors(){

    //                    let (code, message) = err.get_code_and_message();
    //                    ui.horizontal(|ui| {
    //                        ui.label(format!("Error {:#X}",code));
    //                    });
    //                    ui.horizontal(|ui| {
    //                        ui.label(message.to_string());
    //                    });
    //                };

    //                if gui_data.is_command_running(){
    //                    ui.horizontal(|ui| {
    //                        ui.heading("Running");
    //                    });
    //                    ui.horizontal(|ui| {
    //                        ui.label(format!("Status {}",gui_data.get_command_state()));
    //                    });
    //                }

    //                if let Some(str) = gui_data.get_gcode() {
    //                    ui.horizontal(|ui| {
    //                        ui.style_mut().spacing.button_padding = egui::Vec2::new(50., 20.);
    //                        //ui.style_mut().body_text_style = TextStyle::Heading;
    //                        ui.style_mut().override_text_style = Some(TextStyle::Heading);


    //                        ui.centered_and_justified(|ui| {
    //                            if ui.button("Save").clicked() {
    //                                let path = FileDialog::new()
    //                                    .add_filter("gcode", &["gcode"])
    //                                    .show_save_single_file()
    //                                    .unwrap();

    //                                let path = match path {
    //                                    Some(path) => path,
    //                                    None => return,
    //                                };

    //                                let mut file = File::create(path).unwrap();
    //                                file.write_all(str.as_bytes()).unwrap();
    //                            }
    //                        });
    //                    });
    //                }
    //                if let Some(cmds) = gui_data.get_commands() {
    //                     plot_window_resp = egui::Window::new(&get_translated_string(&ctx, lang, "viewer"))
    //                         .open(&mut viewer_open)
    //                         .default_size(egui::Vec2::new(400.0, 400.0))
    //                         .show(&egui_ctx, |ui| {


    //                             let line = Line::new(Values::from_values(vec![Value{x:0.0,y: 0.0},Value{x:0.0,y: build_y as f64 },Value{x:build_x as f64 ,y: build_y as f64 },Value{x:build_x as f64,y: 0.0  },Value{x:0.0,y: 0.0},Value{x:0.0,y: build_y as f64 }])).width(5.0);


    //                             let mut layer_height = 0.0;

    //                             ui.style_mut().spacing.slider_width = ui.available_width() - 100.0;
    //                             let drag_resp = ui.add(egui::Slider::new(&mut index, 1_usize..=(layers.saturating_sub(1_usize)).max(1))
    //                                 .prefix("x: "));


    //                             let plot = Plot::new("items_demo")
    //                                 .legend(Legend::default().position(Corner::RightBottom))
    //                                 .show_x(false)
    //                                 .show_y(false)
    //                                 .data_aspect(1.0);


    //                             let resp = plot.show(ui, |plot_ui| {
    //                                 plot_ui.line(line.name(&get_translated_string(&ctx, lang, "Border")));

    //                                 let p1 = plot_ui.screen_from_plot(Value{x:0.0,y:0.0});
    //                                 let p2 = plot_ui.screen_from_plot(Value{x:0.0,y:1.0});


    //                                 let pixels_per_plot_unit = (p2-p1).length();


    //                                 let mut moves : Vec<_>= (&cmds
    //                                     .iter()
    //                                     .group_by(|cmd|{
    //                                         if let gladius_shared::types::Command::LayerChange { z, index: _index } = cmd{
    //                                             let r = *z == layer_height;
    //                                             layer_height = *z;
    //                                             r
    //                                         }else{
    //                                             true
    //                                         }
    //                                     }))
    //                                     .into_iter()
    //                                     .filter_map(|(change,layer)|{
    //                                         if !change{
    //                                             None
    //                                         }else{
    //                                             let lines :Vec<_> = layer.into_iter().filter_map(|cmd|{

    //                                                 if let gladius_shared::types::Command::MoveAndExtrude{start,end,width,..} = cmd{
    //                                                     Some(Line::new(Values::from_values(vec![Value{x:start.x,y:start.y},Value{x:end.x,y: end.y }]))
    //                                                              .stroke(Stroke{width: *width as f32 * pixels_per_plot_unit* 0.95,color: Color32::BLUE}))

    //                                                 }else{
    //                                                     None
    //                                                 }
    //                                             }).collect();

    //                                             Some(lines)
    //                                         }
    //                                     })
    //                                     .collect();

    //                                 layers = moves.len();

    //                                 for move_line in moves.remove(index){
    //                                     plot_ui.line(move_line.name("Move"));
    //                                 }
    //                             });

    //                             drag_resp.union(resp.response)
    //                         });
    //                }



    //             });


    //             egui::TopBottomPanel::bottom("Bottom Panel").show(&egui_ctx,|ui|{
    //                 ui.horizontal(|ui| {
    //                     let selected = gui_data.object_selected();

    //                     let mut slider = 0.0;



    //                     if ui.add_enabled( selected, egui::Button::new(&get_translated_string(&ctx, lang, "copy"))).clicked() {
    //                         gui_data.copy_selected(&display);
    //                     }
    //                     if ui.add_enabled( selected, egui::Button::new(&get_translated_string(&ctx, lang, "remove"))).clicked() {
    //                         gui_data.delete_selected();
    //                     }
    //                     if ui.add_enabled( selected, egui::Button::new(&get_translated_string(&ctx, lang, "center"))).clicked() {
    //                         gui_data.center_selected();
    //                     }

    //                     let slider_ptr = if selected{
    //                         gui_data.selected_object_z_rotation()
    //                     }else{
    //                         &mut slider
    //                     };

    //                     ui.add_enabled( selected, egui::Slider::new(slider_ptr,0.0..=360.0).text(&get_translated_string(&ctx, lang, "z_rot")));
    //                 });
    //             });
    //             window_rec = plot_window_resp.as_ref().map(|r| r.response.rect);

    //             let full_resp = match plot_window_resp.take(){
    //                 Some(mut window_resp) => {

    //                     resp.response.union(window_resp.response.interact(Sense::click_and_drag()).union(window_resp.inner.take().unwrap()))
    //                 },
    //                 None => {
    //                     resp.response
    //                 }
    //             };

    //            on_render_screen = !egui_ctx.wants_pointer_input();
    //         });

    //         *control_flow = if quit {
    //             glutin::event_loop::ControlFlow::Exit
    //         } else if needs_repaint {

    //             gui_data.check_refresh_and_clear();
    //             display.gl_window().window().request_redraw();

    //             glutin::event_loop::ControlFlow::Poll
    //         } else if gui_data.is_command_running(){
    //             //If command is running keep refreshing
    //             glutin::event_loop::ControlFlow::Poll
    //         }else{
    //             glutin::event_loop::ControlFlow::Wait
    //         };


    //         {
    //             use glium::Surface as _;
    //             let mut target = display.draw();

    //             let color = egui::Rgba::from_rgb(0.0, 0.0, 0.0);
    //             target.clear_color_and_depth((color[0], color[1], color[2], color[3]),1.0);

    //             // draw things behind egui here

    //             let (view ,perspective ) =  gui_data.get_camera_view_and_proj_matrix();

    //             let line_model = glam::Mat4::from_translation(Vec3::new(0.0,0.0,0.0)).to_cols_array_2d();

    //             let params = glium::DrawParameters {
    //                 depth: glium::Depth {
    //                     test: glium::draw_parameters::DepthTest::IfLess,
    //                     write: true,
    //                     .. Default::default()
    //                 },
    //                 //backface_culling: glium::draw_parameters::BackfaceCullingMode::CullClockwise,
    //                 .. Default::default()
    //             };

    //             gui_data.update_colors();
    //             gui_data.update_screen_dimensions(Vec2::new(target.get_dimensions().0 as f32,target.get_dimensions().1 as f32 ));

    //             for obj in gui_data.get_objects(){
    //                 let model = obj.get_model_matrix().to_cols_array_2d();
    //                 let color = obj.color.to_array();
    //                 let (positions, indices) = (&obj.vert_buff,&obj.index_buff);//create_mesh(&display);
    //                 target.draw(positions, indices, &model_program, &uniform! {color: color,  model: model, view: view, perspective: perspective }, &params).unwrap();
    //             }


    //             target.draw(&line_positions, &line_indices, &line_program, &uniform! { model: line_model, view: view, perspective: perspective }, &params).unwrap();

    //             egui_glium.paint(&display, &mut target);

    //             // draw things on top of egui here

    //             target.finish().unwrap();
    //         }
    //     };




    //     if r {
    //         redraw();
    //     }

    //     match event {
    //         // Platform-dependent event handlers to workaround a winit bug
    //         // See: https://github.com/rust-windowing/winit/issues/987
    //         // See: https://github.com/rust-windowing/winit/issues/1619
    //         glutin::event::Event::RedrawEventsCleared if cfg!(windows) => redraw(),
    //         glutin::event::Event::RedrawRequested(_) if !cfg!(windows) => redraw(),

    //         glutin::event::Event::WindowEvent { event, .. } => {

    //             match event{
    //                 WindowEvent::CloseRequested | WindowEvent::Destroyed => {
    //                     *control_flow = glutin::event_loop::ControlFlow::Exit;
    //                 }
    //                 WindowEvent::CursorMoved {position, ..} => {
    //                     on_render_screen = on_render_screen &&  if let Some(rect ) = window_rec {
    //                         !rect.contains(Pos2{x: position.x as f32,y: position.y as f32 })
    //                     }
    //                     else{
    //                         true
    //                     };
    //                     if on_render_screen {
    //                         gui_data.mouse_move(Vec2::new(position.x as f32, position.y as f32));
    //                     }
    //                     in_window = true;
    //                 }
    //                 WindowEvent::CursorLeft {..} =>{
    //                     in_window = false;
    //                 }
    //                 _ => {}
    //             }

    //             egui_glium.on_event(&event);

    //             display.gl_window().window().request_redraw(); // TODO: ask egui if the events warrants a repaint instead
    //         }
    //         glutin::event::Event::DeviceEvent {event, ..} =>
    //         {
    //             match event {
    //                 DeviceEvent::Button {button, state} => {
    //                     if button == 1{
    //                         if left_mouse_button_state == ElementState::Released && state == ElementState::Pressed && on_render_screen {
    //                             gui_data.select_button_pressed()
    //                         }
    //                         if state == ElementState::Released {
    //                             gui_data.select_button_released()
    //                         }
    //                         left_mouse_button_state = state;
    //                     }
    //                 }
    //                 DeviceEvent::MouseMotion {delta: (dx,dy)} =>{
    //                     if left_mouse_button_state == ElementState::Pressed && on_render_screen && in_window{
    //                         gui_data.mouse_move_delta(Vec2::new(dx as f32,dy as f32))
    //                     }
    //                 }
    //                 DeviceEvent::MouseWheel {delta} =>{
    //                     if  on_render_screen && in_window {
    //                         if let MouseScrollDelta::LineDelta(_x, y) = delta {
    //                             gui_data.mouse_wheel(y)
    //                         }
    //                     }
    //                 }
    //                 _ => {
    //                     //println!("{:?}",event);
    //                 }
    //             }
    //         }
    //         _ => {
    //             println!("{:?}",event)
    //         },
    //     }
    // });
    // */