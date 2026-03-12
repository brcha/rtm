const { invoke } = window.__TAURI__.core;
const { open } = window.__TAURI__.dialog;

let items = [];
let fileLoaded = false;
let currentEditIndex = null;
let dueDatePicker = null;
let thresholdDatePicker = null;

async function loadFile() {
  const selected = await open({
    multiple: false,
    filters: [{
      name: 'Text Files',
      extensions: ['txt']
    }]
  });

  if (selected) {
    try {
      await invoke('load_file', { path: selected });
      fileLoaded = true;
      await refreshItems();
      await updateFileName();
    } catch (error) {
      console.error('Failed to load file:', error);
      alert('Failed to load file: ' + error);
    }
  }
}

async function updateFileName() {
  const fileNameEl = document.getElementById('file-name');
  const fileName = await invoke('get_file_name');
  if (fileName) {
    fileNameEl.textContent = fileName;
  } else {
    fileNameEl.textContent = 'No file loaded';
  }
}

async function checkFileLoaded() {
  const hasFile = await invoke('has_file_loaded');
  fileLoaded = hasFile;
}

async function refreshItems() {
  try {
    items = await invoke('get_items');
    renderItems();
    await updateItemCount();
  } catch (error) {
    console.error('Failed to get items:', error);
  }
}

async function updateItemCount() {
  const count = await invoke('get_item_count');
  document.getElementById('item-count').textContent = `Total items: ${count}`;
}

async function addItem() {
  const input = document.getElementById('new-item-input');
  const text = input.value.trim();

  if (!text) {
    return;
  }

  if (!fileLoaded) {
    alert('Please load a file first');
    return;
  }

  try {
    await invoke('add_item', { text });
    input.value = '';
    await refreshItems();
  } catch (error) {
    console.error('Failed to add item:', error);
    alert('Failed to add item: ' + error);
  }
}

async function completeItem(index) {
  const item = items[index];
  if (!item) return;
  try {
    await invoke('complete_item', { index: item.index });
    await refreshItems();
  } catch (error) {
    console.error('Failed to complete item:', error);
    alert('Failed to complete item: ' + error);
  }
}

async function uncompleteItem(index) {
  const item = items[index];
  if (!item) return;
  try {
    await invoke('uncomplete_item', { index: item.index });
    await refreshItems();
  } catch (error) {
    console.error('Failed to uncomplete item:', error);
    alert('Failed to uncomplete item: ' + error);
  }
}

function openEditDialog(index) {
  const item = items[index];
  if (!item) return;

  currentEditIndex = item.index;

  document.getElementById('edit-description').value = item.description;
  document.getElementById('edit-priority').value = item.priority !== null ? item.priority : '';
  document.getElementById('edit-recurrence').value = item.recurrence || '';
  document.getElementById('edit-projects').value = item.projects.join(', ');
  document.getElementById('edit-contexts').value = item.contexts.join(', ');

  if (dueDatePicker) {
    dueDatePicker.setDate(item.due || null, false);
  }
  if (thresholdDatePicker) {
    thresholdDatePicker.setDate(item.threshold || null, false);
  }

  document.getElementById('edit-dialog').style.display = 'flex';
}

function closeEditDialog() {
  document.getElementById('edit-dialog').style.display = 'none';
  currentEditIndex = null;
}

async function saveEdit() {
  if (currentEditIndex === null) return;

  const description = document.getElementById('edit-description').value.trim();
  const priorityStr = document.getElementById('edit-priority').value;
  const dueDate = dueDatePicker ? dueDatePicker.selectedDates[0] : null;
  const recurrence = document.getElementById('edit-recurrence').value.trim() || null;
  const thresholdDate = thresholdDatePicker ? thresholdDatePicker.selectedDates[0] : null;
  const projectsStr = document.getElementById('edit-projects').value;
  const contextsStr = document.getElementById('edit-contexts').value;

  if (!description) {
    alert('Description is required');
    return;
  }

  const due = dueDate ? flatpickr.formatDate(dueDate, 'Y-m-d') : null;
  const threshold = thresholdDate ? flatpickr.formatDate(thresholdDate, 'Y-m-d') : null;
  const priority = priorityStr ? parseInt(priorityStr, 10) : null;
  const projects = projectsStr ? projectsStr.split(',').map(p => p.trim()).filter(p => p) : [];
  const contexts = contextsStr ? contextsStr.split(',').map(c => c.trim()).filter(c => c) : [];

  try {
    await invoke('update_item', {
      request: {
        index: currentEditIndex,
        description,
        priority,
        due,
        recurrence,
        threshold,
        projects,
        contexts
      }
    });
    closeEditDialog();
    await refreshItems();
  } catch (error) {
    console.error('Failed to update item:', error);
    alert('Failed to update item: ' + error);
  }
}

function renderItems() {
  const listEl = document.getElementById('todo-list');

  if (!fileLoaded) {
    listEl.innerHTML = '<p class="no-file">No file loaded. Use \'Load File\' to select a todo.txt file.</p>';
    return;
  }

  if (items.length === 0) {
    listEl.innerHTML = '<p class="no-file">No items found.</p>';
    return;
  }

  listEl.innerHTML = items.map((item, displayIndex) => {
    let priorityClass = '';
    if (item.priority !== null) {
      const letter = String.fromCharCode(65 + item.priority);
      priorityClass = `priority-${letter.toLowerCase()}`;
    }

    return `
      <div class="todo-item ${item.done ? 'completed' : ''}">
        ${!item.done 
          ? `<button class="btn-complete item-complete-btn" onclick="completeItem(${displayIndex})" title="Complete">☐</button>` 
          : `<button class="btn-uncomplete item-complete-btn" onclick="uncompleteItem(${displayIndex})" title="Uncomplete">☑</button>`}
        <div class="item-text" onclick="openEditDialog(${displayIndex})">
          ${item.priority !== null ? `<span class="priority ${priorityClass}">(${String.fromCharCode(65 + item.priority)})</span> ` : ''}
          ${escapeHtml(item.description)}
          ${item.projects.map(p => `<span class="project">+${escapeHtml(p)}</span>`).join(' ')}
          ${item.contexts.map(c => `<span class="context">@${escapeHtml(c)}</span>`).join(' ')}
          ${item.due ? `<span class="due">due:${item.due}</span>` : ''}
          ${item.recurrence ? `<span class="recurrence">rec:${item.recurrence}</span>` : ''}
          ${item.threshold ? `<span class="threshold">t:${item.threshold}</span>` : ''}
        </div>
      </div>
    `;
  }).join('');
}

function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}

async function loadConfig() {
  try {
    const config = await invoke('get_config');
    document.getElementById('show-completed').checked = config.show_completed_items;
    document.getElementById('show-future').checked = config.show_future_items;
    document.getElementById('hide-no-date').checked = config.hide_no_date;
    document.getElementById('reverse-sort').checked = config.reverse_sort;
  } catch (error) {
    console.error('Failed to load config:', error);
  }
}

async function saveConfig() {
  const showCompleted = document.getElementById('show-completed').checked;
  const showFuture = document.getElementById('show-future').checked;
  const hideNoDate = document.getElementById('hide-no-date').checked;
  const reverseSort = document.getElementById('reverse-sort').checked;

  try {
    await invoke('save_config', {
      showCompletedItems: showCompleted,
      showFutureItems: showFuture,
      hideNoDate: hideNoDate,
      reverseSort: reverseSort
    });
    await refreshItems();
  } catch (error) {
    console.error('Failed to save config:', error);
  }
}

window.addEventListener('DOMContentLoaded', async () => {
  dueDatePicker = flatpickr('#edit-due', {
    dateFormat: 'Y-m-d',
    placeholder: 'Select due date',
    allowInput: true
  });

  thresholdDatePicker = flatpickr('#edit-threshold', {
    dateFormat: 'Y-m-d',
    placeholder: 'Select threshold',
    allowInput: true
  });

  document.getElementById('load-btn').addEventListener('click', loadFile);
  
  document.getElementById('add-btn').addEventListener('click', addItem);
  
  document.getElementById('new-item-input').addEventListener('keypress', (e) => {
    if (e.key === 'Enter') {
      addItem();
    }
  });

  document.getElementById('show-completed').addEventListener('change', saveConfig);
  document.getElementById('show-future').addEventListener('change', saveConfig);
  document.getElementById('hide-no-date').addEventListener('change', saveConfig);
  document.getElementById('reverse-sort').addEventListener('change', saveConfig);

  document.getElementById('edit-cancel').addEventListener('click', closeEditDialog);
  document.getElementById('edit-form').addEventListener('submit', (e) => {
    e.preventDefault();
    saveEdit();
  });

  await loadConfig();
  await updateFileName();
  await checkFileLoaded();
  await refreshItems();
});

window.completeItem = completeItem;
window.uncompleteItem = uncompleteItem;
window.openEditDialog = openEditDialog;
