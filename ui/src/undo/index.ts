// Undo/redo command pattern — both Rust-backed and local state

import { canUndo as apiCanUndo, canRedo as apiCanRedo, undo as apiUndo, redo as apiRedo } from '../tauri-api';

export interface Command {
  id: number;
  label: string;
  timestamp: number;
  execute: () => Promise<void>;
  undo: () => Promise<void>;
}

export class CommandHistory {
  private undoStack: Command[] = [];
  private redoStack: Command[] = [];
  private listeners: Array<() => void> = [];
  private nextId = 1;

  subscribe(listener: () => void): () => void {
    this.listeners.push(listener);
    return () => {
      const idx = this.listeners.indexOf(listener);
      if (idx >= 0) this.listeners.splice(idx, 1);
    };
  }

  private notify() {
    for (const listener of this.listeners) listener();
  }

  async execute(command: Omit<Command, 'id' | 'timestamp'>): Promise<void> {
    const cmd: Command = {
      ...command,
      id: this.nextId++,
      timestamp: Date.now(),
    };
    await cmd.execute();
    this.undoStack.push(cmd);
    this.redoStack = [];
    this.notify();
  }

  async undo(): Promise<Command | null> {
    if (this.undoStack.length === 0) return null;
    const cmd = this.undoStack.pop()!;
    await cmd.undo();
    this.redoStack.push(cmd);
    this.notify();
    return cmd;
  }

  async redo(): Promise<Command | null> {
    if (this.redoStack.length === 0) return null;
    const cmd = this.redoStack.pop()!;
    await cmd.execute();
    this.undoStack.push(cmd);
    this.notify();
    return cmd;
  }

  get canUndo(): boolean {
    return this.undoStack.length > 0;
  }

  get canRedo(): boolean {
    return this.redoStack.length > 0;
  }

  clear() {
    this.undoStack = [];
    this.redoStack = [];
    this.notify();
  }

  get history(): Command[] {
    return [...this.undoStack];
  }
}

// Global command history instance
export const commandHistory = new CommandHistory();
