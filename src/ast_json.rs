//! Сериализация AST в JSON по спецификации §7 (V0.5).
//!
//! Формат узла: `{"kind": ..., "span": ..., "data": {...}}`. Span'ы в AST
//! появятся в V0.6 (ERRATA §3), поэтому поле `span` присутствует и равно
//! `null`; порядок ключей детерминирован. Комментарии в AST не попадают.

use crate::ast::Expr;
use crate::json::Json;
use crate::modules;

/// Сериализует программу в узел `Program`.
pub fn program_to_json(forms: &[Expr]) -> Json {
    node(
        "Program",
        vec![(
            "toplevels",
            Json::Array(forms.iter().map(expr_node).collect()),
        )],
    )
}

fn node(kind: &str, data: Vec<(&str, Json)>) -> Json {
    Json::object(vec![
        ("kind", Json::str(kind)),
        ("span", Json::Null),
        ("data", Json::object(data)),
    ])
}

fn literal_node(kind: &str, value: Json) -> Json {
    node(kind, vec![("value", value)])
}

fn symbol_node(name: &str) -> Json {
    match name.split_once('/') {
        Some((module, symbol)) => node(
            "QualifiedName",
            vec![("module", Json::str(module)), ("name", Json::str(symbol))],
        ),
        None => identifier_node(name),
    }
}

fn identifier_node(name: &str) -> Json {
    node("Identifier", vec![("name", Json::str(name))])
}

fn body_array(forms: &[Expr]) -> Json {
    Json::Array(forms.iter().map(expr_node).collect())
}

fn expr_node(expr: &Expr) -> Json {
    match expr {
        Expr::Number(value) => literal_node("LiteralInt", Json::Int(*value)),
        Expr::Float(value) => literal_node("LiteralFloat", Json::Float(*value)),
        Expr::Str(value) => literal_node("LiteralStr", Json::str(value.clone())),
        Expr::Bool(value) => literal_node("LiteralBool", Json::Bool(*value)),
        Expr::None => node("LiteralNone", vec![]),
        Expr::Symbol(name) => symbol_node(name),
        Expr::List(items) => list_node(items),
    }
}

fn list_node(items: &[Expr]) -> Json {
    let Some(Expr::Symbol(head)) = items.first() else {
        return call_node(items);
    };
    match head.as_str() {
        "define" | "define-mutable" | "define-const" | "define-public" => {
            define_node(head, &items[1..])
        }
        "set!" => set_node(&items[1..]),
        "if" => if_node(&items[1..]),
        "cond" => cond_node(&items[1..]),
        "match" => match_node(&items[1..]),
        "if-let" => if_let_node(&items[1..]),
        "while" => node(
            "While",
            vec![
                (
                    "condition",
                    items.get(1).map(expr_node).unwrap_or(Json::Null),
                ),
                ("body", body_array(&items[2..])),
            ],
        ),
        "for" => for_node(&items[1..]),
        "loop" => node("Loop", vec![("body", body_array(&items[1..]))]),
        "import" => import_node(&items[1..]),
        "defstruct" => defstruct_node(&items[1..]),
        "test" => node(
            "Test",
            vec![
                (
                    "name",
                    items
                        .get(1)
                        .map(|value| match value {
                            Expr::Str(name) => Json::str(name.clone()),
                            other => Json::str(format!("{:?}", other)),
                        })
                        .unwrap_or(Json::Null),
                ),
                ("body", body_array(&items[2..])),
            ],
        ),
        "assert" => node(
            "Assert",
            vec![("expr", items.get(1).map(expr_node).unwrap_or(Json::Null))],
        ),
        "#lupus" => node(
            "Directive",
            vec![
                (
                    "name",
                    items
                        .get(1)
                        .map(|value| match value {
                            Expr::Symbol(name) => Json::str(name.clone()),
                            other => Json::str(format!("{:?}", other)),
                        })
                        .unwrap_or(Json::Null),
                ),
                (
                    "args",
                    Json::Array(
                        items[2..]
                            .iter()
                            .map(|value| match value {
                                Expr::Symbol(name) => Json::str(name.clone()),
                                Expr::Str(name) => Json::str(name.clone()),
                                other => Json::str(format!("{:?}", other)),
                            })
                            .collect(),
                    ),
                ),
            ],
        ),
        _ => call_node(items),
    }
}

fn call_node(items: &[Expr]) -> Json {
    node(
        "Call",
        vec![
            ("func", items.first().map(expr_node).unwrap_or(Json::Null)),
            (
                "args",
                Json::Array(items.iter().skip(1).map(expr_node).collect()),
            ),
        ],
    )
}

fn define_node(head: &str, rest: &[Expr]) -> Json {
    let mutable = head == "define-mutable";
    let public = head == "define-public";
    let constant = head == "define-const";

    match rest.first() {
        Some(Expr::Symbol(name)) => node(
            "Define",
            vec![
                ("name", Json::str(name.clone())),
                ("mutable", Json::Bool(mutable)),
                ("public", Json::Bool(public)),
                ("constant", Json::Bool(constant)),
                ("value", rest.get(1).map(expr_node).unwrap_or(Json::Null)),
                ("type_annotation", Json::Null),
            ],
        ),
        Some(Expr::List(header)) => {
            let (name, params, return_type, body) = function_header(head, header, &rest[1..]);
            let lambda = node(
                "Lambda",
                vec![
                    ("params", Json::Array(params)),
                    ("body", body_array(body)),
                    ("return_type", return_type),
                ],
            );
            node(
                "Define",
                vec![
                    ("name", Json::str(name)),
                    ("mutable", Json::Bool(mutable)),
                    ("public", Json::Bool(public)),
                    ("constant", Json::Bool(constant)),
                    ("value", lambda),
                    ("type_annotation", Json::Null),
                ],
            )
        }
        _ => node(
            "Define",
            vec![
                ("name", Json::Null),
                ("mutable", Json::Bool(mutable)),
                ("public", Json::Bool(public)),
                ("constant", Json::Bool(constant)),
                ("value", Json::Null),
                ("type_annotation", Json::Null),
            ],
        ),
    }
}

/// Разбирает заголовок функции и отделяет необязательный `-> type`.
fn function_header<'a>(
    _head: &str,
    header: &'a [Expr],
    rest: &'a [Expr],
) -> (String, Vec<Json>, Json, &'a [Expr]) {
    let name = match header.first() {
        Some(Expr::Symbol(name)) => name.clone(),
        _ => String::new(),
    };
    let params = header
        .iter()
        .skip(1)
        .map(|param| match param {
            Expr::Symbol(name) => node(
                "Param",
                vec![
                    ("name", Json::str(name.clone())),
                    ("type_annotation", Json::Null),
                ],
            ),
            Expr::List(items) => node(
                "Param",
                vec![
                    (
                        "name",
                        items
                            .first()
                            .map(|value| match value {
                                Expr::Symbol(name) => Json::str(name.clone()),
                                other => Json::str(format!("{:?}", other)),
                            })
                            .unwrap_or(Json::Null),
                    ),
                    ("type_annotation", type_node(items.get(1))),
                ],
            ),
            other => node(
                "Param",
                vec![
                    ("name", Json::str(format!("{:?}", other))),
                    ("type_annotation", Json::Null),
                ],
            ),
        })
        .collect();

    if matches!(rest.first(), Some(Expr::Symbol(arrow)) if arrow == "->") {
        let return_type = type_node(rest.get(1));
        let body = if rest.len() > 2 {
            &rest[2..]
        } else {
            &rest[rest.len()..]
        };
        (name, params, return_type, body)
    } else {
        (name, params, Json::Null, rest)
    }
}

fn type_node(expr: Option<&Expr>) -> Json {
    let Some(expr) = expr else {
        return Json::Null;
    };
    match expr {
        Expr::Symbol(kind) => node(
            "Type",
            vec![
                ("kind", Json::str(kind.clone())),
                ("params", Json::Array(Vec::new())),
            ],
        ),
        Expr::List(items) => {
            let kind = match items.first() {
                Some(Expr::Symbol(name)) => name.clone(),
                _ => String::new(),
            };
            node(
                "Type",
                vec![
                    ("kind", Json::str(kind)),
                    (
                        "params",
                        Json::Array(
                            items
                                .iter()
                                .skip(1)
                                .map(|item| type_node(Some(item)))
                                .collect(),
                        ),
                    ),
                ],
            )
        }
        other => node(
            "Type",
            vec![
                ("kind", Json::str(format!("{:?}", other))),
                ("params", Json::Array(Vec::new())),
            ],
        ),
    }
}

fn set_node(rest: &[Expr]) -> Json {
    node(
        "SetBang",
        vec![
            (
                "name",
                rest.first()
                    .map(|value| match value {
                        Expr::Symbol(name) => Json::str(name.clone()),
                        other => Json::str(format!("{:?}", other)),
                    })
                    .unwrap_or(Json::Null),
            ),
            ("value", rest.get(1).map(expr_node).unwrap_or(Json::Null)),
        ],
    )
}

fn if_node(rest: &[Expr]) -> Json {
    node(
        "If",
        vec![
            (
                "condition",
                rest.first().map(expr_node).unwrap_or(Json::Null),
            ),
            (
                "then_branch",
                Json::Array(rest.get(1).map(|e| vec![expr_node(e)]).unwrap_or_default()),
            ),
            (
                "else_branch",
                Json::Array(rest.get(2).map(|e| vec![expr_node(e)]).unwrap_or_default()),
            ),
        ],
    )
}

fn cond_node(rest: &[Expr]) -> Json {
    let clauses = rest
        .iter()
        .map(|clause| match clause {
            Expr::List(items) => Json::object(vec![
                (
                    "condition",
                    items.first().map(expr_node).unwrap_or(Json::Null),
                ),
                ("body", body_array(&items[1..])),
            ]),
            other => Json::object(vec![
                ("condition", expr_node(other)),
                ("body", Json::Array(Vec::new())),
            ]),
        })
        .collect();
    node("Cond", vec![("clauses", Json::Array(clauses))])
}

fn match_node(rest: &[Expr]) -> Json {
    let clauses = rest
        .iter()
        .skip(1)
        .map(|clause| match clause {
            Expr::List(items) => Json::object(vec![
                (
                    "pattern",
                    items.first().map(pattern_node).unwrap_or(Json::Null),
                ),
                ("body", body_array(&items[1..])),
            ]),
            other => Json::object(vec![
                ("pattern", pattern_node(other)),
                ("body", Json::Array(Vec::new())),
            ]),
        })
        .collect();
    node(
        "Match",
        vec![
            ("expr", rest.first().map(expr_node).unwrap_or(Json::Null)),
            ("clauses", Json::Array(clauses)),
        ],
    )
}

fn pattern_node(pattern: &Expr) -> Json {
    match pattern {
        Expr::Symbol(name) if name == "_" => node("PatternWildcard", vec![]),
        Expr::Symbol(name) => node("PatternVar", vec![("name", Json::str(name.clone()))]),
        Expr::None => node("PatternLiteralNone", vec![]),
        Expr::List(items) => {
            let constructor = match items.first() {
                Some(Expr::Symbol(name)) => name.clone(),
                _ => String::new(),
            };
            let args = Json::Array(items.iter().skip(1).map(pattern_node).collect());
            if constructor == "tuple" || constructor == "Tuple" {
                return node("PatternTuple", vec![("args", args)]);
            }
            node(
                "PatternConstructor",
                vec![("constructor", Json::str(constructor)), ("args", args)],
            )
        }
        other => node(
            "PatternConstructor",
            vec![
                ("constructor", Json::str(format!("{:?}", other))),
                ("args", Json::Array(Vec::new())),
            ],
        ),
    }
}

fn if_let_node(rest: &[Expr]) -> Json {
    let binding = match rest.first() {
        Some(Expr::List(items)) if items.len() == 2 => Json::object(vec![
            (
                "name",
                items
                    .first()
                    .map(|value| match value {
                        Expr::Symbol(name) => Json::str(name.clone()),
                        other => Json::str(format!("{:?}", other)),
                    })
                    .unwrap_or(Json::Null),
            ),
            ("expr", expr_node(&items[1])),
        ]),
        _ => Json::Null,
    };
    node(
        "IfLet",
        vec![
            ("binding", binding),
            (
                "then_branch",
                Json::Array(rest.get(1).map(|e| vec![expr_node(e)]).unwrap_or_default()),
            ),
            (
                "else_branch",
                Json::Array(rest.get(2).map(|e| vec![expr_node(e)]).unwrap_or_default()),
            ),
        ],
    )
}

fn for_node(rest: &[Expr]) -> Json {
    node(
        "For",
        vec![
            (
                "var",
                rest.first()
                    .map(|value| match value {
                        Expr::Symbol(name) => Json::str(name.clone()),
                        other => Json::str(format!("{:?}", other)),
                    })
                    .unwrap_or(Json::Null),
            ),
            ("iter", rest.get(2).map(expr_node).unwrap_or(Json::Null)),
            ("body", body_array(rest.get(3..).unwrap_or(&[]))),
        ],
    )
}

fn import_node(rest: &[Expr]) -> Json {
    match modules::parse_import(rest) {
        Ok(spec) => node(
            "Import",
            vec![
                ("package", Json::str(spec.package.clone())),
                (
                    "module",
                    Json::str(spec.module.clone().unwrap_or_else(|| spec.package.clone())),
                ),
                (
                    "alias",
                    spec.alias.clone().map(Json::str).unwrap_or(Json::Null),
                ),
                ("import_all", Json::Bool(spec.import_all)),
            ],
        ),
        Err(_) => node(
            "Import",
            vec![
                ("package", Json::Null),
                ("module", Json::Null),
                ("alias", Json::Null),
                ("import_all", Json::Bool(false)),
            ],
        ),
    }
}

fn defstruct_node(rest: &[Expr]) -> Json {
    let (name, type_params) = match rest.first() {
        Some(Expr::Symbol(name)) => (name.clone(), Vec::new()),
        Some(Expr::List(header)) => {
            let name = match header.first() {
                Some(Expr::Symbol(name)) => name.clone(),
                _ => String::new(),
            };
            let params = header
                .iter()
                .skip(1)
                .filter_map(|param| match param {
                    Expr::Symbol(name) => Some(Json::str(name.clone())),
                    _ => None,
                })
                .collect();
            (name, params)
        }
        _ => (String::new(), Vec::new()),
    };

    let fields = rest
        .iter()
        .skip(1)
        .map(|field| match field {
            Expr::List(items) => Json::object(vec![
                (
                    "name",
                    items
                        .first()
                        .map(|value| match value {
                            Expr::Symbol(name) => Json::str(name.clone()),
                            other => Json::str(format!("{:?}", other)),
                        })
                        .unwrap_or(Json::Null),
                ),
                ("type", type_node(items.get(1))),
            ]),
            other => Json::object(vec![
                ("name", Json::str(format!("{:?}", other))),
                ("type", Json::Null),
            ]),
        })
        .collect();

    node(
        "DefStruct",
        vec![
            ("name", Json::str(name)),
            ("type_params", Json::Array(type_params)),
            ("fields", Json::Array(fields)),
        ],
    )
}
