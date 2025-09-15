use chrono::{Duration, Local};
use clap::{Parser, Subcommand, ValueEnum};
use todotxt::TodoItem;
use todotxt::TodoLibrary;

#[derive(Parser)]
#[command(name = "rtmcli")]
#[command(about = "Simple Todo.txt CLI")]
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
}

#[derive(ValueEnum, Clone)]
enum Filter {
    Today,
    Week,
    All,
    Overdue,
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
    }
}
