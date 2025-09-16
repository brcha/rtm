use dioxus::prelude::*;
use todotxt::{TodoItem, TodoLibrary};

#[derive(Clone, PartialEq)]
struct AppState {
    lib: TodoLibrary,
    new_task: String,
    filter: String,
    completed_filter: bool,
}

fn main() {
    dioxus_desktop::launch(app);
}

fn app(cx: Scope) -> Element {
    let state = use_state(cx, || {
        let mut lib = TodoLibrary::new("todo.txt".to_string());
        lib.load().unwrap_or(());
        AppState {
            lib,
            new_task: String::new(),
            filter: String::new(),
            completed_filter: false,
        }
    });

    render! {
        div { class: "container",
            h1 { "Todo.txt GUI" }

            // Add new task
            div { class: "add-task",
                input {
                    placeholder: "New task...",
                    value: "{state.new_task}",
                    oninput: move |e| {
                        let new_value = e.value.clone();
                        state.set(|s| AppState {
                            new_task: new_value,
                            ..s.clone()
                        });
                    }
                }
                button {
                    onclick: move |_| {
                        if !state.new_task.is_empty() {
                            match state.new_task.parse::<TodoItem>() {
                                Ok(item) => {
                                    let mut new_lib = state.lib.clone();
                                    new_lib.add_item(item);
                                    new_lib.save().unwrap_or_else(|e| eprintln!("Save error: {:?}", e));
                                    state.set(|s| AppState {
                                        lib: new_lib,
                                        new_task: String::new(),
                                        ..s.clone()
                                    });
                                }
                                Err(e) => {
                                    eprintln!("Error parsing todo: {:?}", e);
                                }
                            }
                        }
                    },
                    "Add"
                }
            }

            // Filter
            div { class: "filter",
                input {
                    placeholder: "Filter...",
                    value: "{state.filter}",
                    oninput: move |e| {
                        let new_value = e.value.clone();
                        state.set(|s| AppState {
                            filter: new_value,
                            ..s.clone()
                        });
                    }
                }
                label {
                    input {
                        r#type: "checkbox",
                        checked: "{state.completed_filter}",
                        onchange: move |_| {
                            let new_completed = !state.completed_filter;
                            state.set(|s| AppState {
                                completed_filter: new_completed,
                                ..s.clone()
                            });
                        }
                    }
                    "Completed"
                }
            }

            // List tasks
            div { class: "task-list",
                for (i, item) in state.lib.list_items().iter().enumerate() {
                    if !state.filter.is_empty() && !item.description.contains(&state.filter) {
                        continue;
                    }
                    if item.done != state.completed_filter {
                        continue;
                    }
                    rsx! {
                        div { class: "task",
                            "{i + 1}. {item}"
                            button {
                                onclick: move |_| {
                                    let mut new_lib = state.lib.clone();
                                    new_lib.complete_item(i).unwrap_or_else(|| eprintln!("Complete failed"));
                                    new_lib.save().unwrap_or_else(|e| eprintln!("Save error: {:?}", e));
                                    state.set(|s| AppState {
                                        lib: new_lib,
                                        ..s.clone()
                                    });
                                },
                                "Complete"
                            }
                        }
                    }
                }
            }

            div { class: "footer",
                "Total items: {state.lib.item_count()}"
            }
        }
    }
}
