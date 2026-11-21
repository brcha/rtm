use super::*;

#[test]
fn test_check_one_line_accepts_a_single_line() {
    assert_eq!(
        TodoItem::check_one_line("(A) Buy milk +home due:2026-01-01"),
        Ok(())
    );
    assert_eq!(TodoItem::check_one_line(""), Ok(()));
}

#[test]
fn test_check_one_line_refuses_line_breaks() {
    for text in [
        "Buy milk\nx 2026-01-01 Pay rent",
        "Buy milk\rPay rent",
        "Buy milk\r\nPay rent",
        "Buy milk\n",
        "Buy milk\r",
    ] {
        assert_eq!(
            TodoItem::check_one_line(text),
            Err(TodoItemLineError),
            "{text:?}"
        );
    }
}
