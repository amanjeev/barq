const tooltip = document.getElementById('tooltip');
const dialog = document.getElementById('symbol-dialog');
const dialogContent = document.getElementById('dialog-content');
const search = document.getElementById('search');
const searchStatus = document.getElementById('search-status');
const blocks = [...document.querySelectorAll('.symbol-block')];
const groupInfo = new Map(blocks.map(block => {
  const content = document.getElementById(block.dataset.members).content;
  return [block, {
    members: [...content.querySelectorAll('[data-name]')].map(el => el.dataset),
    overlaps: [...content.querySelectorAll('[data-overlap-name]')].map(el => el.dataset),
    label: block.getAttribute('aria-label'),
    displayName: block.querySelector('.symbol-name').textContent,
    query: '',
  }];
}));
const totalSymbols = [...groupInfo.values()].reduce((total, group) => total + group.members.length, 0);
let hideTimer;
let searchTimer;
let dialogTrigger;

function sectionDescription(section) {
  const matches = name => section === name || section.startsWith(name + '.');
  if (matches('.bss')) return 'Writable data zeroed at startup; occupies RAM.';
  if (matches('.rodata')) return 'Read-only constants; usually stored in Flash.';
  if (matches('.data')) return 'Initialized writable data; lives in RAM, usually copied from Flash at startup.';
  if (matches('.rwtext') || matches('.trap')) return 'RAM-resident code or trap handling in the ESP32-C3 examples.';
  if (matches('.text')) return 'Executable code; usually stored in Flash.';
  if (matches('.vector_table')) return 'Startup and interrupt-handler addresses used by the processor.';
  return 'ELF section containing this symbol; placement depends on the target and linker.';
}


function detailRows(el) {
  const { name, displayName, addr, size, section } = el.dataset;
  const kind = el.classList.contains('function') ? 'Function — executable code.'
    : el.classList.contains('data') ? 'Data — a variable or constant.'
    : 'Other — an ELF symbol such as an address marker or label.';
  const rows = [
    ['label', displayName],
    ['explanation', kind],
    ['addr', `Address: 0x${BigInt(addr).toString(16).padStart(8, '0')}`],
    ['explanation', 'Memory address on the microcontroller.'],
    ['size', `Size: ${size} bytes`],
    ['explanation', size === '0'
      ? 'No size recorded. This can be a useful marker or function alias; it does not establish how much memory the referenced code uses.'
      : 'Byte count recorded for this symbol. Overlapping symbols can describe the same memory.'],
    ['addr', `Section: ${section}`],
    ['explanation', sectionDescription(section)],
  ];
  const group = groupInfo.get(el);
  if (group.members.length > 1) {
    const unknown = group.members.filter(member => member.size === '0').length;
    rows.push(['explanation', `${group.members.length} symbol entries share this starting address; ${unknown} have no recorded size. Sharing an address alone does not prove identical code or data.`]);
  }
  if (group.members.length > 1 || BigInt(el.dataset.shared) > 0n) {
    rows.push(['size', `Known shared bytes: ${el.dataset.shared}`]);
    rows.push(['explanation', 'Only intersections of nonzero-size symbol ranges count. Each shared byte is counted once; zero-size entries do not establish how many bytes they share.']);
  }
  if (group.members.length > 1 || group.overlaps.length > 0) {
    rows.push(['explanation', 'Open details for every name, its recorded size, and intersecting address groups.']);
  }
  if (displayName !== name) {
    rows.push(['explanation', 'This is a readable Rust name. Open details for the original ELF name used by CLI exclusions.']);
  }
  return rows;
}

function renderDetails(container, el) {
  container.replaceChildren();
  for (const [className, text] of detailRows(el)) {
    const row = document.createElement('p');
    row.className = className;
    row.textContent = text;
    container.append(row);
  }
}

function appendTable(container, captionText, headings, rows) {
  const table = document.createElement('table');
  table.className = 'details-table';
  const caption = document.createElement('caption');
  caption.textContent = captionText;
  table.append(caption);
  const head = document.createElement('thead');
  const headingRow = document.createElement('tr');
  for (const heading of headings) {
    const cell = document.createElement('th');
    cell.scope = 'col';
    cell.textContent = heading;
    headingRow.append(cell);
  }
  head.append(headingRow);
  table.append(head);
  const body = document.createElement('tbody');
  for (const { cells, match } of rows) {
    const row = document.createElement('tr');
    cells.forEach((text, index) => {
      const cell = document.createElement('td');
      cell.textContent = typeof text === 'string' ? text : text.display;
      if (typeof text !== 'string' && text.original !== text.display) {
        const original = document.createElement('code');
        original.className = 'original-name';
        original.textContent = `ELF: ${text.original}`;
        cell.append(original);
      }
      if (index === 0 && match) {
        const badge = document.createElement('span');
        badge.className = 'match-badge';
        badge.textContent = 'Search match';
        cell.append(badge);
      }
      row.append(cell);
    });
    body.append(row);
  }
  table.append(body);
  container.append(table);
}

function matchesName(member, query) {
  return member.name.toLowerCase().includes(query)
    || member.displayName.toLowerCase().includes(query);
}

function nameCell(display, original) {
  return { display, original };
}

function renderGroupDetails(el) {
  renderDetails(dialogContent, el);
  const group = groupInfo.get(el);
  appendTable(dialogContent, `Symbol entries at 0x${BigInt(el.dataset.addr).toString(16).padStart(8, '0')}`,
    ['Name', 'Type', 'Recorded size'], group.members.map(member => ({
      cells: [nameCell(member.displayName, member.name), member.kind, member.size === '0' ? 'Unknown (0 recorded)' : `${member.size} bytes`],
      match: group.query !== '' && matchesName(member, group.query),
    })));
  if (group.overlaps.length > 0) {
    appendTable(dialogContent, 'Intersecting address groups in this section',
      ['Group', 'Starting address', 'Shared bytes'], group.overlaps.map(other => ({
        cells: [nameCell(other.overlapDisplayName, other.overlapName), `0x${BigInt(other.overlapAddr).toString(16).padStart(8, '0')}`, other.overlapBytes],
      })));
  }
}

function hideTooltip() {
  clearTimeout(hideTimer);
  tooltip.classList.remove('visible');
}

function scheduleHide() {
  // Allow the pointer to cross the gap between the symbol and its preview.
  clearTimeout(hideTimer);
  hideTimer = setTimeout(hideTooltip, 200);
}

function showTooltip(el, event) {
  clearTimeout(hideTimer);
  if (dialog.open || event.pointerType === 'touch') return;
  renderDetails(tooltip, el);
  tooltip.classList.add('visible');
  const rect = tooltip.getBoundingClientRect();
  tooltip.style.left = Math.max(8, Math.min(event.clientX + 12, window.innerWidth - rect.width - 8)) + 'px';
  tooltip.style.top = Math.max(8, Math.min(event.clientY + 12, window.innerHeight - rect.height - 8)) + 'px';
}

blocks.forEach(el => {
  el.addEventListener('pointerenter', event => showTooltip(el, event));
  el.addEventListener('pointerleave', scheduleHide);
  // Native buttons supply Enter/Space activation and touch support.
  el.addEventListener('click', () => {
    hideTooltip();
    dialogTrigger = el;
    renderGroupDetails(el);
    dialog.showModal();
  });
});

tooltip.addEventListener('pointerenter', () => clearTimeout(hideTimer));
tooltip.addEventListener('pointerleave', scheduleHide);
document.getElementById('close-dialog').addEventListener('click', () => dialog.close());
dialog.addEventListener('close', () => {
  if (dialogTrigger && !dialogTrigger.hidden) dialogTrigger.focus();
});
document.addEventListener('keydown', event => {
  if (event.key === 'Escape') hideTooltip();
  // Escape closes the native dialog without a custom keyboard trap.
});
window.addEventListener('resize', hideTooltip);
window.addEventListener('scroll', hideTooltip, { passive: true });

function updateSearch() {
  const query = search.value.toLowerCase();
  let visible = 0;
  let matchingSymbols = 0;
  hideTooltip();
  for (const el of blocks) {
    const group = groupInfo.get(el);
    const matches = group.members.filter(member => matchesName(member, query));
    group.query = query;
    el.hidden = matches.length === 0;
    if (!el.hidden) {
      visible++;
      matchingSymbols += matches.length;
    }
    // An alias search reveals its group and names the matching entry first.
    el.querySelector('.symbol-name').textContent = query && matches.length ? matches[0].displayName : group.displayName;
    el.setAttribute('aria-label', query && matches.length
      ? `${matches[0].displayName}. Search match. ${group.label}` : group.label);
  }
  for (const group of document.querySelectorAll('.section-group')) {
    group.hidden = ![...group.querySelectorAll('.symbol-block')].some(el => !el.hidden);
  }
  // Delay announcements while typing, while filtering immediately.
  clearTimeout(searchTimer);
  searchTimer = setTimeout(() => {
    searchStatus.textContent = visible === 0
      ? 'No symbols match your search.'
      : `${matchingSymbols} of ${totalSymbols} symbol entries match, in ${visible} of ${blocks.length} address groups.`;
  }, 200);
}
search.addEventListener('input', updateSearch);
updateSearch();
