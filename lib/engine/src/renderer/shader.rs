use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
};

// type declarations

#[derive(Debug)]
pub struct Program {
    shaders: Vec<Box<dyn Shader>>,
    uniforms: Vec<Variable>,
}

pub trait Shader: Debug {
    fn env(&self) -> &[Variable];
}

#[derive(Debug)]
pub struct VertexShader {
    main: Function,
    variables: Vec<Variable>,
}

#[derive(Debug)]
pub struct FragmentShader {
    main: Function,
    variables: Vec<Variable>,
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
pub struct VariableId(usize);

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
    Interface(String, Vec<(String, Type)>),
    Sampler2D,
}

// implementations
impl Program {
    pub fn new(constructor: impl FnOnce(&mut ProgramContext)) -> Self {
        let program = Program {
            shaders: vec![],
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
        self.program.shaders.push(Box::new(shader));
    }

    pub fn frag(&mut self, constructor: impl FnOnce(&mut FragmentContext)) {
        let shader = FragmentShader::new(self, constructor);
        self.program.shaders.push(Box::new(shader));
    }

    pub fn uniform(&mut self, name: &str, ty: Type) -> Expression {
        let id = self.program.uniforms.len();

        self.program.uniforms.push(Variable {
            name: name.to_string(),
            ty,
        });

        Expression::GetVariable {
            variable: VariableId(id),
        }
    }
}

impl VertexShader {
    fn new(prog: &mut ProgramContext, constructor: impl FnOnce(&mut VertexContext)) -> Self {
        let mut ctx = VertexContext { program: prog };

        constructor(&mut ctx);

        VertexShader {
            variables: vec![],
            main: Function {
                blocks: vec![],
                variables: vec![],
            },
        }
    }
}

impl Shader for VertexShader {
    fn env(&self) -> &[Variable] {
        &self.variables
    }
}

impl FragmentShader {
    fn new(prog: &mut ProgramContext, constructor: impl FnOnce(&mut FragmentContext)) -> Self {
        let mut ctx = FragmentContext { program: prog };

        constructor(&mut ctx);

        FragmentShader {
            variables: vec![],
            main: Function {
                blocks: vec![],
                variables: vec![],
            },
        }
    }
}

impl Shader for FragmentShader {
    fn env(&self) -> &[Variable] {
        &self.variables
    }
}

pub struct VertexContext<'a> {
    program: &'a mut ProgramContext,
}

impl<'a> Deref for VertexContext<'a> {
    type Target = ProgramContext;

    fn deref(&self) -> &Self::Target {
        self.program
    }
}

impl<'a> DerefMut for VertexContext<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.program
    }
}

pub struct FragmentContext<'a> {
    program: &'a mut ProgramContext,
}

impl<'a> Deref for FragmentContext<'a> {
    type Target = ProgramContext;

    fn deref(&self) -> &Self::Target {
        self.program
    }
}

impl<'a> DerefMut for FragmentContext<'a> {
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

impl Type {
    #![allow(non_upper_case_globals)]

    pub const Mat4: Type = Type::Matrix(4, 4);
}
