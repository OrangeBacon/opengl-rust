use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    Expr, Ident, ItemStruct, Result, Token,
};

/// The inputs to the attribute
#[derive(Debug)]
struct Attrs {
    accessor: Expr,
    globals: Vec<Ident>,
}

impl Parse for Attrs {
    /// parses `$ident => #($ident,)+`
    fn parse(input: ParseStream) -> Result<Self> {
        let accessor = input.parse()?;

        input.parse::<Token![=]>()?;
        input.parse::<Token![>]>()?;

        let globals = Punctuated::<Ident, Token![,]>::parse_terminated(input)?;

        Ok(Self {
            accessor,
            globals: globals.into_iter().collect(),
        })
    }
}

#[proc_macro_attribute]
pub fn context_globals(attr: TokenStream, input: TokenStream) -> TokenStream {
    let attr = parse_macro_input!(attr as Attrs);
    let input = parse_macro_input!(input as ItemStruct);

    let name = input.ident.clone();
    let name_str = name.to_string();
    let name_main = name_str.strip_suffix("Context").unwrap_or(&name_str[..]);

    let accessor = attr.accessor;
    let vis = input.vis.clone();
    let mut methods = vec![];

    // create the implementations for all the globals
    for global in attr.globals {
        // the input name e.g. `inputs` is the field name, remove the `s` to
        // make it not a plural for the method names
        let global_name = global.to_string();
        let global_name = &global_name[..global_name.len() - 1];

        // get the name for the method with explicit variable location specified
        let mut global_loc = String::from(global_name);
        global_loc.push_str("_loc");
        let global_loc = Ident::new(&global_loc, global.span());

        // infer the name of the GlobalAllocationContext enum varient
        let mut allocation = String::from(name_main);
        allocation.push(
            global_name
                .chars()
                .next()
                .unwrap_or_default()
                .to_ascii_uppercase(),
        );
        allocation.push_str(&global_name[1..]);
        let allocation = Ident::new(&allocation, global.span());

        let global_name = Ident::new(global_name, global.span());

        methods.push(quote! {
            #vis fn #global_name(&mut self, name: &str, ty: Type) -> Expression {
                let id = self.#accessor.#global.len();

                self.#accessor.#global.push(GlobalVariable {
                    name: name.to_string(),
                    ty,
                    start_location: None,
                });

                Expression::GetVariable {
                    variable: VariableId::Global(id, GlobalAllocationContext::#allocation),
                }
            }

            #vis fn #global_loc(&mut self, name: &str, ty: Type, start_location: usize) -> Expression {
                let id = self.#accessor.#global.len();

                self.#accessor.#global.push(GlobalVariable {
                    name: name.to_string(),
                    ty,
                    start_location: Some(start_location),
                });

                Expression::GetVariable {
                    variable: VariableId::Global(id, GlobalAllocationContext::#allocation),
                }
            }
        });
    }

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    TokenStream::from(quote! {
        #input

        impl #impl_generics #name #ty_generics #where_clause {
            #(#methods)*
        }
    })
}
