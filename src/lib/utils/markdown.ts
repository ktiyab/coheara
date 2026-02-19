/**
 * Spec 48 [CA-03]: Lightweight, secure markdown renderer for AI responses.
 *
 * Supports: bold, italic, bullet lists, numbered lists, headers, inline code,
 * simple tables, horizontal rules.
 *
 * Intentionally BLOCKS: links, images, HTML tags (XSS prevention).
 * Patient messages must NEVER use this renderer.
 */

/** Escape HTML entities to prevent XSS. */
function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

/** Render inline markdown (bold, italic, code) within a single line. */
function renderInline(line: string): string {
  let html = escapeHtml(line);
  // Inline code: `text`
  html = html.replace(/`([^`]+)`/g, '<code class="md-code">$1</code>');
  // Bold: **text** or __text__
  html = html.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');
  html = html.replace(/__([^_]+)__/g, '<strong>$1</strong>');
  // Italic: *text* or _text_ (must not match ** or __)
  html = html.replace(/(?<!\*)\*([^*]+)\*(?!\*)/g, '<em>$1</em>');
  html = html.replace(/(?<!_)_([^_]+)_(?!_)/g, '<em>$1</em>');
  return html;
}

/** Parse a markdown table block (lines starting with |). Returns HTML table. */
function renderTable(lines: string[]): string {
  if (lines.length < 2) return lines.map((l) => `<p>${renderInline(l)}</p>`).join('');

  const parseRow = (line: string): string[] =>
    line
      .split('|')
      .slice(1, -1)
      .map((cell) => cell.trim());

  const headerCells = parseRow(lines[0]);
  // Skip separator line (line[1] should be like |---|---|)
  const dataLines = lines.slice(2);

  let html = '<div class="md-table-wrap"><table class="md-table"><thead><tr>';
  for (const cell of headerCells) {
    html += `<th>${renderInline(cell)}</th>`;
  }
  html += '</tr></thead><tbody>';

  for (const line of dataLines) {
    const cells = parseRow(line);
    html += '<tr>';
    for (const cell of cells) {
      html += `<td>${renderInline(cell)}</td>`;
    }
    html += '</tr>';
  }

  html += '</tbody></table></div>';
  return html;
}

/**
 * Render safe markdown to HTML.
 * Only call this with AI-generated content, NEVER with user input.
 */
export function renderSafeMarkdown(text: string): string {
  const lines = text.split('\n');
  const output: string[] = [];
  let i = 0;

  while (i < lines.length) {
    const line = lines[i];
    const trimmed = line.trim();

    // Empty line → paragraph break
    if (trimmed === '') {
      i++;
      continue;
    }

    // Horizontal rule
    if (/^[-*_]{3,}$/.test(trimmed)) {
      output.push('<hr class="md-hr" />');
      i++;
      continue;
    }

    // Headers (## or ### — not h1, too large for chat)
    const headerMatch = trimmed.match(/^(#{1,4})\s+(.+)$/);
    if (headerMatch) {
      const level = Math.min(headerMatch[1].length + 1, 6); // ## → h3, ### → h4
      output.push(`<h${level} class="md-heading">${renderInline(headerMatch[2])}</h${level}>`);
      i++;
      continue;
    }

    // Table block (consecutive lines starting with |)
    if (trimmed.startsWith('|')) {
      const tableLines: string[] = [];
      while (i < lines.length && lines[i].trim().startsWith('|')) {
        tableLines.push(lines[i].trim());
        i++;
      }
      output.push(renderTable(tableLines));
      continue;
    }

    // Unordered list (- item or * item)
    if (/^[-*]\s+/.test(trimmed)) {
      const items: string[] = [];
      while (i < lines.length && /^\s*[-*]\s+/.test(lines[i])) {
        items.push(lines[i].replace(/^\s*[-*]\s+/, '').trim());
        i++;
      }
      output.push(
        '<ul class="md-list">' + items.map((item) => `<li>${renderInline(item)}</li>`).join('') + '</ul>',
      );
      continue;
    }

    // Ordered list (1. item)
    if (/^\d+\.\s+/.test(trimmed)) {
      const items: string[] = [];
      while (i < lines.length && /^\s*\d+\.\s+/.test(lines[i])) {
        items.push(lines[i].replace(/^\s*\d+\.\s+/, '').trim());
        i++;
      }
      output.push(
        '<ol class="md-list">' + items.map((item) => `<li>${renderInline(item)}</li>`).join('') + '</ol>',
      );
      continue;
    }

    // Regular paragraph
    output.push(`<p>${renderInline(trimmed)}</p>`);
    i++;
  }

  return output.join('');
}
