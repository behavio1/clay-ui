fn main() {
    let arguments: Result<Vec<_>, _> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.into_string())
        .collect();
    let Ok(arguments) = arguments else {
        eprintln!("Error: arguments must be valid UTF-8");
        std::process::exit(2);
    };
    std::process::exit(clayui_lib::cli::run(arguments));
}
