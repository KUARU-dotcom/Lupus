//! Точка входа CLI интерпретатора Lupus V0.5.
//!
//! Подкоманды: `run`, `test`, `check`, `ast`, `eval`. Без подкоманды
//! выполняется `run`. Диагностики всегда печатаются в stderr строгим JSON
//! (§6.1); `--pretty` включает человекочитаемый рендер того же JSON.

use lupus::error::Error;
use lupus::json::Json;
use std::path::{Path, PathBuf};
use std::process;

/// Выбранная подкоманда.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Run,
    Test,
    Check,
    Ast,
    Eval,
}

/// Разобранные аргументы CLI.
#[derive(Debug, Default)]
struct Args {
    command: Option<Command>,
    positional: Vec<String>,
    pretty: bool,
    verbose: bool,
    no_lint: bool,
    json: bool,
    lenient: bool,
}

fn print_usage() {
    println!("Использование:");
    println!("  lupus [--lenient-parens] [--no-lint] файл.lupus");
    println!("  lupus run [флаги] файл.lupus");
    println!("  lupus test [--verbose] файл_или_каталог...");
    println!("  lupus check [--no-lint] файл.lupus");
    println!("  lupus ast файл.lupus");
    println!("  lupus eval \"<выражение>\"");
    println!("Флаги: --pretty, --json, --no-lint, --lenient-parens, --verbose, --version");
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut parsed = Args::default();
    let mut index = 0;

    if let Some(first) = args.first() {
        parsed.command = match first.as_str() {
            "run" => Some(Command::Run),
            "test" => Some(Command::Test),
            "check" => Some(Command::Check),
            "ast" => Some(Command::Ast),
            "eval" => Some(Command::Eval),
            _ => None,
        };
        if parsed.command.is_some() {
            index = 1;
        }
    }

    while index < args.len() {
        let arg = &args[index];
        match arg.as_str() {
            "--pretty" => parsed.pretty = true,
            "--verbose" => parsed.verbose = true,
            "--no-lint" => parsed.no_lint = true,
            "--json" => parsed.json = true,
            "--lenient-parens" => parsed.lenient = true,
            "--help" | "-h" => {
                print_usage();
                process::exit(0);
            }
            other if other.starts_with("--") => {
                return Err(format!("Неизвестный флаг: {}", other));
            }
            other => parsed.positional.push(other.to_string()),
        }
        index += 1;
    }

    Ok(parsed)
}

/// Печатает диагностику в stderr: строгий JSON либо `--pretty`-рендер.
fn report(error: &Error, pretty: bool, json: bool) {
    if pretty && !json {
        eprintln!("{}", error.to_pretty());
    } else {
        eprintln!("{}", error.to_json());
    }
}

fn fail(error: &Error, args: &Args) -> ! {
    report(error, args.pretty, args.json);
    process::exit(1);
}

fn read_source(path: &Path) -> Result<String, Error> {
    std::fs::read_to_string(path).map_err(|error| {
        Error::runtime(
            "io-error",
            format!("Не удалось прочитать файл {}: {}", path.display(), error),
        )
        .with_hint("Проверьте, что файл существует и доступен для чтения")
    })
}

fn parse_file(path: &Path, args: &Args) -> Result<Vec<lupus::ast::Expr>, Error> {
    let source = read_source(path)?;
    lupus::parse_source_lenient(&source, args.lenient)
        .map_err(|error| error.with_file_if_located(path.display().to_string()))
}

fn run_file(path: &Path, args: &Args) -> Result<(), Error> {
    let source = read_source(path)?;
    if !args.no_lint {
        let forms = lupus::parse_source_lenient(&source, args.lenient)
            .map_err(|error| error.with_file_if_located(path.display().to_string()))?;
        lupus::lint::check(&forms, Some(path))?;
    }
    lupus::run_source_named(&source, args.lenient, Some(path))
}

fn check_file(path: &Path, args: &Args) -> Result<(), Error> {
    let forms = parse_file(path, args)?;
    if args.no_lint {
        return Ok(());
    }
    lupus::lint::check(&forms, Some(path))
}

fn ast_file(path: &Path, args: &Args) -> Result<(), Error> {
    let forms = parse_file(path, args)?;
    println!("{}", lupus::ast_json::program_to_json(&forms));
    Ok(())
}

fn eval_expression(expression: &str, args: &Args) -> Result<(), Error> {
    let mut interpreter = lupus::Interpreter::with_lenient_parens(args.lenient);
    let value = interpreter.eval_source(expression)?;
    println!("{}", lupus::value::value_to_string(&value));
    Ok(())
}

/// Собирает `.lupus`-файлы из пути (файл или каталог рекурсивно).
fn collect_lupus_files(path: &Path) -> Result<Vec<PathBuf>, Error> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    if !path.is_dir() {
        return Err(Error::lint(
            "unknown-module",
            format!("Путь не найден: {}", path.display()),
        ));
    }

    fn walk(dir: &Path, output: &mut Vec<PathBuf>) -> Result<(), Error> {
        let entries = std::fs::read_dir(dir).map_err(|error| {
            Error::lint(
                "unknown-module",
                format!("Не удалось прочитать каталог {}: {}", dir.display(), error),
            )
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                Error::lint(
                    "unknown-module",
                    format!("Ошибка чтения каталога {}: {}", dir.display(), error),
                )
            })?;
            let path = entry.path();
            if path.is_dir() {
                walk(&path, output)?;
            } else if path.extension().is_some_and(|ext| ext == "lupus") {
                output.push(path);
            }
        }
        Ok(())
    }

    let mut files = Vec::new();
    walk(path, &mut files)?;
    files.sort();
    Ok(files)
}

/// JSON-описание ошибки внутри test-fail (код, location, message).
fn test_error_json(error: &Error) -> Json {
    let error_json = error.to_json_value();
    let Json::Object(entries) = error_json else {
        return Json::Null;
    };
    let mut selected = Vec::new();
    for key in ["code", "location", "message"] {
        if let Some((_, value)) = entries.iter().find(|(name, _)| name == key) {
            selected.push((key.to_string(), value.clone()));
        }
    }
    Json::Object(selected)
}

/// Выполняет тесты в одном файле и печатает JSONL по §5.3.
fn run_test_file(path: &Path, args: &Args) -> bool {
    let source = match read_source(path) {
        Ok(source) => source,
        Err(error) => {
            report(&error, args.pretty, args.json);
            return false;
        }
    };

    let forms = match lupus::parse_source_lenient(&source, args.lenient)
        .map_err(|error| error.with_file_if_located(path.display().to_string()))
    {
        Ok(forms) => forms,
        Err(error) => {
            report(&error, args.pretty, args.json);
            return false;
        }
    };

    if !args.no_lint {
        if let Err(error) = lupus::lint::check(&forms, Some(path)) {
            report(&error, args.pretty, args.json);
            return false;
        }
    }

    let mut interpreter = lupus::Interpreter::with_lenient_parens(args.lenient);
    if let Some(parent) = path.parent() {
        interpreter.set_program_dir(parent.to_path_buf());
    }

    let outcomes = match interpreter.run_tests(&forms, args.verbose) {
        Ok(outcomes) => outcomes,
        Err(error) => {
            report(&error, args.pretty, args.json);
            return false;
        }
    };

    let file = path.display().to_string();
    let total = outcomes.len();
    let mut passed = 0;
    let mut failed = 0;

    for outcome in &outcomes {
        println!(
            "{}",
            Json::object(vec![
                ("type", Json::str("test-start")),
                ("name", Json::str(outcome.name.clone())),
                ("file", Json::str(file.clone())),
            ])
        );

        if outcome.passed {
            passed += 1;
            println!(
                "{}",
                Json::object(vec![
                    ("type", Json::str("test-pass")),
                    ("name", Json::str(outcome.name.clone())),
                    ("file", Json::str(file.clone())),
                    ("duration_ms", Json::Float(outcome.duration_ms)),
                ])
            );
        } else {
            failed += 1;
            let error = outcome
                .error
                .as_ref()
                .map(test_error_json)
                .unwrap_or(Json::Null);
            println!(
                "{}",
                Json::object(vec![
                    ("type", Json::str("test-fail")),
                    ("name", Json::str(outcome.name.clone())),
                    ("file", Json::str(file.clone())),
                    ("error", error),
                    ("duration_ms", Json::Float(outcome.duration_ms)),
                ])
            );
        }
    }

    println!(
        "{}",
        Json::object(vec![
            ("type", Json::str("test-summary")),
            ("total", Json::Int(total as i64)),
            ("passed", Json::Int(passed as i64)),
            ("failed", Json::Int(failed as i64)),
            ("file", Json::str(file)),
        ])
    );

    failed == 0
}

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();

    // Глубина рекурсии Lupus — до 10 000 вызовов. Tree-walk eval в debug-сборке
    // расходует заметный объём стека на вызов, поэтому вычисление выполняется
    // в отдельном потоке с большим стеком; глубина всё равно ограничена
    // счётчиком `MAX_CALL_DEPTH`.
    let worker = std::thread::Builder::new()
        .name("lupus-eval".to_string())
        .stack_size(1024 * 1024 * 1024)
        .spawn(move || run_cli(raw))
        .expect("не удалось запустить поток интерпретатора");
    if worker.join().is_err() {
        process::exit(101);
    }
}

fn run_cli(raw: Vec<String>) {
    if raw.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("Lupus {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let args = match parse_args(&raw) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{}", Error::lint("unexpected-token", message).to_json());
            process::exit(1);
        }
    };

    let command = args.command.unwrap_or(Command::Run);
    let result = match command {
        Command::Run => {
            if args.positional.len() != 1 {
                print_usage();
                process::exit(1);
            }
            run_file(Path::new(&args.positional[0]), &args)
        }
        Command::Check => {
            if args.positional.len() != 1 {
                print_usage();
                process::exit(1);
            }
            check_file(Path::new(&args.positional[0]), &args)
        }
        Command::Ast => {
            if args.positional.len() != 1 {
                print_usage();
                process::exit(1);
            }
            ast_file(Path::new(&args.positional[0]), &args)
        }
        Command::Eval => {
            if args.positional.len() != 1 {
                print_usage();
                process::exit(1);
            }
            eval_expression(&args.positional[0], &args)
        }
        Command::Test => {
            if args.positional.is_empty() {
                print_usage();
                process::exit(1);
            }
            let mut all_ok = true;
            for path in &args.positional {
                match collect_lupus_files(Path::new(path)) {
                    Ok(files) => {
                        for file in files {
                            if !run_test_file(&file, &args) {
                                all_ok = false;
                            }
                        }
                    }
                    Err(error) => {
                        report(&error, args.pretty, args.json);
                        all_ok = false;
                    }
                }
            }
            if all_ok {
                Ok(())
            } else {
                process::exit(1);
            }
        }
    };

    if let Err(error) = result {
        fail(&error, &args);
    }
}
