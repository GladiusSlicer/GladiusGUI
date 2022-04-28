use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::{Arc, RwLock};
use gladius_shared::messages::Message;
use gladius_shared::types::{CalculatedValues};
use glam::{Mat4, Vec2, Vec3};
use itertools::Itertools;
use crate::Errors;
use crate::object::{load, DisplayVertex, Object};

use native_dialog::FileDialog;

pub struct GUIData{
    objects: Vec<Object>,
    print_area_size: Vec3,
    closest_object_point: Option<(usize,Vec3,Vec3 )>,
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

                let this_selected = self.closest_object_point.map(|(i,_,_)| i== index).unwrap_or(false);

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



    pub fn select_button_pressed(&mut self){

        if self.closest_object_point.is_some() && !self.dragging{
            self.dragging = true;
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
    pub fn load_model(&mut self, display: &glium::Display){
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
    }

    pub fn get_commands(&self) ->  Option<Vec<gladius_shared::types::Command>>{
        self.commands.read().unwrap().clone()
    }

}

struct GcodeViewerStates{
    layer_count: usize
}

struct Camera{
    proj_mat: Mat4,
    view_mat: Mat4,
    location:  Vec3,
    center_loc:  Vec3,
    screen_dimensions: Vec2,
    pub zoom: f32,
    pub pitch: f32,
    pub yaw: f32

}

impl  Camera{
    fn new(screen_dimensions: Vec2, center_loc: Vec3) -> Self {
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
