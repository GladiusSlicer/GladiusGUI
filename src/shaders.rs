pub const VERTEX_SHADER_SRC: &str = r#"
    #version 150
    in vec3 position;
    out vec3 viewPosition;
    out vec3 v_position;
    out vec3 v_color;
    uniform vec3 color;
    uniform mat4 perspective;
    uniform mat4 view;
    uniform mat4 model;
    void main() {
        mat4 modelview = view * model;
        viewPosition = (modelview * vec4(position, 0.0)).xyz;
        gl_Position = perspective * modelview * vec4(position, 1.0);
        v_color = color;

        v_position = gl_Position.xyz / gl_Position.w;
    }
"#;

pub const FRAGMENT_SHADER_SRC: &str = r#"
        #version 150
        in vec3 viewPosition;
        in vec3 v_position;
        in vec3 v_color;
        out vec4 color;
        void main() {
            vec3 xTangent = vec3(dFdx(viewPosition.x),dFdx(viewPosition.y),dFdx(viewPosition.z));
            vec3 yTangent = vec3(dFdy(viewPosition.x),dFdy(viewPosition.y),dFdy(viewPosition.z));
            vec3 faceNormal = normalize( cross( xTangent, yTangent ));


            const vec3 ambient_color = vec3(0.0, 0.0, 0.0);
            vec3 diffuse_color = v_color;
            const vec3 specular_color = vec3(1.0, 1.0, 1.0);

            float diffuse = max(dot(normalize(faceNormal), normalize(vec3(0., 0.0, 0.1))), 0.0);
            vec3 camera_dir = -normalize(v_position);
            vec3 half_direction = normalize(camera_dir);
            float specular = pow(max(dot(half_direction, -normalize(faceNormal)), 0.0), 128.0);
            color = vec4(ambient_color + diffuse * diffuse_color + specular * specular_color, 1.0);


        }
    "#;
pub const LINE_VERTEX_SHADER_SRC: &str = r#"
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

pub const LINE_FRAGMENT_SHADER_SRC: &str = r#"
        #version 140
        out vec4 color;
        void main() {

            color = vec4( vec3(0.0, 0.0, 1.0), 1.0);

        }
    "#;
