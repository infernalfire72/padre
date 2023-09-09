use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, Data, DataStruct, DeriveInput, Fields, Ident};

macro_rules! has_attr {
    ($e:expr; $ee:expr) => {
        $e.attrs
            .iter()
            .find(|attr| attr.path.is_ident($ee))
            .is_some()
    };
}

fn part_de_impl(ident: &Ident, sd_crate: TokenStream2, struct_data: &DataStruct) -> TokenStream2 {
    let fields = match struct_data.fields {
        Fields::Named(ref fields) => &fields.named,
        _ => unimplemented!(),
    };

    let field_idents = fields.iter().map(|field| &field.ident).collect::<Vec<_>>();
    let fields_rusted = field_idents
        .iter()
        .map(|ident| {
            let ident = ident.as_ref().unwrap();
            let rust_name = ident
                .to_string()
                .split("_")
                .map(|part| part[..1].to_uppercase() + &part[1..])
                .collect::<Vec<_>>()
                .join("");
            Ident::new(&rust_name, ident.span())
        })
        .collect::<Vec<_>>();
    let required = fields
        .iter()
        .filter_map(|field| {
            if has_attr!(field; "require") {
                Some(&field.ident)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    quote! {
        impl #ident {
            pub fn is_unset(&self) -> bool {
                #(self.#field_idents.is_none())&&*
            }
        }

        impl<'de> #sd_crate::de::Deserialize<'de> for #ident {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: #sd_crate::Deserializer<'de>,
            {
                enum Field { #(#fields_rusted,)* }

                impl<'de> #sd_crate::de::Deserialize<'de> for Field {
                    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                    where
                        D: #sd_crate::Deserializer<'de>,
                    {
                        struct FieldVisitor;

                        impl<'de> #sd_crate::de::Visitor<'de> for FieldVisitor {
                            type Value = Field;

                            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                                formatter.write_str(stringify!(#(#field_idents), *))
                            }

                            fn visit_str<E>(self, value: &str) -> Result<Field, E>
                            where
                                E: #sd_crate::de::Error,
                            {
                                match value {
                                    #(stringify!(#field_idents) => Ok(Field::#fields_rusted),)*
                                    _ => Err(#sd_crate::de::Error::unknown_field(value, FIELDS)),
                                }
                            }
                        }

                        deserializer.deserialize_identifier(FieldVisitor)
                    }
                }

                struct Visitor;

                impl<'de> #sd_crate::de::Visitor<'de> for Visitor {
                    type Value = #ident;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                        formatter.write_str(stringify!(struct #ident))
                    }

                    fn visit_map<V>(self, mut map: V) -> Result<#ident, V::Error>
                    where
                        V: #sd_crate::de::MapAccess<'de>,
                    {
                        #(let mut #field_idents = None;)*
                        while let Some(key) = map.next_key()? {
                            match key {
                                #(
                                    Field::#fields_rusted => {
                                        if #field_idents.is_some() {
                                            return Err(#sd_crate::de::Error::duplicate_field(stringify!(#field_idents)));
                                        }
                                        #field_idents = Some(map.next_value()?);
                                    }
                                )*
                            }
                        }
                        #(let #required = #required.ok_or_else(|| #sd_crate::de::Error::missing_field(stringify!(#required)))?;)*
                        Ok(#ident { #(#field_idents),* })
                    }
                }

                const FIELDS: &'static [&'static str] = &[#(stringify!(#field_idents)),*];
                deserializer.deserialize_struct(stringify!(#ident), FIELDS, Visitor)
            }
        }
    }
}

#[proc_macro_derive(PartialDeserialize, attributes(require, use_serde))]
pub fn partial_deserialize(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let ident = &input.ident;
    let slib_attr = input
        .attrs
        .iter()
        .find(|attr| attr.path.is_ident("use_serde"))
        .unwrap();

    let slib_crate: TokenStream2 = slib_attr.parse_args().unwrap();

    match input.data {
        Data::Struct(ref struct_data) => {
            TokenStream::from(part_de_impl(ident, slib_crate, struct_data))
        }
        _ => unimplemented!(),
    }
}
