// G-code preview with syntax highlighting and line-by-line stepping

import type { GCodeLine } from '../types';

// G-code syntax categories for highlighting
const GCODE_KEYWORDS = /\b(G0|G1|G28|G90|G91|G21|G92|M104|M109|M106|M107|M140|M190|M300|M301|M302|M303|T\d+)\b/;
const GCODE_PARAMS = /\b([XYZEFSIPRUT])([-\d.]+)\b/g;
const GCODE_COMMENT = /;.*$/;

export interface GCodePreviewOptions {
  onLineSelect?: (lineIndex: number, command: GCodeLine) => void;
  onStep?: (lineIndex: number) => void;
}

export class GCodePreview {
  private container: HTMLElement;
  private lines: GCodeLine[] = [];
  private currentLine: number = -1;
  private highlightedLine: number = -1;
  private options: GCodePreviewOptions;

  constructor(container: HTMLElement, options?: GCodePreviewOptions) {
    this.container = container;
    this.options = options ?? {};
    this.container.style.position = 'relative';
    this.container.style.overflow = 'auto';
    this.container.style.fontFamily = "'Cascadia Code', 'Fira Code', 'JetBrains Mono', 'Consolas', monospace";
    this.container.style.fontSize = '13px';
    this.container.style.lineHeight = '1.5';
    this.container.style.background = '#0d1117';
    this.container.style.borderRadius = '6px';
    this.container.style.padding = '8px 0';
  }

  setCode(code: string) {
    this.lines = this.parseGCode(code);
    this.currentLine = -1;
    this.highlightedLine = -1;
    this.render();
  }

  private parseGCode(code: string): GCodeLine[] {
    const rawLines = code.split('\n');
    return rawLines.map((text, i) => {
      const trimmed = text.trim();
      const commentMatch = trimmed.match(GCODE_COMMENT);
      if (commentMatch && commentMatch[0] === trimmed) {
        return { number: i, text: trimmed, type: 'comment' };
      }

      const cmdMatch = trimmed.match(/^(G0|G1|G28|M104|M109|M300|M301|M302|M303)\b/);
      const toolMatch = trimmed.match(/^T(\d+)\b/);

      let type: GCodeLine['type'] = 'other';
      if (cmdMatch) type = cmdMatch[1] as GCodeLine['type'];
      else if (toolMatch) type = 'T';

      return { number: i, text: trimmed, type };
    });
  }

  private escapeHtml(str: string): string {
    return str
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;');
  }

  private highlightLine(text: string): string {
    let html = this.escapeHtml(text);

    // Comments: gray italic
    html = html.replace(/(;.*)$/, '<span style="color:#8b949e;font-style:italic">$1</span>');

    // G-code keywords: bold cyan
    html = html.replace(GCODE_KEYWORDS, (match) => {
      const cls = match.startsWith('G') ? '#00d4ff' : '#ffa657';
      return `<span style="color:${cls};font-weight:bold">${match}</span>`;
    });

    // Parameters: colored by axis
    html = html.replace(GCODE_PARAMS, (match, axis: string, val: string) => {
      const axisColors: Record<string, string> = {
        X: '#ff7b72', Y: '#79c0ff', Z: '#7ee787',
        E: '#d2a8ff', F: '#ffa657',
        S: '#ffa657', P: '#ffa657',
        I: '#79c0ff', R: '#7ee787',
        U: '#d2a8ff', T: '#ffa657',
      };
      const color = axisColors[axis] || '#e6e6e6';
      return `<span style="color:${color}">${match}</span>`;
    });

    return html;
  }

  private render() {
    this.container.innerHTML = '';

    const table = document.createElement('div');
    table.style.display = 'table';
    table.style.width = '100%';
    table.style.borderCollapse = 'collapse';

    for (let i = 0; i < this.lines.length; i++) {
      const line = this.lines[i];
      const row = document.createElement('div');
      row.style.display = 'table-row';

      const isCurrent = i === this.currentLine;
      const isHighlighted = i === this.highlightedLine;
      const bg = isCurrent ? '#1f3a5f' : isHighlighted ? '#16213e' : 'transparent';
      const borderLeft = isCurrent ? '3px solid #00d4ff' : '3px solid transparent';

      row.style.background = bg;
      row.style.borderLeft = borderLeft;
      row.style.cursor = 'pointer';

      // Line number
      const numCell = document.createElement('div');
      numCell.style.display = 'table-cell';
      numCell.style.width = '48px';
      numCell.style.textAlign = 'right';
      numCell.style.padding = '0 12px 0 8px';
      numCell.style.color = '#484f58';
      numCell.style.userSelect = 'none';
      numCell.style.fontSize = '12px';
      numCell.textContent = String(i + 1);

      // Line content
      const contentCell = document.createElement('div');
      contentCell.style.display = 'table-cell';
      contentCell.style.padding = '0 8px';
      contentCell.style.whiteSpace = 'pre';
      contentCell.innerHTML = this.highlightLine(line.text);

      // Highlighted indicator
      if (isHighlighted) {
        const indicator = document.createElement('span');
        indicator.style.position = 'absolute';
        indicator.style.right = '8px';
        indicator.style.color = '#00d4ff';
        indicator.style.fontSize = '10px';
        indicator.textContent = '▶';
        contentCell.appendChild(indicator);
      }

      row.appendChild(numCell);
      row.appendChild(contentCell);

      row.addEventListener('click', () => {
        this.setCurrentLine(i);
        this.options.onLineSelect?.(i, line);
      });

      row.addEventListener('mouseenter', () => {
        if (i !== this.currentLine) {
          row.style.background = '#1c2333';
        }
      });
      row.addEventListener('mouseleave', () => {
        if (i !== this.currentLine) {
          row.style.background = bg;
        }
      });

      table.appendChild(row);
    }

    this.container.appendChild(table);
  }

  setCurrentLine(lineIndex: number) {
    this.currentLine = Math.max(-1, Math.min(lineIndex, this.lines.length - 1));
    this.render();
    this.scrollToLine(this.currentLine);
  }

  setHighlightedLine(lineIndex: number) {
    this.highlightedLine = Math.max(-1, Math.min(lineIndex, this.lines.length - 1));
    this.render();
  }

  stepForward(): boolean {
    if (this.currentLine < this.lines.length - 1) {
      this.setCurrentLine(this.currentLine + 1);
      this.options.onStep?.(this.currentLine);
      return true;
    }
    return false;
  }

  stepBackward(): boolean {
    if (this.currentLine > 0) {
      this.setCurrentLine(this.currentLine - 1);
      this.options.onStep?.(this.currentLine);
      return true;
    }
    return false;
  }

  goToLine(lineIndex: number) {
    this.setCurrentLine(lineIndex);
  }

  getCurrentLine(): number {
    return this.currentLine;
  }

  getTotalLines(): number {
    return this.lines.length;
  }

  getLineText(lineIndex: number): string {
    return this.lines[lineIndex]?.text ?? '';
  }

  private scrollToLine(lineIndex: number) {
    const rows = this.container.children;
    if (rows[lineIndex]) {
      rows[lineIndex].scrollIntoView({ block: 'nearest', behavior: 'smooth' });
    }
  }

  applyStepHighlight(code: string) {
    this.setCode(code);
  }

  dispose() {
    this.container.innerHTML = '';
    this.lines = [];
  }
}
