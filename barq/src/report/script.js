const tooltip = document.getElementById('tooltip');

document.querySelectorAll('.symbol-block').forEach(el => {
  el.addEventListener('mouseenter', e => {
    const name = el.dataset.name;
    const addr = el.dataset.addr;
    const size = el.dataset.size;
    const section = el.dataset.section;
    tooltip.replaceChildren();
    for (const [className, text] of [
      ['label', name],
      ['addr', `Address: 0x${BigInt(addr).toString(16).padStart(8, '0')}`],
      ['size', `Size: ${size} bytes`],
      ['addr', `Section: ${section}`],
    ]) {
      const row = document.createElement('div');
      row.className = className;
      row.textContent = text;
      tooltip.append(row);
    }
    tooltip.classList.add('visible');
  });

  el.addEventListener('mousemove', e => {
    tooltip.style.left = (e.clientX + 12) + 'px';
    tooltip.style.top = (e.clientY + 12) + 'px';
  });

  el.addEventListener('mouseleave', () => {
    tooltip.classList.remove('visible');
  });
});

document.getElementById('search').addEventListener('input', e => {
  const query = e.target.value.toLowerCase();
  document.querySelectorAll('.symbol-block').forEach(el => {
    const name = el.dataset.name.toLowerCase();
    if (query === '' || name.includes(query)) {
      el.classList.remove('hidden');
    } else {
      el.classList.add('hidden');
    }
  });
});
