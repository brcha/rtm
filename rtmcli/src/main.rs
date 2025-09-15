use chrono::{Duration, Local};
use clap::{Parser, Subcommand, ValueEnum};
use todotxt::TodoItem;
use todotxt::TodoLibrary;

#[derive(Parser)]
#[command(name = "rtmcli")]
#[command(about = "Rust Todo.txt Manager CLI")]
struct Cli {
    /// Todo.txt file name
    #[arg(short = 'f', long)]
    file: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List tasks
    List {
        /// List completed tasks
        #[arg(short, long)]
        completed: bool,

        /// Filter by due date
        #[arg(value_enum)]
        filter: Option<Filter>,
    },
    /// Add a new task
    Add {
        /// Todo description in Todo.txt format
        description: String,
    },
    /// Complete tasks by filter or identifier
    Complete {
        /// First arg: filter or identifier
        arg1: String,

        /// Second arg: optional index or identifier
        arg2: Option<String>,
    },
}

#[derive(ValueEnum, Clone, PartialEq)]
enum Filter {
    Today,
    Week,
    All,
    Overdue,
}

fn parse_filter(s: &str) -> Option<Filter> {
    match s {
        "today" => Some(Filter::Today),
        "week" => Some(Filter::Week),
        "all" => Some(Filter::All),
        "overdue" => Some(Filter::Overdue),
        _ => None,
    }
}

fn main() {
    let cli = Cli::parse();
    let file_name = cli
        .file
        .unwrap_or_else(|| std::env::var("TODOTXT").unwrap_or_else(|_| "todo.txt".to_string()));

    let mut lib = TodoLibrary::new(file_name.clone());

    match cli.command {
        Commands::List { completed, filter } => {
            if let Err(e) = lib.load() {
                eprintln!("Error loading file '{}': {}", file_name, e);
                std::process::exit(1);
            }

            let today = Local::now().date_naive();
            let date_filter = match filter {
                Some(Filter::Today) => Some((today, today)),
                Some(Filter::Week) => Some((today, today + Duration::days(6))),
                Some(Filter::All) => None,
                Some(Filter::Overdue) => Some((chrono::NaiveDate::MIN, today - Duration::days(1))),
                None => None,
            };

            let filtered_items: Vec<_> = lib
                .list_items()
                .iter()
                .filter(|item| {
                    item.done == completed
                        && match date_filter {
                            None => true,
                            Some((start, end)) => {
                                item.due.map_or(false, |d| d >= start && d <= end)
                            }
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
        Commands::Add { description } => {
            let item: TodoItem = description.parse().unwrap_or_else(|e| {
                eprintln!("Error parsing todo: {:?}", e);
                std::process::exit(1);
            });
            lib.add_item(item);
            if let Err(e) = lib.save() {
                eprintln!("Error saving file: {}", e);
                std::process::exit(1);
            }
            println!("Added task to '{}'", file_name);
        }
        Commands::Complete { arg1, arg2 } => {
            if let Err(e) = lib.load() {
                eprintln!("Error loading file '{}': {}", file_name, e);
                std::process::exit(1);
            }

            let today = Local::now().date_naive();
            let items = lib.list_items();
            let mut indices_to_complete = Vec::new();

            let arg1_ref = &arg1;
            if let Some(filter_name) = parse_filter(arg1_ref) {
                let date_filter = match filter_name {
                    Filter::Today => Some((today, today)),
                    Filter::Week => Some((today, today + Duration::days(6))),
                    Filter::All => None,
                    Filter::Overdue => Some((chrono::NaiveDate::MIN, today - Duration::days(1))),
                };

                let filtered_indices: Vec<usize> = items
                    .iter()
                    .enumerate()
                    .filter(|(_, item)| {
                        !item.done
                            && date_filter.map_or(true, |(start, end)| {
                                item.due.map_or(false, |d| d >= start && d <= end)
                            })
                    })
                    .map(|(i, _)| i)
                    .collect();

                if let Some(ref arg2) = arg2 {
                    if let Ok(index) = arg2.parse::<usize>() {
                        if index > 0 && index <= filtered_indices.len() {
                            indices_to_complete.push(filtered_indices[index - 1]);
                        } else {
                            eprintln!("Invalid index for filter: {}", index);
                            std::process::exit(1);
                        }
                    } else if let Ok(uuid) = arg2.parse::<uuid::Uuid>() {
                        let mut found = false;
                        for &i in &filtered_indices {
                            if items[i].uuid == Some(uuid) {
                                indices_to_complete.push(i);
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            eprintln!("UUID {} not found in filtered list", uuid);
                            std::process::exit(1);
                        }
                    } else {
                        eprintln!("Invalid second arg: {}", arg2);
                        std::process::exit(1);
                    }
                } else {
                    // No second arg, complete all filtered
                    indices_to_complete = filtered_indices;
                }
            } else {
                // arg1 is identifier
                if let Ok(index) = arg1_ref.parse::<usize>() {
                    let uncompleted: Vec<usize> =
                        (0..items.len()).filter(|&i| !items[i].done).collect();
                    if index > 0 && index <= uncompleted.len() {
                        indices_to_complete.push(uncompleted[index - 1]);
                    } else {
                        eprintln!("Invalid absolute index: {}", index);
                        std::process::exit(1);
                    }
                } else if let Ok(uuid) = arg1_ref.parse::<uuid::Uuid>() {
                    let mut found = false;
                    for i in 0..items.len() {
                        if items[i].uuid == Some(uuid) && !items[i].done {
                            indices_to_complete.push(i);
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        eprintln!("UUID {} not found or already completed", uuid);
                        std::process::exit(1);
                    }
                } else {
                    eprintln!("Invalid arg: {}", arg1);
                    std::process::exit(1);
                }
            }

            for &i in &indices_to_complete {
                lib.complete_item(i).unwrap();
            }

            if !indices_to_complete.is_empty() {
                if let Err(e) = lib.save() {
                    eprintln!("Error saving file: {}", e);
                    std::process::exit(1);
                }
                println!(
                    "Completed {} task(s) in '{}'",
                    indices_to_complete.len(),
                    file_name
                );
            } else {
                println!("No tasks matched the criteria");
            }
        }
    }
}
