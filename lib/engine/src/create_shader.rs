use gltf::AccessorType;

use crate::{gltf, model::Model, model::ModelError, renderer::Pipeline};

pub struct DynamicShader;

impl DynamicShader {
    pub fn new(
        pipeline: &mut Pipeline,
        prim: &gltf::Primitive,
        model: &Model,
    ) -> Result<usize, ModelError> {
        let components: Vec<Attribute> = prim
            .attributes
            .iter()
            .map(|(name, &accessor)| {
                Some(Attribute {
                    accessor_idx: accessor as usize,
                    kind: AttributeType::from(&name)?,
                })
            })
            .flatten()
            .collect();

        let vert = DynamicShader::create_vertex(&components, prim, model);
        let frag = DynamicShader::create_fragment(&components, prim, model);

        pipeline.from_vertex_shader(vert).from_frag_shader(frag);
        Self::set_attribs(pipeline, prim, model, &components)
    }

    fn set_attribs(
        pipeline: &mut Pipeline,
        prim: &gltf::Primitive,
        model: &Model,
        components: &[Attribute],
    ) -> Result<usize, ModelError> {
        if !prim.attributes.contains_key("POSITION") {
            return Err(ModelError::NoPositions);
        }

        let counts = prim
            .attributes
            .iter()
            .enumerate()
            .map(|(idx, (_, &attr))| {
                Self::attrib(pipeline, model, attr, &components[idx].variable())
            })
            .collect::<Result<Vec<_>, _>>()?;

        let zero = counts[0];

        let counts_equal = counts.iter().any(|&v| v != zero);

        if counts_equal {
            Err(ModelError::AttribLen)
        } else {
            Ok(zero)
        }
    }

    fn attrib(
        pipeline: &mut Pipeline,
        model: &Model,
        attr: i32,
        name: &str,
    ) -> Result<usize, ModelError> {
        let accessor =
            model
                .gltf
                .accessors
                .get(attr as usize)
                .ok_or_else(|| ModelError::BadIndex {
                    array: "accessors",
                    max: model.gltf.accessors.len(),
                    got: attr as usize,
                })?;

        let buf = accessor.buffer_view.ok_or_else(|| ModelError::NoSource)?;

        let buffers = &model.gpu_buffers;

        let buf = buffers.get(buf).ok_or_else(|| ModelError::BadIndex {
            array: "views",
            max: buffers.len(),
            got: buf,
        })?;

        Model::load_accessor(pipeline, buf, accessor, name)?;

        Ok(accessor.count)
    }

    fn create_vertex(components: &[Attribute], prim: &gltf::Primitive, model: &Model) -> String {
        let mut shader = String::new();
        shader.push_str("#version 330 core\n");

        for comp in components {
            if let Some(layout) = comp.layout(prim, model) {
                shader.push_str(&format!("in {} {};\n", layout, comp.variable()));
            }
        }

        let mut interface_comp = 0;
        for comp in components {
            if let Some(interface) = comp.interface(prim, model) {
                if interface_comp == 0 {
                    shader.push_str("out VS_OUT {\n");
                }
                shader.push_str(&format!("    {} {};\n", interface, comp.variable()));

                interface_comp += 1;
            }
        }

        if interface_comp > 0 {
            shader.push_str("} OUT;\n");
        }

        for comp in components {
            if let Some(uniforms) = comp.uniform_vert() {
                for un in uniforms {
                    shader.push_str(&format!("uniform {};\n", un,));
                }
            }
        }

        shader.push_str("void main() {\n");

        for comp in components {
            if let Some(code) = comp.vert(prim, model) {
                shader.push_str(&code);
            }
        }

        shader.push_str("}\n");

        shader
    }

    fn create_fragment(components: &[Attribute], prim: &gltf::Primitive, model: &Model) -> String {
        let mut shader = String::new();
        shader.push_str("#version 330 core\n");

        let mut interface_comp = 0;
        for comp in components {
            if let Some(interface) = comp.interface(prim, model) {
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

        for comp in components {
            if let Some(uniforms) = comp.uniform_frag(prim, model) {
                for un in uniforms {
                    shader.push_str(&format!("uniform {};\n", un,));
                }
            }
        }

        shader.push_str("void main() {\n");
        shader.push_str("  Color = ");

        let mut output_count = 0;
        for comp in components {
            if let Some(out) = comp.out(prim, model) {
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
    accessor_idx: usize,
}

impl Attribute {
    fn layout(&self, prim: &gltf::Primitive, model: &Model) -> Option<&'static str> {
        let ret = match self.kind {
            AttributeType::Position => "vec3",
            AttributeType::Color(_) => {
                if model.gltf.accessors[self.accessor_idx].r#type == AccessorType::Vec3 {
                    "vec3"
                } else {
                    "vec4"
                }
            }
            AttributeType::TexCoord(idx) if is_base_color(prim, model, idx) => "vec2",
            _ => return None,
        };

        Some(ret)
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

    fn interface(&self, prim: &gltf::Primitive, model: &Model) -> Option<&'static str> {
        let ret = match self.kind {
            AttributeType::Color(_) => {
                if model.gltf.accessors[self.accessor_idx].r#type == AccessorType::Vec3 {
                    "vec3"
                } else {
                    "vec4"
                }
            }
            AttributeType::TexCoord(idx) if is_base_color(prim, model, idx) => "vec2",
            _ => return None,
        };

        Some(ret)
    }

    fn uniform_vert(&self) -> Option<Vec<&'static str>> {
        match self.kind {
            AttributeType::Position => Some(vec!["mat4 model", "mat4 view", "mat4 projection"]),
            _ => None,
        }
    }

    fn uniform_frag(&self, prim: &gltf::Primitive, model: &Model) -> Option<Vec<&'static str>> {
        let ret = match self.kind {
            AttributeType::TexCoord(idx) if is_base_color(prim, model, idx) => {
                vec!["sampler2D baseColor"]
            }
            _ => return None,
        };

        Some(ret)
    }

    fn vert(&self, prim: &gltf::Primitive, model: &Model) -> Option<String> {
        let ret = match self.kind {
            AttributeType::Position => {
                "    gl_Position = projection * view * model * vec4(Position, 1.0);\n".to_string()
            }
            AttributeType::Color(_) => format!("    OUT.{0} = {0};\n", self.variable()),
            AttributeType::TexCoord(idx) if is_base_color(prim, model, idx) => {
                format!("    OUT.{0} = {0};\n", self.variable())
            }
            _ => return None,
        };

        Some(ret)
    }

    fn out(&self, prim: &gltf::Primitive, model: &Model) -> Option<String> {
        let ret = match self.kind {
            AttributeType::Color(_) => {
                if model.gltf.accessors[self.accessor_idx].r#type == AccessorType::Vec3 {
                    format!("vec4(IN.{}, 1.0)", self.variable())
                } else {
                    format!("IN.{}", self.variable())
                }
            }
            AttributeType::TexCoord(idx) if is_base_color(prim, model, idx) => {
                format!("texture(baseColor, IN.TexCoord{})", idx)
            }
            _ => return None,
        };

        Some(ret)
    }
}

fn is_base_color(prim: &gltf::Primitive, model: &Model, idx: usize) -> bool {
    prim.material
        .and_then(|mat| model.gltf.materials[mat].pbr_metallic_roughness.as_ref())
        .and_then(|pbr| pbr.base_color_texture.as_ref())
        .map(|color| color.tex_coord == idx)
        .unwrap_or(false)
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

        let ty = match comps.as_slice() {
            ["POSITION"] => AttributeType::Position,
            ["NORMAL"] => AttributeType::Normal,
            ["TANGENT"] => AttributeType::Tangent,
            ["TEXCOORD", a] => AttributeType::TexCoord(a.parse().ok()?),
            ["COLOR", a] => AttributeType::Color(a.parse().ok()?),
            ["JOINTS", a] => AttributeType::Joints(a.parse().ok()?),
            ["WEIGHTS", a] => AttributeType::Weights(a.parse().ok()?),
            _ => return None,
        };

        Some(ty)
    }
}
