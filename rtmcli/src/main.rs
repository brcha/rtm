use std::env;
use todotxt::TodoLibrary;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <todo_file>", args[0]);
        std::process::exit(1);
    }

    let file_name = &args[1];
    let mut lib = TodoLibrary::new(file_name.clone());

    if let Err(e) = lib.load() {
        eprintln!("Error loading file '{}': {}", file_name, e);
        std::process::exit(1);
    }

    println!("Tasks in '{}':", file_name);
    for (i, item) in lib.list_items().iter().enumerate() {
        println!("{}. {}", i + 1, item);
    }

    if lib.item_count() == 0 {
        println!("No tasks found.");
    }
}
