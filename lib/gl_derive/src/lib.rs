extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, DeriveInput, Data, DataStruct, Fields, Field, Meta, MetaNameValue, Lit,
};
use proc_macro2::TokenStream as TokenStream2;

#[proc_macro_derive(VertexAttribPointers, attributes(location))]
pub fn vertex_attrib_pointers_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    generate_impl(&input)
}

fn generate_impl(ast: &DeriveInput) -> TokenStream {
    let ident = &ast.ident;
    let generics = &ast.generics;
    let where_clause = &ast.generics.where_clause;

    let fields = generate_calls(&ast.data);

    TokenStream::from(quote! {
        impl #ident #generics #where_clause {
            #[allow(unused_variables)]
            pub fn attrib_pointers() {
                let stride = ::std::mem::size_of::<Self>();
                let offset = 0;

                #(#fields)*
            }
        }
    })
}

fn generate_calls(body: &Data) -> Vec<TokenStream2> {
    match body {
        Data::Enum(_) => panic!("Cannot derive for enum"),
        Data::Union(_) => panic!("Cannot derive for union"),
        Data::Struct(DataStruct {fields: Fields::Unnamed(_), ..}) => panic!("Cannot derive for tuple struct"),
        Data::Struct(DataStruct {fields: Fields::Unit, ..}) => panic!("Cannot derive for unit struct"),
        Data::Struct(DataStruct {fields: Fields::Named(ref a), ..}) => {
            a.named.iter().map(generate_one_call).collect()
        }
    }
}

fn generate_one_call(field: &Field) -> TokenStream2 {
    let field_name = match field.ident {
        Some(ref i) => format!("{}", i),
        None => String::from(""),
    };

    let location_attr = field.attrs
        .iter()
        .filter(|a| {
            let path = &a.path;
            if !(path.leading_colon.is_none() && path.segments.len() == 1) {
                return false;
            }

            let seg = path.segments.first().unwrap();

            seg.arguments.is_empty() && format!("{}", seg.ident) == "location"
        })
        .next()
        .unwrap_or_else(|| panic!(
            "Field {:?} is missing #[location = ?] attribute", field_name
        ));

    let tokens = location_attr.parse_meta().unwrap();
    let int_lit = match tokens {
        Meta::NameValue(MetaNameValue { lit: Lit::Int(a), ..}) => a,
        _ => panic!("Expected name value"),
    };

    let field_ty = &field.ty;

    quote! {
        let location = #int_lit;
        unsafe {
            #field_ty::attrib_pointer(stride, location, offset);
        }
        let offset = offset + ::std::mem::size_of::<#field_ty>();
    }
}