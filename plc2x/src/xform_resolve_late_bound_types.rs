//! Transform that resolves late bound types into specific types.
//! 
//! The IEC 61131-3 syntax has some ambiguous types that are initially
//! parsed into a placeholder. This transform replaces the placeholders
//! with well-known types.
use ironplc_dsl::fold::Fold;
use ironplc_dsl::visitor::Visitor;
use ironplc_dsl::{ast::Id, dsl::*};
use std::collections::HashMap;
use std::fmt::Error;
use phf::{phf_set, Set};

static ELEMENTARY_TYPES_LOWER_CASE: Set<&'static str> = phf_set! {
    // signed_integer_type_name
    "sint",
    "int",
    "dint",
    "lint",
    // unsigned_integer_type_name
    "usint",
    "uint",
    "udint",
    "ulint",
    // real_type_name
    "real",
    "lreal",
    // date_type_name
    "date",
    "time_of_day",
    "tod",
    "date_and_time",
    "dt",
    // bit_string_type_name
    "bool",
    "byte",
    "word",
    "dword",
    "lword",
    // remaining elementary_type_name
    "string",
    "wstring",
    "time"
};

pub fn apply(lib: Library) -> Result<Library, Error> {
    let mut type_map = HashMap::new();

    // Walk the entire library to find the types. We don't need
    // to keep track of contexts because types are global scoped.
    let mut visitor = GlobalTypeDefinitionVisitor {
        types: &mut type_map,
    };
    visitor.walk(&lib)?;

    // Set the types for each item.
    let mut resolver = TypeResolver { types: type_map };
    Ok(resolver.fold(lib))
}

// Finds types that are valid as variable types. These include enumerations,
// function blocks, functions, structures.
struct GlobalTypeDefinitionVisitor<'a> {
    types: &'a mut HashMap<Id, TypeDefinitionKind>,
}
impl<'a> Visitor<Error> for GlobalTypeDefinitionVisitor<'a> {
    type Value = ();
    fn visit_enum_declaration(&mut self, enum_decl: &EnumerationDeclaration) -> Result<(), Error> {
        self.types
            .insert(enum_decl.name.clone(), TypeDefinitionKind::Enumeration);
        Ok(())
    }
}

struct TypeResolver {
    types: HashMap<Id, TypeDefinitionKind>,
}

impl TypeResolver {
    fn is_elementary_type(id: &Id) -> bool {
        ELEMENTARY_TYPES_LOWER_CASE.contains(&id.lower_case().to_string())
    }
}

impl Fold for TypeResolver {
    fn fold_type_initializer(&mut self, node: TypeInitializer) -> TypeInitializer {
        match node {
            TypeInitializer::LateResolvedType(name) => {
                // Try to find the type for the specified name.
                if TypeResolver::is_elementary_type(&name) {
                    return TypeInitializer::Simple { type_name: name, initial_value: None }
                }

                // TODO error handling
                let type_kind = self.types.get(&name).unwrap();
                match type_kind {
                    TypeDefinitionKind::Enumeration => {
                        TypeInitializer::EnumeratedType(EnumeratedTypeInitializer {
                            type_name: name,
                            initial_value: None,
                        })
                    }
                    TypeDefinitionKind::FunctionBlock => {
                        TypeInitializer::FunctionBlock { type_name: name }
                    }
                    TypeDefinitionKind::Function => {
                        // TODO this is wrong and should be an error
                        TypeInitializer::Structure { type_name: name }
                    }
                    TypeDefinitionKind::Structure => TypeInitializer::Structure { type_name: name },
                }
            }
            _ => node,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::*;
    use crate::xform_resolve_late_bound_types::TypeResolver;
    use ironplc_dsl::fold::Fold;
    use ironplc_dsl::{ast::Id, dsl::*};
    use std::collections::HashMap;

    #[test]
    fn fold_when_has_function_block_type_then_resolves_type() {
        let input = new_library::<String>(LibraryElement::FunctionBlockDeclaration(
            FunctionBlockDeclaration {
                name: Id::from("LOGGER"),
                inputs: vec![VarInitDecl::late_bound("var_name", "var_type")],
                outputs: vec![],
                inouts: vec![],
                vars: vec![],
                externals: vec![],
                body: FunctionBlockBody::stmts(vec![]),
            },
        ))
        .unwrap();

        let mut type_map = HashMap::new();
        type_map.insert(Id::from("var_type"), TypeDefinitionKind::FunctionBlock);
        let mut type_resolver = TypeResolver { types: type_map };

        let result = type_resolver.fold(input);

        let expected = new_library::<String>(LibraryElement::FunctionBlockDeclaration(
            FunctionBlockDeclaration {
                name: Id::from("LOGGER"),
                inputs: vec![VarInitDecl::function_block("var_name", "var_type")],
                outputs: vec![],
                inouts: vec![],
                vars: vec![],
                externals: vec![],
                body: FunctionBlockBody::stmts(vec![]),
            },
        ))
        .unwrap();

        assert_eq!(result, expected)
    }
}
