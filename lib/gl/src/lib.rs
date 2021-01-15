#[allow(clippy::all)]
mod bindings {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

pub use bindings::Gl as InnerGl;
pub use bindings::*;

use std::{fmt, ops::Deref, rc::Rc};

#[derive(Clone)]
pub struct Gl {
    inner: Rc<bindings::Gl>,
}

impl Gl {
    pub fn load_with<F>(loadfn: F) -> Gl
    where
        F: FnMut(&'static str) -> *const types::GLvoid,
    {
        Gl {
            inner: Rc::new(bindings::Gl::load_with(loadfn)),
        }
    }
}

impl Deref for Gl {
    type Target = bindings::Gl;

    fn deref(&self) -> &bindings::Gl {
        &self.inner
    }
}

impl fmt::Debug for Gl {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{{gl context}}")
    }
}
