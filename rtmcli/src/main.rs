use chrono::{Duration, Local};
use std::env;
use todotxt::TodoLibrary;

fn main() {
    let mut file_name = env::var("TODOTXT").unwrap_or_else(|_| "todo.txt".to_string());

    let mut args: Vec<String> = env::args().skip(1).collect();

    if !args.is_empty() && args[0] == "-f" {
        if args.len() > 1 {
            file_name = args[1].clone();
            args.drain(0..2);
        }
    }

    let mut lib = TodoLibrary::new(file_name.clone());

    if let Err(e) = lib.load() {
        eprintln!("Error loading file '{}': {}", file_name, e);
        std::process::exit(1);
    }

    let today = Local::now().date_naive();
    let mut status_completed = false;
    let mut date_filter: Option<(chrono::NaiveDate, chrono::NaiveDate)> = None;

    for arg in &args {
        match arg.as_str() {
            "list" => {} // ignore
            "completed" => status_completed = true,
            "uncompleted" => status_completed = false,
            "today" => date_filter = Some((today, today)),
            "week" => date_filter = Some((today, today + Duration::days(6))),
            "all" => date_filter = None,
            "overdue" => date_filter = Some((chrono::NaiveDate::MIN, today - Duration::days(1))),
            _ => {
                eprintln!("Unknown argument: {}", arg);
                eprintln!("Usage: [list] [completed|uncompleted] [today|week|all|overdue]");
                std::process::exit(1);
            }
        }
    }

    let filtered_items: Vec<_> = lib
        .list_items()
        .iter()
        .filter(|item| {
            item.done == status_completed
                && match date_filter {
                    None => true,
                    Some((start, end)) => item.due.map_or(false, |d| d >= start && d <= end),
                }
        })
        .collect();

    println!("Tasks in '{}':", file_name);
    for (i, item) in filtered_items.iter().enumerate() {
        println!("{}. {}", i + 1, item);
    }

    if filtered_items.is_empty() {
        println!("No tasks found.");
    }
}
