use crate::todoitem::TodoItem;

pub struct TodoLibrary {
    pub file_name: String,
    pub items: Vec<TodoItem>,
}

impl TodoLibrary {
    pub fn new(file_name: String) -> Self {
        TodoLibrary {
            file_name,
            items: Vec::new(),
        }
    }

    pub fn load(&mut self) -> Result<(), std::io::Error> {
        let content = std::fs::read_to_string(&self.file_name)?;
        self.items = content
            .lines()
            .filter_map(|line| line.parse::<TodoItem>().ok())
            .collect();
        Ok(())
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let content = self
            .items
            .iter()
            .map(|item| item.to_string())
            .collect::<Vec<String>>()
            .join("\n");
        std::fs::write(&self.file_name, content)?;
        Ok(())
    }

    pub fn add_item(&mut self, item: TodoItem) {
        self.items.push(item);
    }

    pub fn remove_item(&mut self, index: usize) -> Option<TodoItem> {
        if index < self.items.len() {
            Some(self.items.remove(index))
        } else {
            None
        }
    }
    pub fn list_items(&self) -> &Vec<TodoItem> {
        &self.items
    }

    pub fn clear_items(&mut self) {
        self.items.clear();
    }
    pub fn item_count(&self) -> usize {
        self.items.len()
    }
    pub fn complete_item(&mut self, index: usize) -> Option<()> {
        if index < self.items.len() {
            self.items[index].done = true;
            Some(())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_new() {
        let lib = TodoLibrary::new("test.txt".to_string());
        assert_eq!(lib.file_name, "test.txt");
        assert!(lib.items.is_empty());
    }

    #[test]
    fn test_add_item() {
        let mut lib = TodoLibrary::new("dummy.txt".to_string());
        let item = "Test item".parse().unwrap();
        lib.add_item(item);
        assert_eq!(lib.items.len(), 1);
        assert_eq!(lib.items[0].description, "Test item");
    }

    #[test]
    fn test_remove_item() {
        let mut lib = TodoLibrary::new("dummy.txt".to_string());
        lib.add_item("Item 1".parse().unwrap());
        lib.add_item("Item 2".parse().unwrap());
        let removed = lib.remove_item(0);
        assert!(removed.is_some());
        assert_eq!(lib.items.len(), 1);
        assert_eq!(removed.unwrap().description, "Item 1");
    }

    #[test]
    fn test_remove_item_out_of_bounds() {
        let mut lib = TodoLibrary::new("dummy.txt".to_string());
        let removed = lib.remove_item(0);
        assert!(removed.is_none());
    }

    #[test]
    fn test_list_items() {
        let mut lib = TodoLibrary::new("dummy.txt".to_string());
        lib.add_item("Item".parse().unwrap());
        let items = lib.list_items();
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_item_count() {
        let mut lib = TodoLibrary::new("dummy.txt".to_string());
        lib.add_item("Item 1".parse().unwrap());
        lib.add_item("Item 2".parse().unwrap());
        assert_eq!(lib.item_count(), 2);
    }

    #[test]
    fn test_complete_item() {
        let mut lib = TodoLibrary::new("dummy.txt".to_string());
        lib.add_item("Incomplete".parse().unwrap());
        let result = lib.complete_item(0);
        assert!(result.is_some());
        assert!(lib.items[0].done);
    }

    #[test]
    fn test_complete_item_out_of_bounds() {
        let mut lib = TodoLibrary::new("dummy.txt".to_string());
        let result = lib.complete_item(0);
        assert!(result.is_none());
    }

    #[test]
    fn test_load_and_save() {
        let temp_dir = std::env::temp_dir();
        let file_name = format!("{}.txt", uuid::Uuid::new_v4());
        let path = temp_dir.join(file_name);
        let content = "Buy milk\nx Complete task\n";
        fs::write(&path, content).unwrap();
        let mut lib = TodoLibrary::new(path.to_str().unwrap().to_string());
        lib.load().unwrap();
        assert_eq!(lib.items.len(), 2);
        assert_eq!(lib.items[0].description, "Buy milk");
        assert_eq!(lib.items[1].description, "Complete task");
        assert!(lib.items[1].done);
        lib.save().unwrap();
        let saved_content = fs::read_to_string(&path).unwrap();
        assert_eq!(saved_content.trim(), content.trim());
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn test_clear_items() {
        let mut lib = TodoLibrary::new("dummy.txt".to_string());
        lib.add_item("Item 1".parse().unwrap());
        lib.add_item("Item 2".parse().unwrap());
        assert_eq!(lib.item_count(), 2);
        lib.clear_items();
        assert_eq!(lib.item_count(), 0);
        assert!(lib.list_items().is_empty());
    }

    #[test]
    fn test_item_count_empty() {
        let lib = TodoLibrary::new("dummy.txt".to_string());
        assert_eq!(lib.item_count(), 0);
    }

    #[test]
    fn test_list_items_multiple() {
        let mut lib = TodoLibrary::new("dummy.txt".to_string());
        lib.add_item("First".parse().unwrap());
        lib.add_item("Second".parse().unwrap());
        let items = lib.list_items();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].description, "First");
        assert_eq!(items[1].description, "Second");
    }

    #[test]
    fn test_complete_item_success() {
        let mut lib = TodoLibrary::new("dummy.txt".to_string());
        lib.add_item("Uncompleted".parse().unwrap());
        assert!(!lib.items[0].done);
        let result = lib.complete_item(0);
        assert!(result.is_some());
        assert!(lib.items[0].done);
    }

    #[test]
    fn test_save_empty() {
        let lib = TodoLibrary::new("test_empty.txt".to_string());
        lib.save().unwrap();
        let content = fs::read_to_string("test_empty.txt").unwrap();
        assert_eq!(content.trim(), "");
        fs::remove_file("test_empty.txt").unwrap();
    }
}
