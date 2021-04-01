use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
};

// type declarations

#[derive(Debug)]
pub struct Program {
    vertex: Option<VertexShader>,
    frag: Option<FragmentShader>,
    uniforms: Vec<Variable>,
}

pub trait Shader: Debug {
    fn env(&self) -> &[Variable];
}

#[derive(Debug)]
pub struct VertexShader {
    main: Function,
    variables: Vec<Variable>,
    outputs: Vec<Variable>,
    inputs: Vec<Variable>,
}

#[derive(Debug)]
pub struct FragmentShader {
    main: Function,
    variables: Vec<Variable>,
    outputs: Vec<Variable>,
    inputs: Vec<Variable>,
}

#[derive(Debug)]
pub struct Function {
    blocks: Vec<Block>,
    variables: Vec<Variable>,
}

#[derive(Debug)]
pub struct Block {
    statements: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub enum Expression {
    CallBuiltin {
        function: BuiltinFunction,
        arguments: Vec<Expression>,
    },
    MakeFloat {
        value: f32,
    },
    GetVariable {
        variable: VariableId,
    },
}

#[derive(Debug, Clone)]
pub enum Statement {
    CallBuiltin {
        function: BuiltinFunction,
        arguments: Vec<VariableId>,
        result: Option<VariableId>,
    },
    MakeFloat {
        value: f32,
        variable: VariableId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinFunction {
    Add,
    BitAnd,
    BitOr,
    BitXor,
    Div,
    Mul,
    Rem,
    Sub,
    Texture,
    MakeVec4,
    Output,
    AccessField,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VariableId(usize, VariableAllocationContext);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum VariableAllocationContext {
    Uniform,
    VertexInput,
    VertexOutput,
    FragmentInput,
    FragmentOutput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    name: String,
    ty: Type,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinVariable {
    VertexPosition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Vector(usize),
    Matrix(usize, usize),
    Floating,
    Sampler2D,
}

impl Type {
    #![allow(non_upper_case_globals)]

    pub const Mat4: Type = Type::Matrix(4, 4);
}

// implementations
impl Program {
    pub fn new(constructor: impl FnOnce(&mut ProgramContext)) -> Self {
        let program = Program {
            vertex: None,
            frag: None,
            uniforms: vec![],
        };
        let mut ctx = ProgramContext::new(program);

        constructor(&mut ctx);

        ctx.program
    }
}

pub struct ProgramContext {
    program: Program,
}

impl ProgramContext {
    fn new(program: Program) -> Self {
        ProgramContext { program }
    }

    pub fn vertex(&mut self, constructor: impl FnOnce(&mut VertexContext)) {
        let shader = VertexShader::new(self, constructor);
        self.program.vertex = Some(shader);
    }

    pub fn frag(&mut self, constructor: impl FnOnce(&mut FragmentContext)) {
        let shader = FragmentShader::new(self, constructor);
        self.program.frag = Some(shader);
    }

    pub fn uniform(&mut self, name: &str, ty: Type) -> Expression {
        let id = self.program.uniforms.len();

        self.program.uniforms.push(Variable {
            name: name.to_string(),
            ty,
        });

        Expression::GetVariable {
            variable: VariableId(id, VariableAllocationContext::Uniform),
        }
    }
}

impl VertexShader {
    fn new(prog: &mut ProgramContext, constructor: impl FnOnce(&mut VertexContext)) -> Self {
        let mut shader = VertexShader {
            variables: vec![],
            outputs: vec![],
            inputs: vec![],
            main: Function {
                blocks: vec![],
                variables: vec![],
            },
        };

        let mut ctx = VertexContext {
            program: prog,
            shader: &mut shader,
        };

        constructor(&mut ctx);

        shader
    }
}

impl Shader for VertexShader {
    fn env(&self) -> &[Variable] {
        &self.variables
    }
}

impl FragmentShader {
    fn new(prog: &mut ProgramContext, constructor: impl FnOnce(&mut FragmentContext)) -> Self {
        let mut shader = FragmentShader {
            variables: vec![],
            inputs: vec![],
            outputs: vec![],
            main: Function {
                blocks: vec![],
                variables: vec![],
            },
        };

        let mut ctx = FragmentContext {
            program: prog,
            shader: &mut shader,
        };

        constructor(&mut ctx);

        shader
    }
}

impl Shader for FragmentShader {
    fn env(&self) -> &[Variable] {
        &self.variables
    }
}

pub struct VertexContext<'a, 'b> {
    program: &'a mut ProgramContext,
    shader: &'b mut VertexShader,
}

impl<'a, 'b> VertexContext<'a, 'b> {
    pub fn input(&mut self, name: &str, ty: Type) -> Expression {
        let id = self.shader.inputs.len();

        self.shader.inputs.push(Variable {
            name: name.to_string(),
            ty,
        });

        Expression::GetVariable {
            variable: VariableId(id, VariableAllocationContext::VertexInput),
        }
    }

    pub fn output(&mut self, name: &str, ty: Type) -> Expression {
        let id = self.shader.outputs.len();

        self.shader.outputs.push(Variable {
            name: name.to_string(),
            ty,
        });

        Expression::GetVariable {
            variable: VariableId(id, VariableAllocationContext::VertexOutput),
        }
    }
}

impl<'a, 'b> Deref for VertexContext<'a, 'b> {
    type Target = ProgramContext;

    fn deref(&self) -> &Self::Target {
        self.program
    }
}

impl<'a, 'b> DerefMut for VertexContext<'a, 'b> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.program
    }
}

pub struct FragmentContext<'a, 'b> {
    program: &'a mut ProgramContext,
    shader: &'b mut FragmentShader,
}

impl<'a, 'b> FragmentContext<'a, 'b> {
    pub fn input(&mut self, name: &str, ty: Type) -> Expression {
        let id = self.shader.inputs.len();

        self.shader.inputs.push(Variable {
            name: name.to_string(),
            ty,
        });

        Expression::GetVariable {
            variable: VariableId(id, VariableAllocationContext::FragmentInput),
        }
    }

    pub fn output(&mut self, name: &str, ty: Type) -> Expression {
        let id = self.shader.outputs.len();

        self.shader.outputs.push(Variable {
            name: name.to_string(),
            ty,
        });

        Expression::GetVariable {
            variable: VariableId(id, VariableAllocationContext::FragmentOutput),
        }
    }
}

impl<'a, 'b> Deref for FragmentContext<'a, 'b> {
    type Target = ProgramContext;

    fn deref(&self) -> &Self::Target {
        self.program
    }
}

impl<'a, 'b> DerefMut for FragmentContext<'a, 'b> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.program
    }
}

macro_rules! ExpressionOpSelf {
    ($op:ident, $func:ident) => {
        impl ::std::ops::$op for Expression {
            type Output = Self;

            fn $func(self, rhs: Self) -> Self {
                Expression::CallBuiltin {
                    function: BuiltinFunction::$op,
                    arguments: vec![self, rhs],
                }
            }
        }
    };
}

macro_rules! ExpressionOpF32 {
    ($op:ident, $func:ident) => {
        impl ::std::ops::$op<f32> for Expression {
            type Output = Self;

            fn $func(self, rhs: f32) -> Self {
                Expression::CallBuiltin {
                    function: BuiltinFunction::$op,
                    arguments: vec![self, Expression::MakeFloat { value: rhs }],
                }
            }
        }
    };
}

macro_rules! ExpressionOps {
    ($op:ident, $func:ident) => {
        ExpressionOpSelf!($op, $func);
        ExpressionOpF32!($op, $func);
    };

    ($($op:ident, $func:ident;)+) => {
        $(ExpressionOps! {$op, $func})+
    };
}

ExpressionOps! {
    Add, add;
    BitAnd, bitand;
    BitOr, bitor;
    BitXor, bitxor;
    Div, div;
    Mul, mul;
    Rem, rem;
    Sub, sub;
}
