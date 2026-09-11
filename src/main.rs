//! Точка входа CLI интерпретатора Lupus.
//!
//! Использование: `lupus [--lenient-parens] файл.lupus`
//!
//! При ошибке выполнения сообщение печатается в stdout, а процесс
//! завершается с кодом 1 (по аналогии с python-прототипом `lupus_proto.py`).

use std::env;
use std::process;

fn print_usage() {
    println!("Использование: lupus [--lenient-parens] файл.lupus");
}

fn main() {
    let args: Vec<String> = env::args().collect();

    // Флаг `--lenient-parens` включает мягкий режим: лишние `)` в коде,
    // сгенерённом моделью, игнорируются с предупреждением в stderr.
    let lenient = args.iter().any(|a| a == "--lenient-parens");

    // Флаг `--version` / `-V` печатает версию и завершает работу.
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("Lupus {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    // Файлы — все аргументы, не начинающиеся с `-`.
    let files: Vec<String> = args
        .iter()
        .skip(1)
        .filter(|a| !a.starts_with("--"))
        .cloned()
        .collect();

    if files.len() != 1 {
        print_usage();
        process::exit(1);
    }

    match lupus::run_file_lenient(&files[0], lenient) {
        Ok(()) => {}
        Err(e) => {
            println!("{}", e.message);
            process::exit(1);
        }
    }
}
