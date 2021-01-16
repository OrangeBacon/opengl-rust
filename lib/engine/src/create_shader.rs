use std::ffi::CString;

use gltf::Type;

use crate::{gltf, model::Error, Model, Program, Shader};

pub struct DynamicShader;

impl DynamicShader {
    pub fn new(gl: &gl::Gl, prim: &gltf::Primitive, model: &Model) -> Result<Program, Error> {
        let components: Vec<Attribute> = prim
            .attributes
            .iter()
            .enumerate()
            .map(|(loc, (name, &accessor))| {
                Some(Attribute {
                    location: loc,
                    accessor_idx: accessor as usize,
                    kind: AttributeType::from(&name)?,
                })
            })
            .flatten()
            .collect();

        let vert = DynamicShader::create_vertex(&components, model);
        let frag = DynamicShader::create_fragment(&components, model);

        let vert = CString::new(vert).map_err(|_| Error::NullShader)?;
        let frag = CString::new(frag).map_err(|_| Error::NullShader)?;

        let vert = Shader::from_vert(gl, &vert).map_err(|e| Error::ShaderCompile { error: e })?;
        let frag = Shader::from_frag(gl, &frag).map_err(|e| Error::ShaderCompile { error: e })?;

        let program =
            Program::from_shaders(gl, &[vert, frag]).map_err(|e| Error::ShaderLink { error: e })?;

        Ok(program)
    }

    pub fn set_attribs(gl: &gl::Gl, prim: &gltf::Primitive, model: &Model) -> Result<usize, Error> {
        if !prim.attributes.contains_key("POSITION") {
            return Err(Error::NoPositions);
        }

        let counts = prim
            .attributes
            .iter()
            .enumerate()
            .map(|(idx, (_, &attr))| Self::attrib(gl, model, attr, idx as u32))
            .collect::<Result<Vec<_>, _>>()?;

        let zero = counts[0];

        let counts_equal = counts.iter().any(|&v| v != zero);

        if counts_equal {
            Err(Error::AttribLen)
        } else {
            Ok(zero)
        }
    }

    fn attrib(gl: &gl::Gl, model: &Model, attr: i32, index: u32) -> Result<usize, Error> {
        let accessor = model
            .model
            .accessors
            .get(attr as usize)
            .ok_or_else(|| Error::BadIndex {
                array: "accessors",
                max: model.model.accessors.len(),
                got: attr as usize,
            })?;
        model.load_accessor(gl, accessor, index)?;

        Ok(accessor.count)
    }

    fn create_vertex(components: &[Attribute], model: &Model) -> String {
        let mut shader = String::new();
        shader.push_str("#version 330 core\n");

        for comp in components {
            if let Some(layout) = comp.layout(model) {
                shader.push_str(&format!(
                    "layout (location = {}) in {} {};\n",
                    comp.location,
                    layout,
                    comp.variable()
                ));
            }
        }

        let mut interface_comp = 0;
        for comp in components {
            if let Some(interface) = comp.interface(model) {
                if interface_comp == 0 {
                    shader.push_str("out VS_OUT {\n");
                }
                shader.push_str(&format!("    {} {};\n", interface, comp.variable(),));

                interface_comp += 1;
            }
        }

        if interface_comp > 0 {
            shader.push_str("} OUT;\n");
        }

        for comp in components {
            if let Some(uniforms) = comp.uniform() {
                for un in uniforms {
                    shader.push_str(&format!("uniform {};\n", un,));
                }
            }
        }

        shader.push_str("void main() {\n");

        for comp in components {
            if let Some(code) = comp.vert() {
                shader.push_str(&code);
            }
        }

        shader.push_str("}\n");

        shader
    }

    fn create_fragment(components: &[Attribute], model: &Model) -> String {
        let mut shader = String::new();
        shader.push_str("#version 330 core\n");

        let mut interface_comp = 0;
        for comp in components {
            if let Some(interface) = comp.interface(model) {
                if interface_comp == 0 {
                    shader.push_str("in VS_OUT {\n");
                }
                shader.push_str(&format!("    {} {};\n", interface, comp.variable(),));

                interface_comp += 1;
            }
        }

        if interface_comp > 0 {
            shader.push_str("} IN;\n");
        }

        shader.push_str("out vec4 Color;\n");
        shader.push_str("void main() {\n");
        shader.push_str("  Color = ");

        let mut output_count = 0;
        for comp in components {
            if let Some(out) = comp.out(model) {
                if output_count != 0 {
                    shader.push_str(" * ");
                }
                shader.push_str(&out);
                output_count += 1;
            }
        }

        if output_count == 0 {
            shader.push_str("vec4(vec3(0.5), 1.0)");
        }

        shader.push_str(";\n");

        shader.push_str("}\n");

        shader
    }
}

struct Attribute {
    kind: AttributeType,
    location: usize,
    accessor_idx: usize,
}

impl Attribute {
    fn layout(&self, model: &Model) -> Option<&'static str> {
        match self.kind {
            AttributeType::Position => Some("vec3"),
            AttributeType::Color(_) => {
                if model.model.accessors[self.accessor_idx].r#type == Type::Vec3 {
                    Some("vec3")
                } else {
                    Some("vec4")
                }
            }
            _ => None,
        }
    }

    fn variable(&self) -> String {
        match self.kind {
            AttributeType::Position => "Position".to_string(),
            AttributeType::Normal => "Normal".to_string(),
            AttributeType::Tangent => "Tangent".to_string(),
            AttributeType::TexCoord(a) => format!("TexCoord{}", a),
            AttributeType::Color(a) => format!("Color{}", a),
            AttributeType::Joints(a) => format!("Joints{}", a),
            AttributeType::Weights(a) => format!("Weights{}", a),
        }
    }

    fn interface(&self, model: &Model) -> Option<&'static str> {
        match self.kind {
            AttributeType::Color(_) => {
                if model.model.accessors[self.accessor_idx].r#type == Type::Vec3 {
                    Some("vec3")
                } else {
                    Some("vec4")
                }
            }
            _ => None,
        }
    }

    fn uniform(&self) -> Option<Vec<&'static str>> {
        match self.kind {
            AttributeType::Position => Some(vec!["mat4 model", "mat4 view", "mat4 projection"]),
            _ => None,
        }
    }

    fn vert(&self) -> Option<String> {
        match self.kind {
            AttributeType::Position => Some(
                "    gl_Position = projection * view * model * vec4(Position, 1.0);\n".to_string(),
            ),
            AttributeType::Color(_) => Some(format!("    OUT.{0} = {0};\n", self.variable())),
            _ => None,
        }
    }

    fn out(&self, model: &Model) -> Option<String> {
        match self.kind {
            AttributeType::Color(_) => {
                if model.model.accessors[self.accessor_idx].r#type == Type::Vec3 {
                    Some(format!("vec4(IN.{}, 1.0)", self.variable()))
                } else {
                    Some(format!("IN.{}", self.variable()))
                }
            }
            _ => None,
        }
    }
}

enum AttributeType {
    Position,
    Normal,
    Tangent,
    TexCoord(usize),
    Color(usize),
    Joints(usize),
    Weights(usize),
}

impl AttributeType {
    fn from(value: &str) -> Option<Self> {
        let comps: Vec<_> = value.split('_').collect();

        match comps.as_slice() {
            ["POSITION"] => Some(AttributeType::Position),
            ["NORMAL"] => Some(AttributeType::Normal),
            ["TANGENT"] => Some(AttributeType::Tangent),
            ["TEXCOORD", a] => {
                if let Ok(idx) = a.parse() {
                    Some(AttributeType::TexCoord(idx))
                } else {
                    None
                }
            }
            ["COLOR", a] => {
                if let Ok(idx) = a.parse() {
                    Some(AttributeType::Color(idx))
                } else {
                    None
                }
            }
            ["JOINTS", a] => {
                if let Ok(idx) = a.parse() {
                    Some(AttributeType::Joints(idx))
                } else {
                    None
                }
            }
            ["WEIGHTS", a] => {
                if let Ok(idx) = a.parse() {
                    Some(AttributeType::Weights(idx))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}
