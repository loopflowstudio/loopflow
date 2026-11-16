use journey_ownership::runtime::skill;

fn main() {
    let name = std::env::args().nth(1).expect("provide a skill name");
    match skill(&name) {
        Some(body) => println!("{body}"),
        None => std::process::exit(2),
    }
}
