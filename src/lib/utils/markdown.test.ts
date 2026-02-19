import { describe, it, expect } from 'vitest';
import { renderSafeMarkdown } from './markdown';

describe('renderSafeMarkdown', () => {
  // Bold
  it('renders bold text', () => {
    expect(renderSafeMarkdown('**bold**')).toBe('<p><strong>bold</strong></p>');
  });

  it('renders __underline bold__', () => {
    expect(renderSafeMarkdown('__bold__')).toBe('<p><strong>bold</strong></p>');
  });

  // Italic
  it('renders italic text', () => {
    expect(renderSafeMarkdown('*italic*')).toBe('<p><em>italic</em></p>');
  });

  // Inline code
  it('renders inline code', () => {
    const result = renderSafeMarkdown('Value is `0.9 mg/dL`');
    expect(result).toContain('<code class="md-code">0.9 mg/dL</code>');
  });

  // Bold + italic together
  it('renders bold and italic in same line', () => {
    const result = renderSafeMarkdown('**bold** and *italic*');
    expect(result).toContain('<strong>bold</strong>');
    expect(result).toContain('<em>italic</em>');
  });

  // Headers
  it('renders ## header as h3', () => {
    const result = renderSafeMarkdown('## Key Points');
    expect(result).toBe('<h3 class="md-heading">Key Points</h3>');
  });

  it('renders ### header as h4', () => {
    const result = renderSafeMarkdown('### Details');
    expect(result).toBe('<h4 class="md-heading">Details</h4>');
  });

  // Unordered lists
  it('renders bullet list', () => {
    const result = renderSafeMarkdown('- Item 1\n- Item 2\n- Item 3');
    expect(result).toContain('<ul class="md-list">');
    expect(result).toContain('<li>Item 1</li>');
    expect(result).toContain('<li>Item 3</li>');
  });

  it('renders asterisk bullet list', () => {
    const result = renderSafeMarkdown('* Item A\n* Item B');
    expect(result).toContain('<ul class="md-list">');
    expect(result).toContain('<li>Item A</li>');
  });

  // Ordered lists
  it('renders numbered list', () => {
    const result = renderSafeMarkdown('1. First\n2. Second\n3. Third');
    expect(result).toContain('<ol class="md-list">');
    expect(result).toContain('<li>First</li>');
    expect(result).toContain('<li>Third</li>');
  });

  // Horizontal rule
  it('renders horizontal rule', () => {
    expect(renderSafeMarkdown('---')).toBe('<hr class="md-hr" />');
    expect(renderSafeMarkdown('***')).toBe('<hr class="md-hr" />');
  });

  // Tables
  it('renders simple table', () => {
    const input = '| Test | Value | Range |\n| --- | --- | --- |\n| Creatinine | 0.9 | 0.6-1.2 |';
    const result = renderSafeMarkdown(input);
    expect(result).toContain('<table class="md-table">');
    expect(result).toContain('<th>Test</th>');
    expect(result).toContain('<td>0.9</td>');
  });

  // XSS prevention (CRITICAL)
  it('escapes HTML script tags', () => {
    const result = renderSafeMarkdown('<script>alert(1)</script>');
    expect(result).not.toContain('<script>');
    expect(result).toContain('&lt;script&gt;');
  });

  it('escapes HTML in bold text', () => {
    const result = renderSafeMarkdown('**<img onerror=alert(1)>**');
    expect(result).not.toContain('<img');
    expect(result).toContain('&lt;img');
  });

  it('escapes HTML in list items', () => {
    const result = renderSafeMarkdown('- <a href="evil">click</a>');
    expect(result).not.toContain('<a href');
    expect(result).toContain('&lt;a href');
  });

  it('escapes HTML in table cells', () => {
    const input = '| Normal |\n| --- |\n| <script>x</script> |';
    const result = renderSafeMarkdown(input);
    expect(result).not.toContain('<script>');
  });

  // Mixed content
  it('renders mixed markdown content', () => {
    const input = '## Results\n\nYour creatinine is **0.9 mg/dL**.\n\n- Normal range: 0.6-1.2\n- Your value: normal';
    const result = renderSafeMarkdown(input);
    expect(result).toContain('<h3 class="md-heading">Results</h3>');
    expect(result).toContain('<strong>0.9 mg/dL</strong>');
    expect(result).toContain('<ul class="md-list">');
  });

  // Empty / plain text
  it('renders plain text as paragraph', () => {
    expect(renderSafeMarkdown('Hello world')).toBe('<p>Hello world</p>');
  });

  it('handles empty string', () => {
    expect(renderSafeMarkdown('')).toBe('');
  });

  // Graceful with malformed markdown
  it('handles unclosed bold gracefully', () => {
    const result = renderSafeMarkdown('**unclosed bold');
    expect(result).toContain('**unclosed bold');
  });
});
