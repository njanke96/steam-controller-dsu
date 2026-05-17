pub fn main() {
    let return_code = scdsu_cli::entrypoint();
    std::process::exit(return_code);
}
